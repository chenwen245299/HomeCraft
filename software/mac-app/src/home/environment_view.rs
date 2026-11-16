//! Climate presentation: one set of values, one timestamp, diagnostics on demand.
use super::{
    icons::{self, Icon},
    theme::{Tokens, bold, font},
};
use crate::{backend::State, usage};
use eframe::egui::{self, Align2, Rect, RichText, Sense, Stroke};

pub fn stamp(state: &State, now: i64) -> String {
    let Some(d) = &state.telemetry else {
        return if state.connected {
            "等待数据"
        } else {
            "尚未连接"
        }
        .into();
    };
    let time = chrono::DateTime::from_timestamp(d.sampled_at, 0)
        .filter(|_| d.sampled_at > 0)
        .map(|at| at.with_timezone(&chrono::Local).format("%H:%M").to_string());
    if let Some(source) = &state.environment_source {
        return format!("{source} · {}", time.unwrap_or_else(|| "—".into()));
    }
    let prefix = if !state.connected {
        "已断开"
    } else if d.temperature.is_none() {
        "传感器不可用"
    } else if !d.fresh(true, now) {
        "等待更新"
    } else {
        "采样"
    };
    time.map_or_else(|| prefix.into(), |time| format!("{prefix} {time}"))
}

pub fn readings(ui: &mut egui::Ui, state: &State, t: Tokens, compact: bool) {
    let height = 88.;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let mid = rect.center().x;
    ui.painter().line_segment(
        [
            egui::pos2(mid, rect.top() + 6.),
            egui::pos2(mid, rect.bottom() - 5.),
        ],
        Stroke::new(1., t.line),
    );
    let d = state.telemetry.as_ref();
    for (index, label, value, unit, icon, color) in [
        (
            0,
            "温度",
            d.and_then(|d| d.temperature),
            "°C",
            Icon::Thermometer,
            t.accent,
        ),
        (
            1,
            "湿度",
            d.and_then(|d| d.humidity),
            "%",
            Icon::Humidity,
            t.climate,
        ),
    ] {
        let x = if index == 0 { rect.left() } else { mid + 20. };
        icons::draw(
            ui.painter(),
            Rect::from_min_size(egui::pos2(x, rect.top() + 2.), egui::vec2(20., 20.)),
            icon,
            color,
        );
        ui.painter().text(
            egui::pos2(x + 28., rect.top() + 12.),
            Align2::LEFT_CENTER,
            label,
            font(13.),
            t.secondary,
        );
        let number = value
            .map(|n| format!("{n:.1}"))
            .unwrap_or_else(|| "—".into());
        let size = if compact { 38. } else { 48. };
        let galley = ui.painter().layout_no_wrap(number, bold(size), t.text);
        let number_width = galley.size().x;
        ui.painter()
            .galley(egui::pos2(x, rect.top() + 29.), galley, t.text);
        if value.is_some() {
            ui.painter().text(
                egui::pos2(
                    x + number_width + 7.,
                    rect.top() + if compact { 50. } else { 59. },
                ),
                Align2::LEFT_TOP,
                unit,
                font(21.),
                t.muted,
            );
        }
    }
}

