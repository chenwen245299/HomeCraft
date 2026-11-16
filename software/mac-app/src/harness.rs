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
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

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
        };
        (r.packet.s == usage::OK && result.valid()).then_some(result)
    }
}
pub fn effective(local: &Reading, devices: &[DeviceUsage]) -> Reading {
    if local.packet.p != PROVIDER {
        return local.clone();
    }
    let mut devices = merge_devices(devices, &local.tokens.devices);
    // An effective reading still carries the original packet origin for transport compatibility.
    // Its totals must never replace that origin's raw snapshot on a second presentation pass.
    if let Some(current) = local
        .tokens
        .devices
        .is_empty()
        .then(|| DeviceUsage::from_reading(local))
        .flatten()
    {
        if let Some(old) = devices.iter_mut().find(|d| d.id == current.id) {
            if current.t > old.t
                || (current.t == old.t && (current.mt.is_some() || old.mt.is_none()))
            {
                *old = current;
            }
        } else {
            devices.push(current);
        }
    }
    if devices.is_empty() {
        return local.clone();
    }
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let day = day_id(today);
    let week =
        day_id(today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday())));
    let month = day_id(today.with_day(1).unwrap());
    let mut r = local.clone();
    r.packet.s = usage::OK;
    r.packet.d = Some([day, 0]);
    r.packet.wt = Some([week, 0]);
    r.packet.mt = Some([month, 0]);
    r.packet.u = Some([0; 5]);
    for d in &devices {
        if d.d[0] == day {
            r.packet.d.as_mut().unwrap()[1] += d.d[1];
            for i in 0..4 {
                r.packet.u.as_mut().unwrap()[i] += d.u[i];
            }
        }
        if d.wt[0] == week {
            r.packet.wt.as_mut().unwrap()[1] += d.wt[1];
        }
        if let Some(m) = d.mt.filter(|m| m[0] == month) {
            r.packet.mt.as_mut().unwrap()[1] += m[1];
        }
    }
    let u = r.packet.u.as_mut().unwrap();
    u[4] = u[3];
    r.tokens.weekly = devices
        .iter()
        .any(|d| d.wt[0] == week)
        .then_some(r.packet.wt.unwrap()[1] as u64);
    r.tokens.recent_day = devices
        .iter()
        .any(|d| d.d[0] == day)
        .then_some(r.packet.d.unwrap()[1] as u64);
    r.tokens.monthly = devices
        .iter()
        .any(|d| d.mt.is_some_and(|m| m[0] == month))
        .then_some(r.packet.mt.unwrap()[1] as u64);
    r.tokens.input = Some(u[0] as u64);
    r.tokens.output = Some(u[1] as u64);
    r.tokens.cached = Some(u[2] as u64);
    r.tokens.calls = Some(u[3] as u64);
    if r.tokens.recent_day.is_none() {
        r.packet.d = None;
        r.packet.u = None;
        r.tokens.input = None;
        r.tokens.output = None;
        r.tokens.cached = None;
        r.tokens.calls = None;
    }
    if r.tokens.weekly.is_none() {
        r.packet.wt = None;
    }
    if r.tokens.monthly.is_none() {
        r.packet.mt = None;
    }
    r.tokens.note = format!("{} 台 Mac · 已同步的本地记录 · UTC+8", devices.len());
    r.tokens.devices = devices;
    r
}
pub fn merge_devices(a: &[DeviceUsage], b: &[DeviceUsage]) -> Vec<DeviceUsage> {
    let mut result = Vec::<DeviceUsage>::new();
    for d in a.iter().chain(b).filter(|d| d.valid()) {
        if let Some(old) = result.iter_mut().find(|old| old.id == d.id) {
            if d.t > old.t || (d.t == old.t && (d.mt.is_some() || old.mt.is_none())) {
                *old = d.clone();
            }
        } else if result.len() < 8 {
            result.push(d.clone());
        }
    }
    result
}
/// Per-Mac displays must receive original sources, never an effective aggregate retaining the
/// local origin. Aggregate-only displays receive one packet and strip its origin at transport.
pub fn display_packets(reading: &Reading, per_mac: bool) -> Result<Vec<usage::Packet>> {
    if reading.packet.p == PROVIDER && per_mac && !reading.tokens.devices.is_empty() {
        return reading
            .tokens
            .devices
            .iter()
            .map(DeviceUsage::packet)
            .collect();
    }
    Ok(vec![reading.packet.clone()])
}
/// Legacy per-Mac firmware rejects an older snapshot, including an equal timestamp with
/// different values. Read its sources first and send only newer observations; never restamp
/// a peer's data merely to force it into the device cache.
pub fn display_updates(reading: &Reading, known: &[DeviceUsage]) -> Result<Vec<usage::Packet>> {
    Ok(display_packets(reading, true)?
        .into_iter()
        .filter(|p| {
            p.p != PROVIDER
                || p.s != usage::OK
                || p.o.as_ref().is_none_or(|o| {
                    known
                        .iter()
                        .find(|d| d.id == o.id)
                        .is_none_or(|d| p.t > d.t)
                })
        })
        .collect())
}
pub fn load_devices(root: &Path) -> Vec<DeviceUsage> {
    let devices: Vec<DeviceUsage> =
        crate::config::read_private(&root.join(".harness-devices.json")).unwrap_or_default();
    if devices.len() > 8 || devices.iter().any(|d| !d.valid()) {
        return Vec::new();
    }
    merge_devices(&[], &devices)
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
        file.write_all(&serde_json::to_vec(devices)?)?;
        file.sync_all()?;
        fs::rename(&tmp, root.join(".harness-devices.json"))?;
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

#[derive(Clone, Copy, Default, Deserialize)]
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
}
#[derive(Default)]
struct Fold {
    result: Session,
    version: u32,
    seq: u64,
    seeded: bool,
    marker: bool,
    last: Option<((u64, u64), NaiveDate, Day)>,
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
            self.last = None;
            self.marker = true;
            return Ok(());
        }
        if row.kind == "llm/retry-started" {
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
        let millis = row
            .time
            .ok_or_else(|| anyhow::anyhow!("Harness 用量缺少时间"))?;
        let date = (chrono::DateTime::from_timestamp_millis(millis)
            .ok_or_else(|| anyhow::anyhow!("Harness 时间无效"))?
            + chrono::Duration::hours(8))
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
    session: Session,
}
fn scan(root: &Path, cache: &mut HashMap<PathBuf, Cached>) -> Result<BTreeMap<NaiveDate, Day>> {
    let mut found = HashSet::new();
    let mut ids = HashSet::new();
    let mut days = BTreeMap::<NaiveDate, Day>::new();
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
            let session = if let Some(saved) = cache
                .get(&path)
                .filter(|c| c.len == meta.len() && c.modified == modified)
            {
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
                        session: session.clone(),
                    },
                );
                session
            };
            found.insert(path);
            ensure!(ids.insert(session.id), "Harness 出现重复会话，未合并计数");
            for (day, value) in session.days {
                days.entry(day).or_default().add(value)?;
            }
        }
    }
    cache.retain(|p, _| found.contains(p));
    Ok(days)
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
        let read = (|| {
            let origin = Origin::local(&root)?;
            let mut cache = CACHE
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .map_err(|_| anyhow::anyhow!("Harness 缓存不可用"))?;
            let days = scan(&sessions, &mut cache)?;
            reading(
                &days,
                (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive(),
                origin,
            )
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
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 21).unwrap()
    }
    fn header(seeded: bool) -> Value {
        json!({"type":"session","version":4,"id":"synthetic-session","isSeeded":seeded})
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
        assert_eq!(scan(&root, &mut cache).unwrap()[&date()].total(), 165);
        assert_eq!(cache.len(), 1);
        assert_eq!(scan(&root, &mut cache).unwrap()[&date()].total(), 165);
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
    fn two_macs_replace_without_echoing_the_aggregate_and_ignore_stale_days() {
        let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
        let mut days = BTreeMap::new();
        days.insert(
            today,
            Day {
                input: 100,
                output: 20,
                cached: 40,
                calls: 2,
            },
        );
        let local = reading(&days, today, origin("0000000000000001")).unwrap();
        let mut other = DeviceUsage::from_reading(&local).unwrap();
        other.id = "0000000000000002".into();
        let merged = effective(
            &local,
            &[DeviceUsage::from_reading(&local).unwrap(), other.clone()],
        );
        assert_eq!(merged.packet.d.unwrap()[1], 240);
        assert_eq!(merged.tokens.devices.len(), 2);
        assert_eq!(local.packet.d.unwrap()[1], 120);
        let packets = display_packets(&merged, true).unwrap();
        assert_eq!(packets.len(), 2);
        assert!(packets.iter().all(|p| p.d.unwrap()[1] == 120));
        assert_ne!(
            packets[0].o.as_ref().unwrap().id,
            packets[1].o.as_ref().unwrap().id
        );
        assert_eq!(
            display_packets(&merged, false).unwrap()[0].d.unwrap()[1],
            240
        );
        // A local reader, without an aggregate view, remains a single stable source.
        assert_eq!(display_packets(&local, true).unwrap()[0].d.unwrap()[1], 120);
        assert_eq!(
            display_updates(&merged, &merged.tokens.devices)
                .unwrap()
                .len(),
            0
        );
        let mut newer = merged.tokens.devices.clone();
        newer[0].t += 1;
        assert_eq!(display_updates(&merged, &newer).unwrap().len(), 0);
        let mut older = merged.tokens.devices.clone();
        older[0].t -= 1;
        let updates = display_updates(&merged, &older).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].o.as_ref().unwrap().id, older[0].id);
        assert_eq!(updates[0].t, local.packet.t);
        assert_eq!(
            merged.packet.u.unwrap()[0] + merged.packet.u.unwrap()[1],
            240
        );
        other.d[0] = day_id(today - chrono::Duration::days(1));
        assert_eq!(effective(&local, &[other]).packet.d.unwrap()[1], 120);
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
