//! Numeric, content-free identities for billed attempts shared by synchronized logs.
use super::{Day, LIMIT, day_id};
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

const MAX_CALLS: usize = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub key: String,
    pub at: i64,
    pub revision: u64,
    pub settled: bool,
    pub tokens: [u64; 3],
}

impl Call {
    pub(super) fn key(session: &str, turn: u64, step: u64, retry: u64) -> String {
        let mut hash = Sha256::new();
        hash.update(b"HomeCraft/harness-attempt/v1\0");
        hash.update((session.len() as u64).to_le_bytes());
        hash.update(session.as_bytes());
        for n in [turn, step, retry] {
            hash.update(n.to_le_bytes());
        }
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }
    fn date(&self) -> Option<NaiveDate> {
        chrono::DateTime::from_timestamp_millis(self.at)
            .and_then(|t| t.checked_add_signed(chrono::Duration::hours(8)))
            .map(|t| t.date_naive())
    }
    fn valid(&self) -> bool {
        self.key.len() == 64
            && self
                .key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.revision > 0
            && self
                .date()
                .is_some_and(|d| (2025..=2099).contains(&d.year()))
            && self.tokens[0]
                .checked_add(self.tokens[1])
                .is_some_and(|n| n <= LIMIT / 8)
            && self.tokens[2] <= self.tokens[0]
    }
    fn day(&self) -> Day {
        Day {
            input: self.tokens[0],
            output: self.tokens[1],
            cached: self.tokens[2],
            calls: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub version: u8,
    pub day: i64,
    // Immutable between imports: state and presentation snapshots share the records.
    // Serde retains the existing JSON array without changing the file schema.
    pub calls: Arc<Vec<Call>>,
    /// Conflicting equal revisions cannot be resolved by upload time or token magnitude.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub conflict: bool,
}
pub(super) fn date(id: i64) -> Option<NaiveDate> {
    (20250101..=20991231)
        .contains(&id)
        .then(|| NaiveDate::parse_from_str(&id.to_string(), "%Y%m%d").ok())
        .flatten()
}
fn first(day: NaiveDate) -> NaiveDate {
    day.with_day(1)
        .unwrap()
        .min(day - chrono::Duration::days(i64::from(day.weekday().num_days_from_monday())))
}
impl Ledger {
    pub(super) fn new(day: NaiveDate, calls: impl IntoIterator<Item = Call>) -> Result<Self> {
        let mut result = Self {
            version: 1,
            day: day_id(day),
            calls: Arc::default(),
            conflict: false,
        };
        let start = first(day);
        let mut entries = BTreeMap::new();
        for call in calls {
            ensure!(call.valid(), "Harness 调用明细无效");
            if call.date().is_some_and(|d| d >= start && d <= day) {
                Self::insert(&mut entries, call)?;
                ensure!(entries.len() <= MAX_CALLS, "Harness 调用明细过多");
            }
        }
        result.calls = Arc::new(entries.into_values().collect());
        result.days()?;
        Ok(result)
    }
    fn insert(entries: &mut BTreeMap<String, Call>, call: Call) -> Result<()> {
        if let Some(old) = entries.get_mut(&call.key) {
            let rank = |c: &Call| (c.settled, c.revision);
            if rank(&call) == rank(old) {
                ensure!(
                    old.tokens == call.tokens && old.date() == call.date(),
                    "Harness 同一调用存在冲突，等待核对"
                );
                // Synchronized copies can timestamp the same usage differently. The
                // UTC+8 billing day and counters must agree; use a stable timestamp
                // so repeated imports converge regardless of source order.
                old.at = old.at.min(call.at);
            } else if rank(&call) > rank(old) {
                *old = call;
            }
        } else {
            entries.insert(call.key.clone(), call);
        }
        Ok(())
    }
    pub fn valid(&self) -> bool {
        let Some(day) = date(self.day) else {
            return false;
        };
        self.version == 1
            && self.calls.len() <= MAX_CALLS
            && self.calls.windows(2).all(|w| w[0].key < w[1].key)
            && self
                .calls
                .iter()
                .all(|c| c.valid() && c.date().is_some_and(|d| d >= first(day) && d <= day))
    }
    pub(super) fn days(&self) -> Result<BTreeMap<NaiveDate, Day>> {
        ensure!(
            self.valid() && !self.conflict,
            "Harness 调用明细冲突或无效，等待核对"
        );
        let mut days = BTreeMap::<NaiveDate, Day>::new();
        for call in self.calls.iter() {
            days.entry(call.date().unwrap())
                .or_default()
                .add(call.day())?;
        }
        ensure!(
            days.values()
                .try_fold(0u64, |sum, d| sum.checked_add(d.total()))
                .is_some_and(|n| n <= LIMIT / 8),
            "Harness 调用汇总越界"
        );
        Ok(days)
    }
    pub(super) fn union<'a>(
        day: NaiveDate,
        ledgers: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Self> {
        let mut calls = Vec::new();
        for ledger in ledgers {
            ensure!(
                ledger.valid() && !ledger.conflict,
                "Harness 调用明细冲突或无效，等待核对"
            );
            calls.extend(ledger.calls.iter().cloned());
        }
        Self::new(day, calls)
    }
    pub(super) fn merge(&self, other: &Self) -> Self {
        let day = date(self.day.max(other.day)).unwrap();
        Self::union(day, [self, other]).unwrap_or_else(|_| {
            let mut result = if self.day >= other.day {
                self.clone()
            } else {
                other.clone()
            };
            result.conflict = true;
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn day(id: i64) -> NaiveDate {
        date(id).unwrap()
    }
    fn call(session: &str, id: i64, retry: u64, n: u64) -> Call {
        Call {
            key: Call::key(session, 1, 1, retry),
            at: day(id)
                .and_hms_opt(4, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis(),
            revision: 1,
            settled: true,
            tokens: [n, 10, 5],
        }
    }
    #[test]
    fn union_is_idempotent_commutative_and_associative_with_partial_copies() {
        let a = Ledger::new(day(20261021), [call("a", 20261021, 0, 100)]).unwrap();
        let b = Ledger::new(
            day(20261021),
            [call("a", 20261021, 0, 100), call("b", 20261021, 0, 200)],
        )
        .unwrap();
        let c = Ledger::new(day(20261021), [call("a", 20261021, 1, 30)]).unwrap();
        let ab = Ledger::union(day(20261021), [&a, &b]).unwrap();
        assert_eq!(ab, Ledger::union(day(20261021), [&b, &a, &a, &b]).unwrap());
        assert_eq!(ab.calls.len(), 2);
        let bc = Ledger::union(day(20261021), [&b, &c]).unwrap();
        assert_eq!(
            Ledger::union(day(20261021), [&ab, &c]).unwrap(),
            Ledger::union(day(20261021), [&a, &bc]).unwrap()
        );
        assert_eq!(bc.days().unwrap()[&day(20261021)].total(), 360);
    }
    #[test]
    fn settlement_can_reduce_usage_and_move_the_same_attempt_across_midnight() {
        let final_call = call("a", 20261001, 0, 60);
        let mut partial = final_call.clone();
        partial.at = day(20260930)
            .and_hms_opt(15, 59, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        partial.settled = false;
        partial.tokens[0] = 100;
        partial.revision = 99;
        let a = Ledger::new(day(20260930), [partial]).unwrap();
        let b = Ledger::new(day(20261001), [final_call]).unwrap();
        let union = Ledger::union(day(20261001), [&a, &b]).unwrap();
        assert_eq!(union.calls.len(), 1);
        assert!(!union.days().unwrap().contains_key(&day(20260930)));
        assert_eq!(union.days().unwrap()[&day(20261001)].total(), 70);
    }
    #[test]
    fn same_day_timestamp_variants_converge_without_counting_copies_twice() {
        let original = call("shared", 20261021, 0, 100);
        let mut replayed = original.clone();
        replayed.at += 95 * 60 * 1000;
        let mut third = original.clone();
        third.at += 23_322;
        let a = Ledger::new(day(20261021), [original.clone()]).unwrap();
        let b = Ledger::new(day(20261021), [replayed]).unwrap();
        let c = Ledger::new(day(20261021), [third]).unwrap();
        let ab = Ledger::union(day(20261021), [&a, &b]).unwrap();
        let bc = Ledger::union(day(20261021), [&b, &c]).unwrap();
        assert_eq!(*ab.calls, vec![original]);
        assert_eq!(ab.days().unwrap()[&day(20261021)].total(), 110);
        assert_eq!(ab, Ledger::union(day(20261021), [&b, &a, &b]).unwrap());
        assert_eq!(
            Ledger::union(day(20261021), [&ab, &c]).unwrap(),
            Ledger::union(day(20261021), [&a, &bc]).unwrap()
        );
        assert_eq!(a.merge(&b).merge(&c), c.merge(&b).merge(&a));
        assert!(!a.merge(&b).conflict);
    }
    #[test]
    fn equal_revision_counter_or_billing_day_conflicts_remain_unknown() {
        let original = call("shared", 20261021, 0, 100);
        for changed in [
            Call {
                tokens: [101, 10, 5],
                ..original.clone()
            },
            Call {
                tokens: [100, 10, 6],
                ..original.clone()
            },
            Call {
                at: original.at + 24 * 60 * 60 * 1000,
                ..original.clone()
            },
        ] {
            let a = Ledger::new(day(20261022), [original.clone()]).unwrap();
            let b = Ledger::new(day(20261022), [changed]).unwrap();
            assert!(Ledger::union(day(20261022), [&a, &b]).is_err());
            assert!(a.merge(&b).conflict);
        }
    }
    #[test]
    fn calendar_window_retains_the_previous_month_part_of_this_week() {
        let old = Ledger::new(
            day(20260930),
            [
                call("old", 20260910, 0, 900),
                call("week", 20260930, 0, 100),
            ],
        )
        .unwrap();
        let current = Ledger::new(day(20261001), [call("new", 20261001, 0, 200)]).unwrap();
        let merged = Ledger::union(day(20261001), [&old, &current]).unwrap();
        let days = merged.days().unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[&day(20260930)].total(), 110);
        assert_eq!(days[&day(20261001)].total(), 210);
        let next_week = Ledger::union(day(20261005), [&merged]).unwrap();
        assert_eq!(next_week.calls.len(), 1);
    }
    #[test]
    fn schemas_reject_content_duplicate_keys_negative_counts_and_unknown_versions() {
        let ledger = Ledger::new(
            day(20261021),
            [call("private-session-id", 20261021, 0, 100)],
        )
        .unwrap();
        let raw = serde_json::to_string(&ledger).unwrap();
        assert!(!raw.contains("private-session-id"));
        let mut value = serde_json::to_value(&ledger).unwrap();
        value["prompt"] = serde_json::json!("not allowed");
        assert!(serde_json::from_value::<Ledger>(value).is_err());
        let mut invalid = ledger.clone();
        let duplicate = invalid.calls[0].clone();
        Arc::make_mut(&mut invalid.calls).push(duplicate);
        assert!(!invalid.valid());
        let mut invalid = ledger.clone();
        invalid.version = 2;
        assert!(!invalid.valid());
        let mut invalid = ledger.clone();
        Arc::make_mut(&mut invalid.calls)[0].at = i64::MAX;
        assert!(!invalid.valid());
        let mut value = serde_json::to_value(&ledger).unwrap();
        value["calls"][0]["tokens"][0] = serde_json::json!(-1);
        assert!(serde_json::from_value::<Ledger>(value).is_err());
    }
    #[test]
    fn snapshot_clones_share_records_without_changing_serialization_or_the_original() {
        let ledger = Ledger::new(day(20261021), [call("shared", 20261021, 0, 100)]).unwrap();
        let mut copy = ledger.clone();
        assert!(Arc::ptr_eq(&ledger.calls, &copy.calls));
        let value = serde_json::to_value(&ledger).unwrap();
        assert!(value["calls"].is_array());
        assert_eq!(serde_json::from_value::<Ledger>(value).unwrap(), ledger);
        Arc::make_mut(&mut copy.calls)[0].tokens[0] = 200;
        assert_eq!(ledger.calls[0].tokens[0], 100);
        assert!(!Arc::ptr_eq(&ledger.calls, &copy.calls));
    }
}
