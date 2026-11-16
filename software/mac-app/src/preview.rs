//! Offscreen visual audit using the production egui widgets, fonts and GPU renderer.
//! This entry point never starts App, Bluetooth, account clients or persistence.
use crate::{
    backend::State,
    home::{
        Home,
        model::{Nav, Preferences, Sheet, Tab},
        theme,
    },
    usage,
};
use anyhow::{Result, ensure};
use eframe::{egui, egui_wgpu, wgpu};
use std::{path::Path, sync::mpsc, time::Duration};

pub async fn render(directory: &Path) -> Result<()> {
    ensure!(directory.is_absolute(), "预览输出目录必须是绝对路径");
    std::fs::create_dir_all(directory)?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await?;
    let mut manifest = Vec::new();
    let mut layout_manifest = Vec::new();
    for dusk in [false, true] {
        for (name, nav, sheet, tab) in [
            ("home", Nav::Home, Sheet::None, Tab::Overview),
            ("home-connected", Nav::Home, Sheet::None, Tab::Overview),
            ("home-empty", Nav::Home, Sheet::None, Tab::Overview),
            ("home-stale", Nav::Home, Sheet::None, Tab::Overview),
            ("home-large", Nav::Home, Sheet::None, Tab::Overview),
            ("automation", Nav::Automation, Sheet::None, Tab::Overview),
            ("discover", Nav::Discover, Sheet::None, Tab::Overview),
            ("climate", Nav::Climate, Sheet::None, Tab::Overview),
            ("climate-hover", Nav::Climate, Sheet::None, Tab::Overview),
            (
                "climate-details",
                Nav::Climate,
                Sheet::Environment,
                Tab::Overview,
            ),
            ("climate-loading", Nav::Climate, Sheet::None, Tab::Overview),
            (
                "climate-disconnected",
                Nav::Climate,
                Sheet::None,
                Tab::Overview,
            ),
            ("harness", Nav::Usage, Sheet::None, Tab::Overview),
            ("harness-bottom", Nav::Usage, Sheet::None, Tab::Overview),
            ("displays", Nav::Displays, Sheet::None, Tab::Overview),
            ("usage", Nav::Usage, Sheet::None, Tab::Overview),
            ("room", Nav::Room, Sheet::None, Tab::Overview),
            ("device", Nav::Home, Sheet::Device, Tab::Overview),
            ("device-usage", Nav::Home, Sheet::Device, Tab::Usage),
            (
                "device-connection",
                Nav::Home,
                Sheet::Device,
                Tab::Connection,
            ),
            ("connection", Nav::Home, Sheet::Add, Tab::Connection),
            (
                "connection-remembered",
                Nav::Home,
                Sheet::Add,
                Tab::Connection,
            ),
            (
                "connection-recovered",
                Nav::Home,
                Sheet::Add,
                Tab::Connection,
            ),
            ("settings", Nav::Home, Sheet::Settings, Tab::Overview),
            ("updates", Nav::Home, Sheet::Update, Tab::Overview),
            (
                "updates-downloading",
                Nav::Home,
                Sheet::Update,
                Tab::Overview,
            ),
            ("updates-error", Nav::Home, Sheet::Update, Tab::Overview),
            ("help", Nav::Home, Sheet::Help, Tab::Overview),
            ("cloud-empty", Nav::Home, Sheet::Cloud, Tab::Overview),
            ("cloud", Nav::Home, Sheet::Cloud, Tab::Overview),
            ("cloud-bottom", Nav::Home, Sheet::Cloud, Tab::Overview),
            ("cloud-loading", Nav::Home, Sheet::Cloud, Tab::Overview),
            ("cloud-error", Nav::Home, Sheet::Cloud, Tab::Overview),
            ("cloud-fallback", Nav::Climate, Sheet::None, Tab::Overview),
            ("codex-settings", Nav::Home, Sheet::Codex, Tab::Overview),
            ("settings-bottom", Nav::Home, Sheet::Settings, Tab::Overview),
            ("device-usage-bottom", Nav::Home, Sheet::Device, Tab::Usage),
            (
                "connection-populated",
                Nav::Home,
                Sheet::Add,
                Tab::Connection,
            ),
            ("connection-bottom", Nav::Home, Sheet::Add, Tab::Connection),
            ("usage-error", Nav::Usage, Sheet::None, Tab::Overview),
            ("usage-loading", Nav::Usage, Sheet::None, Tab::Overview),
            ("codex-loading", Nav::Usage, Sheet::None, Tab::Overview),
            ("usage-empty", Nav::Usage, Sheet::None, Tab::Overview),
            ("displays-empty", Nav::Displays, Sheet::None, Tab::Overview),
            ("home-long", Nav::Home, Sheet::None, Tab::Overview),
            ("settings-dirty", Nav::Home, Sheet::Settings, Tab::Overview),
            (
                "refresh-settings",
                Nav::Usage,
                Sheet::Refresh,
                Tab::Overview,
            ),
            (
                "refresh-disabled",
                Nav::Usage,
                Sheet::Refresh,
                Tab::Overview,
            ),
            ("refresh-dirty", Nav::Usage, Sheet::Refresh, Tab::Overview),
            ("settings-child", Nav::Home, Sheet::Settings, Tab::Overview),
            ("usage-all", Nav::Usage, Sheet::None, Tab::Overview),
            ("usage-all-middle", Nav::Usage, Sheet::None, Tab::Overview),
            ("usage-all-bottom", Nav::Usage, Sheet::None, Tab::Overview),
            ("todo-quadrant", Nav::Todo, Sheet::None, Tab::Overview),
            ("todo-timeline", Nav::Todo, Sheet::None, Tab::Overview),
            ("todo-calendar", Nav::Todo, Sheet::None, Tab::Overview),
            ("todo-edit", Nav::Todo, Sheet::Todo, Tab::Overview),
            ("todo-new", Nav::Todo, Sheet::Todo, Tab::Overview),
            ("weather-settings", Nav::Home, Sheet::Weather, Tab::Overview),
            ("home-hub", Nav::Home, Sheet::None, Tab::Overview),
            ("climate-places", Nav::Climate, Sheet::None, Tab::Overview),
            (
                "climate-places-hover",
                Nav::Climate,
                Sheet::None,
                Tab::Overview,
            ),
            ("displays-poem", Nav::Displays, Sheet::None, Tab::Overview),
        ] {
            // HOMECRAFT_PREVIEW_FILTER=todo,weather renders only matching scenarios.
            if let Ok(filter) = std::env::var("HOMECRAFT_PREVIEW_FILTER")
                && !filter.split(',').any(|f| name.contains(f.trim()))
            {
                continue;
            }
            for (width, height) in [(1100, 780), (850, 620), (1440, 900)] {
                let ctx = egui::Context::default();
                theme::install(&ctx);
                let mut home = Home::new(
                    &ctx,
                    Preferences {
                        dusk,
                        ..Preferences::default()
                    },
                );
                home.nav = nav;
                home.sheet = sheet;
                home.tab = tab;
                if name.starts_with("updates") {
                    let release=crate::update::Release {version:"0.3.2".into(),build:50,
                        notes:"## 更新说明\n- 改进多来源同步和离线显示。\n- 保留本机设置及设备绑定。\n\n本次只更新 HomeCraft App。".into(),
                        asset:crate::update::Asset {url:String::new(),sha256:"0".repeat(64),size:16_000_000}};
                    home.updates = match name {
                        "updates-downloading" => crate::update::Status::Downloading {
                            release,
                            done: 6_400_000,
                            total: 16_000_000,
                        },
                        "updates-error" => {
                            crate::update::Status::Failed("无法连接更新服务，请稍后重试".into())
                        }
                        _ => crate::update::Status::Available(release),
                    };
                }
                let mut state = State {
                    last_read: Some(usage::now()),
                    next_read: Some(usage::now() + 300),
                    ..State::default()
                };
                let mut codex = usage::empty(0, usage::OK, "界面样例数据；不查询真实账户");
                codex.packet.w[0] = [670, 300, 0];
                codex.packet.w[1] = [880, 10080, 0];
                let mut argus = usage::empty(3, usage::OK, "界面样例数据；不查询真实文献库");
                argus.packet.n = Some("Argus".into());
                argus.packet.d = Some([crate::home::model::day(), 266200]);
                argus.tokens.weekly = Some(40700000);
                let week = crate::home::model::week_start(crate::home::model::day()).unwrap();
                argus.packet.wt = Some([week, 40_700_000]);
                argus.packet.mt = Some([crate::home::model::day() / 100 * 100 + 1, 120_700_000]);
                codex.packet.d = Some([crate::home::model::day(), -1]);
                codex.packet.wt = Some([week, 118_459_404]);
                codex.packet.mt = Some([crate::home::model::day() / 100 * 100 + 1, 230_000_000]);
                codex.tokens.account = crate::account::Identity::parse(
                    Some(
                        &serde_json::json!({"account":{"type":"chatgpt","email":"sample@example.test","planType":"plus"}}),
                    ),
                    crate::harness::Origin {
                        id: "1111111111111111".into(),
                        name: "Mac-1".into(),
                    },
                );
                if name.starts_with("cloud") && name != "cloud-empty" {
                    state.cloud.folder =
                        Some(std::path::PathBuf::from("/sample/iCloud Drive/HomeCraft"));
                    state.cloud.message = "已写入 iCloud Drive".into();
                    state.cloud.last = Some(usage::now());
                    let mut remote = codex.clone();
                    remote.tokens.account = crate::account::Identity::parse(
                        Some(
                            &serde_json::json!({"account":{"type":"chatgpt","email":"other@example.test","planType":"pro"}}),
                        ),
                        crate::harness::Origin {
                            id: "2222222222222222".into(),
                            name: "Mac-2".into(),
                        },
                    );
                    state.cloud_data.snapshots.push(crate::cloud::Snapshot {
                        version: 1,
                        origin: crate::harness::Origin {
                            id: "2222222222222222".into(),
                            name: "Mac-2".into(),
                        },
                        day: crate::home::model::day(),
                        usages: vec![crate::cloud::Summary {
                            packet: remote.packet,
                            account: remote.tokens.account,
                            lifetime: Some(180_000_000),
                            peak_daily: None,
                            context: None,
                            breakdown: None,
                        }],
                        environment: Some(crate::cloud::Environment {
                            device: "EPaper_123ABC".into(),
                            sampled_at: usage::now(),
                            temperature: Some(23.4),
                            humidity: Some(48.2),
                            history: Some(crate::device::History {
                                day: crate::home::model::day() as u32,
                                revision: 1,
                                received_at: usage::now(),
                                points: (0..96)
                                    .map(|i| {
                                        Some((
                                            22. + (i as f32 / 12.).sin(),
                                            45. + (i as f32 / 10.).cos() * 3.,
                                        ))
                                    })
                                    .collect(),
                            }),
                        }),
                        displays: vec![crate::cloud::Environment {
                            device: "EPaper_EC240E".into(),
                            sampled_at: usage::now() - 120,
                            temperature: Some(28.6),
                            humidity: Some(51.3),
                            history: None,
                        }],
                    });
                }
                state.readings = vec![codex, argus];
                if name == "cloud-empty" {
                    state.readings.clear();
                }
                if name == "cloud-loading" {
                    state.cloud.busy = true;
                }
                if name == "cloud-error" {
                    state.cloud.error = true;
                    state.cloud.message = "暂未完成，将自动重试".into();
                }
                if name == "cloud-fallback" {
                    state.readings.clear();
                    state = crate::cloud::presentation(state);
                }

                if matches!(
                    name,
                    "home-connected"
                        | "home-stale"
                        | "home-large"
                        | "climate"
                        | "climate-hover"
                        | "climate-details"
                        | "climate-loading"
                        | "climate-disconnected"
                        | "displays"
                        | "device"
                ) {
                    let now = usage::now();
                    let day = crate::home::model::day() as u32;
                    state.connected = true;
                    state.device_supported = true;
                    state.device_read_at = Some(now);
                    state.telemetry = Some(crate::device::Telemetry {
                        temperature: Some(22.7),
                        humidity: Some(46.),
                        sampled_at: now,
                        age_ms: 0,
                        page: "overview".into(),
                        pages: 243,
                        rotation: 0,
                        auto_rotate: true,
                        paused: false,
                        busy: false,
                        partial: true,
                        power: (0, 1, 0, 78),
                        sd: (1, day, 840, 1, now, 0),
                        history: 1,
                        sample: None,
                        shown: Some((Some(22.6), Some(45.9), now - 35, 0, 0, true, 10)),
                        received_at: now,
                    });
                    state.history = Some(crate::device::History {
                        day,
                        revision: 1,
                        received_at: now,
                        points: (0..96)
                            .map(|i| {
                                if (40..44).contains(&i) || i > 76 {
                                    None
                                } else {
                                    Some((
                                        21. + (i as f32 / 12.).sin(),
                                        47. + (i as f32 / 15.).cos() * 3.,
                                    ))
                                }
                            })
                            .collect(),
                    });
                }
                if name == "climate-loading" {
                    state.controlling_device = true;
                }
                if name == "home-empty" {
                    state.readings.clear();
                    state.last_read = None;
                }
                if name == "home-stale" {
                    state.connected = false;
                    if let Some(d) = state.telemetry.as_mut() {
                        d.sampled_at -= 3600;
                        d.received_at -= 3600;
                    }
                }
                if name == "home-large" {
                    state.readings[1].packet.d =
                        Some([crate::home::model::day(), 9_007_199_254_740_991]);
                    state.readings[1].packet.wt = Some([week, 9_007_199_254_740_991]);
                    state.readings[1].packet.mt = Some([
                        crate::home::model::month_start(crate::home::model::day()).unwrap(),
                        9_007_199_254_740_991,
                    ]);
                    if let Some(d) = state.telemetry.as_mut() {
                        d.temperature = Some(-45.);
                        d.humidity = Some(100.);
                    }
                }
                if name == "climate-disconnected" {
                    state.connected = false;
                }
                if name.starts_with("harness") {
                    home.activate("usage.harness");
                    let day = crate::home::model::day();
                    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
                    use chrono::Datelike;
                    let monday = today
                        - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday()));
                    let week = i64::from(monday.year()) * 10000
                        + i64::from(monday.month()) * 100
                        + i64::from(monday.day());
                    let mut h = usage::empty(
                        crate::harness::PROVIDER,
                        usage::OK,
                        "界面样例，不读取真实会话",
                    );
                    h.packet.n = Some(crate::harness::NAME.into());
                    state.harness_devices = vec![
                        crate::harness::DeviceUsage {
                            id: "0000000000000001".into(),
                            name: "Mac-mini".into(),
                            t: usage::now(),
                            d: [day, 9548778],
                            wt: [week, 9639050],
                            mt: Some([day - day % 100 + 1, 20_000_000]),
                            u: [9503090, 45688, 9350016, 102],
                        },
                        crate::harness::DeviceUsage {
                            id: "0000000000000002".into(),
                            name: "MacBook-Pro".into(),
                            t: usage::now() - 3600,
                            d: [day, 4240000],
                            wt: [week, 12800000],
                            mt: Some([day - day % 100 + 1, 30_000_000]),
                            u: [4000000, 240000, 2600000, 80],
                        },
                    ];
                    state.readings = vec![h];
                }

                if name == "usage-error" {
                    state.readings[0] =
                        usage::empty(0, 5, "界面样例：官方额度读取失败，请检查登录或网络。");
                    state.cli_diagnostic = Some(crate::codex_cli::Diagnostic {
                        message: "界面样例：CLI文件不存在或不可执行。".into(),
                        ..Default::default()
                    });
                }
                if name == "usage-loading" {
                    state.refreshing_usage = true;
                }
                if name.starts_with("usage-all") {
                    let mut claude = usage::empty(1, usage::OK, "界面样例：官方statusLine数据");
                    claude.packet.w[0] = [520, 300, 0];
                    claude.packet.w[1] = [740, 10080, 0];
                    let mut deepseek = usage::empty(2, usage::OK, "界面样例：余额");
                    deepseek.packet.b = Some(12.34);
                    deepseek.packet.c = Some("CNY".into());
                    state.readings.splice(1..1, [claude, deepseek]);
                }
                if name.starts_with("connection-") {
                    state.devices = vec![crate::backend::Device {
                        id: "preview-only".into(),
                        name: "EPaper · 界面样例".into(),
                        rssi: Some(-52),
                        bound: true,
                        enrollment: false,
                        recovered: false,
                    }];
                    home.selected = "preview-only".into();
                }
                if name == "connection-remembered" {
                    state.devices.clear();
                    state.bound_name = "EPaper_ABCDEF".into();
                }
                if name == "connection-recovered" {
                    state.devices[0].recovered = true;
                    state.devices[0].rssi = None;
                }
                if name == "usage-empty" {
                    state.readings.clear();
                }
                if name == "home-long" {
                    home.prefs.home = "这是一个名字比较长的家庭名称用于布局检查".into();
                    home.prefs.room = "一个拥有很长名字的工作室与客厅".into();
                }
                if name == "settings-dirty" {
                    home.prefs.home = "工作室".into();
                }
                if name == "refresh-disabled" {
                    home.prefs.refresh.enabled = false;
                    state.refresh.enabled = false;
                }
                if name == "refresh-dirty" {
                    home.prefs.refresh.seconds = 300;
                }
                if name == "settings-child" {
                    home.activate("refresh.settings");
                }
                if name == "codex-loading" {
                    state.refreshing_codex = true;
                }
                if name.starts_with("todo") || name == "home-hub" {
                    let today = crate::todo::today();
                    for (i, (title, q, day, time)) in [
                        ("交季度报告", 0u8, Some(today), Some(570u16)),
                        ("回复王老师邮件", 0, Some(today), None),
                        ("读完《万历十五年》", 1, None, None),
                        ("整理 HomeCraft 待办", 1, Some(today), Some(840)),
                        ("取快递", 2, Some(today), Some(1110)),
                        ("给绿萝浇水", 3, None, None),
                        ("周末跑步十公里", 1, None, None),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let mut t = crate::todo::Todo::new(title, q);
                        t.date = day;
                        t.time = time;
                        t.minutes = time.map(|_| 60);
                        t.updated = 1 + i as i64;
                        t.origin = "preview".into();
                        t.notes = match i {
                            0 => "附第三季度数据表\n下午三点前发给财务".into(),
                            3 => "把详情和时间快捷选择也测一遍".into(),
                            _ => String::new(),
                        };
                        state.todos.push(t);
                    }
                    home.show_todo_view(match name {
                        "todo-timeline" => 1,
                        "todo-calendar" => 2,
                        _ => 0,
                    });
                    if name == "todo-edit" {
                        let first = state.todos[0].clone();
                        home.preview_edit_todo(first);
                    }
                    if name == "todo-new" {
                        // As if typed into a quadrant's quick-add field, then 详细.
                        home.preview_edit_todo(crate::todo::Todo::new("约牙医复查", 1));
                    }
                }
                if name == "weather-settings" || name == "home-hub" {
                    home.prefs.weather.second_city = "Tokyo".into();
                    home.mark_preferences_saved();
                    let now = usage::now();
                    for (slot, (city, region, temp, code)) in
                        [("新加坡", "", 271i16, 2u8), ("Tokyo", "Japan", 158, 61)]
                            .into_iter()
                            .enumerate()
                    {
                        state.weather[slot] = Some(crate::weather::Snapshot {
                            city: city.into(),
                            region: region.into(),
                            fetched_at: now,
                            temperature_tenths: temp,
                            low_tenths: temp - 30,
                            high_tenths: temp + 30,
                            code,
                            tomorrow: Some((95, temp - 20, temp + 40)),
                        });
                    }
                }
                if name.starts_with("climate-places") {
                    let now = usage::now();
                    for (id, model, base) in [
                        ("a9b7115a", "3Inch7EPaper", 24.0f32),
                        ("95c193b8", "reTerminalE1002", 28.5),
                    ] {
                        state.fleet.insert(
                            id.into(),
                            crate::backend::FleetStatus {
                                id: id.into(),
                                name: format!("EPaper_{}", &id[..6].to_uppercase()),
                                model: model.into(),
                                connected: true,
                                climate: Some((base, 52.0, now)),
                                ..Default::default()
                            },
                        );
                        for i in 0..288 {
                            if id == "95c193b8" && i < 260 {
                                continue;
                            }
                            let at = now - (288 - i) * 300;
                            let wave = ((i as f32) / 40.).sin();
                            crate::envlog::record(
                                &mut state.env_log,
                                id,
                                at,
                                base + wave * 1.5,
                                50. + wave * 6.,
                            );
                        }
                    }
                    home.prefs
                        .locations
                        .insert("95c193b8".into(), "客厅".into());
                    home.mark_preferences_saved();
                }
                if name == "displays-poem" {
                    state.connected = true;
                }
                let filename = format!("{name}-{}-{width}.png", if dusk { "dusk" } else { "day" });
                let path = directory.join(&filename);
                ensure!(!path.exists(), "预览图片已存在，请换一个输出目录");
                let mut renderer = egui_wgpu::Renderer::new(
                    &device,
                    wgpu::TextureFormat::Rgba8Unorm,
                    egui_wgpu::RendererOptions::default(),
                );
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("HomeCraft audit"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                for frame in 0..3 {
                    let mut input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width as f32, height as f32),
                        )),
                        time: Some(frame as f64),
                        ..Default::default()
                    };
                    if (name.ends_with("-bottom") && frame > 0)
                        || (name.ends_with("-middle") && frame == 1)
                    {
                        input.events.push(egui::Event::PointerMoved(egui::pos2(
                            width as f32 / 2.,
                            height as f32 / 2.,
                        )));
                        input.events.push(egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: egui::vec2(
                                0.,
                                if name.ends_with("-middle") {
                                    -500.
                                } else {
                                    -850.
                                },
                            ),
                            modifiers: egui::Modifiers::default(),
                        });
                    }
                    if (name == "climate-hover" || name == "climate-places-hover")
                        && frame > 0
                        && let Some(r) = home.targets.get("places.chart.temperature")
                    {
                        input.events.push(egui::Event::PointerMoved(egui::pos2(
                            r[0] + r[2] * 0.92,
                            r[1] + r[3] * 0.5,
                        )));
                    }
                    let mut output = ctx.run_ui(input, |ui| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(ui, |ui| {
                                let _ = home.render(ui, &state);
                            });
                        ui.painter().text(
                            egui::pos2(width as f32 - 16., height as f32 - 12.),
                            egui::Align2::RIGHT_BOTTOM,
                            "界面样例 · 非真实用量",
                            theme::font(11.),
                            theme::Tokens::home().secondary,
                        );
                    });
                    let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
                    let screen = egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [width, height],
                        pixels_per_point: output.pixels_per_point,
                    };
                    for (id, deltas) in &output.textures_delta.set {
                        for delta in deltas {
                            renderer.update_texture(&device, &queue, *id, delta);
                        }
                    }
                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                    let commands =
                        renderer.update_buffers(&device, &queue, &mut encoder, &jobs, &screen);
                    {
                        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("HomeCraft audit"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
                    }
                    queue.submit(commands.into_iter().chain([encoder.finish()]));
                    for id in &output.textures_delta.free {
                        renderer.free_texture(id);
                    }
                    output.textures_delta.clear();
                }
                let row = (width * 4).div_ceil(256) * 256;
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("HomeCraft audit readback"),
                    size: u64::from(row * height),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                encoder.copy_texture_to_buffer(
                    texture.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row),
                            rows_per_image: None,
                        },
                    },
                    texture.size(),
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = mpsc::channel();
                buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
                device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(10)),
                })?;
                rx.recv_timeout(Duration::from_secs(10))??;
                let mapped = buffer.slice(..).get_mapped_range()?;
                let rgba: Vec<u8> = mapped
                    .chunks(row as usize)
                    .flat_map(|r| r.iter().take((width * 4) as usize).copied())
                    .collect();
                image::save_buffer(&path, &rgba, width, height, image::ColorType::Rgba8)?;
                drop(mapped);
                buffer.unmap();
                layout_manifest.push(serde_json::json!({"file":filename,"window":[width,height],"targets":home.targets}));
                manifest.push(filename);
            }
        }
    }
    std::fs::write(
        directory.join("layout-targets.json"),
        serde_json::to_vec_pretty(&layout_manifest)?,
    )?;
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"fixture_only":true,"native_window_chrome":false,"files":manifest}),
        )?,
    )?;
    println!(
        "已使用正式界面组件离屏渲染 {} 张预览，无账户或设备访问。",
        manifest.len()
    );
    Ok(())
}
