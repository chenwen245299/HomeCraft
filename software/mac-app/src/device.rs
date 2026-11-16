//! Authenticated board telemetry. No account credentials or SD file access.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::device_pages::PAGE_MASK_LIMIT;
pub use crate::device_pages::PAGES;
pub type DisplayedFrame = (Option<f32>, Option<f32>, i64, u8, u8, bool, u32);
pub const TELEMETRY_SECONDS: u64 = 300;
pub fn telemetry_due(
    last: std::time::Instant,
    now: std::time::Instant,
    awaiting_action: bool,
) -> bool {
    now.saturating_duration_since(last)
        >= std::time::Duration::from_secs(if awaiting_action {
            1
        } else {
            TELEMETRY_SECONDS
        })
}
#[derive(Clone, Debug)]
pub enum Action {
    Page(String),
    Rotate(Option<u8>),
    Refresh,
    Read,
    Status,
}
impl Action {
    pub fn label(&self) -> String {
        match self {
            Self::Read => "采样并更新".into(),
            Self::Status => "读取设备状态".into(),
            Self::Refresh => "整屏清除残影".into(),
            Self::Page(page) => format!(
                "切换到{}",
                PAGES
                    .iter()
                    .find(|(p, _)| *p == page)
                    .map(|(_, n)| *n)
                    .unwrap_or("页面")
            ),
            Self::Rotate(None) => "恢复自动方向".into(),
            Self::Rotate(Some(_)) => "调整屏幕方向".into(),
        }
    }
    pub fn completed(&self, t: &Telemetry, baseline: u32) -> bool {
        let Some(frame) = &t.shown else {
            return false;
        };
        if t.busy || frame.6 == 0 {
            return false;
        }
        match self {
            Self::Page(page) => PAGES.get(frame.3 as usize).is_some_and(|(p, _)| *p == page),
            Self::Rotate(Some(rotation)) => !t.auto_rotate && frame.4 == *rotation,
            Self::Rotate(None) => t.auto_rotate && frame.4 == t.rotation,
            Self::Refresh => frame.6 > baseline,
            Self::Read => t
                .sample
                .is_some_and(|(id, result)| id == baseline && (result == 1 || result == 2)),
            Self::Status => true,
        }
    }
    pub fn request(&self) -> Result<Value> {
        Ok(match self {
            Self::Page(page) => {
                ensure!(PAGES.iter().any(|(p, _)| *p == page), "无效页面");
                json!({"v":1,"op":"page","page":page})
            }
            Self::Rotate(Some(rotation)) => {
                ensure!(*rotation < 4, "无效方向");
                json!({"v":1,"op":"rotate","rotation":rotation})
            }
            Self::Rotate(None) => json!({"v":1,"op":"rotate","rotation":"auto"}),
            Self::Refresh => json!({"v":1,"op":"refresh"}),
            Self::Read => json!({"v":1,"op":"sample"}),
            Self::Status => json!({"v":1,"op":"status"}),
        })
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Telemetry {
    #[serde(rename = "t")]
    pub temperature: Option<f32>,
    #[serde(rename = "h")]
    pub humidity: Option<f32>,
    #[serde(rename = "at")]
    pub sampled_at: i64,
    #[serde(rename = "age")]
    pub age_ms: u64,
    pub page: String,
    pub pages: u16,
    pub rotation: u8,
    #[serde(rename = "auto")]
    pub auto_rotate: bool,
    pub paused: bool,
    pub busy: bool,
    pub partial: bool,
    // USB, battery present, charging, percentage (-1 means unavailable).
    pub power: (u8, u8, u8, i16),
    // mounted, UTC+8 day, rows, write OK, latest saved timestamp, last error.
    pub sd: (u8, u32, u32, u8, i64, i32),
    pub history: u32,
    #[serde(default)]
    pub shown: Option<DisplayedFrame>,
    #[serde(default)]
    pub sample: Option<(u32, u8)>,
    #[serde(skip)]
    pub received_at: i64,
}
impl Telemetry {
    pub fn parse(value: &Value, now: i64) -> Result<Self> {
        ensure!(value["ok"] == true && value["v"] == 1, "设备状态版本不支持");
        let mut t: Self = serde_json::from_value(value["device"].clone())?;
        ensure!(
            t.rotation < 4 && t.pages < PAGE_MASK_LIMIT && PAGES.iter().any(|(p, _)| *p == t.page),
            "无效屏幕状态"
        );
        ensure!(
            t.temperature.is_none() == t.humidity.is_none(),
            "传感器字段不完整"
        );
        if let (Some(temp), Some(hum)) = (t.temperature, t.humidity) {
            ensure!(
                temp.is_finite()
                    && hum.is_finite()
                    && (-45.0..=130.0).contains(&temp)
                    && (0.0..=100.0).contains(&hum),
                "传感器数值超出范围"
            );
        }
        ensure!(
            t.power.0 <= 1
                && t.power.1 <= 1
                && t.power.2 <= 1
                && (-1..=100).contains(&t.power.3)
                && t.sd.0 <= 1
                && t.sd.3 <= 1,
            "无效电源或 SD 状态"
        );
        ensure!(
            t.sample.is_none_or(|(id, result)| id > 0 && result <= 4),
            "无效采样结果"
        );
        ensure!(valid_day(t.sd.1), "无效历史日期");
        t.received_at = now;
        Ok(t)
    }
    pub fn fresh(&self, connected: bool, now: i64) -> bool {
        connected
            && self.temperature.is_some()
            && self.humidity.is_some()
            && now.saturating_sub(self.received_at) <= TELEMETRY_SECONDS as i64 + 30
            && self.age_ms <= 45000
    }
    pub fn page_label(&self) -> &str {
        PAGES
            .iter()
            .find(|(p, _)| *p == self.page)
            .map(|(_, label)| *label)
            .unwrap_or("未知页面")
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryChunk {
    pub day: u32,
    pub revision: u32,
    pub offset: usize,
    pub points: Vec<Option<(i16, u16)>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    pub day: u32,
    pub revision: u32,
    pub points: Vec<Option<(f32, f32)>>,
    pub received_at: i64,
}
fn valid_day(day: u32) -> bool {
    day == 0
        || ((20200101..=21001231).contains(&day)
            && chrono::NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d").is_ok())
}
impl History {
    pub fn assemble(chunks: Vec<HistoryChunk>, now: i64) -> Result<Self> {
        ensure!(chunks.len() == 12, "历史分块不完整");
        let day = chunks[0].day;
        let revision = chunks[0].revision;
        ensure!(valid_day(day), "历史日期无效");
        let mut points = Vec::with_capacity(96);
        for (index, chunk) in chunks.into_iter().enumerate() {
            ensure!(
                chunk.day == day && chunk.revision == revision,
                "采样期间历史发生更新，稍后重读"
            );
            ensure!(
                chunk.offset == index * 8 && chunk.points.len() == 8,
                "历史位置不连续"
            );
            for point in chunk.points {
                if let Some((t, h)) = point {
                    ensure!(
                        day != 0 && (-4500..=13000).contains(&t) && h <= 10000,
                        "无效历史采样"
                    );
                }
                points.push(point.map(|(t, h)| (f32::from(t) / 100., f32::from(h) / 100.)));
            }
        }
        Ok(Self {
            day,
            revision,
            points,
            received_at: now,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn chunks() -> Vec<HistoryChunk> {
        (0..12)
            .map(|i| HistoryChunk {
                day: 20261021,
                revision: 7,
                offset: i * 8,
                points: vec![Some((2345, 5678)); 8],
            })
            .collect()
    }
    #[test]
    fn history_preserves_gaps_and_refuses_mixed_days_or_revisions() {
        let mut c = chunks();
        c[0].points[0] = None;
        let h = History::assemble(c, 123).unwrap();
        assert_eq!(h.points.len(), 96);
        assert_eq!(h.points[0], None);
        assert_eq!(h.points[1], Some((23.45, 56.78)));
        let mut c = chunks();
        c[11].revision += 1;
        assert!(History::assemble(c, 123).is_err());
        let mut c = chunks();
        c[11].day += 1;
        assert!(History::assemble(c, 123).is_err());
        let mut c = chunks();
        c[2].offset = 0;
        assert!(History::assemble(c, 123).is_err());
        let mut c = chunks();
        c[2].points[0] = Some((32000, 60000));
        assert!(History::assemble(c, 123).is_err());
        assert!(!valid_day(20260230));
    }
    #[test]
    fn telemetry_missing_and_stale_are_not_zero() {
        let mut v = json!({"ok":true,"v":1,"device":{"t":null,"h":null,"at":0,"age":5,"page":"environment","pages":127,"rotation":0,"auto":true,"paused":false,"busy":false,"partial":true,"power":[1,0,0,-1],"sd":[0,0,0,0,0,0],"history":0}});
        let t = Telemetry::parse(&v, 100).unwrap();
        assert_eq!(t.temperature, None);
        assert!(!t.fresh(true, 110));
        assert!(!t.fresh(false, 101));
        assert!(!t.fresh(true, 121));
        v["device"]["t"] = json!(31.2);
        assert!(Telemetry::parse(&v, 100).is_err());
        v["device"]["h"] = json!(43.);
        assert!(Telemetry::parse(&v, 100).is_ok());
        v["device"]["h"] = json!(101.);
        assert!(Telemetry::parse(&v, 100).is_err());
    }
    #[test]
    fn passive_reads_do_not_follow_token_updates_and_manual_reads_bypass_timer() {
        let start = std::time::Instant::now();
        for seconds in (30..300).step_by(30) {
            let now = start + std::time::Duration::from_secs(seconds);
            assert!(!telemetry_due(start, now, false));
            assert!(telemetry_due(start, now, true));
        }
        assert!(telemetry_due(
            start,
            start + std::time::Duration::from_secs(300),
            false
        ));
        let value = json!({"ok":true,"v":1,"device":{"t":31.2,"h":45.1,"at":100,"age":29000,"page":"environment","pages":127,"rotation":0,"auto":true,"paused":false,"busy":false,"partial":true,"power":[1,0,0,-1],"sd":[0,0,0,0,0,0],"history":0}});
        let t = Telemetry::parse(&value, 100).unwrap();
        assert!(t.fresh(true, 399));
        assert!(!t.fresh(true, 431));
    }
    #[test]
    fn controls_have_explicit_allowlist() {
        assert!(Action::Page("password".into()).request().is_err());
        assert!(Action::Rotate(Some(4)).request().is_err());
        assert_eq!(Action::Rotate(None).request().unwrap()["rotation"], "auto");
    }
    #[test]
    fn a_received_page_command_is_not_a_completed_screen_refresh() {
        let value = json!({"ok":true,"v":1,"device":{"t":32.3,"h":54.0,"at":100,"age":0,"page":"environment","pages":243,"rotation":0,"auto":true,"paused":false,"busy":false,"partial":true,"power":[1,0,0,-1],"sd":[1,20261021,10,1,100,0],"history":1,"shown":[32.1,54.3,95,0,0,true,3]}});
        let mut t = Telemetry::parse(&value, 100).unwrap();
        let page = Action::Page("environment".into());
        assert!(!page.completed(&t, 3));
        t.shown.as_mut().unwrap().3 = 4;
        t.shown.as_mut().unwrap().6 = 4;
        t.busy = true;
        assert!(!page.completed(&t, 3));
        t.busy = false;
        assert!(page.completed(&t, 3));
        assert!(!Action::Refresh.completed(&t, 4));
        assert!(Action::Refresh.completed(&t, 3));
    }
    #[test]
    fn manual_sample_requires_its_own_device_completion() {
        let value = json!({"ok":true,"v":1,"device":{"t":32.3,"h":54.,"at":100,"age":0,"page":"overview","pages":243,"rotation":0,"auto":true,"paused":false,"busy":false,"partial":true,"power":[1,0,0,-1],"sd":[1,20261021,10,1,100,0],"history":1,"shown":[32.3,54.,100,0,0,true,12]}});
        let mut t = Telemetry::parse(&value, 100).unwrap();
        assert_eq!(Action::Read.request().unwrap()["op"], "sample");
        assert_eq!(Action::Status.request().unwrap()["op"], "status");
        assert!(!Action::Read.completed(&t, 7));
        for result in [0, 3, 4] {
            t.sample = Some((7, result));
            assert!(!Action::Read.completed(&t, 7));
        }
        t.sample = Some((6, 1));
        assert!(!Action::Read.completed(&t, 7));
        for result in [1, 2] {
            t.sample = Some((7, result));
            assert!(Action::Read.completed(&t, 7));
            t.busy = true;
            assert!(!Action::Read.completed(&t, 7));
            t.busy = false;
        }
    }
}
#[test]
fn music_page_is_additive_and_unknown_mask_bits_are_rejected() {
    let mut value = json!({"ok":true,"v":1,"device":{"t":null,"h":null,"at":0,"age":0,"page":"music","pages":4096,"rotation":0,"auto":true,"paused":false,"busy":false,"partial":true,"power":[1,1,0,100],"sd":[1,20261025,1,1,100,0],"history":0,"shown":[null,null,100,12,0,true,1]}});
    let t = Telemetry::parse(&value, 100).unwrap();
    assert_eq!(t.page_label(), "音乐");
    assert!(Action::Page("music".into()).completed(&t, 0));
    assert_eq!(
        Action::Page("music".into()).request().unwrap()["page"],
        "music"
    );
    value["device"]["page"] = json!("usage");
    value["device"]["pages"] = json!(8193);
    assert!(Telemetry::parse(&value, 100).is_ok());
    value["device"]["pages"] = json!(PAGE_MASK_LIMIT);
    assert!(Telemetry::parse(&value, 100).is_err());
}
