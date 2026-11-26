//! Offline interaction checks for the actual widgets. No backend, accounts or Bluetooth.
use super::*;
use crate::device::Action;
fn frame(
    ctx: &egui::Context,
    home: &mut Home,
    state: &State,
    size: [f32; 2],
    events: Vec<egui::Event>,
) -> Vec<Intent> {
    let mut intents = vec![];
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    intents = home.render(ui, state);
                });
        },
    );
    output.platform_output.commands.clear();
    output.textures_delta.clear();
    intents
}
fn settle(ctx: &egui::Context, home: &mut Home, state: &State, size: [f32; 2]) {
    for _ in 0..4 {
        frame(ctx, home, state, size, vec![]);
    }
}
fn click(
    ctx: &egui::Context,
    home: &mut Home,
    state: &State,
    size: [f32; 2],
    key: &str,
) -> Vec<Intent> {
    let r = home.targets[key];
    let pos = egui::pos2(r[0] + r[2] / 2., r[1] + r[3] / 2.);
    frame(
        ctx,
        home,
        state,
        size,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        ctx,
        home,
        state,
        size,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
}
fn setup() -> (egui::Context, Home, State) {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let home = Home::new(&ctx, Preferences::default());
    let mut r = usage::empty(0, usage::OK, "");
    r.packet.w[0] = [860, 300, 0];
    r.packet.w[1] = [60, 10080, 0];
    let state = State {
        readings: vec![r],
        ..Default::default()
    };
    (ctx, home, state)
}
#[test]
fn usage_page_settles_without_alternating_layouts() {
    for size in [[850., 620.], [1100., 780.], [1440., 900.]] {
        let (ctx, mut home, mut state) = setup();
        home.activate("usage.harness");
        let local = crate::harness::test_reading("1111111111111111", 500);
        let peer = crate::harness::test_reading("2222222222222222", 700);
        state.readings.push(local);
        state.harness_devices = vec![crate::harness::DeviceUsage::from_reading(&peer).unwrap()];
        state = crate::cloud::presentation(state);
        let mut layouts = vec![];
        for index in 0..16 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(size[0], size[1]),
                    )),
                    time: Some(index as f64 * 0.25),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ui, |ui| {
                            home.render(ui, &state);
                        });
                },
            );
            layouts.push(format!("{:?}", output.shapes));
            output.platform_output.commands.clear();
            output.textures_delta.clear();
        }
        if let Some(pair) = layouts[8..].windows(2).find(|w| w[0] != w[1]) {
            let at = pair[0]
                .chars()
                .zip(pair[1].chars())
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            let detail = pair
                .iter()
                .map(|s| {
                    s.chars()
                        .skip(at.saturating_sub(180))
                        .take(500)
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            panic!("usage layout keeps changing at {size:?}: {detail:?}");
        }
    }
}
#[test]
fn environment_location_filter_range_and_explicit_name_editor_work() {
    for size in [[850., 620.], [1100., 780.], [1440., 900.]] {
        let (ctx, mut home, mut state) = setup();
        home.nav = Nav::Climate;
        let now = usage::now();
        for id in ["room-a", "room-b"] {
            state.fleet.insert(
                id.into(),
                crate::backend::FleetStatus {
                    id: id.into(),
                    name: id.into(),
                    model: "Test display".into(),
                    connected: true,
                    climate: Some((24., 50., now)),
                    ..Default::default()
                },
            );
            state
                .env_log
                .insert(id.into(), vec![(now - 300, 24., 50.), (now, 25., 52.)]);
        }
        settle(&ctx, &mut home, &state, size);
        assert!(!home.targets.contains_key("place.name.room-a"));
        assert!(click(&ctx, &mut home, &state, size, "place.select.room-a").is_empty());
        assert_eq!(home.places_view.trend.active.as_deref(), Some("room-a"));
        assert!(click(&ctx, &mut home, &state, size, "places.range.6").is_empty());
        assert_eq!(home.places_view.trend.hours, 6);
        click(&ctx, &mut home, &state, size, "places.all");
        assert!(home.places_view.trend.active.is_none());
        let rect = home.targets["places.chart.temperature"];
        frame(
            &ctx,
            &mut home,
            &state,
            size,
            vec![egui::Event::PointerMoved(egui::pos2(
                rect[0] + rect[2] * 0.9,
                rect[1] + rect[3] * 0.5,
            ))],
        );
        assert!(home.places_view.trend.cursor.is_some());
        settle(&ctx, &mut home, &state, size);
        let after = home.targets["places.chart.temperature"];
        assert_eq!(rect, after, "Hover readouts must not move the plot");
        click(&ctx, &mut home, &state, size, "places.edit");
        assert!(home.places_view.editing);
        assert!(home.targets.contains_key("place.name.room-a"));
        let names = home.prefs.locations.clone();
        click(&ctx, &mut home, &state, size, "places.edit");
        assert!(!home.places_view.editing);
        assert_eq!(home.prefs.locations, names);
    }
}
#[test]
fn home_summaries_fit_small_and_large_windows_with_extreme_or_missing_values() {
    for size in [[850., 620.], [1100., 780.], [1440., 900.]] {
        for populated in [false, true] {
            let (ctx, mut home, mut state) = setup();
            if populated {
                state.readings[0].packet.d = Some([model::day(), 9_007_199_254_740_991]);
                state.readings[0].packet.wt = Some([
                    model::week_start(model::day()).unwrap(),
                    9_007_199_254_740_991,
                ]);
                state.readings[0].packet.mt = Some([
                    model::month_start(model::day()).unwrap(),
                    9_007_199_254_740_991,
                ]);
                state.connected = true;
                let now = usage::now();
                state.telemetry=Some(crate::device::Telemetry::parse(&serde_json::json!({"ok":true,"v":1,"device":{
                    "t":-45.,"h":100.,"at":now,"age":0,"page":"overview","pages":1,"rotation":0,"auto":true,
                    "paused":false,"busy":false,"partial":true,"power":[0,1,0,50],"sd":[1,model::day(),1,1,now,0],"history":1
                }}),now).unwrap());
            } else {
                state.readings.clear();
            }
            settle(&ctx, &mut home, &state, size);
            let tokens = home.targets["home.tokens"];
            let environment = home.targets["home.environment"];
            assert_eq!(tokens[1], environment[1]);
            assert_eq!(tokens[3], environment[3]);
            assert!(tokens[0] + tokens[2] + 15. <= environment[0]);
            for (index, id) in ["today", "week", "month"].iter().enumerate() {
                let value = home.targets[&format!("home.tokens.{id}.value")];
                let left = tokens[0] + index as f32 * tokens[2] / 3.;
                assert!(value[0] >= left + 11.);
                assert!(
                    value[0] + value[2] <= left + tokens[2] / 3. - 11.,
                    "{id}: {value:?}"
                );
            }
            for id in ["temperature", "humidity"] {
                let value = home.targets[&format!("home.environment.{id}")];
                assert!(value[0] >= environment[0] + 80., "{id}: {value:?}");
                assert!(value[0] + value[2] <= environment[0] + environment[2] - 17.);
            }
            let device = home.targets["device.epaper"];
            assert!(device[1] >= tokens[1] + tokens[3] + 15.);
            assert!(device[1] + device[3] < size[1] - 44.);
        }
    }
}
#[test]
fn home_summary_clicks_open_details_without_any_hardware_request() {
    let (ctx, mut home, state) = setup();
    let size = [850., 620.];
    settle(&ctx, &mut home, &state, size);
    assert!(click(&ctx, &mut home, &state, size, "home.tokens").is_empty());
    assert_eq!(home.nav, Nav::Usage);
    home.nav = Nav::Home;
    settle(&ctx, &mut home, &state, size);
    assert!(click(&ctx, &mut home, &state, size, "home.environment").is_empty());
    assert_eq!(home.nav, Nav::Climate);
}
#[test]
fn timestamp_and_actions_share_a_row_at_both_sizes_and_never_jump_when_loading() {
    for size in [[850., 620.], [1100., 780.]] {
        for device_sheet in [false, true] {
            let (ctx, mut home, mut state) = setup();
            home.nav = Nav::Usage;
            if device_sheet {
                home.sheet = Sheet::Device;
                home.tab = Tab::Usage;
            }
            settle(&ctx, &mut home, &state, size);
            let initial = home.targets["codex.card.refresh"];
            for (check, refresh, all) in [
                (false, false, false),
                (true, false, false),
                (false, true, false),
                (false, false, true),
            ] {
                state.checking_cli = check;
                state.refreshing_codex = refresh;
                state.refreshing_usage = all;
                settle(&ctx, &mut home, &state, size);
                let stamp = home.targets["codex.card.updated"];
                let button = home.targets["codex.card.refresh"];
                let settings = home.targets["codex.card.settings"];
                assert!((stamp[1] + stamp[3] / 2. - button[1] - button[3] / 2.).abs() < 1.);
                assert!(stamp[0] >= settings[0] + settings[2]);
                assert!(stamp[0] + stamp[2] < size[0]);
                assert!(
                    (button[0] - initial[0]).abs() < 0.1 && (button[2] - initial[2]).abs() < 0.1,
                    "button moves in busy state"
                );
            }
        }
    }
}
#[test]
fn settings_child_back_restores_parent_and_short_forms_fit_their_content() {
    let (ctx, mut home, state) = setup();
    let size = [850., 620.];
    home.sheet = Sheet::Settings;
    settle(&ctx, &mut home, &state, size);
    assert!(click(&ctx, &mut home, &state, size, "codex.settings").is_empty());
    assert_eq!(home.sheet, Sheet::Codex);
    settle(&ctx, &mut home, &state, size);
    assert!(home.targets.contains_key("sheet.back"));
    assert!(click(&ctx, &mut home, &state, size, "sheet.back").is_empty());
    assert_eq!(home.sheet, Sheet::Settings);
    settle(&ctx, &mut home, &state, size);
    let result = click(&ctx, &mut home, &state, size, "refresh.settings");
    assert!(result.is_empty());
    assert_eq!(home.sheet, Sheet::Refresh);
    settle(&ctx, &mut home, &state, size);
    assert!(home.targets["refresh.save"][1] + home.targets["refresh.save"][3] < size[1] - 30.);
    assert!(click(&ctx, &mut home, &state, size, "sheet.close").is_empty());
    assert_eq!(home.sheet, Sheet::None);
    assert!(home.sheet_history.is_empty());
}
#[test]
fn login_item_is_explicit_immediate_and_independent_of_general_preferences() {
    let (ctx, mut home, state) = setup();
    let size = [1100., 1000.];
    home.sheet = Sheet::Settings;
    settle(&ctx, &mut home, &state, size);
    assert!(!home.login_item.selected());
    assert!(!home.preferences_changed());
    assert!(
        click(&ctx, &mut home, &state, size, "startup.enabled")
            .iter()
            .any(|i| matches!(i, Intent::SetLoginItem(true)))
    );
    assert!(!home.preferences_changed());
    home.login_item = epaper_macos::LoginItemStatus::RequiresApproval;
    settle(&ctx, &mut home, &state, size);
    assert!(
        click(&ctx, &mut home, &state, size, "startup.system")
            .iter()
            .any(|i| matches!(i, Intent::OpenLoginItems))
    );
    home.login_item = epaper_macos::LoginItemStatus::Unavailable;
    settle(&ctx, &mut home, &state, size);
    assert!(click(&ctx, &mut home, &state, size, "startup.enabled").is_empty());
}
#[test]
fn software_update_sheet_has_explicit_actions_and_saved_auto_preferences() {
    let (ctx, mut home, state) = setup();
    let size = [1100., 1000.];
    assert!(home.activate("update.settings"));
    settle(&ctx, &mut home, &state, size);
    assert_eq!(home.sheet, Sheet::Update);
    assert!(
        click(&ctx, &mut home, &state, size, "update.check")
            .iter()
            .any(|i| matches!(i, Intent::CheckUpdates))
    );
    let release = crate::update::Release {
        version: "0.3.2".into(),
        build: 50,
        notes: "## 更新\n- 改进同步\n".repeat(20),
        asset: crate::update::Asset {
            url: String::new(),
            sha256: "0".repeat(64),
            size: 100,
        },
    };
    home.updates = crate::update::Status::Available(release.clone());
    settle(&ctx, &mut home, &state, size);
    assert!(
        click(&ctx, &mut home, &state, size, "update.download")
            .iter()
            .any(|i| matches!(i, Intent::DownloadUpdate))
    );
    assert!(!home.targets.contains_key("update.install"));
    home.updates = crate::update::Status::Downloading {
        release,
        done: 40,
        total: 100,
    };
    settle(&ctx, &mut home, &state, size);
    assert!(click(&ctx, &mut home, &state, size, "update.check").is_empty());
    assert!(
        click(&ctx, &mut home, &state, size, "update.later")
            .iter()
            .any(|i| matches!(i, Intent::LaterUpdate))
    );
    home.updates = crate::update::Status::Idle;
    home.prefs.updates.auto_download = false;
    settle(&ctx, &mut home, &state, size);
    assert!(
        click(&ctx, &mut home, &state, size, "update.save")
            .iter()
            .any(|i| matches!(i, Intent::SavePreferences))
    );
}
#[test]
fn save_requires_a_change_and_updates_the_saved_state_only_after_success() {
    let (ctx, mut home, state) = setup();
    let size = [850., 620.];
    home.sheet = Sheet::Settings;
    settle(&ctx, &mut home, &state, size);
    assert!(click(&ctx, &mut home, &state, size, "preferences.save").is_empty());
    home.prefs.home = "工作室".into();
    settle(&ctx, &mut home, &state, size);
    assert!(
        click(&ctx, &mut home, &state, size, "preferences.save")
            .iter()
            .any(|i| matches!(i, Intent::SavePreferences))
    );
    assert!(home.preferences_changed());
    home.mark_preferences_saved();
    assert!(!home.preferences_changed());
}
#[test]
fn navigation_rows_open_their_real_destination_without_io() {
    for (nav, key, expected_nav, expected_sheet) in [
        (
            Nav::Automation,
            "automation.usage",
            Nav::Automation,
            Sheet::Refresh,
        ),
        (
            Nav::Automation,
            "automation.connection",
            Nav::Automation,
            Sheet::Device,
        ),
        (
            Nav::Automation,
            "automation.environment",
            Nav::Climate,
            Sheet::None,
        ),
        (
            Nav::Automation,
            "automation.cloud",
            Nav::Automation,
            Sheet::Cloud,
        ),
        (Nav::Discover, "discover.usage", Nav::Usage, Sheet::None),
        (Nav::Discover, "discover.climate", Nav::Climate, Sheet::None),
        (
            Nav::Discover,
            "discover.connection",
            Nav::Discover,
            Sheet::Add,
        ),
        (Nav::Home, "home.room", Nav::Room, Sheet::None),
        (Nav::Home, "connection-banner", Nav::Home, Sheet::Add),
    ] {
        let (ctx, mut home, state) = setup();
        home.nav = nav;
        settle(&ctx, &mut home, &state, [850., 620.]);
        let intents = click(&ctx, &mut home, &state, [850., 620.], key);
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, Intent::Device(_) | Intent::Connect(_) | Intent::Scan)),
            "unexpected I/O from {key}"
        );
        assert_eq!(home.nav, expected_nav);
        assert_eq!(home.sheet, expected_sheet);
    }
}
#[test]
fn screen_buttons_request_the_selected_page_and_do_nothing_when_disconnected() {
    let (ctx, mut home, mut state) = setup();
    home.nav = Nav::Displays;
    state.connected = true;
    state.device_supported = true;
    state.telemetry=Some(crate::device::Telemetry::parse(&serde_json::json!({"ok":true,"v":1,"device":{"t":23.4,"h":48.2,"at":100,"age":0,"page":"overview","pages":243,"rotation":0,"auto":true,"paused":false,"busy":false,"partial":true,"power":[1,0,0,-1],"sd":[1,20261022,10,1,100,0],"history":1}}),100).unwrap());
    settle(&ctx, &mut home, &state, [850., 620.]);
    assert!(
        click(&ctx, &mut home, &state, [850., 620.], "screen.environment")
            .iter()
            .any(|i| matches!(i,Intent::Device(Action::Page(page)) if page=="environment"))
    );
    state.connected = false;
    settle(&ctx, &mut home, &state, [850., 620.]);
    assert!(click(&ctx, &mut home, &state, [850., 620.], "screen.environment").is_empty());
    let intents = click(&ctx, &mut home, &state, [850., 620.], "screen.connect");
    assert!(
        !intents
            .iter()
            .any(|i| matches!(i, Intent::Device(_) | Intent::Connect(_) | Intent::Scan))
    );
    assert_eq!(home.sheet, Sheet::Add);
}
#[test]
fn declined_rotation_is_hidden_and_only_advertised_pages_are_offered() {
    let (ctx, mut home, mut state) = setup();
    home.nav = Nav::Displays;
    state.connected = true;
    state.device_supported = true;
    state.telemetry=Some(crate::device::Telemetry::parse(&serde_json::json!({"ok":true,"v":1,"device":{"t":23.4,"h":48.2,"at":100,"age":0,"page":"overview","pages":8193,"rotation":0,"auto":false,"paused":false,"busy":false,"partial":true,"power":[0,0,0,-1],"sd":[0,0,0,0,0,0],"history":0}}),100).unwrap());
    settle(&ctx, &mut home, &state, [850., 620.]);
    assert!(home.targets.contains_key("screen.rotation.auto"));
    state.rotation_supported = false;
    state.history_supported = false;
    home.targets.clear();
    settle(&ctx, &mut home, &state, [850., 620.]);
    assert!(home.targets.contains_key("screen.overview"));
    assert!(home.targets.contains_key("screen.usage"));
    assert!(!home.targets.contains_key("screen.environment"));
    assert!(
        !home
            .targets
            .keys()
            .any(|k| k.starts_with("screen.rotation"))
    );
}
#[test]
fn long_names_elide_without_covering_header_controls() {
    let (ctx, mut home, state) = setup();
    home.prefs.home = "名字很长很长的家庭名称".repeat(5);
    home.prefs.room = "名字很长很长的房间".repeat(5);
    settle(&ctx, &mut home, &state, [850., 620.]);
    let room = home.targets["home.room"];
    assert!(room[0] + room[2] <= 850. - 24.);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let galley = theme::fitted_text(
            ui.painter(),
            &home.prefs.room,
            bold(17.),
            Color32::WHITE,
            120.,
        );
        assert!(galley.elided);
        assert!(galley.size().x <= 120. + 0.5);
    });
    output.textures_delta.clear();
}

