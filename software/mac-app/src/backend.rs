use crate::{
    config::{self, Lock},
    security::{Cipher, Handshake},
    usage::{self, Reading},
};
use anyhow::{Context, Result, ensure};
use btleplug::{
    api::{
        Central, CentralState, Characteristic, Manager as _, Peripheral as _,
        RetrievePeripheralsOptions, ScanFilter, WriteType,
    },
    platform::{Adapter, Manager, Peripheral},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;
use zeroize::Zeroizing;
#[path = "fleet.rs"]
mod fleet;
pub use fleet::Status as FleetStatus;
pub const SERVICE_UUID: &str = "1775244d-6b43-439b-877c-060f2d9bed07";
pub const ENROLLMENT_UUID: &str = "1775244d-6b43-439b-877c-060f2d9bed08";
#[derive(Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub rssi: Option<i16>,
    pub bound: bool,
    pub enrollment: bool,
    pub recovered: bool,
}
#[derive(Clone)]
pub struct State {
    /// True only on a presentation copy; imported/derived numbers are never collected again.
    pub usage_resolved: bool,
    pub fleet: std::collections::BTreeMap<String, fleet::Status>,
    pub active_device: String,
    /// One slot per configured city (at most two).
    pub weather: [Option<crate::weather::Snapshot>; 2],
    pub weather_notice: String,
    /// Hub-owned todos (tombstones excluded) and where they sync.
    pub todos: Vec<crate::todo::Todo>,
    pub todo_note: String,
    pub poem: Option<crate::poem::Poem>,
    /// Temperature/humidity history per display, recorded by the hub.
    pub env_log: crate::envlog::Log,
    /// Saved place names, sent to displays advertising places:1.
    pub places: crate::home::model::Places,
    // Presentation-only provenance. Backend snapshots always keep their local origin.
    pub cloud_sources: std::collections::BTreeMap<u8, String>,
    pub environment_source: Option<String>,
    pub cloud: crate::cloud::Status,
    pub cloud_data: crate::cloud::Catalog,
    pub devices: Vec<Device>,
    pub scanning: bool,
    pub connected: bool,
    pub connecting: bool,
    pub pairing: bool,
    pub status: String,
    pub readings: Vec<Reading>,
    pub harness_devices: Vec<crate::harness::DeviceUsage>,
    pub synced: Option<i64>,
    pub logs: Vec<String>,
    pub bound_name: String,
    pub cli_diagnostic: Option<crate::codex_cli::Diagnostic>,
    pub checking_cli: bool,
    pub refreshing_codex: bool,
    pub refreshing_usage: bool,
    pub last_read: Option<i64>,
    pub next_read: Option<i64>,
    pub refresh: crate::refresh::Settings,
    pub device_supported: bool,
    /// Legacy firmware omits these capabilities and supports both; newer boards may decline.
    pub rotation_supported: bool,
    pub history_supported: bool,
    pub telemetry: Option<crate::device::Telemetry>,
    pub history: Option<crate::device::History>,
    pub controlling_device: bool,
    pub device_notice: String,
    pub device_read_at: Option<i64>,
    pub feedback: Option<crate::feedback::Feedback>,
    pub connection_error: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            usage_resolved: false,
            fleet: Default::default(),
            active_device: String::new(),
            weather: [None, None],
            weather_notice: String::new(),
            todos: vec![],
            todo_note: String::new(),
            poem: None,
            env_log: Default::default(),
            places: Default::default(),
            cloud_sources: Default::default(),
            environment_source: None,
            cloud: crate::cloud::Status::default(),
            cloud_data: crate::cloud::Catalog::default(),
            devices: vec![],
            scanning: false,
            connected: false,
            connecting: false,
            pairing: false,
            status: "等待扫描墨水屏".into(),
            readings: vec![],
            harness_devices: vec![],
            synced: None,
            logs: vec![],
            bound_name: String::new(),
            cli_diagnostic: None,
            checking_cli: false,
            refreshing_codex: false,
            refreshing_usage: false,
            last_read: None,
            next_read: None,
            refresh: crate::refresh::Settings::default(),
            device_supported: false,
            rotation_supported: true,
            history_supported: true,
            telemetry: None,
            history: None,
            controlling_device: false,
            device_notice: String::new(),
            device_read_at: None,
            feedback: None,
            connection_error: None,
        }
    }
}
pub enum Command {
    Cloud(crate::cloud::Command),
    Todo(crate::todo::Command),
    Scan,
    Connect(String),
    ConnectionForm {
        active: bool,
        disconnect: bool,
        target: Option<String>,
    },
    Disconnect,
    DisconnectAll,
    PauseDevice(String),
    Refresh,
    ReadUsage,
    CheckCodex(crate::codex_cli::Settings),
    RefreshCodex,
    ConfigureRefresh(crate::refresh::Settings),
    Device(crate::device::Action),
    PairUsb,
    PairBluetooth {
        id: String,
        code: Zeroizing<String>,
    },
    Quit,
}
pub type Shared = Arc<Mutex<State>>;
#[derive(Clone)]
pub struct Events {
    pub state: Shared,
    pub repaint: eframe::egui::Context,
}
impl Events {
    fn feedback(&self, message: impl Into<String>, busy: bool, error: bool) {
        self.update(|s| s.feedback = Some(crate::feedback::Feedback::new(message, busy, error)));
    }
    pub fn update(&self, f: impl FnOnce(&mut State)) {
        if let Ok(mut s) = self.state.lock() {
            f(&mut s);
        }
        self.repaint.request_repaint();
    }
    fn log(&self, message: &str) {
        self.update(|s| {
            s.status = message.into();
            s.logs.push(format!(
                "{}  {message}",
                chrono::Local::now().format("%H:%M:%S")
            ));
            if s.logs.len() > 20 {
                s.logs.remove(0);
            }
        });
    }
}
async fn adapter() -> Result<Adapter> {
    Manager::new()
        .await?
        .adapters()
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("未找到蓝牙适配器；请在系统设置开启蓝牙并允许本 App 访问。"))
}
async fn scan(
    adapter: &Adapter,
    root: &Path,
    events: Option<&Events>,
) -> Result<Vec<(Device, Peripheral)>> {
    if let Some(e) = events {
        e.update(|s| s.scanning = true);
        e.log("正在扫描附近的 EPaper…");
    }
    ensure!(
        adapter.adapter_state().await? == CentralState::PoweredOn,
        "Mac蓝牙不可用，请检查蓝牙开关与HomeCraft权限。"
    );
    let bindings = crate::registry::list(root)?;
    let legacy = config::pairing(root).ok();
    adapter
        .start_scan(ScanFilter {
            services: vec![
                Uuid::parse_str(SERVICE_UUID)?,
                Uuid::parse_str(ENROLLMENT_UUID)?,
            ],
        })
        .await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    let result: Result<Vec<(Device, Peripheral)>> = async {
        let mut found = Vec::new();
        for p in adapter.peripherals().await? {
            if let Some(props) = p.properties().await? {
                let name = props
                    .advertisement_name
                    .or(props.local_name)
                    .unwrap_or_default();
                let enrollment = name.starts_with("EPair_")
                    && props.services.contains(&Uuid::parse_str(ENROLLMENT_UUID)?);
                if !name.starts_with("EPaper_") && !enrollment {
                    continue;
                }
                let ordinary = name.replacen("EPair_", "EPaper_", 1);
                let bound = bindings.iter().any(|b| b.name == ordinary)
                    || legacy.as_ref().is_some_and(|b| b.name == ordinary);
                found.push((
                    Device {
                        id: p.id().to_string(),
                        name,
                        rssi: props.rssi,
                        bound,
                        enrollment,
                        recovered: false,
                    },
                    p,
                ));
            }
        }
        let identifiers: Vec<_> = bindings
            .iter()
            .filter_map(|b| Uuid::parse_str(&b.id).ok().map(Into::into))
            .collect();
        if !identifiers.is_empty() {
            let restored = tokio::time::timeout(
                Duration::from_secs(3),
                adapter.retrieve_peripherals(RetrievePeripheralsOptions {
                    identifiers: Some(identifiers),
                    services: None,
                }),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
            for p in restored {
                if found.iter().any(|(_, old)| old.id() == p.id()) {
                    continue;
                }
                let Some(b) = bindings.iter().find(|b| b.id == p.id().to_string()) else {
                    continue;
                };
                found.push((
                    Device {
                        id: b.id.clone(),
                        name: b.name.clone(),
                        rssi: None,
                        bound: true,
                        enrollment: false,
                        recovered: true,
                    },
                    p,
                ));
            }
        }
        found.sort_by_key(|(d, _)| (!d.bound, d.name.clone()));
        Ok(found)
    }
    .await;
    let _ = adapter.stop_scan().await;
    if let Some(e) = events {
        e.update(|s| s.scanning = false);
    }
    result
}
async fn exchange(p: &Peripheral, c: &Characteristic, b: &[u8]) -> Result<Vec<u8>> {
    tokio::time::timeout(Duration::from_secs(15), async {
        p.write(c, b, WriteType::WithResponse).await?;
        let r = p.read(c).await?;
        ensure!(r.len() <= 2048, "蓝牙回复过长。");
        Ok(r)
    })
    .await?
}

async fn authenticate(
    p: &Peripheral,
    service: &str,
    username: &str,
    password: &str,
    data_name: &str,
) -> Result<(Characteristic, Cipher)> {
    ensure!(
        (service == SERVICE_UUID && data_name == "usage-data")
            || (service == ENROLLMENT_UUID && data_name == "pair-device"),
        "认证模式不匹配。"
    );
    p.connect_with_timeout(Duration::from_secs(25))
        .await
        .context("建立蓝牙连接失败")?;
    p.discover_services_with_timeout(Duration::from_secs(20))
        .await
        .context("发现蓝牙服务失败")?;
    let mut services: HashMap<String, HashMap<String, Characteristic>> = HashMap::new();
    for c in p
        .characteristics()
        .into_iter()
        .filter(|c| [SERVICE_UUID, ENROLLMENT_UUID].contains(&c.service_uuid.to_string().as_str()))
    {
        for d in &c.descriptors {
            if d.uuid == Uuid::from_u128(0x0000290100001000800000805f9b34fb) {
                let name = String::from_utf8(p.read_descriptor(d).await?)?;
                if ["proto-ver", "prov-session", "usage-data", "pair-device"]
                    .contains(&name.as_str())
                {
                    let group = services.entry(c.service_uuid.to_string()).or_default();
                    ensure!(
                        group.insert(name, c.clone()).is_none(),
                        "设备存在重复认证端点。"
                    );
                }
            }
        }
    }
    // CoreBluetooth can retain the old known service UUID after an in-place mode
    // switch. Resolve the actual endpoint names within ONE known service, before
    // sending any PIN/long-term proof. Opposite modes must never be authenticated.
    let endpoints = select_mode_endpoints(services, service, data_name)?;
    let endpoint = |name: &str| {
        endpoints
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("设备缺少 {name} 服务；请确认设备模式及固件版本。"))
    };
    // Resolve the expected mode before attempting auth: a temporary pairing PIN must
    // never be retried automatically against the regular data service (or vice versa).
    let data = endpoint(data_name)?;
    let version: serde_json::Value =
        serde_json::from_slice(&exchange(p, &endpoint("proto-ver")?, b"EPaper").await?)?;
    ensure!(
        version["prov"]["sec_ver"] == 2 && version["prov"]["sec_patch_ver"] == 1,
        "设备未启用 Security 2 计数器协议，已拒绝连接。"
    );
    let session = endpoint("prov-session")?;
    let mut h = Handshake::new();
    let r0 = exchange(p, &session, &h.first(username))
        .await
        .context("读取设备认证挑战失败")?;
    let request = h.challenge(&r0, username, password)?;
    let r1 = exchange(p, &session, &request)
        .await
        .context("提交认证证明失败")?;
    Ok((data, h.finish(&r1)?))
}

