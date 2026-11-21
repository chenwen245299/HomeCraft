mod chart;
mod cloud_view;
mod dashboard;
mod environment_view;
mod harness_view;
pub mod icons;
pub mod model;
mod places_chart;
mod places_view;
pub mod theme;
mod todo_view;
#[cfg(test)]
mod ui_audit_tests;
mod update_view;
mod weather_view;
use crate::{
    backend::State,
    usage::{self, Reading},
};
use eframe::egui::{self, Align, Align2, Color32, Rect, RichText, Sense, Stroke};
use icons::{Icon, draw};
use model::{Nav, Preferences, Sheet, Tab};
use std::collections::BTreeMap;
use theme::{Tokens, bold, font};

pub enum Intent {
    Scan,
    Connect(String),
    ConnectionForm {
        active: bool,
        disconnect: bool,
    },
    DismissFeedback,
    Disconnect,
    Sync,
    Pair,
    PairBluetooth {
        id: String,
        code: zeroize::Zeroizing<String>,
    },
    InstallClaude,
    CheckCodex(crate::codex_cli::Settings),
    RefreshCodex,
    Device(crate::device::Action),
    Todo(crate::todo::Command),
    PickCodex,
    PickNode,
    PickCloud,
    Cloud(crate::cloud::Command),
    OpenProject,
    SavePreferences,
    SetLoginItem(bool),
    OpenLoginItems,
    CheckUpdates,
    DownloadUpdate,
    InstallUpdate,
    LaterUpdate,
    OpenReleases,
    Quit,
}
pub struct Home {
    pub updates: crate::update::Status,
    pub login_item: epaper_macos::LoginItemStatus,
    pub nav: Nav,
    pub sheet: Sheet,
    pub tab: Tab,
    pub prefs: Preferences,
    pub selected: String,
    pub notice: String,
    pub targets: BTreeMap<String, [f32; 4]>,
    environment_selection: chart::Selection,
    places_view: places_view::View,
    harness_scope: Option<String>,
    cloud_source: Option<String>,
    cloud_day: Option<i64>,
    cloud_selection: chart::Selection,
    usage_filter: Option<u8>,
    pairing_code: zeroize::Zeroizing<String>,
    connection_form_active: bool,
    wallpaper: egui::TextureHandle,
    frosted_wallpaper: egui::TextureHandle,
    wallpaper_dusk: bool,
    brands: Vec<Option<egui::TextureHandle>>,
    saved_prefs: Preferences,
    sheet_history: Vec<(Sheet, Tab)>,
    todo_view: todo_view::TodoView,
    /// Done items listed under the open ones; hidden by default.
    todo_show_done: bool,
    todo_day: u32,
    todo_month: u32,
    todo_draft: todo_view::Draft,
    todo_quick: [String; 4],
}
impl Home {
    pub fn new(ctx: &egui::Context, prefs: Preferences) -> Self {
        let (wallpaper, frosted_wallpaper) = theme::wallpaper(ctx, prefs.dusk);
        let wallpaper_dusk = prefs.dusk;
        let brands = [
            include_bytes!("../../../assets/brands/openai.png").as_slice(),
            include_bytes!("../../../assets/brands/claude.png").as_slice(),
            include_bytes!("../../../assets/brands/deepseek.ico").as_slice(),
            include_bytes!("../../../assets/brands/argus.png").as_slice(),
        ]
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            let mut image = image::load_from_memory(bytes).ok()?.to_rgba8();
            // Brand glyphs use template ink on glass. OpenAI's supplied bitmap has an
            // opaque white canvas; use its luminance as coverage instead of a white tile.
            if i < 3 {
                for pixel in image.pixels_mut() {
                    let alpha = if i == 0 {
                        let luminance =
                            (u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 3;
                        (u16::from(pixel[3]) * (255 - luminance) / 255) as u8
                    } else {
                        pixel[3]
                    };
                    *pixel = image::Rgba([248, 250, 252, alpha]);
                }
            }
            let mut bounds = (image.width(), image.height(), 0, 0);
            for (x, y, pixel) in image.enumerate_pixels() {
                if pixel[3] > 24 {
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x);
                    bounds.3 = bounds.3.max(y);
                }
            }
            if bounds.0 <= bounds.2 && bounds.1 <= bounds.3 {
                image = image::imageops::crop_imm(
                    &image,
                    bounds.0,
                    bounds.1,
                    bounds.2 - bounds.0 + 1,
                    bounds.3 - bounds.1 + 1,
                )
                .to_image();
            }
            let size = [image.width() as usize, image.height() as usize];
            Some(ctx.load_texture(
                format!("provider-{i}"),
                egui::ColorImage::from_rgba_unmultiplied(size, &image),
                egui::TextureOptions::LINEAR,
            ))
        })
        .collect();
        Self {
            updates: crate::update::Status::default(),
            login_item: epaper_macos::LoginItemStatus::default(),
            nav: Nav::Home,
            sheet: Sheet::None,
            tab: Tab::Overview,
            saved_prefs: prefs.clone(),
            sheet_history: vec![],
            prefs,
            selected: String::new(),
            notice: String::new(),
            targets: BTreeMap::new(),
            environment_selection: chart::Selection::default(),
            places_view: Default::default(),
            harness_scope: None,
            cloud_source: None,
            cloud_day: None,
            cloud_selection: chart::Selection::default(),
            usage_filter: None,
            pairing_code: zeroize::Zeroizing::new(String::new()),
            connection_form_active: false,
            wallpaper,
            frosted_wallpaper,
            wallpaper_dusk,
            brands,
            todo_view: Default::default(),
            todo_show_done: false,
            todo_day: 0,
            todo_month: 0,
            todo_draft: Default::default(),
            todo_quick: Default::default(),
        }
    }
    fn target(&mut self, id: &str, r: Rect) {
        self.targets
            .insert(id.into(), [r.min.x, r.min.y, r.width(), r.height()]);
    }
    pub fn mark_preferences_saved(&mut self) {
        self.saved_prefs = self.prefs.clone();
    }
    fn preferences_changed(&self) -> bool {
        self.saved_prefs != self.prefs
    }
    fn open_sheet(&mut self, sheet: Sheet) {
        if sheet == self.sheet {
            return;
        }
        if self.sheet == Sheet::None {
            self.sheet_history.clear();
        } else {
            self.sheet_history.push((self.sheet, self.tab));
        }
        self.sheet = sheet;
    }
    fn close_sheet(&mut self) {
        self.sheet = Sheet::None;
        self.sheet_history.clear();
        self.pairing_code.clear();
    }
    fn back_sheet(&mut self) {
        if let Some((sheet, tab)) = self.sheet_history.pop() {
            self.sheet = sheet;
            self.tab = tab;
        }
    }
    fn nav_row(
        &mut self,
        ui: &mut egui::Ui,
        r: Rect,
        nav: Nav,
        label: &str,
        icon: Icon,
        t: Tokens,
    ) {
        let response = ui.interact(r, ui.id().with(nav.id()), Sense::click());
        let active = self.nav == nav;
        if active || response.hovered() || response.has_focus() {
            ui.painter().rect_filled(
                r,
                theme::CONTROL_RADIUS,
                if active { t.accent } else { t.line },
            );
        }
        draw(
            ui.painter(),
            Rect::from_min_size(r.min + egui::vec2(11., 7.), egui::vec2(21., 21.)),
            icon,
            if active { t.on_accent } else { t.accent },
        );
        let color = if active { t.on_accent } else { t.text };
        let galley = theme::fitted_text(ui.painter(), label, bold(15.), color, r.width() - 55.);
        ui.painter().galley(
            egui::pos2(r.left() + 43., r.center().y - galley.size().y / 2.),
            galley,
            color,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, label)
        });
        response
            .clone()
            .on_hover_text(label)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        self.target(&format!("nav.{}", nav.id()), r);
        if response.clicked() {
            self.nav = nav;
        }
    }
    fn sidebar(&mut self, ui: &mut egui::Ui, r: Rect, t: Tokens) {
        ui.painter().rect_filled(r, 0, t.sidebar);
        let left = r.left() + 10.;
        let width = r.width() - 20.;
        let mut y = r.top() + 62.;
        for (nav, label, icon) in [
            (Nav::Home, "家庭", Icon::Home),
            (Nav::Automation, "自动化", Icon::Automation),
            (Nav::Discover, "探索", Icon::Star),
        ] {
            self.nav_row(
                ui,
                Rect::from_min_size(egui::pos2(left, y), egui::vec2(width, theme::ROW)),
                nav,
                label,
                icon,
                t,
            );
            y += theme::ROW;
        }
        y += 25.;
        ui.painter().text(
            egui::pos2(left + 9., y),
            Align2::LEFT_TOP,
            "类别",
            bold(12.),
            t.muted,
        );
        y += 25.;
        for (nav, label, icon) in [
            (Nav::Climate, "环境", Icon::Climate),
            (Nav::Displays, "显示", Icon::Display),
            (Nav::Usage, "AI 用量", Icon::Usage),
            (Nav::Todo, "待办", Icon::Automation),
        ] {
            self.nav_row(
                ui,
                Rect::from_min_size(egui::pos2(left, y), egui::vec2(width, theme::ROW)),
                nav,
                label,
                icon,
                t,
            );
            y += theme::ROW;
        }
        y += 25.;
        ui.painter().text(
            egui::pos2(left + 9., y),
            Align2::LEFT_TOP,
            "房间",
            bold(12.),
            t.muted,
        );
        y += 25.;
        let room = self.prefs.room.clone();
        self.nav_row(
            ui,
            Rect::from_min_size(egui::pos2(left, y), egui::vec2(width, theme::ROW)),
            Nav::Room,
            &room,
            Icon::Room,
            t,
        );
        let settings =
            Rect::from_min_size(egui::pos2(left, r.bottom() - 48.), egui::vec2(width, 32.));
        let response = ui.interact(settings, ui.id().with("settings"), Sense::click());
        if response.hovered() {
            ui.painter()
                .rect_filled(settings, theme::CONTROL_RADIUS, t.line);
        }
        draw(
            ui.painter(),
            Rect::from_min_size(settings.min + egui::vec2(12., 7.), egui::vec2(18., 18.)),
            Icon::Settings,
            t.muted,
        );
        ui.painter().text(
            settings.min + egui::vec2(42., 16.),
            Align2::LEFT_CENTER,
            "家庭设置",
            font(13.),
            t.secondary,
        );
        self.target("settings", settings);
        if response.clicked() {
            self.open_sheet(Sheet::Settings);
        }
    }
    fn icon_button(&mut self, ui: &mut egui::Ui, r: Rect, id: &str, icon: Icon, t: Tokens) -> bool {
        let label = match id {
            "add" => "添加配件",
            _ => "家庭设置 · ⌘,",
        };
        let response = ui
            .interact(r, ui.id().with(id), Sense::click())
            .on_hover_text(label)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        if response.hovered() || response.has_focus() {
            ui.painter().rect_filled(r, 18, t.line);
        }
        draw(ui.painter(), r.shrink(9.), icon, t.text);
        self.target(id, r);
        response.clicked()
    }
    fn header(&mut self, ui: &mut egui::Ui, r: Rect, t: Tokens) {
        let title = match self.nav {
            Nav::Home => self.prefs.home.as_str(),
            Nav::Automation => "自动化",
            Nav::Discover => "探索",
            Nav::Climate => "环境",
            Nav::Displays => "显示",
            Nav::Usage => "AI 用量",
            Nav::Room => self.prefs.room.as_str(),
            Nav::Todo => "待办",
        };
        let title_galley =
            theme::fitted_text(ui.painter(), title, bold(19.), t.text, r.width() - 138.);
        ui.painter().galley(
            r.min + egui::vec2(24., 27. - title_galley.size().y / 2.),
            title_galley,
            t.text,
        );
        let drag = Rect::from_min_max(
            r.min + egui::vec2(0., 0.),
            egui::pos2(r.right() - 104., r.top() + theme::TOOLBAR),
        );
        let response = ui.interact(drag, ui.id().with("window-drag"), Sense::drag());
        if response.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let capsule = Rect::from_min_size(
            egui::pos2(r.right() - 95., r.top() + 11.),
            egui::vec2(78., 34.),
        );
        ui.painter().rect_filled(capsule, 18, t.line);
        if self.icon_button(
            ui,
            Rect::from_min_size(capsule.min, egui::vec2(39., 34.)),
            "add",
            Icon::Plus,
            t,
        ) {
            self.open_sheet(Sheet::Add);
        }
        if self.icon_button(
            ui,
            Rect::from_min_size(capsule.min + egui::vec2(39., 0.), egui::vec2(39., 34.)),
            "more",
            Icon::More,
            t,
        ) {
            self.open_sheet(Sheet::Settings);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn chip(
        &mut self,
        ui: &mut egui::Ui,
        pos: egui::Pos2,
        label: &str,
        icon: Icon,
        color: Color32,
        nav: Nav,
        t: Tokens,
    ) -> f32 {
        let galley = ui.painter().layout_no_wrap(label.into(), bold(12.), t.text);
        let width = galley.size().x + 48.;
        let rect = Rect::from_min_size(pos, egui::vec2(width, 35.));
        let response = ui.interact(rect, ui.id().with(("chip", nav.id())), Sense::click());
        ui.painter().rect_filled(
            rect,
            18,
            if response.hovered() {
                t.tile_hover
            } else {
                t.glass
            },
        );
        ui.painter().rect_stroke(
            rect,
            theme::SURFACE_RADIUS,
            Stroke::new(1., t.line),
            egui::StrokeKind::Inside,
        );
        draw(
            ui.painter(),
            Rect::from_min_size(pos + egui::vec2(11., 9.), egui::vec2(17., 17.)),
            icon,
            color,
        );
        ui.painter()
            .galley(pos + egui::vec2(34., 10.), galley, t.text);
        self.target(&format!("chip.{}", nav.id()), rect);
        if response.clicked() {
            self.nav = nav;
        }
        width
    }
    fn accessory(&mut self, ui: &mut egui::Ui, pos: egui::Pos2, state: &State, t: Tokens) {
        let rect = Rect::from_min_size(pos, theme::TILE);
        let response = ui.interact(rect, ui.id().with("epaper-tile"), Sense::click());
        ui.painter().rect_filled(
            rect,
            theme::SURFACE_RADIUS,
            if response.hovered() {
                t.tile_hover
            } else {
                t.tile
            },
        );
        draw(
            ui.painter(),
            Rect::from_min_size(pos + egui::vec2(16., 17.), egui::vec2(30., 30.)),
            Icon::Display,
            if state.connected {
                t.usage
            } else {
                t.secondary
            },
        );
        ui.painter().rect_stroke(
            rect,
            theme::SURFACE_RADIUS,
            Stroke::new(1., t.line),
            egui::StrokeKind::Inside,
        );
        let live = state.connected;
        if let Some(device) = &state.telemetry {
            let fresh = device.fresh(state.connected, usage::now());
            ui.painter().text(
                pos + egui::vec2(15., 72.),
                Align2::LEFT_CENTER,
                match (device.temperature, device.humidity) {
                    (Some(c), Some(h)) => format!("{c:.1}°   {h:.0}%"),
                    _ => "—°   —%".into(),
                },
                bold(23.),
                if fresh { t.text } else { t.muted },
            );
        }
        ui.painter().circle_filled(
            pos + egui::vec2(151., 24.),
            3.,
            if live { t.safe } else { t.muted },
        );
        ui.painter().text(
            pos + egui::vec2(15., 102.),
            Align2::LEFT_CENTER,
            "EPaper",
            bold(16.),
            t.text,
        );
        ui.painter().text(
            pos + egui::vec2(15., 125.),
            Align2::LEFT_CENTER,
            model::connection(state),
            font(13.),
            if live { t.text } else { t.muted },
        );
        self.target("device.epaper", rect);
        if response.clicked() {
            self.open_sheet(Sheet::Device);
            self.tab = Tab::Overview;
        }
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "EPaper"));
        response
            .on_hover_text("查看墨水屏、AI 用量和蓝牙连接")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    fn home_page(&mut self, ui: &mut egui::Ui, r: Rect, state: &State, t: Tokens) {
        let mut x = r.left() + 24.;
        for (label, icon, color, nav) in [
            ("环境", Icon::Climate, t.climate, Nav::Climate),
            ("显示", Icon::Display, t.usage, Nav::Displays),
            ("AI 用量", Icon::Usage, t.safe, Nav::Usage),
            ("待办", Icon::Automation, t.accent, Nav::Todo),
        ] {
            x += self.chip(ui, egui::pos2(x, r.top() + 64.), label, icon, color, nav, t) + 8.;
        }
        let status = Rect::from_min_size(
            egui::pos2(r.left() + 24., r.top() + 125.),
            egui::vec2(r.width() - 48., 64.),
        );
        // A background session counts too: the controlled display may simply be switched off.
        let online = state.fleet.values().filter(|row| row.connected).count();
        let connected = state.connected || online > 0;
        ui.painter()
            .rect_filled(status, theme::SURFACE_RADIUS, t.banner);
        ui.painter().rect_stroke(
            status,
            theme::SURFACE_RADIUS,
            Stroke::new(1., t.line),
            egui::StrokeKind::Inside,
        );
        draw(
            ui.painter(),
            Rect::from_min_size(status.min + egui::vec2(16., 19.), egui::vec2(26., 26.)),
            if connected {
                Icon::Bluetooth
            } else {
                Icon::Info
            },
            if connected { t.safe } else { t.accent },
        );
        ui.painter().text(
            status.min + egui::vec2(58., 21.),
            Align2::LEFT_CENTER,
            if online > 1 {
                format!("{online} 台墨水屏已连接")
            } else if connected {
                "墨水屏已连接".into()
            } else {
                "连接你的墨水屏".into()
            },
            bold(14.),
            t.text,
        );
        let action = Rect::from_min_size(status.min + egui::vec2(58., 38.), egui::vec2(170., 20.));
        ui.painter().text(
            action.min,
            Align2::LEFT_TOP,
            if connected {
                "查看设备  ›"
            } else {
                "添加配件  ›"
            },
            font(12.),
            t.accent,
        );
        self.target("connection-banner", status);
        let banner = ui
            .interact(status, ui.id().with("connection-banner"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        banner.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                if connected {
                    "查看设备"
                } else {
                    "添加配件"
                },
            )
        });
        if banner.hovered() {
            ui.painter().rect_stroke(
                status,
                theme::SURFACE_RADIUS,
                Stroke::new(1., t.accent),
                egui::StrokeKind::Inside,
            );
        }
        if banner.clicked() {
            if state.connected {
                self.open_sheet(Sheet::Device);
            } else if connected {
                self.nav = Nav::Displays;
            } else {
                self.open_sheet(Sheet::Add);
            }
        }
        let heading = egui::pos2(r.left() + 24., r.top() + 225.);
        let room = theme::fitted_text(
            ui.painter(),
            &self.prefs.room,
            bold(17.),
            t.text,
            (r.width() - 90.).min(360.),
        );
        let room_width = room.size().x;
        ui.painter().galley(heading, room, t.text);
        draw(
            ui.painter(),
            Rect::from_min_size(
                heading + egui::vec2(room_width + 7., 0.),
                egui::vec2(15., 20.),
            ),
            Icon::Chevron,
            t.secondary,
        );
        let room_rect = Rect::from_min_size(
            heading - egui::vec2(0., 5.),
            egui::vec2(room_width + 32., 28.),
        );
        self.target("home.room", room_rect);
        if ui
            .interact(room_rect, ui.id().with("home.room"), Sense::click())
            .on_hover_text("查看房间")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.nav = Nav::Room;
        }
        self.dashboard(ui, r, heading + egui::vec2(0., 32.), state, t);
    }
    fn category(&mut self, ui: &mut egui::Ui, r: Rect, state: &State, t: Tokens) {
        let title = match self.nav {
            Nav::Room => self.prefs.room.clone(),
            Nav::Climate => "环境配件".into(),
            Nav::Displays => "显示配件".into(),
            _ => "家庭配件".into(),
        };
        let galley = theme::fitted_text(ui.painter(), &title, bold(24.), t.text, r.width() - 48.);
        ui.painter()
            .galley(r.min + egui::vec2(24., 82.), galley, t.text);
        self.accessory(ui, r.min + egui::vec2(24., 136.), state, t);
    }
    fn panel_content(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        egui::ScrollArea::vertical()
            .id_salt(("main-scroll", self.nav.id()))
            .show(ui, |ui| {
                ui.add_space(12.);
                match self.nav {
                    Nav::Climate => {
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.y = 6.;
                            ui.spacing_mut().interact_size.y = theme::COMPACT_ROW;
                            ui.spacing_mut().button_padding = egui::vec2(10., 4.);
                            self.places(ui, state, t, intents);
                        });
                        ui.add_space(16.);
                        self.weather_section(ui, state, t);
                        ui.add_space(16.);
                        egui::CollapsingHeader::new(
                            RichText::new("设备今日记录与其他来源")
                                .font(bold(14.))
                                .color(t.secondary),
                        )
                        .id_salt("environment-device-details")
                        .show(ui, |ui| {
                            self.environment_panel(ui, state, t, intents, false)
                        });
                    }
                    Nav::Todo => self.todo_page(ui, state, t, intents),
                    Nav::Displays => {
                        self.screen_controls(ui, state, t, intents);
                        ui.add_space(14.);
                        self.poem_card(ui, state, t, intents);
                        ui.add_space(14.);
                        self.environment_panel(ui, state, t, intents, true);
                    }
                    Nav::Usage => {
                        ui.columns(2, |columns| {
                            columns[0].label(
                                RichText::new("今日 Token · 已知合计")
                                    .font(bold(18.))
                                    .color(t.text),
                            );
                            columns[0]
                                .label(
                                    RichText::new(model::compact(model::today_total(
                                        state,
                                        model::day(),
                                    )))
                                    .font(bold(48.))
                                    .color(t.text),
                                )
                                .on_hover_text(
                                    "UTC+8 · 已知合计：只包含已取得的当天统计，缺失项不计入。",
                                );
                            columns[1].horizontal_wrapped(|ui| {
                                if ui
                                    .add_enabled(
                                        !state.refreshing_usage && !state.controlling_device,
                                        theme::primary_button(
                                            if state.refreshing_usage {
                                                "正在刷新…"
                                            } else {
                                                "刷新全部"
                                            },
                                            t,
                                        ),
                                    )
                                    .clicked()
                                {
                                    intents.push(Intent::Sync);
                                }
                                let settings = secondary(ui, "刷新设置", t);
                                self.target("refresh.settings", settings.rect);
                                if settings.clicked() {
                                    self.open_sheet(Sheet::Refresh);
                                }
                            });
                            self.refresh_summary(&mut columns[1], state, t, false);
                        });
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .selectable_label(self.usage_filter.is_none(), "全部")
                                .clicked()
                            {
                                self.usage_filter = None;
                            }
                            for r in state.readings.iter().filter(|r| r.packet.s != 0) {
                                let label = r.packet.n.as_deref().unwrap_or(match r.packet.p {
                                    0 => "Codex",
                                    1 => "Claude Code",
                                    2 => "DeepSeek",
                                    _ => "AI 服务",
                                });
                                let button = ui
                                    .selectable_label(self.usage_filter == Some(r.packet.p), label);
                                self.target(&format!("usage.provider.{}", r.packet.p), button.rect);
                                if button.clicked() {
                                    self.usage_filter = Some(r.packet.p);
                                }
                            }
                        });
                        ui.add_space(12.);
                        if self.usage_filter.is_none_or(|p| p == 0)
                            && !state.readings.iter().any(|r| r.packet.p == 0)
                        {
                            self.provider(
                                ui,
                                &usage::empty(0, 0, "点击刷新，读取这台 Mac 上的 Codex 官方数据。"),
                                t,
                                state,
                                intents,
                            );
                            ui.add_space(10.);
                        }
                        for reading in state.readings.iter().filter(|r| {
                            r.packet.p == 0
                                || r.packet.s != 0
                                || (r.packet.p == crate::harness::PROVIDER
                                    && !state.harness_devices.is_empty())
                        }) {
                            if self.usage_filter.is_some_and(|p| p != reading.packet.p) {
                                continue;
                            }
                            self.provider(ui, reading, t, state, intents);
                            ui.add_space(10.);
                        }
                    }
                    Nav::Automation => {
                        for (id, title, detail, route) in [
                            (
                                "automation.usage",
                                "同步 AI 用量",
                                if state.refresh.enabled {
                                    format!("每 {}", state.refresh.label())
                                } else {
                                    "已关闭".into()
                                },
                                0,
                            ),
                            (
                                "automation.connection",
                                "连接墨水屏",
                                if self.prefs.auto_connect {
                                    "启动时自动连接".into()
                                } else {
                                    "手动连接".into()
                                },
                                1,
                            ),
                            (
                                "automation.environment",
                                "温湿度记录",
                                "30 秒采样 · 每分钟保存".into(),
                                2,
                            ),
                            (
                                "automation.cloud",
                                "iCloud 同步",
                                if state.cloud.folder.is_some() {
                                    "每 30 秒交换".into()
                                } else {
                                    "未启用".into()
                                },
                                3,
                            ),
                        ] {
                            let response = theme::settings_link(ui, title, &detail, t);
                            self.target(id, response.rect);
                            if response.clicked() {
                                match route {
                                    0 => self.open_sheet(Sheet::Refresh),
                                    1 => {
                                        self.open_sheet(Sheet::Device);
                                        self.tab = Tab::Connection;
                                    }
                                    2 => self.nav = Nav::Climate,
                                    _ => self.open_sheet(Sheet::Cloud),
                                }
                            }
                        }
                    }
                    _ => {
                        ui.label(
                            RichText::new("从一块墨水屏开始")
                                .font(bold(27.))
                                .color(t.text),
                        );
                        ui.add_space(18.);
                        for (id, title, detail, route) in [
                            ("discover.usage", "AI 用量", "查看额度与 Token", 0),
                            ("discover.climate", "温湿度趋势", "查看环境记录", 1),
                            (
                                "discover.connection",
                                "连接 EPaper",
                                "安全添加或连接设备",
                                2,
                            ),
                        ] {
                            let response = theme::settings_link(ui, title, detail, t);
                            self.target(id, response.rect);
                            if response.clicked() {
                                match route {
                                    0 => self.nav = Nav::Usage,
                                    1 => self.nav = Nav::Climate,
                                    _ => self.open_sheet(Sheet::Add),
                                }
                            }
                        }
                        if primary(ui, "查看 EPaper", t).clicked() {
                            self.open_sheet(Sheet::Device);
                        }
                    }
                }
            });
    }
    pub fn render(&mut self, ui: &mut egui::Ui, state: &State) -> Vec<Intent> {
        let mut intents = Vec::new();
        let t = Tokens::home();
        self.targets.clear();
        if ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Comma,
            ))
        }) {
            self.open_sheet(Sheet::Settings);
        }
        if self.preferences_changed()
            && ui.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::S,
                ))
            })
        {
            intents.push(Intent::SavePreferences);
        }
        if self.sheet == Sheet::None
            && ui.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::R,
                ))
            })
        {
            match self.nav {
                Nav::Usage if !state.refreshing_usage && !state.refreshing_codex => {
                    intents.push(if self.usage_filter == Some(0) {
                        Intent::RefreshCodex
                    } else {
                        Intent::Sync
                    })
                }
                Nav::Climate
                    if state.connected && state.device_supported && !state.controlling_device =>
                {
                    intents.push(Intent::Device(crate::device::Action::Read))
                }
                _ => {}
            }
        }
        if self.prefs.dusk != self.wallpaper_dusk {
            (self.wallpaper, self.frosted_wallpaper) = theme::wallpaper(ui.ctx(), self.prefs.dusk);
            self.wallpaper_dusk = self.prefs.dusk;
        }
        let rect = ui.max_rect();
        let side = Rect::from_min_size(rect.min, egui::vec2(theme::SIDEBAR, rect.height()));
        let main_full = Rect::from_min_max(egui::pos2(side.right(), rect.top()), rect.max);
        let feedback_visible = state
            .feedback
            .as_ref()
            .is_some_and(|f| f.visible(usage::now()));
        let main = if feedback_visible && self.sheet == Sheet::None {
            Rect::from_min_max(main_full.min, main_full.max - egui::vec2(0., 44.))
        } else {
            main_full
        };
        ui.painter().image(
            self.wallpaper.id(),
            rect,
            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
        ui.painter().rect_filled(
            rect,
            0,
            Color32::from_black_alpha(if self.prefs.dusk { 110 } else { 135 }),
        );
        self.sidebar(ui, side, t);
        self.header(ui, main, t);
        match self.nav {
            Nav::Home => self.home_page(ui, main, state, t),
            Nav::Room => self.category(ui, main, state, t),
            _ => {
                let content = Rect::from_min_max(
                    main.min + egui::vec2(24., theme::TOOLBAR + 10.),
                    main.max - egui::vec2(24., 20.),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
                    self.panel_content(ui, state, t, &mut intents)
                });
            }
        }
        self.sync_connection_form(&mut intents);
        if self.sheet != Sheet::None {
            self.sheet_ui(ui, state, t, &mut intents);
        } else if feedback_visible {
            // Reserve a separate status row: it never overlays page controls.
            let status = Rect::from_min_size(
                egui::pos2(
                    (main_full.right() - 462.).max(main_full.left() + 24.),
                    main_full.bottom() - 40.,
                ),
                egui::vec2((main_full.width() - 48.).min(438.), 34.),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(status), |ui| {
                self.feedback_row(ui, state, t, &mut intents);
            });
        }
        self.sync_connection_form(&mut intents);
        if feedback_visible {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
        intents
    }
    fn sync_connection_form(&mut self, intents: &mut Vec<Intent>) {
        let active = self.sheet == Sheet::Add
            || (self.sheet == Sheet::Device && self.tab == Tab::Connection);
        if self.connection_form_active != active {
            self.connection_form_active = active;
            if !active {
                self.pairing_code.clear();
            }
            intents.push(Intent::ConnectionForm {
                active,
                disconnect: false,
            });
        }
    }
    fn feedback_row(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        let Some(f) = &state.feedback else { return };
        if !f.visible(usage::now()) {
            return;
        }
        egui::Frame::new()
            .fill(t.glass)
            .corner_radius(theme::CONTROL_RADIUS)
            .stroke(Stroke::new(1., t.line))
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.spacing_mut().interact_size.y = 22.;
                ui.horizontal(|ui| {
                    if f.busy {
                        ui.add(egui::Spinner::new().size(14.));
                    } else {
                        ui.label(
                            RichText::new(if f.error { "!" } else { "✓" }).color(if f.error {
                                t.error
                            } else {
                                t.safe
                            }),
                        );
                    }
                    let width = (ui.available_width() - 30.).max(40.);
                    ui.add_sized(
                        [width, 22.],
                        egui::Label::new(RichText::new(f.summary()).size(12.).color(t.text))
                            .truncate(),
                    )
                    .on_hover_text(&f.message);
                    let (rect, close) =
                        ui.allocate_exact_size(egui::vec2(22., 22.), Sense::click());
                    let close = close
                        .on_hover_text("关闭提示")
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    close.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "关闭提示")
                    });
                    if close.hovered() {
                        ui.painter().rect_filled(rect, 5, t.line);
                    }
                    draw(ui.painter(), rect.shrink(5.), Icon::Close, t.sheet_muted);
                    self.target("feedback.close", rect);
                    if close.clicked() {
                        intents.push(Intent::DismissFeedback);
                    }
                });
            });
    }
    fn provider(
        &mut self,
        ui: &mut egui::Ui,
        r: &Reading,
        t: Tokens,
        state: &State,
        intents: &mut Vec<Intent>,
    ) {
        let display = crate::harness::effective(r, &state.harness_devices);
        let r = &display;
        theme::surface(t).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let name = r.packet.n.as_deref().unwrap_or(match r.packet.p {
                0 => "Codex",
                1 => "Claude Code",
                2 => "DeepSeek",
                _ => "AI 服务",
            });
            ui.horizontal_wrapped(|ui| {
                if let Some(logo) = self
                    .brands
                    .get(if r.packet.p == crate::harness::PROVIDER {
                        2
                    } else {
                        usize::from(r.packet.p)
                    })
                    .and_then(Option::as_ref)
                {
                    ui.add(
                        egui::Image::new(logo)
                            .fit_to_exact_size(egui::vec2(25., 25.))
                            .corner_radius(theme::ICON_RADIUS),
                    );
                }
                if r.packet.p == crate::harness::PROVIDER {
                    ui.label(RichText::new("DeepSeek").font(bold(17.)).color(t.ink));
                    ui.label(RichText::new("Harness").font(bold(12.)).color(t.secondary));
                } else {
                    ui.label(RichText::new(name).font(bold(17.)).color(t.ink));
                }
                if r.packet.p == 0 {
                    if let Some(a) = &r.tokens.account {
                        ui.label(RichText::new(&a.label).size(12.).color(t.secondary))
                            .on_hover_text(format!(
                                "{} · {}",
                                a.device.name,
                                if a.verified {
                                    "官方登录账号"
                                } else {
                                    "设备来源 · 账号未提供"
                                }
                            ));
                    }
                    let account = ui.link("账号记录");
                    self.target("codex.accounts", account.rect);
                    if account.clicked() {
                        self.open_sheet(Sheet::Cloud);
                    }
                }
                if r.packet.p == crate::harness::PROVIDER && !r.tokens.devices.is_empty() {
                    ui.label(
                        RichText::new(format!("去重 · {} 台 Mac", r.tokens.devices.len()))
                            .size(12.)
                            .color(t.secondary),
                    );
                }
                if r.packet.p == 0 && state.refreshing_codex {
                    ui.add(egui::Spinner::new().size(15.))
                        .on_hover_text("正在读取 Codex…");
                } else if r.packet.p == 0 && r.packet.s == 0 {
                    ui.label(RichText::new("尚未读取").color(t.sheet_muted));
                } else if r.packet.s == 0 {
                    ui.label(RichText::new("未配置").color(t.sheet_muted));
                } else if r.packet.s != usage::OK {
                    ui.label(RichText::new("暂未更新").color(t.error));
                }
            });
            ui.add_space(10.);
            if let Some(source) = state.cloud_sources.get(&r.packet.p) {
                ui.label(RichText::new(source).size(12.).color(t.secondary));
            }
            if r.packet.p == 3 {
                let count = crate::hub::argus_sources(state).len();
                if count > 1 {
                    ui.label(
                        RichText::new(format!("共享文献库 · {count} 台 Mac 来源（只计一次）"))
                            .size(12.)
                            .color(t.secondary),
                    );
                }
            }
            if r.packet.p == crate::harness::PROVIDER {
                for device in state.fleet.values().filter(|d| d.connected) {
                    if let Some(notice) = &device.usage_notice {
                        ui.label(
                            RichText::new(format!("{} · {notice}", device.name))
                                .size(12.)
                                .color(t.error),
                        );
                    }
                }
                harness_view::show(ui, r, t, &mut self.harness_scope);
                return;
            }
            match r.packet.p {
                0 | 1 => {
                    ui.columns(2, |columns| {
                        for (i, label) in ["5 小时", "周额度"].iter().enumerate() {
                            if let Some(column) = columns.get_mut(i) {
                                let quota = r.packet.w.get(i).map_or(-1, |w| w[0]);
                                column.label(RichText::new(*label).color(t.sheet_muted));
                                let value = if quota >= 0 {
                                    format!("{:.0}%", quota as f64 / 10.)
                                } else {
                                    "未提供".into()
                                };
                                column.allocate_ui_with_layout(
                                    egui::vec2(column.available_width(), 40.),
                                    egui::Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(value)
                                                .font(bold(if i == 0 { 32. } else { 28. }))
                                                .color(t.ink),
                                        );
                                    },
                                );
                                if quota >= 0 {
                                    column.add(
                                        egui::ProgressBar::new(quota as f32 / 1000.)
                                            .fill(t.accent)
                                            .desired_height(5.),
                                    );
                                } else {
                                    column.allocate_space(egui::vec2(column.available_width(), 5.));
                                }
                            }
                        }
                    });
                }
                2 => {
                    ui.label(
                        RichText::new(
                            r.packet
                                .b
                                .map(|n| format!("{n:.2} {}", r.packet.c.as_deref().unwrap_or("")))
                                .unwrap_or_else(|| "未提供余额".into()),
                        )
                        .font(bold(27.))
                        .color(t.ink),
                    );
                }
                _ => {
                    let daily = r
                        .packet
                        .d
                        .filter(|d| d[0] == model::day() && d[1] >= 0)
                        .map(|d| d[1] as u64);
                    ui.columns(3, |columns| {
                        let month = r
                            .packet
                            .mt
                            .filter(|m| Some(m[0]) == model::month_start(model::day()))
                            .map(|m| m[1] as u64);
                        for (i, (label, value)) in [
                            ("今日", daily),
                            (
                                "本周",
                                r.packet
                                    .wt
                                    .filter(|w| Some(w[0]) == model::week_start(model::day()))
                                    .map(|w| w[1] as u64),
                            ),
                            ("本月", month),
                        ]
                        .iter()
                        .enumerate()
                        {
                            if let Some(column) = columns.get_mut(i) {
                                column.label(RichText::new(*label).color(t.sheet_muted));
                                column.label(
                                    RichText::new(model::compact(*value))
                                        .font(bold(if i == 0 { 30. } else { 25. }))
                                        .color(t.ink),
                                );
                            }
                        }
                    });
                }
            }
            ui.add_space(8.);
            egui::CollapsingHeader::new(if r.packet.s != 0 && r.packet.s != usage::OK {
                "错误详情"
            } else {
                "详细用量"
            })
            .id_salt(("provider-details", r.packet.p))
            .show_background(false)
            .show(ui, |ui| {
                match r.packet.p {
                    0 => {
                        ui.label(format!("累计 Token {}", number(r.tokens.lifetime)));
                        if let Some(weekly) = r
                            .packet
                            .wt
                            .filter(|w| Some(w[0]) == model::week_start(model::day()))
                        {
                            ui.label(format!("本周已知 Token {}", number(Some(weekly[1] as u64))));
                        }
                        if let Some(month) = r
                            .packet
                            .mt
                            .filter(|m| Some(m[0]) == model::month_start(model::day()))
                        {
                            ui.label(format!("本月已知 Token {}", number(Some(month[1] as u64))));
                        }
                        if let Some(date) = &r.tokens.day {
                            ui.label(format!(
                                "最近统计 {date} · {} Token",
                                number(r.tokens.recent_day)
                            ));
                        }
                    }
                    1 => {
                        ui.label(format!(
                            "上下文输入 {} · 输出 {}",
                            number(r.tokens.input),
                            number(r.tokens.output)
                        ));
                    }
                    3.. => {
                        if r.packet.p == 3 {
                            for (origin, source) in crate::hub::argus_sources(state) {
                                ui.label(format!(
                                    "{} · 今日 {} · 本周 {} · 本月 {}",
                                    origin.name,
                                    model::compact(
                                        source
                                            .packet
                                            .d
                                            .filter(|d| d[0] == model::day() && d[1] >= 0)
                                            .map(|d| d[1] as u64)
                                    ),
                                    model::compact(
                                        source
                                            .packet
                                            .wt
                                            .filter(
                                                |d| Some(d[0]) == model::week_start(model::day())
                                            )
                                            .map(|d| d[1] as u64)
                                    ),
                                    model::compact(
                                        source
                                            .packet
                                            .mt
                                            .filter(
                                                |d| Some(d[0]) == model::month_start(model::day())
                                            )
                                            .map(|d| d[1] as u64)
                                    )
                                ));
                            }
                        }
                        if r.packet.p == crate::harness::PROVIDER {
                            for device in &r.tokens.devices {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new(&device.name).font(bold(13.)));
                                    ui.label(format!(
                                        "今日 {} · 本周 {}",
                                        model::compact(
                                            (device.d[0] == model::day())
                                                .then_some(device.d[1] as u64)
                                        ),
                                        model::compact(
                                            (Some(device.wt[0]) == r.packet.wt.map(|w| w[0]))
                                                .then_some(device.wt[1] as u64)
                                        )
                                    ))
                                    .on_hover_text(
                                        chrono::DateTime::from_timestamp(device.t, 0)
                                            .map(|d| {
                                                format!(
                                                    "最后同步 {}",
                                                    d.with_timezone(&chrono::Local)
                                                        .format("%m/%d %H:%M")
                                                )
                                            })
                                            .unwrap_or_default(),
                                    );
                                });
                            }
                            ui.add_space(8.);
                        }
                        ui.label(format!(
                            "输入 {} · 输出 {}",
                            number(r.tokens.input),
                            number(r.tokens.output)
                        ));
                        ui.label(format!(
                            "缓存命中 {} · {} 次调用",
                            number(r.tokens.cached),
                            number(r.tokens.calls)
                        ))
                        .on_hover_text("缓存命中已包含在输入 Token 中。");
                        ui.label(
                            r.tokens
                                .cost_usd
                                .map(|n| format!("已记录费用 ${n:.3}"))
                                .unwrap_or_else(|| "费用未记录".into()),
                        );
                    }
                    _ => {}
                }
                if !r.tokens.note.is_empty() {
                    ui.label(RichText::new(&r.tokens.note).size(12.).color(t.sheet_muted));
                }
            });
            if r.packet.p == 0 {
                self.codex_actions(ui, r, state, t, intents);
            }
        });
    }
    fn codex_actions(
        &mut self,
        ui: &mut egui::Ui,
        r: &Reading,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        ui.add_space(4.);
        let stamp = update_stamp(r.packet.t, r.packet.s == usage::OK);
        let (_, stamp_rect) = theme::action_row(
            ui,
            (r.packet.s != 0).then_some((stamp.0.as_str(), stamp.1.as_str())),
            t,
            |ui| {
                let busy = state.checking_cli || state.refreshing_codex || state.refreshing_usage;
                let check = ui
                    .add_enabled(
                        !busy,
                        theme::secondary_button(
                            if state.checking_cli {
                                "检查中…"
                            } else {
                                "检查 CLI"
                            },
                            t,
                        )
                        .min_size(egui::vec2(95., 34.)),
                    )
                    .on_hover_text("检查本机 Codex CLI 路径与版本");
                self.target("codex.card.check", check.rect);
                if check.clicked() {
                    intents.push(Intent::CheckCodex(crate::codex_cli::Settings {
                        codex_cli: self.prefs.codex_cli.clone(),
                        codex_node: self.prefs.codex_node.clone(),
                    }));
                }
                let refresh = ui
                    .add_enabled(
                        !busy,
                        theme::primary_button(
                            if state.refreshing_codex {
                                "刷新中…"
                            } else {
                                "刷新"
                            },
                            t,
                        )
                        .min_size(egui::vec2(96., 34.)),
                    )
                    .on_hover_text("立即重新读取 Codex 官方用量");
                self.target("codex.card.refresh", refresh.rect);
                if refresh.clicked() {
                    intents.push(Intent::RefreshCodex);
                }
                let settings = secondary(ui, "设置", t).on_hover_text("Codex CLI 设置");
                self.target("codex.card.settings", settings.rect);
                if settings.clicked() {
                    self.open_sheet(Sheet::Codex);
                }
            },
        );
        if let Some(rect) = stamp_rect {
            self.target("codex.card.updated", rect);
        }
    }
    fn refresh_summary(&self, ui: &mut egui::Ui, state: &State, t: Tokens, sheet: bool) {
        let color = if sheet { t.sheet_muted } else { t.secondary };
        ui.horizontal_wrapped(|ui| {
            if state.refreshing_usage {
                ui.spinner();
                ui.label(RichText::new("正在刷新…").size(12.).color(color));
            } else {
                let label = if state.refresh.enabled {
                    format!("每 {}刷新", state.refresh.label())
                } else {
                    "手动刷新".into()
                };
                let text = state
                    .last_read
                    .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
                    .map(|at| {
                        format!(
                            "{label} · 更新于 {}",
                            at.with_timezone(&chrono::Local).format("%H:%M")
                        )
                    })
                    .unwrap_or(label);
                let response = ui.label(RichText::new(text).size(12.).color(color));
                if let Some(next) = state
                    .next_read
                    .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
                {
                    response.on_hover_text(format!(
                        "下次刷新 {}",
                        next.with_timezone(&chrono::Local).format("%H:%M:%S")
                    ));
                }
            }
        });
    }

    fn refresh_settings(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        let enabled = ui.checkbox(&mut self.prefs.refresh.enabled, "自动刷新");
        self.target("refresh.enabled", enabled.rect);
        ui.add_enabled_ui(self.prefs.refresh.enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("刷新间隔");
                let interval = ui.add(
                    egui::DragValue::new(&mut self.prefs.refresh.seconds)
                        .range(30..=7200)
                        .speed(30)
                        .suffix(" 秒"),
                );
                self.target("refresh.interval", interval.rect);
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("常用").size(12.).color(t.sheet_muted));
                for seconds in [30, 60, 300, 1800] {
                    let preset = ui.selectable_label(
                        self.prefs.refresh.seconds == seconds,
                        crate::refresh::Settings {
                            seconds,
                            ..self.prefs.refresh
                        }
                        .label(),
                    );
                    self.target(&format!("refresh.preset.{seconds}"), preset.rect);
                    if preset.clicked() {
                        self.prefs.refresh.seconds = seconds;
                    }
                }
            });
        });
        let changed = self.prefs.refresh != state.refresh;
        ui.add_space(4.);
        let note = if changed {
            "未保存".to_owned()
        } else if !state.refresh.enabled {
            "自动刷新已关闭".into()
        } else {
            let at = state
                .last_read
                .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
                .map(|at| at.with_timezone(&chrono::Local).format("%H:%M").to_string());
            at.map_or_else(
                || format!("每 {}", state.refresh.label()),
                |at| format!("每 {} · 更新于 {at}", state.refresh.label()),
            )
        };
        theme::action_row(
            ui,
            Some((note.as_str(), "自定义间隔支持 30 秒–120 分钟")),
            t,
            |ui| {
                let save = ui.add_enabled(
                    changed && self.prefs.validate(),
                    theme::primary_button("保存", t),
                );
                self.target("refresh.save", save.rect);
                if save.clicked() {
                    intents.push(Intent::SavePreferences);
                }
            },
        );
    }
    fn general_settings(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        ui.columns(2, |cols| {
            cols[0].label(RichText::new("家庭名称").size(12.).color(t.secondary));
            cols[0].add(
                theme::text_field(&mut self.prefs.home)
                    .desired_width(f32::INFINITY)
                    .char_limit(32),
            );
            cols[1].label(RichText::new("房间").size(12.).color(t.secondary));
            cols[1].add(
                theme::text_field(&mut self.prefs.room)
                    .desired_width(f32::INFINITY)
                    .char_limit(32),
            );
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.prefs.auto_connect, "启动时连接已绑定设备");
            ui.checkbox(&mut self.prefs.dusk, "暮色背景");
        });
        theme::section(ui, "天气", t);
        let summary = weather_view::summary(&self.prefs.weather);
        let link = theme::settings_link(ui, "天气地点", &summary, t);
        self.target("weather.settings", link.rect);
        if link.clicked() {
            self.open_sheet(Sheet::Weather);
        }
        let dirty = self.preferences_changed();
        theme::action_row(
            ui,
            dirty.then_some(("未保存", "⌘S 保存修改")),
            t,
            |ui| {
                let save = ui
                    .add_enabled(
                        dirty && self.prefs.validate(),
                        theme::primary_button("保存设置", t),
                    )
                    .on_hover_text("保存本页修改 · ⌘S");
                self.target("preferences.save", save.rect);
                if save.clicked() {
                    intents.push(Intent::SavePreferences);
                }
            },
        );
        if self.prefs.home.trim().is_empty() || self.prefs.room.trim().is_empty() {
            ui.label(
                RichText::new("家庭名称和房间不能为空")
                    .size(12.)
                    .color(t.error),
            );
        }
        theme::section(ui, "连接与数据", t);
        for (id, title, detail, sheet) in [
            (
                "refresh.settings",
                "用量刷新",
                if state.refresh.enabled {
                    format!("每 {}", state.refresh.label())
                } else {
                    "手动刷新".into()
                },
                Sheet::Refresh,
            ),
            (
                "cloud.settings",
                "iCloud 与来源",
                if state.cloud.folder.is_some() {
                    "已启用 · 查看账号与历史".into()
                } else {
                    "未启用".into()
                },
                Sheet::Cloud,
            ),
            (
                "codex.settings",
                "Codex CLI",
                if self.prefs.codex_cli.is_empty() {
                    "自动查找".into()
                } else {
                    "自定义路径".into()
                },
                Sheet::Codex,
            ),
        ] {
            let response = theme::settings_link(ui, title, &detail, t);
            self.target(id, response.rect);
            if response.clicked() {
                self.open_sheet(sheet);
            }
        }
        theme::section(ui, "启动", t);
        let mut login_enabled = self.login_item.selected();
        let login = ui.add_enabled(
            self.login_item != epaper_macos::LoginItemStatus::Unavailable,
            egui::Checkbox::new(&mut login_enabled, "开机自启动（登录 Mac 后）"),
        );
        self.target("startup.enabled", login.rect);
        if login.changed() {
            intents.push(Intent::SetLoginItem(login_enabled));
        }
        ui.label(
            RichText::new(match self.login_item {
                epaper_macos::LoginItemStatus::RequiresApproval => {
                    "等待系统允许，请在“登录项”中允许 HomeCraft。"
                }
                epaper_macos::LoginItemStatus::Unavailable => {
                    "自启动需要 macOS 13 或更新版本，并从 HomeCraft.app 设置。"
                }
                _ => "此开关立即生效；自动启动后在菜单栏同步。",
            })
            .size(12.)
            .color(t.secondary),
        );
        if self.login_item == epaper_macos::LoginItemStatus::RequiresApproval {
            let settings = secondary(ui, "打开系统登录项", t);
            self.target("startup.system", settings.rect);
            if settings.clicked() {
                intents.push(Intent::OpenLoginItems);
            }
        }
        egui::CollapsingHeader::new("其他服务").show(ui, |ui| {
            if secondary(ui, "启用 Claude Code 状态栏", t).clicked() {
                intents.push(Intent::InstallClaude);
            }
            if secondary(ui, "打开配置目录", t).clicked() {
                intents.push(Intent::OpenProject);
            }
        });
        ui.add_space(4.);
        theme::action_row(
            ui,
            Some((
                concat!("HomeCraft ", env!("CARGO_PKG_VERSION")),
                "Apple 芯片 · 本机设置",
            )),
            t,
            |ui| {
                let update = secondary(ui, "软件更新", t);
                self.target("update.settings", update.rect);
                if update.clicked() {
                    self.open_sheet(Sheet::Update);
                }
                if secondary(ui, "帮助", t).clicked() {
                    self.open_sheet(Sheet::Help);
                }
                if secondary(ui, "退出程序", t).clicked() {
                    intents.push(Intent::Quit);
                }
            },
        );
    }
    fn cli_settings(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        ui.label(RichText::new("Codex CLI").font(bold(16.)));
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 116.).max(100.);
            ui.add(
                theme::text_field(&mut self.prefs.codex_cli)
                    .hint_text("自动检测 Codex CLI")
                    .desired_width(width)
                    .char_limit(4096),
            );
            let pick = secondary(ui, "选择文件…", t);
            self.target("codex.pick", pick.rect);
            if pick.clicked() {
                intents.push(Intent::PickCodex);
            }
        });
        egui::CollapsingHeader::new("Node.js").show(ui, |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 116.).max(100.);
                ui.add(
                    theme::text_field(&mut self.prefs.codex_node)
                        .hint_text("自动检测 Node.js")
                        .desired_width(width)
                        .char_limit(4096),
                );
                if secondary(ui, "选择文件…", t).clicked() {
                    intents.push(Intent::PickNode);
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            let check = ui.add_enabled(
                !state.checking_cli && !state.refreshing_codex && !state.refreshing_usage,
                theme::secondary_button(
                    if state.checking_cli {
                        "正在检测…"
                    } else {
                        "检测当前设置"
                    },
                    t,
                ),
            );
            self.target("codex.check", check.rect);
            if check.clicked() {
                intents.push(Intent::CheckCodex(crate::codex_cli::Settings {
                    codex_cli: self.prefs.codex_cli.clone(),
                    codex_node: self.prefs.codex_node.clone(),
                }));
            }
            let save = ui.add_enabled(
                self.preferences_changed() && self.prefs.validate(),
                theme::primary_button("保存", t),
            );
            self.target("codex.save", save.rect);
            if save.clicked() {
                intents.push(Intent::SavePreferences);
            }
            let reset = ui.add_enabled(
                !self.prefs.codex_cli.is_empty() || !self.prefs.codex_node.is_empty(),
                theme::secondary_button("恢复自动检测", t),
            );
            self.target("codex.reset", reset.rect);
            if reset.clicked() {
                self.prefs.codex_cli.clear();
                self.prefs.codex_node.clear();
                intents.push(Intent::SavePreferences);
                intents.push(Intent::CheckCodex(crate::codex_cli::Settings::default()));
            }
        });
        if let Some(report) = &state.cli_diagnostic {
            ui.label(
                RichText::new(format!("上次检测：{}", report.message))
                    .size(12.)
                    .color(if report.ok { t.safe } else { t.error }),
            );
            egui::CollapsingHeader::new("诊断详情").show(ui, |ui| {
                for text in [
                    &report.executable,
                    &report.node,
                    &report.version,
                    &report.source,
                ] {
                    if !text.is_empty() {
                        ui.add(egui::Label::new(RichText::new(text).monospace().size(12.)).wrap());
                    }
                }
            });
        }
    }
    fn connection_settings(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(20., 20.), Sense::hover());
            draw(ui.painter(), rect, Icon::Lock, t.safe);
            ui.label(RichText::new("蓝牙连接").font(bold(17.)).color(t.ink));
        });
        ui.add_space(8.);
        if state.devices.is_empty() {
            theme::surface(t).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(if state.scanning {
                    "正在查找附近的墨水屏…"
                } else if !state.bound_name.is_empty() {
                    "暂未收到已记住设备的广播"
                } else {
                    "暂未收到墨水屏广播"
                });
                if !state.bound_name.is_empty() {
                    ui.label(RichText::new(&state.bound_name).font(bold(15.)));
                }
                ui.label(
                    RichText::new("设备可能正由另一台 Mac 连接。若正在切换电脑，请先在原电脑暂停同步，再重新扫描。")
                        .color(t.sheet_muted),
                );
                if !state.bound_name.is_empty() {
                    ui.label(RichText::new("原绑定已保留，无需重新配对。").color(t.sheet_muted));
                }
            });
        }
        for device in &state.devices {
            let chosen = self.selected == device.id;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 66.), Sense::click());
            self.target(&format!("connection.device.{}", device.id), rect);
            ui.painter().rect_filled(
                rect,
                theme::SURFACE_RADIUS,
                if response.hovered() {
                    t.tile_hover
                } else {
                    t.card
                },
            );
            if chosen {
                ui.painter().rect_stroke(
                    rect,
                    theme::SURFACE_RADIUS,
                    Stroke::new(1.5, t.accent),
                    egui::StrokeKind::Inside,
                );
            }
            draw(
                ui.painter(),
                Rect::from_min_size(rect.min + egui::vec2(13., 17.), egui::vec2(28., 28.)),
                Icon::Display,
                t.accent,
            );
            let name = theme::fitted_text(
                ui.painter(),
                &device.name,
                bold(15.),
                t.ink,
                rect.width() - 148.,
            );
            ui.painter().galley(
                rect.min + egui::vec2(56., 22. - name.size().y / 2.),
                name,
                t.ink,
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, &device.name)
            });
            response
                .clone()
                .on_hover_text(&device.name)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().text(
                rect.min + egui::vec2(56., 45.),
                Align2::LEFT_CENTER,
                if state.fleet.get(&device.id).is_some_and(|s| s.connected) {
                    "已连接 · 正在同步"
                } else if state.fleet.get(&device.id).is_some_and(|s| s.paused) {
                    "已暂停"
                } else if device.recovered {
                    "已记住 · 暂未收到广播"
                } else if device.bound {
                    "已记住"
                } else {
                    if device.enrollment {
                        "等待配对"
                    } else {
                        "未配对"
                    }
                },
                font(12.),
                t.sheet_muted,
            );
            if let Some(rssi) = device.rssi {
                ui.painter().text(
                    rect.right_top() + egui::vec2(-14., 25.),
                    Align2::RIGHT_CENTER,
                    format!("{rssi} dBm"),
                    font(12.),
                    t.sheet_muted,
                );
            }
            if response.clicked() {
                self.selected = device.id.clone();
                intents.push(Intent::ConnectionForm {
                    active: true,
                    disconnect: false,
                });
            }
        }
        ui.add_space(12.);
        let selected_active = self.selected == state.active_device
            || (state.active_device.is_empty() && state.fleet.is_empty());
        let selected_connected = selected_active && state.connected;
        let selected_connecting = selected_active && state.connecting;
        let background_connected = state.fleet.get(&self.selected).is_some_and(|d| d.connected);
        ui.horizontal(|ui| {
            let scan = ui.add_enabled(
                !state.scanning && !state.pairing,
                theme::secondary_button(
                    if state.scanning {
                        "正在扫描…"
                    } else {
                        "重新扫描"
                    },
                    t,
                ),
            );
            self.target("connection.scan", scan.rect);
            if scan.clicked() {
                intents.push(Intent::Scan);
            }
            let pause = ui.add_enabled(
                !state.pairing
                    && (selected_connected || selected_connecting || background_connected),
                theme::secondary_button("暂停这台设备", t),
            );
            self.target("connection.disconnect", pause.rect);
            if pause.clicked() {
                intents.push(Intent::Disconnect);
            }
        });
        if state.devices.is_empty() && !state.bound_name.is_empty() {
            return;
        }
        ui.add_space(16.);
        ui.separator();
        ui.add_space(12.);
        let selected = state.devices.iter().find(|d| d.id == self.selected);
        ui.label(RichText::new("配对码").font(bold(15.)).color(t.ink));
        let needs_code = selected.is_some_and(|d| !d.bound || d.enrollment);
        if needs_code {
            ui.label(
                RichText::new("E1002长按左键2秒；旧屏在连接页双击BOOT，获取8位码。")
                    .size(12.)
                    .color(t.sheet_muted),
            );
        }
        let field = ui.add_enabled(
            selected.is_some() && !state.pairing && !selected_connected,
            theme::text_field(&mut *self.pairing_code)
                .password(true)
                .char_limit(8)
                .hint_text(if selected.is_none() {
                    "请先选择设备"
                } else if needs_code {
                    "8 位屏幕码"
                } else {
                    "已记住的设备可留空"
                })
                .desired_width(240.),
        );
        self.target("connection.code", field.rect);
        self.pairing_code.retain(|ch| ch.is_ascii_digit());
        if field.changed() {
            intents.push(Intent::ConnectionForm {
                active: true,
                disconnect: true,
            });
        }
        ui.add_space(8.);
        let ready = !state.pairing
            && !selected_connected
            && !selected_connecting
            && !state.scanning
            && selected
                .is_some_and(|d| connection_ready(d.bound, d.enrollment, &self.pairing_code));
        let connect = ui.add_enabled(
            ready,
            theme::primary_button(
                if selected_connected {
                    "已连接"
                } else if state.pairing {
                    "正在认证…"
                } else if selected_connecting {
                    "正在连接…"
                } else if background_connected {
                    "设为当前控制设备"
                } else {
                    "安全连接"
                },
                t,
            )
            .min_size(egui::vec2(240., 34.)),
        );
        self.target("connection.connect", connect.rect);
        if connect.clicked() {
            if self.pairing_code.is_empty() {
                intents.push(Intent::Connect(self.selected.clone()));
            } else {
                intents.push(Intent::PairBluetooth {
                    id: self.selected.clone(),
                    code: zeroize::Zeroizing::new(std::mem::take(&mut *self.pairing_code)),
                });
            }
        }
        if !self.pairing_code.is_empty() && self.pairing_code.len() < 8 {
            ui.label(
                RichText::new("请输入完整的 8 位码")
                    .size(12.)
                    .color(t.sheet_muted),
            );
        }
        ui.add_space(8.);
        let help = ui.link("连接帮助");
        self.target("connection.help", help.rect);
        if help.clicked() {
            self.open_sheet(Sheet::Help);
        }
        egui::CollapsingHeader::new("维护选项").show(ui, |ui| {
            if secondary(ui, "USB 维护绑定", t).clicked() {
                intents.push(Intent::Pair);
            }
        });
        if let Some(error) = &state.connection_error {
            egui::CollapsingHeader::new("查看连接详情").show(ui, |ui| {
                ui.label(RichText::new(error).size(12.).color(t.error));
            });
        }
    }
    fn screen_controls(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        theme::surface(t).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let enabled = state.connected && state.device_supported && !state.controlling_device;
            ui.horizontal(|ui| {
                ui.label(RichText::new("显示内容").font(bold(16.)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let clear = environment_view::icon_button(
                        ui,
                        Icon::Refresh,
                        "整屏清除残影",
                        enabled,
                        t,
                        false,
                    );
                    self.target("screen.refresh", clear.rect);
                    if clear.clicked() {
                        intents.push(Intent::Device(crate::device::Action::Refresh));
                    }
                });
            });
            if let Some(d) = &state.telemetry {
                ui.label(
                    RichText::new(format!(
                        "{}{}{}",
                        if state.connected {
                            "当前页面："
                        } else {
                            "上次页面："
                        },
                        d.page_label(),
                        if d.busy { " · 正在刷新…" } else { "" }
                    ))
                    .color(t.secondary),
                );
            } else {
                ui.label(
                    RichText::new(if state.connected {
                        "正在读取设备状态…"
                    } else {
                        "连接 EPaper 后，可在这里切换页面。"
                    })
                    .color(t.secondary),
                );
            }
            if !state.connected {
                let connect = ui.link("连接设备");
                self.target("screen.connect", connect.rect);
                if connect.clicked() {
                    self.open_sheet(Sheet::Add);
                }
            }
            ui.add_space(8.);
            ui.add_enabled_ui(enabled, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (index, (page, label)) in crate::device::PAGES.iter().enumerate() {
                        if state
                            .telemetry
                            .as_ref()
                            .is_none_or(|d| d.pages & (1 << index) == 0)
                        {
                            continue;
                        }
                        let active = state.telemetry.as_ref().is_some_and(|d| d.page == *page);
                        let response = ui.add(theme::secondary_button(label, t).fill(if active {
                            Color32::from_rgba_unmultiplied(255, 151, 41, 64)
                        } else {
                            t.field
                        }));
                        self.target(&format!("screen.{page}"), response.rect);
                        if response.clicked() {
                            intents
                                .push(Intent::Device(crate::device::Action::Page((*page).into())));
                        }
                    }
                });
                if !state.rotation_supported {
                    return;
                }
                ui.add_space(8.);
                ui.horizontal_wrapped(|ui| {
                    let automatic = state.telemetry.as_ref().is_none_or(|d| d.auto_rotate);
                    for (rotation, label) in [
                        (None, "自动方向"),
                        (Some(0), "横屏"),
                        (Some(1), "竖屏"),
                        (Some(2), "反向横屏"),
                        (Some(3), "反向竖屏"),
                    ] {
                        let active = if let Some(r) = rotation {
                            !automatic && state.telemetry.as_ref().is_some_and(|d| d.rotation == r)
                        } else {
                            automatic
                        };
                        let response = ui.add(theme::secondary_button(label, t).fill(if active {
                            Color32::from_rgba_unmultiplied(255, 151, 41, 64)
                        } else {
                            t.field
                        }));
                        self.target(
                            &format!(
                                "screen.rotation.{}",
                                rotation.map(|r| r.to_string()).unwrap_or("auto".into())
                            ),
                            response.rect,
                        );
                        if response.clicked() {
                            intents.push(Intent::Device(crate::device::Action::Rotate(rotation)));
                        }
                    }
                });
            });
        });
    }
    fn environment_panel(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
        compact: bool,
    ) {
        theme::surface(t).show(ui, |ui| {
            if compact {
                ui.label(RichText::new("温湿度").font(bold(16.)).color(t.text));
                ui.add_space(14.);
            }
            environment_view::readings(ui, state, t, compact);
            ui.add_space(2.);
            ui.horizontal(|ui| {
                environment_view::status(ui, state, t);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let detail_theme = if state
                        .telemetry
                        .as_ref()
                        .is_some_and(|d| d.sd.0 == 0 || d.sd.3 == 0 || d.sd.5 != 0)
                    {
                        Tokens {
                            secondary: t.error,
                            ..t
                        }
                    } else {
                        t
                    };
                    let detail = environment_view::icon_button(
                        ui,
                        Icon::More,
                        "设备详情",
                        true,
                        detail_theme,
                        false,
                    );
                    self.target("environment.details", detail.rect);
                    if detail.clicked() {
                        self.open_sheet(Sheet::Environment);
                    }
                    let read = environment_view::icon_button(
                        ui,
                        Icon::Refresh,
                        if state.controlling_device {
                            "正在更新…"
                        } else {
                            "刷新温湿度"
                        },
                        state.connected && state.device_supported && !state.controlling_device,
                        t,
                        state.controlling_device,
                    );
                    self.target("device.read", read.rect);
                    if read.clicked() {
                        intents.push(Intent::Device(crate::device::Action::Read));
                    }
                    if compact && state.history_supported && ui.link("今日趋势").clicked() {
                        self.sheet = Sheet::None;
                        self.nav = Nav::Climate;
                    }
                });
            });
            if !compact {
                environment_view::other_sources(ui, state, t);
            }
        });
        if compact {
            return;
        }
        ui.add_space(16.);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(
                    if state
                        .history
                        .as_ref()
                        .is_some_and(|h| h.day as i64 != model::day())
                    {
                        "历史趋势"
                    } else {
                        "今日趋势"
                    },
                )
                .font(bold(16.))
                .color(t.text),
            );
            if let Some(h) = state.history.as_ref().filter(|h| h.day > 0) {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}-{:02}-{:02}",
                            h.day / 10000,
                            h.day / 100 % 100,
                            h.day % 100
                        ))
                        .size(12.)
                        .color(t.muted),
                    );
                });
            }
        });
        ui.add_space(12.);
        if !state.history_supported {
            ui.add_space(24.);
            ui.label(RichText::new("这台设备不提供温湿度历史曲线").color(t.secondary));
            ui.add_space(24.);
        } else if let Some(history) = &state.history {
            if state
                .telemetry
                .as_ref()
                .is_some_and(|d| d.sd.1 != history.day)
            {
                ui.label(RichText::new("正在更新今日记录…").color(t.secondary));
            } else {
                self.environment_chart(ui, history, true, t);
                ui.add_space(12.);
                self.environment_chart(ui, history, false, t);
            }
        } else {
            ui.add_space(24.);
            ui.label(RichText::new("暂无记录").color(t.secondary));
            ui.add_space(24.);
        }
    }
    fn environment_chart(
        &mut self,
        ui: &mut egui::Ui,
        history: &crate::device::History,
        temperature: bool,
        t: Tokens,
    ) {
        let rect = chart::show(ui, history, temperature, t, &mut self.environment_selection);
        self.target(
            if temperature {
                "chart.temperature"
            } else {
                "chart.humidity"
            },
            rect,
        );
    }
    fn device_overview(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 130.), Sense::hover());
        let frame =
            Rect::from_center_size(rect.center() - egui::vec2(0., 4.), egui::vec2(185., 112.));
        ui.painter()
            .rect_filled(frame.translate(egui::vec2(0., 4.)), 12, t.field);
        ui.painter().rect_filled(frame, 12, t.card);
        ui.painter().rect_stroke(
            frame,
            12,
            Stroke::new(1., t.field),
            egui::StrokeKind::Inside,
        );
        let screen = frame.shrink(11.);
        ui.painter().rect_filled(screen, 4, t.field);
        ui.painter().text(
            screen.left_top() + egui::vec2(9., 8.),
            Align2::LEFT_TOP,
            state
                .telemetry
                .as_ref()
                .map(|d| d.page_label())
                .unwrap_or("EPaper"),
            bold(10.),
            t.ink,
        );
        ui.painter().line_segment(
            [
                screen.min + egui::vec2(9., 26.),
                screen.right_top() + egui::vec2(-9., 26.),
            ],
            Stroke::new(1., t.sheet_muted),
        );
        for i in 0..2 {
            draw(
                ui.painter(),
                Rect::from_min_size(
                    screen.min + egui::vec2(12. + i as f32 * 77., 39.),
                    egui::vec2(24., 24.),
                ),
                if i == 0 { Icon::Usage } else { Icon::Climate },
                t.sheet_muted,
            );
        }
        ui.add_space(4.);
        ui.columns(2, |columns| {
            if let Some(column) = columns.get_mut(0) {
                metric(column, "连接状态", model::connection(state), t);
            }
            if let Some(column) = columns.get_mut(1) {
                let value = state
                    .synced
                    .and_then(|n| chrono::DateTime::from_timestamp(n, 0))
                    .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
                    .unwrap_or_else(|| "—".into());
                metric(column, "最近同步", &value, t);
            }
        });
        ui.add_space(12.);
        metric(
            ui,
            "今日 Token · 已知合计",
            &model::compact(model::today_total(state, model::day())),
            t,
        );
        ui.add_space(12.);
        self.screen_controls(ui, state, t, intents);
        ui.add_space(12.);
        self.environment_panel(ui, state, t, intents, true);
        ui.add_space(10.);
        ui.horizontal(|ui| {
            if primary(ui, "立即同步", t).clicked() {
                intents.push(Intent::Sync);
            }
            if secondary(ui, "连接设置", t).clicked() {
                self.tab = Tab::Connection;
            }
        });
    }
    fn sheet_ui(&mut self, ui: &mut egui::Ui, state: &State, t: Tokens, intents: &mut Vec<Intent>) {
        let sheet = self.sheet;
        let modal_id = egui::Id::new("home-sheet-v2");
        let modal = egui::Modal::new(modal_id)
            .area(egui::Modal::default_area(modal_id).default_size(egui::vec2(
                582.,
                (ui.ctx().content_rect().height() - 40.).min(720.),
            )))
            .frame(theme::modal(t))
            .backdrop_color(t.backdrop)
            .show(ui.ctx(), |ui| {
                let material = ui.painter().add(egui::Shape::Noop);
                ui.set_width((ui.ctx().content_rect().width() - 96.).min(530.));
                // An Area reuses the previous sheet's measured size. Reopen the
                // vertical layout budget so returning from a short child can grow.
                ui.set_max_height((ui.ctx().content_rect().height() - 92.).max(260.));
                ui.horizontal(|ui| {
                    if !self.sheet_history.is_empty() {
                        let back =
                            environment_view::icon_button(ui, Icon::Back, "返回", true, t, false);
                        self.target("sheet.back", back.rect);
                        if back.clicked() {
                            self.back_sheet();
                        }
                    }
                    let title = match sheet {
                        Sheet::Device => "EPaper",
                        Sheet::Add => "添加配件",
                        Sheet::Help => "帮助",
                        Sheet::Codex => "Codex 设置",
                        Sheet::Environment => "设备状态",
                        Sheet::Cloud => "iCloud 与来源",
                        Sheet::Refresh => "刷新设置",
                        Sheet::Update => "软件更新",
                        Sheet::Weather => "天气",
                        Sheet::Todo
                            if self
                                .todo_draft
                                .todo
                                .as_ref()
                                .is_some_and(|t| t.updated == 0) =>
                        {
                            "新建待办"
                        }
                        Sheet::Todo => "编辑待办",
                        _ => "家庭设置",
                    };
                    ui.label(RichText::new(title).font(bold(23.)).color(t.ink));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let close = environment_view::icon_button(
                            ui,
                            Icon::Close,
                            "关闭 · Esc",
                            true,
                            t,
                            false,
                        );
                        self.target("sheet.close", close.rect);
                        if close.clicked() {
                            self.close_sheet();
                        }
                    });
                });
                self.feedback_row(ui, state, t, intents);
                if sheet == Sheet::Device {
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            self.prefs.room,
                            model::connection(state)
                        ))
                        .color(t.sheet_muted),
                    );
                    ui.add_space(10.);
                    ui.horizontal(|ui| {
                        for (tab, label) in [
                            (Tab::Overview, "概览"),
                            (Tab::Usage, "AI 用量"),
                            (Tab::Connection, "连接"),
                        ] {
                            let active = self.tab == tab;
                            let r = ui.add(theme::secondary_button(label, t).fill(if active {
                                Color32::from_rgba_unmultiplied(255, 151, 41, 64)
                            } else {
                                t.field
                            }));
                            let id = match tab {
                                Tab::Overview => "overview",
                                Tab::Usage => "usage",
                                Tab::Connection => "connection",
                            };
                            self.target(&format!("tab.{id}"), r.rect);
                            if r.clicked() {
                                self.tab = tab;
                            }
                        }
                    });
                    ui.add_space(12.);
                } else {
                    ui.add_space(16.);
                }
                egui::ScrollArea::vertical()
                    .id_salt((
                        "sheet-scroll",
                        format!("{:?}", self.sheet),
                        format!("{:?}", self.tab),
                    ))
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .max_height(
                        (ui.ctx().content_rect().height()
                            - if sheet == Sheet::Device { 230. } else { 144. })
                        .max(180.),
                    )
                    .show(ui, |ui| {
                        match sheet {
                            Sheet::Device => match self.tab {
                                Tab::Overview => self.device_overview(ui, state, t, intents),
                                Tab::Usage => {
                                    if !state.readings.iter().any(|r| r.packet.p == 0) {
                                        self.provider(
                                            ui,
                                            &usage::empty(0, 0, "点击刷新，读取 Codex 官方数据。"),
                                            t,
                                            state,
                                            intents,
                                        );
                                        ui.add_space(10.);
                                    }
                                    for r in &state.readings {
                                        self.provider(ui, r, t, state, intents);
                                        ui.add_space(10.);
                                    }
                                    ui.horizontal_wrapped(|ui| {
                                        let refresh = ui.add_enabled(
                                            !state.refreshing_usage,
                                            theme::primary_button(
                                                if state.refreshing_usage {
                                                    "刷新中…"
                                                } else {
                                                    "刷新全部"
                                                },
                                                t,
                                            ),
                                        );
                                        self.target("device.usage.refresh", refresh.rect);
                                        if refresh.clicked() {
                                            intents.push(Intent::Sync);
                                        }
                                        if secondary(ui, "刷新设置", t).clicked() {
                                            self.open_sheet(Sheet::Refresh);
                                        }
                                    });
                                }
                                Tab::Connection => self.connection_settings(ui, state, t, intents),
                            },
                            Sheet::Add => self.connection_settings(ui, state, t, intents),
                            Sheet::Help => self.help_content(ui, t),
                            Sheet::Environment => environment_view::diagnostics(ui, state, t),
                            Sheet::Cloud => self.cloud_settings(ui, state, t, intents),
                            Sheet::Codex => self.cli_settings(ui, state, t, intents),
                            Sheet::Refresh => self.refresh_settings(ui, state, t, intents),
                            Sheet::Weather => self.weather_settings(ui, state, t, intents),
                            Sheet::Todo => self.todo_editor(ui, t, intents),
                            Sheet::Update => self.update_settings(ui, t, intents),
                            _ => self.general_settings(ui, state, t, intents),
                        }
                        if self.notice.contains("失败") || self.notice.contains("名称需") {
                            ui.add_space(12.);
                            ui.label(RichText::new(&self.notice).color(
                                if self.notice.contains("失败") || self.notice.contains("名称需")
                                {
                                    t.error
                                } else {
                                    t.secondary
                                },
                            ));
                        }
                        if sheet == Sheet::Device && self.tab == Tab::Connection {
                            ui.add_space(18.);
                            egui::CollapsingHeader::new("最近连接状态").show(ui, |ui| {
                                for message in state.logs.iter().rev().take(12) {
                                    ui.label(RichText::new(message).size(12.).color(t.sheet_muted));
                                }
                            });
                        }
                    });
                // The frosted backing replaces legible UI behind the sheet; translucency
                // should reveal soft wallpaper rather than duplicate labels through fields.
                let frame = ui.min_rect().expand(26.);
                let viewport = ui.ctx().content_rect();
                let uv = Rect::from_min_max(
                    egui::pos2(
                        (frame.left() - viewport.left()) / viewport.width(),
                        (frame.top() - viewport.top()) / viewport.height(),
                    ),
                    egui::pos2(
                        (frame.right() - viewport.left()) / viewport.width(),
                        (frame.bottom() - viewport.top()) / viewport.height(),
                    ),
                );
                ui.painter().set(
                    material,
                    egui::Shape::Vec(vec![
                        egui::epaint::RectShape::filled(frame, theme::MODAL_RADIUS, Color32::WHITE)
                            .with_texture(self.frosted_wallpaper.id(), uv)
                            .into(),
                        egui::Shape::rect_filled(frame, theme::MODAL_RADIUS, t.sheet),
                        egui::Shape::rect_stroke(
                            frame,
                            theme::MODAL_RADIUS,
                            Stroke::new(1., t.line),
                            egui::StrokeKind::Inside,
                        ),
                    ]),
                );
            });
        if modal.should_close() {
            self.close_sheet();
        }
    }
    fn help_content(&self, ui: &mut egui::Ui, t: Tokens) {
        for (title, body) in [
            (
                "添加与切换设备",
                "首次添加：3Inch7EPaper在连接页双击BOOT，E1002长按左键2秒。选择对应EPair_设备，输入屏幕8位码，再点安全连接。每台设备独立绑定，可同时同步。切换Mac时先暂停原电脑连接。",
            ),
            (
                "用量统计",
                "今日／本周／本月分别按 UTC+8 当天、周一和每月1日起算，只合计已取得的有效统计。Codex 周／月来自官方已返回的对应日桶，缺失日期不补0。Argus 为当前文献库数据；DeepSeek Harness 按同步日志的调用标识去重，旧版总数等待明细后再纳入。缓存已含在输入中，历史累计和 Claude Code 当前上下文不混入。Codex昨日周额度为同账号昨天最后记录的已用百分比，不能当作昨天新增消耗，缺历史保留未知。",
            ),
            (
                "刷新与后台",
                "关闭窗口后继续在菜单栏运行，退出 App 或 Mac 休眠时暂停同步。用量间隔可在设置中调整。DeepSeek 余额至少缓存 5 分钟，限流后 15 分钟；Claude Code 读取官方状态栏的最新记录。",
            ),
            (
                "温湿度与墨水屏",
                "传感器每 30 秒采样，每分钟写入 SD，App 每 5 分钟读取；手动刷新立即采样。曲线为 15 分钟均值，缺测处留空。墨水屏普通更新使用局刷，切页与旋转使用全刷。",
            ),
            (
                "数据与隐私",
                "只使用官方入口：Codex App Server、Claude Code statusLine、Argus MCP 和 DeepSeek 余额 API。账户凭据留在 Mac，蓝牙通过加密连接传输经过校验的统计与设备状态。",
            ),
        ] {
            egui::CollapsingHeader::new(title).show(ui, |ui| {
                ui.label(RichText::new(body).color(t.sheet_muted));
            });
            ui.add_space(8.);
        }
        egui::CollapsingHeader::new("官方文档").show(ui, |ui| {
            ui.hyperlink_to("Codex", "https://learn.chatgpt.com/docs/app-server");
            ui.hyperlink_to("Claude Code", "https://code.claude.com/docs/en/statusline");
        });
    }
    /// Test channel routes use the same presentation states as the visible controls.
    pub fn activate(&mut self, id: &str) -> bool {
        if let Some(nav) = id.strip_prefix("nav.").and_then(Nav::parse) {
            self.nav = nav;
            return true;
        }
        match id {
            "usage.harness" => {
                self.nav = Nav::Usage;
                self.usage_filter = Some(crate::harness::PROVIDER);
            }
            "device.epaper" => {
                self.open_sheet(Sheet::Device);
                self.tab = Tab::Overview;
            }
            "add" => self.open_sheet(Sheet::Add),
            "settings" | "more" => self.open_sheet(Sheet::Settings),
            "help" => self.open_sheet(Sheet::Help),
            "codex.settings" => self.open_sheet(Sheet::Codex),
            "sheet.close" => self.close_sheet(),
            "sheet.back" => self.back_sheet(),
            "refresh.settings" => self.open_sheet(Sheet::Refresh),
            "home.tokens" => {
                self.nav = Nav::Usage;
                self.usage_filter = None;
            }
            "home.environment" => self.nav = Nav::Climate,
            "environment.details" => self.open_sheet(Sheet::Environment),
            "cloud.settings" => self.open_sheet(Sheet::Cloud),
            "update.settings" => self.open_sheet(Sheet::Update),
            "tab.usage" => self.tab = Tab::Usage,
            "tab.connection" => self.tab = Tab::Connection,
            "tab.overview" => self.tab = Tab::Overview,
            _ => return false,
        }
        true
    }
}
fn update_stamp(at: i64, success: bool) -> (String, String) {
    let Some(date) = chrono::DateTime::from_timestamp(at, 0) else {
        return (String::new(), String::new());
    };
    let local = date.with_timezone(&chrono::Local);
    let short = local.format(if local.date_naive() == chrono::Local::now().date_naive() {
        "%H:%M"
    } else {
        "%m/%d %H:%M"
    });
    (
        format!("{} {short}", if success { "更新于" } else { "尝试于" }),
        format!(
            "{} {}",
            if success {
                "读取完成"
            } else {
                "上次尝试，非成功更新"
            },
            local.format("%Y-%m-%d %H:%M:%S")
        ),
    )
}