#[test]
fn keyboard_shortcuts_preserve_manual_and_disconnected_behavior() {
    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }
    }
    let (ctx, mut home, mut state) = setup();
    let size = [850., 620.];
    home.nav = Nav::Usage;
    settle(&ctx, &mut home, &state, size);
    assert!(
        frame(&ctx, &mut home, &state, size, vec![key(egui::Key::R)])
            .iter()
            .any(|i| matches!(i, Intent::Sync))
    );
    home.nav = Nav::Climate;
    assert!(
        !frame(&ctx, &mut home, &state, size, vec![key(egui::Key::R)])
            .iter()
            .any(|i| matches!(i, Intent::Device(_)))
    );
    state.connected = true;
    state.device_supported = true;
    assert!(
        frame(&ctx, &mut home, &state, size, vec![key(egui::Key::R)])
            .iter()
            .any(|i| matches!(i, Intent::Device(Action::Read)))
    );
    frame(&ctx, &mut home, &state, size, vec![key(egui::Key::Comma)]);
    assert_eq!(home.sheet, Sheet::Settings);
    home.prefs.home = "新的家庭".into();
    assert!(
        frame(&ctx, &mut home, &state, size, vec![key(egui::Key::S)])
            .iter()
            .any(|i| matches!(i, Intent::SavePreferences))
    );
}
#[test]
fn todo_add_row_stays_put_details_prefill_and_drag_moves_quadrant() {
    use crate::todo::{Command, Todo};
    let (ctx, mut home, mut state) = setup();
    let size = [1100., 780.];
    home.nav = Nav::Todo;
    settle(&ctx, &mut home, &state, size);
    let empty = home.targets["todo.quick.1"];
    for i in 0..5 {
        let mut t = Todo::new(&format!("事项{i}"), 1);
        t.updated = 1 + i;
        state.todos.push(t);
    }
    settle(&ctx, &mut home, &state, size);
    assert_eq!(home.targets["todo.quick.1"], empty, "add row must not move");
    // Typed text goes into the full editor instead of being saved straight away.
    home.todo_quick[2] = "约牙医".into();
    assert!(click(&ctx, &mut home, &state, size, "todo.quick.more.2").is_empty());
    assert_eq!(home.sheet, Sheet::Todo);
    let draft = home.todo_draft.todo.clone().unwrap();
    assert_eq!((draft.title.as_str(), draft.quadrant), ("约牙医", 2));
    assert!(home.todo_quick[2].is_empty());
    home.close_sheet();
    settle(&ctx, &mut home, &state, size);
    // Drag the top visible title (newest undated first) onto the urgent-not-important card.
    let id = state.todos[4].id.clone();
    let r = home.targets[&format!("todo.item.{id}")];
    let from = egui::pos2(r[0] + 10., r[1] + r[3] / 2.);
    let q = home.targets["todo.quick.2"];
    let to = egui::pos2(q[0] + q[2] / 2., q[1] + q[3] / 2.);
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(
        &ctx,
        &mut home,
        &state,
        size,
        vec![egui::Event::PointerMoved(from), press(from, true)],
    );
    for step in 1..=4 {
        let at = from + (to - from) * (step as f32 / 4.);
        frame(
            &ctx,
            &mut home,
            &state,
            size,
            vec![egui::Event::PointerMoved(at)],
        );
    }
    let intents = frame(&ctx, &mut home, &state, size, vec![press(to, false)]);
    assert!(
        intents
            .iter()
            .any(|i| matches!(i, Intent::Todo(Command::Save(t)) if t.id == id && t.quadrant == 2)),
        "drop should save the todo into quadrant 2"
    );
    assert_eq!(home.sheet, Sheet::None, "a drag is not a click");
}