fn select_mode_endpoints<T>(
    mut services: HashMap<String, HashMap<String, T>>,
    expected_service: &str,
    endpoint: &str,
) -> Result<HashMap<String, T>> {
    let opposite = if endpoint == "pair-device" {
        "usage-data"
    } else {
        "pair-device"
    };
    let eligible = |group: &HashMap<String, T>| {
        group.contains_key(endpoint)
            && group.contains_key("proto-ver")
            && group.contains_key("prov-session")
            && !group.contains_key(opposite)
    };
    if services.get(expected_service).is_some_and(&eligible) {
        return Ok(services.remove(expected_service).unwrap());
    }
    let candidates: Vec<_> = services
        .iter()
        .filter(|(uuid, group)| {
            [SERVICE_UUID, ENROLLMENT_UUID].contains(&uuid.as_str()) && eligible(group)
        })
        .map(|(uuid, _)| uuid.clone())
        .collect();
    if candidates.len() != 1 {
        let other_mode = services.iter().any(|(uuid, group)| {
            [SERVICE_UUID, ENROLLMENT_UUID].contains(&uuid.as_str())
                && group.contains_key(opposite)
                && !group.contains_key(endpoint)
        });
        return Err(if other_mode && endpoint == "usage-data" {
            AuthModeError::PairingOpen
        } else if other_mode {
            AuthModeError::PairingClosed
        } else {
            AuthModeError::Unavailable
        }
        .into());
    }
    Ok(services.remove(&candidates[0]).unwrap())
}

#[derive(Debug)]
enum AuthModeError {
    PairingOpen,
    PairingClosed,
    Unavailable,
}
impl std::fmt::Display for AuthModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::PairingOpen => "设备正在配对。请在连接页输入屏幕码，再点击下方的安全连接。",
            Self::PairingClosed => {
                "屏幕配对窗口尚未打开或已结束。请在墨水屏连接页双击 BOOT，再重新扫描。"
            }
            Self::Unavailable => {
                "暂时无法确认设备的连接模式，请重新扫描；若仍失败，请重新打开屏幕配对窗口。"
            }
        })
    }
}
impl std::error::Error for AuthModeError {}

