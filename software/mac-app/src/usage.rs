use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    time::timeout,
};

pub const OK: u8 = 2;
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub v: u8,
    pub p: u8,
    pub t: i64,
    pub s: u8,
    pub w: [[i64; 3]; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mu: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ml: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<[i64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ld: Option<[i64; 2]>,
    // Monday date and tokens used this calendar week; separate from quota windows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wt: Option<[i64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mt: Option<[i64; 2]>,
    // Yesterday's official tokens and last observed weekly quota used (permille).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yd: Option<[i64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<String>,
    // Daily input, output, cached input, logged calls, calls without recorded cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub u: Option<[i64; 5]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dc: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<crate::harness::Origin>,
}
impl Packet {
    pub fn wire_for_device(
        &self,
        daily_supported: bool,
        profiles_supported: bool,
        weekly_supported: bool,
        monthly_supported: bool,
        yesterday_supported: bool,
    ) -> Result<Vec<u8>> {
        let mut wire = self.clone();
        // Origin is a separately negotiated extension, never implied by profiles.
        wire.o = None;
        if !daily_supported {
            wire.d = None;
            wire.ld = None;
        }
        if !profiles_supported {
            wire.n = None;
            wire.u = None;
            wire.dc = None;
        }
        if !weekly_supported {
            wire.wt = None;
        }
        if !monthly_supported {
            wire.mt = None;
        }
        if !yesterday_supported {
            wire.yd = None;
        }
        wire.encode()
    }
    pub fn new(p: u8, s: u8) -> Self {
        Self {
            v: 1,
            p,
            t: now(),
            s,
            w: [[-1, 0, 0]; 3],
            b: None,
            c: None,
            mu: None,
            ml: None,
            d: None,
            ld: None,
            wt: None,
            mt: None,
            yd: None,
            n: None,
            u: None,
            dc: None,
            o: None,
        }
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        if let Some(origin) = &self.o {
            ensure!(
                origin.valid()
                    && self.p == crate::harness::PROVIDER
                    && self.n.as_deref() == Some(crate::harness::NAME),
                "Harness 来源无效"
            );
        }
        ensure!(
            self.v == 1
                && self.p <= 7
                && self.s <= 7
                && (1735689600..=4102444800).contains(&self.t),
            "快照标头无效。"
        );
        if let Some(name) = &self.n {
            ensure!(
                !name.is_empty()
                    && name.len() <= 24
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b" ._-".contains(&b)),
                "服务名称无效。"
            );
            ensure!(
                self.p >= 3 && (self.p != 3 || name == "Argus"),
                "服务身份无效。"
            );
            ensure!(
                self.p < 4
                    || !["Codex", "Claude Code", "DeepSeek", "Argus"].contains(&name.as_str()),
                "服务名称重复。"
            );
        }
        ensure!(self.p < 3 || self.n.is_some(), "缺少服务名称。");
        if let Some(u) = self.u {
            ensure!(
                u.iter().all(|n| (0..=9_007_199_254_740_991).contains(n))
                    && u[2] <= u[0]
                    && u[4] <= u[3],
                "用量计数无效。"
            );
            ensure!(
                self.d
                    .is_some_and(|d| d[1] >= 0 && u[0].checked_add(u[1]) == Some(d[1])),
                "用量总数不一致。"
            );
        }
        if let Some(cost) = self.dc {
            ensure!(
                self.u.is_some() && cost.is_finite() && (0.0..=1e12).contains(&cost),
                "记录费用无效。"
            );
        }
        for w in self.w {
            ensure!(
                (-1..=1000).contains(&w[0])
                    && (0..=525600).contains(&w[1])
                    && (0..=4102444800).contains(&w[2]),
                "快照窗口无效。"
            );
        }
        for n in [self.b, self.mu, self.ml].into_iter().flatten() {
            ensure!(n.is_finite() && n.abs() <= 1e12, "金额无效。");
        }
        for daily in [self.d, self.ld].into_iter().flatten() {
            let day = daily[0];
            ensure!(
                (20250101..=20991231).contains(&day)
                    && chrono::NaiveDate::from_ymd_opt(
                        (day / 10000) as i32,
                        (day / 100 % 100) as u32,
                        (day % 100) as u32
                    )
                    .is_some()
                    && (-1..=9_007_199_254_740_991).contains(&daily[1]),
                "日统计字段无效。"
            );
        }
        if let Some([day, tokens, used]) = self.yd {
            ensure!(
                self.p == 0
                    && (20241231..=20991231).contains(&day)
                    && chrono::NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d").is_ok()
                    && (-1..=9_007_199_254_740_991).contains(&tokens)
                    && (-1..=1000).contains(&used),
                "昨日统计字段无效。"
            );
        }
        if let Some([day, total]) = self.wt {
            use chrono::Datelike;
            let date = chrono::NaiveDate::from_ymd_opt(
                (day / 10000) as i32,
                (day / 100 % 100) as u32,
                (day % 100) as u32,
            );
            ensure!(
                (20241230..=20991231).contains(&day)
                    && date.is_some_and(|d| d.weekday() == chrono::Weekday::Mon)
                    && (0..=9_007_199_254_740_991).contains(&total),
                "周 Token 字段无效。"
            );
        }
        if let Some([day, total]) = self.mt {
            ensure!(
                (20250101..=20991201).contains(&day)
                    && day % 100 == 1
                    && chrono::NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d").is_ok()
                    && (0..=9_007_199_254_740_991).contains(&total),
                "月 Token 字段无效。"
            );
        }
        let bytes = serde_json::to_vec(self)?;
        ensure!(bytes.len() <= 480, "蓝牙快照过长。");
        Ok(bytes)
    }
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct TokenStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<crate::account::Identity>,
    #[serde(default)]
    pub devices: Vec<crate::harness::DeviceUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_ledger: Option<crate::harness::Ledger>,
    #[serde(default)]
    pub breakdown: Option<crate::breakdown::Breakdown>,
    #[serde(default)]
    pub weekly: Option<u64>,
    #[serde(default)]
    pub monthly: Option<u64>,
    #[serde(default)]
    pub week_start: Option<String>,
    pub lifetime: Option<u64>,
    pub peak_daily: Option<u64>,
    pub recent_day: Option<u64>,
    pub day: Option<String>,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub context_size: Option<u64>,
    pub note: String,
    #[serde(default)]
    pub cached: Option<u64>,
    #[serde(default)]
    pub calls: Option<u64>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub models: Option<usize>,
    #[serde(default)]
    pub providers: Vec<(String, u64)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reading {
    pub packet: Packet,
    pub tokens: TokenStats,
    pub source: String,
}
pub fn empty(p: u8, s: u8, note: &str) -> Reading {
    Reading {
        packet: Packet::new(p, s),
        tokens: TokenStats {
            note: note.into(),
            ..Default::default()
        },
        source: note.into(),
    }
}

struct Rpc {
    _child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}
impl Rpc {
    async fn start(root: &Path) -> Result<Self> {
        let launch = crate::codex_cli::resolve(root, None, false).await?;
        let mut child = launch
            .command()
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("Codex文件已找到，但无法启动；请在设置中检测CLI与Node.js。")?;
        let mut rpc = Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            _child: child,
            id: 0,
        };
        rpc.request("initialize",Some(json!({"clientInfo":{"name":"homecraft","title":"HomeCraft","version":env!("CARGO_PKG_VERSION")}}))).await?;
        rpc.send(json!({"method":"initialized","params":{}}))
            .await?;
        Ok(rpc)
    }
    async fn send(&mut self, value: Value) -> Result<()> {
        let mut b = serde_json::to_vec(&value)?;
        b.push(b'\n');
        self.input.write_all(&b).await?;
        self.input.flush().await?;
        Ok(())
    }
    async fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        self.id += 1;
        let id = self.id;
        let mut req = json!({"id":id,"method":method});
        if let Some(p) = params {
            req["params"] = p;
        }
        self.send(req).await?;
        timeout(Duration::from_secs(30), async {
            loop {
                let mut line = Vec::new();
                let count = (&mut self.output)
                    .take(1_048_577)
                    .read_until(b'\n', &mut line)
                    .await?;
                ensure!(
                    count > 0 && count <= 1_048_576,
                    "官方客户端连接关闭或回复过长。"
                );
                let reply: Value = serde_json::from_slice(&line)?;
                if reply.get("id").and_then(Value::as_u64) != Some(id) {
                    continue;
                }
                if reply.get("error").is_some() {
                    bail!(
                        "官方接口 {method} 不可用（代码 {}）。",
                        reply["error"]["code"]
                    );
                }
                return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
            }
        })
        .await?
    }
}
fn window(value: &Value, minutes: Option<i64>) -> [i64; 3] {
    let used = value
        .get("usedPercent")
        .or_else(|| value.get("used_percentage"))
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite());
    let mins = minutes
        .or_else(|| value["windowDurationMins"].as_i64())
        .filter(|n| (1..=525600).contains(n))
        .unwrap_or(0);
    let reset = value
        .get("resetsAt")
        .or_else(|| value.get("resets_at"))
        .and_then(Value::as_i64)
        .filter(|n| (1..=4102444800).contains(n))
        .unwrap_or(0);
    [
        used.map(|u| ((100. - u).clamp(0., 100.) * 10.).round() as i64)
            .unwrap_or(-1),
        mins,
        reset,
    ]
}
fn put(p: &mut Packet, w: [i64; 3]) {
    let i = if (38880..=46080).contains(&w[1]) {
        2
    } else if (7200..=11520).contains(&w[1]) {
        1
    } else {
        0
    };
    p.w[i] = w;
}
pub fn codex_packet(value: &Value) -> Result<Packet> {
    let b = value
        .pointer("/rateLimitsByLimitId/codex")
        .filter(|v| v.is_object())
        .or_else(|| value.get("rateLimits").filter(|v| v.is_object()))
        .ok_or_else(|| anyhow::anyhow!("官方未返回 Codex 额度，请用 ChatGPT 登录官方 Codex。"))?;
    let mut p = Packet::new(0, OK);
    for n in ["primary", "secondary"] {
        if b[n].is_object() {
            put(&mut p, window(&b[n], None));
        }
    }
    Ok(p)
}
async fn codex(root: &Path) -> Reading {
    let mut rpc = match Rpc::start(root).await {
        Ok(r) => r,
        Err(e) => return empty(0, 5, &format!("Codex启动失败：{e}")),
    };
    let before = rpc
        .request("account/read", Some(json!({"refreshToken":false})))
        .await
        .ok();
    let identity = crate::account::Identity::from_official(before.as_ref(), root);
    let limits = rpc.request("account/rateLimits/read", None).await;
    let mut p = match limits.and_then(|v| codex_packet(&v)) {
        Ok(p) => p,
        Err(_) => return empty(0, 5, "官方额度读取失败；请检查 Codex 登录"),
    };
    let mut tokens = match rpc.request("account/usage/read", None).await {
        Ok(v) => {
            let today = chrono::Utc::now()
                .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
                .date_naive();
            let daily = official_daily(&v, today);
            p.d = Some(daily.0);
            p.ld = daily.1;
            p.wt = official_weekly(&v, today);
            p.mt = official_monthly(&v, today);
            p.yd = Some(official_yesterday(&v, today));
            let latest = v["dailyUsageBuckets"].as_array().and_then(|a| {
                a.iter()
                    .filter(|d| {
                        d["startDate"].as_str().is_some_and(|s| {
                            s.len() == 10 && s.bytes().all(|b| b.is_ascii_digit() || b == b'-')
                        })
                    })
                    .max_by_key(|d| d["startDate"].as_str().unwrap())
            });
            TokenStats {
                weekly: p.wt.map(|w| w[1] as u64),
                monthly: p.mt.map(|m| m[1] as u64),
                week_start: p
                    .wt
                    .and_then(|w| {
                        chrono::NaiveDate::parse_from_str(&w[0].to_string(), "%Y%m%d").ok()
                    })
                    .map(|d| d.to_string()),
                lifetime: v["summary"]["lifetimeTokens"].as_u64(),
                peak_daily: v["summary"]["peakDailyTokens"].as_u64(),
                recent_day: latest.and_then(|d| d["tokens"].as_u64()),
                day: latest
                    .and_then(|d| d["startDate"].as_str())
                    .map(str::to_owned),
                note: "官方账户 Token 统计；本周合计已返回的本周日统计，缺失日期不按 0 计算".into(),
                ..Default::default()
            }
        }
        Err(_) => TokenStats {
            note: "本机官方接口未提供 Token 统计，额度查询仍可用".into(),
            ..Default::default()
        },
    };
    // A login change during the two reads must never attach another account's usage.
    let after = rpc
        .request("account/read", Some(json!({"refreshToken":false})))
        .await
        .ok();
    let final_identity = crate::account::Identity::from_official(after.as_ref(), root);
    if identity != final_identity {
        return empty(0, 5, "Codex 账号已切换，请重新刷新");
    }
    if p.yd.is_none() {
        let yesterday = (chrono::Utc::now() + chrono::Duration::hours(8))
            .date_naive()
            .pred_opt()
            .unwrap();
        p.yd = Some([day_number(yesterday), -1, -1]);
    }
    if let Some(yesterday) = p.yd.as_mut() {
        yesterday[2] = identity
            .as_ref()
            .and_then(|id| crate::cloud::codex_historical_week_used(root, id, yesterday[0]))
            .unwrap_or(-1);
    }
    tokens.account = identity;
    Reading {
        packet: p,
        tokens,
        source: "官方 Codex app-server".into(),
    }
}