fn local_time(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0)
        .filter(|_| at > 0)
        .map(|at| {
            at.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "未提供".into())
}
/// Every other room's latest reading, so a display elsewhere (another Mac via iCloud) stays
/// visible while the card above shows the nearest one. This Mac's own cache is not a room.
pub fn other_sources(ui: &mut egui::Ui, state: &State, t: Tokens) {
    // The card names a Mac, not the display: its reading's time tells which one it shows.
    let shown = state
        .environment_source
        .as_deref()
        .zip(state.telemetry.as_ref().map(|d| d.sampled_at));
    let rows: Vec<_> = crate::cloud::climate_sources(state)
        .into_iter()
        .filter(|s| !s.own && shown != Some((s.label.as_str(), s.sampled_at)))
        .collect();
    if rows.is_empty() {
        return;
    }
    ui.add_space(6.);
    ui.label(RichText::new("其他来源").font(bold(13.)).color(t.secondary));
    let today = chrono::Local::now().date_naive();
    for s in rows {
        let at = chrono::DateTime::from_timestamp(s.sampled_at, 0)
            .map(|at| at.with_timezone(&chrono::Local))
            .map_or_else(
                || "—".into(),
                |at| {
                    at.format(if at.date_naive() == today {
                        "%H:%M"
                    } else {
                        "%m-%d %H:%M"
                    })
                    .to_string()
                },
            );
        let value =
            |v: Option<f32>, unit: &str| v.map_or_else(|| "—".into(), |v| format!("{v:.1}{unit}"));
        let title = if s.live {
            s.label
        } else {
            format!("{} · {}", s.label, s.device)
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).font(font(13.)).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {at}",
                        value(s.temperature, "°C"),
                        value(s.humidity, "%")
                    ))
                    .font(font(13.))
                    .color(t.secondary),
                );
            });
        });
    }
}
pub fn diagnostics(ui: &mut egui::Ui, state: &State, t: Tokens) {
    if let Some(source) = &state.environment_source {
        ui.label(RichText::new(source).color(t.secondary));
        ui.label("这里只有温湿度，设备状态需连接主屏后读取");
        return;
    }
    let Some(d) = &state.telemetry else {
        ui.label(RichText::new("连接 EPaper 后可查看设备状态").color(t.secondary));
        return;
    };
    let power = if d.power.1 == 1 && d.power.3 >= 0 {
        format!(
            "{}%{}",
            d.power.3,
            if d.power.2 == 1 { " · 充电中" } else { "" }
        )
    } else if d.power.0 == 1 {
        "USB 供电".into()
    } else {
        "未提供".into()
    };
    let storage = if d.sd.0 == 1 {
        format!(
            "{} 条{}",
            d.sd.2,
            if d.sd.3 == 1 { "" } else { " · 等待写入" }
        )
    } else {
        "未挂载".into()
    };
    egui::Grid::new("environment-diagnostics")
        .spacing([28., 16.])
        .show(ui, |ui| {
            for (label, value) in [
                (
                    "连接",
                    if state.connected {
                        "蓝牙已连接".into()
                    } else {
                        "已断开".into()
                    },
                ),
                ("电源", power),
                ("SD 记录", storage),
                ("采样时间", local_time(d.sampled_at)),
                ("采样间隔", "30 秒".into()),
                ("自动回传", "5 分钟".into()),
            ] {
                ui.label(RichText::new(label).color(t.muted));
                ui.label(RichText::new(value).color(t.text));
                ui.end_row();
            }
        });
    if d.sd.5 != 0 {
        ui.add_space(16.);
        ui.label(RichText::new(format!("SD 记录异常 ({})", d.sd.5)).color(t.error));
    }
    if let Some(frame) = &d.shown
        && frame.6 > 0
    {
        ui.add_space(22.);
        egui::CollapsingHeader::new("屏幕显示")
            .show_background(false)
            .show(ui, |ui| {
                let shown = match (frame.0, frame.1) {
                    (Some(c), Some(h)) => format!("{c:.1} °C · {h:.1}%"),
                    _ => "未提供".into(),
                };
                ui.label(RichText::new(shown).color(t.text));
                ui.label(RichText::new(local_time(frame.2)).size(12.).color(t.muted));
            });
    }
}

pub fn status(ui: &mut egui::Ui, state: &State, t: Tokens) {
    let fresh = state
        .telemetry
        .as_ref()
        .is_some_and(|d| d.fresh(state.connected, usage::now()));
    let (dot, _) = ui.allocate_exact_size(egui::vec2(7., 7.), Sense::hover());
    ui.painter()
        .circle_filled(dot.center(), 2.5, if fresh { t.safe } else { t.muted });
    ui.label(
        RichText::new(stamp(state, usage::now()))
            .size(12.)
            .color(t.muted),
    )
    .on_hover_text("每 30 秒采样 · 每 5 分钟回传");
}

pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    t: Tokens,
    busy: bool,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(32., 32.), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, 8, t.field);
        }
        if busy {
            egui::Spinner::new()
                .color(t.accent)
                .paint_at(ui, rect.shrink(6.));
        } else {
            icons::draw(
                ui.painter(),
                rect.shrink(7.),
                icon,
                if enabled { t.secondary } else { t.muted },
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
        response.on_hover_text(label)
    })
    .inner
}
