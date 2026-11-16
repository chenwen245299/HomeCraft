//! Account polling is independent of the display connection and its keepalive.
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Settings {
    pub enabled: bool,
    pub seconds: u32,
}
impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Stored {
            enabled: Option<bool>,
            seconds: Option<u32>,
            minutes: Option<u32>,
        }
        let stored = Stored::deserialize(deserializer)?;
        Ok(Self {
            enabled: stored.enabled.unwrap_or(true),
            seconds: stored
                .seconds
                .or_else(|| stored.minutes.map(|m| m.saturating_mul(60)))
                .unwrap_or(30),
        })
    }
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            seconds: 30,
        }
    }
}
impl Settings {
    pub fn valid(self) -> bool {
        (30..=7200).contains(&self.seconds)
    }
    fn interval(self) -> Duration {
        Duration::from_secs(u64::from(self.seconds.clamp(30, 7200)))
    }
    pub fn label(self) -> String {
        if self.seconds.is_multiple_of(60) {
            format!("{} 分钟", self.seconds / 60)
        } else {
            format!("{} 秒", self.seconds)
        }
    }
}

pub struct Schedule {
    pub settings: Settings,
    last: Option<Instant>,
    next: Option<Instant>,
}
impl Schedule {
    pub fn new(settings: Settings, now: Instant) -> Self {
        Self {
            settings,
            last: None,
            next: settings.enabled.then_some(now),
        }
    }
    pub fn configure(&mut self, settings: Settings, now: Instant) {
        self.settings = settings;
        self.next = settings.enabled.then(|| {
            self.last
                .map_or(now, |last| (last + settings.interval()).max(now))
        });
    }
    pub fn due(&self, now: Instant) -> bool {
        self.next.is_some_and(|next| now >= next)
    }
    pub fn request(&mut self, now: Instant) {
        self.next = Some(now); // Manual actions bypass the automatic timer.
    }
    pub fn finish(&mut self, now: Instant) {
        self.last = Some(now);
        self.next = self
            .settings
            .enabled
            .then_some(now + self.settings.interval());
    }
    pub fn next_unix(&self, now: Instant, unix: i64) -> Option<i64> {
        self.next
            .map(|next| unix + next.saturating_duration_since(now).as_secs() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_legacy_settings_and_bounds() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::default());
        let legacy: Settings = serde_json::from_str(r#"{"enabled":false,"minutes":5}"#).unwrap();
        assert_eq!(
            legacy,
            Settings {
                enabled: false,
                seconds: 300
            }
        );
        let modern: Settings =
            serde_json::from_str(r#"{"enabled":true,"seconds":30,"minutes":5}"#).unwrap();
        assert_eq!(modern, Settings::default());
        assert_eq!(serde_json::to_value(modern).unwrap()["seconds"], 30);
        for seconds in [0, 29, 7201, u32::MAX] {
            assert!(
                !Settings {
                    seconds,
                    ..settings
                }
                .valid()
            );
        }
    }
    #[test]
    fn polling_runs_without_a_device_and_live_changes_use_last_completion() {
        let t = Instant::now();
        let mut s = Schedule::new(Settings::default(), t);
        assert!(s.due(t));
        s.finish(t);
        assert!(!s.due(t + Duration::from_secs(29)));
        assert!(s.due(t + Duration::from_secs(30)));
        s.configure(
            Settings {
                seconds: 600,
                ..Settings::default()
            },
            t + Duration::from_secs(100),
        );
        assert_eq!(s.next_unix(t, 1000), Some(1600));
        s.configure(
            Settings {
                seconds: 30,
                ..Settings::default()
            },
            t + Duration::from_secs(100),
        );
        assert!(s.due(t + Duration::from_secs(100)));
        s.finish(t + Duration::from_secs(100));
        assert!(!s.due(t + Duration::from_secs(129)));
    }
    #[test]
    fn disabling_stops_automatic_reads_but_preserves_manual_refresh() {
        let t = Instant::now();
        let mut s = Schedule::new(
            Settings {
                enabled: false,
                ..Settings::default()
            },
            t,
        );
        assert!(!s.due(t + Duration::from_secs(900)));
        assert_eq!(s.next_unix(t, 1000), None);
        s.request(t);
        assert!(s.due(t));
        s.finish(t);
        assert_eq!(s.next_unix(t, 1000), None);
        s.request(t + Duration::from_secs(5));
        assert!(s.due(t + Duration::from_secs(5)));
        s.configure(
            Settings {
                enabled: false,
                ..Settings::default()
            },
            t,
        );
        assert!(!s.due(t + Duration::from_secs(900)));
    }
    #[test]
    fn missed_deadlines_coalesce_into_one_read() {
        let t = Instant::now();
        let mut s = Schedule::new(Settings::default(), t);
        s.finish(t);
        let later = t + Duration::from_secs(3600);
        assert!(s.due(later));
        s.finish(later);
        assert_eq!(s.next_unix(later, 5000), Some(5030));
        assert!(!s.due(later));
    }
}