// Already account-wide: never add lifetime/context counters or re-add per-thread totals.
// Match official date labels exactly to the display's UTC+8 calendar date.
fn day_number(d: chrono::NaiveDate) -> i64 {
    use chrono::Datelike;
    i64::from(d.year()) * 10000 + i64::from(d.month()) * 100 + i64::from(d.day())
}
fn official_buckets(v: &Value) -> std::collections::BTreeMap<chrono::NaiveDate, i64> {
    let mut buckets = std::collections::BTreeMap::<chrono::NaiveDate, i64>::new();
    if let Some(rows) = v["dailyUsageBuckets"].as_array() {
        for row in rows {
            let Some(date) = row["startDate"]
                .as_str()
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            else {
                continue;
            };
            let Some(tokens) = row["tokens"]
                .as_i64()
                .filter(|n| *n >= 0 && *n <= 9_007_199_254_740_991)
            else {
                continue;
            };
            // This official API exposes one total per date. Duplicate snapshots are
            // not disjoint sessions: use the maximum, never double the same date.
            buckets
                .entry(date)
                .and_modify(|n| *n = (*n).max(tokens))
                .or_insert(tokens);
        }
    }
    buckets
}
fn official_daily(v: &Value, today: chrono::NaiveDate) -> ([i64; 2], Option<[i64; 2]>) {
    let buckets = official_buckets(v);
    let current = [
        day_number(today),
        buckets.get(&today).copied().unwrap_or(-1),
    ];
    let latest = buckets
        .into_iter()
        .rfind(|(d, _)| *d <= today)
        .map(|(d, n)| [day_number(d), n]);
    (current, latest)
}
fn official_yesterday(v: &Value, today: chrono::NaiveDate) -> [i64; 3] {
    let day = today.pred_opt().unwrap();
    [
        day_number(day),
        official_buckets(v).get(&day).copied().unwrap_or(-1),
        -1,
    ]
}
fn official_weekly(v: &Value, today: chrono::NaiveDate) -> Option<[i64; 2]> {
    use chrono::Datelike;
    let monday = today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let buckets = official_buckets(v);
    let mut values = buckets.range(monday..=today).map(|(_, n)| *n);
    let first = values.next()?;
    let total = values.try_fold(first, |sum, n| sum.checked_add(n))?;
    (total <= 9_007_199_254_740_991).then_some([day_number(monday), total])
}
fn official_monthly(v: &Value, today: chrono::NaiveDate) -> Option<[i64; 2]> {
    use chrono::Datelike;
    let first = today.with_day(1)?;
    let buckets = official_buckets(v);
    let mut values = buckets.range(first..=today).map(|(_, n)| *n);
    let initial = values.next()?;
    let total = values.try_fold(initial, |sum, n| sum.checked_add(n))?;
    (total <= 9_007_199_254_740_991).then_some([day_number(first), total])
}