fn primary(ui: &mut egui::Ui, label: &str, t: Tokens) -> egui::Response {
    ui.add(theme::primary_button(label, t))
}
fn connection_ready(bound: bool, enrollment: bool, code: &str) -> bool {
    if code.is_empty() {
        bound && !enrollment
    } else {
        code.len() == 8 && code.bytes().all(|c| c.is_ascii_digit())
    }
}
fn secondary(ui: &mut egui::Ui, label: &str, t: Tokens) -> egui::Response {
    ui.add(theme::secondary_button(label, t))
}
fn metric(ui: &mut egui::Ui, label: &str, value: &str, t: Tokens) {
    theme::surface(t).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(label).size(12.).color(t.sheet_muted));
        ui.label(RichText::new(value).font(bold(24.)).color(t.ink));
    });
}
fn number(n: Option<u64>) -> String {
    n.map(|n| {
        let text = n.to_string();
        let mut result = String::new();
        for (i, ch) in text.chars().enumerate() {
            if i > 0 && (text.len() - i) % 3 == 0 {
                result.push(',');
            }
            result.push(ch);
        }
        result
    })
    .unwrap_or_else(|| "未提供".into())
}

#[cfg(test)]
mod connection_ui_tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        home: &mut Home,
        state: &State,
        events: Vec<egui::Event>,
    ) -> Vec<Intent> {
        let mut intents = vec![];
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(850., 620.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    intents = home.render(ui, state);
                });
            },
        );
        output.textures_delta.clear();
        intents
    }
    fn center(r: [f32; 4]) -> egui::Pos2 {
        egui::pos2(r[0] + r[2] / 2., r[1] + r[3] / 2.)
    }
    fn click(
        ctx: &egui::Context,
        home: &mut Home,
        state: &State,
        point: egui::Pos2,
    ) -> Vec<Intent> {
        frame(
            ctx,
            home,
            state,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
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
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        )
    }
    #[test]
    fn pin_is_optional_only_for_a_remembered_device_in_data_mode() {
        assert!(connection_ready(true, false, ""));
        assert!(!connection_ready(false, false, ""));
        assert!(!connection_ready(true, true, ""));
        assert!(!connection_ready(true, false, "1234567"));
        assert!(!connection_ready(false, true, "1234567x"));
        assert!(connection_ready(false, true, "12345678"));
    }
    #[test]
    fn sensor_update_button_requests_sampling_instead_of_full_refresh() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut home = Home::new(&ctx, Preferences::default());
        home.nav = Nav::Climate;
        let state = State {
            connected: true,
            device_supported: true,
            ..Default::default()
        };
        for _ in 0..3 {
            frame(&ctx, &mut home, &state, vec![]);
        }
        let point = center(home.targets["device.read"]);
        let intents = click(&ctx, &mut home, &state, point);
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, Intent::Device(crate::device::Action::Read)))
        );
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, Intent::Device(crate::device::Action::Refresh)))
        );
    }
    #[test]
    fn climate_details_open_and_close_without_requesting_hardware() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut home = Home::new(&ctx, Preferences::default());
        home.nav = Nav::Climate;
        let state = State::default();
        for _ in 0..3 {
            frame(&ctx, &mut home, &state, vec![]);
        }
        let point = center(home.targets["environment.details"]);
        assert!(click(&ctx, &mut home, &state, point).is_empty());
        assert_eq!(home.sheet, Sheet::Environment);
        for _ in 0..3 {
            frame(&ctx, &mut home, &state, vec![]);
        }
        let point = center(home.targets["sheet.close"]);
        assert!(click(&ctx, &mut home, &state, point).is_empty());
        assert_eq!(home.sheet, Sheet::None);
    }
    #[test]
    fn remembered_device_without_advertisement_does_not_request_new_pairing() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut home = Home::new(&ctx, Preferences::default());
        home.sheet = Sheet::Add;
        let state = State {
            bound_name: "EPaper_ABCDEF".into(),
            ..Default::default()
        };
        for _ in 0..3 {
            assert!(
                !frame(&ctx, &mut home, &state, vec![])
                    .iter()
                    .any(|i| matches!(
                        i,
                        Intent::PairBluetooth { .. } | Intent::Connect(_) | Intent::Scan
                    ))
            );
        }
        assert!(home.targets.contains_key("connection.scan"));
        assert!(
            !home.targets.contains_key("connection.code")
                && !home.targets.contains_key("connection.connect")
        );
    }
    #[test]
    fn cloud_controls_require_explicit_user_action_and_emit_no_device_command() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut home = Home::new(&ctx, Preferences::default());
        home.sheet = Sheet::Cloud;
        let state = State::default();
        for _ in 0..3 {
            assert!(frame(&ctx, &mut home, &state, vec![]).is_empty());
        }
        let point = center(home.targets["cloud.choose"]);
        let intents = click(&ctx, &mut home, &state, point);
        assert!(intents.iter().any(|i| matches!(i, Intent::PickCloud)));
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, Intent::Device(_) | Intent::Connect(_) | Intent::Scan))
        );
    }
    #[test]
    fn typing_a_complete_pin_requires_an_explicit_click_below_the_field() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut home = Home::new(&ctx, Preferences::default());
        home.sheet = Sheet::Add;
        home.selected = "preview".into();
        let state = State {
            devices: vec![crate::backend::Device {
                id: "preview".into(),
                name: "EPair_ABCDEF".into(),
                rssi: None,
                bound: false,
                enrollment: true,
                recovered: false,
            }],
            ..Default::default()
        };
        for _ in 0..3 {
            frame(&ctx, &mut home, &state, vec![]);
        }
        let field = center(home.targets["connection.code"]);
        click(&ctx, &mut home, &state, field);
        let intents = frame(
            &ctx,
            &mut home,
            &state,
            vec![egui::Event::Text("12345678".into())],
        );
        assert_eq!(home.pairing_code.len(), 8);
        assert!(intents.iter().any(|i| matches!(
            i,
            Intent::ConnectionForm {
                active: true,
                disconnect: true
            }
        )));
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, Intent::Connect(_) | Intent::PairBluetooth { .. }))
        );
        let code = home.targets["connection.code"];
        let button = home.targets["connection.connect"];
        assert!(button[1] >= code[1] + code[3]);
        let intents = click(&ctx, &mut home, &state, center(button));
        assert_eq!(
            intents
                .iter()
                .filter(|i| matches!(i, Intent::PairBluetooth { .. }))
                .count(),
            1
        );
        assert!(home.pairing_code.is_empty());
    }
    #[test]
    fn main_status_close_stays_fully_inside_the_window() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut home = Home::new(&ctx, Preferences::default());
        let state = State {
            feedback: Some(crate::feedback::Feedback::new(
                "很长的错误".repeat(100),
                false,
                true,
            )),
            ..Default::default()
        };
        for _ in 0..3 {
            frame(&ctx, &mut home, &state, vec![]);
        }
        let close = home.targets["feedback.close"];
        assert!(close[0] >= 0. && close[0] + close[2] <= 850.);
        assert!(close[1] >= 0. && close[1] + close[3] <= 620.);
        assert!(close[3] <= 22.);
        let intents = click(&ctx, &mut home, &state, center(close));
        assert!(intents.iter().any(|i| matches!(i, Intent::DismissFeedback)));
    }
    #[test]
    fn long_feedback_is_one_row_and_can_be_dismissed_without_hiding_connection_controls() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut home = Home::new(&ctx, Preferences::default());
        home.sheet = Sheet::Add;
        let mut state = State {
            feedback: Some(crate::feedback::Feedback::new(
                "连接未完成：".repeat(100),
                false,
                true,
            )),
            ..Default::default()
        };
        for _ in 0..3 {
            frame(&ctx, &mut home, &state, vec![]);
        }
        let close = home.targets["feedback.close"];
        let connect = home.targets["connection.connect"];
        assert!(close[1] + close[3] < connect[1]);
        let intents = click(&ctx, &mut home, &state, center(close));
        assert!(intents.iter().any(|i| matches!(i, Intent::DismissFeedback)));
        state.feedback = Some(crate::feedback::Feedback::new("短提示", false, false));
        for _ in 0..2 {
            frame(&ctx, &mut home, &state, vec![]);
        }
        assert!((home.targets["connection.connect"][1] - connect[1]).abs() < 1.);
        state.feedback = None;
        frame(&ctx, &mut home, &state, vec![]);
        assert!(!home.targets.contains_key("feedback.close"));
    }
}
