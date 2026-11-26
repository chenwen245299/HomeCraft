//! Read-only adapter for DeepSeek's documented JSONL persistence and tokenUsage fold.
//! Never opens credentials, provider settings, prompt exports, or remote account endpoints.
use crate::usage::{self, Reading};
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

pub(super) mod ledger;
use ledger::Call;
pub use ledger::Ledger;

pub const PROVIDER: u8 = 4;
pub const NAME: &str = "DeepSeek Harness";
const LIMIT: u64 = 9_007_199_254_740_991;
const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_FRAME: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Origin {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceUsage {
    pub id: String,
    pub name: String,
    pub t: i64,
    pub d: [i64; 2],
    pub wt: [i64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mt: Option<[i64; 2]>,
    pub u: [i64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger: Option<Ledger>,
}
impl DeviceUsage {
    pub fn packet(&self) -> Result<usage::Packet> {
        ensure!(self.valid(), "Harness 设备缓存无效");
        let mut p = usage::Packet::new(PROVIDER, usage::OK);
        p.t = self.t;
        p.n = Some(NAME.into());
        p.o = Some(Origin {
            id: self.id.clone(),
            name: self.name.clone(),
        });
        p.d = Some(self.d);
        p.wt = Some(self.wt);
        p.mt = self.mt;
        p.u = Some([self.u[0], self.u[1], self.u[2], self.u[3], self.u[3]]);
        p.encode()?;
        Ok(p)
    }
    pub fn valid(&self) -> bool {
        let origin = Origin {
            id: self.id.clone(),
            name: self.name.clone(),
        };
        let date = |d: i64| {
            NaiveDate::from_ymd_opt((d / 10000) as i32, (d / 100 % 100) as u32, (d % 100) as u32)
        };
        origin.valid()
            && (1735689600..=4102444800).contains(&self.t)
            && date(self.d[0]).zip(date(self.wt[0])).is_some_and(|(d, w)| {
                d - chrono::Duration::days(i64::from(d.weekday().num_days_from_monday())) == w
            })
            && self.d[1] >= 0
            && self.wt[1] >= self.d[1]
            && self.wt[1] <= LIMIT as i64 / 8
            && self.mt.is_none_or(|m| {
                date(m[0]).zip(date(self.d[0])).is_some_and(|(month, day)| {
                    month.day() == 1 && month.year() == day.year() && month.month() == day.month()
                }) && m[1] >= self.d[1]
                    && m[1] <= LIMIT as i64 / 8
            })
            && self.u.iter().all(|n| *n >= 0 && *n <= LIMIT as i64 / 8)
            && self.u[0].checked_add(self.u[1]) == Some(self.d[1])
            && self.u[2] <= self.u[0]
            && self.ledger.as_ref().is_none_or(|ledger| {
                ledger.valid()
                    && ledger.day == self.d[0]
                    && (ledger.conflict
                        || ledger.ambiguous()
                        || ledger.days().is_ok_and(|days| {
                            reading(&days, ledger::date(ledger.day).unwrap(), origin.clone())
                                .is_ok_and(|r| {
                                    r.packet.d == Some(self.d)
                                        && r.packet.wt == Some(self.wt)
                                        && r.packet.mt == self.mt
                                        && r.packet.u.map(|u| [u[0], u[1], u[2], u[3]])
                                            == Some(self.u)
                                })
                        }))
            })
    }
    pub fn from_reading(r: &Reading) -> Option<Self> {
        let o = r.packet.o.as_ref()?;
        let u = r.packet.u?;
        let result = Self {
            id: o.id.clone(),
            name: o.name.clone(),
            t: r.packet.t,
            d: r.packet.d?,
            wt: r.packet.wt?,
            mt: r.packet.mt,
            u: [u[0], u[1], u[2], u[3]],
            ledger: r.tokens.harness_ledger.clone(),
        };
        (r.packet.s == usage::OK && result.valid()).then_some(result)
    }
}
pub fn effective(local: &Reading, devices: &[DeviceUsage]) -> Reading {
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    effective_on(local, devices, today)
}
pub(crate) fn effective_on(local: &Reading, devices: &[DeviceUsage], today: NaiveDate) -> Reading {
    effective_inner(local, devices, today, true)
}
fn effective_inner(
    local: &Reading,
    devices: &[DeviceUsage],
    today: NaiveDate,
    device_views: bool,
) -> Reading {
    if local.packet.p != PROVIDER {
        return local.clone();
    }
    let mut devices = merge_devices(devices, &local.tokens.devices);
    // An already resolved reading must never become a new local observation.
    if local.tokens.devices.is_empty()
        && let Some(current) = DeviceUsage::from_reading(local)
    {
        devices = merge_devices(&devices, &[current]);
    }
    if devices.is_empty() {
        return local.clone();
    }
    let ledgers = devices
        .iter()
        .filter_map(|d| d.ledger.as_ref())
        .collect::<Vec<_>>();
    let mut r = if ledgers.is_empty() {
        let mut r = usage::empty(PROVIDER, 1, "当前只有旧版总数，缺少可去重的调用明细");
        r.packet.n = Some(NAME.into());
        r
    } else {
        match Ledger::periods(today, ledgers.iter().copied()).and_then(|periods| {
            let origin = local.packet.o.clone().unwrap_or_else(|| Origin {
                id: devices[0].id.clone(),
                name: devices[0].name.clone(),
            });
            let mut r = reading(&BTreeMap::new(), today, origin)?;
            let monday =
                today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday()));
            let [daily, weekly, monthly] = periods.values;
            r.packet.d = daily.map(|d| [day_id(today), d.total() as i64]);
            r.packet.wt = weekly.map(|d| [day_id(monday), d.total() as i64]);
            r.packet.mt = monthly.map(|d| [day_id(today.with_day(1).unwrap()), d.total() as i64]);
            r.packet.u = daily.map(|d| {
                [
                    d.input as i64,
                    d.output as i64,
                    d.cached as i64,
                    d.calls as i64,
                    d.calls as i64,
                ]
            });
            r.tokens.recent_day = daily.map(Day::total);
            r.tokens.weekly = weekly.map(Day::total);
            r.tokens.monthly = monthly.map(Day::total);
            r.tokens.input = daily.map(|d| d.input);
            r.tokens.output = daily.map(|d| d.output);
            r.tokens.cached = daily.map(|d| d.cached);
            r.tokens.calls = daily.map(|d| d.calls);
            let affected = ["今日", "本周", "本月"]
                .into_iter()
                .zip(periods.disputed)
                .filter_map(|(label, disputed)| disputed.then_some(label))
                .collect::<Vec<_>>();
            r.tokens.note = if affected.is_empty() {
                String::new()
            } else {
                format!(
                    "{}调用记录有分歧，数值待核对{}",
                    affected.join("、"),
                    if affected.len() < 3 {
                        "；其他周期照常显示"
                    } else {
                        ""
                    }
                )
            };
            if periods.disputed.iter().any(|d| *d) && periods.values.iter().all(Option::is_none) {
                r.packet.s = 6;
                r.source = r.tokens.note.clone();
            }
            r.packet.encode()?;
            Ok(r)
        }) {
            Ok(r) => r,
            Err(e) => {
                let mut r = usage::empty(PROVIDER, 6, &e.to_string());
                r.packet.n = Some(NAME.into());
                r
            }
        }
    };
    let legacy = devices
        .iter()
        .filter(|d| d.ledger.is_none() && d.d[0] >= day_id(today.with_day(1).unwrap()))
        .count();
    let note = std::mem::take(&mut r.tokens.note);
    r.tokens.note = format!("{} 台 Mac 观测 · 调用去重 · UTC+8", ledgers.len());
    if legacy > 0 {
        r.tokens
            .note
            .push_str(&format!("；{legacy} 台仅有旧版总数，未计入"));
    }
    if !note.is_empty() {
        r.tokens.note.push_str(&format!("；{note}"));
    }
    if device_views {
        for device in &devices {
            if device.ledger.is_none() {
                if let Ok(packet) = device.packet() {
                    r.tokens.device_periods.insert(device.id.clone(), packet);
                }
                continue;
            }
            let empty = usage::empty(PROVIDER, 0, "");
            let view = effective_inner(&empty, std::slice::from_ref(device), today, false);
            r.tokens
                .device_periods
                .insert(device.id.clone(), view.packet);
        }
    }
    r.tokens.devices = devices;
    r
}
pub fn merge_devices(a: &[DeviceUsage], b: &[DeviceUsage]) -> Vec<DeviceUsage> {
    let mut sources = BTreeMap::<String, DeviceUsage>::new();
    for d in a.iter().chain(b).filter(|d| d.valid()) {
        if let Some(old) = sources.get_mut(&d.id) {
            if old.ledger.is_some() && d.ledger.is_none() {
                // Legacy totals cannot change an identity-backed observation. In particular,
                // old daily cloud snapshots must not refold its entire call history.
                continue;
            }
            let ledger = match (&old.ledger, &d.ledger) {
                (Some(a), Some(b)) => Some(a.merge(b)),
                (a, b) => a.clone().or_else(|| b.clone()),
            };
            // A device-only total can never supersede verified attempt identities.
            if (d.ledger.is_some(), d.t, d.mt.is_some(), &d.name)
                > (old.ledger.is_some(), old.t, old.mt.is_some(), &old.name)
            {
                *old = d.clone();
            }
            if let Some(ledger) = ledger {
                if let Ok(days) = ledger.days()
                    && let Ok(mut r) = reading(
                        &days,
                        ledger::date(ledger.day).unwrap(),
                        Origin {
                            id: old.id.clone(),
                            name: old.name.clone(),
                        },
                    )
                {
                    r.packet.t = old.t;
                    r.tokens.harness_ledger = Some(ledger.clone());
                    if let Some(merged) = DeviceUsage::from_reading(&r) {
                        *old = merged;
                    }
                }
                old.ledger = Some(ledger);
            }
        } else {
            sources.insert(d.id.clone(), d.clone());
        }
    }
    // Prefer sources with call identities if an old display has a full eight-entry cache.
    let mut sources = sources.into_values().collect::<Vec<_>>();
    sources.sort_by(|a, b| {
        b.ledger
            .is_some()
            .cmp(&a.ledger.is_some())
            .then(a.id.cmp(&b.id))
    });
    sources.truncate(8);
    sources
}
/// A per-Mac-only display cannot deduplicate shared conversations. Send no source totals.
/// Aggregate-capable displays receive the resolved result, without a machine origin.
pub fn display_packets(reading: &Reading, per_mac: bool) -> Result<Vec<usage::Packet>> {
    if reading.packet.p == PROVIDER && per_mac {
        return Ok(vec![]);
    }
    let mut packet = reading.packet.clone();
    if packet.p == PROVIDER {
        if packet.s != usage::OK {
            // Older aggregate receivers keep previous numbers on a transient error.
            // Explicitly withdraw this derived value, while the App keeps the real reason.
            let mut clear = usage::Packet::new(PROVIDER, 0);
            clear.t = packet.t;
            clear.n = Some(NAME.into());
            packet = clear;
        }
        packet.o = None;
        if packet.s == usage::OK && packet.d.is_none() {
            packet.d = Some([
                day_id((chrono::Utc::now() + chrono::Duration::hours(8)).date_naive()),
                -1,
            ]);
        }
    }
    packet.encode()?;
    Ok(vec![packet])
}
pub fn load_devices(root: &Path) -> Vec<DeviceUsage> {
    let current = crate::config::read_private_bounded::<Vec<DeviceUsage>>(
        &root.join(".harness-devices-v4.json"),
        192 * 1024 * 1024,
    )
    .or_else(|_| {
        crate::config::read_private_bounded(
            &root.join(".harness-devices-v3.json"),
            192 * 1024 * 1024,
        )
    });
    let migrating = current.is_err();
    let devices = current
        .or_else(|_| {
            crate::config::read_private_bounded(
                &root.join(".harness-devices-v2.json"),
                192 * 1024 * 1024,
            )
        })
        .or_else(|_| crate::config::read_private(&root.join(".harness-devices.json")))
        .unwrap_or_default();
    if devices.len() > 8 || devices.iter().any(|d| !d.valid()) {
        return Vec::new();
    }
    merge_devices(
        &[],
        &devices
            .into_iter()
            .filter(|d| !migrating || !d.ledger.as_ref().is_some_and(|l| l.conflict))
            .collect::<Vec<_>>(),
    )
}
pub fn save_devices(root: &Path, devices: &[DeviceUsage]) -> Result<()> {
    use std::io::Write;
    ensure!(
        devices.len() <= 8 && devices.iter().all(DeviceUsage::valid),
        "Harness 设备缓存无效"
    );
    let tmp = root.join(format!(
        ".harness-devices-{:016x}.tmp",
        rand::random::<u64>()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)?;
        let bytes = serde_json::to_vec(devices)?;
        ensure!(bytes.len() <= 192 * 1024 * 1024, "Harness 设备缓存过长");
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, root.join(".harness-devices-v4.json"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
impl Origin {
    pub fn valid(&self) -> bool {
        self.id.len() == 16
            && self
                .id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            && !self.name.is_empty()
            && self.name.len() <= 24
            && self
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b" ._-".contains(&c))
    }
    pub fn local(root: &Path) -> Result<Self> {
        use std::io::Write;
        let path = root.join(".harness-origin.json");
        use sha2::{Digest, Sha256};
        unsafe extern "C" {
            fn gethostuuid(id: *mut u8, wait: *const libc::timespec) -> libc::c_int;
        }
        let mut uuid = [0u8; 16];
        let wait = libc::timespec {
            tv_sec: 1,
            tv_nsec: 0,
        };
        ensure!(
            unsafe { gethostuuid(uuid.as_mut_ptr(), &wait) } == 0,
            "无法取得本机设备标识，请稍后重试"
        );
        let mut digest = Sha256::new();
        digest.update(b"HomeCraft/harness-device/v1");
        digest.update(uuid);
        digest.update(unsafe { libc::getuid() }.to_le_bytes());
        let stable_id = digest.finalize()[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if path.exists() {
            let origin: Self = crate::config::read_private(&path)?;
            ensure!(origin.valid(), "Harness 设备标识无效");
            ensure!(
                origin.id == stable_id,
                "Harness 设备标识属于另一台 Mac，请重新生成本机统计配置"
            );
            return Ok(origin);
        }
        let mut hostname = [0u8; 256];
        // A display label only. Authentication remains Security 2; this is not a credential.
        unsafe {
            libc::gethostname(hostname.as_mut_ptr().cast(), hostname.len());
        }
        let label = String::from_utf8_lossy(&hostname)
            .split('\0')
            .next()
            .unwrap_or("Mac")
            .trim_end_matches(".local")
            .bytes()
            .filter(|c| c.is_ascii_alphanumeric() || b" ._-".contains(c))
            .take(24)
            .map(char::from)
            .collect::<String>();
        let origin = Self {
            id: stable_id,
            name: if label.is_empty() {
                "Mac".into()
            } else {
                label
            },
        };
        let temporary = root.join(format!(
            ".harness-origin-{:016x}.tmp",
            rand::random::<u64>()
        ));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(&origin)?)?;
        file.sync_all()?;
        // Atomic no-overwrite publication also makes two simultaneous first reads agree.
        let published = fs::hard_link(&temporary, &path);
        let _ = fs::remove_file(&temporary);
        if let Err(e) = published {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                let saved: Self = crate::config::read_private(&path)?;
                ensure!(saved.valid(), "Harness 设备标识无效");
                return Ok(saved);
            }
            return Err(e.into());
        }
        Ok(origin)
    }
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Counters {
    input_tokens: u64,
    output_tokens: u64,
    #[serde(default)]
    cache_read_tokens: u64,
    #[serde(default)]
    cache_write_tokens: u64,
}
impl Counters {
    fn summary(self) -> Result<Day> {
        let input = self
            .input_tokens
            .checked_add(self.cache_read_tokens)
            .and_then(|n| n.checked_add(self.cache_write_tokens));
        let input = input
            .filter(|n| *n <= LIMIT)
            .ok_or_else(|| anyhow::anyhow!("Harness 输入计数越界"))?;
        ensure!(
            input
                .checked_add(self.output_tokens)
                .is_some_and(|n| n <= LIMIT),
            "Harness 总计数越界"
        );
        Ok(Day {
            input,
            output: self.output_tokens,
            cached: self.cache_read_tokens,
            calls: 1,
        })
    }
}
#[derive(Default, Deserialize)]
struct Chunk {
    #[serde(rename = "type")]
    kind: String,
    usage: Option<Counters>,
}
#[derive(Default, Deserialize)]
struct Stream {
    #[serde(rename = "type")]
    kind: String,
    time: Option<i64>,
    chunk: Option<Chunk>,
}
#[derive(Default, Deserialize)]
struct Data {
    turn: Option<u64>,
    step: Option<u64>,
    #[serde(default)]
    inherited: bool,
    usage: Option<Counters>,
    #[serde(default)]
    stream: Vec<Stream>,
}
#[derive(Deserialize)]
struct Row {
    #[serde(rename = "type")]
    kind: String,
    version: Option<u32>,
    id: Option<String>,
    #[serde(rename = "isSeeded")]
    seeded: Option<bool>,
    seq: Option<u64>,
    time: Option<i64>,
    #[serde(default)]
    data: Data,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Day {
    input: u64,
    output: u64,
    cached: u64,
    calls: u64,
}
impl Day {
    fn total(self) -> u64 {
        self.input + self.output
    }
    fn add(&mut self, other: Self) -> Result<()> {
        let add = |a: u64, b: u64| {
            a.checked_add(b)
                .filter(|n| *n <= LIMIT)
                .ok_or_else(|| anyhow::anyhow!("Harness 汇总计数越界"))
        };
        self.input = add(self.input, other.input)?;
        self.output = add(self.output, other.output)?;
        self.cached = add(self.cached, other.cached)?;
        self.calls = add(self.calls, other.calls)?;
        ensure!(self.total() <= LIMIT, "Harness 汇总计数越界");
        Ok(())
    }
    fn sub(&mut self, other: Self) {
        self.input -= other.input;
        self.output -= other.output;
        self.cached -= other.cached;
        self.calls -= other.calls;
    }
}
#[derive(Clone, Default)]
struct Session {
    id: String,
    days: BTreeMap<NaiveDate, Day>,
    calls: BTreeMap<String, Call>,
}
#[derive(Default)]
struct Fold {
    result: Session,
    version: u32,
    seq: u64,
    seeded: bool,
    marker: bool,
    last: Option<((u64, u64), NaiveDate, Day)>,
    retries: HashMap<(u64, u64), u64>,
}
impl Fold {
    fn row(&mut self, row: Row) -> Result<()> {
        if self.result.id.is_empty() {
            ensure!(
                row.kind == "session" && row.version.is_some_and(|v| (2..=4).contains(&v)),
                "Harness 日志格式暂不支持，请更新 HomeCraft"
            );
            self.result.id = row
                .id
                .filter(|id| !id.is_empty() && id.len() <= 256)
                .ok_or_else(|| anyhow::anyhow!("Harness 日志标头无效"))?;
            self.version = row.version.unwrap();
            self.seeded = row
                .seeded
                .ok_or_else(|| anyhow::anyhow!("Harness 日志标头无效"))?;
            return Ok(());
        }
        ensure!(row.seq == Some(self.seq), "Harness 日志序号不连续");
        self.seq += 1;
        if row.kind == "session/end-seed" && row.data.inherited {
            ensure!(self.seeded, "Harness 继承边界无效");
            self.result.days.clear();
            self.result.calls.clear();
            self.retries.clear();
            self.last = None;
            self.marker = true;
            return Ok(());
        }
        if row.kind == "llm/retry-started" {
            let key = row
                .data
                .turn
                .zip(row.data.step)
                .ok_or_else(|| anyhow::anyhow!("Harness 重试缺少轮次或步骤"))?;
            *self.retries.entry(key).or_default() += 1;
            if self.last.is_some_and(|(key, _, _)| {
                Some(key.0) == row.data.turn && Some(key.1) == row.data.step
            }) {
                self.last = None;
            }
            return Ok(());
        }
        if row.kind != "assistant/message" && row.kind != "assistant/attempt" {
            return Ok(());
        }
        let sample = row.data.usage.or_else(|| {
            row.data
                .stream
                .iter()
                .rev()
                .filter(|s| s.kind == "chunk")
                .filter_map(|s| s.chunk.as_ref())
                .find(|c| c.kind == "usage")
                .and_then(|c| c.usage)
        });
        let Some(sample) = sample else { return Ok(()) };
        let original_time = row
            .data
            .stream
            .iter()
            .rev()
            .find(|r| {
                r.kind == "chunk"
                    && r.chunk
                        .as_ref()
                        .is_some_and(|c| c.kind == "usage" && c.usage == Some(sample))
            })
            .and_then(|r| r.time);
        let millis = original_time
            .or(row.time)
            .ok_or_else(|| anyhow::anyhow!("Harness 用量缺少时间"))?;
        let date = chrono::DateTime::from_timestamp_millis(millis)
            .and_then(|t| t.checked_add_signed(chrono::Duration::hours(8)))
            .ok_or_else(|| anyhow::anyhow!("Harness 时间无效"))?
            .date_naive();
        ensure!((2025..=2099).contains(&date.year()), "Harness 时间越界");
        let key = (
            row.data
                .turn
                .ok_or_else(|| anyhow::anyhow!("Harness 用量缺少轮次"))?,
            row.data
                .step
                .ok_or_else(|| anyhow::anyhow!("Harness 用量缺少步骤"))?,
        );
        if let Some((old_key, old_day, old)) = self.last
            && old_key == key
        {
            self.result.days.get_mut(&old_day).unwrap().sub(old);
        }
        let value = sample.summary()?;
        let identity = Call::key(
            &self.result.id,
            key.0,
            key.1,
            self.retries.get(&key).copied().unwrap_or(0),
        );
        ensure!(
            !self.result.calls.contains_key(&identity)
                || self.last.is_some_and(|(old, _, _)| old == key),
            "Harness 调用顺序冲突，等待核对"
        );
        let revision = self
            .result
            .calls
            .get(&identity)
            .map_or(1, |c| c.revision + 1);
        self.result.calls.insert(
            identity.clone(),
            Call {
                through: None,
                usage_time: original_time.is_some(),
                retain_day: if original_time.is_some() {
                    row.time
                        .and_then(chrono::DateTime::from_timestamp_millis)
                        .map(|t| (t + chrono::Duration::hours(8)).date_naive())
                        .filter(|d| *d != date)
                        .map(day_id)
                } else {
                    None
                },
                key: identity,
                at: millis,
                revision,
                settled: row.kind == "assistant/message",
                tokens: [value.input, value.output, value.cached],
            },
        );
        self.result.days.entry(date).or_default().add(value)?;
        self.last = Some((key, date, value));
        Ok(())
    }
    fn finish(self) -> Result<Session> {
        ensure!(
            !self.result.id.is_empty() && (!self.seeded || self.marker),
            "Harness 日志缺少继承边界"
        );
        Ok(self.result)
    }
}

// Complete-frame boundaries follow the documented first-party JSONL Zstandard container.
// A concurrently appended, incomplete final frame is ignored; checksum/corruption is never ignored.
fn frame_end(bytes: &[u8], start: usize) -> Result<Option<usize>> {
    let mut p = start;
    if bytes.len() - p < 4 {
        return Ok(None);
    }
    ensure!(
        bytes[p..p + 4] == [0x28, 0xb5, 0x2f, 0xfd],
        "Harness 压缩帧损坏"
    );
    p += 4;
    let Some(&descriptor) = bytes.get(p) else {
        return Ok(None);
    };
    p += 1;
    ensure!(descriptor & 0x18 == 0, "Harness 压缩帧无效");
    let single = descriptor & 0x20 != 0;
    let flag = descriptor >> 6;
    let dict = match descriptor & 3 {
        3 => 4,
        n => n as usize,
    };
    p += usize::from(!single)
        + dict
        + if flag == 0 {
            usize::from(single)
        } else {
            1usize << flag
        };
    if p > bytes.len() {
        return Ok(None);
    }
    loop {
        if bytes.len() - p < 3 {
            return Ok(None);
        }
        let header = u32::from_le_bytes([bytes[p], bytes[p + 1], bytes[p + 2], 0]);
        p += 3;
        let kind = (header >> 1) & 3;
        ensure!(kind != 3, "Harness 压缩块损坏");
        p += if kind == 1 { 1 } else { (header >> 3) as usize };
        if p > bytes.len() {
            return Ok(None);
        }
        if header & 1 != 0 {
            break;
        }
    }
    if descriptor & 4 != 0 {
        p += 4;
    }
    Ok((p <= bytes.len()).then_some(p))
}
fn lines(fold: &mut Fold, bytes: &[u8]) -> Result<()> {
    for line in bytes.split_inclusive(|c| *c == b'\n') {
        if line.last() != Some(&b'\n') {
            break;
        }
        ensure!(line.len() <= 16 * 1024 * 1024, "Harness 单条记录过长");
        let row: Row =
            serde_json::from_slice(line).map_err(|_| anyhow::anyhow!("Harness 日志格式无效"))?;
        fold.row(row)?;
    }
    Ok(())
}
fn parse(bytes: &[u8], compressed: bool) -> Result<Session> {
    let mut fold = Fold::default();
    if compressed {
        let mut pos = 0;
        while pos < bytes.len() {
            let Some(end) = frame_end(bytes, pos)? else {
                break;
            };
            let mut plain = Vec::new();
            zstd::stream::read::Decoder::new(&bytes[pos..end])?
                .take(MAX_FRAME + 1)
                .read_to_end(&mut plain)?;
            ensure!(plain.len() as u64 <= MAX_FRAME, "Harness 压缩帧展开过长");
            ensure!(plain.last() == Some(&b'\n'), "Harness 完整压缩帧缺少行边界");
            lines(&mut fold, &plain)?;
            pos = end;
        }
    } else {
        lines(&mut fold, bytes)?
    }
    fold.finish()
}
fn generation(name: &str) -> Option<(u32, bool)> {
    let (name, zstd) = name
        .strip_suffix(".zstd")
        .map_or((name, false), |s| (s, true));
    if name == "session.jsonl" {
        return Some((0, zstd));
    }
    let digits = name.strip_prefix("session.v")?.strip_suffix(".jsonl")?;
    if digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((digits.parse().ok()?, zstd))
}
#[derive(Clone)]
struct Cached {
    len: u64,
    modified: SystemTime,
    inode: u64,
    device: u64,
    changed: (i64, i64),
    session: Session,
}
fn scan(root: &Path, cache: &mut HashMap<PathBuf, Cached>, today: NaiveDate) -> Result<Ledger> {
    scan_with_references(root, cache, today, &HashSet::new())
}
fn scan_with_references(
    root: &Path,
    cache: &mut HashMap<PathBuf, Cached>,
    today: NaiveDate,
    referenced: &HashSet<String>,
) -> Result<Ledger> {
    let mut found = HashSet::new();
    let mut calls = Vec::new();
    let mut count = 0;
    for project in fs::read_dir(root)? {
        let project = project?;
        if !project.file_type()?.is_dir() {
            continue;
        }
        for directory in fs::read_dir(project.path())? {
            let directory = directory?;
            if !directory.file_type()?.is_dir() {
                continue;
            }
            let mut selected: Option<(u32, bool, PathBuf)> = None;
            for file in fs::read_dir(directory.path())? {
                let file = file?;
                if !file.file_type()?.is_file() {
                    continue;
                }
                let Some((version, compressed)) = generation(&file.file_name().to_string_lossy())
                else {
                    continue;
                };
                if let Some((v, _, _)) = &selected {
                    ensure!(*v != version, "Harness 同一日志存在重复格式");
                    if *v > version {
                        continue;
                    }
                }
                selected = Some((version, compressed, file.path()));
            }
            let Some((version, compressed, path)) = selected else {
                continue;
            };
            count += 1;
            ensure!(
                count <= 10000 && (2..=4).contains(&version),
                "Harness 日志数量或格式暂不支持"
            );
            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)?;
            let meta = file.metadata()?;
            ensure!(meta.len() <= MAX_FILE, "Harness 单个日志过大");
            let modified = meta.modified()?;
            let session = if let Some(saved) = cache.get(&path).filter(|c| {
                c.len == meta.len()
                    && c.modified == modified
                    && c.inode == meta.ino()
                    && c.device == meta.dev()
                    && c.changed == (meta.ctime(), meta.ctime_nsec())
            }) {
                saved.session.clone()
            } else {
                let mut bytes = Vec::new();
                file.take(MAX_FILE + 1).read_to_end(&mut bytes)?;
                ensure!(bytes.len() as u64 <= MAX_FILE, "Harness 单个日志过大");
                let session = parse(&bytes, compressed)?;
                cache.insert(
                    path.clone(),
                    Cached {
                        len: meta.len(),
                        modified,
                        inode: meta.ino(),
                        device: meta.dev(),
                        changed: (meta.ctime(), meta.ctime_nsec()),
                        session: session.clone(),
                    },
                );
                session
            };
            found.insert(path);
            calls.extend(session.calls.into_values().map(|mut call| {
                if call.usage_time
                    && referenced.contains(&call.key)
                    && call.date().is_some_and(|d| d < ledger::first(today))
                {
                    call.retain_day = Some(day_id(today));
                }
                call
            }));
        }
    }
    cache.retain(|p, _| found.contains(p));
    Ledger::new(today, calls)
}
fn day_id(day: NaiveDate) -> i64 {
    i64::from(day.year()) * 10000 + i64::from(day.month()) * 100 + i64::from(day.day())
}
fn reading(days: &BTreeMap<NaiveDate, Day>, today: NaiveDate, origin: Origin) -> Result<Reading> {
    let day = days.get(&today).copied().unwrap_or_default();
    let monday = today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let mut week = Day::default();
    let first = today.with_day(1).unwrap();
    let mut month = Day::default();
    for (_, value) in days.range(monday..=today) {
        week.add(*value)?;
    }
    for (_, value) in days.range(first..=today) {
        month.add(*value)?;
    }
    let mut r = usage::empty(PROVIDER, usage::OK, "DeepSeek Harness 官方本地日志");
    r.packet.n = Some(NAME.into());
    r.packet.o = Some(origin.clone());
    r.packet.d = Some([day_id(today), day.total() as i64]);
    r.packet.wt = Some([day_id(monday), week.total() as i64]);
    r.packet.mt = Some([day_id(first), month.total() as i64]);
    r.packet.u = Some([
        day.input as i64,
        day.output as i64,
        day.cached as i64,
        day.calls as i64,
        day.calls as i64,
    ]);
    r.tokens.recent_day = Some(day.total());
    r.tokens.day = Some(today.to_string());
    r.tokens.weekly = Some(week.total());
    r.tokens.monthly = Some(month.total());
    r.tokens.week_start = Some(monday.to_string());
    r.tokens.input = Some(day.input);
    r.tokens.output = Some(day.output);
    r.tokens.cached = Some(day.cached);
    r.tokens.calls = Some(day.calls);
    r.tokens.note = format!("{} · 本机记录 · UTC+8；缓存已含在输入中", origin.name);
    r.packet.encode()?;
    Ok(r)
}
pub async fn collect(root: &Path) -> Reading {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || {
        static CACHE: OnceLock<Mutex<HashMap<PathBuf, Cached>>> = OnceLock::new();
        let home = std::env::var_os("HOME");
        let sessions = std::env::var_os("HOMECRAFT_HARNESS_SESSIONS")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| PathBuf::from(h).join(".dsh/sessions")));
        let mut result = usage::empty(PROVIDER, 0, "尚无本机 Harness 记录");
        result.packet.n = Some(NAME.into());
        let Some(sessions) = sessions.filter(|s| s.is_dir()) else {
            return result;
        };
        let read: Result<Reading> = (|| {
            let origin = Origin::local(&root)?;
            let mut cache = CACHE
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .map_err(|_| anyhow::anyhow!("Harness 缓存不可用"))?;
            let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
            // Old clients can publish an import date in the current period for an
            // original call from a previous month. Keep only the referenced original
            // evidence needed to correct those keys; these are still local observations.
            let referenced = load_devices(&root)
                .into_iter()
                .chain(crate::cloud::cached_harness(&root))
                .filter_map(|d| d.ledger)
                .flat_map(|l| {
                    l.calls
                        .iter()
                        .filter(|c| !c.usage_time)
                        .map(|c| c.key.clone())
                        .collect::<Vec<_>>()
                })
                .collect::<HashSet<_>>();
            let ledger = if referenced.is_empty() {
                scan(&sessions, &mut cache, today)?
            } else {
                scan_with_references(&sessions, &mut cache, today, &referenced)?
            };
            let mut result = reading(&ledger.days()?, today, origin)?;
            result.tokens.harness_ledger = Some(ledger);
            Ok(result)
        })();
        read.unwrap_or_else(|e| {
            result.packet.s = 6;
            result.source = e.to_string();
            result
        })
    })
    .await
    .unwrap_or_else(|_| {
        let mut r = usage::empty(PROVIDER, 6, "Harness 读取任务失败");
        r.packet.n = Some(NAME.into());
        r
    })
}