pub fn claude_from_statusline(v: &Value) -> Reading {
    let mut r = empty(1, OK, "官方 Claude Code statusLine");
    for (name, mins) in [("five_hour", 300), ("seven_day", 10080)] {
        if v["rate_limits"][name].is_object() {
            put(&mut r.packet, window(&v["rate_limits"][name], Some(mins)));
        }
    }
    r.tokens = TokenStats {
        input: v["context_window"]["total_input_tokens"].as_u64(),
        output: v["context_window"]["total_output_tokens"].as_u64(),
        context_size: v["context_window"]["context_window_size"].as_u64(),
        note: "当前上下文 Token，含缓存；不是账户累计消费".into(),
        ..Default::default()
    };
    r
}
fn claude(root: &Path) -> Reading {
    let path = root.join(".claude-statusline.json");
    let Ok(mut r) = crate::config::read_private::<Reading>(&path) else {
        return empty(1, 0, "等待 Claude Code 官方状态栏数据");
    };
    if r.packet.p != 1 || r.packet.encode().is_err() {
        return empty(1, 6, "Claude 状态栏数据无效");
    }
    // Expired windows must not survive merely because the desktop keeps running.
    for w in &mut r.packet.w {
        if w[2] > 0 && w[2] <= now() {
            *w = [-1, 0, 0];
        }
    }
    if now() - r.packet.t > 900 {
        r.source = "Claude 官方状态栏旧数据，等待客户端更新".into();
    }
    r
}
async fn deepseek(root: &Path) -> Reading {
    let config =
        crate::config::read_private::<Value>(&root.join(".ble-settings.json")).unwrap_or(json!({}));
    let key = std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .or_else(|| config["deepseek_api_key"].as_str().map(str::to_owned));
    let Some(key) = key.filter(|k| !k.is_empty()) else {
        return empty(2, 0, "DeepSeek 官方 API 未配置");
    };
    let result: Result<Reading> = async {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()?;
        let response = client
            .get("https://api.deepseek.com/user/balance")
            .bearer_auth(&key)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Ok(empty(
                2,
                match status.as_u16() {
                    401 => 3,
                    403 => 7,
                    429 => 4,
                    _ => 5,
                },
                "DeepSeek 官方 API 查询失败",
            ));
        }
        ensure!(
            response.content_length().unwrap_or(0) <= 32768,
            "官方回复过长。"
        );
        let mut response = response;
        let mut body = Vec::new();
        while let Some(c) = response.chunk().await? {
            ensure!(body.len() + c.len() <= 32768, "官方回复过长。");
            body.extend_from_slice(&c);
        }
        let v: Value = serde_json::from_slice(&body)?;
        let a = v["balance_infos"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("缺少余额。"))?;
        let b = a
            .iter()
            .find(|b| b["currency"] == "CNY")
            .or_else(|| a.iter().find(|b| b["currency"] == "USD"))
            .ok_or_else(|| anyhow::anyhow!("缺少币种。"))?;
        let balance = b["total_balance"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| b["total_balance"].as_f64());
        ensure!(
            balance.is_some_and(|n| n.is_finite() && n.abs() <= 1e12),
            "余额字段无效。"
        );
        let mut r = empty(2, OK, "DeepSeek 官方余额接口");
        r.packet.b = balance;
        r.packet.c = b["currency"].as_str().map(str::to_owned);
        Ok(r)
    }
    .await;
    result.unwrap_or_else(|_| empty(2, 5, "DeepSeek 官方 API 暂不可用"))
}
type CodexCache = Option<(std::time::Instant, [std::ffi::OsString; 4], Reading)>;

