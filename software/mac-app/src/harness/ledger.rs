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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub through: Option<i64>,
    /// `at` came from the matching, preserved usage chunk, not the live append envelope.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub usage_time: bool,
    /// Retention only: keep original-time evidence while a retimed peer references it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_day: Option<i64>,
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
    pub(super) fn date(&self) -> Option<NaiveDate> {
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
            && self.retain_day.is_none_or(|d| date(d).is_some())
            && self
                .date()
                .is_some_and(|d| (2025..=2099).contains(&d.year()))
            && self.through.is_none_or(|at| {
                at >= self.at
                    && chrono::DateTime::from_timestamp_millis(at).is_some_and(|t| {
                        (2025..=2099).contains(&(t + chrono::Duration::hours(8)).year())
                    })
            })
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
pub(super) fn first(day: NaiveDate) -> NaiveDate {
    day.with_day(1)
        .unwrap()
        .min(day - chrono::Duration::days(i64::from(day.weekday().num_days_from_monday())))
}
/// A disagreement only invalidates periods whose membership or counters differ.
/// Keep the original observations intact; never invent a canonical billing date.
pub(super) struct Periods {
    pub values: [Option<Day>; 3],
    pub disputed: [bool; 3],
}
impl Ledger {
    pub(super) fn periods<'a>(
        today: NaiveDate,
        ledgers: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Periods> {
        let starts = [
            today,
            today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday())),
            today.with_day(1).unwrap(),
        ];
        let mut result = Periods {
            values: [None; 3],
            disputed: [false; 3],
        };
        let mut entries = BTreeMap::<&str, Vec<&Call>>::new();
        for ledger in ledgers {
            ensure!(
                ledger.valid() && !ledger.conflict,
                "Harness 调用明细冲突或无效，需核对来源"
            );
            for (i, start) in starts.iter().enumerate() {
                if date(ledger.day).is_some_and(|d| d >= *start && d <= today) {
                    result.values[i] = Some(Day::default());
                }
            }
            for call in ledger.calls.iter() {
                let candidates = entries.entry(&call.key).or_default();
                let rank = |c: &Call| (c.settled, c.revision);
                if candidates.first().is_some_and(|old| rank(call) < rank(old)) {
                    continue;
                }
                if candidates.first().is_some_and(|old| rank(call) > rank(old)) {
                    candidates.clear();
                }
                candidates.push(call);
            }
        }
        for candidates in entries.into_values() {
            // A preserved usage timestamp resolves old envelope-only copies, but must
            // never select between conflicting counters of the same revision.
            let original = candidates.iter().any(|c| c.usage_time)
                && candidates.iter().all(|c| c.tokens == candidates[0].tokens);
            let candidates = candidates
                .into_iter()
                .filter(|c| !original || c.usage_time)
                .collect::<Vec<_>>();
            for (i, start) in starts.iter().enumerate() {
                let contribution = |c: &Call| {
                    let first = c.date().unwrap();
                    let last = chrono::DateTime::from_timestamp_millis(c.through.unwrap_or(c.at))
                        .unwrap()
                        + chrono::Duration::hours(8);
                    let last = last.date_naive();
                    if last < *start || first > today {
                        Ok(None)
                    } else if first >= *start && last <= today {
                        Ok(Some(c.day()))
                    } else {
                        Err(())
                    }
                };
                let value = contribution(candidates[0]);
                if value.is_err() || candidates.iter().any(|c| contribution(c) != value) {
                    result.disputed[i] = true;
                } else if let Ok(Some(value)) = value {
                    result.values[i].get_or_insert_default().add(value)?;
                }
            }
        }
        for (i, value) in result.values.iter_mut().enumerate() {
            if result.disputed[i] {
                *value = None;
            }
            ensure!(
                value.is_none_or(|v| v.total() <= LIMIT / 8),
                "Harness 调用汇总越界"
            );
        }
        Ok(result)
    }
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
            Self::insert(&mut entries, call)?;
            ensure!(entries.len() <= MAX_CALLS * 8, "Harness 调用明细过多");
        }
        entries.retain(|_, call| {
            let last = chrono::DateTime::from_timestamp_millis(call.through.unwrap_or(call.at))
                .unwrap()
                + chrono::Duration::hours(8);
            (last.date_naive() >= start
                || call
                    .retain_day
                    .and_then(date)
                    .is_some_and(|d| d >= start && d <= day))
                && call.date().is_some_and(|d| d <= day)
        });
        ensure!(entries.len() <= MAX_CALLS, "Harness 调用明细过多");
        result.calls = Arc::new(entries.into_values().collect());
        result.version = if result
            .calls
            .iter()
            .any(|c| c.usage_time || c.retain_day.is_some())
        {
            3
        } else if result.ambiguous() {
            2
        } else {
            1
        };
        ensure!(result.valid(), "Harness 调用明细无效");
        let mut total = Day::default();
        for call in result.calls.iter() {
            total.add(call.day())?;
        }
        ensure!(total.total() <= LIMIT / 8, "Harness 调用汇总越界");
        Ok(result)
    }
    fn insert(entries: &mut BTreeMap<String, Call>, call: Call) -> Result<()> {
        if let Some(old) = entries.get_mut(&call.key) {
            let mut retain = old.retain_day.max(call.retain_day);
            let rank = |c: &Call| (c.settled, c.revision);
            if rank(&call) == rank(old) {
                ensure!(
                    old.tokens == call.tokens,
                    "Harness 同一调用存在冲突，等待核对"
                );
                // Preserve the date bounds of equal counters. Period totals can be
                // certain even when the billing day is not; imports must converge.
                if old.usage_time != call.usage_time {
                    let envelope = if old.usage_time {
                        call.date()
                    } else {
                        old.date()
                    };
                    retain = retain.max(envelope.map(day_id));
                    if call.usage_time {
                        old.at = call.at;
                        old.through = call.through;
                        old.usage_time = true;
                    }
                } else {
                    let last = old
                        .through
                        .unwrap_or(old.at)
                        .max(call.through.unwrap_or(call.at));
                    old.at = old.at.min(call.at);
                    let last_day = (chrono::DateTime::from_timestamp_millis(last).unwrap()
                        + chrono::Duration::hours(8))
                    .date_naive();
                    old.through = (old.date() != Some(last_day)).then_some(last);
                }
            } else if rank(&call) > rank(old) {
                *old = call;
            }
            old.retain_day = retain;
        } else {
            entries.insert(call.key.clone(), call);
        }
        Ok(())
    }
    pub fn valid(&self) -> bool {
        let Some(day) = date(self.day) else {
            return false;
        };
        matches!(self.version, 1..=3)
            && (self.version >= 2 || !self.ambiguous())
            && (self.version >= 3
                || self
                    .calls
                    .iter()
                    .all(|c| !c.usage_time && c.retain_day.is_none()))
            && self.calls.len() <= MAX_CALLS
            && self.calls.windows(2).all(|w| w[0].key < w[1].key)
            && self.calls.iter().all(|c| {
                c.valid()
                    && c.date().is_some_and(|d| d <= day)
                    && chrono::DateTime::from_timestamp_millis(c.through.unwrap_or(c.at))
                        .is_some_and(|t| {
                            let d = (t + chrono::Duration::hours(8)).date_naive();
                            (d >= first(day)
                                || c.retain_day
                                    .and_then(date)
                                    .is_some_and(|d| d >= first(day) && d <= day))
                                && d <= day
                        })
            })
    }
    pub fn ambiguous(&self) -> bool {
        self.calls.iter().any(|c| c.through.is_some())
    }
    pub fn legacy(&self) -> Result<Self> {
        let mut result = Self::new(
            date(self.day).unwrap(),
            self.calls.iter().cloned().map(|mut c| {
                c.usage_time = false;
                c.retain_day = None;
                c
            }),
        )?;
        result.conflict = self.conflict;
        Ok(result)
    }
    pub(super) fn days(&self) -> Result<BTreeMap<NaiveDate, Day>> {
        ensure!(
            self.valid() && !self.conflict && !self.ambiguous(),
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
            through: None,
            usage_time: false,
            retain_day: None,
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
    fn equal_revision_counter_conflicts_remain_unknown() {
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
        ] {
            let a = Ledger::new(day(20261022), [original.clone()]).unwrap();
            let b = Ledger::new(day(20261022), [changed]).unwrap();
            assert!(Ledger::union(day(20261022), [&a, &b]).is_err());
            assert!(a.merge(&b).conflict);
        }
    }
    #[test]
    fn date_disagreements_only_withdraw_affected_periods() {
        for (today, other, expected) in [
            (20261125, 20261124, [None, Some(110), Some(110)]),
            (20261123, 20261122, [None, None, Some(110)]),
            (20261001, 20260930, [None, Some(110), None]),
        ] {
            let a = Ledger::new(day(today), [call("shared", today, 0, 100)]).unwrap();
            let b = Ledger::new(day(today), [call("shared", other, 0, 100)]).unwrap();
            for inputs in [[&a, &b], [&b, &a]] {
                let result = Ledger::periods(day(today), inputs).unwrap();
                assert_eq!(result.values.map(|v| v.map(Day::total)), expected);
            }
            // Original dates survive serialization and another import.
            let restored: Ledger =
                serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
            assert_eq!(
                Ledger::periods(day(today), [&a, &restored, &b])
                    .unwrap()
                    .values
                    .map(|v| v.map(Day::total)),
                expected
            );
        }
    }
    #[test]
    fn merged_date_spans_survive_reload_and_calendar_boundaries() {
        for (today, previous, expected) in [
            (20261125, 20261124, [None, Some(110), Some(110)]),
            (20261123, 20261122, [None, None, Some(110)]),
            (20261001, 20260930, [None, Some(110), None]),
            (20260601, 20260531, [None, None, None]),
        ] {
            let a = Ledger::new(day(previous), [call("shared", previous, 0, 100)]).unwrap();
            let b = Ledger::new(day(today), [call("shared", today, 0, 100)]).unwrap();
            let merged = a.merge(&b);
            assert!(merged.valid() && !merged.conflict && merged.ambiguous());
            assert_eq!(merged.version, 2);
            assert_eq!(merged, b.merge(&a));
            let restored: Ledger =
                serde_json::from_slice(&serde_json::to_vec(&merged).unwrap()).unwrap();
            assert_eq!(
                Ledger::periods(day(today), [&restored])
                    .unwrap()
                    .values
                    .map(|v| v.map(Day::total)),
                expected
            );
            assert_eq!(restored.merge(&a), restored.merge(&b));
        }
    }
    #[test]
    fn historical_date_or_counter_disagreements_do_not_hide_unrelated_periods() {
        let a = Ledger::new(day(20261126), [call("shared", 20261124, 0, 100)]).unwrap();
        let b = Ledger::new(day(20261126), [call("shared", 20261125, 0, 100)]).unwrap();
        let result = Ledger::periods(day(20261126), [&a, &b]).unwrap();
        assert_eq!(
            result.values.map(|v| v.map(Day::total)),
            [Some(0), Some(110), Some(110)]
        );
        assert_eq!(result.disputed, [false; 3]);
        let a = Ledger::new(day(20261126), [call("shared", 20261120, 0, 100)]).unwrap();
        let b = Ledger::new(day(20261126), [call("shared", 20261120, 0, 101)]).unwrap();
        let result = Ledger::periods(day(20261126), [&a, &b]).unwrap();
        assert_eq!(
            result.values.map(|v| v.map(Day::total)),
            [Some(0), Some(0), None]
        );
        let result =
            Ledger::periods(day(20261126), [&Ledger::new(day(20261125), []).unwrap()]).unwrap();
        assert_eq!(
            result.values.map(|v| v.map(Day::total)),
            [None, Some(0), Some(0)]
        );
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
        invalid.version = 4;
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
