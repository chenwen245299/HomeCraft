use super::{
    icons::{self, Icon},
    model,
    theme::{self, Tokens, bold},
};
use crate::{harness::DeviceUsage, usage::Reading};
use eframe::egui::{self, Align, Layout, RichText, Stroke};

pub fn show(ui: &mut egui::Ui, r: &Reading, t: Tokens, scope: &mut Option<String>) {
    if scope
        .as_ref()
        .is_some_and(|id| !r.tokens.devices.iter().any(|d| &d.id == id))
    {
        *scope = None;
    }
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(scope.is_none(), "全部设备").clicked() {
            *scope = None;
        }
        for device in &r.tokens.devices {
            if ui
                .selectable_label(scope.as_deref() == Some(&device.id), &device.name)
                .clicked()
            {
                *scope = Some(device.id.clone());
            }
        }
    });
    ui.add_space(14.);
    let selected = scope
        .as_ref()
        .and_then(|id| r.tokens.devices.iter().find(|d| &d.id == id));
    let day = model::day();
    let week = model::week_start(day);
    let daily = selected.map_or_else(
        || r.packet.d.filter(|d| d[0] == day).map(|d| d[1] as u64),
        |d| (d.d[0] == day).then_some(d.d[1] as u64),
    );
    let weekly = selected.map_or(
        r.packet
            .wt
            .filter(|w| Some(w[0]) == week)
            .map(|w| w[1] as u64),
        |d| (Some(d.wt[0]) == week).then_some(d.wt[1] as u64),
    );
    let month = model::month_start(day);
    let monthly = selected.map_or_else(
        || {
            r.packet
                .mt
                .filter(|m| Some(m[0]) == month)
                .map(|m| m[1] as u64)
        },
        |d| d.mt.filter(|m| Some(m[0]) == month).map(|m| m[1] as u64),
    );
    ui.columns(3, |cols| {
        for (i, label, value) in [
            (0, "今日", daily),
            (1, "本周", weekly),
            (2, "本月", monthly),
        ] {
            cols[i].label(RichText::new(label).size(13.).color(t.secondary));
            cols[i].label(
                RichText::new(model::compact(value))
                    .font(bold(if i == 0 { 36. } else { 28. }))
                    .color(if i == 0 { t.text } else { t.secondary }),
            );
        }
    });
    ui.add_space(12.);
    let counters = selected
        .map(|d| d.u)
        .or_else(|| r.packet.u.map(|u| [u[0], u[1], u[2], u[3]]));
    if let Some(u) = counters {
        ui.columns(3, |cols| {
            for (i, label, text) in [
                (
                    0,
                    "输入 / 输出",
                    format!(
                        "{} / {}",
                        model::compact(Some(u[0] as u64)),
                        model::compact(Some(u[1] as u64))
                    ),
                ),
                (
                    1,
                    "缓存命中",
                    if u[0] > 0 {
                        format!("{:.1}%", u[2] as f64 / u[0] as f64 * 100.)
                    } else {
                        "—".into()
                    },
                ),
                (2, "调用次数", u[3].to_string()),
            ] {
                cols[i].label(RichText::new(label).size(11.).color(t.muted));
                cols[i].label(RichText::new(text).font(bold(15.)).color(t.secondary));
            }
        });
    }
    ui.add_space(18.);
    ui.label(RichText::new("设备明细").font(bold(13.)).color(t.secondary));
    ui.add_space(7.);
    let mut devices = r.tokens.devices.iter().collect::<Vec<_>>();
    devices.sort_by(|a, b| b.d[1].cmp(&a.d[1]));
    let max = devices
        .iter()
        .filter(|d| d.d[0] == day)
        .map(|d| d.d[1])
        .max()
        .unwrap_or(1)
        .max(1);
    for d in devices {
        device_row(ui, d, day, week, max, t);
        ui.add_space(6.);
    }
    if r.tokens.devices.is_empty() {
        ui.label(RichText::new(&r.source).size(12.).color(t.secondary));
    }
}
fn device_row(
    ui: &mut egui::Ui,
    d: &DeviceUsage,
    day: i64,
    week: Option<i64>,
    max: i64,
    t: Tokens,
) {
    egui::Frame::new()
        .fill(t.field)
        .corner_radius(theme::CONTROL_RADIUS)
        .inner_margin(11.)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (icon, _) = ui.allocate_exact_size(egui::vec2(26., 26.), egui::Sense::hover());
                icons::draw(ui.painter(), icon, Icon::Display, t.climate);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&d.name).font(bold(14.)).color(t.text));
                    let at = chrono::DateTime::from_timestamp(d.t, 0)
                        .map(|v| {
                            v.with_timezone(&chrono::Local)
                                .format("%m/%d %H:%M")
                                .to_string()
                        })
                        .unwrap_or_default();
                    ui.label(RichText::new(format!("{at} 同步")).size(10.).color(t.muted));
                });
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 48.),
                    Layout::top_down(Align::Max),
                    |ui| {
                        ui.label(
                            RichText::new(model::compact((d.d[0] == day).then_some(d.d[1] as u64)))
                                .font(bold(22.))
                                .color(t.text),
                        );
                        ui.label(
                            RichText::new(format!(
                                "本周 {} · 本月 {}",
                                model::compact((Some(d.wt[0]) == week).then_some(d.wt[1] as u64)),
                                model::compact(
                                    d.mt.filter(|m| Some(m[0]) == model::month_start(day))
                                        .map(|m| m[1] as u64)
                                )
                            ))
                            .size(11.)
                            .color(t.secondary),
                        );
                    },
                );
            });
            let (line, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.), egui::Sense::hover());
            ui.painter().line_segment(
                [line.left_center(), line.right_center()],
                Stroke::new(2., t.line),
            );
            if d.d[0] == day {
                let right = line.left() + line.width() * d.d[1] as f32 / max as f32;
                ui.painter().line_segment(
                    [line.left_center(), egui::pos2(right, line.center().y)],
                    Stroke::new(3., t.accent),
                );
            }
        });
}