async fn cached_codex(
    cache: &tokio::sync::Mutex<CodexCache>,
    key: [std::ffi::OsString; 4],
    force: bool,
    query: impl std::future::Future<Output = Reading>,
) -> Reading {
    let mut cache = cache.lock().await;
    if !force
        && let Some((t, saved_key, r)) = &*cache
        && saved_key == &key
        && t.elapsed() < Duration::from_secs(30)
    {
        return r.clone();
    }
    let r = timeout(Duration::from_secs(75), query)
        .await
        .unwrap_or_else(|_| empty(0, 5, "Codex 读取超时，请检查网络后重试。"));
    *cache = Some((std::time::Instant::now(), key, r.clone()));
    r
}

/// Explicit refresh bypasses both successful readings and cached startup errors.
pub async fn collect_codex(root: &Path, force: bool) -> Reading {
    static CODEX: std::sync::OnceLock<tokio::sync::Mutex<CodexCache>> = std::sync::OnceLock::new();
    cached_codex(
        CODEX.get_or_init(|| tokio::sync::Mutex::new(None)),
        crate::codex_cli::cache_key(root),
        force,
        codex(root),
    )
    .await
}

pub async fn collect(root: &Path) -> Vec<Reading> {
    use std::{sync::OnceLock, time::Instant};
    static DEEPSEEK: OnceLock<tokio::sync::Mutex<Option<(Instant, Reading)>>> = OnceLock::new();
    let c = collect_codex(root, false);
    let d = async {
        let mut cache = DEEPSEEK
            .get_or_init(|| tokio::sync::Mutex::new(None))
            .lock()
            .await;
        if let Some((t, r)) = &*cache
            && t.elapsed() < Duration::from_secs(if r.packet.s == 4 { 900 } else { 300 })
        {
            return r.clone();
        }
        let r = deepseek(root).await;
        *cache = Some((Instant::now(), r.clone()));
        r
    };
    let (c, d, a, h) = tokio::join!(
        c,
        d,
        crate::argus::collect(root),
        crate::harness::collect(root)
    );
    vec![c, claude(root), d, a, h]
}

