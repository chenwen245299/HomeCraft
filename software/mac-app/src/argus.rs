//! Argus' explicit, read-only MCP tool. No library database or provider config reads.
use crate::usage::{OK, Reading, empty};
use anyhow::{Result, bail, ensure};
use chrono::{Datelike, NaiveDate};
use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

const LIMIT: i64 = 9_007_199_254_740_991;
fn integer(v: &Value, key: &str) -> Result<i64> {
    v[key]
        .as_i64()
        .filter(|n| (0..=LIMIT).contains(n))
        .ok_or_else(|| anyhow::anyhow!("Argus 汇总缺少有效 {key}。"))
}
fn week_start(day: NaiveDate) -> NaiveDate {
    day - chrono::Duration::days(i64::from(day.weekday().num_days_from_monday()))
}
fn attach_week(r: &mut Reading, report: &Value, monday: NaiveDate) -> Result<()> {
    let s = &report["summary"];
    let input = integer(s, "input_tokens")?;
    let output = integer(s, "output_tokens")?;
    let total = integer(s, "total_tokens")?;
    ensure!(
        input.checked_add(output) == Some(total)
            && integer(s, "cache_hit_tokens")? <= input
            && total >= r.packet.d.map(|d| d[1]).unwrap_or(0),
        "Argus 本周汇总计数不一致。"
    );
    r.packet.wt = Some([
        i64::from(monday.year()) * 10000
            + i64::from(monday.month()) * 100
            + i64::from(monday.day()),
        total,
    ]);
    r.tokens.weekly = Some(total as u64);
    r.tokens.week_start = Some(monday.to_string());
    r.packet.encode()?;
    Ok(())
}
fn attach_month(r: &mut Reading, report: &Value, first: NaiveDate) -> Result<()> {
    let s = &report["summary"];
    let input = integer(s, "input_tokens")?;
    let output = integer(s, "output_tokens")?;
    let total = integer(s, "total_tokens")?;
    ensure!(
        input.checked_add(output) == Some(total)
            && integer(s, "cache_hit_tokens")? <= input
            && total >= r.packet.d.map(|d| d[1]).unwrap_or(0),
        "Argus 本月汇总计数不一致。"
    );
    r.packet.mt = Some([
        i64::from(first.year()) * 10000 + i64::from(first.month()) * 100 + 1,
        total,
    ]);
    r.tokens.monthly = Some(total as u64);
    r.packet.encode()?;
    Ok(())
}

fn checked_report(result: &Value, start: &str, end: &str) -> Result<Value> {
    ensure!(
        result["isError"] != true,
        "Argus 用量工具返回错误，请检查 MCP 开关和当前文献库。"
    );
    let report = &result["structuredContent"];
    for (key, wanted) in [("start_time", start), ("end_time", end)] {
        let actual = report["query"][key]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
        let wanted = chrono::DateTime::parse_from_rfc3339(wanted)?;
        ensure!(actual == Some(wanted), "Argus 返回的日期范围不匹配。");
    }
    Ok(report.clone())
}
pub fn from_report(report: &Value, day: NaiveDate) -> Result<Reading> {
    let s = &report["summary"];
    let input = integer(s, "input_tokens")?;
    let output = integer(s, "output_tokens")?;
    let total = integer(s, "total_tokens")?;
    let cache = integer(s, "cache_hit_tokens")?;
    let calls = integer(s, "records")?;
    let missing = integer(s, "records_without_cost")?;
    ensure!(
        input.checked_add(output) == Some(total) && cache <= input && missing <= calls,
        "Argus 汇总计数不一致。"
    );
    let cost = if s["reported_cost_usd"].is_null() {
        None
    } else {
        Some(
            s["reported_cost_usd"]
                .as_f64()
                .filter(|n| n.is_finite() && (0.0..=1e12).contains(n))
                .ok_or_else(|| anyhow::anyhow!("Argus 记录费用无效。"))?,
        )
    };
    let mut r = empty(3, OK, "Argus 官方本机 MCP · 当前文献库");
    r.packet.n = Some("Argus".into());
    r.packet.d = Some([
        i64::from(day.year()) * 10000 + i64::from(day.month()) * 100 + i64::from(day.day()),
        total,
    ]);
    r.packet.u = Some([input, output, cache, calls, missing]);
    r.packet.dc = cost;
    r.tokens.recent_day = Some(total as u64);
    r.tokens.day = Some(day.to_string());
    r.tokens.input = Some(input as u64);
    r.tokens.output = Some(output as u64);
    r.tokens.cached = Some(cache as u64);
    r.tokens.calls = Some(calls as u64);
    r.tokens.cost_usd = cost;
    r.tokens.models = report["by_model"].as_array().map(Vec::len);
    r.tokens.breakdown = Some(crate::breakdown::Breakdown::from_report(
        report,
        total as u64,
    ));
    if let Some(items) = report["by_provider"].as_array() {
        for item in items.iter().take(32) {
            if let (Some(name), Some(n)) = (
                item["key"].as_str(),
                item["totals"]["total_tokens"].as_u64(),
            ) {
                // Display only bounded labels, never endpoints, settings, or individual records.
                if name.len() <= 80 && !name.chars().any(char::is_control) {
                    r.tokens.providers.push((name.into(), n));
                }
            }
        }
    }
    r.tokens.note = if missing > 0 {
        format!("当前文献库当天日志；{missing} 次调用未记录费用，缓存已含在输入中")
    } else {
        "当前文献库当天日志；记录费用非账户账单，缓存已含在输入中".into()
    };
    r.packet.encode()?;
    Ok(r)
}

