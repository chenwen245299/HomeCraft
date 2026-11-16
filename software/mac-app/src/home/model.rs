//! Presentation models and persisted preferences, independent of Bluetooth side effects.
use crate::{backend::State, usage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Home,
    Automation,
    Discover,
    Climate,
    Displays,
    Usage,
    Room,
    Todo,
}
impl Nav {
    pub fn id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Automation => "automation",
            Self::Discover => "discover",
            Self::Climate => "climate",
            Self::Displays => "displays",
            Self::Usage => "usage",
            Self::Room => "room",
            Self::Todo => "todo",
        }
    }
    pub fn parse(v: &str) -> Option<Self> {
        Some(match v {
            "home" => Self::Home,
            "automation" => Self::Automation,
            "discover" => Self::Discover,
            "climate" => Self::Climate,
            "displays" => Self::Displays,
            "usage" => Self::Usage,
            "room" => Self::Room,
            "todo" => Self::Todo,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sheet {
    None,
    Device,
    Add,
    Settings,
    Help,
    Codex,
    Environment,
    Cloud,
    Refresh,
    Weather,
    Todo,
    Update,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Overview,
    Usage,
    Connection,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub home: String,
    pub room: String,
    pub auto_connect: bool,
    pub dusk: bool,
    pub codex_cli: String,
    pub codex_node: String,
    pub refresh: crate::refresh::Settings,
    pub weather: crate::weather::Settings,
    pub poem: crate::poem::Settings,
    pub updates: crate::update::Settings,
    /// Display id -> location shown on the Climate page (e.g. 书房, 卧室).
    pub locations: std::collections::BTreeMap<String, String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            home: "My Home".into(),
            room: "书房".into(),
            auto_connect: true,
            dusk: false,
            codex_cli: String::new(),
            codex_node: String::new(),
            refresh: crate::refresh::Settings::default(),
            weather: crate::weather::Settings::default(),
            poem: crate::poem::Settings::default(),
            updates: crate::update::Settings::default(),
            locations: Default::default(),
        }
    }
}
impl Preferences {
    pub fn save(&self, root: &std::path::Path) -> anyhow::Result<()> {
        use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
        anyhow::ensure!(self.validate(), "家庭名称或房间名称无效");
        let temporary = root.join(format!(".home-ui-{:016x}.tmp", rand::random::<u64>()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&serde_json::to_vec(self)?)?;
            file.sync_all()?;
            std::fs::rename(&temporary, root.join(".home-ui.json"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        result
    }

    pub fn validate(&self) -> bool {
        [&self.home, &self.room].iter().all(|s| {
            !s.trim().is_empty() && s.chars().count() <= 32 && !s.chars().any(char::is_control)
        }) && crate::codex_cli::valid_setting(&self.codex_cli)
            && crate::codex_cli::valid_setting(&self.codex_node)
            && self.refresh.valid()
            && self.weather.valid()
            && self.locations.len() <= 8
            && self.locations.iter().all(|(id, name)| {
                id.len() <= 64 && name.chars().count() <= 16 && !name.chars().any(char::is_control)
            })
    }
}
pub fn connection(state: &State) -> &'static str {
    if state.connected {
        "已连接"
    } else if state.connecting {
        "正在连接"
    } else if state.scanning {
        "正在查找"
    } else {
        "未连接"
    }
}
pub fn today_total(state: &State, day: i64) -> Option<u64> {
    crate::hub::readings(state)
        .into_iter()
        .filter(|r| r.packet.s == usage::OK && r.packet.p != 0)
        .filter_map(|r| r.packet.d)
        .filter(|d| d[0] == day && d[1] >= 0)
        .map(|d| d[1] as u64)
        .chain(crate::cloud::codex_total(state, day))
        .try_fold(None, |sum: Option<u64>, n| {
            sum.unwrap_or(0).checked_add(n).map(Some)
        })
        .flatten()
}
pub fn week_start(day: i64) -> Option<i64> {
    use chrono::Datelike;
    let date = chrono::NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d").ok()?;
    let monday = date - chrono::Duration::days(i64::from(date.weekday().num_days_from_monday()));
    Some(
        i64::from(monday.year()) * 10000
            + i64::from(monday.month()) * 100
            + i64::from(monday.day()),
    )
}
pub fn week_total(state: &State, day: i64) -> Option<u64> {
    let week = week_start(day)?;
    crate::hub::readings(state)
        .into_iter()
        .filter(|r| r.packet.s == usage::OK && r.packet.p != 0)
        .filter_map(|r| r.packet.wt)
        .filter(|w| w[0] == week && w[1] >= 0)
        .map(|w| w[1] as u64)
        .chain(crate::cloud::codex_week_total(state, week))
        .try_fold(None, |sum: Option<u64>, n| {
            sum.unwrap_or(0).checked_add(n).map(Some)
        })
        .flatten()
}
pub fn compact(n: Option<u64>) -> String {
    match n {
        None => "—".into(),
        Some(n) if n >= 1_000_000_000_000_000 => format!("{:.2}P", n as f64 / 1e15),
        Some(n) if n >= 1_000_000_000_000 => format!("{:.2}T", n as f64 / 1e12),
        Some(n) if n >= 1_000_000_000 => format!("{:.2}B", n as f64 / 1e9),
        Some(n) if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        Some(n) if n >= 1_000 => format!("{:.1}K", n as f64 / 1e3),
        Some(n) => n.to_string(),
    }
}
pub fn month_start(day: i64) -> Option<i64> {
    chrono::NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d")
        .ok()
        .map(|_| day - day % 100 + 1)
}
pub fn month_total(state: &State, day: i64) -> Option<u64> {
    let month = month_start(day)?;
    crate::hub::readings(state)
        .into_iter()
        .filter(|r| r.packet.s == usage::OK && r.packet.p != 0)
        .filter_map(|r| r.packet.mt)
        .filter(|m| m[0] == month && m[1] >= 0)
        .map(|m| m[1] as u64)
        .chain(crate::cloud::codex_month_total(state, month))
        .try_fold(None, |sum: Option<u64>, n| {
            sum.unwrap_or(0).checked_add(n).map(Some)
        })
        .flatten()
}
/// The home page's three totals for displays advertising totals:1. A display adding its own
/// packets would miss every Codex account but the one it receives; -1 is unknown, never 0.
pub fn totals_wire(state: &State, day: i64) -> Option<Vec<u8>> {
    // All three consumers resolve through hub::readings; raw collection stays untouched.
    let tokens = |n: Option<u64>| {
        n.filter(|n| *n <= 9_007_199_254_740_991)
            .map_or(-1, |n| n as i64)
    };
    let message = serde_json::json!({
        "v": 1,
        "op": "totals",
        "d": [day, tokens(today_total(state, day))],
        "wt": [week_start(day)?, tokens(week_total(state, day))],
        "mt": [month_start(day)?, tokens(month_total(state, day))],
    });
    serde_json::to_vec(&message).ok()
}
/// Where each display stands, as saved on the Climate page; displays advertising places:1 show them.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Places {
    pub room: String,
    pub locations: std::collections::BTreeMap<String, String>,
}
impl Places {
    pub fn from(prefs: &Preferences) -> Self {
        Self {
            room: prefs.room.clone(),
            locations: prefs.locations.clone(),
        }
    }
}
/// A display's place: the user's name for it, the room for the primary 3.7-inch display, else its own name.
pub fn place_name(
    locations: &std::collections::BTreeMap<String, String>,
    room: &str,
    state: &State,
    id: &str,
) -> String {
    if let Some(name) = locations.get(id).filter(|n| !n.trim().is_empty()) {
        return name.clone();
    }
    if let Some(row) = state.fleet.get(id) {
        return if row.model == "3Inch7EPaper" && row.name == state.bound_name {
            room.into()
        } else {
            row.name.clone()
        };
    }
    // A paired display that has not connected this run: the primary one still goes by the room.
    state.devices.iter().find(|d| d.id == id).map_or_else(
        || id.chars().take(8).collect(),
        |d| {
            if d.name == state.bound_name {
                room.into()
            } else {
                d.name.clone()
            }
        },
    )
}
/// The displays the Climate page lists: every one connected this run and every paired one, in a stable order.
pub fn place_ids(state: &State) -> Vec<String> {
    state
        .fleet
        .keys()
        .cloned()
        .chain(
            state
                .devices
                .iter()
                .filter(|d| d.bound)
                .map(|d| d.id.clone()),
        )
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
/// Up to two places for the display `target` (places:1), in the Climate page's order: the ones the user
/// named, else every listed display. The target is `here` and shows its own sensor (named 室内 until the user names
/// it); another carries the latest reading HomeCraft has from it, or only its name when there is none.
pub fn places_wire(state: &State, target: &str) -> Option<Vec<u8>> {
    let places = &state.places;
    let named = |id: &str| {
        places
            .locations
            .get(id)
            .is_some_and(|n| !n.trim().is_empty())
    };
    let listed = place_ids(state);
    let ids: std::collections::BTreeSet<&str> = listed
        .iter()
        .map(String::as_str)
        .chain(
            places
                .locations
                .keys()
                .map(String::as_str)
                .filter(|id| named(id)),
        )
        .collect();
    let any = ids.iter().any(|id| named(id));
    let now = usage::now();
    let rows: Vec<_> = ids
        .into_iter()
        .filter(|id| !any || named(id))
        .take(2)
        .map(|id| {
            let name = if id == target && !named(id) {
                "室内".into()
            } else {
                place_name(&places.locations, &places.room, state, id)
            };
            // The display limit is 48 bytes, cut on a character boundary; never empty.
            let mut end = 0;
            for (i, c) in name.char_indices() {
                if i + c.len_utf8() > 48 {
                    break;
                }
                end = i + c.len_utf8();
            }
            let name = match name[..end].trim() {
                "" => "位置",
                cut => cut,
            };
            if id == target {
                return serde_json::json!({"name": name, "here": true});
            }
            let logged = state
                .env_log
                .get(id)
                .and_then(|p| p.last())
                .map(|&(at, t, h)| (t, h, at));
            match state
                .fleet
                .get(id)
                .and_then(|r| r.climate)
                .into_iter()
                .chain(logged)
                .filter(|(t, h, at)| {
                    (-40.0..=125.0).contains(t)
                        && (0.0..=100.0).contains(h)
                        && (1_735_689_600..=now + 300).contains(at)
                })
                .max_by_key(|c| c.2)
            {
                Some((t, h, at)) => serde_json::json!({
                    "name": name,
                    "t": (t * 10.).round() as i64,
                    "h": (h * 10.).round() as i64,
                    "at": at,
                }),
                None => serde_json::json!({"name": name}),
            }
        })
        .collect();
    serde_json::to_vec(&serde_json::json!({"v": 1, "op": "places", "rows": rows})).ok()
}
pub fn day() -> i64 {
    use chrono::Datelike;
    let d = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    i64::from(d.year()) * 10000 + i64::from(d.month()) * 100 + i64::from(d.day())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn total_excludes_history_and_weekly() {
        let mut s = State::default();
        let mut r = usage::empty(0, usage::OK, "");
        r.packet.d = Some([20261021, 120]);
        r.packet.wt = Some([20261019, 900]);
        s.readings.push(r.clone());
        r.packet.p = 3;
        r.packet.d = Some([20261021, 0]);
        s.readings.push(r);
        assert_eq!(today_total(&s, 20261021), Some(120));
        assert_eq!(today_total(&s, 20261022), None);
    }
    #[test]
    fn week_total_keeps_unknown_zero_and_week_boundaries_distinct() {
        let mut s = State::default();
        assert_eq!(week_total(&s, 20261024), None);
        let mut r = usage::empty(3, usage::OK, "");
        r.packet.d = Some([20261024, 100]);
        r.tokens.lifetime = Some(99999);
        r.packet.wt = Some([20261019, 0]);
        s.readings.push(r.clone());
        assert_eq!(week_total(&s, 20261024), Some(0));
        s.readings[0].packet.wt = Some([20261019, 120]);
        r.packet.p = 0;
        r.packet.wt = Some([20261019, 30]);
        s.readings.push(r);
        assert_eq!(week_total(&s, 20261024), Some(150));
        assert_eq!(week_total(&s, 20261026), None);
        assert_eq!(week_start(20260101), Some(20251229));
        assert_eq!(week_total(&s, 20260230), None);
    }
    #[test]
    fn month_total_keeps_unknown_zero_and_month_boundaries_distinct() {
        let mut s = State::default();
        assert_eq!(month_total(&s, 20261024), None);
        let mut r = usage::empty(3, usage::OK, "");
        r.packet.mt = Some([20261001, 0]);
        s.readings.push(r.clone());
        assert_eq!(month_total(&s, 20261024), Some(0));
        r.packet.p = 0;
        r.packet.mt = Some([20261001, 30]);
        s.readings.push(r);
        assert_eq!(month_total(&s, 20261031), Some(30));
        assert_eq!(month_total(&s, 20261101), None);
        assert_eq!(month_total(&s, 20260230), None);
    }
    #[test]
    fn totals_wire_matches_the_home_totals_and_keeps_unknown() {
        let mut s = State::default();
        let mut r = usage::empty(3, usage::OK, "");
        r.packet.d = Some([20261027, 17]);
        r.packet.mt = Some([20261001, 278]);
        s.readings.push(r);
        let v: serde_json::Value =
            serde_json::from_slice(&totals_wire(&s, 20261027).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"v":1,"op":"totals","d":[20261027,17],"wt":[20261026,-1],"mt":[20261001,278]})
        );
        assert!(totals_wire(&s, 20260230).is_none());
    }
    #[test]
    fn totals_wire_uses_the_same_cloud_fallback_as_the_visible_home() {
        let day = day();
        let week = week_start(day).unwrap();
        let month = month_start(day).unwrap();
        let mut state = State::default();
        state.cloud.origin = Some("2222222222222222".into());
        state.readings.push(usage::empty(3, 5, "网络读取失败"));
        let mut packet = usage::empty(3, usage::OK, "").packet;
        packet.d = Some([day, 17]);
        packet.wt = Some([week, 23]);
        packet.mt = Some([month, 278]);
        state.cloud_data.snapshots.push(crate::cloud::Snapshot {
            version: 2,
            origin: crate::harness::Origin {
                id: "1111111111111111".into(),
                name: "Other Mac".into(),
            },
            day,
            usages: vec![crate::cloud::Summary {
                packet,
                account: None,
                lifetime: None,
                peak_daily: None,
                context: None,
                breakdown: None,
            }],
            environment: None,
            displays: vec![],
        });
        let home = crate::cloud::presentation(state.clone());
        let wire: serde_json::Value =
            serde_json::from_slice(&totals_wire(&state, day).unwrap()).unwrap();
        assert_eq!(wire["d"][1], today_total(&home, day).unwrap());
        assert_eq!(wire["wt"][1], week_total(&home, day).unwrap());
        assert_eq!(wire["mt"][1], month_total(&home, day).unwrap());
        assert_eq!(wire["mt"][1], 278);
        assert_eq!(state.readings[0].packet.s, 5);
        assert_eq!(state.readings[0].packet.mt, None);
        state.readings[0].packet.s = usage::OK;
        state.readings[0].packet.mt = Some([month, 0]);
        let current: serde_json::Value =
            serde_json::from_slice(&totals_wire(&state, day).unwrap()).unwrap();
        // Argus is the same shared library: a newer valid zero replaces its older observation.
        assert_eq!(current["mt"][1], 0);
    }
    #[test]
    fn preferences_roundtrip_is_private_and_contains_no_pairing_data() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("epaper-prefs-{:016x}", rand::random::<u64>()));
        std::fs::create_dir_all(&root).unwrap();
        let p = Preferences {
            home: "My Home 测试".into(),
            refresh: crate::refresh::Settings {
                enabled: false,
                seconds: 1020,
            },
            ..Preferences::default()
        };
        p.save(&root).unwrap();
        let path = root.join(".home-ui.json");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let saved: Preferences = crate::config::read_private(&path).unwrap();
        assert_eq!(saved.home, p.home);
        assert_eq!(saved.refresh, p.refresh);
        assert!(!std::fs::read_to_string(path).unwrap().contains("password"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn names_are_bounded_and_routes_are_explicit() {
        assert!(Preferences::default().validate());
        let p = Preferences {
            home: "\n".into(),
            ..Preferences::default()
        };
        assert!(!p.validate());
        assert_eq!(Nav::parse("room"), Some(Nav::Room));
        assert_eq!(Nav::parse("shell"), None);
    }
    #[test]
    fn places_name_the_receiving_display_here_and_carry_the_others_reading() {
        let now = usage::now();
        let (e1002, small) = ("95c193b8", "a9b7115a");
        let mut s = State {
            bound_name: "EPaper_B96AD4".into(),
            places: Places {
                room: "书房".into(),
                locations: Default::default(),
            },
            ..Default::default()
        };
        for (id, name, model, climate) in [
            (
                e1002,
                "EPaper_EC240E",
                "reTerminalE1002",
                Some((30.4, 48.4, now - 60)),
            ),
            (
                small,
                "EPaper_B96AD4",
                "3Inch7EPaper",
                Some((23.46, 48.04, now - 30)),
            ),
        ] {
            s.fleet.insert(
                id.into(),
                crate::backend::FleetStatus {
                    id: id.into(),
                    name: name.into(),
                    model: model.into(),
                    climate,
                    ..Default::default()
                },
            );
        }
        let rows = |s: &State| -> serde_json::Value {
            let bytes = places_wire(s, e1002).unwrap();
            assert!(bytes.len() <= 480);
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["rows"].clone()
        };
        // Nothing named yet: every display, this one as 室内, the primary 3.7-inch one as its room.
        assert_eq!(
            rows(&s),
            serde_json::json!([
                {"name": "室内", "here": true},
                {"name": "书房", "t": 235, "h": 480, "at": now - 30},
            ])
        );
        // Named places only; one that has not connected this run still shows its logged reading.
        let other = "c0ffee00";
        s.places.locations.insert(e1002.into(), "客厅".into());
        s.places.locations.insert(other.into(), "卧室".into());
        s.env_log
            .insert(other.into(), vec![(now - 900, 22.0, 55.0)]);
        assert_eq!(
            rows(&s),
            serde_json::json!([
                {"name": "客厅", "here": true},
                {"name": "卧室", "t": 220, "h": 550, "at": now - 900},
            ])
        );
        // No reading at all: the name alone. A long room name is cut to 48 bytes.
        s.env_log.clear();
        s.places.locations.clear();
        s.places.room = "一二三四五六七八九十一二三四五六七八九十".into();
        s.fleet.get_mut(small).unwrap().climate = None;
        let r = rows(&s);
        assert_eq!(r[1]["name"], "一二三四五六七八九十一二三四五六");
        assert!(r[1].get("t").is_none());
        // The paired primary display, not connected this run: listed by its room, reading unknown.
        s.places.room = "书房".into();
        s.fleet.remove(small);
        s.devices.push(crate::backend::Device {
            id: small.into(),
            name: "EPaper_B96AD4".into(),
            rssi: None,
            bound: true,
            enrollment: false,
            recovered: true,
        });
        assert_eq!(
            rows(&s),
            serde_json::json!([{"name": "室内", "here": true}, {"name": "书房"}])
        );
    }
}
