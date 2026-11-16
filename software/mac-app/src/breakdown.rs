//! Bounded Argus aggregate labels/counts only; never provider settings or records.
use crate::usage::{OK, Packet};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;

const LIMIT: u64 = 9_007_199_254_740_991;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Breakdown {
    pub models: Vec<(String, u64)>,
    pub providers: Vec<(String, u64)>,
    pub model_count: usize,
    pub provider_count: usize,
}
fn label(raw: &str, provider: bool) -> String {
    if provider && uuid::Uuid::parse_str(raw).is_ok() {
        return format!(
            "ID {}",
            &uuid::Uuid::parse_str(raw).unwrap().simple().to_string()[..8]
        );
    }
    if raw.is_empty()
        || raw.len() > 160
        || raw.split('/').any(|part| part.starts_with("sk-"))
        || raw.starts_with("Bearer ")
        || !raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" ._-/".contains(&b))
    {
        return "Unknown".into();
    }
    if raw == "Other" {
        return if provider {
            "Other provider"
        } else {
            "Other model"
        }
        .into();
    }
    if raw.len() <= 32 {
        return raw.into();
    }
    let tail = raw.rsplit('/').next().unwrap_or(raw);
    if tail.len() <= 32 {
        tail.into()
    } else {
        format!("{}~", &tail[..31])
    }
}
fn group(
    report: &Value,
    key: &str,
    total: u64,
    provider: bool,
) -> Option<(Vec<(String, u64)>, usize)> {
    let rows = report[key].as_array()?;
    if rows.len() > 4096 {
        return None;
    }
    let mut parsed = Vec::new();
    let mut seen = HashSet::new();
    let mut sum = 0u64;
    for row in rows {
        let name = row["key"].as_str()?;
        let tokens = row["totals"]["total_tokens"]
            .as_u64()
            .filter(|n| *n <= LIMIT)?;
        sum = sum.checked_add(tokens)?;
        if !seen.insert(name) {
            return None;
        }
        parsed.push((name.to_owned(), tokens));
    }
    if sum != total {
        return None;
    }
    parsed.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let count = parsed.len();
    let mut ranked: Vec<_> = parsed
        .iter()
        .take(if count > 3 { 2 } else { 3 })
        .map(|(name, n)| (label(name, provider), *n))
        .collect();
    if count > 3 {
        ranked.push(("Other".into(), parsed.iter().skip(2).map(|(_, n)| n).sum()));
    }
    // Truncation/opaque IDs must not silently merge distinct categories.
    for i in 0..ranked.len() {
        if ranked[..i].iter().any(|(name, _)| name == &ranked[i].0) {
            let mut suffix = 1;
            loop {
                let candidate =
                    format!("{} {}", if provider { "Provider" } else { "Model" }, suffix);
                if !ranked[..i].iter().any(|(name, _)| name == &candidate) {
                    ranked[i].0 = candidate;
                    break;
                }
                suffix += 1;
            }
        }
    }
    Some((ranked, count))
}
impl Breakdown {
    pub fn from_report(report: &Value, total: u64) -> Self {
        let (models, model_count) = group(report, "by_model", total, false).unwrap_or_default();
        let (providers, provider_count) =
            group(report, "by_provider", total, true).unwrap_or_default();
        Self {
            models,
            providers,
            model_count,
            provider_count,
        }
    }
    pub fn wire(&self, packet: &Packet) -> Result<Vec<u8>> {
        packet.encode()?;
        ensure!(
            packet.p == 3 && packet.s == OK,
            "仅Argus有效日统计可附带分类汇总。"
        );
        let day = packet
            .d
            .filter(|d| d[1] >= 0)
            .ok_or_else(|| anyhow::anyhow!("分类统计缺少当天总数。"))?;
        for (rows, count) in [
            (&self.models, self.model_count),
            (&self.providers, self.provider_count),
        ] {
            ensure!(
                count <= 4096 && rows.len() == count.min(3),
                "分类条数无效。"
            );
            let mut sum = 0u64;
            let mut seen = HashSet::new();
            for (name, tokens) in rows {
                ensure!(
                    !name.is_empty()
                        && name.len() <= 32
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b" ._-/~".contains(&b))
                        && *tokens <= LIMIT,
                    "分类标签或计数无效。"
                );
                ensure!(seen.insert(name), "分类标签重复。");
                sum = sum
                    .checked_add(*tokens)
                    .ok_or_else(|| anyhow::anyhow!("分类计数溢出。"))?;
            }
            ensure!(
                rows.is_empty() || sum == day[1] as u64,
                "分类汇总和当天总数不一致。"
            );
            ensure!(
                count <= 3 || rows.last().is_some_and(|(name, _)| name == "Other"),
                "缺少其他分类合计。"
            );
        }
        let bytes = serde_json::to_vec(&json!({"v":1,"p":3,"t":packet.t,"d":day,
            "g":[self.model_count,self.provider_count],"m":self.models,"r":self.providers}))?;
        ensure!(bytes.len() <= 480, "分类快照过长。");
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(models: Vec<(&str, u64)>, providers: Vec<(&str, u64)>) -> Value {
        let rows = |items: Vec<(&str, u64)>| {
            items
                .into_iter()
                .map(|(key, n)| json!({"key":key,"totals":{"total_tokens":n}}))
                .collect::<Vec<_>>()
        };
        json!({"by_model":rows(models),"by_provider":rows(providers)})
    }
    fn packet(total: i64) -> Packet {
        let mut p = Packet::new(3, OK);
        p.t = 1792584000;
        p.n = Some("Argus".into());
        p.d = Some([20261021, total]);
        p
    }
    #[test]
    fn aggregate_wire_matches_the_shared_c_fixture() {
        let r = report(
            vec![("model-c", 10), ("model-a", 70), ("model-b", 40)],
            vec![
                ("22222222-2222-4222-8222-222222222222", 20),
                ("11111111-1111-4111-8111-111111111111", 100),
            ],
        );
        let b = Breakdown::from_report(&r, 120);
        let actual: Value = serde_json::from_slice(&b.wire(&packet(120)).unwrap()).unwrap();
        let expected: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/argus-breakdown-v1.json"))
                .unwrap();
        assert_eq!(actual, expected);
    }
    #[test]
    fn top_two_plus_other_preserves_total_and_unsafe_labels_are_not_forwarded() {
        let r = report(
            vec![("a", 1), ("b", 4), ("c", 2), ("d", 3)],
            vec![("https://private.example/key?secret", 10)],
        );
        let b = Breakdown::from_report(&r, 10);
        assert_eq!(
            b.models,
            vec![("b".into(), 4), ("d".into(), 3), ("Other".into(), 3)]
        );
        assert_eq!(b.model_count, 4);
        assert_eq!(b.providers[0].0, "Unknown");
        let bytes = b.wire(&packet(10)).unwrap();
        assert!(
            !String::from_utf8(bytes)
                .unwrap()
                .contains("private.example")
        );
        let mut invalid = r;
        invalid["by_model"][0]["totals"]["total_tokens"] = json!(100);
        let b = Breakdown::from_report(&invalid, 10);
        assert!(b.models.is_empty());
        assert_eq!(b.providers[0].1, 10);
    }
    #[test]
    fn worst_case_payload_is_bounded_and_bad_counts_are_rejected() {
        let total = LIMIT;
        let mut b = Breakdown {
            models: vec![
                ("a".repeat(32), total - 2),
                ("b".repeat(32), 1),
                ("c".repeat(32), 1),
            ],
            providers: vec![
                ("d".repeat(32), total - 2),
                ("e".repeat(32), 1),
                ("f".repeat(32), 1),
            ],
            model_count: 3,
            provider_count: 3,
        };
        assert!(b.wire(&packet(total as i64)).unwrap().len() <= 480);
        b.providers[0].1 += 1;
        assert!(b.wire(&packet(total as i64)).is_err());
        assert!(Breakdown::default().wire(&packet(0)).is_ok());
        let duplicate = report(vec![("a", 3), ("a", 4)], vec![("p", 7)]);
        assert!(Breakdown::from_report(&duplicate, 7).models.is_empty());
    }
}