async fn pair_bluetooth(
    adapter: &Adapter,
    p: &Peripheral,
    root: &Path,
    name: &str,
    code: &str,
) -> Result<String> {
    ensure!(
        code.len() == 8 && code.bytes().all(|c| c.is_ascii_digit()),
        "请输入屏幕上的8位配对码。"
    );
    let _lock = Lock::take(root, ".ble-enrollment.lock")?;
    let fresh = adapter
        .retrieve_peripherals(RetrievePeripheralsOptions {
            identifiers: Some(vec![p.id()]),
            services: None,
        })
        .await?
        .into_iter()
        .find(|fresh| fresh.id() == p.id())
        .ok_or_else(|| anyhow::anyhow!("配对设备暂不可用，请重新扫描。"))?;
    let p = &fresh;
    let result: Result<String> = async {
        let (endpoint, mut cipher) =
            authenticate(p, ENROLLMENT_UUID, "wifiprov", code, "pair-device").await?;
        let encrypted = cipher.encrypt(b"get")?;
        let response = exchange(p, &endpoint, &encrypted).await?;
        let plain = Zeroizing::new(cipher.decrypt(&response)?);
        ensure!(plain.len() <= 512, "设备认证回复过长。");
        // The public name may still be cached across the mode switch. Validate
        // the encrypted reply against the same physical device's suffix.
        let suffix = name
            .strip_prefix("EPair_")
            .or_else(|| name.strip_prefix("EPaper_"))
            .context("请选择 EPaper 设备。")?;
        let config = config::wireless_pairing(&plain, &format!("EPair_{suffix}"))?;
        crate::registry::store(root, &config, &p.id().to_string())?;
        Ok(config.name.clone())
    }
    .await;
    let _ = p.disconnect().await;
    result
}
struct Link {
    peripheral: Peripheral,
    data: Characteristic,
    cipher: Cipher,
    daily_supported: bool,
    profiles_supported: bool,
    harness_supported: bool,
    weekly_supported: bool,
    monthly_supported: bool,
    yesterday_supported: bool,
    breakdown_supported: bool,
    sample_supported: bool,
    device_supported: bool,
    rotation_supported: bool,
    history_supported: bool,
    /// Weather slots the display accepts (weather:1 or weather:2).
    weather_slots: usize,
    forecast_supported: bool,
    harness_aggregate_supported: bool,
    todo_supported: bool,
    poem_supported: bool,
    totals_supported: bool,
    places_supported: bool,
    /// Exact messages last sent after connecting; resent only when they change.
    last_todo: Option<Vec<Vec<u8>>>,
    last_poem: Option<Vec<u8>>,
    last_totals: Option<Vec<u8>>,
    last_places: Option<Vec<u8>>,
    /// Per slot: None until the first sync after connecting; Some(0) once told to clear.
    last_weather: [Option<i64>; 2],
    display_timeout: u64,
    _lock: Lock,
}
impl Link {
    async fn connect(p: Peripheral, root: &Path, name: &str) -> Result<Self> {
        let config = config::pairing(root)?;
        ensure!(name == config.name, "首次连接请输入设备屏幕上的配对码。");
        let lock = Lock::take(root, ".ble-sender.lock")?;
        let auth = authenticate(
            &p,
            SERVICE_UUID,
            &config.username,
            &config.password,
            "usage-data",
        )
        .await;
        match auth {
            Ok((data, cipher)) => {
                let mut link = Self {
                    peripheral: p,
                    data,
                    cipher,
                    daily_supported: false,
                    profiles_supported: false,
                    harness_supported: false,
                    weekly_supported: false,
                    monthly_supported: false,
                    yesterday_supported: false,
                    breakdown_supported: false,
                    sample_supported: false,
                    device_supported: false,
                    rotation_supported: true,
                    history_supported: true,
                    weather_slots: 0,
                    forecast_supported: false,
                    harness_aggregate_supported: false,
                    todo_supported: false,
                    poem_supported: false,
                    totals_supported: false,
                    places_supported: false,
                    last_todo: None,
                    last_poem: None,
                    last_totals: None,
                    last_places: None,
                    last_weather: [None, None],
                    display_timeout: 35,
                    _lock: lock,
                };
                if let Err(error) = link.send(b"?").await {
                    link.disconnect().await;
                    return Err(error);
                }
                Ok(link)
            }
            Err(e) => {
                let _ = p.disconnect().await;
                Err(e)
            }
        }
    }
    async fn exchange_json(&mut self, b: &[u8]) -> Result<serde_json::Value> {
        let encrypted = self.cipher.encrypt(b)?;
        let response = exchange(&self.peripheral, &self.data, &encrypted).await?;
        let plain = self.cipher.decrypt(&response)?;
        let v: serde_json::Value = serde_json::from_slice(&plain)?;
        ensure!(v["ok"] == true, "设备拒绝快照。");
        if v.get("daily").is_some() {
            self.daily_supported = v["daily"] == true;
            self.profiles_supported = v["profiles"] == true;
            self.harness_supported = v["harness_devices"] == 1;
            self.weekly_supported = v["weekly_tokens"] == true;
            self.monthly_supported = v["monthly_tokens"] == true;
            self.yesterday_supported = v["yesterday_usage"] == true;
            self.breakdown_supported = v["argus_breakdown"] == 1;
            self.sample_supported = v["sample_refresh"] == 1;
            self.device_supported = v["device_control"] == 1;
            self.rotation_supported = v.get("rotation_control").is_none_or(|x| x == 1);
            self.history_supported = v.get("environment_history").is_none_or(|x| x == 1);
            self.weather_slots = v["weather"].as_u64().map_or(0, |n| n.min(2) as usize);
            self.forecast_supported = v["forecast"] == 1;
            self.harness_aggregate_supported = v["harness_aggregate"] == 1;
            self.todo_supported = v["todo"] == 1;
            self.poem_supported = v["poem"] == 1;
            self.totals_supported = v["totals"] == 1;
            self.places_supported = v["places"] == 1;
            self.display_timeout = if v["model"] == "reTerminalE1002" {
                90
            } else {
                35
            };
        }
        Ok(v)
    }
    async fn send(&mut self, b: &[u8]) -> Result<u64> {
        let v = self.exchange_json(b).await?;
        Ok(v["refresh"].as_u64().unwrap_or(0))
    }
    async fn device(&mut self, action: &crate::device::Action) -> Result<serde_json::Value> {
        ensure!(
            self.device_supported,
            "当前固件尚未支持双向控制，请更新墨水屏固件。"
        );
        let request = if matches!(action, crate::device::Action::Read) && !self.sample_supported {
            crate::device::Action::Status.request()?
        } else {
            action.request()?
        };
        self.exchange_json(&serde_json::to_vec(&request)?).await
    }
    async fn telemetry(&mut self) -> Result<crate::device::Telemetry> {
        crate::device::Telemetry::parse(
            &self.device(&crate::device::Action::Status).await?,
            usage::now(),
        )
    }
    async fn history(&mut self) -> Result<Option<crate::device::History>> {
        let mut chunks = Vec::new();
        for offset in (0..96).step_by(8) {
            let value = self
                .exchange_json(&serde_json::to_vec(
                    &serde_json::json!({"v":1,"op":"history","offset":offset}),
                )?)
                .await?;
            ensure!(value["v"] == 1, "历史版本不支持");
            chunks.push(serde_json::from_value(value)?);
        }
        Ok(crate::device::History::assemble(chunks, usage::now()).ok())
    }
    async fn send_usage(&mut self, packet: &usage::Packet) -> Result<u64> {
        if packet.p == crate::harness::PROVIDER && !self.harness_aggregate_supported {
            return Ok(0);
        }
        if packet.p == crate::harness::PROVIDER {
            let mut packet = packet.clone();
            packet.o = None;
            if !self.monthly_supported {
                packet.mt = None;
            }
            return self.send(&packet.encode()?).await;
        }
        self.send(&packet.wire_for_device(
            self.daily_supported,
            self.profiles_supported,
            self.weekly_supported,
            self.monthly_supported,
            self.yesterday_supported,
        )?)
        .await
    }
    async fn harness_devices(&mut self) -> Result<Vec<crate::harness::DeviceUsage>> {
        ensure!(self.harness_supported, "设备尚不支持多 Mac 汇总");
        let mut rows = Vec::new();
        let mut count = None;
        for offset in 0..8 {
            let v = self
                .exchange_json(&serde_json::to_vec(
                    &serde_json::json!({"v":1,"op":"harness","offset":offset}),
                )?)
                .await?;
            let n = v["count"]
                .as_u64()
                .filter(|n| *n <= 8)
                .ok_or_else(|| anyhow::anyhow!("Harness 设备数量无效"))?;
            ensure!(
                count.is_none_or(|c| c == n) && v["v"] == 1 && v["offset"] == offset,
                "Harness 汇总分块不一致"
            );
            count = Some(n);
            let batch: Vec<crate::harness::DeviceUsage> =
                serde_json::from_value(v["devices"].clone())?;
            ensure!(
                batch.len() == usize::from((offset as u64) < n),
                "Harness 汇总分块缺失"
            );
            for row in batch {
                ensure!(
                    row.valid()
                        && !rows
                            .iter()
                            .any(|d: &crate::harness::DeviceUsage| d.id == row.id),
                    "Harness 设备快照无效"
                );
                rows.push(row);
            }
            if rows.len() as u64 == n {
                return Ok(rows);
            }
        }
        anyhow::bail!("Harness 汇总不完整")
    }
    async fn send_reading(&mut self, reading: &usage::Reading) -> Result<u64> {
        let mut sequence = 0;
        if reading.packet.p == crate::harness::PROVIDER && !self.harness_aggregate_supported {
            return Ok(0);
        }
        let packets = crate::harness::display_packets(reading, false)?;
        for packet in packets {
            sequence = self.send_usage(&packet).await?;
        }
        if self.breakdown_supported
            && reading.packet.p == 3
            && reading.packet.s == usage::OK
            && let Some(detail) = &reading.tokens.breakdown
        {
            self.send(&detail.wire(&reading.packet)?).await?;
        }
        Ok(sequence)
    }
    /// Keeps a display in step with the hub: send each fresh summary once, clear one that is off or expired.
    async fn sync_weather(
        &mut self,
        weather: &[Option<crate::weather::Snapshot>; 2],
    ) -> Result<()> {
        let now = usage::now();
        for (slot, current) in weather.iter().enumerate().take(self.weather_slots) {
            let current = current.as_ref();
            match crate::weather::sync_action(self.last_weather[slot], current, now) {
                Some(crate::weather::Sync::Send(at)) => {
                    let packet = current.context("天气摘要缺失")?.wire(
                        now,
                        slot,
                        self.forecast_supported,
                    )?;
                    self.send(&packet).await?;
                    self.last_weather[slot] = Some(at);
                }
                Some(crate::weather::Sync::Clear) => {
                    self.send(&serde_json::to_vec(
                        &serde_json::json!({"v":1,"op":"weather","enabled":false,"slot":slot}),
                    )?)
                    .await?;
                    self.last_weather[slot] = Some(0);
                }
                None => {}
            }
        }
        Ok(())
    }
    /// Todos and the daily poem for displays that advertise todo:1 / poem:1.
    async fn sync_extras(
        &mut self,
        todos: &[crate::todo::Todo],
        poem: Option<&crate::poem::Poem>,
    ) -> Result<()> {
        if self.todo_supported {
            let messages = crate::todo::wire(todos, crate::todo::today())?;
            if self.last_todo.as_ref() != Some(&messages) {
                for m in &messages {
                    self.send(m).await?;
                }
                self.last_todo = Some(messages);
            }
        }
        if self.poem_supported {
            let message = match poem {
                Some(p) => p.wire()?,
                None => br#"{"v":1,"op":"poem","enabled":false}"#.to_vec(),
            };
            if self.last_poem.as_ref() != Some(&message) {
                self.send(&message).await?;
                self.last_poem = Some(message);
            }
        }
        Ok(())
    }
    /// The home totals, so the display never re-adds packets that carry one Codex account.
    async fn sync_totals(&mut self, message: Option<Vec<u8>>) -> Result<()> {
        if let Some(message) = message.filter(|_| self.totals_supported)
            && self.last_totals.as_ref() != Some(&message)
        {
            self.send(&message).await?;
            self.last_totals = Some(message);
        }
        Ok(())
    }
    /// Named places with each one's latest reading, for displays that advertise places:1.
    async fn sync_places(&mut self, message: Option<Vec<u8>>) -> Result<()> {
        if let Some(message) = message.filter(|_| self.places_supported)
            && self.last_places.as_ref() != Some(&message)
        {
            self.send(&message).await?;
            self.last_places = Some(message);
        }
        Ok(())
    }
    async fn disconnect(self) {
        let _ = self.peripheral.disconnect().await;
    }
}

