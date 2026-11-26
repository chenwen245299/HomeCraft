//! Native app lifecycle and commands. Visuals live in home/, service work in backend/usage.
use crate::{
    backend::{self, Command, Events, Shared, State},
    config::{self, Lock},
    control::{self, Control},
    home::{self, Home, Intent},
    usage,
};
use anyhow::Result;
use eframe::egui;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

enum Action {
    Updates,
    Show,
    Refresh,
    Disconnect,
    Quit,
}
#[derive(Default)]
pub struct Options {
    pub background: bool,
    pub auto_connect: bool,
    pub preview: bool,
    pub control: Option<PathBuf>,
}
struct App {
    updater: crate::update::Updater,
    background: bool,
    last_login_check: std::time::Instant,
    state: Shared,
    view: crate::view_state::Cache,
    tx: UnboundedSender<Command>,
    menu_rx: mpsc::Receiver<Action>,
    _tray: TrayIcon,
    _instance: Option<Lock>,
    quitting: bool,
    root: PathBuf,
    home: Home,
    material: bool,
    chrome_geometry: Option<(u32, u32, u64, bool, bool)>,
    chrome_updates: u64,
    opaque_window: bool,
    activated: bool,
    control: Option<Control>,
    pending_screenshot: Option<(PathBuf, mpsc::Sender<Value>, std::time::Instant)>,
    native_visible: bool,
    preview: bool,
    pending_input: Vec<egui::Event>,
}
fn tray_icon() -> Result<Icon> {
    let image = image::load_from_memory(include_bytes!(
        "../../assets/app-icon/homecraft-tray-44.png"
    ))?
    .to_rgba8();
    let (width, height) = image.dimensions();
    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}
impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        root: PathBuf,
        instance: Option<Lock>,
        options: Options,
    ) -> Result<Self> {
        let ctx = cc.egui_ctx.clone();
        if options.control.is_some() {
            eprintln!(
                "UI capture format: {:?}",
                cc.wgpu_render_state.as_ref().map(|r| r.target_format)
            );
        }
        home::theme::install(&ctx);
        let prefs = config::read_private::<home::model::Preferences>(&root.join(".home-ui.json"))
            .ok()
            .filter(|p| p.validate())
            .unwrap_or_default();
        let auto_connect = options.auto_connect || prefs.auto_connect;
        let mut home = Home::new(&ctx, prefs);
        if !options.preview {
            home.login_item = epaper_macos::login_item_status();
        }
        let repaint = ctx.clone();
        let updater = crate::update::Updater::new(
            home.prefs.updates,
            !options.preview,
            Arc::new(move || repaint.request_repaint()),
        );
        let state = Arc::new(Mutex::new(State {
            refresh: home.prefs.refresh,
            places: home::model::Places::from(&home.prefs),
            ..State::default()
        }));
        let view = crate::view_state::Cache::start(state.clone(), ctx.clone());
        let (tx, rx) = unbounded_channel();
        if !options.preview {
            let events = Events {
                state: state.clone(),
                view_changed: view.notifier(),
            };
            let dir = root.clone();
            std::thread::spawn(move || {
                match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt.block_on(backend::worker(dir, events, rx, auto_connect)),
                    Err(_) => events.update(|s| s.status = "后台运行时启动失败".into()),
                }
            });
        }
        let menu = Menu::new();
        let show = MenuItem::new("显示我的家庭", true, None);
        let refresh = MenuItem::new("立即同步", !options.preview, None);
        let disconnect = MenuItem::new("暂停全部设备", !options.preview, None);
        let quit = MenuItem::new("退出 HomeCraft", true, None);
        let updates = MenuItem::new("检查软件更新…", true, None);
        menu.append_items(&[
            &show,
            &refresh,
            &disconnect,
            &updates,
            &PredefinedMenuItem::separator(),
            &quit,
        ])?;
        let ids = (
            show.id().clone(),
            refresh.id().clone(),
            disconnect.id().clone(),
            quit.id().clone(),
            updates.id().clone(),
        );
        let (menu_tx, menu_rx) = mpsc::channel();
        let wake = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let action = if e.id == ids.0 {
                Action::Show
            } else if e.id == ids.1 {
                Action::Refresh
            } else if e.id == ids.2 {
                Action::Disconnect
            } else if e.id == ids.3 {
                Action::Quit
            } else if e.id == ids.4 {
                Action::Updates
            } else {
                return;
            };
            let _ = menu_tx.send(action);
            wake.request_repaint();
        }));
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(tray_icon()?)
            .with_icon_as_template(true)
            .with_tooltip("HomeCraft · 我的家庭")
            .build()?;
        let control = options
            .control
            .map(|dir| Control::start(&dir, ctx.clone()))
            .transpose()?;
        Ok(Self {
            updater,
            background: options.background,
            last_login_check: std::time::Instant::now(),
            state,
            view,
            tx,
            menu_rx,
            _tray: tray,
            _instance: instance,
            quitting: false,
            root,
            home,
            material: false,
            chrome_geometry: None,
            chrome_updates: 0,
            opaque_window: false,
            activated: false,
            control,
            pending_screenshot: None,
            native_visible: true,
            preview: options.preview,
            pending_input: Vec::new(),
        })
    }
    fn send(&self, c: Command) {
        if !self.preview {
            let _ = self.tx.send(c);
        }
    }
    fn state(&self) -> Arc<State> {
        self.view.snapshot()
    }
    fn intent(&mut self, intent: Intent, ctx: &egui::Context) {
        let starting = match &intent {
            Intent::Scan => Some("正在扫描附近设备…".into()),
            Intent::Connect(_) => Some("正在连接墨水屏…".into()),
            Intent::Disconnect => Some("正在暂停连接…".into()),
            Intent::Sync => Some("正在安排读取官方用量…".into()),
            Intent::CheckCodex(_) => Some("正在检测 Codex CLI…".into()),
            Intent::RefreshCodex => Some("正在刷新 Codex 用量…".into()),
            Intent::Pair | Intent::PairBluetooth { .. } => Some("正在进行安全认证…".into()),
            Intent::Device(action) => Some(format!("正在{}…", action.label())),
            _ => None,
        };
        if let Ok(mut state) = self.state.lock() {
            if let Some(message) = starting {
                state.feedback = Some(crate::feedback::Feedback::new(
                    if self.preview {
                        "预览模式，不执行设备或账户操作。".into()
                    } else {
                        message
                    },
                    !self.preview,
                    false,
                ));
            }
            if !self.preview {
                match &intent {
                    Intent::Scan => state.scanning = true,
                    Intent::Connect(_) => state.connecting = true,
                    Intent::Device(_) => state.controlling_device = true,
                    Intent::PairBluetooth { .. } => state.pairing = true,
                    _ => {}
                }
            }
        }
        let local_result = matches!(
            &intent,
            Intent::InstallClaude
                | Intent::PickCodex
                | Intent::PickNode
                | Intent::PickCloud
                | Intent::SavePreferences
                | Intent::SetLoginItem(_)
                | Intent::OpenProject
        );
        match intent {
            Intent::Scan => self.send(Command::Scan),
            Intent::Connect(id) => self.send(Command::Connect(id)),
            Intent::ConnectionForm { active, disconnect } => {
                self.send(Command::ConnectionForm {
                    active,
                    disconnect,
                    target: (!self.home.selected.is_empty()).then(|| self.home.selected.clone()),
                });
            }
            Intent::DismissFeedback => {
                if let Ok(mut state) = self.state.lock() {
                    state.feedback = None;
                }
            }
            Intent::Disconnect => {
                if !self.home.selected.is_empty() {
                    self.send(Command::PauseDevice(self.home.selected.clone()));
                } else {
                    self.send(Command::Disconnect);
                }
            }
            Intent::Sync => self.send(if self.state().connected {
                Command::Refresh
            } else {
                Command::ReadUsage
            }),
            Intent::Pair => self.send(Command::PairUsb),
            Intent::PairBluetooth { id, code } => self.send(Command::PairBluetooth { id, code }),
            Intent::InstallClaude => {
                self.home.notice = if self.preview {
                    "预览模式不修改账户设置。".into()
                } else {
                    match usage::install_statusline() {
                        Ok(()) => "已启用；重启 Claude Code 会话后生效。".into(),
                        Err(e) => e.to_string(),
                    }
                };
            }
            Intent::CheckCodex(settings) => {
                if self.preview {
                    self.home.notice = "预览模式不启动外部CLI；正式App可检测本机安装。".into();
                } else {
                    if let Ok(mut state) = self.state.lock() {
                        state.checking_cli = true;
                    }
                    self.send(Command::CheckCodex(settings));
                }
            }
            Intent::RefreshCodex => {
                if self.preview {
                    self.home.notice = "预览模式不读取账户；正式 App 可刷新 Codex 用量。".into();
                } else {
                    if let Ok(mut state) = self.state.lock() {
                        state.refreshing_codex = true;
                    }
                    self.send(Command::RefreshCodex);
                }
            }
            Intent::Device(action) => self.send(Command::Device(action)),
            Intent::PickCodex => {
                if let Some(path) = epaper_macos::choose_executable("选择 Codex CLI 可执行文件")
                {
                    self.home.prefs.codex_cli = path.to_string_lossy().into();
                    self.home.notice = "已选择CLI；请点击检测当前设置，再保存设置。".into();
                } else {
                    self.home.notice = "已取消选择，Codex 路径未更改。".into();
                }
            }
            Intent::PickNode => {
                if let Some(path) = epaper_macos::choose_executable("选择 Node.js 可执行文件")
                {
                    self.home.prefs.codex_node = path.to_string_lossy().into();
                    self.home.notice = "已选择Node.js；请检测并保存设置。".into();
                } else {
                    self.home.notice = "已取消选择，Node 路径未更改。".into();
                }
            }
            Intent::PickCloud => {
                self.home.notice = if self.preview {
                    "预览模式不访问 iCloud。".into()
                } else if let Some(path) = epaper_macos::choose_sync_directory() {
                    if epaper_macos::is_icloud_directory(&path) {
                        self.send(Command::Cloud(crate::cloud::Command::Configure(Some(path))));
                        "已选择，正在同步。".into()
                    } else {
                        "请选择 iCloud Drive 中的文件夹。".into()
                    }
                } else {
                    "已取消选择。".into()
                };
            }
            Intent::Todo(command) => {
                if self.preview {
                    self.home.notice = "预览模式不保存待办。".into();
                } else {
                    self.send(Command::Todo(command));
                }
            }
            Intent::Cloud(command) => {
                if self.preview {
                    self.home.notice = "预览模式不写入 iCloud。".into();
                } else {
                    self.send(Command::Cloud(command));
                }
            }
            Intent::OpenProject => {
                self.home.notice = match std::process::Command::new("/usr/bin/open")
                    .arg(&self.root)
                    .spawn()
                {
                    Ok(_) => "已在 Finder 打开本机配置目录。".into(),
                    Err(_) => "打开配置目录失败。".into(),
                };
            }
            Intent::SavePreferences => {
                if !self.home.prefs.validate() {
                    self.home.notice =
                        "名称需 1–32 字；刷新间隔需 30 秒–120 分钟；CLI 需为绝对路径。".into();
                } else if self.preview {
                    self.updater.configure(self.home.prefs.updates);
                    if let Ok(mut state) = self.state.lock() {
                        state.refresh = self.home.prefs.refresh;
                        state.places = home::model::Places::from(&self.home.prefs);
                    }
                    self.home.notice = "预览设置已应用，不写入日常配置。".into();
                    self.home.mark_preferences_saved();
                } else {
                    self.home.notice = match self.home.prefs.save(&self.root) {
                        Ok(()) => {
                            self.updater.configure(self.home.prefs.updates);
                            if let Ok(mut state) = self.state.lock() {
                                state.places = home::model::Places::from(&self.home.prefs);
                            }
                            self.send(Command::ConfigureRefresh(self.home.prefs.refresh));
                            self.home.mark_preferences_saved();
                            "设置已保存，刷新计划已更新。".into()
                        }
                        Err(_) => "设置保存失败，请检查目录权限。".into(),
                    };
                }
            }
            Intent::SetLoginItem(enabled) => {
                let result = if self.preview {
                    Ok(if enabled {
                        epaper_macos::LoginItemStatus::Enabled
                    } else {
                        epaper_macos::LoginItemStatus::Disabled
                    })
                } else {
                    epaper_macos::set_login_item(enabled)
                };
                self.home.notice = match result {
                    Ok(status) => {
                        self.home.login_item = status;
                        match status {
                            epaper_macos::LoginItemStatus::RequiresApproval => {
                                "已登记，请在系统“登录项”中允许 HomeCraft。"
                            }
                            epaper_macos::LoginItemStatus::Enabled => "已开启开机自启动。",
                            _ => "已关闭开机自启动。",
                        }
                        .into()
                    }
                    Err(error) => {
                        self.home.login_item = epaper_macos::login_item_status();
                        error
                    }
                };
            }
            Intent::OpenLoginItems => {
                if !self.preview {
                    epaper_macos::open_login_items();
                }
            }
            Intent::CheckUpdates => {
                self.home.activate("update.settings");
                if !self.preview {
                    self.updater.check(true);
                }
            }
            Intent::DownloadUpdate => {
                if !self.preview {
                    self.updater.download();
                }
            }
            Intent::LaterUpdate => {
                self.updater.later();
                self.home.activate("sheet.close");
            }
            Intent::OpenReleases => {
                if !self.preview {
                    let _ = std::process::Command::new("/usr/bin/open")
                        .arg(crate::update::releases_url())
                        .spawn();
                }
            }
            Intent::InstallUpdate => {
                if !self.preview && matches!(self.updater.status, crate::update::Status::Ready(_)) {
                    match self.updater.install(true) {
                        Ok(()) => self.intent(Intent::Quit, ctx),
                        Err(error) => {
                            self.updater.status = crate::update::Status::Failed(error.to_string())
                        }
                    }
                }
            }
            Intent::Quit => {
                if !self.preview
                    && let Err(error) = self.updater.install(false)
                {
                    self.updater.status = crate::update::Status::Failed(error.to_string());
                    self.home.activate("update.settings");
                    return;
                }
                self.quitting = true;
                self.send(Command::Quit);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if local_result && let Ok(mut state) = self.state.lock() {
            let error = self.home.notice.contains("失败") || self.home.notice.contains("名称需");
            state.feedback = Some(crate::feedback::Feedback::new(
                self.home.notice.clone(),
                false,
                error,
            ));
        }
        self.view.invalidate();
        ctx.request_repaint();
    }
    fn control(&mut self, req: control::Request, ctx: &egui::Context) {
        let method = req.value["method"].as_str().unwrap_or("");
        let params = &req.value["params"];
        let result: Result<Value> = (|| {
            Ok(match method {
                "state.inspect" => {
                    let state = self.state();
                    let mut v = json!({"page":self.home.nav.id(),"sheet":format!("{:?}",self.home.sheet),"connected":state.connected,"connecting":state.connecting,"pairing":state.pairing,"connection_error":state.connection_error,"bound_name":state.bound_name,"devices":state.devices.iter().map(|d|json!({"id":d.id,"name":d.name,"bound":d.bound,"enrollment":d.enrollment,"recovered":d.recovered})).collect::<Vec<_>>(),"preview":self.preview,"synced":state.synced,"providers":state.readings.iter().map(|r|json!({"id":r.packet.p,"status":r.packet.s,"d":r.packet.d,"wt":r.packet.wt,"mt":r.packet.mt})).collect::<Vec<_>>(),"harness_devices":state.harness_devices,"harness_local_day":state.readings.iter().find(|r|r.packet.p==4).and_then(|r|r.packet.d),"argus_day":state.readings.iter().find(|r|r.packet.p==3).and_then(|r|r.packet.d),"argus_breakdown":state.readings.iter().find(|r|r.packet.p==3).and_then(|r|r.tokens.breakdown.as_ref()),"checking_cli":state.checking_cli,"refreshing_codex":state.refreshing_codex,"refreshing_usage":state.refreshing_usage,"refresh":state.refresh,"draft_refresh":self.home.prefs.refresh,"last_read":state.last_read,"next_read":state.next_read,"cli_diagnostic":state.cli_diagnostic,"notice":self.home.notice,"device_supported":state.device_supported,"telemetry":state.telemetry,"history":state.history,"device_notice":state.device_notice,"feedback":state.feedback,"device_read_at":state.device_read_at,"controlling_device":state.controlling_device,"material":self.material,"targets":self.home.targets,"visible":self.native_visible,"dock_visible":epaper_macos::dock_visible(),"minimized":ctx.input(|i|i.viewport().minimized),"focused":ctx.input(|i|i.viewport().focused),"size":[ctx.content_rect().width(),ctx.content_rect().height()]});
                    // Per-display status, weather and declined controls for multi-device checks.
                    v["fleet"] = state.fleet.values().map(|r| json!({"id":r.id,"name":r.name,"model":r.model,"connected":r.connected,"connecting":r.connecting,"paused":r.paused,"synced":r.synced,"notice":r.notice})).collect();
                    v["active_device"] = json!(state.active_device);
                    v["environment_view"] = self.home.environment_inspect();
                    v["native_chrome_updates"] = json!(self.chrome_updates);
                    v["native_window_opaque"] = json!(self.opaque_window);
                    v["home_totals"] =
                        crate::home::model::totals_wire(&state, crate::home::model::day())
                            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                            .unwrap_or(Value::Null);
                    v["weather"] = json!(state.weather);
                    v["login_item"] = json!(format!("{:?}", self.home.login_item));
                    v["software_update"] = json!({"status":self.updater.status.summary(),"busy":self.updater.status.busy(),"current_version":env!("CARGO_PKG_VERSION"),"current_build":crate::update::current_build(),"settings":self.updater.settings});
                    v["cloud"] = json!({"configured":state.cloud.folder.is_some(),"busy":state.cloud.busy,"last":state.cloud.last,"message":state.cloud.message,"error":state.cloud.error,"snapshots":state.cloud_data.snapshots.len()});
                    v["weather_notice"] = json!(state.weather_notice);
                    v["capabilities"] = json!({"rotation":state.rotation_supported,"history":state.history_supported});
                    v
                }
                "ui.activate" => {
                    anyhow::ensure!(
                        self.home.activate(params["id"].as_str().unwrap_or("")),
                        "未知界面动作"
                    );
                    ctx.request_repaint();
                    json!("ok")
                }
                "device.action" => {
                    let action = match params["op"].as_str().unwrap_or("") {
                        "sample" => crate::device::Action::Read,
                        "refresh" => crate::device::Action::Refresh,
                        "page" => crate::device::Action::Page(
                            params["page"].as_str().unwrap_or("").into(),
                        ),
                        "rotate" if params["rotation"] == "auto" => {
                            crate::device::Action::Rotate(None)
                        }
                        "rotate" => crate::device::Action::Rotate(Some(
                            params["rotation"]
                                .as_u64()
                                .filter(|n| *n < 4)
                                .ok_or_else(|| anyhow::anyhow!("无效方向"))?
                                as u8,
                        )),
                        _ => anyhow::bail!("未知设备操作"),
                    };
                    action.request()?;
                    anyhow::ensure!(!self.preview && self.state().connected, "设备未连接");
                    self.intent(home::Intent::Device(action), ctx);
                    json!("queued")
                }
                "ui.pointer" | "ui.hover" => {
                    let x = params["x"].as_f64().unwrap_or(-1.) as f32;
                    let y = params["y"].as_f64().unwrap_or(-1.) as f32;
                    let pos = egui::pos2(x, y);
                    anyhow::ensure!(ctx.content_rect().contains(pos), "点击坐标超出窗口");
                    self.pending_input.push(egui::Event::PointerMoved(pos));
                    for pressed in [true, false].into_iter().filter(|_| method == "ui.pointer") {
                        self.pending_input.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        });
                    }
                    ctx.request_repaint();
                    json!("ok")
                }
                "ui.close" => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    json!("ok")
                }
                "ui.resize" => {
                    let w = params["width"].as_f64().unwrap_or(0.);
                    let h = params["height"].as_f64().unwrap_or(0.);
                    anyhow::ensure!(
                        (850.0..=1800.).contains(&w) && (620.0..=1200.).contains(&h),
                        "窗口尺寸超出测试范围"
                    );
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        w as f32, h as f32,
                    )));
                    json!("ok")
                }
                "ui.theme" => {
                    self.home.prefs.dusk = params["dusk"] == true;
                    ctx.request_repaint();
                    json!("ok")
                }
                "ui.show" => {
                    epaper_macos::activate();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    json!("ok")
                }
                "ui.hide" => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    epaper_macos::set_dock_visible(false);
                    json!("ok")
                }
                "ui.minimize" => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(
                        params["minimized"] == true,
                    ));
                    json!("ok")
                }
                "ui.screenshot" => {
                    anyhow::ensure!(self.pending_screenshot.is_none(), "截图尚未完成");
                    let path = self
                        .control
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("控制通道未启用"))?
                        .image_path(params["name"].as_str().unwrap_or(""))?;
                    self.pending_screenshot =
                        Some((path, req.reply.clone(), std::time::Instant::now()));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                    return Ok(Value::Null);
                }
                "app.quit" => {
                    self.intent(Intent::Quit, ctx);
                    json!("ok")
                }
                _ => anyhow::bail!("未知控制命令"),
            })
        })();
        if method == "ui.screenshot" && result.as_ref().is_ok_and(Value::is_null) {
            return;
        }
        let answer = match result {
            Ok(v) => json!({"ok":true,"result":v}),
            Err(e) => json!({"ok":false,"error":e.to_string()}),
        };
        let _ = req.reply.send(answer);
    }
}
impl eframe::App for App {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        input.events.append(&mut self.pending_input);
    }
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [24. / 255., 39. / 255., 48. / 255., 1.]
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.activated {
            if self.background || epaper_macos::launched_at_login() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                epaper_macos::set_dock_visible(false);
            } else {
                epaper_macos::activate();
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            self.activated = true;
        }
        if !self.preview && self.last_login_check.elapsed() >= std::time::Duration::from_secs(5) {
            self.last_login_check = std::time::Instant::now();
            self.home.login_item = epaper_macos::login_item_status();
        }
        self.updater.tick();
        self.home.updates = self.updater.status.clone();
        if self.updater.show && self.home.sheet == crate::home::model::Sheet::None {
            self.home.activate("update.settings");
            self.updater.show = false;
        }
        if self.home.sheet == crate::home::model::Sheet::Update {
            self.updater.show = false;
        }
        if self.updater.status.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        } else if !self.preview {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        self.native_visible = frame
            .winit_window()
            .and_then(|w| w.is_visible())
            .unwrap_or(true);
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        // AppKit may relayout its titlebar after setFrame. Do not move native views
        // again on every data repaint; only update when the window geometry changes.
        let geometry = frame.winit_window().map(|w| {
            let size = w.inner_size();
            (
                size.width,
                size.height,
                w.scale_factor().to_bits(),
                fullscreen,
                self.native_visible,
            )
        });
        if !self.material || geometry != self.chrome_geometry {
            self.material = epaper_macos::update(frame, self.material, fullscreen);
            self.chrome_geometry = geometry;
            self.chrome_updates += 1;
            self.opaque_window = epaper_macos::is_opaque(frame);
        }
        while let Ok(action) = self.menu_rx.try_recv() {
            match action {
                Action::Show => {
                    epaper_macos::activate();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Action::Refresh => self.intent(Intent::Sync, ctx),
                Action::Disconnect => self.send(Command::DisconnectAll),
                Action::Quit => self.intent(Intent::Quit, ctx),
                Action::Updates => {
                    epaper_macos::activate();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    self.intent(Intent::CheckUpdates, ctx);
                }
            }
        }
        if !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            epaper_macos::set_dock_visible(false);
        }
        let requests = self
            .control
            .as_ref()
            .map(|c| c.rx.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for request in requests {
            self.control(request, ctx);
        }
        let screenshot = ctx.input(|i| {
            i.raw.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = screenshot
            && let Some((path, reply, _)) = self.pending_screenshot.take()
        {
            let bytes = image
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect::<Vec<_>>();
            let result = image::save_buffer(
                &path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            );
            let _ = reply.send(match result {
                Ok(()) => json!({"ok":true,"result":{"path":path,"size":image.size}}),
                Err(e) => json!({"ok":false,"error":e.to_string()}),
            });
        }
        if let Some((_, _, at)) = &self.pending_screenshot {
            if at.elapsed() > std::time::Duration::from_secs(8) {
                if let Some((_, reply, _)) = self.pending_screenshot.take() {
                    let _ = reply.send(json!({"ok":false,"error":"窗口截图超时"}));
                }
            } else {
                if let Some(render) = frame.wgpu_render_state() {
                    let _ = render.device.poll(eframe::wgpu::PollType::Poll);
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let state = self.state();
        if self.home.selected.is_empty()
            && let Some(device) = state.devices.iter().find(|d| d.bound)
        {
            self.home.selected = device.id.clone();
        }
        let mut intents = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                intents = self.home.render(ui, &state);
            });
        for intent in intents {
            self.intent(intent, ui.ctx());
        }
        if self.preview {
            let t = home::theme::Tokens::home();
            let r = ui.ctx().content_rect();
            ui.painter().text(
                r.right_bottom() - egui::vec2(20., 16.),
                egui::Align2::RIGHT_BOTTOM,
                "界面预览 · 未连接设备",
                home::theme::font(11.),
                t.secondary,
            );
        }
    }
    fn on_exit(&mut self) {
        self.send(Command::Quit);
    }
}
pub fn run(root: PathBuf, options: Options) -> Result<()> {
    let instance = if options.preview {
        None
    } else {
        Some(Lock::take(&root, ".ble-app.lock")?)
    };
    let viewport = egui::ViewportBuilder::default()
        .with_title("HomeCraft")
        .with_app_id("local.epaper.bluetooth")
        .with_inner_size([1100., 780.])
        .with_icon(Arc::new({
            let bytes = include_bytes!("../../assets/app-icon/homecraft-128.png");
            let image = image::load_from_memory(bytes)?.to_rgba8();
            egui::IconData {
                width: image.width(),
                height: image.height(),
                rgba: image.into_raw(),
            }
        }))
        .with_min_inner_size([850., 620.])
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false)
        .with_transparent(false);
    let native = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "HomeCraft",
        native,
        Box::new(move |cc| Ok(Box::new(App::new(cc, root, instance, options)?))),
    )
    .map_err(|e| anyhow::anyhow!("无法打开窗口：{e}"))
}