async fn request(
    input: &mut tokio::process::ChildStdin,
    output: &mut BufReader<tokio::process::ChildStdout>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value> {
    let line =
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
    input.write_all(&line).await?;
    input.write_all(b"\n").await?;
    input.flush().await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let mut line = Vec::new();
            let count = (&mut *output)
                .take(2_097_153)
                .read_until(b'\n', &mut line)
                .await?;
            ensure!(
                count > 0 && count <= 2_097_152,
                "Argus MCP 未响应，请检查设置中的 MCP 开关与程序版本。"
            );
            let v: Value = serde_json::from_slice(&line)?;
            if v["id"].as_u64() != Some(id) {
                continue;
            }
            if v.get("error").is_some() {
                bail!("Argus MCP 不支持该调用，请更新含 get_token_usage 的版本。");
            }
            return Ok(v["result"].clone());
        }
    })
    .await?
}

async fn query(root: &Path) -> Result<Reading> {
    let settings =
        crate::config::read_private::<Value>(&root.join(".ble-settings.json")).unwrap_or(json!({}));
    let executable = std::env::var("EPAPER_ARGUS")
        .ok()
        .or_else(|| settings["argus_executable"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "/Applications/Argus.app/Contents/MacOS/Argus".into());
    ensure!(
        Path::new(&executable).is_absolute(),
        "Argus 可执行文件必须为绝对路径。"
    );
    let mut child = Command::new(executable)
        .arg("--mcp-stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let init=request(&mut input,&mut output,1,"initialize",json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"homecraft","version":env!("CARGO_PKG_VERSION")}})).await?;
    ensure!(
        ["2025-06-18", "2024-11-05", "2025-03-26"]
            .contains(&init["protocolVersion"].as_str().unwrap_or("")),
        "Argus MCP 协议版本不支持。"
    );
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await?;
    input.flush().await?;
    let tools = request(&mut input, &mut output, 2, "tools/list", json!({})).await?;
    ensure!(
        tools["tools"]
            .as_array()
            .is_some_and(|a| a.iter().any(|t| t["name"] == "get_token_usage")),
        "Argus 版本缺少 get_token_usage，请更新 Argus。"
    );
    let day = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
        .date_naive();
    let next = day.succ_opt().unwrap();
    let start = format!("{day}T00:00:00+08:00");
    let end = format!("{next}T00:00:00+08:00");
    let result=request(&mut input,&mut output,3,"tools/call",json!({"name":"get_token_usage","arguments":{"start_time":start,"end_time":end,"include_records":false,"granularity":"none","timezone_offset_minutes":480}})).await?;
    // Require the server to confirm each range; never silently show all history.
    let report = checked_report(&result, &start, &end)?;
    let mut r = from_report(&report, day)?;
    let monday = week_start(day);
    let week_begin = format!("{monday}T00:00:00+08:00");
    // A missing/slow weekly report must not discard a valid daily reading.
    let weekly = tokio::time::timeout(Duration::from_secs(5), async {
        let result = request(
            &mut input,
            &mut output,
            4,
            "tools/call",
            json!({
                "name":"get_token_usage", "arguments":{
                    "start_time":week_begin,"end_time":end,"include_records":false,
                    "granularity":"none","timezone_offset_minutes":480
                }
            }),
        )
        .await?;
        let report = checked_report(&result, &week_begin, &end)?;
        attach_week(&mut r, &report, monday)
    })
    .await;
    if !matches!(weekly, Ok(Ok(()))) {
        r.packet.wt = None;
        r.tokens.weekly = None;
        r.tokens.week_start = None;
        r.tokens.note.push_str("；本周统计暂不可用");
    }
    let first = day.with_day(1).unwrap();
    let month_begin = format!("{first}T00:00:00+08:00");
    let monthly=tokio::time::timeout(Duration::from_secs(5),async {
        let result=request(&mut input,&mut output,5,"tools/call",json!({"name":"get_token_usage","arguments":{
            "start_time":month_begin,"end_time":end,"include_records":false,"granularity":"none","timezone_offset_minutes":480
        }})).await?;
        let report=checked_report(&result,&month_begin,&end)?;
        attach_month(&mut r,&report,first)
    }).await;
    if !matches!(monthly, Ok(Ok(()))) {
        r.packet.mt = None;
        r.tokens.monthly = None;
        r.tokens.note.push_str("；本月统计暂不可用");
    }
    drop(input);
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    Ok(r)
}
pub async fn collect(root: &Path) -> Reading {
    let result = tokio::time::timeout(Duration::from_secs(40), query(root)).await;
    match result {
        Ok(Ok(r)) => r,
        other => {
            let note = match other {
                Ok(Err(e)) => e.to_string(),
                _ => "Argus MCP 查询超时".into(),
            };
            let mut r = empty(3, 6, &note);
            r.packet.n = Some("Argus".into());
            r
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monday_range_and_optional_week_keep_daily_scope() {
        for (day, expected) in [
            ("2026-10-19", "2026-10-19"),
            ("2026-10-25", "2026-10-19"),
            ("2026-10-26", "2026-10-26"),
            ("2025-01-01", "2024-12-30"),
            ("2024-03-01", "2024-02-26"),
        ] {
            let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap();
            assert_eq!(week_start(date).to_string(), expected);
        }
        let day = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let daily = json!({"summary":{"records":0,"input_tokens":0,"output_tokens":0,"total_tokens":0,
            "cache_hit_tokens":0,"reported_cost_usd":null,"records_without_cost":0}});
        let mut r = from_report(&daily, day).unwrap();
        let week = json!({"summary":{"input_tokens":100,"output_tokens":20,"total_tokens":120,"cache_hit_tokens":60}});
        attach_week(&mut r, &week, week_start(day)).unwrap();
        assert_eq!(r.packet.d, Some([20261021, 0]));
        assert_eq!(r.packet.wt, Some([20261019, 120]));
        assert_eq!(r.tokens.weekly, Some(120));
        assert_eq!(r.packet.w, [[-1, 0, 0]; 3]);
        let start = "2026-10-19T00:00:00+08:00";
        let end = "2026-10-22T00:00:00+08:00";
        let mut reply = json!({"structuredContent":{"query":{"start_time":"2026-10-18T16:00:00Z","end_time":"2026-10-21T16:00:00Z"}}});
        assert!(checked_report(&reply, start, end).is_ok());
        reply["structuredContent"]["query"]["start_time"] = json!("2026-10-20T16:00:00Z");
        assert!(checked_report(&reply, start, end).is_err()); // A day report is not a week report.
        reply["isError"] = json!(true);
        assert!(checked_report(&reply, start, end).is_err());
        let invalid = json!({"summary":{"input_tokens":100,"output_tokens":20,"total_tokens":180,"cache_hit_tokens":60}});
        assert!(attach_week(&mut r, &invalid, week_start(day)).is_err());
        assert_eq!(r.packet.d, Some([20261021, 0])); // A broken week does not invalidate today.
    }
    #[test]
    fn scope_and_cache_are_not_counted_twice() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let v = json!({"summary":{"records":3,"input_tokens":100,"output_tokens":20,"total_tokens":120,"cache_hit_tokens":60,"reported_cost_usd":null,"records_without_cost":3},"by_provider":[{"key":"deepseek","totals":{"total_tokens":120}}],"by_model":[]});
        let r = from_report(&v, day).unwrap();
        assert_eq!(r.packet.d, Some([20261020, 120]));
        assert!(r.packet.dc.is_none());
        assert_eq!(r.tokens.cached, Some(60));
        assert_eq!(r.packet.w, [[-1, 0, 0]; 3]);
        let mut invalid = v.clone();
        invalid["summary"]["total_tokens"] = json!(180);
        assert!(from_report(&invalid, day).is_err());
        let mut zero = v;
        for key in [
            "records",
            "input_tokens",
            "output_tokens",
            "total_tokens",
            "cache_hit_tokens",
            "records_without_cost",
        ] {
            zero["summary"][key] = json!(0);
        }
        zero["summary"]["reported_cost_usd"] = json!(0.0);
        let r = from_report(&zero, day).unwrap();
        assert_eq!(r.packet.d, Some([20261020, 0]));
        assert_eq!(r.packet.dc, Some(0.0));
    }
    #[test]
    fn month_is_independent_of_week_across_a_month_boundary() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let report = json!({"summary":{"records":0,"input_tokens":0,"output_tokens":0,"total_tokens":0,"cache_hit_tokens":0,"reported_cost_usd":null,"records_without_cost":0}});
        let mut r = from_report(&report, day).unwrap();
        attach_week(&mut r,&json!({"summary":{"input_tokens":100,"output_tokens":20,"total_tokens":120,"cache_hit_tokens":60}}),week_start(day)).unwrap();
        attach_month(&mut r,&json!({"summary":{"input_tokens":50,"output_tokens":10,"total_tokens":60,"cache_hit_tokens":30}}),day.with_day(1).unwrap()).unwrap();
        assert_eq!(r.packet.mt, Some([20261001, 60]));
        assert_eq!(r.packet.wt.unwrap()[1], 120);
        assert!(attach_month(&mut r,&json!({"summary":{"input_tokens":50,"output_tokens":10,"total_tokens":90,"cache_hit_tokens":30}}),day.with_day(1).unwrap()).is_err());
        assert_eq!(r.packet.d.unwrap()[1], 0);
    }
}
