//! Public weather summaries for up to two user-chosen cities (default Singapore), shared by every display.
//! No IP/GPS lookup; no coordinates or account tokens are sent to hardware.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
pub const DEFAULT_CITY: &str = "新加坡";
/// Older than this, a summary is withheld from displays and cleared from them.
pub const MAX_AGE: i64 = 21600;
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub city: String,
    /// Optional second city; empty means one city.
    pub second_city: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            city: DEFAULT_CITY.into(),
            second_city: String::new(),
        }
    }
}
impl Settings {
    pub fn valid(&self) -> bool {
        [&self.city, &self.second_city]
            .iter()
            .all(|c| c.chars().count() <= 32 && !c.chars().any(char::is_control))
    }
    /// Cities in display order: the first (or the default) and an optional distinct second one.
    pub fn cities(&self) -> Vec<String> {
        let mut out = vec![self.city().to_owned()];
        let second = self.second_city.trim();
        if !second.is_empty() && second != out[0] {
            out.push(second.to_owned());
        }
        out
    }
    /// A cleared field (or an older automatic-city preference) falls back to the default city.
    pub fn city(&self) -> &str {
        match self.city.trim() {
            "" => DEFAULT_CITY,
            city => city,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub city: String,
    /// Country/province of the resolved place, shown only in HomeCraft so a wrong match is visible.
    #[serde(default)]
    pub region: String,
    pub fetched_at: i64,
    pub temperature_tenths: i16,
    pub low_tenths: i16,
    pub high_tenths: i16,
    pub code: u8,
    /// Tomorrow's WMO code, low and high; absent in summaries cached by older versions.
    #[serde(default)]
    pub tomorrow: Option<(u8, i16, i16)>,
}
impl Snapshot {
    pub fn valid(&self) -> bool {
        !self.city.is_empty()
            && self.city.len() <= 48
            && !self.city.chars().any(char::is_control)
            && self.region.len() <= 96
            && !self.region.chars().any(char::is_control)
            && (-900..=700).contains(&self.temperature_tenths)
            && (-900..=700).contains(&self.low_tenths)
            && (-900..=700).contains(&self.high_tenths)
            && self.low_tenths <= self.high_tenths
            && valid_code(self.code)
            && self.fetched_at >= 1735689600
            && self.tomorrow.is_none_or(|(code, low, high)| {
                valid_code(code)
                    && (-900..=700).contains(&low)
                    && (-900..=700).contains(&high)
                    && low <= high
            })
    }
    pub fn fresh(&self, now: i64) -> bool {
        self.valid() && self.fetched_at <= now + 60 && now - self.fetched_at <= MAX_AGE
    }
    /// Slot 0 keeps the original message; slot 1 is only sent to displays advertising weather:2,
    /// tomorrow only to displays advertising forecast:1.
    pub fn wire(&self, now: i64, slot: usize, forecast: bool) -> Result<Vec<u8>> {
        ensure!(self.fresh(now) && slot < 2, "天气数据已过期");
        let mut v = json!({"v":1,"op":"weather","city":self.city,"values":[self.fetched_at,i64::from(self.temperature_tenths),i64::from(self.low_tenths),i64::from(self.high_tenths),i64::from(self.code)]});
        if slot > 0 {
            v["slot"] = json!(slot);
        }
        if let Some((code, low, high)) = self.tomorrow.filter(|_| forecast) {
            v["tomorrow"] = json!([code, low, high]);
        }
        let bytes = serde_json::to_vec(&v)?;
        ensure!(bytes.len() <= 480, "天气摘要过长");
        Ok(bytes)
    }
    pub fn description(&self) -> &'static str {
        describe(self.code)
    }
}
pub fn describe(code: u8) -> &'static str {
    match code {
        0 => "晴",
        1..=3 => "多云",
        45 | 48 => "雾",
        51..=67 => "雨",
        71..=77 => "雪",
        80..=82 => "阵雨",
        85 | 86 => "阵雪",
        95..=99 => "雷雨",
        _ => "未知",
    }
}
fn valid_code(n: u8) -> bool {
    matches!(
        n,
        0..=3
            | 45
            | 48
            | 51
            | 53
            | 55
            | 56
            | 57
            | 61
            | 63
            | 65
            | 66
            | 67
            | 71
            | 73
            | 75
            | 77
            | 80
            | 81
            | 82
            | 85
            | 86
            | 95
            | 96
            | 99
    )
}
#[derive(Serialize, Deserialize)]
struct Location {
    city: String,
    #[serde(default)]
    region: String,
    latitude: f64,
    longitude: f64,
    fetched_at: i64,
    query: String,
}
impl Location {
    fn valid(&self) -> bool {
        !self.city.is_empty()
            && self.city.len() <= 48
            && !self.city.chars().any(char::is_control)
            && self.region.len() <= 96
            && !self.region.chars().any(char::is_control)
            && self.latitude.is_finite()
            && self.longitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
    }
}
async fn get(client: &reqwest::Client, url: &str, query: &[(&str, String)]) -> Result<Value> {
    let response = client
        .get(url)
        .query(query)
        .send()
        .await?
        .error_for_status()?;
    ensure!(
        response.content_length().is_none_or(|n| n <= 65536),
        "天气服务回复过长"
    );
    let bytes = response.bytes().await?;
    ensure!(bytes.len() <= 65536, "天气服务回复过长");
    Ok(serde_json::from_slice(&bytes)?)
}
fn city_cache(root: &Path, slot: usize) -> PathBuf {
    root.join(if slot == 0 {
        ".weather-city.json"
    } else {
        ".weather-city-2.json"
    })
}
async fn location(
    client: &reqwest::Client,
    root: &Path,
    query: &str,
    slot: usize,
    now: i64,
) -> Result<Location> {
    if let Ok(old) = crate::config::read_private::<Location>(&city_cache(root, slot))
        && old.valid()
        && old.query == query
        && old.fetched_at <= now
        && now - old.fetched_at < 21600
    {
        return Ok(old);
    }
    // Chinese input is shown exactly as typed (the geocoder may answer in traditional script the
    // display font lacks); other input uses the geocoder's English name.
    let chinese = query
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    let v = get(
        client,
        "https://geocoding-api.open-meteo.com/v1/search",
        &[
            ("name", query.into()),
            ("count", "1".into()),
            ("language", if chinese { "zh" } else { "en" }.into()),
            ("format", "json".into()),
        ],
    )
    .await?;
    let row = v["results"]
        .as_array()
        .and_then(|r| r.first())
        .context("未找到这个城市")?;
    let name = row["name"].as_str().context("城市名称无效")?;
    let city = truncate(if chinese { query } else { name }, 48);
    let mut region = String::new();
    for part in [&row["country"], &row["admin1"]] {
        if let Some(part) = part.as_str().filter(|p| *p != city && !region.contains(p)) {
            if !region.is_empty() {
                region.push(' ');
            }
            region.push_str(part);
        }
    }
    let region = truncate(&region, 96);
    let result = Location {
        city,
        region,
        latitude: (row["latitude"].as_f64().context("城市位置无效")? * 100.).round() / 100.,
        longitude: (row["longitude"].as_f64().context("城市位置无效")? * 100.).round() / 100.,
        fetched_at: now,
        query: query.into(),
    };
    ensure!(result.valid(), "城市数据无效");
    crate::config::write_private(&city_cache(root, slot), &result)?;
    Ok(result)
}
fn truncate(s: &str, bytes: usize) -> String {
    s.chars()
        .scan(0, |count, ch| {
            *count += ch.len_utf8();
            (*count <= bytes).then_some(ch)
        })
        .collect()
}
fn parse_forecast(v: &Value, city: &str, region: &str, now: i64) -> Result<Snapshot> {
    let current = &v["current"];
    let daily = &v["daily"];
    let temperature = |v: &Value| -> Result<i16> {
        let n = v.as_f64().context("天气缺少温度")?;
        ensure!(n.is_finite() && (-90.0..=70.0).contains(&n), "天气温度无效");
        Ok((n * 10.).round() as i16)
    };
    let code = current["weather_code"]
        .as_u64()
        .filter(|n| *n <= 99)
        .context("天气代码无效")? as u8;
    let t = current["time"].as_i64().context("天气缺少时间")?;
    ensure!(t <= now + 900 && now - t <= 7200, "天气服务数据已过期");
    // Tomorrow is optional: a short daily series only loses the forecast row.
    let tomorrow = (|| -> Result<(u8, i16, i16)> {
        let code = daily["weather_code"][1]
            .as_u64()
            .filter(|n| *n <= 99 && valid_code(*n as u8))
            .context("明天天气代码无效")? as u8;
        Ok((
            code,
            temperature(&daily["temperature_2m_min"][1])?,
            temperature(&daily["temperature_2m_max"][1])?,
        ))
    })()
    .ok()
    .filter(|(_, low, high)| low <= high);
    let s = Snapshot {
        city: city.into(),
        region: region.into(),
        fetched_at: now,
        temperature_tenths: temperature(&current["temperature_2m"])?,
        low_tenths: temperature(&daily["temperature_2m_min"][0])?,
        high_tenths: temperature(&daily["temperature_2m_max"][0])?,
        code,
        tomorrow,
    };
    ensure!(s.valid(), "天气摘要无效");
    Ok(s)
}
pub async fn fetch(root: &Path, settings: &Settings) -> Result<Snapshot> {
    ensure!(settings.enabled && settings.valid(), "天气已关闭或设置无效");
    fetch_city(root, &settings.cities()[0], 0).await
}
pub async fn fetch_city(root: &Path, query: &str, slot: usize) -> Result<Snapshot> {
    ensure!(slot < 2, "最多两个城市");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("HomeCraft/0.3 weather")
        .build()?;
    let now = crate::usage::now();
    let city = location(&client, root, query, slot, now).await?;
    let v = get(
        &client,
        "https://api.open-meteo.com/v1/forecast",
        &[
            ("latitude", city.latitude.to_string()),
            ("longitude", city.longitude.to_string()),
            ("current", "temperature_2m,weather_code".into()),
            (
                "daily",
                "temperature_2m_min,temperature_2m_max,weather_code".into(),
            ),
            ("forecast_days", "2".into()),
            ("timeformat", "unixtime".into()),
            ("timezone", "auto".into()),
        ],
    )
    .await?;
    parse_forecast(&v, &city.city, &city.region, now)
}
#[derive(Debug, PartialEq)]
pub enum Sync {
    Send(i64),
    Clear,
}
/// What a display needs given what it was last sent (None: unknown since connecting; Some(0): cleared).
pub fn sync_action(last: Option<i64>, weather: Option<&Snapshot>, now: i64) -> Option<Sync> {
    match weather.filter(|w| w.fresh(now)) {
        Some(w) if last != Some(w.fetched_at) => Some(Sync::Send(w.fetched_at)),
        None if last != Some(0) => Some(Sync::Clear),
        _ => None,
    }
}
pub fn spawn(root: PathBuf, events: crate::backend::Events) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut previous = None;
        let mut next = [0i64; 2];
        loop {
            let settings = crate::config::read_private::<crate::home::model::Preferences>(
                &root.join(".home-ui.json"),
            )
            .ok()
            .filter(|p| p.validate())
            .unwrap_or_default()
            .weather;
            if previous.as_ref() != Some(&settings) {
                // A changed city must not keep showing the previous city's summary.
                if previous.is_some() {
                    events.update(|s| s.weather = [None, None]);
                }
                next = [0; 2];
                previous = Some(settings.clone());
            }
            let cities = settings.cities();
            if !settings.enabled {
                events.update(|s| {
                    s.weather = [None, None];
                    s.weather_notice = "天气已关闭".into();
                });
            } else {
                for (slot, city) in cities.iter().enumerate() {
                    if crate::usage::now() < next[slot] {
                        continue;
                    }
                    match fetch_city(&root, city, slot).await {
                        Ok(w) => {
                            events.update(|s| {
                                s.weather[slot] = Some(w);
                                s.weather_notice = "已更新 · Open-Meteo".into();
                            });
                            next[slot] = crate::usage::now() + 1800;
                        }
                        Err(_) => {
                            events.update(|s| {
                                s.weather_notice =
                                    format!("{city} 天气暂未更新，可稍后重试或检查城市名称")
                            });
                            next[slot] = crate::usage::now() + 300;
                        }
                    }
                }
                if cities.len() < 2 {
                    events.update(|s| s.weather[1] = None);
                }
            }
            // Never keep showing an expired summary; displays clear it on the next sync.
            let now = crate::usage::now();
            events.update(|s| {
                for slot in &mut s.weather {
                    if slot.as_ref().is_some_and(|w| !w.fresh(now)) {
                        *slot = None;
                        s.weather_notice = "天气已过期，等待重新获取".into();
                    }
                }
            });
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_and_stale_forecasts_are_not_sent() {
        let now = 1792987200;
        let mut v = json!({"current":{"time":now,"temperature_2m":22.3,"weather_code":3},"daily":{"temperature_2m_min":[18.0],"temperature_2m_max":[26.0]}});
        let w = parse_forecast(&v, "新加坡", "", now).unwrap();
        assert_eq!(w.temperature_tenths, 223);
        assert!(w.wire(now, 0, true).is_ok());
        assert_eq!(w.tomorrow, None);
        assert!(
            String::from_utf8(w.wire(now, 1, false).unwrap())
                .unwrap()
                .contains("\"slot\":1")
        );
        assert!(
            !String::from_utf8(w.wire(now, 0, false).unwrap())
                .unwrap()
                .contains("slot")
        );
        assert!(w.fresh(now + MAX_AGE));
        assert!(!w.fresh(now + MAX_AGE + 1));
        assert!(w.wire(now + MAX_AGE + 1, 0, false).is_err());
        v["current"]["weather_code"] = json!(4);
        assert!(parse_forecast(&v, "新加坡", "", now).is_err());
        v["current"]["weather_code"] = json!(3);
        v["current"]["time"] = json!(now - 7201);
        assert!(parse_forecast(&v, "新加坡", "", now).is_err());
    }
    #[test]
    fn weather_packet_contains_no_location_coordinates_or_ip() {
        let now = 1792987200;
        let w = Snapshot {
            city: "上海".into(),
            region: "中国".into(),
            fetched_at: now,
            temperature_tenths: 190,
            low_tenths: 150,
            high_tenths: 220,
            code: 0,
            tomorrow: Some((61, 160, 210)),
        };
        let bytes = w.wire(now, 0, true).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains("上海"));
        assert!(!s.contains("latitude"));
        assert!(!s.contains("中国"));
        assert!(!s.contains("password"));
        assert!(s.contains(r#""tomorrow":[61,160,210]"#));
        assert!(
            !String::from_utf8(w.wire(now, 0, false).unwrap())
                .unwrap()
                .contains("tomorrow")
        );
    }
    #[test]
    fn tomorrow_comes_from_the_second_daily_entry_and_is_optional() {
        let now = 1792987200;
        let v = json!({"current":{"time":now,"temperature_2m":27.0,"weather_code":2},"daily":{"temperature_2m_min":[25.0,24.6],"temperature_2m_max":[31.0,30.4],"weather_code":[2,95]}});
        let w = parse_forecast(&v, "新加坡", "", now).unwrap();
        assert_eq!(w.tomorrow, Some((95, 246, 304)));
        let mut bad = v.clone();
        bad["daily"]["weather_code"][1] = json!(4);
        assert_eq!(
            parse_forecast(&bad, "新加坡", "", now).unwrap().tomorrow,
            None
        );
        let old: Snapshot = serde_json::from_str(r#"{"city":"新加坡","fetched_at":1792987200,"temperature_tenths":270,"low_tenths":250,"high_tenths":310,"code":2}"#).unwrap();
        assert!(old.valid() && old.tomorrow.is_none());
    }
    #[test]
    fn displays_get_each_fresh_summary_once_and_a_clear_when_off_or_expired() {
        let now = 1792987200;
        let w = Snapshot {
            city: "新加坡".into(),
            region: String::new(),
            fetched_at: now,
            temperature_tenths: 271,
            low_tenths: 252,
            high_tenths: 298,
            code: 2,
            tomorrow: None,
        };
        // Unknown after (re)connect: send fresh weather, or clear stale content left on the screen.
        assert_eq!(sync_action(None, Some(&w), now), Some(Sync::Send(now)));
        assert_eq!(sync_action(None, None, now), Some(Sync::Clear));
        assert_eq!(sync_action(Some(now), Some(&w), now + 60), None);
        // Weather turned off, or the hub copy expired without a successful refetch.
        assert_eq!(sync_action(Some(now), None, now), Some(Sync::Clear));
        assert_eq!(
            sync_action(Some(now), Some(&w), now + MAX_AGE + 1),
            Some(Sync::Clear)
        );
        assert_eq!(sync_action(Some(0), None, now), None);
        let mut newer = w.clone();
        newer.fetched_at = now + 1800;
        assert_eq!(
            sync_action(Some(now), Some(&newer), now + 1800),
            Some(Sync::Send(now + 1800))
        );
    }
    #[test]
    fn city_defaults_to_singapore_and_old_automatic_preference_falls_back() {
        assert_eq!(Settings::default().city(), DEFAULT_CITY);
        let old: Settings =
            serde_json::from_str(r#"{"enabled":true,"automatic_city":true,"city":""}"#).unwrap();
        assert_eq!(old.city(), DEFAULT_CITY);
        let manual = Settings {
            enabled: true,
            city: "  Kuala Lumpur ".into(),
            second_city: " 新加坡 ".into(),
        };
        assert_eq!(manual.city(), "Kuala Lumpur");
        assert_eq!(manual.cities(), vec!["Kuala Lumpur", "新加坡"]);
        let same = Settings {
            second_city: DEFAULT_CITY.into(),
            ..Settings::default()
        };
        assert_eq!(same.cities().len(), 1);
        assert!(manual.valid());
        assert!(
            !Settings {
                city: "a\nb".into(),
                ..Settings::default()
            }
            .valid()
        );
        assert!(
            !Settings {
                second_city: "x".repeat(33),
                ..Settings::default()
            }
            .valid()
        );
    }
}
