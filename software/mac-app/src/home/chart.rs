//! Immediate readout, linked crosshair, click pin and keyboard navigation.
use super::theme::{self, Tokens, bold, font};
use crate::device::History;
use eframe::egui::{self, Align2, Rect, RichText, Sense, Stroke};

#[derive(Default)]
pub struct Selection {
    day: u32,
    pub index: Option<usize>,
    pinned: bool,
}
pub fn bucket(fraction: f32) -> usize {
    (fraction.clamp(0., 1.) * 96.).floor().min(95.) as usize
}
pub fn interval(index: usize) -> String {
    format!(
        "{:02}:{:02}–{:02}:{:02}",
        index / 4,
        index % 4 * 15,
        (index + 1) / 4,
        (index + 1) % 4 * 15
    )
}
impl Selection {
    fn prepare(&mut self, day: u32) {
        if self.day != day {
            self.day = day;
            self.index = None;
            self.pinned = false;
        }
    }
}
pub fn show(
    ui: &mut egui::Ui,
    history: &History,
    temperature: bool,
    t: Tokens,
    selection: &mut Selection,
) -> Rect {
    selection.prepare(history.day);
    let previous_selection = (selection.index, selection.pinned);
    let values: Vec<Option<f32>> = history
        .points
        .iter()
        .map(|p| p.map(|(c, h)| if temperature { c } else { h }))
        .collect();
    let known: Vec<f32> = values.iter().flatten().copied().collect();
    let color = if temperature { t.accent } else { t.climate };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 198.), Sense::click());
    ui.painter()
        .rect_filled(rect, theme::CONTROL_RADIUS, t.field);
    let plot = Rect::from_min_max(
        rect.min + egui::vec2(46., 69.),
        rect.max - egui::vec2(17., 27.),
    );
    if let Some(pointer) = response.hover_pos().filter(|p| plot.contains(*p)) {
        let index = bucket((pointer.x - plot.left()) / plot.width());
        if !selection.pinned || response.clicked() {
            selection.index = Some(index);
        }
        if response.clicked() {
            selection.pinned = !selection.pinned;
            response.request_focus();
        }
    }
    if response.has_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            selection.index = None;
            selection.pinned = false;
        }
        for (key, delta) in [(egui::Key::ArrowLeft, -1), (egui::Key::ArrowRight, 1)] {
            if ui.input(|i| i.key_pressed(key)) {
                selection.index =
                    Some((selection.index.unwrap_or(0) as i32 + delta).clamp(0, 95) as usize);
                selection.pinned = true;
            }
        }
    }
    if previous_selection != (selection.index, selection.pinned) {
        ui.ctx().request_repaint();
    }
    let selected = selection
        .index
        .or_else(|| values.iter().rposition(Option::is_some));
    let value = selected.and_then(|i| values.get(i).copied().flatten());
    let unit = if temperature { "°C" } else { "%" };
    ui.painter().text(
        rect.min + egui::vec2(13., 12.),
        Align2::LEFT_TOP,
        if temperature { "温度" } else { "湿度" },
        bold(14.),
        t.text,
    );
    ui.painter().text(
        rect.right_top() + egui::vec2(-14., 8.),
        Align2::RIGHT_TOP,
        value
            .map(|v| format!("{v:.2} {unit}"))
            .unwrap_or_else(|| "无采样".into()),
        bold(24.),
        color,
    );
    let description = selected
        .map(|i| {
            format!(
                "{} · 15分钟均值{}",
                interval(i),
                if selection.pinned {
                    " · 已固定"
                } else {
                    ""
                }
            )
        })
        .unwrap_or_else(|| "今天还没有有效采样".into());
    ui.painter().text(
        rect.min + egui::vec2(13., 40.),
        Align2::LEFT_TOP,
        description,
        font(11.),
        t.secondary,
    );
    let min = known
        .iter()
        .copied()
        .reduce(f32::min)
        .unwrap_or(if temperature { 20. } else { 40. });
    let max = known
        .iter()
        .copied()
        .reduce(f32::max)
        .unwrap_or(if temperature { 30. } else { 60. });
    let margin = ((max - min) * 0.15).max(if temperature { 0.5 } else { 2. });
    let lo = min - margin;
    let hi = max + margin;
    for i in 0..3 {
        let f = i as f32 / 2.;
        let y = plot.bottom() - f * plot.height();
        ui.painter().line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(1., t.line),
        );
        ui.painter().text(
            egui::pos2(plot.left() - 7., y),
            Align2::RIGHT_CENTER,
            format!("{:.1}", lo + (hi - lo) * f),
            font(10.),
            t.muted,
        );
    }
    for hour in [0, 6, 12, 18, 24] {
        ui.painter().text(
            egui::pos2(
                plot.left() + plot.width() * hour as f32 / 24.,
                plot.bottom() + 9.,
            ),
            Align2::CENTER_TOP,
            format!("{hour:02}"),
            font(10.),
            t.muted,
        );
    }
    let at = |i: usize, v: f32| {
        egui::pos2(
            plot.left() + plot.width() * (i as f32 + 0.5) / 96.,
            plot.bottom() - (v - lo) / (hi - lo) * plot.height(),
        )
    };
    let mut previous = None;
    for (i, value) in values.iter().enumerate() {
        if let Some(v) = value {
            let p = at(i, *v);
            if let Some(previous) = previous {
                ui.painter()
                    .line_segment([previous, p], Stroke::new(2., color));
            }
            ui.painter().circle_filled(p, 2., color);
            previous = Some(p);
        } else {
            previous = None;
        }
    }
    if let Some(i) = selected {
        let x = plot.left() + plot.width() * (i as f32 + 0.5) / 96.;
        ui.painter().line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(1., t.secondary),
        );
        if let Some(v) = value {
            let p = at(i, v);
            ui.painter().circle_filled(p, 5., t.field);
            ui.painter().circle_stroke(p, 4., Stroke::new(2., color));
        }
    }
    response.on_hover_ui(|ui| {
        ui.label(RichText::new("点击固定 · ← → 查看相邻时段 · Esc 取消").size(12.));
    });
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_buckets_and_midnight_without_interpolating_gaps() {
        assert_eq!(bucket(0.), 0);
        assert_eq!(bucket(0.5), 48);
        assert_eq!(bucket(1.), 95);
        assert_eq!(interval(95), "23:45–24:00");
    }
    #[test]
    fn midnight_resets_pinned_selection() {
        let mut s = Selection {
            day: 20261021,
            index: Some(65),
            pinned: true,
        };
        s.prepare(20261022);
        assert_eq!(s.index, None);
        assert!(!s.pinned);
    }
    #[test]
    fn actual_widget_hover_click_and_gaps_have_immediate_readouts() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut selection = Selection::default();
        let mut points = vec![Some((22.5, 45.)); 96];
        points[12] = None;
        let history = History {
            day: 20261021,
            revision: 1,
            received_at: 0,
            points,
        };
        let mut rect = Rect::NOTHING;
        let mut frame = |events: Vec<egui::Event>, selection: &mut Selection| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600., 300.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = show(ui, &history, true, Tokens::home(), selection);
                },
            );
            output.textures_delta.clear();
        };
        frame(vec![], &mut selection);
        let p = egui::pos2(46. + (600. - 46. - 17.) * 12.5 / 96., 115.);
        frame(vec![egui::Event::PointerMoved(p)], &mut selection);
        assert_eq!(selection.index, Some(12));
        frame(
            vec![
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
            &mut selection,
        );
        assert!(selection.pinned);
        frame(
            vec![egui::Event::PointerMoved(egui::pos2(500., 115.))],
            &mut selection,
        );
        assert_eq!(selection.index, Some(12));
    }
}