pub fn statusline(root: &Path) -> Result<()> {
    use std::{
        fs::OpenOptions,
        io::{Read, Write},
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };
    let mut input = Vec::new();
    std::io::stdin().take(1_048_577).read_to_end(&mut input)?;
    ensure!(input.len() <= 1_048_576, "状态栏输入过长。");
    let v: Value = serde_json::from_slice(&input)?;
    let r = claude_from_statusline(&v);
    // Save only numeric usage fields. No transcript path, prompts, account, token or session ID.
    let temp = root.join(format!(".claude-statusline-{}.tmp", std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    f.write_all(&serde_json::to_vec(&r)?)?;
    std::fs::rename(temp, root.join(".claude-statusline.json"))?;
    println!(
        "Claude · 上下文 {} 入 / {} 出 · EPaper 已更新",
        r.tokens
            .input
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".into()),
        r.tokens
            .output
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".into())
    );
    Ok(())
}

pub fn install_statusline() -> Result<()> {
    use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("找不到用户目录。"))?;
    let dir = std::path::PathBuf::from(home).join(".claude");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("settings.json");
    let mut v: Value = if path.exists() {
        serde_json::from_slice(&std::fs::read(&path)?)?
    } else {
        json!({})
    };
    ensure!(v.is_object(), "Claude 设置格式无效。");
    let exe = std::env::current_exe()?;
    let command = format!(
        "'{}' --claude-statusline",
        exe.to_string_lossy().replace('\'', "'\\''")
    );
    if v.get("statusLine").is_some_and(|s| !s.is_null()) {
        if v["statusLine"]["command"] == command {
            return Ok(());
        }
        bail!("已存在 Claude 状态栏设置，已保留；请参照项目说明手动接入。");
    }
    v["statusLine"] = json!({"type":"command","command":command});
    let temp = dir.join(format!(".epaper-settings-{}.tmp", std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    f.write_all(&serde_json::to_vec_pretty(&v)?)?;
    f.sync_all()?;
    std::fs::rename(temp, path)?;
    println!("Claude Code 官方状态栏接收已启用；重启 Claude Code 会话后生效。");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn manual_refresh_bypasses_error_and_success_cache() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = AtomicUsize::new(0);
        let cache = tokio::sync::Mutex::new(None);
        let key = std::array::from_fn(|_| std::ffi::OsString::new());
        let counter = &calls;
        let query = |status| async move {
            counter.fetch_add(1, Ordering::SeqCst);
            empty(0, status, "test")
        };
        assert_eq!(
            cached_codex(&cache, key.clone(), false, query(5))
                .await
                .packet
                .s,
            5
        );
        assert_eq!(
            cached_codex(&cache, key.clone(), false, query(OK))
                .await
                .packet
                .s,
            5
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            cached_codex(&cache, key.clone(), true, query(OK))
                .await
                .packet
                .s,
            OK
        );
        assert_eq!(
            cached_codex(&cache, key, true, query(OK)).await.packet.s,
            OK
        );
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn codex_cache_tracks_configuration_and_expires() {
        let cache = tokio::sync::Mutex::new(None);
        let key = std::array::from_fn(|_| std::ffi::OsString::new());
        cached_codex(&cache, key.clone(), false, async { empty(0, 5, "old") }).await;
        let mut changed = key.clone();
        changed[0] = "/different/codex".into();
        assert_eq!(
            cached_codex(&cache, changed.clone(), false, async {
                empty(0, OK, "new")
            })
            .await
            .packet
            .s,
            OK
        );
        cache.lock().await.as_mut().unwrap().0 =
            std::time::Instant::now() - Duration::from_secs(61);
        assert_eq!(
            cached_codex(&cache, changed, false, async { empty(0, 5, "expired") })
                .await
                .tokens
                .note,
            "expired"
        );
    }

    #[test]
    fn no_credentials_on_wire() {
        let r=codex_packet(&json!({"access_token":"secret","rateLimits":{"primary":{"usedPercent":12,"windowDurationMins":300,"resetsAt":1792500000}}})).unwrap();
        assert_eq!(r.w[0][0], 880);
        assert_eq!(r.w[2], [-1, 0, 0]);
        assert!(
            !String::from_utf8(r.encode().unwrap())
                .unwrap()
                .contains("secret")
        );
        assert!(serde_json::from_value::<Packet>(json!({"access_token":"secret"})).is_err());
    }
    #[test]
    fn claude_is_context_not_lifetime() {
        let r = claude_from_statusline(
            &json!({"access_token":"secret","context_window":{"total_input_tokens":123,"total_output_tokens":12},"rate_limits":{"seven_day":{"used_percentage":20,"resets_at":1792500000}}}),
        );
        assert_eq!(r.packet.w[1][0], 800);
        assert_eq!(r.tokens.input, Some(123));
        assert_eq!(r.tokens.lifetime, None);
        assert!(!serde_json::to_string(&r).unwrap().contains("secret"));
    }
    #[test]
    fn bounds() {
        let mut p = Packet::new(0, OK);
        p.w[0][0] = 1001;
        assert!(p.encode().is_err());
        p.w[0][0] = -1;
        p.b = Some(f64::INFINITY);
        assert!(p.encode().is_err());
    }
    #[test]
    fn yesterday_is_exact_date_and_separately_negotiated() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let value = json!({"dailyUsageBuckets":[{"startDate":"2026-10-23","tokens":999},{"startDate":"2026-10-24","tokens":10},{"startDate":"2026-10-24","tokens":12},{"startDate":"2026-10-25","tokens":300}]});
        assert_eq!(official_yesterday(&value, today), [20261024, 12, -1]);
        assert_eq!(official_yesterday(&json!({}), today), [20261024, -1, -1]);
        let mut p = Packet::new(0, OK);
        p.yd = Some([20261024, 12, 280]);
        let old: Value =
            serde_json::from_slice(&p.wire_for_device(true, true, true, true, false).unwrap())
                .unwrap();
        assert!(old.get("yd").is_none());
        let new: Value =
            serde_json::from_slice(&p.wire_for_device(true, true, true, true, true).unwrap())
                .unwrap();
        assert_eq!(new["yd"], json!([20261024, 12, 280]));
        p.p = 3;
        assert!(p.encode().is_err());
        p.p = 0;
        p.yd = Some([20260230, 1, 0]);
        assert!(p.encode().is_err());
        p.yd = Some([20261024, 0, 1001]);
        assert!(p.encode().is_err());
    }
    #[test]
    fn daily_buckets_never_use_yesterday_lifetime_or_duplicate_totals() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let value = json!({"summary":{"lifetimeTokens":9000000},"dailyUsageBuckets":[{"startDate":"2026-10-19","tokens":500},{"startDate":"2026-10-20","tokens":120},{"startDate":"2026-10-20","tokens":120}]});
        assert_eq!(official_daily(&value, today).0, [20261020, 120]);
        let missing = json!({"dailyUsageBuckets":[{"startDate":"2026-10-19","tokens":500}]});
        assert_eq!(
            official_daily(&missing, today),
            ([20261020, -1], Some([20261019, 500]))
        );
        assert_eq!(
            official_daily(
                &json!({"dailyUsageBuckets":[{"startDate":"2026-10-20","tokens":0}]}),
                today
            )
            .0,
            [20261020, 0]
        );
        let mut p = Packet::new(0, OK);
        p.d = Some([20260230, 1]);
        assert!(p.encode().is_err());
    }
    #[test]
    fn official_week_uses_only_known_valid_days_and_deduplicates() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let v = json!({"summary":{"lifetimeTokens":999999},"dailyUsageBuckets":[
            {"startDate":"2026-10-18","tokens":999},
            {"startDate":"2026-10-19","tokens":10},
            {"startDate":"2026-10-19","tokens":12},
            {"startDate":"2026-10-23","tokens":30},
            {"startDate":"2026-10-25","tokens":888},
            {"startDate":"2026-10-22","tokens":-1},
            {"startDate":"2026-10-21","tokens":null}
        ]});
        assert_eq!(official_weekly(&v, today), Some([20261019, 42]));
        assert_eq!(official_daily(&v, today).0, [20261024, -1]);
        let monday = chrono::NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        assert_eq!(official_weekly(&v, monday), None);
        assert_eq!(
            official_weekly(&json!({"dailyUsageBuckets":null}), today),
            None
        );
        assert_eq!(
            official_weekly(
                &json!({"dailyUsageBuckets":[{"startDate":"2026-10-24","tokens":0}]}),
                today
            ),
            Some([20261019, 0])
        );
        assert_eq!(
            official_weekly(
                &json!({"dailyUsageBuckets":[{"startDate":"2026-10-19","tokens":9007199254740991i64},{"startDate":"2026-10-20","tokens":1}]}),
                today
            ),
            None
        );
    }
    #[test]
    fn legacy_firmware_never_receives_new_fields() {
        let mut p = Packet::new(0, OK);
        p.d = Some([20261020, 42]);
        p.ld = Some([20261020, 42]);
        p.wt = Some([20261019, 420]);
        let old: Value = serde_json::from_slice(
            &p.wire_for_device(false, false, false, false, false)
                .unwrap(),
        )
        .unwrap();
        assert!(old.get("d").is_none() && old.get("ld").is_none());
        assert!(old.get("wt").is_none());
        let daily_only: Value =
            serde_json::from_slice(&p.wire_for_device(true, true, false, false, false).unwrap())
                .unwrap();
        assert!(daily_only.get("wt").is_none());
        let new: Value =
            serde_json::from_slice(&p.wire_for_device(true, true, true, false, false).unwrap())
                .unwrap();
        assert_eq!(new["d"][1], 42);
        assert_eq!(new["wt"][1], 420);
        p.wt = Some([20261020, 420]); // Tuesday cannot identify a Monday-start week.
        assert!(p.encode().is_err());
        p.wt = Some([20260230, 420]);
        assert!(p.encode().is_err());
        p.wt = Some([20261019, -1]);
        assert!(p.encode().is_err());
        p.wt = Some([20261019, 9_007_199_254_740_992]);
        assert!(p.encode().is_err());
        p.wt = Some([20241230, 0]); // The first week of 2025 starts in 2024.
        assert!(p.encode().is_ok());
    }
    #[test]
    fn monthly_buckets_are_known_calendar_month_and_capability_gated() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let value = json!({"dailyUsageBuckets":[{"startDate":"2026-09-30","tokens":900},{"startDate":"2026-10-01","tokens":10},{"startDate":"2026-10-01","tokens":12},{"startDate":"2026-10-02","tokens":5},{"startDate":"2026-10-03","tokens":800}]});
        assert_eq!(official_monthly(&value, today), Some([20261001, 17]));
        assert_eq!(
            official_monthly(&json!({"dailyUsageBuckets":null}), today),
            None
        );
        let mut packet = Packet::new(0, OK);
        packet.mt = Some([20261001, 17]);
        let old: Value = serde_json::from_slice(
            &packet
                .wire_for_device(true, true, true, false, false)
                .unwrap(),
        )
        .unwrap();
        assert!(old.get("mt").is_none());
        let new: Value = serde_json::from_slice(
            &packet
                .wire_for_device(true, true, true, true, false)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(new["mt"], json!([20261001, 17]));
        packet.mt = Some([20261002, 17]);
        assert!(packet.encode().is_err());
    }
}
