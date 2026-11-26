//! User-selected iCloud Drive document sync. Only this explicit numeric schema leaves
//! the private app directory. Auth, BLE bindings, CLI paths and raw replies never enter it.
use crate::{
    account::Identity,
    backend::{Events, State},
    device::History,
    harness::{DeviceUsage, Origin},
    usage::{self, Packet, Reading},
};
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

const MAX_FILE: u64 = 256 * 1024;
const MAX_HARNESS_FILE: u64 = 24 * 1024 * 1024;
const MAX_HARNESS_CACHE: u64 = 192 * 1024 * 1024;
const MAX_SNAPSHOTS: usize = 800;
const MAX_CACHE: u64 = 32 * 1024 * 1024;
/// Displays per Mac and day; the device registry holds at most 8.
const MAX_DISPLAYS: usize = 16;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub folder: Option<PathBuf>,
}
#[derive(Clone, Default)]
pub struct Status {
    pub folder: Option<PathBuf>,
    pub busy: bool,
    pub last: Option<i64>,
    pub message: String,
    pub error: bool,
    /// This Mac's sync identity: its own earlier captures are a local cache, not iCloud data.
    pub origin: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub packet: Packet,
    pub account: Option<Identity>,
    pub lifetime: Option<u64>,
    pub peak_daily: Option<u64>,
    #[serde(default)]
    pub context: Option<[u64; 3]>,
    #[serde(default)]
    pub breakdown: Option<crate::breakdown::Breakdown>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_ledger: Option<crate::harness::Ledger>,
}
impl Summary {
    pub(crate) fn from_reading(r: &Reading) -> Option<Self> {
        if r.packet.s != usage::OK
            || r.packet.p > 4
            || (r.packet.p == 4 && !r.tokens.devices.is_empty())
        {
            return None;
        }
        let mut packet = r.packet.clone();
        // Derived display history is sent only to negotiated devices, not the
        // shared schemas used by older Apps. Source quota snapshots remain intact.
        packet.yd = None;
        // Canonical labels only. In particular source/note/error text is never exported.
        packet.n = match packet.p {
            3 => Some("Argus".into()),
            4 => Some(crate::harness::NAME.into()),
            _ => None,
        };
        if !matches!(packet.c.as_deref(), Some("USD" | "CNY") | None) {
            packet.c = None;
            packet.b = None;
        }
        let s = Self {
            harness_ledger: r.tokens.harness_ledger.clone().filter(|_| r.packet.p == 4),
            packet,
            account: (r.packet.p == 0)
                .then(|| r.tokens.account.clone())
                .flatten(),
            lifetime: r.tokens.lifetime,
            peak_daily: r.tokens.peak_daily,
            context: if r.packet.p == 1 {
                r.tokens
                    .input
                    .zip(r.tokens.output)
                    .zip(r.tokens.context_size)
                    .map(|((i, o), c)| [i, o, c])
            } else {
                None
            },
            breakdown: r
                .tokens
                .breakdown
                .clone()
                .filter(|b| b.wire(&r.packet).is_ok()),
        };
        s.valid().then_some(s)
    }
    pub(crate) fn reading(&self) -> Reading {
        let tokens = crate::usage::TokenStats {
            account: self.account.clone(),
            harness_ledger: self.harness_ledger.clone(),
            lifetime: self.lifetime,
            peak_daily: self.peak_daily,
            weekly: self.packet.wt.map(|w| w[1] as u64),
            monthly: self.packet.mt.map(|m| m[1] as u64),
            input: self.packet.u.map(|n| n[0] as u64),
            output: self.packet.u.map(|n| n[1] as u64),
            cached: self.packet.u.map(|n| n[2] as u64),
            calls: self.packet.u.map(|n| n[3] as u64),
            cost_usd: self.packet.dc,
            breakdown: self.breakdown.clone(),
            context_size: self.context.map(|c| c[2]),
            ..Default::default()
        };
        let mut tokens = tokens;
        if let Some([input, output, _]) = self.context {
            tokens.input = Some(input);
            tokens.output = Some(output);
        }
        Reading {
            packet: self.packet.clone(),
            tokens,
            source: "iCloud Drive 数值摘要".into(),
        }
    }
    fn valid(&self) -> bool {
        self.harness_ledger.as_ref().is_none_or(|l| {
            self.packet.p == 4 && l.valid() && DeviceUsage::from_reading(&self.reading()).is_some()
        }) && self.packet.p <= 4
            && self.packet.yd.is_none()
            && self.packet.s == usage::OK
            && valid_time(self.packet.t)
            && self.packet.encode().is_ok()
            && self
                .account
                .as_ref()
                .is_none_or(|a| self.packet.p == 0 && a.valid())
            && self
                .context
                .is_none_or(|c| self.packet.p == 1 && c.iter().all(|n| *n <= 9_007_199_254_740_991))
            && self
                .breakdown
                .as_ref()
                .is_none_or(|b| b.wire(&self.packet).is_ok())
            && [self.lifetime, self.peak_daily]
                .into_iter()
                .flatten()
                .all(|n| n <= 9_007_199_254_740_991)
            && self
                .packet
                .c
                .as_ref()
                .is_none_or(|c| c == "USD" || c == "CNY")
    }
    fn key(&self) -> String {
        format!(
            "{}:{}",
            self.packet.p,
            self.account
                .as_ref()
                .map(|a| a.key.as_str())
                .unwrap_or("local")
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub device: String,
    pub sampled_at: i64,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub history: Option<History>,
}
impl Environment {
    fn valid(&self) -> bool {
        self.device
            .strip_prefix("EPaper_")
            .is_some_and(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            && valid_time(self.sampled_at)
            && self
                .temperature
                .is_none_or(|n| n.is_finite() && (-40.0..=125.0).contains(&n))
            && self
                .humidity
                .is_none_or(|n| n.is_finite() && (0.0..=100.0).contains(&n))
            && self.history.as_ref().is_none_or(|h| {
                valid_day(h.day as i64)
                    && timestamp_day(self.sampled_at) == Some(h.day as i64)
                    && valid_time(h.received_at)
                    && h.points.len() == 96
                    && h.points.iter().flatten().all(|(t, h)| {
                        t.is_finite()
                            && h.is_finite()
                            && (-40.0..=125.0).contains(t)
                            && (0.0..=100.0).contains(h)
                    })
            })
    }
    /// The same display: newer values win, observed history buckets are united.
    fn merge(&mut self, new: &Self) {
        let history = match (&self.history, &new.history) {
            (Some(a), Some(b)) if a.day == b.day => {
                // Union observed buckets. A later authoritative revision replaces
                // overlapping buckets; a sparse older Mac can't erase newer data.
                let (mut h, secondary) = if a.received_at >= b.received_at {
                    (a.clone(), b)
                } else {
                    (b.clone(), a)
                };
                for (p, other) in h.points.iter_mut().zip(&secondary.points) {
                    if p.is_none() {
                        *p = *other;
                    }
                }
                Some(h)
            }
            (Some(a), Some(b)) => Some(if a.day > b.day { a.clone() } else { b.clone() }),
            (a, b) => a.clone().or_else(|| b.clone()),
        };
        if new.sampled_at >= self.sampled_at {
            *self = new.clone();
        }
        self.history = history;
    }
}
/// Adds one display's reading to a day's list, one entry per device.
fn add_climate(all: &mut Vec<Environment>, new: &Environment) {
    match all.iter_mut().find(|e| e.device == new.device) {
        Some(old) => old.merge(new),
        None => all.push(new.clone()),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u8,
    pub origin: Origin,
    pub day: i64,
    pub usages: Vec<Summary>,
    /// The newest display's reading; files from builds before per-display summaries have only this.
    pub environment: Option<Environment>,
    /// Every other display this Mac read that day, one entry per device. Left out when empty, so a
    /// single-display file stays readable by build 42.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub displays: Vec<Environment>,
}
impl Snapshot {
    /// Every display's reading that day.
    pub fn climate(&self) -> impl Iterator<Item = &Environment> {
        self.environment.iter().chain(&self.displays)
    }
    /// Newest first: the first goes in `environment`, the rest in `displays`.
    fn set_climate(&mut self, mut all: Vec<Environment>) {
        all.sort_by(|a, b| {
            b.sampled_at
                .cmp(&a.sampled_at)
                .then_with(|| a.device.cmp(&b.device))
        });
        all.truncate(MAX_DISPLAYS);
        let mut all = all.into_iter();
        self.environment = all.next();
        self.displays = all.collect();
    }
    fn valid(&self) -> bool {
        let devices = self
            .climate()
            .map(|e| e.device.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        (1..=6).contains(&self.version)
            && (!matches!(self.version, 4..=6)
                || (self.usages.len() == 1
                    && self.usages[0].harness_ledger.is_some()
                    && (self.version == 6
                        || (self.version == 5
                            && self.usages[0]
                                .harness_ledger
                                .as_ref()
                                .is_some_and(|l| l.version <= 2))
                        || self.usages[0]
                            .harness_ledger
                            .as_ref()
                            .is_some_and(|l| l.version == 1))
                    && self.climate().next().is_none()))
            && self.origin.valid()
            && valid_day(self.day)
            && self.usages.len() <= 24
            && self.usages.iter().all(|s| {
                s.valid()
                    && timestamp_day(s.packet.t) == Some(self.day)
                    && s.account
                        .as_ref()
                        .is_none_or(|a| a.device.id == self.origin.id)
                    && (s.packet.p != 4
                        || s.packet.o.as_ref().is_some_and(|o| o.id == self.origin.id))
            })
            && devices.len() == self.climate().count()
            && devices.len() <= MAX_DISPLAYS
            && self
                .climate()
                .all(|e| e.valid() && timestamp_day(e.sampled_at) == Some(self.day))
    }
    fn merge(&mut self, other: &Self) {
        for u in &other.usages {
            if let Some(old) = self.usages.iter_mut().find(|a| a.key() == u.key()) {
                if old.packet.p == 4 && (old.harness_ledger.is_some() || u.harness_ledger.is_some())
                {
                    let devices = crate::harness::merge_devices(
                        &DeviceUsage::from_reading(&old.reading())
                            .into_iter()
                            .collect::<Vec<_>>(),
                        &DeviceUsage::from_reading(&u.reading())
                            .into_iter()
                            .collect::<Vec<_>>(),
                    );
                    if let Some(d) = devices.first() {
                        let mut next = if u.packet.t > old.packet.t {
                            u.clone()
                        } else {
                            old.clone()
                        };
                        if let Ok(packet) = d.packet() {
                            next.packet = packet;
                            next.harness_ledger = d.ledger.clone();
                            *old = next;
                        }
                    }
                    continue;
                }
                if u.packet.t > old.packet.t
                    || (u.packet.t == old.packet.t
                        && (u.packet.mt.is_some() || old.packet.mt.is_none()))
                {
                    let mut next = u.clone();
                    if next.packet.p == 0 {
                        // A successful quota/account response can omit token fields. Keep the
                        // last known observation of the SAME period when merging today's file.
                        let day = timestamp_day(next.packet.t).unwrap_or(0);
                        for (target, previous, period) in [
                            (&mut next.packet.d, old.packet.d, Some(day)),
                            (
                                &mut next.packet.wt,
                                old.packet.wt,
                                crate::home::model::week_start(day),
                            ),
                            (
                                &mut next.packet.mt,
                                old.packet.mt,
                                crate::home::model::month_start(day),
                            ),
                        ] {
                            if target.is_none_or(|v| v[1] < 0) {
                                *target = previous
                                    .filter(|v| Some(v[0]) == period && v[1] >= 0)
                                    .or(*target);
                            }
                        }
                    }
                    *old = next;
                }
            } else if self.usages.len() < 24 {
                self.usages.push(u.clone());
            }
        }
        let mut climate = self.climate().cloned().collect();
        for new in other.climate() {
            add_climate(&mut climate, new);
        }
        self.set_climate(climate);
        self.usages.sort_by_key(Summary::key);
        self.version = if self.displays.is_empty() {
            FORMAT
        } else {
            DISPLAYS_FORMAT
        };
    }
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub snapshots: Vec<Snapshot>,
}
impl Catalog {
    fn merge(&mut self, snapshot: Snapshot) {
        if !snapshot.valid() {
            return;
        }
        if let Some(old) = self
            .snapshots
            .iter_mut()
            .find(|s| s.day == snapshot.day && s.origin.id == snapshot.origin.id)
        {
            old.merge(&snapshot);
        } else {
            self.snapshots.push(snapshot);
        }
        self.snapshots
            .sort_by(|a, b| b.day.cmp(&a.day).then(a.origin.id.cmp(&b.origin.id)));
        let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
        let earliest = day_id(today - chrono::Duration::days(89));
        self.snapshots
            .retain(|s| s.day >= earliest && s.day <= day_id(today));
        self.snapshots.truncate(MAX_SNAPSHOTS);
        let mut newest = std::collections::BTreeSet::new();
        for s in &mut self.snapshots {
            for u in &mut s.usages {
                if u.harness_ledger.is_some() && !newest.insert(s.origin.id.clone()) {
                    u.harness_ledger = None;
                }
            }
        }
    }
    pub(crate) fn capture(&mut self, state: &State, origin: Origin) {
        if state.usage_resolved {
            return;
        }
        let day = day_id((chrono::Utc::now() + chrono::Duration::hours(8)).date_naive());
        let usages = state
            .readings
            .iter()
            .filter(|r| !state.cloud_sources.contains_key(&r.packet.p))
            .filter(|r| r.packet.p != 4 || r.packet.o.as_ref().is_some_and(|o| o.id == origin.id))
            .filter(|r| timestamp_day(r.packet.t) == Some(day))
            .filter_map(Summary::from_reading)
            .collect();
        let primary = state
            .telemetry
            .as_ref()
            .filter(|_| state.environment_source.is_none())
            .map(|d| Environment {
                device: state.bound_name.clone(),
                sampled_at: d.sampled_at,
                temperature: d.temperature,
                humidity: d.humidity,
                history: state
                    .history
                    .clone()
                    .filter(|h| timestamp_day(d.sampled_at) == Some(h.day as i64)),
            });
        // Displays held in background sessions report readings only, no 15-minute history.
        let background = state.fleet.values().filter_map(|r| {
            r.climate
                .map(|(temperature, humidity, sampled_at)| Environment {
                    device: r.name.clone(),
                    sampled_at,
                    temperature: Some(temperature),
                    humidity: Some(humidity),
                    history: None,
                })
        });
        let mut climate = vec![];
        for e in primary
            .into_iter()
            .chain(background)
            .filter(|e| e.valid() && timestamp_day(e.sampled_at) == Some(day))
        {
            add_climate(&mut climate, &e);
        }
        let mut snapshot = Snapshot {
            version: 1,
            origin,
            day,
            usages,
            environment: None,
            displays: vec![],
        };
        snapshot.set_climate(climate);
        self.merge(snapshot);
    }
    /// Latest per account, not a sum per Mac. Unknown accounts remain device-scoped.
    pub fn accounts(&self, readings: &[Reading]) -> Vec<Summary> {
        let mut accounts = BTreeMap::<String, Summary>::new();
        for s in self
            .snapshots
            .iter()
            .flat_map(|s| s.usages.iter())
            .filter(|s| s.packet.p == 0)
            .cloned()
            .chain(
                readings
                    .iter()
                    .filter(|r| r.packet.p == 0)
                    .filter_map(Summary::from_reading),
            )
        {
            let Some(a) = &s.account else {
                continue;
            };
            let key = a.key.clone();
            if accounts.get(&key).is_none_or(|old| {
                s.packet.t > old.packet.t
                    || (s.packet.t == old.packet.t
                        && (s.packet.mt.is_some(), &a.device.id)
                            > (
                                old.packet.mt.is_some(),
                                &old.account.as_ref().unwrap().device.id,
                            ))
            }) {
                accounts.insert(key, s);
            }
        }
        accounts.into_values().collect()
    }
    pub fn harness(&self) -> Vec<DeviceUsage> {
        let mut result = vec![];
        for snapshot in &self.snapshots {
            let devices = snapshot
                .usages
                .iter()
                .filter(|s| s.packet.p == 4)
                .filter_map(|s| DeviceUsage::from_reading(&s.reading()))
                .collect::<Vec<_>>();
            if !devices.is_empty() {
                result = crate::harness::merge_devices(&result, &devices);
            }
        }
        result
    }
}
fn day_id(d: NaiveDate) -> i64 {
    i64::from(d.year()) * 10000 + i64::from(d.month()) * 100 + i64::from(d.day())
}
fn valid_day(day: i64) -> bool {
    (20250101..=20991231).contains(&day)
        && NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d").is_ok()
}
fn valid_time(t: i64) -> bool {
    (1735689600..=4102444800).contains(&t) && t <= usage::now() + 300
}
/// A UI-only copy. Never feed imported fallback readings back to capture()/BLE.
/// Local readings with iCloud summaries filling providers this Mac has no valid reading for
/// (Codex: the same account's newer summary). Never adds across Macs. Also names each source.
pub fn merged_readings(
    readings: &[Reading],
    catalog: &Catalog,
) -> (Vec<Reading>, BTreeMap<u8, Origin>) {
    let mut out = readings.to_vec();
    let mut sources = BTreeMap::new();
    for p in 0..=4 {
        let local = out.iter().find(|r| r.packet.p == p);
        let key = local
            .and_then(|r| r.tokens.account.as_ref())
            .map(|a| a.key.as_str());
        let candidate = catalog
            .snapshots
            .iter()
            .flat_map(|s| s.usages.iter().map(move |u| (s, u)))
            .filter(|(_, u)| u.packet.p == p)
            .filter(|(_, u)| {
                p != 0 || key.is_none_or(|k| u.account.as_ref().is_some_and(|a| a.key == k))
            })
            .max_by_key(|(_, u)| u.packet.t);
        let Some((snapshot, u)) = candidate else {
            continue;
        };
        if local.is_some_and(|r| r.packet.s == usage::OK && (p != 0 || r.packet.t >= u.packet.t)) {
            continue;
        }
        let reading = u.reading();
        sources.insert(p, snapshot.origin.clone());
        if let Some(old) = out.iter_mut().find(|r| r.packet.p == p) {
            *old = reading;
        } else {
            out.push(reading);
        }
    }
    (out, sources)
}
/// Where a cached summary came from, as shown beside it.
fn source_label(origin: &Origin, local: Option<&str>) -> String {
    if local == Some(origin.id.as_str()) {
        "本机缓存".into()
    } else {
        format!("iCloud · {}", origin.name)
    }
}
/// One place a temperature/humidity reading comes from.
pub struct ClimateSource {
    /// The display itself when live, otherwise where its summary came from (本机缓存 / iCloud · Mac).
    pub label: String,
    pub device: String,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub sampled_at: i64,
    /// A display connected in the background right now.
    pub live: bool,
    /// This Mac's own earlier capture: only a fallback for the card, never another room.
    pub own: bool,
    history: Option<History>,
}
/// Displays connected in the background, then every display in each Mac's latest summary in the
/// shared folder (this Mac's own capture included, marked `own`). A cached copy of a display that
/// is live here, the primary included, is left out.
pub fn climate_sources(state: &State) -> Vec<ClimateSource> {
    let local = state.cloud.origin.as_deref();
    let mut out: Vec<ClimateSource> = state
        .fleet
        .values()
        .filter(|r| r.connected)
        .filter_map(|r| {
            r.climate
                .map(|(temperature, humidity, sampled_at)| ClimateSource {
                    label: r.name.clone(),
                    device: r.name.clone(),
                    temperature: Some(temperature),
                    humidity: Some(humidity),
                    sampled_at,
                    live: true,
                    own: false,
                    history: None,
                })
        })
        .collect();
    let live: Vec<String> = out
        .iter()
        .map(|s| s.device.clone())
        .chain(state.connected.then(|| state.bound_name.clone()))
        .collect();
    let mut latest: BTreeMap<&str, &Snapshot> = BTreeMap::new();
    for s in &state.cloud_data.snapshots {
        if s.climate().next().is_some()
            && latest
                .get(s.origin.id.as_str())
                .is_none_or(|old| old.day < s.day)
        {
            latest.insert(&s.origin.id, s);
        }
    }
    for s in latest.into_values() {
        for e in s.climate().filter(|e| !live.contains(&e.device)) {
            out.push(ClimateSource {
                label: source_label(&s.origin, local),
                device: e.device.clone(),
                temperature: e.temperature,
                humidity: e.humidity,
                sampled_at: e.sampled_at,
                live: false,
                own: local == Some(s.origin.id.as_str()),
                history: e.history.clone(),
            });
        }
    }
    out
}
pub fn presentation(mut state: State) -> State {
    if state.usage_resolved {
        return state;
    }
    state.cloud_data = crate::hub::catalog(&state);
    let (_, sources) = merged_readings(&state.readings, &state.cloud_data);
    (state.readings, state.harness_devices) = crate::hub::readings_in(&state, &state.cloud_data);
    state.usage_resolved = true;
    let local = state.cloud.origin.clone();
    state.cloud_sources.extend(
        sources
            .iter()
            .map(|(p, o)| (*p, source_label(o, local.as_deref()))),
    );
    if !state.connected {
        // A display connected in the background is live and always wins, so the card never
        // flips between rooms by a few seconds; otherwise the freshest cached summary.
        let (live, cached): (Vec<_>, Vec<_>) =
            climate_sources(&state).into_iter().partition(|c| c.live);
        let candidate = live
            .into_iter()
            .max_by_key(|c| c.sampled_at)
            .or_else(|| cached.into_iter().max_by_key(|c| c.sampled_at));
        if let Some(e) = candidate.filter(|e| {
            state
                .telemetry
                .as_ref()
                .is_none_or(|d| d.sampled_at < e.sampled_at)
        }) {
            state.environment_source = Some(e.label);
            state.telemetry = Some(crate::device::Telemetry {
                temperature: e.temperature,
                humidity: e.humidity,
                sampled_at: e.sampled_at,
                age_ms: 0,
                page: "environment".into(),
                pages: 0,
                rotation: 0,
                auto_rotate: false,
                paused: true,
                busy: false,
                partial: false,
                power: (0, 0, 0, -1),
                sd: (0, e.history.as_ref().map_or(0, |h| h.day), 0, 0, 0, 0),
                history: e.history.as_ref().map_or(0, |h| h.revision),
                shown: None,
                sample: None,
                received_at: e.sampled_at,
            });
            state.history = e.history.clone();
        }
    }
    state
}
fn timestamp_day(t: i64) -> Option<i64> {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| day_id((d + chrono::Duration::hours(8)).date_naive()))
}
pub fn codex_total(state: &State, day: i64) -> Option<u64> {
    codex_period_total(state, day, 0)
}
pub fn codex_week_total(state: &State, week: i64) -> Option<u64> {
    codex_period_total(state, week, 1)
}
pub fn codex_month_total(state: &State, month: i64) -> Option<u64> {
    codex_period_total(state, month, 2)
}
fn codex_period_total(state: &State, period: i64, scope: u8) -> Option<u64> {
    let value = |p: &Packet| match scope {
        1 => p.wt,
        2 => p.mt,
        _ => p.d,
    };
    // Select the latest VALID observation for this period per account. A newer Mac whose
    // CLI omitted monthly/daily usage must not erase an already-known numeric summary.
    let mut latest = BTreeMap::<String, Summary>::new();
    for s in state
        .cloud_data
        .snapshots
        .iter()
        .flat_map(|s| &s.usages)
        .cloned()
        .chain(
            state
                .readings
                .iter()
                .filter(|r| r.packet.p == 0)
                .filter_map(Summary::from_reading),
        )
        .filter(|s| {
            s.packet.p == 0 && value(&s.packet).is_some_and(|d| d[0] == period && d[1] >= 0)
        })
    {
        let Some(a) = &s.account else {
            continue;
        };
        let replace = latest.get(&a.key).is_none_or(|old| {
            (s.packet.t, &a.device.id) > (old.packet.t, &old.account.as_ref().unwrap().device.id)
        });
        if replace {
            latest.insert(a.key.clone(), s);
        }
    }
    let accounts: Vec<_> = latest.into_values().collect();
    if accounts.is_empty() {
        return state
            .readings
            .iter()
            .find(|r| r.packet.p == 0 && r.packet.s == usage::OK)
            .and_then(|r| value(&r.packet))
            .filter(|d| d[0] == period && d[1] >= 0)
            .map(|d| d[1] as u64);
    }
    let local_unknown = state
        .readings
        .iter()
        .find(|r| r.packet.p == 0)
        .and_then(|r| r.tokens.account.as_ref())
        .filter(|a| !a.verified)
        .map(|a| a.key.as_str());
    // Unknown account identities cannot be proven disjoint. Include at most the
    // current local source, and don't add it to known accounts from another Mac.
    let verified = accounts
        .iter()
        .any(|s| s.account.as_ref().is_some_and(|a| a.verified));
    let values = accounts
        .iter()
        .filter(|s| {
            s.account.as_ref().is_some_and(|a| {
                if verified {
                    a.verified
                } else {
                    Some(a.key.as_str()) == local_unknown
                }
            })
        })
        .filter_map(|s| value(&s.packet))
        .filter(|d| d[0] == period && d[1] >= 0)
        .map(|d| d[1] as u64)
        .collect::<Vec<_>>();
    (!values.is_empty())
        .then(|| values.into_iter().try_fold(0u64, u64::checked_add))
        .flatten()
}
pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    ensure!(
        f.metadata()?.is_file() && f.metadata()?.len() <= limit,
        "同步文档过大或类型无效"
    );
    let mut bytes = vec![];
    f.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "同步文档过大");
    Ok(serde_json::from_slice(&bytes)?)
}
pub(crate) fn atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp = path.with_file_name(format!(".homecraft-{:016x}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn load(root: &Path) -> Catalog {
    let current = read_json::<Catalog>(&root.join(".cloud-cache-v5.json"), MAX_HARNESS_CACHE)
        .or_else(|_| read_json::<Catalog>(&root.join(".cloud-cache-v4.json"), MAX_HARNESS_CACHE));
    let migrating = current.is_err();
    let value = current
        .or_else(|_| read_json::<Catalog>(&root.join(".cloud-cache-v3.json"), MAX_HARNESS_CACHE))
        .or_else(|_| read_json::<Catalog>(&root.join(".cloud-cache-v2.json"), MAX_CACHE))
        .or_else(|_| read_json::<Catalog>(&root.join(".cloud-cache.json"), MAX_CACHE))
        .unwrap_or_default();
    let mut result = Catalog::default();
    for mut s in value.snapshots.into_iter().take(MAX_SNAPSHOTS) {
        if migrating {
            s.usages
                .retain(|u| !u.harness_ledger.as_ref().is_some_and(|l| l.conflict));
        }
        result.merge(s);
    }
    result
}
pub fn cached_harness(root: &Path) -> Vec<DeviceUsage> {
    load(root).harness()
}
pub fn codex_historical_week_used(root: &Path, identity: &Identity, day: i64) -> Option<i64> {
    if !identity.verified || !identity.valid() || !valid_day(day) {
        return None;
    }
    load(root)
        .snapshots
        .iter()
        .filter(|s| s.day == day)
        .flat_map(|s| &s.usages)
        .filter(|s| {
            s.packet.p == 0
                && s.packet.s == usage::OK
                && timestamp_day(s.packet.t) == Some(day)
                && s.account
                    .as_ref()
                    .is_some_and(|a| a.verified && a.key == identity.key)
        })
        .filter(|s| {
            s.packet.w[1][1] == 10080
                && (0..=1000).contains(&s.packet.w[1][0])
                && (s.packet.w[1][2] == 0 || s.packet.w[1][2] > s.packet.t)
        })
        .max_by_key(|s| s.packet.t)
        .map(|s| 1000 - s.packet.w[1][0])
}
fn save_cache(root: &Path, catalog: &Catalog) -> bool {
    let save = |name: &str, value: &Catalog, limit: u64| {
        serde_json::to_vec(value)
            .ok()
            .filter(|b| b.len() as u64 <= limit)
            .is_some_and(|bytes| atomic(&root.join(name), &bytes).is_ok())
    };
    let mut compatible = catalog.clone();
    for s in &mut compatible.snapshots {
        s.version = if s.displays.is_empty() {
            FORMAT
        } else {
            DISPLAYS_FORMAT
        };
        // Rollbacks retain other providers without reintroducing additive Harness totals.
        s.usages.retain(|u| u.packet.p != 4);
    }
    let mut previous = catalog.clone();
    for s in &mut previous.snapshots {
        s.version = s.version.min(5);
        for u in &mut s.usages {
            if let Some(l) = &u.harness_ledger {
                u.harness_ledger = l.legacy().ok();
            }
        }
    }
    if !save(".cloud-cache-v5.json", catalog, MAX_HARNESS_CACHE)
        || !save(".cloud-cache-v4.json", &previous, MAX_HARNESS_CACHE)
        || !save(".cloud-cache-v3.json", &compatible, MAX_HARNESS_CACHE)
        || !save(".cloud-cache-v2.json", &compatible, MAX_CACHE)
    {
        return false;
    }
    for s in &mut compatible.snapshots {
        s.version = 1;
        s.displays.clear();
        for u in &mut s.usages {
            u.packet.mt = None;
        }
    }
    save(".cloud-cache.json", &compatible, MAX_CACHE)
}
/// One folder per kind of data, one file per Mac and day; the format version lives inside the
/// file. Earlier builds wrote every snapshot twice into HomeCraft-Data-v1/v2: those folders are
/// still read so a Mac that has not updated keeps showing up, but never written or created.
pub const USAGE_DIR: &str = "AI用量";
pub const CLIMATE_DIR: &str = "温湿度";
const LEGACY_DIRS: [&str; 2] = ["HomeCraft-Data-v2", "HomeCraft-Data-v1"];
/// Written snapshot format: per-account usage with monthly totals, or one environment summary.
const FORMAT: u8 = 2;
/// A climate file with more than one display adds `displays`, which build 42 can't read.
const DISPLAYS_FORMAT: u8 = 3;
fn folder_ok(folder: &Path) -> Result<()> {
    ensure!(
        folder.is_absolute() && fs::symlink_metadata(folder)?.is_dir(),
        "同步文件夹不可用，请重新选择"
    );
    Ok(())
}
fn directory(folder: &Path, name: &str) -> Result<PathBuf> {
    folder_ok(folder)?;
    let dir = folder.join(name);
    if !dir.exists() {
        fs::create_dir(&dir)?;
    }
    let m = fs::symlink_metadata(&dir)?;
    ensure!(
        m.is_dir() && !m.file_type().is_symlink(),
        "同步目录不能是链接"
    );
    Ok(dir)
}
/// An earlier build's folder, only when it is really there; never created.
fn legacy_directory(folder: &Path, name: &str) -> Option<PathBuf> {
    let dir = folder.join(name);
    fs::symlink_metadata(&dir)
        .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        .then_some(dir)
}
fn sync_days() -> (i64, i64) {
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    (day_id(today - chrono::Duration::days(89)), day_id(today))
}
/// Only publish our own partitions. Imported sources can never be echoed as our usage.
fn exchange(folder: &Path, catalog: &mut Catalog, origin: &Origin) -> Result<usize> {
    folder_ok(folder)?;
    let (usage, climate) = (
        directory(folder, USAGE_DIR)?,
        directory(folder, CLIMATE_DIR)?,
    );
    let mut invalid = 0;
    for dir in LEGACY_DIRS
        .iter()
        .filter_map(|name| legacy_directory(folder, name))
        .chain([usage.clone(), climate.clone()])
    {
        invalid += import(&dir, catalog)?;
    }
    let harness_dir = if usage.join("Harness").exists()
        || usage.join("Harness-v3").exists()
        || usage.join("Harness-v2").exists()
        || catalog.snapshots.iter().any(|s| {
            s.origin.id == origin.id && s.usages.iter().any(|u| u.harness_ledger.is_some())
        }) {
        Some(directory(&usage, "Harness")?)
    } else {
        None
    };
    let harness_v2 = harness_dir
        .as_ref()
        .map(|_| directory(&usage, "Harness-v2"))
        .transpose()?;
    let harness_v3 = harness_dir
        .as_ref()
        .map(|_| directory(&usage, "Harness-v3"))
        .transpose()?;
    let mut upgraded = std::collections::BTreeSet::new();
    if let Some(dir) = &harness_v3 {
        let (bad, sources) = import_harness(dir, catalog, 6, &upgraded)?;
        invalid += bad;
        upgraded.extend(sources);
    }
    if let Some(dir) = &harness_v2 {
        let (bad, sources) = import_harness(dir, catalog, 5, &upgraded)?;
        invalid += bad;
        upgraded.extend(sources);
    }
    if let Some(dir) = &harness_dir {
        invalid += import_harness(dir, catalog, 4, &upgraded)?.0;
    }
    let (earliest, latest) = sync_days();
    for snapshot in catalog
        .snapshots
        .iter()
        .filter(|s| s.origin.id == origin.id && s.day >= earliest && s.day <= latest)
    {
        let name = format!("{}-{}.json", snapshot.day, origin.id);
        let part =
            |usages: Vec<Summary>, environment: Option<Environment>, displays: Vec<_>| Snapshot {
                version: if displays.is_empty() {
                    FORMAT
                } else {
                    DISPLAYS_FORMAT
                },
                origin: snapshot.origin.clone(),
                day: snapshot.day,
                usages,
                environment,
                displays,
            };
        if !snapshot.usages.is_empty() {
            publish(
                &usage.join(&name),
                &part(
                    snapshot
                        .usages
                        .iter()
                        .filter(|u| u.harness_ledger.is_none())
                        .cloned()
                        .collect(),
                    None,
                    vec![],
                ),
            )?;
        }
        if snapshot.climate().next().is_some() {
            publish(
                &climate.join(&name),
                &part(
                    vec![],
                    snapshot.environment.clone(),
                    snapshot.displays.clone(),
                ),
            )?;
        }
    }
    if let Some(harness_dir) = harness_dir
        && let Some(snapshot) = catalog.snapshots.iter().find(|s| {
            s.origin.id == origin.id && s.usages.iter().any(|u| u.harness_ledger.is_some())
        })
    {
        let mut shared = snapshot.clone();
        shared.version = 6;
        shared.usages.retain(|u| u.harness_ledger.is_some());
        shared.environment = None;
        shared.displays.clear();
        publish_limit(
            &harness_v3
                .as_ref()
                .unwrap()
                .join(format!("{}.json", origin.id)),
            &shared,
            MAX_HARNESS_FILE,
        )?;
        for u in &mut shared.usages {
            if let Some(l) = &u.harness_ledger {
                u.harness_ledger = Some(l.legacy()?);
            }
        }
        shared.version = 5;
        publish_limit(
            &harness_v2
                .as_ref()
                .unwrap()
                .join(format!("{}.json", origin.id)),
            &shared,
            MAX_HARNESS_FILE,
        )?;
        if shared.usages.iter().all(|u| {
            u.harness_ledger
                .as_ref()
                .is_some_and(|l| l.version == 1 && !l.conflict)
        }) {
            shared.version = 4;
            publish_limit(
                &harness_dir.join(format!("{}.json", origin.id)),
                &shared,
                MAX_HARNESS_FILE,
            )?;
        }
    }
    Ok(invalid)
}
fn import_harness(
    dir: &Path,
    catalog: &mut Catalog,
    version: u8,
    skip: &std::collections::BTreeSet<String>,
) -> Result<(usize, std::collections::BTreeSet<String>)> {
    let (earliest, latest) = sync_days();
    let mut paths = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>();
    paths.sort();
    let mut invalid = 0;
    let mut sources = std::collections::BTreeSet::new();
    let mut budget = MAX_HARNESS_CACHE;
    for path in paths.into_iter().take(32) {
        let Some(raw) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name = raw
            .strip_prefix('.')
            .and_then(|n| n.strip_suffix(".icloud"))
            .unwrap_or(raw);
        let Some(id) = name.get(..16).filter(|s| {
            s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) else {
            continue;
        };
        if !name.ends_with(".json") {
            continue;
        }
        if skip.contains(id) {
            continue;
        }
        let path = path.with_file_name(name);
        let size = fs::symlink_metadata(&path).map_or(MAX_HARNESS_FILE, |m| m.len());
        if size > MAX_HARNESS_FILE || size > budget {
            invalid += 1;
            continue;
        }
        budget -= size;
        let read = epaper_macos::coordinated(&path, false, |p| {
            read_json::<Snapshot>(p, MAX_HARNESS_FILE)
                .map_err(|_| std::io::Error::other("Harness 同步明细无效"))
        });
        match read {
            Ok(s)
                if s.version == version
                    && s.valid()
                    && (version >= 5
                        || s.usages
                            .iter()
                            .all(|u| !u.harness_ledger.as_ref().is_some_and(|l| l.conflict)))
                    && s.origin.id == id
                    && s.day >= earliest
                    && s.day <= latest =>
            {
                sources.insert(s.origin.id.clone());
                catalog.merge(s)
            }
            _ => invalid += 1,
        }
    }
    Ok((invalid, sources))
}
/// Reads every day file in one folder (iCloud placeholders download on coordinated read).
fn import(dir: &Path, catalog: &mut Catalog) -> Result<usize> {
    let (earliest, latest) = sync_days();
    let files = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| {
                    let name = name
                        .strip_prefix('.')
                        .and_then(|s| s.strip_suffix(".icloud"))
                        .unwrap_or(name);
                    name.ends_with(".json")
                        && name
                            .get(..8)
                            .and_then(|s| s.parse::<i64>().ok())
                            .is_some_and(|d| d >= earliest && d <= latest)
                })
        })
        .take(MAX_SNAPSHOTS * 3)
        .collect::<Vec<_>>();
    let mut invalid = 0;
    let mut budget = MAX_CACHE;
    for path in files {
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let name = name
            .strip_prefix('.')
            .and_then(|s| s.strip_suffix(".icloud"))
            .unwrap_or(name);
        let path = path.with_file_name(name);
        // Also accepts iCloud conflict-copy suffixes, but never arbitrary hidden files.
        if name.starts_with('.') || !name.ends_with(".json") {
            continue;
        }
        let Some(day) = name.get(..8).and_then(|s| s.parse::<i64>().ok()) else {
            continue;
        };
        if day < earliest || day > latest {
            continue;
        }
        let size = fs::symlink_metadata(&path)
            .map(|m| m.len())
            .unwrap_or(MAX_FILE);
        if size > MAX_FILE || size > budget {
            invalid += 1;
            continue;
        }
        budget = budget.saturating_sub(size);
        let read = epaper_macos::coordinated(&path, false, |p| {
            read_json::<Snapshot>(p, MAX_FILE).map_err(|_| std::io::Error::other("同步文档无效"))
        });
        match read {
            Ok(s)
                if s.valid()
                    && s.day == day
                    && s.version <= 3
                    && s.usages.iter().all(|u| u.harness_ledger.is_none()) =>
            {
                catalog.merge(s)
            }
            _ => invalid += 1,
        }
    }
    Ok(invalid)
}
fn publish(path: &Path, snapshot: &Snapshot) -> Result<()> {
    publish_limit(path, snapshot, MAX_FILE)
}
fn publish_limit(path: &Path, snapshot: &Snapshot, limit: u64) -> Result<()> {
    let bytes = serde_json::to_vec(snapshot)?;
    ensure!(bytes.len() as u64 <= limit, "同步摘要过大");
    epaper_macos::coordinated(path, true, |p| {
        // Avoid a new upload when nothing changed.
        if fs::symlink_metadata(p)
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink() && m.len() <= limit)
            && fs::read(p).ok().as_deref() == Some(bytes.as_slice())
        {
            return Ok(());
        }
        atomic(p, &bytes)
    })?;
    Ok(())
}
#[derive(Clone)]
pub enum Command {
    Configure(Option<PathBuf>),
    Sync,
}
/// Names, sizes and modification times of the shared data folders. iCloud Drive syncs them on
/// its own; a changed fingerprint means another Mac's summary landed on disk and should be read.
fn fingerprint(folder: &Path) -> Vec<(String, u64, u128)> {
    let mut rows = vec![];
    for name in [
        USAGE_DIR,
        CLIMATE_DIR,
        "AI用量/Harness",
        "AI用量/Harness-v2",
        "AI用量/Harness-v3",
    ]
    .into_iter()
    .chain(LEGACY_DIRS)
    {
        let dir = folder.join(name);
        for e in fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            if let Ok(m) = e.metadata() {
                let at = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                rows.push((
                    format!("{name}/{}", e.file_name().to_string_lossy()),
                    m.len(),
                    at,
                ));
            }
        }
    }
    rows.sort();
    rows
}
pub fn spawn(root: PathBuf, events: Events) -> tokio::sync::mpsc::UnboundedSender<Command> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut settings =
            crate::config::read_private::<Settings>(&root.join(".cloud-settings.json"))
                .unwrap_or_default();
        let origin = match Origin::local(&root) {
            Ok(o) => o,
            Err(_) => return,
        };
        let mut catalog = load(&root);
        events.update(|s| {
            s.cloud.folder = settings.folder.clone();
            s.cloud.origin = Some(origin.id.clone());
            s.cloud_data = catalog.clone();
            s.harness_devices =
                crate::harness::merge_devices(&s.harness_devices, &catalog.harness());
        });
        let mut timer = tokio::time::interval(Duration::from_secs(30));
        let mut watch = tokio::time::interval(Duration::from_secs(5));
        let mut seen: Option<Vec<(String, u64, u128)>> = None;
        let mut job: Option<tokio::task::JoinHandle<(Catalog, Result<usize>)>> = None;
        let mut pending = true;
        let mut generation = 0u64;
        let mut job_generation = 0;
        loop {
            tokio::select! {
                command=rx.recv()=>match command {
                    None=>break,
                    Some(Command::Configure(folder))=> {
                        let next=Settings{folder};
                        if atomic(&root.join(".cloud-settings.json"),&serde_json::to_vec(&next).unwrap_or_default()).is_err(){events.update(|s|{s.cloud.error=true;s.cloud.message="设置未保存，请重试".into();});continue;}
                        settings=next;generation+=1;pending=true;
                        events.update(|s|{s.cloud.folder=settings.folder.clone();s.cloud.busy=false;s.cloud.error=false;s.cloud.message=if settings.folder.is_some(){"等待同步"}else{"已暂停"}.into();});
                    }
                    Some(Command::Sync)=>pending=true,
                },
                _=timer.tick()=>pending=true,
                _=watch.tick()=> {
                    if let Some(folder)=settings.folder.as_ref() {
                        let print=fingerprint(folder);
                        if seen.as_ref().is_some_and(|old| *old!=print) {pending=true;}
                        seen=Some(print);
                    }
                }
                result=async {job.as_mut().unwrap().await},if job.is_some()=> {
                    job=None;
                    if let Ok((next,result))=result {
                        catalog=next;
                        if job_generation==generation {
                            events.update(|s| {
                                s.cloud_data=catalog.clone();
                                s.harness_devices=crate::harness::merge_devices(&s.harness_devices,&catalog.harness());
                                s.cloud.busy=false;
                                s.cloud.error=result.is_err();
                                if settings.folder.is_some(){match result {
                                    Ok(invalid)=> {s.cloud.last=Some(usage::now());s.cloud.message=if invalid>0{format!("已写入 · {invalid} 个文件待重试")}else{"已写入 iCloud Drive".into()};},
                                    Err(_)=>s.cloud.message="暂未完成，将自动重试".into(),
                                }}
                            });
                        }
                    } else {events.update(|s|{s.cloud.busy=false;s.cloud.error=true;s.cloud.message="同步中断，将自动重试".into();});}
                }
            }
            if pending && job.is_none() {
                pending = false;
                job_generation = generation;
                let state = events.state.lock().ok().map(|s| s.clone());
                let Some(state) = state else {
                    continue;
                };
                let (root, origin, mut next, folder) = (
                    root.clone(),
                    origin.clone(),
                    catalog.clone(),
                    settings.folder.clone(),
                );
                events.update(|s| s.cloud.busy = folder.is_some());
                job = Some(tokio::task::spawn_blocking(move || {
                    next.capture(&state, origin.clone());
                    let result = folder.as_ref().map_or(Ok(0), |folder| {
                        ensure!(
                            epaper_macos::is_icloud_directory(folder),
                            "iCloud Drive 文件夹暂不可用"
                        );
                        exchange(folder, &mut next, &origin)
                    });
                    let saved = save_cache(&root, &next);
                    (
                        next,
                        if saved {
                            result
                        } else {
                            Err(anyhow::anyhow!("本机数据未保存"))
                        },
                    )
                }));
            }
        }
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn origin(id: &str) -> Origin {
        Origin {
            id: id.into(),
            name: "Mac-1".into(),
        }
    }
    fn summary(id: &str, mail: &str, t: i64, tokens: i64) -> Summary {
        let mut p = Packet::new(0, usage::OK);
        p.t = t;
        p.d = Some([crate::home::model::day(), tokens]);
        Summary {
            packet: p,
            account: Identity::parse(
                Some(&json!({"account":{"type":"chatgpt","email":mail,"planType":"plus"}})),
                origin(id),
            ),
            lifetime: Some(2000),
            peak_daily: None,
            context: None,
            harness_ledger: None,
            breakdown: None,
        }
    }
    fn snap(id: &str, usages: Vec<Summary>) -> Snapshot {
        Snapshot {
            version: 1,
            origin: origin(id),
            day: crate::home::model::day(),
            usages,
            environment: None,
            displays: vec![],
        }
    }
    fn display(device: &str, at: i64, temperature: f32) -> Environment {
        Environment {
            device: device.into(),
            sampled_at: at,
            temperature: Some(temperature),
            humidity: Some(40.),
            history: None,
        }
    }
    /// The primary display connected with a 15-minute history, plus one background session.
    fn two_displays(now: i64) -> State {
        let mut points = vec![None; 96];
        points[3] = Some((23.5, 44.));
        let mut state = State {
            bound_name: "EPaper_B96AD4".into(),
            history: Some(History {
                day: crate::home::model::day() as u32,
                revision: 1,
                points,
                received_at: now - 10,
            }),
            telemetry: Some(crate::device::Telemetry {
                temperature: Some(24.),
                humidity: Some(45.),
                sampled_at: now - 10,
                age_ms: 0,
                page: "environment".into(),
                pages: 0,
                rotation: 0,
                auto_rotate: false,
                paused: false,
                busy: false,
                partial: false,
                power: (0, 0, 0, -1),
                sd: (0, 0, 0, 0, 0, 0),
                history: 1,
                shown: None,
                sample: None,
                received_at: now - 10,
            }),
            ..Default::default()
        };
        state.fleet.insert(
            "e1002".into(),
            crate::backend::FleetStatus {
                name: "EPaper_EC240E".into(),
                connected: true,
                climate: Some((30.4, 48.4, now - 20)),
                ..Default::default()
            },
        );
        state
    }
    /// The snapshot schema of build 42 and earlier, which reject unknown fields.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Build42Snapshot {
        version: u8,
        origin: Origin,
        day: i64,
        usages: Vec<Summary>,
        environment: Option<Environment>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Build42Catalog {
        snapshots: Vec<Build42Snapshot>,
    }
    #[test]
    fn same_codex_account_replaces_per_device_does_not_sum() {
        let mut c = Catalog::default();
        let t = usage::now();
        c.merge(snap(
            "1111111111111111",
            vec![summary("1111111111111111", "alice@example.test", t, 10)],
        ));
        c.merge(snap(
            "2222222222222222",
            vec![summary("2222222222222222", "alice@example.test", t + 1, 20)],
        ));
        assert_eq!(c.accounts(&[]).len(), 1);
        assert_eq!(c.accounts(&[])[0].packet.d.unwrap()[1], 20);
        c.merge(snap(
            "2222222222222222",
            vec![summary("2222222222222222", "bob@example.test", t + 2, 30)],
        ));
        assert_eq!(c.accounts(&[]).len(), 2);
        c.merge(snap(
            "1111111111111111",
            vec![summary("1111111111111111", "alice@example.test", t - 1, 5)],
        ));
        assert_eq!(
            c.accounts(&[])
                .iter()
                .find(|s| s.account.as_ref().unwrap().label == "ali***")
                .unwrap()
                .packet
                .d
                .unwrap()[1],
            20
        );
    }
    #[test]
    fn safe_schema_drops_notes_full_email_and_rejects_extra_fields() {
        let mut r = usage::empty(0, usage::OK, "SECRET note");
        r.source = "SECRET credential".into();
        r.tokens.note = "SECRET prompt".into();
        r.tokens.account =
            summary("1111111111111111", "alice@example.test", usage::now(), 1).account;
        let safe = Summary::from_reading(&r).unwrap();
        let bytes = serde_json::to_string(&safe).unwrap();
        for s in ["SECRET", "alice@", "example.test", "note", "source"] {
            assert!(!bytes.contains(s));
        }
        let mut value = serde_json::to_value(&safe).unwrap();
        value["api_key"] = json!("secret");
        assert!(serde_json::from_value::<Summary>(value).is_err());
    }
    #[test]
    fn two_macs_exchange_owned_files_and_survive_conflict_copies() {
        let temp = std::env::temp_dir().join(format!(
            "homecraft-sync-test-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&temp).unwrap();
        let a = origin("1111111111111111");
        let b = origin("2222222222222222");
        let t = usage::now();
        let mut x = Catalog::default();
        x.merge(snap(
            &a.id,
            vec![summary(&a.id, "alice@example.test", t, 10)],
        ));
        exchange(&temp, &mut x, &a).unwrap();
        let mut y = Catalog::default();
        y.merge(snap(
            &b.id,
            vec![summary(&b.id, "alice@example.test", t + 1, 20)],
        ));
        exchange(&temp, &mut y, &b).unwrap();
        exchange(&temp, &mut x, &a).unwrap();
        assert_eq!(x.accounts(&[])[0].packet.d.unwrap()[1], 20);
        let dir = directory(&temp, USAGE_DIR).unwrap();
        let path = dir.join(format!(
            "{}-{} conflict.json",
            crate::home::model::day(),
            a.id
        ));
        atomic(
            &path,
            &serde_json::to_vec(&snap(
                &a.id,
                vec![summary(&a.id, "alice@example.test", t - 1, 5)],
            ))
            .unwrap(),
        )
        .unwrap();
        exchange(&temp, &mut y, &b).unwrap();
        assert_eq!(y.accounts(&[])[0].packet.d.unwrap()[1], 20);
        assert_eq!(fs::read_dir(dir).unwrap().count(), 3);
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn sparse_sensor_history_does_not_erase_other_observations() {
        let mut a = snap("1111111111111111", vec![]);
        let mut points = vec![None; 96];
        points[0] = Some((20., 40.));
        a.environment = Some(Environment {
            device: "EPaper_123ABC".into(),
            sampled_at: usage::now(),
            temperature: Some(20.),
            humidity: Some(40.),
            history: Some(History {
                day: crate::home::model::day() as u32,
                revision: 1,
                points,
                received_at: usage::now(),
            }),
        });
        let mut b = a.clone();
        let h = b.environment.as_mut().unwrap().history.as_mut().unwrap();
        h.points[0] = None;
        h.points[1] = Some((21., 41.));
        h.received_at += 1;
        a.merge(&b);
        let p = &a.environment.unwrap().history.unwrap().points;
        assert!(p[0].is_some() && p[1].is_some());
    }
    #[test]
    fn totals_deduplicate_accounts_and_do_not_turn_missing_days_into_zero() {
        let mut state = State::default();
        let t = usage::now();
        let day = crate::home::model::day();
        state.cloud_data.merge(snap(
            "1111111111111111",
            vec![summary("1111111111111111", "alice@example.test", t, 10)],
        ));
        state.cloud_data.merge(snap(
            "2222222222222222",
            vec![
                summary("2222222222222222", "alice@example.test", t + 1, 20),
                summary("2222222222222222", "bob@example.test", t + 1, 30),
            ],
        ));
        assert_eq!(codex_total(&state, day), Some(50));
        assert_eq!(codex_total(&state, day + 1), None);
        assert_eq!(crate::home::model::today_total(&state, day), Some(50));
    }
    #[test]
    fn weekly_codex_totals_keep_account_deduplication_and_week_boundary() {
        let mut state = State::default();
        let now = usage::now();
        let week = crate::home::model::week_start(crate::home::model::day()).unwrap();
        let mut a = summary("1111111111111111", "alice@example.test", now, 1);
        a.packet.wt = Some([week, 100]);
        let mut newer = summary("2222222222222222", "alice@example.test", now + 1, 2);
        newer.packet.wt = Some([week, 120]);
        let mut other = summary("2222222222222222", "bob@example.test", now + 1, 3);
        other.packet.wt = Some([week, 50]);
        state.cloud_data.merge(snap("1111111111111111", vec![a]));
        state
            .cloud_data
            .merge(snap("2222222222222222", vec![newer, other]));
        assert_eq!(codex_week_total(&state, week), Some(170));
        assert_eq!(codex_week_total(&state, 20990105), None);
    }
    #[test]
    fn monthly_codex_totals_deduplicate_and_prefer_equal_time_month_data() {
        let mut state = State::default();
        let month = crate::home::model::month_start(crate::home::model::day()).unwrap();
        let mut a = summary("1111111111111111", "alice@example.test", usage::now(), 1);
        a.packet.mt = Some([month, 120]);
        let mut legacy = summary("2222222222222222", "alice@example.test", a.packet.t, 1);
        state.cloud_data.merge(snap("1111111111111111", vec![a]));
        state
            .cloud_data
            .merge(snap("2222222222222222", vec![legacy.clone()]));
        assert_eq!(codex_month_total(&state, month), Some(120));
        legacy.packet.t += 1;
        legacy.packet.mt = Some([month, 150]);
        let mut other = summary("2222222222222222", "bob@example.test", legacy.packet.t, 1);
        other.packet.mt = Some([month, 0]);
        state
            .cloud_data
            .merge(snap("2222222222222222", vec![legacy, other]));
        assert_eq!(codex_month_total(&state, month), Some(150));
        assert_eq!(codex_month_total(&state, 20990101), None);
    }
    #[test]
    fn yesterday_quota_requires_matching_verified_account_and_last_valid_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "homecraft-yesterday-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&root).unwrap();
        let t = usage::now() - 86400;
        let day = timestamp_day(t).unwrap();
        let mut a = summary("1111111111111111", "alice@example.test", t, 0);
        a.packet.d = Some([day, 0]);
        a.packet.w[1] = [720, 10080, t + 86400 * 7];
        let identity = a.account.clone().unwrap();
        let mut c = Catalog::default();
        let mut s = snap("1111111111111111", vec![a.clone()]);
        s.day = day;
        c.merge(s);
        assert!(save_cache(&root, &c));
        assert_eq!(codex_historical_week_used(&root, &identity, day), Some(280));
        assert_eq!(
            codex_historical_week_used(&root, &identity, crate::home::model::day()),
            None
        );
        let other = summary("1111111111111111", "bob@example.test", t, 0)
            .account
            .unwrap();
        assert_eq!(codex_historical_week_used(&root, &other, day), None);
        a.packet.t += 1;
        a.packet.w[1][0] = 1000;
        a.packet.yd = Some([day - 1, 123, 456]);
        let r = Reading {
            packet: a.packet.clone(),
            tokens: usage::TokenStats {
                account: a.account.clone(),
                ..Default::default()
            },
            source: "test".into(),
        };
        let exported = Summary::from_reading(&r).unwrap();
        assert!(exported.packet.yd.is_none());
        a.packet.yd = None;
        let mut s = snap("1111111111111111", vec![a]);
        s.day = day;
        c.merge(s);
        assert!(save_cache(&root, &c));
        assert_eq!(codex_historical_week_used(&root, &identity, day), Some(0));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn exports_one_copy_per_kind_and_never_create_old_folders() {
        let temp = std::env::temp_dir().join(format!(
            "homecraft-month-export-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&temp).unwrap();
        let a = origin("1111111111111111");
        let month = crate::home::model::month_start(crate::home::model::day()).unwrap();
        let mut u = summary(&a.id, "alice@example.test", usage::now(), 1);
        u.packet.mt = Some([month, 123]);
        let mut c = Catalog::default();
        c.merge(snap(&a.id, vec![u]));
        exchange(&temp, &mut c, &a).unwrap();
        let name = format!("{}-{}.json", crate::home::model::day(), a.id);
        let exported: Snapshot = read_json(&temp.join(USAGE_DIR).join(&name), MAX_FILE).unwrap();
        assert_eq!(exported.usages[0].packet.mt, Some([month, 123]));
        assert!(exported.environment.is_none());
        // No environment yet, so no climate file; the old folders are never created.
        assert!(!temp.join(CLIMATE_DIR).join(&name).exists());
        assert!(LEGACY_DIRS.iter().all(|d| !temp.join(d).exists()));
        assert!(save_cache(&temp, &c));
        assert_eq!(load(&temp).accounts(&[])[0].packet.mt, Some([month, 123]));
        let legacy: Catalog = read_json(&temp.join(".cloud-cache.json"), MAX_CACHE).unwrap();
        assert_eq!(legacy.accounts(&[])[0].packet.mt, None);
        let mut imported = Catalog::default();
        exchange(&temp, &mut imported, &origin("2222222222222222")).unwrap();
        assert_eq!(imported.accounts(&[])[0].packet.mt, Some([month, 123]));
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn a_mac_on_an_older_build_is_still_read_and_climate_gets_its_own_folder() {
        let temp = std::env::temp_dir().join(format!(
            "homecraft-legacy-read-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&temp).unwrap();
        let now = usage::now();
        let reading = |temperature: f32| Environment {
            device: "EPaper_B96AD4".into(),
            sampled_at: now,
            temperature: Some(temperature),
            humidity: Some(42.),
            history: None,
        };
        // An older build on another Mac: the same day written to both legacy folders.
        let mut old = snap("2222222222222222", vec![]);
        old.environment = Some(reading(28.9));
        let name = format!("{}-{}.json", old.day, old.origin.id);
        for dir in LEGACY_DIRS {
            fs::create_dir(temp.join(dir)).unwrap();
            atomic(
                &temp.join(dir).join(&name),
                &serde_json::to_vec(&old).unwrap(),
            )
            .unwrap();
        }
        let a = origin("1111111111111111");
        let mut own = snap(&a.id, vec![]);
        own.environment = Some(reading(30.4));
        let mut c = Catalog::default();
        c.merge(own);
        assert_eq!(exchange(&temp, &mut c, &a).unwrap(), 0);
        let theirs = c
            .snapshots
            .iter()
            .find(|s| s.origin.id == old.origin.id)
            .unwrap();
        assert_eq!(theirs.environment.as_ref().unwrap().temperature, Some(28.9));
        let mine = format!("{}-{}.json", c.snapshots[0].day, a.id);
        let written: Snapshot = read_json(&temp.join(CLIMATE_DIR).join(&mine), MAX_FILE).unwrap();
        assert!(written.usages.is_empty());
        assert_eq!(written.environment.unwrap().temperature, Some(30.4));
        // Nothing of ours goes into the old folders, and an empty usage day writes no file.
        assert_eq!(fs::read_dir(temp.join(LEGACY_DIRS[0])).unwrap().count(), 1);
        assert!(!temp.join(USAGE_DIR).join(&mine).exists());
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn harness_sidecars_union_copies_survive_restart_and_never_echo_peer_usage() {
        let temp =
            std::env::temp_dir().join(format!("homecraft-calls-{:016x}", rand::random::<u64>()));
        fs::create_dir(&temp).unwrap();
        let a = origin("1111111111111111");
        let b = origin("2222222222222222");
        let first = crate::harness::test_reading(&a.id, 100);
        let mut second = first.clone();
        second.packet.o = Some(b.clone());
        // A synchronized log may replay the same billed attempt later that day.
        std::sync::Arc::make_mut(&mut second.tokens.harness_ledger.as_mut().unwrap().calls)[0]
            .at += 95 * 60 * 1000;
        let mut x = Catalog::default();
        x.merge(snap(
            &a.id,
            vec![
                Summary::from_reading(&first).unwrap(),
                summary(&a.id, "a@example.test", usage::now(), 10),
            ],
        ));
        assert_eq!(exchange(&temp, &mut x, &a).unwrap(), 0);
        let path_a = temp
            .join(USAGE_DIR)
            .join("Harness")
            .join(format!("{}.json", a.id));
        assert!(path_a.is_file());
        let raw = fs::read_to_string(&path_a).unwrap();
        assert!(!raw.contains("source:") && !raw.contains("prompt") && !raw.contains("messages"));
        let ordinary =
            temp.join(USAGE_DIR)
                .join(format!("{}-{}.json", crate::home::model::day(), a.id));
        let old: Snapshot = read_json(&ordinary, MAX_FILE).unwrap();
        assert!(
            old.usages
                .iter()
                .all(|u| u.packet.p != 4 && u.harness_ledger.is_none())
        );
        let mut y = Catalog::default();
        y.merge(snap(&b.id, vec![Summary::from_reading(&second).unwrap()]));
        assert_eq!(exchange(&temp, &mut y, &b).unwrap(), 0);
        assert_eq!(fs::read(&path_a).unwrap(), raw.as_bytes());
        let resolved = crate::harness::effective(&second, &y.harness());
        assert_eq!(resolved.packet.d.unwrap()[1], 100);
        assert!(Summary::from_reading(&resolved).is_none());
        let path_b = temp
            .join(USAGE_DIR)
            .join("Harness")
            .join(format!("{}.json", b.id));
        let published: Snapshot = read_json(&path_b, MAX_HARNESS_FILE).unwrap();
        assert_eq!(
            published.usages[0]
                .harness_ledger
                .as_ref()
                .unwrap()
                .calls
                .len(),
            1
        );
        assert_eq!(published.usages[0].packet.o.as_ref().unwrap().id, b.id);
        assert_eq!(
            published.usages[0].harness_ledger,
            second.tokens.harness_ledger
        );
        fs::copy(
            &path_a,
            path_a.with_file_name(format!("{} (conflict copy).json", a.id)),
        )
        .unwrap();
        assert_eq!(exchange(&temp, &mut y, &b).unwrap(), 0);
        assert!(save_cache(&temp, &y));
        let restored = load(&temp);
        assert_eq!(
            crate::harness::effective(&usage::empty(4, 0, "no local logs"), &restored.harness())
                .packet
                .d
                .unwrap()[1],
            100
        );
        // Normal provider data merging into a sidecar-first snapshot stays valid on reload.
        y.merge(snap(
            &b.id,
            vec![summary(&b.id, "b@example.test", usage::now(), 20)],
        ));
        assert!(y.snapshots.iter().all(Snapshot::valid));
        assert!(save_cache(&temp, &y));
        assert_eq!(load(&temp).accounts(&[]).len(), 2);
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn date_spans_sync_in_versioned_sidecars_and_survive_cache_reload() {
        let temp = std::env::temp_dir().join(format!(
            "homecraft-date-spans-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&temp).unwrap();
        let a = origin("1111111111111111");
        let day = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let first =
            crate::harness::test_reading_on(&a.id, 100, day, day - chrono::Duration::days(1));
        let second = crate::harness::test_reading_on(&a.id, 100, day, day);
        let mut c = Catalog::default();
        c.merge(snap(&a.id, vec![Summary::from_reading(&first).unwrap()]));
        c.merge(snap(&a.id, vec![Summary::from_reading(&second).unwrap()]));
        let source = c.harness().remove(0);
        assert!(source.valid() && source.ledger.as_ref().unwrap().ambiguous());
        assert!(!source.ledger.as_ref().unwrap().conflict);
        assert_eq!(exchange(&temp, &mut c, &a).unwrap(), 0);
        let new_path = temp
            .join(USAGE_DIR)
            .join("Harness-v2")
            .join(format!("{}.json", a.id));
        let shared: Snapshot = read_json(&new_path, MAX_HARNESS_FILE).unwrap();
        assert_eq!(shared.version, 5);
        assert_eq!(shared.usages[0].harness_ledger.as_ref().unwrap().version, 2);
        assert!(
            !temp
                .join(USAGE_DIR)
                .join("Harness")
                .join(format!("{}.json", a.id))
                .exists()
        );
        let mut peer = Catalog::default();
        exchange(&temp, &mut peer, &origin("2222222222222222")).unwrap();
        assert!(save_cache(&temp, &peer));
        let reloaded = load(&temp);
        let r = crate::harness::effective_on(&usage::empty(4, 0, ""), &reloaded.harness(), day);
        assert_eq!(r.packet.s, usage::OK);
        assert!(r.packet.d.is_none());
        assert_eq!(r.packet.wt, Some([20261019, 100]));
        assert_eq!(r.packet.mt, Some([20261001, 100]));
        let view = &r.tokens.device_periods[&a.id];
        assert!(view.d.is_none());
        assert_eq!(view.mt, r.packet.mt);
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn original_usage_time_survives_sidecars_legacy_peers_and_cache_reload() {
        let temp = std::env::temp_dir().join(format!(
            "homecraft-original-time-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&temp).unwrap();
        let a = origin("1111111111111111");
        let b = origin("2222222222222222");
        let day = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
        // Keep this exchange fixture inside the live retention window even when
        // the test runs on a Monday that is also the first day of a month.
        let start = day
            .with_day(1)
            .unwrap()
            .min(day - chrono::Duration::days(i64::from(day.weekday().num_days_from_monday())));
        let billed = (day - chrono::Duration::days(1)).max(start);
        let mut original = crate::harness::test_reading_on(&a.id, 100, day, billed);
        let ledger = original.tokens.harness_ledger.as_mut().unwrap();
        ledger.version = 3;
        let call = &mut std::sync::Arc::make_mut(&mut ledger.calls)[0];
        call.usage_time = true;
        call.retain_day = Some(day_id(day));
        let mut source = Catalog::default();
        source.merge(snap(&a.id, vec![Summary::from_reading(&original).unwrap()]));
        assert_eq!(exchange(&temp, &mut source, &a).unwrap(), 0);
        let sidecar =
            |folder: &str, id: &str| temp.join(USAGE_DIR).join(folder).join(format!("{id}.json"));
        let shared: Snapshot = read_json(&sidecar("Harness-v3", &a.id), MAX_HARNESS_FILE).unwrap();
        assert_eq!(shared.version, 6);
        assert_eq!(
            shared.usages[0].harness_ledger,
            original.tokens.harness_ledger
        );
        let legacy: Snapshot = read_json(&sidecar("Harness-v2", &a.id), MAX_HARNESS_FILE).unwrap();
        assert_eq!(legacy.version, 5);
        let old_ledger = legacy.usages[0].harness_ledger.as_ref().unwrap();
        assert!(old_ledger.version <= 2 && old_ledger.valid());
        assert!(
            old_ledger
                .calls
                .iter()
                .all(|c| !c.usage_time && c.retain_day.is_none())
        );

        // A stale lower-format file must not override the upgraded source, and an
        // older peer can still contribute an envelope-only copy of the same call.
        let retimed = crate::harness::test_reading_on(&a.id, 100, day, day);
        let mut stale = snap(&a.id, vec![Summary::from_reading(&retimed).unwrap()]);
        stale.version = 5;
        publish_limit(&sidecar("Harness-v2", &a.id), &stale, MAX_HARNESS_FILE).unwrap();
        let mut peer_reading = crate::harness::test_reading_on(&b.id, 100, day, day);
        std::sync::Arc::make_mut(&mut peer_reading.tokens.harness_ledger.as_mut().unwrap().calls)
            [0]
        .key = original.tokens.harness_ledger.as_ref().unwrap().calls[0]
            .key
            .clone();
        let mut peer = Catalog::default();
        peer.merge(snap(
            &b.id,
            vec![Summary::from_reading(&peer_reading).unwrap()],
        ));
        assert_eq!(exchange(&temp, &mut peer, &b).unwrap(), 0);
        assert!(save_cache(&temp, &peer));
        let restored = load(&temp);
        let resolved =
            crate::harness::effective_on(&usage::empty(4, 0, ""), &restored.harness(), day);
        assert_eq!(resolved.packet.s, usage::OK);
        assert_eq!(resolved.packet.d, original.packet.d);
        assert_eq!(resolved.packet.wt, original.packet.wt);
        assert_eq!(resolved.packet.mt, original.packet.mt);
        let restored_source = restored
            .harness()
            .into_iter()
            .find(|d| d.id == a.id)
            .unwrap();
        assert!(restored_source.ledger.unwrap().calls[0].usage_time);
        let published_peer: Snapshot =
            read_json(&sidecar("Harness-v3", &b.id), MAX_HARNESS_FILE).unwrap();
        assert!(
            !published_peer.usages[0]
                .harness_ledger
                .as_ref()
                .unwrap()
                .calls[0]
                .usage_time
        );
        let compatible: Catalog =
            read_json(&temp.join(".cloud-cache-v4.json"), MAX_HARNESS_CACHE).unwrap();
        assert!(compatible.snapshots.iter().all(Snapshot::valid));
        assert!(compatible.harness().iter().all(|d| {
            d.ledger.as_ref().is_some_and(|l| {
                l.version <= 2
                    && l.calls
                        .iter()
                        .all(|c| !c.usage_time && c.retain_day.is_none())
            })
        }));
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn migration_rebuilds_legacy_conflict_markers_but_new_cache_preserves_real_conflicts() {
        let temp = std::env::temp_dir().join(format!(
            "homecraft-conflict-migration-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&temp).unwrap();
        let a = origin("1111111111111111");
        let mut r = crate::harness::test_reading(&a.id, 100);
        r.tokens.harness_ledger.as_mut().unwrap().conflict = true;
        let mut c = Catalog::default();
        c.merge(snap(
            &a.id,
            vec![
                Summary::from_reading(&r).unwrap(),
                summary(&a.id, "a@example.test", usage::now(), 10),
            ],
        ));
        assert!(save_cache(&temp, &c));
        assert!(load(&temp).harness()[0].ledger.as_ref().unwrap().conflict);
        fs::remove_file(temp.join(".cloud-cache-v5.json")).unwrap();
        fs::remove_file(temp.join(".cloud-cache-v4.json")).unwrap();
        atomic(
            &temp.join(".cloud-cache-v3.json"),
            &serde_json::to_vec(&c).unwrap(),
        )
        .unwrap();
        let migrated = load(&temp);
        assert!(migrated.harness().is_empty());
        assert_eq!(migrated.accounts(&[]).len(), 1);
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn harness_same_mac_conflict_marks_unknown_without_losing_other_providers() {
        let id = "1111111111111111";
        let first = crate::harness::test_reading(id, 100);
        let mut other = crate::harness::test_reading(id, 101);
        other.packet.t += 1;
        // Equal logical revisions with different reported usage are contradictory.
        let mut c = Catalog::default();
        c.merge(snap(
            id,
            vec![
                Summary::from_reading(&first).unwrap(),
                summary(id, "a@example.test", usage::now(), 10),
            ],
        ));
        c.merge(snap(id, vec![Summary::from_reading(&other).unwrap()]));
        assert_eq!(c.accounts(&[]).len(), 1);
        let r = crate::harness::effective(&first, &c.harness());
        assert_eq!(r.packet.s, 6);
        assert!(r.packet.d.is_none());
        assert!(c.snapshots.iter().all(Snapshot::valid));
    }
    #[test]
    fn cached_remote_harness_is_not_exported_as_local_usage() {
        let mut state = State::default();
        let mut p = Packet::new(4, usage::OK);
        p.n = Some(crate::harness::NAME.into());
        p.o = Some(origin("2222222222222222"));
        state.readings.push(Reading {
            packet: p,
            tokens: Default::default(),
            source: "remote".into(),
        });
        let mut c = Catalog::default();
        c.capture(&state, origin("1111111111111111"));
        assert!(c.snapshots[0].usages.is_empty());
    }
    #[test]
    fn files_cannot_redirect_into_private_configuration() {
        let temp = std::env::temp_dir().join(format!(
            "homecraft-sync-symlink-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir(&temp).unwrap();
        let target = temp.join("private.json");
        fs::write(&target, b"private").unwrap();
        let link = temp.join("link.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(read_json::<Snapshot>(&link, MAX_FILE).is_err());
        std::os::unix::fs::symlink(&target, temp.join(USAGE_DIR)).unwrap();
        assert!(directory(&temp, USAGE_DIR).is_err());
        std::os::unix::fs::symlink(&temp, temp.join(LEGACY_DIRS[0])).unwrap();
        assert!(legacy_directory(&temp, LEGACY_DIRS[0]).is_none());
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn climate_prefers_a_live_display_and_never_calls_this_mac_icloud() {
        let now = usage::now();
        let reading = |at: i64, temperature: f32| Environment {
            device: "EPaper_123ABC".into(),
            sampled_at: at,
            temperature: Some(temperature),
            humidity: Some(40.),
            history: None,
        };
        let mut state = State::default();
        state.cloud.origin = Some("1111111111111111".into());
        let mut own = snap("1111111111111111", vec![]);
        own.environment = Some(reading(now - 100, 20.));
        state.cloud_data.merge(own);
        let view = presentation(state.clone());
        assert_eq!(view.environment_source.as_deref(), Some("本机缓存"));
        let mut other = snap("2222222222222222", vec![]);
        other.environment = Some(reading(now - 50, 21.));
        state.cloud_data.merge(other);
        let view = presentation(state.clone());
        assert_eq!(view.environment_source.as_deref(), Some("iCloud · Mac-1"));
        assert_eq!(view.telemetry.unwrap().temperature, Some(21.));
        // One row per Mac, its latest reading; this Mac's own capture is flagged, not a room.
        let mut older = snap("2222222222222222", vec![]);
        older.environment = Some(reading(now - 80, 19.));
        state.cloud_data.merge(older);
        let sources = climate_sources(&state);
        assert_eq!(sources.len(), 2);
        assert!(sources.iter().any(|s| s.own && s.label == "本机缓存"));
        assert!(sources.iter().any(|s| !s.own && s.temperature == Some(21.)));
        // A display connected in the background wins even when a summary is a little newer.
        state.fleet.insert(
            "e1002".into(),
            crate::backend::FleetStatus {
                name: "EPaper_EC240E".into(),
                connected: true,
                climate: Some((27.5, 55., now - 500)),
                ..Default::default()
            },
        );
        let view = presentation(state);
        assert_eq!(view.environment_source.as_deref(), Some("EPaper_EC240E"));
        assert_eq!(view.telemetry.unwrap().temperature, Some(27.5));
        assert!(view.history.is_none());
    }
    #[test]
    fn every_display_is_captured_once_and_another_mac_lists_each() {
        let temp =
            std::env::temp_dir().join(format!("homecraft-displays-{:016x}", rand::random::<u64>()));
        fs::create_dir(&temp).unwrap();
        let now = usage::now();
        let a = origin("1111111111111111");
        let mut state = two_displays(now);
        // A stale row for the primary itself must not add a second entry for it.
        state.fleet.insert(
            "old".into(),
            crate::backend::FleetStatus {
                name: "EPaper_B96AD4".into(),
                climate: Some((22., 41., now - 900)),
                ..Default::default()
            },
        );
        let mut c = Catalog::default();
        c.capture(&state, a.clone());
        let s = &c.snapshots[0];
        assert_eq!(s.climate().count(), 2);
        let primary = s.climate().find(|e| e.device == "EPaper_B96AD4").unwrap();
        assert_eq!(primary.temperature, Some(24.));
        assert!(primary.history.is_some());
        // A later reading of one display updates only that entry; the history stays.
        state.fleet.get_mut("e1002").unwrap().climate = Some((30.9, 47., now - 5));
        c.capture(&state, a.clone());
        let s = &c.snapshots[0];
        assert_eq!(s.environment.as_ref().unwrap().device, "EPaper_EC240E");
        assert_eq!(s.environment.as_ref().unwrap().temperature, Some(30.9));
        assert!(s.displays[0].history.is_some());
        let name = format!("{}-{}.json", s.day, a.id);
        exchange(&temp, &mut c, &a).unwrap();
        let written: Snapshot = read_json(&temp.join(CLIMATE_DIR).join(&name), MAX_FILE).unwrap();
        assert_eq!(written.version, DISPLAYS_FORMAT);
        assert_eq!(written.climate().count(), 2);
        // Another Mac lists each display under this Mac; a copy of one it holds live is left out.
        let mut other = State::default();
        other.cloud.origin = Some("2222222222222222".into());
        exchange(&temp, &mut other.cloud_data, &origin("2222222222222222")).unwrap();
        let rows = climate_sources(&other);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.label == "iCloud · Mac-1" && !r.own));
        let e1002 = rows.iter().find(|r| r.device == "EPaper_EC240E").unwrap();
        assert_eq!(e1002.temperature, Some(30.9));
        other.connected = true;
        other.bound_name = "EPaper_B96AD4".into();
        let rows = climate_sources(&other);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].device, "EPaper_EC240E");
        // The card picks the newest; the time tells it apart from the Mac's other display.
        other.connected = false;
        let view = presentation(other);
        assert_eq!(view.environment_source.as_deref(), Some("iCloud · Mac-1"));
        assert_eq!(view.telemetry.unwrap().sampled_at, now - 5);
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn one_display_files_and_the_old_cache_stay_readable_by_build_42() {
        let temp =
            std::env::temp_dir().join(format!("homecraft-build42-{:016x}", rand::random::<u64>()));
        fs::create_dir(&temp).unwrap();
        let now = usage::now();
        let a = origin("1111111111111111");
        let mut state = two_displays(now);
        state.fleet.clear();
        let mut c = Catalog::default();
        c.capture(&state, a.clone());
        exchange(&temp, &mut c, &a).unwrap();
        let name = format!("{}-{}.json", c.snapshots[0].day, a.id);
        let bytes = fs::read(temp.join(CLIMATE_DIR).join(&name)).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("displays"));
        let old: Build42Snapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(old.version, FORMAT);
        assert!(old.environment.unwrap().history.is_some());
        // With a background display the file is version 3; the rollback cache keeps the newest.
        c.capture(&two_displays(now), a.clone());
        assert!(save_cache(&temp, &c));
        let old: Build42Catalog = read_json(&temp.join(".cloud-cache.json"), MAX_CACHE).unwrap();
        assert_eq!(
            old.snapshots[0].environment.as_ref().unwrap().device,
            "EPaper_B96AD4"
        );
        assert!(
            read_json::<Build42Catalog>(&temp.join(".cloud-cache-v2.json"), MAX_CACHE).is_err()
        );
        assert_eq!(load(&temp).snapshots[0].climate().count(), 2);
        // A file written by build 42 or an older build: one environment, no displays key.
        let legacy = json!({
            "version": 2,
            "origin": {"id": "2222222222222222", "name": "Trayvons-MacBook-Pro-14"},
            "day": crate::home::model::day(),
            "usages": [],
            "environment": {
                "device": "EPaper_B96AD4",
                "sampled_at": now - 600,
                "temperature": 28.9,
                "humidity": 42.2,
                "history": null
            }
        });
        let legacy: Snapshot = serde_json::from_value(legacy).unwrap();
        assert!(legacy.valid() && legacy.displays.is_empty());
        let mut value = serde_json::to_value(&c.snapshots[0]).unwrap();
        value["extra"] = json!(1);
        assert!(serde_json::from_value::<Snapshot>(value).is_err());
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn displays_merge_per_device_and_duplicates_are_rejected() {
        let now = usage::now();
        let mut a = snap("1111111111111111", vec![]);
        a.environment = Some(display("EPaper_B96AD4", now - 100, 24.));
        let mut b = snap("1111111111111111", vec![]);
        b.environment = Some(display("EPaper_EC240E", now - 50, 30.));
        b.displays = vec![display("EPaper_B96AD4", now - 200, 22.)];
        a.merge(&b);
        let devices: Vec<_> = a
            .climate()
            .map(|e| (e.device.as_str(), e.temperature))
            .collect();
        assert_eq!(
            devices,
            [("EPaper_EC240E", Some(30.)), ("EPaper_B96AD4", Some(24.))]
        );
        assert!(a.valid());
        a.displays.push(display("EPaper_EC240E", now - 40, 31.));
        assert!(!a.valid());
    }
    #[test]
    fn cloud_fallback_is_labeled_and_not_captured_as_local_data() {
        let mut state = State::default();
        state.cloud_data.merge(snap(
            "2222222222222222",
            vec![summary(
                "2222222222222222",
                "alice@example.test",
                usage::now(),
                20,
            )],
        ));
        let view = presentation(state.clone());
        assert!(state.readings.is_empty());
        assert_eq!(view.readings.len(), 1);
        assert_eq!(view.cloud_sources.get(&0).unwrap(), "iCloud · Mac-1");
        let mut local = Catalog::default();
        local.capture(&view, origin("1111111111111111"));
        assert!(local.snapshots.is_empty());
        let mut local_reading = usage::empty(0, usage::OK, "");
        local_reading.packet.t = usage::now() + 1;
        state.readings.push(local_reading);
        assert!(presentation(state).cloud_sources.is_empty());
    }
}
