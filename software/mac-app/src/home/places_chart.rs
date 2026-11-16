//! Multi-location history with linked hover, pinning and real-time ranges. Missing data stays empty.
use super::theme::{self, Tokens, bold, font};
use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke};

pub(super) struct Series {
    pub id: String,
    pub name: String,
    pub color: Color32,
    pub points: Vec<(i64, f32, f32)>,
}
pub(super) struct Trend {
    pub hours: i64,
    pub active: Option<String>,
    pub cursor: Option<i64>,
    pub pinned: bool,
    ignore_pointer: Option<egui::Pos2>,
}
impl Default for Trend {
    fn default() -> Self {
        Self {
            hours: 24,
            active: None,
            cursor: None,
            pinned: false,
            ignore_pointer: None,
        }
    }
}
impl Trend {
    pub fn clear_cursor(&mut self) {
        self.cursor = None;
        self.pinned = false;
        self.ignore_pointer = None;
    }
    pub fn select(&mut self, id: Option<String>) {
        self.active = id;
        self.clear_cursor();
    }
}
pub(super) fn stamp(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%m/%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "未知时间".into())
}
pub(super) fn sample(points: &[(i64, f32, f32)], at: i64) -> Option<(i64, f32, f32)> {
    points
        .iter()
        .copied()
        .filter(|(_, t, h)| t.is_finite() && h.is_finite())
        .min_by_key(|(time, _, _)| time.abs_diff(at))
        .filter(|(time, _, _)| time.abs_diff(at) <= 600)
}
fn visible(series: &Series, trend: &Trend) -> bool {
    trend.active.as_ref().is_none_or(|id| *id == series.id)
}
pub(super) fn show(
    ui: &mut egui::Ui,
    all: &[Series],
    trend: &mut Trend,
    temperature: bool,
    now: i64,
    t: Tokens,
) -> (Rect, bool) {
    let unit = if temperature { "°C" } else { "%" };
    let start = now - trend.hours * 3600;
    let row_height = ui.spacing().interact_size.y;
    ui.spacing_mut().interact_size.y = 18.;
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(if temperature { "温度" } else { "湿度" })
                .font(bold(15.))
                .color(t.text),
        );
        ui.label(egui::RichText::new(unit).font(font(12.)).color(t.muted));
        let selected = trend.cursor;
        ui.spacing_mut().item_spacing.x = 18.;
        for series in all.iter().filter(|s| visible(s, trend)) {
            let point = if let Some(at) = selected {
                sample(&series.points, at)
            } else {
                series
                    .points
                    .iter()
                    .rev()
                    .copied()
                    .find(|(at, _, _)| *at >= start && *at <= now)
            }
            .filter(|(at, _, _)| *at >= start && *at <= now);
            let value = point.map(|(_, a, b)| if temperature { a } else { b });
            let value = value
                .map(|v| format!("{v:.1}{unit}"))
                .unwrap_or_else(|| "—".into());
            let name_width = ui
                .painter()
                .layout_no_wrap(series.name.clone(), bold(12.), series.color)
                .size()
                .x;
            let width = (name_width + 86.).clamp(132., 220.);
            let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 18.), Sense::hover());
            let name = theme::fitted_text(
                ui.painter(),
                &format!("● {}", series.name),
                bold(12.),
                series.color,
                width - 66.,
            );
            ui.painter()
                .galley(rect.min + egui::vec2(0., 2.), name, series.color);
            ui.painter().text(
                rect.right_center(),
                Align2::RIGHT_CENTER,
                value,
                bold(12.),
                series.color,
            );
            response.on_hover_text(
                point
                    .map(|(at, _, _)| format!("{} · {} · 就近采样", series.name, stamp(at)))
                    .unwrap_or_else(|| format!("{} · 此时没有记录", series.name)),
            );
        }
    });
    ui.spacing_mut().interact_size.y = row_height;
    ui.add_space(6.);
    let height = if ui.available_width() < 650. {
        128.
    } else {
        148.
    };
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    let plot = Rect::from_min_max(
        rect.min + egui::vec2(42., 12.),
        rect.max - egui::vec2(14., 26.),
    );
    let before = (trend.cursor, trend.pinned);
    let hovered = response.hover_pos().is_some_and(|p| plot.contains(p));
    if let Some(pointer) = response
        .interact_pointer_pos()
        .or_else(|| response.hover_pos())
        .filter(|p| plot.contains(*p))
    {
        let at = start
            + ((pointer.x - plot.left()) / plot.width() * (now - start) as f32).round() as i64;
        let nearest = all
            .iter()
            .filter(|s| visible(s, trend))
            .flat_map(|s| &s.points)
            .filter(|(time, _, _)| *time >= start && *time <= now)
            .min_by_key(|(time, _, _)| time.abs_diff(at))
            .map(|(time, _, _)| *time)
            .filter(|time| time.abs_diff(at) <= 600)
            .unwrap_or(at);
        let ignored = trend.ignore_pointer == Some(pointer);
        if !ignored {
            trend.ignore_pointer = None;
        }
        if response.dragged() {
            trend.cursor = Some(nearest);
            trend.pinned = true;
        } else if response.clicked() {
            let unpin = trend.pinned && trend.cursor == Some(nearest);
            trend.cursor = Some(nearest);
            trend.pinned = !unpin;
            response.request_focus();
        } else if !trend.pinned && !ignored {
            trend.cursor = Some(nearest);
        }
    }
    if trend.cursor.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        trend.clear_cursor();
        trend.ignore_pointer = ui.input(|i| i.pointer.hover_pos());
    }
    if response.has_focus() {
        for (key, forward) in [(egui::Key::ArrowLeft, false), (egui::Key::ArrowRight, true)] {
            if ui.input(|i| i.key_pressed(key)) {
                let at = trend.cursor.unwrap_or(now);
                let mut times: Vec<_> = all
                    .iter()
                    .filter(|s| visible(s, trend))
                    .flat_map(|s| &s.points)
                    .map(|(at, _, _)| *at)
                    .filter(|at| *at >= start && *at <= now)
                    .collect();
                times.sort_unstable();
                times.dedup();
                let next = if forward {
                    times.iter().copied().find(|t| *t > at)
                } else {
                    times.iter().copied().rev().find(|t| *t < at)
                };
                if let Some(at) = next {
                    trend.cursor = Some(at);
                    trend.pinned = true;
                }
            }
        }
    }
    if before != (trend.cursor, trend.pinned) {
        ui.ctx().request_repaint();
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::CONTROL_RADIUS, Color32::from_black_alpha(28));
    let points: Vec<_> = all
        .iter()
        .filter(|s| visible(s, trend))
        .flat_map(|s| &s.points)
        .filter(|(at, a, b)| *at >= start && *at <= now && a.is_finite() && b.is_finite())
        .collect();
    let values = points
        .iter()
        .map(|(_, a, b)| if temperature { *a } else { *b });
    let (low, high) = values.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    });
    let (low, high) = if points.is_empty() {
        if temperature { (20., 30.) } else { (40., 60.) }
    } else {
        let pad = ((high - low) * 0.18).max(if temperature { 0.5 } else { 2. });
        if temperature {
            ((low - pad).floor(), (high + pad).ceil())
        } else {
            ((low - pad).floor().max(0.), (high + pad).ceil().min(100.))
        }
    };
    for i in 0..3 {
        let f = i as f32 / 2.;
        let y = plot.bottom() - plot.height() * f;
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(1., t.line),
        );
        painter.text(
            egui::pos2(plot.left() - 8., y),
            Align2::RIGHT_CENTER,
            format!("{:.0}", low + (high - low) * f),
            font(10.),
            t.muted,
        );
    }
    for i in 0..5 {
        let at = start + (now - start) * i / 4;
        let label = if i == 4 {
            "现在".into()
        } else {
            chrono::DateTime::from_timestamp(at, 0)
                .map(|d| {
                    d.with_timezone(&chrono::Local)
                        .format(if trend.hours >= 24 {
                            "%d日 %H:%M"
                        } else {
                            "%H:%M"
                        })
                        .to_string()
                })
                .unwrap_or_default()
        };
        painter.text(
            egui::pos2(
                plot.left() + plot.width() * i as f32 / 4.,
                plot.bottom() + 10.,
            ),
            if i == 0 {
                Align2::LEFT_TOP
            } else if i == 4 {
                Align2::RIGHT_TOP
            } else {
                Align2::CENTER_TOP
            },
            label,
            font(10.),
            t.muted,
        );
    }
    let at = |time: i64, value: f32| {
        egui::pos2(
            plot.left() + (time - start) as f32 / (now - start) as f32 * plot.width(),
            plot.bottom() - (value - low) / (high - low) * plot.height(),
        )
    };
    for series in all.iter().filter(|s| visible(s, trend)) {
        let mut path = Vec::new();
        let mut previous = None;
        let draw = |path: &mut Vec<egui::Pos2>| {
            if path.len() == 1 {
                painter.circle_filled(path[0], 2.5, series.color);
            } else if !path.is_empty() {
                painter.add(egui::Shape::line(
                    std::mem::take(path),
                    Stroke::new(2.2, series.color),
                ));
            }
            path.clear();
        };
        for &(time, a, b) in series
            .points
            .iter()
            .filter(|(time, a, b)| *time >= start && *time <= now && a.is_finite() && b.is_finite())
        {
            if previous.is_some_and(|last| time - last > 1800) {
                draw(&mut path);
            }
            path.push(at(time, if temperature { a } else { b }));
            previous = Some(time);
        }
        draw(&mut path);
        if let Some(time) = trend.cursor.filter(|time| *time >= start && *time <= now)
            && let Some((sample_at, a, b)) =
                sample(&series.points, time).filter(|(time, _, _)| *time >= start && *time <= now)
        {
            let point = at(sample_at, if temperature { a } else { b });
            painter.circle_filled(point, 4., series.color);
            painter.circle_stroke(point, 6., Stroke::new(1.5, t.text));
        }
    }
    if points.is_empty() {
        painter.text(
            plot.center(),
            Align2::CENTER_CENTER,
            "这个时段暂无记录",
            font(13.),
            t.secondary,
        );
    }
    if let Some(time) = trend.cursor.filter(|time| *time >= start && *time <= now) {
        let x = at(time, low).x;
        painter.vline(x, plot.y_range(), Stroke::new(1., t.secondary));
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
    (plot, hovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gaps_do_not_return_a_distant_or_interpolated_value() {
        let points = [(1000, 20., 40.), (5000, 30., 60.)];
        assert_eq!(sample(&points, 1200), Some(points[0]));
        assert_eq!(sample(&points, 3000), None);
        assert_eq!(sample(&points, 5000), Some(points[1]));
    }
    #[test]
    fn actual_plot_hover_pin_keyboard_and_escape_keep_a_shared_time() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let now = 100000;
        let series = vec![Series {
            id: "a".into(),
            name: "客厅".into(),
            color: Color32::WHITE,
            points: vec![(now - 3600, 22., 45.), (now, 24., 50.)],
        }];
        let mut trend = Trend::default();
        let frame = |trend: &mut Trend, events| {
            let mut rect = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700., 300.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = show(ui, &series, trend, true, now, Tokens::home()).0;
                },
            );
            output.textures_delta.clear();
            rect
        };
        frame(&mut trend, vec![]);
        let rect = frame(&mut trend, vec![]);
        let p = egui::pos2(rect.left() + rect.width() * 0.75, rect.center().y);
        frame(&mut trend, vec![egui::Event::PointerMoved(p)]);
        let at = trend.cursor.unwrap();
        assert!(sample(&series[0].points, at).is_none());
        let click = |pressed| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&mut trend, vec![click(true), click(false)]);
        assert!(trend.pinned);
        frame(
            &mut trend,
            vec![egui::Event::PointerMoved(p + egui::vec2(10., 0.))],
        );
        assert_eq!(trend.cursor, Some(at));
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(&mut trend, vec![key(egui::Key::ArrowRight)]);
        assert_eq!(trend.cursor, Some(now - 3600));
        frame(&mut trend, vec![key(egui::Key::Escape)]);
        assert_eq!(trend.cursor, None);
        assert!(!trend.pinned);
        frame(&mut trend, vec![]);
        assert_eq!(trend.cursor, None);
    }
}
