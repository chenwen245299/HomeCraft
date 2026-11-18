//! One resolved numeric view for every Mac window and display session.
//! Collection and cloud publication always keep the original, machine-scoped readings.
use crate::{
    backend::State,
    cloud, harness,
    usage::{self, Reading},
};
use std::collections::BTreeMap;

pub fn catalog(state: &State) -> cloud::Catalog {
    let mut catalog = state.cloud_data.clone();
    if !state.usage_resolved
        && let Some(id) = &state.cloud.origin
    {
        let origin = catalog
            .snapshots
            .iter()
            .find(|s| &s.origin.id == id)
            .map(|s| s.origin.clone())
            .unwrap_or_else(|| harness::Origin {
                id: id.clone(),
                name: "This Mac".into(),
            });
        catalog.capture(state, origin);
    }
    catalog
}

/// Argus reports the current local library. Each Mac publishes its own original summary once.
/// Never insert a merged reading here: that would count the same peer again at every exchange.
pub fn argus_sources(state: &State) -> Vec<(harness::Origin, cloud::Summary)> {
    let mut sources = BTreeMap::new();
    for s in &catalog(state).snapshots {
        for u in s.usages.iter().filter(|u| u.packet.p == 3) {
            let entry = sources
                .entry(s.origin.id.clone())
                .or_insert_with(|| (s.origin.clone(), u.clone()));
            if u.packet.t > entry.1.packet.t {
                *entry = (s.origin.clone(), u.clone());
            }
        }
    }
    sources.into_values().collect()
}

fn argus(state: &State) -> Option<Reading> {
    // These Macs use the same synced library (confirmed by the user). The MCP result is
    // already library-wide: select the newest valid observation, never sum per-Mac copies.
    let sources = argus_sources(state);
    let (_, newest) = sources
        .iter()
        .max_by_key(|(origin, s)| (s.packet.t, &origin.id))?;
    let mut r = newest.reading();
    let catalog = catalog(state);
    let day = crate::home::model::day();
    let latest = |period, select: fn(&usage::Packet) -> Option<[i64; 2]>| {
        catalog
            .snapshots
            .iter()
            .flat_map(|s| s.usages.iter().map(move |u| (&s.origin, u)))
            .filter(|(_, u)| {
                u.packet.p == 3 && select(&u.packet).is_some_and(|v| v[0] == period && v[1] >= 0)
            })
            .max_by_key(|(origin, u)| (u.packet.t, &origin.id))
            .and_then(|(_, u)| select(&u.packet))
    };
    r.packet.wt = latest(crate::home::model::week_start(day)?, |p| p.wt).or(r.packet.wt);
    r.packet.mt = latest(crate::home::model::month_start(day)?, |p| p.mt).or(r.packet.mt);
    r.tokens.weekly = r.packet.wt.map(|v| v[1] as u64);
    r.tokens.monthly = r.packet.mt.map(|v| v[1] as u64);
    r.tokens.note = "共享文献库 · 最新有效汇总只计一次 · UTC+8".into();
    Some(r)
}

