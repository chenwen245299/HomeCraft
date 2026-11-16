//! Weather on the home page and its settings sheet: up to two cities from Open-Meteo, no IP lookup.
use super::{
    Home, Intent,
    model::Sheet,
    theme::{self, Tokens, bold, font},
};
use crate::backend::State;
use eframe::egui::{self, Align2, Rect, RichText, Sense};

pub fn summary(s: &crate::weather::Settings) -> String {
    if s.enabled {
        s.cities().join(" · ")
    } else {
        "已关闭".into()
    }
}
fn place(w: &crate::weather::Snapshot) -> String {
    if w.region.is_empty() {
        w.city.clone()
    } else {
        format!("{} · {}", w.city, w.region)
    }
}
impl Home {
    /// Home card: each configured city with its current reading; opens the weather settings.
    pub(super) fn weather_card(&mut self, ui: &mut egui::Ui, rect: Rect, state: &State, t: Tokens) {
        let response = ui.interact(rect, ui.id().with("home.weather"), Sense::click());
        theme::paint_surface(ui.painter(), rect, t, response.hovered());
        let painter = ui.painter();
        painter.text(
            rect.min + egui::vec2(18., 22.),
            Align2::LEFT_CENTER,
            "天气",
            bold(15.),
            t.text,
        );
        painter.text(
            rect.right_top() + egui::vec2(-18., 22.),
            Align2::RIGHT_CENTER,
            "设置地点",
            font(12.),
            t.secondary,
        );
        if !self.prefs.weather.enabled {
            painter.text(
                rect.min + egui::vec2(18., 56.),
                Align2::LEFT_CENTER,
                "天气已关闭",
                font(14.),
                t.muted,
            );
        }
        for (i, city) in self
            .prefs
            .weather
            .cities()
            .iter()
            .enumerate()
            .filter(|_| self.prefs.weather.enabled)
        {
            let y = rect.top() + 56. + i as f32 * 30.;
            let reading = state.weather[i].as_ref();
            painter.text(
                egui::pos2(rect.left() + 18., y),
                Align2::LEFT_CENTER,
                reading.map_or(city.as_str(), |w| w.city.as_str()),
                font(14.),
                t.text,
            );
            let (detail, value) = reading.map_or(("等待更新".to_owned(), String::new()), |w| {
                (
                    format!(
                        "{}  {:.0}°/{:.0}°",
                        w.description(),
                        f64::from(w.low_tenths) / 10.,
                        f64::from(w.high_tenths) / 10.
                    ),
                    format!("{:.0}°", f64::from(w.temperature_tenths) / 10.),
                )
            });
            painter.text(
                egui::pos2(rect.left() + 120., y),
                Align2::LEFT_CENTER,
                detail,
                font(13.),
                t.secondary,
            );
            painter.text(
                egui::pos2(rect.right() - 18., y),
                Align2::RIGHT_CENTER,
                value,
                bold(18.),
                t.text,
            );
        }
        self.target("home.weather", rect);
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.open_sheet(Sheet::Weather);
        }
    }
    /// The Environment page's weather beside the sensors: each city now, today's range and tomorrow.
    pub(super) fn weather_section(&mut self, ui: &mut egui::Ui, state: &State, t: Tokens) {
        theme::surface(t).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("天气").font(bold(16.)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let edit = ui.add(theme::compact_button("设置地点", t, false));
                    self.target("environment.weather", edit.rect);
                    if edit.clicked() {
                        self.open_sheet(Sheet::Weather);
                    }
                });
            });
            ui.add_space(6.);
            if !self.prefs.weather.enabled {
                ui.label(RichText::new("天气已关闭").font(font(13.)).color(t.muted));
                return;
            }
            let degrees = |tenths: i16| format!("{:.0}°", f64::from(tenths) / 10.);
            egui::Grid::new("environment-weather")
                .num_columns(4)
                .spacing([24., 8.])
                .show(ui, |ui| {
                    for (i, city) in self.prefs.weather.cities().iter().enumerate() {
                        let w = state.weather[i].as_ref();
                        ui.label(
                            RichText::new(w.map_or_else(|| city.clone(), place))
                                .font(bold(14.))
                                .color(t.text),
                        );
                        let Some(w) = w else {
                            ui.label(RichText::new("等待更新").font(font(13.)).color(t.muted));
                            ui.end_row();
                            continue;
                        };
                        ui.label(
                            RichText::new(degrees(w.temperature_tenths))
                                .font(bold(18.))
                                .color(t.text),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} {}/{}",
                                w.description(),
                                degrees(w.low_tenths),
                                degrees(w.high_tenths)
                            ))
                            .font(font(13.))
                            .color(t.secondary),
                        );
                        ui.label(
                            RichText::new(w.tomorrow.map_or_else(
                                || "明天 —".into(),
                                |(code, low, high)| {
                                    format!(
                                        "明天 {} {}/{}",
                                        crate::weather::describe(code),
                                        degrees(low),
                                        degrees(high)
                                    )
                                },
                            ))
                            .font(font(13.))
                            .color(t.secondary),
                        );
                        ui.end_row();
                    }
                });
            if !state.weather_notice.is_empty() {
                ui.label(
                    RichText::new(&state.weather_notice)
                        .size(12.)
                        .color(t.muted),
                );
            }
        });
    }
    pub(super) fn weather_settings(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        ui.checkbox(&mut self.prefs.weather.enabled, "同步天气到支持的屏幕");
        ui.add_space(8.);
        let enabled = self.prefs.weather.enabled;
        ui.add_enabled_ui(enabled, |ui| {
            for (label, hint, slot) in [
                ("城市一", crate::weather::DEFAULT_CITY, 0),
                ("城市二（可选）", "例如 Tokyo", 1),
            ] {
                ui.label(RichText::new(label).size(12.).color(t.sheet_muted));
                let field = if slot == 0 {
                    &mut self.prefs.weather.city
                } else {
                    &mut self.prefs.weather.second_city
                };
                let response = ui.add(
                    theme::text_field(field)
                        .hint_text(hint)
                        .desired_width(f32::INFINITY)
                        .char_limit(32),
                );
                self.target(&format!("weather.city.{slot}"), response.rect);
                ui.add_space(6.);
            }
        });
        ui.label(
            RichText::new("最多两个城市；墨水屏会同时显示。显示的地点不对时可改用英文名（如 Tokyo）。天气来自 Open-Meteo，不按 IP 定位，只把城市名和天气摘要发给屏幕。")
                .font(font(12.))
                .color(t.sheet_muted),
        );
        ui.add_space(8.);
        for (i, w) in state.weather.iter().enumerate() {
            if let Some(w) = w {
                let tomorrow = w.tomorrow.map_or(String::new(), |(code, low, high)| {
                    format!(
                        " · 明天 {} {:.0}°/{:.0}°",
                        crate::weather::describe(code),
                        f64::from(low) / 10.,
                        f64::from(high) / 10.
                    )
                });
                ui.label(format!(
                    "城市{} · {}  {:.1}°C  {} · {:.0}°/{:.0}°{tomorrow}",
                    if i == 0 { "一" } else { "二" },
                    place(w),
                    f64::from(w.temperature_tenths) / 10.,
                    w.description(),
                    f64::from(w.low_tenths) / 10.,
                    f64::from(w.high_tenths) / 10.
                ));
            }
        }
        if !state.weather_notice.is_empty() {
            ui.label(
                RichText::new(&state.weather_notice)
                    .font(font(12.))
                    .color(t.sheet_muted),
            );
        }
        ui.add_space(10.);
        let dirty = self.preferences_changed();
        theme::action_row(
            ui,
            dirty.then_some(("未保存", "⌘S 保存修改")),
            t,
            |ui| {
                let save = ui.add_enabled(
                    dirty && self.prefs.validate(),
                    theme::primary_button("保存", t),
                );
                self.target("weather.save", save.rect);
                if save.clicked() {
                    intents.push(Intent::SavePreferences);
                }
            },
        );
    }
}