#[cfg(test)]
pub(crate) fn test_reading(id: &str, n: u64) -> Reading {
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    test_reading_on(id, n, today, today)
}
#[cfg(test)]
pub(crate) fn test_reading_on(id: &str, n: u64, today: NaiveDate, billed: NaiveDate) -> Reading {
    let call = Call {
        through: None,
        usage_time: false,
        retain_day: None,
        key: Call::key(id, 1, 1, 0),
        at: billed
            .and_hms_opt(4, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis(),
        revision: 1,
        settled: true,
        tokens: [n, 0, 0],
    };
    let ledger = Ledger::new(today, [call]).unwrap();
    let mut r = reading(
        &ledger.days().unwrap(),
        today,
        Origin {
            id: id.into(),
            name: "Test Mac".into(),
        },
    )
    .unwrap();
    r.tokens.harness_ledger = Some(ledger);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 21).unwrap()
    }
    fn header(seeded: bool) -> Value {
        json!({"type":"session","version":4,"id":"synthetic-session","isSeeded":seeded})
    }
    fn live_sync_fixture() -> Vec<Value> {
        // Produced by the installed iCloud plugin's real appendOpen adapter, with a
        // synthetic Session implementing the SDK's Date.now() envelope stamping.
        serde_json::from_str(include_str!("../tests/harness-live-sync.json")).unwrap()
    }
    #[test]
    fn live_icloud_handoff_uses_original_usage_time_across_month_and_week_boundary() {
        let rows = live_sync_fixture();
        let session = parse(&encode(&rows), false).unwrap();
        let call = session.calls.into_values().next().unwrap();
        assert_eq!(call.at, 1780243198000); // May 31 UTC+8, not the June 1 live append.
        assert!(call.usage_time);
        assert_eq!(call.retain_day, Some(20260601));
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let mut old = call.clone();
        old.at = rows[1]["time"].as_i64().unwrap();
        old.usage_time = false;
        old.retain_day = None;
        let original = Ledger::new(today, [call]).unwrap();
        let legacy = Ledger::new(today, [old.clone()]).unwrap();
        let merged = original.merge(&legacy);
        assert!(!merged.conflict && !merged.ambiguous());
        assert_eq!(merged.version, 3);
        assert_eq!(merged, legacy.merge(&original));
        let restarted: Ledger =
            serde_json::from_slice(&serde_json::to_vec(&merged).unwrap()).unwrap();
        assert_eq!(
            Ledger::periods(today, [&restarted, &legacy])
                .unwrap()
                .values
                .map(|v| v.map(Day::total)),
            [Some(0); 3]
        );
        // Time authority must never choose between conflicting counters.
        old.tokens[0] += 1;
        let conflicting = Ledger::new(today, [old]).unwrap();
        assert!(
            Ledger::periods(today, [&original, &conflicting])
                .unwrap()
                .values
                .iter()
                .all(Option::is_none)
        );
    }
    #[test]
    fn usage_time_requires_matching_counters_and_valid_original_timestamp() {
        let mut rows = live_sync_fixture();
        rows[1]["data"]["usage"]["inputTokens"] = json!(200);
        let session = parse(&encode(&rows), false).unwrap();
        let call = session.calls.into_values().next().unwrap();
        assert!(!call.usage_time);
        assert_eq!(call.at, rows[1]["time"].as_i64().unwrap());
        let mut rows = live_sync_fixture();
        rows[1]["data"]["stream"][0]["time"] = json!(-1);
        assert!(parse(&encode(&rows), false).is_err());
    }
    #[test]
    fn original_time_evidence_is_retained_for_legacy_peers_current_period_keys() {
        let root =
            std::env::temp_dir().join(format!("harness-canonical-{:016x}", rand::random::<u64>()));
        fs::create_dir_all(root.join("p/s")).unwrap();
        let mut rows = live_sync_fixture();
        rows[1]["time"] = rows[1]["data"]["stream"][0]["time"].clone();
        fs::write(root.join("p/s/session.v4.jsonl"), encode(&rows)).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let mut cache = HashMap::new();
        assert!(scan(&root, &mut cache, today).unwrap().calls.is_empty());
        let key = Call::key("synthetic-icloud-handoff", 1, 1, 0);
        let ledger = scan_with_references(&root, &mut cache, today, &HashSet::from([key])).unwrap();
        assert_eq!(ledger.calls.len(), 1);
        assert_eq!(ledger.calls[0].retain_day, Some(20260601));
        assert_eq!(
            Ledger::periods(today, [&ledger])
                .unwrap()
                .values
                .map(|v| v.map(Day::total)),
            [Some(0); 3]
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn atomic_log_replacement_invalidates_cache_even_with_preserved_length_and_mtime() {
        let root =
            std::env::temp_dir().join(format!("harness-replaced-{:016x}", rand::random::<u64>()));
        fs::create_dir_all(root.join("p/s")).unwrap();
        let path = root.join("p/s/session.v4.jsonl");
        fs::write(&path, encode(&[header(false), message(0, 100)])).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let mut cache = HashMap::new();
        assert_eq!(
            scan(&root, &mut cache, date()).unwrap().days().unwrap()[&date()].total(),
            165
        );
        let temp = path.with_extension("tmp");
        fs::write(&temp, encode(&[header(false), message(0, 200)])).unwrap();
        fs::File::options()
            .write(true)
            .open(&temp)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        fs::rename(temp, &path).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), metadata.len());
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            metadata.modified().unwrap()
        );
        assert_eq!(
            scan(&root, &mut cache, date()).unwrap().days().unwrap()[&date()].total(),
            265
        );
        fs::remove_dir_all(root).unwrap();
    }
    fn event(seq: u64, kind: &str, data: Value) -> Value {
        json!({"type":kind,"seq":seq,"time":1792540800000i64,"data":data})
    }
    fn sample(n: u64) -> Value {
        json!({"inputTokens":n,"outputTokens":10,"cacheReadTokens":50,"cacheWriteTokens":5,"reasoningTokens":3})
    }
    fn message(seq: u64, n: u64) -> Value {
        event(
            seq,
            "assistant/message",
            json!({"turn":1,"step":1,"usage":sample(n)}),
        )
    }
    fn encode(rows: &[Value]) -> Vec<u8> {
        rows.iter()
            .flat_map(|r| {
                let mut b = serde_json::to_vec(r).unwrap();
                b.push(b'\n');
                b
            })
            .collect()
    }
    fn origin(id: &str) -> Origin {
        Origin {
            id: id.into(),
            name: "Mac-mini".into(),
        }
    }
    #[test]
    fn disjoint_cache_and_reasoning_are_counted_once() {
        let s = parse(&encode(&[header(false), message(0, 100)]), false).unwrap();
        let d = s.days[&date()];
        assert_eq!((d.input, d.output, d.cached, d.total()), (155, 10, 50, 165));
    }
    #[test]
    fn streaming_settlement_replaces_and_retry_adds() {
        let stream = event(
            0,
            "assistant/attempt",
            json!({"turn":1,"step":1,"stream":[{"type":"chunk","chunk":{"type":"usage","usage":sample(90)}}]}),
        );
        let s = parse(
            &encode(&[
                header(false),
                stream,
                message(1, 100),
                event(2, "llm/retry-started", json!({"turn":1,"step":1})),
                message(3, 200),
            ]),
            false,
        )
        .unwrap();
        assert_eq!(s.days[&date()].total(), 430);
        assert_eq!(s.days[&date()].calls, 2);
        let ledger = Ledger::new(date(), s.calls.into_values()).unwrap();
        assert_eq!(ledger.calls.len(), 2);
        assert_eq!(ledger.days().unwrap()[&date()].total(), 430);
    }
    #[test]
    fn inherited_calls_are_excluded_and_missing_cut_is_refused() {
        let rows = [
            header(true),
            message(0, 999),
            event(1, "session/end-seed", json!({"inherited":true})),
            message(2, 100),
        ];
        assert_eq!(
            parse(&encode(&rows), false).unwrap().days[&date()].total(),
            165
        );
        assert!(parse(&encode(&rows[..2]), false).is_err());
        assert_eq!(parse(&encode(&rows), false).unwrap().calls.len(), 1);
    }
    #[test]
    fn generation_migration_and_physical_row_changes_preserve_call_identity() {
        let mut old_header = header(false);
        old_header["version"] = json!(2);
        let old = parse(&encode(&[old_header, message(0, 100)]), false).unwrap();
        let new = parse(
            &encode(&[
                header(false),
                event(0, "ignored", json!({})),
                message(1, 100),
            ]),
            false,
        )
        .unwrap();
        let merged = Ledger::new(
            date(),
            old.calls.into_values().chain(new.calls.into_values()),
        )
        .unwrap();
        assert_eq!(merged.calls.len(), 1);
        assert_eq!(merged.days().unwrap()[&date()].total(), 165);
    }
    #[test]
    fn private_device_cache_retains_call_identities_after_restart() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("harness-restart-{:016x}", rand::random::<u64>()));
        fs::create_dir(&root).unwrap();
        let r = test_reading("1111111111111111", 100);
        let d = DeviceUsage::from_reading(&r).unwrap();
        save_devices(&root, std::slice::from_ref(&d)).unwrap();
        assert_eq!(
            fs::metadata(root.join(".harness-devices-v4.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let loaded = load_devices(&root);
        assert_eq!(loaded, vec![d]);
        assert_eq!(effective(&r, &loaded).packet.d.unwrap()[1], 100);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn concatenated_frames_and_incomplete_tail() {
        let mut bytes = zstd::stream::encode_all(&encode(&[header(false)])[..], 1).unwrap();
        let data = zstd::stream::encode_all(&encode(&[message(0, 100)])[..], 1).unwrap();
        bytes.extend_from_slice(&data);
        assert_eq!(parse(&bytes, true).unwrap().days[&date()].total(), 165);
        bytes.extend_from_slice(&data[..data.len() / 2]);
        assert_eq!(parse(&bytes, true).unwrap().days[&date()].total(), 165);
        assert!(frame_end(&[0, 0, 0, 0], 0).is_err());
    }
    #[test]
    fn utc8_midnight_belongs_to_next_day() {
        let mut m = message(0, 100);
        m["time"] = json!(1792598400000i64);
        let s = parse(&encode(&[header(false), m]), false).unwrap();
        assert!(
            s.days
                .contains_key(&NaiveDate::from_ymd_opt(2026, 10, 22).unwrap())
        );
    }
    #[test]
    fn only_latest_generation_is_read_and_unchanged_cache_is_reused() {
        let root =
            std::env::temp_dir().join(format!("harness-test-{:016x}", rand::random::<u64>()));
        let dir = root.join("project/session");
        fs::create_dir_all(&dir).unwrap();
        let mut old = header(false);
        old["version"] = json!(2);
        fs::write(
            dir.join("session.v2.jsonl"),
            encode(&[old, message(0, 999)]),
        )
        .unwrap();
        fs::write(
            dir.join("session.v4.jsonl"),
            encode(&[header(false), message(0, 100)]),
        )
        .unwrap();
        let mut cache = HashMap::new();
        assert_eq!(
            scan(&root, &mut cache, date()).unwrap().days().unwrap()[&date()].total(),
            165
        );
        assert_eq!(cache.len(), 1);
        assert_eq!(
            scan(&root, &mut cache, date()).unwrap().days().unwrap()[&date()].total(),
            165
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_usage_versions_and_gaps_do_not_become_zero() {
        let mut h = header(false);
        h["version"] = json!(5);
        assert!(parse(&encode(&[h]), false).is_err());
        assert!(parse(&encode(&[header(false), message(2, 100)]), false).is_err());
        let mut m = message(0, 100);
        m["data"]["usage"]["inputTokens"] = json!(-1);
        assert!(parse(&encode(&[header(false), m]), false).is_err());
    }
    #[test]
    fn synced_calls_union_and_repeated_presentation_never_add_copies() {
        let local = test_reading("0000000000000001", 120);
        let mut other = DeviceUsage::from_reading(&local).unwrap();
        other.id = "0000000000000002".into();
        let merged = effective(&local, &[other]);
        assert_eq!(merged.packet.d.unwrap()[1], 120);
        assert_eq!(merged.tokens.calls, Some(1));
        assert_eq!(merged.tokens.devices.len(), 2);
        assert_eq!(
            effective(&merged, &merged.tokens.devices).packet.d,
            merged.packet.d
        );
        assert!(display_packets(&merged, true).unwrap().is_empty());
        let wire = display_packets(&merged, false).unwrap();
        assert_eq!(wire.len(), 1);
        assert_eq!(wire[0].d.unwrap()[1], 120);
        assert!(wire[0].o.is_none());
    }
    #[test]
    fn partial_sync_preserves_disjoint_calls_and_ignores_legacy_totals() {
        let local = test_reading("0000000000000001", 120);
        let other = test_reading("0000000000000002", 30);
        let mut legacy = DeviceUsage::from_reading(&local).unwrap();
        legacy.id = "0000000000000003".into();
        legacy.ledger = None;
        let result = effective(
            &local,
            &[DeviceUsage::from_reading(&other).unwrap(), legacy.clone()],
        );
        for value in [result.packet.d, result.packet.wt, result.packet.mt] {
            assert_eq!(value.unwrap()[1], 150);
        }
        assert_eq!(result.tokens.calls, Some(2));
        let mut old = local.clone();
        old.tokens.harness_ledger = None;
        let unknown = effective(&old, &[legacy]);
        assert!(
            unknown.packet.d.is_none()
                && unknown.packet.wt.is_none()
                && unknown.packet.mt.is_none()
        );
        assert_ne!(unknown.packet.s, usage::OK);
    }
    #[test]
    fn local_duplicate_paths_are_deduplicated_and_conflicts_fail_closed() {
        let root =
            std::env::temp_dir().join(format!("harness-copies-{:016x}", rand::random::<u64>()));
        for name in ["a/s", "b/s"] {
            fs::create_dir_all(root.join(name)).unwrap();
            fs::write(
                root.join(name).join("session.v4.jsonl"),
                encode(&[header(false), message(0, 100)]),
            )
            .unwrap();
        }
        let mut cache = HashMap::new();
        let full = scan(&root, &mut cache, date()).unwrap();
        assert_eq!(full.calls.len(), 1);
        assert_eq!(full.days().unwrap()[&date()].total(), 165);
        fs::write(
            root.join("b/s/session.v4.jsonl"),
            encode(&[header(false), message(0, 222)]),
        )
        .unwrap();
        assert!(scan(&root, &mut cache, date()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cross_mac_conflicts_and_legacy_newer_snapshots_cannot_forge_totals() {
        let local = test_reading("0000000000000001", 120);
        let a = DeviceUsage::from_reading(&local).unwrap();
        let mut older_copy = a.clone();
        older_copy.ledger = None;
        older_copy.t += 100;
        assert!(
            merge_devices(std::slice::from_ref(&a), &[older_copy])[0]
                .ledger
                .is_some()
        );
        let mut conflict = a.clone();
        conflict.id = "0000000000000002".into();
        conflict.d[1] += 1;
        conflict.wt[1] += 1;
        conflict.mt.as_mut().unwrap()[1] += 1;
        conflict.u[0] += 1;
        std::sync::Arc::make_mut(&mut conflict.ledger.as_mut().unwrap().calls)[0].tokens[0] += 1;
        assert!(conflict.valid());
        let r = effective(&local, &[conflict]);
        assert_eq!(r.packet.s, 6);
        assert!(r.packet.d.is_none() && r.packet.wt.is_none() && r.packet.mt.is_none());
        let clear = display_packets(&r, false).unwrap().remove(0);
        assert_eq!(clear.s, 0);
        assert!(clear.o.is_none() && clear.d.is_none() && clear.u.is_none());
        assert_eq!(r.packet.s, 6); // The App still explains the original conflict.
    }
    #[test]
    fn cross_day_copies_preserve_week_and_month_through_cache_and_display() {
        let snapshot = |id: &str, billed: NaiveDate| {
            let origin = Origin {
                id: id.into(),
                name: "Test Mac".into(),
            };
            let ledger = Ledger::new(
                date(),
                [Call {
                    through: None,
                    usage_time: false,
                    retain_day: None,
                    key: Call::key("shared-session", 1, 1, 0),
                    at: billed
                        .and_hms_opt(4, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp_millis(),
                    revision: 1,
                    settled: true,
                    tokens: [100, 10, 5],
                }],
            )
            .unwrap();
            let mut r = reading(&ledger.days().unwrap(), date(), origin).unwrap();
            r.tokens.harness_ledger = Some(ledger);
            r
        };
        let local = snapshot("1111111111111111", date());
        let peer = snapshot("2222222222222222", date() - chrono::Duration::days(1));
        let devices = vec![DeviceUsage::from_reading(&peer).unwrap()];
        let restored: Vec<DeviceUsage> =
            serde_json::from_slice(&serde_json::to_vec(&devices).unwrap()).unwrap();
        for devices in [devices, restored] {
            let result = effective_on(&local, &devices, date());
            assert_eq!(result.packet.s, usage::OK);
            assert!(result.packet.d.is_none() && result.packet.u.is_none());
            assert_eq!(result.packet.wt, Some([20261019, 110]));
            assert_eq!(result.packet.mt, Some([20261001, 110]));
            assert!(result.tokens.note.contains("今日调用记录有分歧"));
            let packet = display_packets(&result, false).unwrap().remove(0);
            assert_eq!(packet.s, usage::OK);
            assert_eq!(packet.d.unwrap()[1], -1);
            assert!(packet.o.is_none());
            assert_eq!(packet.mt, Some([20261001, 110]));
            let repeated = effective_on(&result, &devices, date());
            assert_eq!(repeated.packet.mt, result.packet.mt);
        }
    }
    #[test]
    fn large_call_caches_reload_without_relaxing_private_configuration_limits() {
        let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
        let mut base = test_reading("1111111111111111", 100);
        let original = base.tokens.harness_ledger.as_ref().unwrap().calls[0].clone();
        let ledger = Ledger::new(
            today,
            (0..2048).map(|i| Call {
                key: Call::key("large", i, 1, 0),
                ..original.clone()
            }),
        )
        .unwrap();
        base = reading(&ledger.days().unwrap(), today, base.packet.o.unwrap()).unwrap();
        base.tokens.harness_ledger = Some(ledger);
        let device = DeviceUsage::from_reading(&base).unwrap();
        let root =
            std::env::temp_dir().join(format!("harness-large-{:016x}", rand::random::<u64>()));
        fs::create_dir(&root).unwrap();
        save_devices(&root, std::slice::from_ref(&device)).unwrap();
        let path = root.join(".harness-devices-v4.json");
        assert!(fs::metadata(&path).unwrap().len() > 32_768);
        assert!(crate::config::read_private::<Vec<DeviceUsage>>(&path).is_err());
        assert_eq!(load_devices(&root), vec![device.clone()]);
        fs::rename(&path, root.join(".harness-devices-v2.json")).unwrap();
        assert_eq!(load_devices(&root), vec![device]);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn origin_extension_requires_explicit_capability_and_stays_under_gatt_limit() {
        let mut days = BTreeMap::new();
        days.insert(
            date(),
            Day {
                input: 155,
                output: 10,
                cached: 50,
                calls: 1,
            },
        );
        let local = reading(&days, date(), origin("0000000000000001")).unwrap();
        assert!(local.packet.encode().unwrap().len() <= 480);
        let wire: Value = serde_json::from_slice(
            &local
                .packet
                .wire_for_device(true, true, true, false, false)
                .unwrap(),
        )
        .unwrap();
        assert!(wire.get("o").is_none());
        let mut invalid = local.packet;
        invalid.p = 3;
        assert!(invalid.encode().is_err());
    }
    #[test]
    fn monthly_records_use_calendar_boundary_and_replace_each_mac() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let mut days = BTreeMap::new();
        for (day, n) in [
            (today - chrono::Duration::days(1), 200),
            (today, 100),
            (today + chrono::Duration::days(1), 900),
        ] {
            days.insert(
                day,
                Day {
                    input: n,
                    ..Default::default()
                },
            );
        }
        let local = reading(&days, today, origin("0000000000000001")).unwrap();
        assert_eq!(local.packet.wt.unwrap()[1], 300);
        assert_eq!(local.packet.mt, Some([20261001, 100]));
        let full = DeviceUsage::from_reading(&local).unwrap();
        let mut legacy_value = serde_json::to_value(&full).unwrap();
        legacy_value.as_object_mut().unwrap().remove("mt");
        let legacy: DeviceUsage = serde_json::from_value(legacy_value).unwrap();
        assert!(legacy.valid() && legacy.mt.is_none());
        assert_eq!(
            merge_devices(std::slice::from_ref(&full), &[legacy])[0].mt,
            full.mt
        );
        let mut replacement = full.clone();
        replacement.t += 1;
        replacement.mt = Some([20261001, 130]);
        assert_eq!(
            merge_devices(&[full], &[replacement])[0].mt,
            Some([20261001, 130])
        );
    }
    #[test]
    fn origin_is_private_and_stable_across_install_locations_on_one_mac() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("harness-ids-{:016x}", rand::random::<u64>()));
        let a = base.join("a");
        let b = base.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let id = Origin::local(&a).unwrap().id;
        assert_eq!(id, Origin::local(&a).unwrap().id);
        assert_eq!(id, Origin::local(&b).unwrap().id);
        assert_eq!(
            fs::metadata(a.join(".harness-origin.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::remove_dir_all(base).unwrap();
    }
}