enum QueryWork {
    All,
    Codex,
    Check(crate::codex_cli::Settings),
}
enum QueryResult {
    All(Vec<Reading>),
    Codex(Box<Reading>),
    Check(crate::codex_cli::Diagnostic),
}
async fn query_work(root: PathBuf, work: QueryWork) -> QueryResult {
    match work {
        QueryWork::All => QueryResult::All(usage::collect(&root).await),
        QueryWork::Codex => QueryResult::Codex(Box::new(usage::collect_codex(&root, true).await)),
        QueryWork::Check(settings) => {
            QueryResult::Check(crate::codex_cli::diagnose(&root, Some(settings)).await)
        }
    }
}

pub async fn worker(
    root: PathBuf,
    events: Events,
    mut commands: UnboundedReceiver<Command>,
    auto_connect: bool,
) {
    let mut adapter = None;
    let cached = crate::harness::load_devices(&root);
    // Remembered places remain usable while CoreBluetooth is starting or awaiting permission.
    // This is metadata only: a remembered row never proves a connection or live reading.
    let remembered = crate::registry::list(&root).unwrap_or_default();
    events.update(|s| {
        s.harness_devices = cached;
        s.bound_name = config::pairing(&root)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        for binding in remembered {
            s.fleet.insert(
                binding.id.clone(),
                fleet::Status {
                    id: binding.id,
                    name: binding.name,
                    model: binding.model,
                    notice: "等待蓝牙连接".into(),
                    ..Default::default()
                },
            );
        }
    });
    let cloud = crate::cloud::spawn(root.clone(), events.clone());
    let weather_task = crate::weather::spawn(root.clone(), events.clone());
    let poem_task = crate::poem::spawn(root.clone(), events.clone());
    let todo = crate::todo::spawn(root.clone(), events.clone());
    let env_log = crate::envlog::load(&root);
    events.update(|s| s.env_log = env_log);
    let mut fleet = fleet::Manager::new(auto_connect, root.clone());
    let mut active_id: Option<String> = None;
    let mut devices: Vec<(Device, Peripheral)> = vec![];
    let mut link: Option<Link> = None;
    let mut desired: Option<String> = None;
    let mut reconnect = Instant::now();
    let mut backoff = 3u64;
    let settings =
        config::read_private::<crate::home::model::Preferences>(&root.join(".home-ui.json"))
            .ok()
            .filter(|p| p.validate())
            .unwrap_or_default()
            .refresh;
    let mut schedule = crate::refresh::Schedule::new(settings, Instant::now());
    let mut needs_sync = false;
    // Packets last sent to the controlled display; iCloud updates from other Macs resend.
    let mut sent_revision: Option<Vec<u8>> = None;
    let mut manual_usage = false;
    let mut query_task: Option<tokio::task::JoinHandle<QueryResult>> = None;
    let mut query_queue = std::collections::VecDeque::new();
    let mut codex_to_send: Option<Reading> = None;
    let mut request_usage_display = false;
    let mut pending_usage_display = false;
    let mut pending_control: Option<(crate::device::Action, u32, Instant)> = None;
    events.update(|s| {
        s.refresh = settings;
        s.next_read = schedule.next_unix(Instant::now(), usage::now());
    });
    let mut last_ping = Instant::now();
    let mut last_device = Instant::now() - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
    let mut last_history = Instant::now() - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
    let mut last_harness = Instant::now() - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
    let mut sequence = None;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut initial = true;
    let mut startup_scan = true;
    let mut reconnect_suspended = false;
    let mut connect_feedback = false;
    let mut deferred_command = None;
    loop {
        // Account I/O never holds the BLE/control loop while a network service is slow.
        if query_task.as_ref().is_some_and(|task| task.is_finished()) {
            match query_task.take().unwrap().await {
                Ok(QueryResult::All(readings)) => {
                    schedule.finish(Instant::now());
                    events.update(|s| {
                        let local = readings
                            .iter()
                            .find(|r| r.packet.p == crate::harness::PROVIDER);
                        if let Some(local) = local {
                            s.harness_devices =
                                crate::harness::effective(local, &s.harness_devices)
                                    .tokens
                                    .devices;
                            if crate::harness::save_devices(&root, &s.harness_devices).is_err() {
                                s.device_notice = "电脑统计缓存未保存".into();
                            }
                        }
                        s.readings = readings;
                        s.last_read = Some(usage::now());
                        s.next_read = schedule.next_unix(Instant::now(), usage::now());
                        s.refreshing_usage = false;
                    });
                    needs_sync = true;
                    if manual_usage && link.is_none() {
                        manual_usage = false;
                        events.feedback("用量已读取，设备未连接", false, false);
                    }
                }
                Ok(QueryResult::Codex(reading)) => {
                    let reading = *reading;
                    let ok = reading.packet.s == usage::OK;
                    events.update(|s| {
                        if let Some(old) = s.readings.iter_mut().find(|r| r.packet.p == 0) {
                            *old = reading.clone();
                        } else {
                            s.readings.insert(0, reading.clone());
                        }
                        s.refreshing_codex = false;
                    });
                    events.feedback(
                        if ok {
                            "Codex 用量已更新".into()
                        } else {
                            reading.tokens.note.clone()
                        },
                        false,
                        !ok,
                    );
                    codex_to_send = link.as_ref().map(|_| reading);
                }
                Ok(QueryResult::Check(report)) => {
                    events.feedback(report.message.clone(), false, !report.ok);
                    events.update(|s| {
                        s.checking_cli = false;
                        s.cli_diagnostic = Some(report);
                    });
                }
                Err(_) => {
                    query_queue.clear();
                    schedule.finish(Instant::now());
                    events.update(|s| {
                        s.refreshing_usage = false;
                        s.refreshing_codex = false;
                        s.checking_cli = false;
                    });
                    events.feedback("读取未完成，请重试", false, true);
                }
            }
        }
        let command = if let Some(c) = deferred_command.take() {
            Some(c)
        } else if initial {
            initial = false;
            Some(Command::Scan)
        } else {
            tokio::select! {c=commands.recv()=>match c{Some(c)=>Some(c),None=>Some(Command::Quit)},_=tick.tick()=>None}
        };
        if let Some(c) = command {
            match c {
                Command::Cloud(command) => {
                    let _ = cloud.send(command);
                }
                Command::Todo(command) => {
                    let _ = todo.send(command);
                }
                Command::Quit => {
                    weather_task.abort();
                    poem_task.abort();
                    fleet.shutdown().await;
                    if let Some(task) = query_task.take() {
                        task.abort();
                    }
                    if let Some(l) = link.take() {
                        l.disconnect().await;
                    }
                    break;
                }
                Command::Scan => {
                    let connect_on_scan = startup_scan && auto_connect;
                    startup_scan = false;
                    events.feedback("正在扫描附近的墨水屏…", true, false);
                    if adapter.is_none() {
                        match self::adapter().await {
                            Ok(a) => adapter = Some(a),
                            Err(_) => {
                                events.update(|s| s.scanning = false);
                                events.log("蓝牙不可用，请检查系统蓝牙与 App 权限。");
                                events.feedback(
                                    "蓝牙不可用，请检查系统蓝牙与 App 权限。",
                                    false,
                                    true,
                                );
                                continue;
                            }
                        }
                    }
                    match scan(adapter.as_ref().unwrap(), &root, Some(&events)).await {
                        Ok(found) => {
                            devices = found;
                            events.update(|s| {
                                s.devices = devices.iter().map(|(d, _)| d.clone()).collect();
                                s.bound_name = config::pairing(&root)
                                    .map(|p| p.name.clone())
                                    .unwrap_or_default();
                            });
                            events.log(if devices.is_empty() {
                                "暂未收到墨水屏广播；设备可能正在由另一台Mac连接。"
                            } else {
                                "请选择墨水屏并连接"
                            });
                            events.feedback(
                                if devices.is_empty() {
                                    "扫描完成，暂未收到墨水屏广播。".into()
                                } else if devices.iter().all(|(d, _)| d.recovered) {
                                    "已找回保存的设备，暂未收到广播。".into()
                                } else {
                                    format!("扫描完成，发现 {} 台墨水屏。", devices.len())
                                },
                                false,
                                false,
                            );
                            if connect_on_scan && desired.is_none() && link.is_none() {
                                desired = devices
                                    .iter()
                                    .find(|(d, _)| d.bound && !d.enrollment)
                                    .map(|(d, _)| d.id.clone());
                            }
                        }
                        Err(error) => {
                            events.update(|s| {
                                s.scanning = false;
                                s.connection_error = Some(error.to_string());
                            });
                            events.log("扫描失败，请检查蓝牙权限后重试。");
                            events.feedback("扫描失败，请检查蓝牙权限后重试。", false, true);
                        }
                    }
                }
                Command::Connect(id) => {
                    pending_control = None;
                    pending_usage_display = false;
                    events.update(|s| {
                        s.telemetry = None;
                        s.history = None;
                        s.controlling_device = false;
                    });
                    fleet.stop(&id).await;
                    fleet.resume(&id);
                    reconnect_suspended = false;
                    connect_feedback = true;
                    events.update(|s| s.connection_error = None);
                    events.feedback("正在连接并验证设备身份…", true, false);
                    if let Some(l) = link.take() {
                        if let Some(old) = active_id.as_ref().and_then(|old| {
                            crate::registry::list(&root)
                                .ok()?
                                .into_iter()
                                .find(|b| &b.id == old)
                        }) {
                            fleet.attach(old, l, events.clone());
                        } else {
                            l.disconnect().await;
                        }
                    }
                    events.update(|s| s.active_device = id.clone());
                    desired = Some(id);
                    backoff = 3;
                    reconnect = Instant::now();
                }
                Command::ConnectionForm {
                    active,
                    disconnect,
                    target,
                } => {
                    let stop_target = active
                        && target.as_ref().is_some_and(|id| {
                            disconnect
                                || events.state.lock().ok().is_none_or(|s| {
                                    s.fleet.get(id).is_none_or(|row| !row.connected)
                                })
                        });
                    if stop_target && let Some(id) = target.as_ref() {
                        fleet.stop(id).await;
                    }
                    fleet.form_target(if active { target.clone() } else { None });
                    let disconnect = disconnect
                        && target
                            .as_ref()
                            .is_none_or(|id| active_id.as_ref() == Some(id));
                    reconnect_suspended = active;
                    // Editing a connection form never initiates a connection.
                    // Cancel a pending retry, while preserving an established link.
                    if (active && link.is_none()) || disconnect {
                        desired = None;
                    }
                    if active && link.is_none() {
                        events.update(|s| {
                            s.connecting = false;
                            if s.feedback
                                .as_ref()
                                .is_some_and(|f| f.busy && f.message.contains("连接"))
                            {
                                s.feedback = None;
                            }
                        });
                    }
                    if disconnect {
                        if let Some(l) = link.take() {
                            l.disconnect().await;
                        }
                        pending_control = None;
                        events.update(|s| {
                            s.connected = false;
                            s.connecting = false;
                            s.controlling_device = false;
                        });
                    }
                }
                Command::Disconnect => {
                    if let Some(id) = active_id.as_ref() {
                        fleet.pause(id);
                        fleet.stop(id).await;
                        events.update(|s| {
                            if let Some(row) = s.fleet.get_mut(id) {
                                row.connected = false;
                                row.paused = true;
                                row.notice = "已暂停".into();
                            }
                        });
                    }
                    pending_usage_display = false;
                    request_usage_display = false;
                    desired = None;
                    if let Some(l) = link.take() {
                        l.disconnect().await;
                    }
                    events.update(|s| {
                        s.connected = false;
                        s.connecting = false;
                    });
                    events.log("已暂停蓝牙同步；菜单栏程序继续运行。");
                    pending_control = None;
                    events.update(|s| s.controlling_device = false);
                    events.feedback("蓝牙已暂停，另一台 Mac 现在可以连接。", false, false);
                }
                Command::PauseDevice(id) => {
                    if active_id.as_ref() == Some(&id) {
                        deferred_command = Some(Command::Disconnect);
                        continue;
                    }
                    fleet.pause(&id);
                    fleet.stop(&id).await;
                    events.update(|s| {
                        if let Some(row) = s.fleet.get_mut(&id) {
                            row.connected = false;
                            row.connecting = false;
                            row.paused = true;
                            row.notice = "已暂停".into();
                        }
                    });
                    events.feedback("这台设备已暂停", false, false);
                }
                Command::DisconnectAll => {
                    if let Ok(rows) = crate::registry::list(&root) {
                        for row in rows {
                            fleet.pause(&row.id);
                        }
                    }
                    fleet.shutdown().await;
                    if let Some(l) = link.take() {
                        l.disconnect().await;
                    }
                    desired = None;
                    reconnect_suspended = true;
                    pending_control = None;
                    events.update(|s| {
                        s.connected = false;
                        s.connecting = false;
                        s.controlling_device = false;
                        for row in s.fleet.values_mut() {
                            row.connected = false;
                            row.connecting = false;
                            row.paused = true;
                            row.notice = "已暂停".into();
                        }
                    });
                    events.feedback("全部设备已暂停，用量读取继续运行", false, false);
                }
                Command::Refresh | Command::ReadUsage => {
                    manual_usage = true;
                    events.feedback("正在读取官方用量…", true, false);
                    schedule.request(Instant::now());
                    events
                        .update(|s| s.next_read = schedule.next_unix(Instant::now(), usage::now()));
                    if !schedule.due(Instant::now()) {
                        events.log("已排队读取。");
                        events.feedback(
                            "读取已排队，最短间隔结束后自动执行；无需重复点击。",
                            true,
                            false,
                        );
                    }
                }
                Command::ConfigureRefresh(settings) => {
                    if settings.valid() {
                        schedule.configure(settings, Instant::now());
                        events.update(|s| {
                            s.refresh = settings;
                            s.next_read = schedule.next_unix(Instant::now(), usage::now());
                        });
                        events.log(if settings.enabled {
                            "自动刷新设置已生效。"
                        } else {
                            "自动刷新已关闭，仍可手动读取和同步。"
                        });
                        events.feedback("设置已保存，刷新计划已生效。", false, false);
                    }
                }
                Command::Device(action) => {
                    if pending_control.is_some() {
                        events.feedback("设备正在更新，请稍候", true, false);
                        continue;
                    }
                    pending_usage_display = false;
                    events.update(|s| s.controlling_device = true);
                    events.feedback(format!("正在{}…", action.label()), true, false);
                    let result: Result<()> = async {
                        let l = link
                            .as_mut()
                            .ok_or_else(|| anyhow::anyhow!("请先连接墨水屏。"))?;
                        let response = l.device(&action).await?;
                        if matches!(action, crate::device::Action::Read) && !l.sample_supported {
                            let telemetry =
                                crate::device::Telemetry::parse(&response, usage::now())?;
                            events.update(|s| s.telemetry = Some(telemetry));
                            events.feedback("已读回设备状态，正在更新当天曲线…", true, false);
                            let history = if l.history_supported {
                                l.history().await?
                            } else {
                                None
                            };
                            last_history = Instant::now();
                            last_device = Instant::now();
                            let at = usage::now();
                            let time = chrono::Local::now().format("%H:%M:%S");
                            let message = format!(
                                "已读取 · 旧固件不支持主动采样 · {time}{}",
                                if history.is_none() && l.history_supported {
                                    " · 历史正在变化，稍后重读"
                                } else {
                                    ""
                                }
                            );
                            events.update(|s| {
                                if let Some(h) = history {
                                    s.history = Some(h);
                                }
                                s.device_read_at = Some(at);
                                s.device_notice = message.clone();
                                s.controlling_device = false;
                            });
                            events.feedback(message, false, false);
                        } else {
                            let baseline = if matches!(action, crate::device::Action::Read) {
                                response["sample"]
                                    .as_u64()
                                    .filter(|id| *id > 0 && *id <= u64::from(u32::MAX))
                                    .ok_or_else(|| anyhow::anyhow!("设备未确认采样请求"))?
                                    as u32
                            } else {
                                events
                                    .state
                                    .lock()
                                    .ok()
                                    .and_then(|s| {
                                        s.telemetry
                                            .as_ref()
                                            .and_then(|t| t.shown.as_ref().map(|f| f.6))
                                    })
                                    .unwrap_or(0)
                            };
                            pending_control = Some((action, baseline, Instant::now()));
                            last_device = Instant::now()
                                - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
                            events.feedback("设备已收到指令，等待屏幕刷新完成…", true, false);
                        }
                        Ok(())
                    }
                    .await;
                    if let Err(error) = result {
                        pending_control = None;
                        if let Some(l) = link.take() {
                            l.disconnect().await;
                        }
                        let message = format!("设备操作未完成：{error}");
                        events.update(|s| {
                            s.connected = false;
                            s.controlling_device = false;
                            s.device_notice = message.clone();
                        });
                        events.feedback(message, false, true);
                        reconnect = Instant::now() + Duration::from_secs(3);
                    }
                }
                Command::CheckCodex(settings) => {
                    events.feedback("正在检查 Codex…", true, false);
                    events.update(|s| s.checking_cli = true);
                    query_queue.push_back(QueryWork::Check(settings));
                }
                Command::RefreshCodex => {
                    events.feedback("正在读取 Codex 用量…", true, false);
                    events.update(|s| s.refreshing_codex = true);
                    query_queue.push_back(QueryWork::Codex);
                }
                Command::PairUsb => {
                    events.feedback("正在等待 USB 维护绑定…", true, false);
                    if let Some(l) = link.take() {
                        l.disconnect().await;
                    }
                    desired = None;
                    events.update(|s| {
                        s.connected = false;
                        s.connecting = false;
                    });
                    events.log("请在设备网络页双击 BOOT，正在等待 USB 绑定…");
                    let dir = root.clone();
                    match tokio::task::spawn_blocking(move || config::pair_usb(&dir)).await {
                        Ok(Ok(name)) => {
                            events.update(|s| s.bound_name = name);
                            events.log("USB 绑定完成，点击扫描后选择设备。");
                            events.feedback("USB 绑定完成，重新扫描后连接。", false, false);
                        }
                        Ok(Err(e)) => {
                            events.log(&e.to_string());
                            events.feedback(e.to_string(), false, true);
                        }
                        Err(_) => {
                            events.log("USB 绑定失败，请重试。");
                            events.feedback("USB 绑定失败，请重试。", false, true);
                        }
                    }
                }
                Command::PairBluetooth { id, code } => {
                    fleet.stop(&id).await;
                    reconnect_suspended = true;
                    events.update(|s| s.connection_error = None);
                    events.feedback("正在验证屏幕配对码…", true, false);
                    if let Some(l) = link.take() {
                        if let Some(old) = active_id.as_ref().and_then(|old| {
                            crate::registry::list(&root)
                                .ok()?
                                .into_iter()
                                .find(|b| &b.id == old)
                        }) {
                            fleet.attach(old, l, events.clone());
                        } else {
                            l.disconnect().await;
                        }
                    }
                    desired = None;
                    events.update(|s| {
                        s.connected = false;
                        s.connecting = false;
                        s.pairing = true;
                    });
                    events.log("正在通过屏幕配对码验证设备…");
                    let result = if let Some((d, p)) = devices.iter().find(|(d, _)| d.id == id) {
                        if let Some(a) = adapter.as_ref() {
                            pair_bluetooth(a, p, &root, &d.name, &code).await
                        } else {
                            Err(anyhow::anyhow!("蓝牙适配器不可用"))
                        }
                    } else {
                        Err(anyhow::anyhow!("请先选择附近的 EPaper 设备。"))
                    };
                    let safe_error = result
                        .as_ref()
                        .err()
                        .map(|error| format!("{error:#}").replace(code.as_str(), "[配对码]"));
                    drop(code);
                    match result {
                        Ok(name) => {
                            reconnect_suspended = false;
                            connect_feedback = true;
                            events.update(|s| s.bound_name = name.clone());
                            events.log("蓝牙认证已记住；等待设备恢复数据连接…");
                            tokio::time::sleep(Duration::from_secs(6)).await;
                            if let Some(a) = adapter.as_ref()
                                && let Ok(found) = scan(a, &root, Some(&events)).await
                            {
                                devices = found;
                                desired = devices
                                    .iter()
                                    .find(|(d, _)| d.name == name && !d.enrollment)
                                    .map(|(d, _)| d.id.clone());
                                events.update(|s| {
                                    s.devices = devices.iter().map(|(d, _)| d.clone()).collect()
                                });
                                backoff = 3;
                                reconnect = Instant::now();
                            }
                            events.log("首次认证完成，今后切换电脑无需重新配对。");
                            events.feedback("首次认证完成，正在恢复普通数据连接。", false, false);
                        }
                        Err(_) => {
                            let reason = safe_error.unwrap_or_else(|| "请检查配对窗口。".into());
                            events.update(|s| s.connection_error = Some(reason.clone()));
                            events.log(&format!("认证未完成：{reason}"));
                            events.feedback(
                                format!("认证未完成：{reason}；不会自动重试配对码。"),
                                false,
                                true,
                            );
                        }
                    }
                    events.update(|s| s.pairing = false);
                }
            }
        }
        // Process form edits/pause queued during scanning before any retry.
        if !commands.is_empty() {
            continue;
        }
        if !reconnect_suspended
            && link.is_none()
            && desired.is_some()
            && Instant::now() >= reconnect
        {
            let target = desired.as_ref().unwrap();
            events.update(|s| {
                s.connecting = true;
                s.connected = false;
            });
            events.log("正在连接并验证设备身份…");
            if let Some((d, p)) = devices.iter().find(|(d, _)| &d.id == target) {
                // CoreBluetooth removes the internal peripheral on disconnect. A cached
                // Rust Peripheral is not a reconnect handle: retrieve a live native one.
                let connection = async {
                    let a = adapter
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("蓝牙适配器不可用"))?;
                    let fresh = a
                        .retrieve_peripherals(RetrievePeripheralsOptions {
                            identifiers: Some(vec![p.id()]),
                            services: None,
                        })
                        .await?
                        .into_iter()
                        .find(|fresh| fresh.id() == p.id())
                        .ok_or_else(|| anyhow::anyhow!("设备暂不可用，请确认已通电或重新扫描"))?;
                    let directory = crate::registry::find(&root, &d.name)
                        .map(|b| b.directory)
                        .unwrap_or_else(|_| root.clone());
                    Link::connect(fresh, &directory, &d.name).await
                };
                let connection: Result<Link> = tokio::select! {
                    biased;
                    command = commands.recv() => {
                        deferred_command = Some(command.unwrap_or(Command::Quit));
                        let _ = p.disconnect().await;
                        events.update(|s| s.connecting = false);
                        continue;
                    }
                    result = connection => result,
                };
                match connection {
                    Ok(mut l) => {
                        let directory = crate::registry::find(&root, &d.name)
                            .map(|b| b.directory)
                            .unwrap_or_else(|_| root.clone());
                        let _ = config::remember_device(&directory, &d.name, &d.id);
                        active_id = Some(d.id.clone());
                        let supported = l.device_supported;
                        let (rotation_supported, history_supported) =
                            (l.rotation_supported, l.history_supported);
                        let telemetry = if supported {
                            l.telemetry().await.ok()
                        } else {
                            None
                        };
                        link = Some(l);
                        backoff = 3;
                        needs_sync = true;
                        last_ping = Instant::now();
                        sequence = None;
                        last_device =
                            Instant::now() - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
                        last_history =
                            Instant::now() - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
                        last_harness =
                            Instant::now() - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
                        events.update(|s| {
                            s.connected = true;
                            s.active_device = d.id.clone();
                            s.bound_name = d.name.clone();
                            s.fleet.insert(
                                d.id.clone(),
                                fleet::Status {
                                    id: d.id.clone(),
                                    name: d.name.clone(),
                                    model: config::pairing(&directory)
                                        .ok()
                                        .and_then(|p| p.model.clone())
                                        .unwrap_or_else(|| "3Inch7EPaper".into()),
                                    connected: true,
                                    notice: "已安全连接".into(),
                                    ..Default::default()
                                },
                            );
                            s.connecting = false;
                            s.device_supported = supported;
                            s.rotation_supported = rotation_supported;
                            s.history_supported = history_supported;
                            s.telemetry = telemetry;
                            s.history = None;
                            s.device_notice = if supported {
                                "温湿度每 30 秒采样，每 5 分钟回传。".into()
                            } else {
                                "当前固件仅支持用量同步；更新后可查看温湿度和切换屏幕。".into()
                            };
                        });
                        events.log("Security 2 已认证，蓝牙同步运行中。");
                        events.update(|s| s.connection_error = None);
                        events.feedback("蓝牙安全连接成功，正在同步设备数据。", false, false);
                        connect_feedback = false;
                    }
                    Err(e) => {
                        events.update(|s| {
                            if let Some(row) = s.fleet.get_mut(&d.id) {
                                row.connected = false;
                                row.connecting = false;
                                row.notice = "等待重连".into();
                            }
                        });
                        events.update(|s| s.connecting = false);
                        let mode_error = e.downcast_ref::<AuthModeError>().is_some();
                        events.update(|s| s.connection_error = Some(format!("{e:#}")));
                        events.log(&format!("连接未完成：{e}"));
                        if mode_error {
                            desired = None;
                            reconnect_suspended = true;
                        }
                        if connect_feedback || mode_error {
                            events.feedback(
                                if mode_error {
                                    e.to_string()
                                } else {
                                    "连接暂未成功，原因见连接页；后台会稍后重试。".into()
                                },
                                false,
                                true,
                            );
                        }
                        connect_feedback = false;
                        reconnect = Instant::now() + Duration::from_secs(backoff);
                        backoff = (backoff * 2).min(60);
                    }
                }
            } else {
                desired = None;
                events.update(|s| s.connecting = false);
                events.log("请重新扫描并选择设备。");
                events.feedback("设备不在当前列表，请重新扫描。", false, true);
            }
        }
        fleet.reconcile(&root, desired.as_deref(), &events).await;
        if query_task.is_none() {
            let work = query_queue.pop_front().or_else(|| {
                if schedule.due(Instant::now()) {
                    events.update(|s| {
                        s.refreshing_usage = true;
                        s.next_read = None;
                    });
                    Some(QueryWork::All)
                } else {
                    None
                }
            });
            if let Some(work) = work {
                query_task = Some(tokio::spawn(query_work(root.clone(), work)));
            }
        }
        if link.is_none() {
            sent_revision = None;
        } else if let Ok(s) = events.state.lock() {
            let merged = crate::hub::readings(&s);
            let revision =
                serde_json::to_vec(&merged.iter().map(|r| &r.packet).collect::<Vec<_>>()).ok();
            if !merged.is_empty() && revision != sent_revision {
                needs_sync = true;
                sent_revision = revision;
            }
        }
        if let Some(l) = link.as_mut() {
            let result: Result<()> = async {
                let weather = events
                    .state
                    .lock()
                    .map(|s| s.weather.clone())
                    .unwrap_or_default();
                l.sync_weather(&weather).await?;
                let (todos, poem) = events
                    .state
                    .lock()
                    .map(|s| (s.todos.clone(), s.poem.clone()))
                    .unwrap_or_default();
                l.sync_extras(&todos, poem.as_ref()).await?;
                let totals =
                    events.state.lock().ok().and_then(|s| {
                        crate::home::model::totals_wire(&s, crate::home::model::day())
                    });
                l.sync_totals(totals).await?;
                let places = events.state.lock().ok().and_then(|s| {
                    active_id
                        .as_deref()
                        .and_then(|id| crate::home::model::places_wire(&s, id))
                });
                l.sync_places(places).await?;
                if let Some(reading) = codex_to_send.take() {
                    sequence = Some(l.send_reading(&reading).await?);
                    last_ping = Instant::now();
                    request_usage_display = true;
                    events.update(|s| {
                        s.synced = Some(usage::now());
                        if let Some(id) = active_id.as_ref()
                            && let Some(row) = s.fleet.get_mut(id)
                        {
                            row.synced = s.synced;
                        }
                    });
                }
                if request_usage_display && pending_control.is_none() {
                    request_usage_display = false;
                    if l.sample_supported {
                        let reply = l.device(&crate::device::Action::Read).await?;
                        let id = reply["sample"]
                            .as_u64()
                            .filter(|id| *id > 0 && *id <= u64::from(u32::MAX))
                            .ok_or_else(|| anyhow::anyhow!("设备未确认更新请求"))?
                            as u32;
                        pending_control = Some((crate::device::Action::Read, id, Instant::now()));
                        pending_usage_display = true;
                        events.update(|s| s.controlling_device = true);
                        events.feedback("用量已同步，正在更新墨水屏…", true, false);
                        last_device =
                            Instant::now() - Duration::from_secs(crate::device::TELEMETRY_SECONDS);
                    }
                }
                if l.device_supported
                    && crate::device::telemetry_due(
                        last_device,
                        Instant::now(),
                        pending_control.is_some(),
                    )
                {
                    let telemetry = l.telemetry().await?;
                    if let Some((action, baseline, started)) = &pending_control {
                        let sampling = matches!(action, crate::device::Action::Read);
                        let sample_result = telemetry
                            .sample
                            .filter(|(id, _)| *id == *baseline)
                            .map(|(_, result)| result);
                        if sampling && sample_result.is_some_and(|result| result >= 3) {
                            let message = if sample_result == Some(3) {
                                "采样失败，请检查传感器"
                            } else {
                                "屏幕更新失败，请重试"
                            };
                            events.update(|s| {
                                s.controlling_device = false;
                                s.device_notice = message.into();
                            });
                            events.feedback(message, false, true);
                            pending_control = None;
                            pending_usage_display = false;
                        } else if action.completed(&telemetry, *baseline) {
                            let message = if sampling && pending_usage_display {
                                if sample_result == Some(2) {
                                    "用量已同步，画面无变化".into()
                                } else {
                                    "用量及墨水屏已更新".into()
                                }
                            } else if sampling {
                                let time = chrono::Local::now().format("%H:%M:%S");
                                if sample_result == Some(2) {
                                    format!("采样完成，画面无变化 · {time}")
                                } else {
                                    format!("采样并更新完成 · {time}")
                                }
                            } else {
                                format!("已完成：{}", action.label())
                            };
                            if sampling && l.history_supported {
                                if let Some(history) = l.history().await? {
                                    events.update(|s| s.history = Some(history));
                                }
                                last_history = Instant::now();
                            }
                            events.update(|s| {
                                if sampling {
                                    s.device_read_at = Some(usage::now());
                                }
                                s.controlling_device = false;
                                s.device_notice = message.clone();
                            });
                            events.feedback(message, false, false);
                            pending_control = None;
                            pending_usage_display = false;
                        } else if started.elapsed()
                            >= Duration::from_secs(if l.display_timeout > 35 {
                                l.display_timeout
                            } else if sampling {
                                35
                            } else {
                                18
                            })
                        {
                            events.update(|s| {
                                s.controlling_device = false;
                                s.device_notice = "屏幕尚未确认完成，请查看设备或重试。".into();
                            });
                            events.feedback("屏幕尚未确认完成，请查看设备或重试。", false, true);
                            pending_control = None;
                            pending_usage_display = false;
                        }
                    }
                    if let (Some(id), Some(t), Some(h)) =
                        (active_id.clone(), telemetry.temperature, telemetry.humidity)
                    {
                        let at = telemetry.sampled_at;
                        events.update(|s| {
                            if let Some(row) = s.fleet.get_mut(&id) {
                                row.climate = Some((t, h, at));
                            }
                            if crate::envlog::record(&mut s.env_log, &id, at, t, h) {
                                crate::envlog::save(&root, &s.env_log);
                            }
                        });
                    }
                    events.update(|s| s.telemetry = Some(telemetry));
                    last_device = Instant::now();
                    if l.history_supported
                        && last_history.elapsed()
                            >= Duration::from_secs(crate::device::TELEMETRY_SECONDS)
                    {
                        let changed = events
                            .state
                            .lock()
                            .map(|s| match (&s.telemetry, &s.history) {
                                (Some(t), Some(h)) => t.sd.1 != h.day || t.history != h.revision,
                                _ => true,
                            })
                            .unwrap_or(false);
                        if changed {
                            if let Some(history) = l.history().await? {
                                events.update(|s| {
                                    s.history = Some(history);
                                    if s.device_notice.is_empty() {
                                        s.device_notice =
                                            "设备数据已更新 · 采样 30 秒 / 回传 5 分钟".into();
                                    }
                                });
                            } else {
                                events.update(|s| {
                                    s.device_notice = "记录正在更新，下次读取时补齐曲线。".into()
                                });
                            }
                        }
                        last_history = Instant::now();
                    }
                }
                if needs_sync {
                    let r = events
                        .state
                        .lock()
                        .map(|s| crate::hub::readings(&s))
                        .unwrap_or_default();
                    let mut sent = false;
                    for reading in &r {
                        if reading.packet.p >= 3 && !l.profiles_supported {
                            continue;
                        }
                        let effective = events
                            .state
                            .lock()
                            .ok()
                            .map(|s| crate::harness::effective(reading, &s.harness_devices))
                            .unwrap_or_else(|| reading.clone());
                        sequence = Some(l.send_reading(&effective).await?);
                        sent = true;
                    }
                    if l.harness_supported
                        && (manual_usage
                            || last_harness.elapsed()
                                >= Duration::from_secs(crate::device::TELEMETRY_SECONDS))
                    {
                        last_harness = Instant::now();
                        match l.harness_devices().await {
                            Ok(devices) => {
                                let cached = events
                                    .state
                                    .lock()
                                    .map(|s| s.harness_devices.clone())
                                    .unwrap_or_default();
                                let merged = crate::harness::merge_devices(&cached, &devices);
                                // Legacy totals are observation-only: they cannot be restored as
                                // independent consumption once Harness conversations synchronize.
                                events.update(|s| {
                                    s.harness_devices = merged;
                                    if crate::harness::save_devices(&root, &s.harness_devices)
                                        .is_err()
                                    {
                                        s.device_notice = "电脑统计缓存未保存".into();
                                    }
                                });
                            }
                            Err(_) => events.log("多 Mac 用量暂未取回，将在下次同步重试。"),
                        }
                    }
                    needs_sync = false;
                    last_ping = Instant::now();
                    if sent {
                        events.update(|s| {
                            s.synced = Some(usage::now());
                            if let Some(id) = active_id.as_ref()
                                && let Some(row) = s.fleet.get_mut(id)
                            {
                                row.synced = s.synced;
                            }
                        });
                        events.log("已通过加密蓝牙同步；自动刷新按设置执行。");
                        if manual_usage {
                            manual_usage = false;
                            if l.sample_supported {
                                request_usage_display = true;
                                events.feedback("用量已同步，正在更新墨水屏…", true, false);
                            } else {
                                events.feedback("用量已发送", false, false);
                            }
                        }
                    }
                } else if last_ping.elapsed() >= Duration::from_secs(30) {
                    let seq = l.send(b"?").await?;
                    if sequence.is_some_and(|s| s != seq) {
                        schedule.request(Instant::now());
                        events.update(|s| {
                            s.next_read = schedule.next_unix(Instant::now(), usage::now())
                        });
                    }
                    sequence = Some(seq);
                    last_ping = Instant::now();
                }
                Ok(())
            }
            .await;
            if result.is_err() {
                events.update(|s| {
                    if let Some(id) = active_id.as_ref()
                        && let Some(row) = s.fleet.get_mut(id)
                    {
                        row.connected = false;
                        row.notice = "等待重连".into();
                    }
                });
                if manual_usage {
                    manual_usage = false;
                    events.feedback(
                        "用量已读取，但蓝牙同步未完成；正在等待重新连接。",
                        false,
                        true,
                    );
                }
                if pending_control.take().is_some() {
                    events.update(|s| s.controlling_device = false);
                    events.feedback("连接已断开，未能确认屏幕操作完成。", false, true);
                }
                if let Some(l) = link.take() {
                    l.disconnect().await;
                }
                events.update(|s| s.connected = false);
                reconnect = Instant::now() + Duration::from_secs(backoff);
                events.log("蓝牙连接中断，等待自动重连。");
            }
        }
    }
}
pub async fn smoke(root: &Path) -> Result<()> {
    let a = adapter().await?;
    let found = scan(&a, root, None).await?;
    for (d, _) in &found {
        println!("设备：{} · 已绑定={} · RSSI={:?}", d.name, d.bound, d.rssi);
    }
    let (d, p) = found
        .into_iter()
        .find(|(d, _)| d.bound)
        .ok_or_else(|| anyhow::anyhow!("未发现已绑定设备。"))?;
    let mut l = Link::connect(p, root, &d.name).await?;
    println!("Security 2 双向认证通过");
    let result: Result<()> = async {
        let readings = usage::collect(root).await;
        for r in readings {
            if r.packet.p >= 3 && !l.profiles_supported {
                continue;
            }
            l.send_reading(&r).await?;
            println!(
                "provider={} status={} source={} token_note={}",
                r.packet.p, r.packet.s, r.source, r.tokens.note
            );
        }
        Ok(())
    }
    .await;
    l.disconnect().await;
    result?;
    println!("真实快照加密发送并收到设备 ACK");
    Ok(())
}