pub fn readings(state: &State) -> Vec<Reading> {
    if state.usage_resolved {
        return state.readings.clone();
    }
    let catalog = catalog(state);
    let mut out = cloud::merged_readings(&state.readings, &catalog).0;
    if let Some(argus) = argus(state) {
        if let Some(old) = out.iter_mut().find(|r| r.packet.p == 3) {
            *old = argus;
        } else {
            out.push(argus);
        }
    }
    let devices = harness::merge_devices(&state.harness_devices, &catalog.harness());
    if !devices.is_empty() && !out.iter().any(|r| r.packet.p == harness::PROVIDER) {
        out.push(usage::empty(harness::PROVIDER, 0, "已同步的 Harness 记录"));
    }
    out.into_iter()
        .map(|r| harness::effective(&r, &devices))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::model;
    fn origin(id: &str) -> harness::Origin {
        harness::Origin {
            id: id.into(),
            name: format!("Mac-{}", &id[..1]),
        }
    }
    fn reading(p: u8, id: &str, n: i64) -> Reading {
        if p == 4 {
            return harness::test_reading(id, n as u64);
        }
        let mut r = usage::empty(p, usage::OK, "test original");
        let day = model::day();
        r.packet.d = Some([day, n]);
        r.packet.wt = Some([model::week_start(day).unwrap(), n * 2]);
        r.packet.mt = Some([model::month_start(day).unwrap(), n * 3]);
        if p == 0 {
            r.tokens.account = crate::account::Identity::parse(
                Some(
                    &serde_json::json!({"account":{"type":"chatgpt","email":"same@example.test"}}),
                ),
                origin(id),
            );
        } else {
            r.packet.n = Some(if p == 3 { "Argus" } else { harness::NAME }.into());
            r.packet.u = Some([n, 0, 0, 1, 1]);
            if p == 4 {
                r.packet.o = Some(origin(id));
            }
        }
        r
    }
    fn pair() -> (State, State) {
        let a = "1111111111111111";
        let b = "2222222222222222";
        let mut first = State {
            readings: vec![reading(0, a, 1000), reading(3, a, 100), reading(4, a, 10)],
            ..Default::default()
        };
        first.cloud.origin = Some(a.into());
        let mut second = State {
            readings: vec![reading(0, b, 1000), reading(3, b, 200), reading(4, b, 20)],
            ..Default::default()
        };
        second.cloud.origin = Some(b.into());
        let mut c = cloud::Catalog::default();
        c.capture(&first, origin(a));
        c.capture(&second, origin(b));
        first.cloud_data = c.clone();
        second.cloud_data = c;
        (first, second)
    }
    #[test]
    fn two_macs_converge_for_all_periods_without_counting_codex_twice() {
        let (a, b) = pair();
        let day = model::day();
        assert_eq!(model::today_total(&a, day), Some(1230));
        assert_eq!(model::week_total(&a, day), Some(2430));
        assert_eq!(model::month_total(&a, day), Some(3630));
        assert_eq!(model::totals_wire(&a, day), model::totals_wire(&b, day));
        for state in [a, b] {
            let view = cloud::presentation(state.clone());
            let again = cloud::presentation(view.clone());
            assert_eq!(
                model::totals_wire(&state, day),
                model::totals_wire(&view, day)
            );
            assert_eq!(
                model::totals_wire(&view, day),
                model::totals_wire(&again, day)
            );
            let harness = view.readings.iter().find(|r| r.packet.p == 4).unwrap();
            assert_eq!(
                harness::effective(harness, &view.harness_devices).packet.d,
                Some([day, 30])
            );
            let mut publish = cloud::Catalog::default();
            publish.capture(&view, origin("1111111111111111"));
            assert!(publish.snapshots.is_empty());
        }
    }
    #[test]
    fn local_failure_and_no_local_harness_still_show_the_hub() {
        let (mut a, b) = pair();
        a.readings.iter_mut().for_each(|r| r.packet.s = 6);
        a.readings.retain(|r| r.packet.p != 4);
        assert_eq!(
            model::totals_wire(&a, model::day()),
            model::totals_wire(&b, model::day())
        );
        let view = readings(&a);
        assert_eq!(
            view.iter()
                .find(|r| r.packet.p == 4)
                .unwrap()
                .packet
                .d
                .unwrap()[1],
            30
        );
    }
    #[test]
    fn latest_local_value_replaces_its_cache_and_exports_stay_original() {
        let (mut a, _) = pair();
        let old = a.readings[1].packet.t;
        a.readings[1] = reading(3, "1111111111111111", 150);
        a.readings[1].packet.t = old + 1;
        assert_eq!(model::today_total(&a, model::day()), Some(1180));
        let c = catalog(&a);
        let own = c
            .snapshots
            .iter()
            .find(|s| s.origin.id == "1111111111111111")
            .unwrap();
        assert_eq!(
            own.usages
                .iter()
                .find(|u| u.packet.p == 3)
                .unwrap()
                .packet
                .d
                .unwrap()[1],
            150
        );
        assert_eq!(
            own.usages
                .iter()
                .find(|u| u.packet.p == 4)
                .unwrap()
                .packet
                .d
                .unwrap()[1],
            10
        );
    }
    #[test]
    fn newer_codex_without_period_fields_keeps_last_valid_account_observation() {
        let (mut a, _) = pair();
        a.readings[0].packet.t += 2;
        a.readings[0].packet.d = Some([model::day(), -1]);
        a.readings[0].packet.wt = None;
        a.readings[0].packet.mt = None;
        assert_eq!(model::today_total(&a, model::day()), Some(1230));
        assert_eq!(model::week_total(&a, model::day()), Some(2430));
        assert_eq!(model::month_total(&a, model::day()), Some(3630));
        a.cloud_data = catalog(&a);
        a.readings.clear();
        assert_eq!(model::today_total(&a, model::day()), Some(1230));
        assert_eq!(model::week_total(&a, model::day()), Some(2430));
        assert_eq!(model::month_total(&a, model::day()), Some(3630));
    }
}
