//! Location cards and a linked, interactive comparison of the hub's real environment records.
use super::{
    Home, Intent,
    places_chart::{self, Series, Trend},
    theme::{self, Tokens, bold, font},
};
use crate::backend::State;
use eframe::egui::{self, Align2, Color32, RichText, Sense, Stroke};
use std::collections::BTreeMap;

const SERIES: [Color32; 8] = [
    Color32::from_rgb(255, 151, 41),
    Color32::from_rgb(51, 207, 239),
    Color32::from_rgb(111, 216, 163),
    Color32::from_rgb(187, 151, 255),
    Color32::from_rgb(255, 133, 161),
    Color32::from_rgb(240, 211, 111),
    Color32::from_rgb(125, 175, 255),
    Color32::from_rgb(208, 197, 173),
];
#[derive(Default)]
pub(super) struct View {
    pub trend: Trend,
    pub editing: bool,
    draft: BTreeMap<String, String>,
}
impl Home {
    pub fn environment_inspect(&self) -> serde_json::Value {
        serde_json::json!({"hours":self.places_view.trend.hours,"location":self.places_view.trend.active,
            "cursor":self.places_view.trend.cursor,"pinned":self.places_view.trend.pinned,"editing":self.places_view.editing})
    }
    pub(super) fn location_name(&self, state: &State, id: &str) -> String {
        super::model::place_name(&self.prefs.locations, &self.prefs.room, state, id)
    }
    pub(super) fn places(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        let ids = super::model::place_ids(state);
        if self
            .places_view
            .trend
            .active
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            self.places_view.trend.select(None);
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("室内环境").font(bold(20.)).color(t.text));
            ui.label(
                RichText::new(format!("{} 个位置", ids.len()))
                    .font(font(12.))
                    .color(t.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let edit = ui.add(theme::compact_button(
                    if self.places_view.editing {
                        "取消编辑"
                    } else {
                        "编辑位置"
                    },
                    t,
                    false,
                ));
                self.target("places.edit", edit.rect);
                if edit.clicked() {
                    self.places_view.editing = !self.places_view.editing;
                    self.places_view.draft = self.prefs.locations.clone();
                }
                let details = ui.add(theme::compact_button("设备详情", t, false));
                self.target("environment.details", details.rect);
                if details.clicked() {
                    self.open_sheet(super::model::Sheet::Environment);
                }
                let target = self.location_name(state, &state.active_device);
                let read = super::environment_view::icon_button(
                    ui,
                    super::icons::Icon::Refresh,
                    &format!("刷新{target}的温湿度"),
                    state.connected && state.device_supported && !state.controlling_device,
                    t,
                    state.controlling_device,
                );
                self.target("device.read", read.rect);
                if read.clicked() {
                    intents.push(Intent::Device(crate::device::Action::Read));
                }
            });
        });
        ui.label(
            RichText::new(if self.places_view.editing {
                "给屏幕所在的位置起个名字，保存后也会同步到支持的墨水屏。"
            } else {
                "点击位置卡片查看单独趋势，或对比全部位置。"
            })
            .font(font(12.))
            .color(t.secondary),
        );
        ui.add_space(12.);
        if ids.is_empty() {
            theme::surface(t).show(ui, |ui| {
                ui.label(
                    RichText::new("还没有环境读数")
                        .font(bold(16.))
                        .color(t.text),
                );
                ui.label(
                    RichText::new("添加一块带温湿度传感器的墨水屏后，就能在这里查看读数和趋势。")
                        .color(t.secondary),
                );
                let add = ui.add(theme::secondary_button("添加墨水屏", t));
                self.target("places.add", add.rect);
                if add.clicked() {
                    self.open_sheet(super::model::Sheet::Add);
                }
            });
            return;
        }
        let now = crate::usage::now();
        let columns = if ui.available_width() >= 900. {
            ids.len().min(3)
        } else if ui.available_width() >= 480. {
            ids.len().min(2)
        } else {
            1
        };
        let width = (ui.available_width() - 12. * (columns - 1) as f32) / columns as f32;
        for chunk in ids.chunks(columns) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.;
                for id in chunk {
                    let index = ids.iter().position(|v| v == id).unwrap();
                    let color = SERIES[index % SERIES.len()];
                    let selected = self
                        .places_view
                        .trend
                        .active
                        .as_ref()
                        .is_none_or(|v| v == id);
                    let editing = self.places_view.editing;
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(width, 112.),
                        if editing {
                            Sense::hover()
                        } else {
                            Sense::click()
                        },
                    );
                    let painter = ui.painter_at(rect);
                    theme::paint_surface(&painter, rect, t, response.hovered() && !editing);
                    if self.places_view.trend.active.as_ref() == Some(id) {
                        painter.rect_stroke(
                            rect,
                            theme::SURFACE_RADIUS,
                            Stroke::new(1.5, color),
                            egui::StrokeKind::Inside,
                        );
                    }
                    painter.circle_filled(
                        rect.min + egui::vec2(17., 23.),
                        4.,
                        if selected { color } else { t.muted },
                    );
                    if editing {
                        let mut name = self.places_view.draft.get(id).cloned().unwrap_or_default();
                        let field = ui.put(
                            egui::Rect::from_min_size(
                                rect.min + egui::vec2(29., 9.),
                                egui::vec2(width - 44., 28.),
                            ),
                            theme::compact_field(&mut name)
                                .hint_text(self.location_name(state, id))
                                .char_limit(16),
                        );
                        self.target(&format!("place.name.{id}"), field.rect);
                        if field.changed() {
                            if name.trim().is_empty() {
                                self.places_view.draft.remove(id);
                            } else {
                                self.places_view.draft.insert(id.clone(), name);
                            }
                        }
                    } else {
                        let name = self.location_name(state, id);
                        let galley =
                            theme::fitted_text(&painter, &name, bold(15.), t.text, width - 54.);
                        painter.galley(rect.min + egui::vec2(29., 14.), galley, t.text);
                        self.target(&format!("place.select.{id}"), rect);
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::SelectableLabel,
                                true,
                                &name,
                            )
                        });
                        if response.clicked() {
                            self.places_view.trend.select(Some(id.clone()));
                        }
                        super::icons::draw(
                            &painter,
                            egui::Rect::from_center_size(
                                rect.right_top() + egui::vec2(-18., 23.),
                                egui::vec2(12., 12.),
                            ),
                            super::icons::Icon::Chevron,
                            t.muted,
                        );
                    }
                    let row = state.fleet.get(id);
                    let climate = row.and_then(|r| r.climate).or_else(|| {
                        state
                            .env_log
                            .get(id)
                            .and_then(|p| p.last())
                            .map(|(at, t, h)| (*t, *h, *at))
                    });
                    let (temperature, humidity) = climate
                        .map(|(a, b, _)| (format!("{a:.1}°C"), format!("{b:.0}%")))
                        .unwrap_or_else(|| ("—".into(), "—".into()));
                    painter.text(
                        rect.min + egui::vec2(15., 45.),
                        Align2::LEFT_TOP,
                        temperature,
                        bold(29.),
                        if selected { t.text } else { t.secondary },
                    );
                    painter.text(
                        rect.min + egui::vec2(width * 0.59, 48.),
                        Align2::LEFT_TOP,
                        humidity,
                        bold(25.),
                        if selected { t.text } else { t.secondary },
                    );
                    let status = if row.is_some_and(|r| r.connected) {
                        "在线"
                    } else {
                        "离线"
                    };
                    let detail = climate
                        .map(|(_, _, at)| {
                            let time = chrono::DateTime::from_timestamp(at, 0)
                                .map(|d| {
                                    d.with_timezone(&chrono::Local).format("%H:%M").to_string()
                                })
                                .unwrap_or_default();
                            format!(
                                "{status} · {} {time}",
                                if now - at > 600 {
                                    "最后记录"
                                } else {
                                    "更新于"
                                }
                            )
                        })
                        .unwrap_or_else(|| format!("{status} · 等待首次读数"));
                    painter.text(
                        rect.min + egui::vec2(16., 89.),
                        Align2::LEFT_TOP,
                        detail,
                        font(11.),
                        t.muted,
                    );
                    response
                        .on_hover_cursor(if editing {
                            egui::CursorIcon::Default
                        } else {
                            egui::CursorIcon::PointingHand
                        })
                        .on_hover_text(
                            row.map(|r| format!("{} · {}", r.model, r.name))
                                .unwrap_or_else(|| id.clone()),
                        );
                }
            });
            ui.add_space(12.);
        }
        if self.places_view.editing {
            ui.horizontal(|ui| {
                let mut candidate = self.prefs.clone();
                candidate.locations = self.places_view.draft.clone();
                let save = ui.add_enabled(
                    candidate.validate(),
                    theme::compact_button("保存位置名称", t, true),
                );
                self.target("places.save", save.rect);
                if save.clicked() {
                    self.prefs.locations = self.places_view.draft.clone();
                    self.places_view.editing = false;
                    intents.push(Intent::SavePreferences);
                }
                ui.label(
                    RichText::new("例如：客厅、书房、卧室")
                        .font(font(11.))
                        .color(t.muted),
                );
            });
            ui.add_space(12.);
        }
        let series: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| Series {
                id: id.clone(),
                name: self.location_name(state, id),
                color: SERIES[i % SERIES.len()],
                points: state.env_log.get(id).cloned().unwrap_or_default(),
            })
            .collect();
        theme::surface(t).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("环境趋势").font(bold(17.)).color(t.text))
                    .on_hover_text("移动鼠标查看，点击或拖动固定时间。方向键逐条查看，Esc 返回实时。曲线断开表示该时段没有记录。");
                ui.add_space(8.);
                let all = ui.add(theme::compact_button(
                    "全部位置",
                    t,
                    self.places_view.trend.active.is_none(),
                ));
                self.target("places.all", all.rect);
                if all.clicked() {
                    self.places_view.trend.select(None);
                }
                for hours in [1, 6, 24, 48] {
                    let button = ui.add(theme::compact_button(
                        &format!("{hours}小时"),
                        t,
                        self.places_view.trend.hours == hours,
                    ));
                    self.target(&format!("places.range.{hours}"), button.rect);
                    if button.clicked() {
                        self.places_view.trend.hours = hours;
                        self.places_view.trend.clear_cursor();
                    }
                }
            });
            ui.add_space(6.);
            ui.horizontal_wrapped(|ui| {
                ui.set_min_height(28.);
                let label = self
                    .places_view
                    .trend
                    .cursor
                    .map(|at| {
                        format!(
                            "{}{}",
                            places_chart::stamp(at),
                            if self.places_view.trend.pinned {
                                " · 已固定"
                            } else {
                                " · 查看记录"
                            }
                        )
                    })
                    .unwrap_or_else(|| "移动鼠标查看 · 点击固定时间".into());
                ui.label(RichText::new(label).font(font(12.)).color(t.secondary));
                if self.places_view.trend.cursor.is_some() {
                    let live = ui.add(theme::compact_button("返回实时", t, false));
                    self.target("places.live", live.rect);
                    if live.clicked() {
                        self.places_view.trend.clear_cursor();
                    }
                }
            });
            ui.add_space(10.);
            let (temp, hover_temp) =
                places_chart::show(ui, &series, &mut self.places_view.trend, true, now, t);
            self.target("places.chart.temperature", temp);
            ui.add_space(12.);
            let (humid, hover_humid) =
                places_chart::show(ui, &series, &mut self.places_view.trend, false, now, t);
            self.target("places.chart.humidity", humid);
            if !hover_temp
                && !hover_humid
                && !self.places_view.trend.pinned
                && self.places_view.trend.cursor.take().is_some()
            {
                ui.ctx().request_repaint();
            }
            ui.add_space(10.);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("按已有记录对比 · 曲线断开表示没有记录")
                        .font(font(11.))
                        .color(t.muted),
                );
            });
        });
    }
    /// Today's poem on the Displays page, with "next" and an on/off switch.
    pub(super) fn poem_card(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        theme::surface(t).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("今日古诗").font(bold(16.)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut enabled = self.prefs.poem.enabled;
                    if ui.checkbox(&mut enabled, "显示在墨水屏").changed() {
                        self.prefs.poem.enabled = enabled;
                        intents.push(Intent::SavePreferences);
                    }
                    let next = ui.add_enabled(enabled, theme::secondary_button("换一首", t));
                    self.target("poem.next", next.rect);
                    if next.clicked() {
                        self.prefs.poem.offset = self.prefs.poem.offset.wrapping_add(1);
                        intents.push(Intent::SavePreferences);
                    }
                });
            });
            let poem =
                crate::poem::pick(crate::usage::now(), &self.prefs.poem).or(state.poem.as_ref());
            match poem.filter(|_| self.prefs.poem.enabled) {
                Some(p) => {
                    ui.label(
                        RichText::new(format!("{}  {}", p.title, p.byline()))
                            .font(font(14.))
                            .color(t.secondary),
                    );
                    for line in &p.lines {
                        ui.label(RichText::new(line).font(font(16.)).color(t.text));
                    }
                }
                None => {
                    ui.label(
                        RichText::new(if self.prefs.poem.enabled {
                            "诗库未就绪"
                        } else {
                            "已关闭"
                        })
                        .color(t.muted),
                    );
                }
            }
        });
    }
}
