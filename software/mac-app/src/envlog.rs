//! Hub-side temperature/humidity history per display. Some displays (E1002) keep none, so the hub
//! records what each one reports and the Climate page can compare locations.
use std::{collections::BTreeMap, path::Path};

pub type Log = BTreeMap<String, Vec<(i64, f32, f32)>>;
const KEEP: i64 = 48 * 3600;
const SPACING: i64 = 240;
const MAX_POINTS: usize = 1000;
const MAX_DEVICES: usize = 8;
/// Adds a sample unless it repeats or crowds the previous one; returns true when stored.
pub fn record(log: &mut Log, id: &str, at: i64, temperature: f32, humidity: f32) -> bool {
    if !temperature.is_finite()
        || !humidity.is_finite()
        || !(-45.0..=130.0).contains(&temperature)
        || !(0.0..=100.0).contains(&humidity)
        || (!log.contains_key(id) && log.len() >= MAX_DEVICES)
    {
        return false;
    }
    let points = log.entry(id.into()).or_default();
    if points
        .last()
        .is_some_and(|(last, _, _)| at - last < SPACING)
    {
        return false;
    }
    points.push((at, temperature, humidity));
    points.retain(|(t, _, _)| at - t <= KEEP);
    if points.len() > MAX_POINTS {
        points.drain(..points.len() - MAX_POINTS);
    }
    true
}
pub fn load(root: &Path) -> Log {
    crate::cloud::read_json::<Log>(&root.join(".environment-log.json"), 2 * 1024 * 1024)
        .unwrap_or_default()
}
pub fn save(root: &Path, log: &Log) {
    if let Ok(bytes) = serde_json::to_vec(log) {
        let _ = crate::cloud::atomic(&root.join(".environment-log.json"), &bytes);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_are_spaced_bounded_and_trimmed() {
        let mut log = Log::new();
        assert!(record(&mut log, "a", 1000, 24.0, 50.0));
        assert!(!record(&mut log, "a", 1100, 24.1, 50.0));
        assert!(record(&mut log, "a", 1300, 24.2, 51.0));
        assert!(!record(&mut log, "a", 1600, f32::NAN, 50.0));
        assert!(record(&mut log, "a", 1000 + KEEP + 400, 25.0, 52.0));
        assert_eq!(log["a"].len(), 1);
        for i in 0..MAX_DEVICES {
            record(&mut log, &format!("d{i}"), 1, 20.0, 40.0);
        }
        assert!(!record(&mut log, "extra", 1, 20.0, 40.0));
    }
}