#[cfg(test)]
mod enrollment_endpoint_tests {
    use super::*;
    fn group(endpoint: &str) -> HashMap<String, u8> {
        ["proto-ver", "prov-session", endpoint]
            .into_iter()
            .map(|name| (name.to_owned(), 1))
            .collect()
    }
    #[test]
    fn corebluetooth_cached_service_uuid_uses_actual_mode_endpoints() {
        let services = [(SERVICE_UUID.to_owned(), group("pair-device"))]
            .into_iter()
            .collect();
        assert!(select_mode_endpoints(services, ENROLLMENT_UUID, "pair-device").is_ok());
        let services = [(ENROLLMENT_UUID.to_owned(), group("usage-data"))]
            .into_iter()
            .collect();
        assert!(select_mode_endpoints(services, SERVICE_UUID, "usage-data").is_ok());
    }
    #[test]
    fn opposite_or_ambiguous_mode_is_rejected_before_authentication() {
        let services = [(SERVICE_UUID.to_owned(), group("usage-data"))]
            .into_iter()
            .collect();
        assert!(select_mode_endpoints(services, ENROLLMENT_UUID, "pair-device").is_err());
        let mut both = group("pair-device");
        both.insert("usage-data".into(), 1);
        assert!(
            select_mode_endpoints(
                [(SERVICE_UUID.to_owned(), both)].into_iter().collect(),
                ENROLLMENT_UUID,
                "pair-device"
            )
            .is_err()
        );
        assert!(
            select_mode_endpoints(
                [("unknown-service".to_owned(), group("pair-device"))]
                    .into_iter()
                    .collect(),
                ENROLLMENT_UUID,
                "pair-device"
            )
            .is_err()
        );
    }
}
