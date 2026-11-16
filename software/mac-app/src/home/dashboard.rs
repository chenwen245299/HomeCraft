//! Home summaries read presentation state only; opening them never contacts hardware.
use super::{
    Home,
    icons::{Icon, draw},
    model::{self, Nav, Sheet, Tab},
    theme::{self, Tokens, bold, font},
};
use crate::{backend::State, usage};
use eframe::egui::{self, Align2, Rect, Sense, Stroke};

impl Home {
    pub(super) fn dashboard(
        &mut self,
        ui: &mut egui::Ui,
        main: Rect,
        pos: egui::Pos2,
        state: &State,
        t: Tokens,
    ) {
        let width = main.width() - 48.;
        let gap = 16.;
        let token_width = (width - gap) * 0.62;
        let tokens = Rect::from_min_size(pos, egui::vec2(token_width, 214.));
        let climate = Rect::from_min_max(
            egui::pos2(tokens.right() + gap, pos.y),
            egui::pos2(main.right() - 24., tokens.bottom()),
        );
        self.token_summary(ui, tokens, state, t);
        self.climate_summary(ui, climate, state, t);

        // Weather and today's todos share one row when the window is tall enough to keep the
        // display row visible; smaller windows reach them from the chips and settings.
        let half = (width - gap) / 2.;
        let mut top = tokens.bottom() + gap;
        if main.bottom() - top >= 120. + gap + 64. + 44. {
            let weather = Rect::from_min_size(egui::pos2(pos.x, top), egui::vec2(half, 120.));
            let todos = Rect::from_min_size(
                egui::pos2(weather.right() + gap, top),
                egui::vec2(half, 120.),
            );
            self.weather_card(ui, weather, state, t);
            self.todo_card(ui, todos, state, t);
            top = weather.bottom() + gap;
        }
        let device = Rect::from_min_size(egui::pos2(pos.x, top), egui::vec2(width, 64.));
        let response = ui.interact(device, ui.id().with("home.device"), Sense::click());
        theme::paint_surface(ui.painter(), device, t, response.hovered());
        draw(
            ui.painter(),
            Rect::from_min_size(device.min + egui::vec2(18., 18.), egui::vec2(28., 28.)),
            Icon::Display,
            t.usage,
        );
        ui.painter().text(
            device.min + egui::vec2(60., 22.),
            Align2::LEFT_CENTER,
            state
                .fleet
                .get(&state.active_device)
                .map(|d| d.model.as_str())
                .unwrap_or("EPaper"),
            bold(15.),
            t.text,
        );
        ui.painter().text(
            device.min + egui::vec2(60., 43.),
            Align2::LEFT_CENTER,
            model::connection(state),
            font(12.),
            t.muted,
        );
        draw(
            ui.painter(),
            Rect::from_center_size(
                device.right_center() - egui::vec2(25., 0.),
                egui::vec2(16., 16.),
            ),
            Icon::Chevron,
            t.secondary,
        );
        self.target("device.epaper", device);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "EPaper · 查看设备")
        });
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.open_sheet(Sheet::Device);
            self.tab = Tab::Overview;
        }
        // Every saved display stays visible even while another display is selected for controls.
        for (index, row) in state
            .fleet
            .values()
            .filter(|row| row.id != state.active_device)
            .enumerate()
        {
            let rect = Rect::from_min_size(
                egui::pos2(pos.x, device.bottom() + 12. + index as f32 * 76.),
                egui::vec2(width, 64.),
            );
            let response = ui.interact(rect, ui.id().with(("home.fleet", &row.id)), Sense::click());
            theme::paint_surface(ui.painter(), rect, t, response.hovered());
            draw(
                ui.painter(),
                Rect::from_min_size(rect.min + egui::vec2(18., 18.), egui::vec2(28., 28.)),
                Icon::Display,
                if row.connected { t.safe } else { t.muted },
            );
            ui.painter().text(
                rect.min + egui::vec2(60., 22.),
                Align2::LEFT_CENTER,
                format!("{} · {}", row.model, row.name),
                bold(15.),
                t.text,
            );
            ui.painter().text(
                rect.min + egui::vec2(60., 43.),
                Align2::LEFT_CENTER,
                &row.notice,
                font(12.),
                t.muted,
            );
            self.target(&format!("device.fleet.{}", row.id), rect);
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.selected = row.id.clone();
                self.open_sheet(Sheet::Device);
                self.tab = Tab::Connection;
            }
        }
    }

    fn token_summary(&mut self, ui: &mut egui::Ui, rect: Rect, state: &State, t: Tokens) {
        let response = ui.interact(rect, ui.id().with("home.tokens"), Sense::click());
        theme::paint_surface(ui.painter(), rect, t, response.hovered());
        draw(
            ui.painter(),
            Rect::from_min_size(rect.min + egui::vec2(18., 19.), egui::vec2(21., 21.)),
            Icon::Usage,
            t.safe,
        );
        ui.painter().text(
            rect.min + egui::vec2(48., 30.),
            Align2::LEFT_CENTER,
            "AI 用量",
            bold(16.),
            t.text,
        );
        ui.painter().text(
            egui::pos2(rect.right() - 18., rect.top() + 30.),
            Align2::RIGHT_CENTER,
            "已知合计",
            font(12.),
            t.muted,
        );
        let third = rect.width() / 3.;
        for i in 1..3 {
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left() + third * i as f32, rect.top() + 72.),
                    egui::pos2(rect.left() + third * i as f32, rect.bottom() - 55.),
                ],
                Stroke::new(1., t.line),
            );
        }
        let day = model::day();
        for (index, id, label, value, color) in [
            (0, "today", "今日", model::today_total(state, day), t.accent),
            (1, "week", "本周", model::week_total(state, day), t.text),
            (2, "month", "本月", model::month_total(state, day), t.text),
        ] {
            let x = rect.left() + index as f32 * third + 12.;
            let available = third - 24.;
            ui.painter().text(
                egui::pos2(x, rect.top() + 83.),
                Align2::LEFT_CENTER,
                label,
                font(14.),
                t.secondary,
            );
            let text = model::compact(value);
            let mut size = if index == 0 { 40. } else { 32. };
            while size > 18.
                && ui
                    .painter()
                    .layout_no_wrap(text.clone(), bold(size), color)
                    .size()
                    .x
                    > available
            {
                size -= 2.;
            }
            let galley = ui.painter().layout_no_wrap(text, bold(size), color);
            let value_rect = Rect::from_min_size(egui::pos2(x, rect.top() + 108.), galley.size());
            ui.painter().galley(value_rect.min, galley, color);
            self.target(&format!("home.tokens.{id}.value"), value_rect);
        }
        ui.painter().text(
            rect.min + egui::vec2(18., rect.height() - 23.),
            Align2::LEFT_CENTER,
            "查看各项用量",
            font(12.),
            t.muted,
        );
        draw(
            ui.painter(),
            Rect::from_center_size(
                egui::pos2(rect.right() - 23., rect.bottom() - 23.),
                egui::vec2(14., 14.),
            ),
            Icon::Chevron,
            t.secondary,
        );
        self.target("home.tokens", rect);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                "今日、本周、本月 · 查看 AI 用量",
            )
        });
        if response.on_hover_text("UTC+8 · 本周从周一、本月从1日开始。只合计已取得的有效统计；Codex 日统计可能延迟，缺失日期不按 0 计算。同一 Codex 账号不按 Mac 重复相加。")
            .on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.nav = Nav::Usage;
            self.usage_filter = None;
        }
    }

    fn climate_summary(&mut self, ui: &mut egui::Ui, rect: Rect, state: &State, t: Tokens) {
        let response = ui.interact(rect, ui.id().with("home.environment"), Sense::click());
        theme::paint_surface(ui.painter(), rect, t, response.hovered());
        draw(
            ui.painter(),
            Rect::from_min_size(rect.min + egui::vec2(18., 19.), egui::vec2(21., 21.)),
            Icon::Climate,
            t.climate,
        );
        ui.painter().text(
            rect.min + egui::vec2(48., 30.),
            Align2::LEFT_CENTER,
            "室内环境",
            bold(16.),
            t.text,
        );
        draw(
            ui.painter(),
            Rect::from_center_size(
                egui::pos2(rect.right() - 23., rect.top() + 30.),
                egui::vec2(14., 14.),
            ),
            Icon::Chevron,
            t.secondary,
        );
        let device = state.telemetry.as_ref();
        let color = if device.is_some_and(|d| d.fresh(state.connected, usage::now())) {
            t.text
        } else {
            t.muted
        };
        for (id, label, value, unit, icon, accent, y) in [
            (
                "temperature",
                "温度",
                device.and_then(|d| d.temperature),
                "°C",
                Icon::Thermometer,
                t.accent,
                82.,
            ),
            (
                "humidity",
                "湿度",
                device.and_then(|d| d.humidity),
                "%",
                Icon::Humidity,
                t.climate,
                140.,
            ),
        ] {
            draw(
                ui.painter(),
                Rect::from_center_size(
                    egui::pos2(rect.left() + 27., rect.top() + y),
                    egui::vec2(18., 18.),
                ),
                icon,
                accent,
            );
            ui.painter().text(
                egui::pos2(rect.left() + 43., rect.top() + y),
                Align2::LEFT_CENTER,
                label,
                font(13.),
                t.secondary,
            );
            let text = value
                .map(|n| format!("{n:.1}{unit}"))
                .unwrap_or_else(|| "—".into());
            let mut size = if rect.width() < 260. { 28. } else { 34. };
            while size > 20.
                && ui
                    .painter()
                    .layout_no_wrap(text.clone(), bold(size), color)
                    .size()
                    .x
                    > rect.width() - 104.
            {
                size -= 1.;
            }
            let galley = ui.painter().layout_no_wrap(text, bold(size), color);
            let value_rect = Rect::from_min_size(
                egui::pos2(
                    rect.right() - 18. - galley.size().x,
                    rect.top() + y - galley.size().y / 2.,
                ),
                galley.size(),
            );
            ui.painter().galley(value_rect.min, galley, color);
            self.target(&format!("home.environment.{id}"), value_rect);
        }
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + 18., rect.top() + 111.),
                egui::pos2(rect.right() - 18., rect.top() + 111.),
            ],
            Stroke::new(1., t.line),
        );
        let stamp = super::environment_view::stamp(state, usage::now());
        let galley =
            theme::fitted_text(ui.painter(), &stamp, font(12.), t.muted, rect.width() - 36.);
        let stamp_rect = Rect::from_min_size(
            rect.min + egui::vec2(18., rect.height() - 31.),
            galley.size(),
        );
        ui.painter().galley(stamp_rect.min, galley, t.muted);
        self.target("home.environment.stamp", stamp_rect);
        self.target("home.environment", rect);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "温度与湿度 · 查看环境趋势")
        });
        if response
            .on_hover_text(stamp)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.nav = Nav::Climate;
        }
    }
}
