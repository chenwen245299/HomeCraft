//! Todos: four-quadrant, day-timeline and month-calendar views over the hub's list. Edits go to
//! the backend's todo task, which stores them and writes this Mac's iCloud Drive partition.
use super::{
    Home, Intent,
    model::Sheet,
    theme::{self, Tokens, bold, font},
};
use crate::{
    backend::State,
    todo::{self, Command, Todo},
};
use chrono::{Datelike, NaiveDate};
use eframe::egui::{self, Color32, RichText, Sense};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TodoView {
    #[default]
    Quadrant,
    Timeline,
    Calendar,
}
pub const COLORS: [Color32; 4] = [
    Color32::from_rgb(235, 87, 87),
    Color32::from_rgb(86, 140, 255),
    Color32::from_rgb(240, 196, 64),
    Color32::from_rgb(70, 190, 120),
];
/// Editing state for the todo sheet; text fields are parsed only when saving.
#[derive(Default)]
pub struct Draft {
    pub todo: Option<Todo>,
    pub date: String,
    pub time: String,
    /// Put the cursor in the sheet once: the title when empty, else the details.
    pub focus: bool,
}
/// Quick-pick times in the editor; picking one on an undated todo also sets today.
const TIMES: [(&str, u16); 5] = [
    ("09:00", 540),
    ("12:00", 720),
    ("14:00", 840),
    ("18:00", 1080),
    ("20:00", 1200),
];
fn hhmm(m: u16) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}
/// Drag payload: the id of the todo being moved between quadrants.
struct Dragged(String);
/// The todo to save when `id` is dropped on quadrant `q`; None when nothing changes.
fn moved(todos: &[Todo], id: &str, q: u8) -> Option<Todo> {
    let mut todo = todos.iter().find(|t| t.id == id && !t.deleted)?.clone();
    (todo.quadrant != q && q < 4).then(|| {
        todo.quadrant = q;
        todo
    })
}
/// Small "has details" mark drawn next to a title.
fn notes_mark(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12., 12.), Sense::hover());
    for (i, width) in [10., 10., 6.].into_iter().enumerate() {
        let y = rect.top() + 3. + i as f32 * 3.;
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + 1., y),
                egui::pos2(rect.left() + 1. + width, y),
            ],
            egui::Stroke::new(1.3, color),
        );
    }
}
fn date(day: u32) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d").ok()
}
fn day_id(d: NaiveDate) -> u32 {
    d.year() as u32 * 10000 + d.month() * 100 + d.day()
}
fn shift(day: u32, days: i64) -> u32 {
    date(day).map_or(day, |d| day_id(d + chrono::Duration::days(days)))
}
pub fn when(t: &Todo, today: u32) -> String {
    let day = match t.date {
        None => String::new(),
        Some(d) if d == today => "今天".into(),
        Some(d) if d == shift(today, 1) => "明天".into(),
        Some(d) if d < today => format!("逾期 {}月{}日", d / 100 % 100, d % 100),
        Some(d) => format!("{}月{}日", d / 100 % 100, d % 100),
    };
    match t.time {
        Some(m) => format!("{day} {:02}:{:02}", m / 60, m % 60)
            .trim()
            .to_owned(),
        None => day,
    }
}
fn parse_time(s: &str) -> Option<Option<u16>> {
    let s = s.trim();
    if s.is_empty() {
        return Some(None);
    }
    let (h, m) = s.split_once(':').or_else(|| s.split_once('：'))?;
    let (h, m): (u16, u16) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (h < 24 && m < 60).then_some(Some(h * 60 + m))
}
fn parse_date(s: &str) -> Option<Option<u32>> {
    let s = s.trim();
    if s.is_empty() {
        return Some(None);
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(|d| Some(day_id(d)))
        .filter(|d| d.is_some_and(todo::valid_day))
}
impl Home {
    /// Preview/test hook: 0 quadrant, 1 timeline, 2 calendar.
    pub fn show_todo_view(&mut self, view: u8) {
        self.todo_view = match view {
            1 => TodoView::Timeline,
            2 => TodoView::Calendar,
            _ => TodoView::Quadrant,
        };
    }
    pub fn preview_edit_todo(&mut self, todo: Todo) {
        self.edit_todo(todo);
    }
    pub(super) fn todo_page(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        let today = todo::today();
        if self.todo_day == 0 {
            self.todo_day = today;
        }
        // A list page: compact controls so the todos, not the chrome, take the room.
        ui.spacing_mut().button_padding = egui::vec2(10., 4.);
        ui.spacing_mut().interact_size.y = theme::COMPACT_ROW;
        ui.horizontal(|ui| {
            for (view, label) in [
                (TodoView::Quadrant, "四象限"),
                (TodoView::Timeline, "时间线"),
                (TodoView::Calendar, "日历"),
            ] {
                let button = ui.add(theme::compact_button(label, t, false).fill(
                    if self.todo_view == view {
                        Color32::from_rgba_unmultiplied(255, 151, 41, 64)
                    } else {
                        t.field
                    },
                ));
                self.target(&format!("todo.view.{label}"), button.rect);
                if button.clicked() {
                    self.todo_view = view;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add = ui.add(theme::compact_button("新建待办", t, true));
                self.target("todo.new", add.rect);
                if add.clicked() {
                    let mut fresh = Todo::new("", 0);
                    if self.todo_view != TodoView::Quadrant {
                        fresh.date = Some(self.todo_day);
                    }
                    self.edit_todo(fresh);
                }
                let done = ui.add(theme::compact_button("显示已完成", t, false).fill(
                    if self.todo_show_done {
                        Color32::from_rgba_unmultiplied(255, 151, 41, 64)
                    } else {
                        t.field
                    },
                ));
                self.target("todo.show_done", done.rect);
                if done.clicked() {
                    self.todo_show_done = !self.todo_show_done;
                }
                // The count shares the tab row instead of taking a line of its own.
                let open = state.todos.iter().filter(|t| t.open()).count();
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{open} 项未完成{}",
                            if state.todo_note.is_empty() {
                                String::new()
                            } else {
                                format!(" · {}", state.todo_note)
                            }
                        ))
                        .font(font(12.))
                        .color(t.muted),
                    )
                    .truncate(),
                );
            });
        });
        match self.todo_view {
            TodoView::Quadrant => self.todo_quadrants(ui, state, t, intents, today),
            TodoView::Timeline => self.todo_timeline(ui, state, t, intents, today),
            TodoView::Calendar => self.todo_calendar(ui, state, t, today),
        }
    }
    fn edit_todo(&mut self, todo: Todo) {
        self.todo_draft = Draft {
            date: todo
                .date
                .and_then(date)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            time: todo.time.map(hhmm).unwrap_or_default(),
            focus: todo.updated == 0,
            todo: Some(todo),
        };
        self.open_sheet(Sheet::Todo);
    }
    fn todo_row(
        &mut self,
        ui: &mut egui::Ui,
        todo: &Todo,
        t: Tokens,
        intents: &mut Vec<Intent>,
        today: u32,
        dot: bool,
    ) {
        ui.horizontal(|ui| {
            let mut done = todo.done;
            if ui.checkbox(&mut done, "").changed() {
                intents.push(Intent::Todo(Command::Toggle(todo.id.clone())));
            }
            // Only where quadrants mix; a quadrant card already shows its colour in the header.
            if dot {
                let (dot, _) = ui.allocate_exact_size(egui::vec2(8., 8.), Sense::hover());
                ui.painter().circle_filled(
                    dot.center(),
                    4.,
                    COLORS[usize::from(todo.quadrant.min(3))],
                );
            }
            let title = RichText::new(&todo.title)
                .font(font(14.))
                .color(if todo.done { t.muted } else { t.text });
            let title = if todo.done {
                title.strikethrough()
            } else {
                title
            };
            let w = when(todo, today);
            let has_notes = !todo.notes.is_empty();
            // Leave room for the details mark and the date so a long title truncates first.
            let mut reserve = 0.;
            if !w.is_empty() {
                reserve += ui
                    .painter()
                    .layout_no_wrap(w.clone(), font(12.), t.secondary)
                    .size()
                    .x
                    + ui.spacing().item_spacing.x;
            }
            if has_notes {
                reserve += 12. + ui.spacing().item_spacing.x;
            }
            let label = egui::Label::new(title)
                .truncate()
                .show_tooltip_when_elided(!has_notes)
                .sense(Sense::click_and_drag());
            let mut label = ui
                .allocate_ui_with_layout(
                    egui::vec2(
                        (ui.available_width() - reserve).max(40.),
                        ui.spacing().interact_size.y,
                    ),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| ui.add(label),
                )
                .inner;
            self.target(&format!("todo.item.{}", todo.id), label.rect);
            if has_notes {
                label = label.on_hover_ui(|ui| {
                    ui.label(RichText::new(&todo.title).font(bold(13.)));
                    ui.label(RichText::new(&todo.notes).font(font(12.)));
                });
                notes_mark(ui, t.secondary);
            }
            if label.drag_started() {
                egui::DragAndDrop::set_payload(ui.ctx(), Dragged(todo.id.clone()));
            }
            if label.dragged()
                && let Some(at) = ui.ctx().pointer_interact_pos()
            {
                // The title follows the pointer above everything else while dragging.
                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("todo.drag"),
                ));
                let galley = theme::fitted_text(&painter, &todo.title, bold(13.), t.text, 240.);
                let chip = egui::Rect::from_min_size(
                    at + egui::vec2(12., 8.),
                    galley.size() + egui::vec2(28., 12.),
                );
                painter.rect_filled(chip, theme::CONTROL_RADIUS, t.sheet);
                painter.rect_stroke(
                    chip,
                    theme::CONTROL_RADIUS,
                    egui::Stroke::new(1., COLORS[usize::from(todo.quadrant.min(3))]),
                    egui::StrokeKind::Inside,
                );
                painter.circle_filled(
                    chip.left_center() + egui::vec2(10., 0.),
                    4.,
                    COLORS[usize::from(todo.quadrant.min(3))],
                );
                painter.galley(chip.min + egui::vec2(20., 6.), galley, t.text);
            }
            if label
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.edit_todo(todo.clone());
            }
            if !w.is_empty() {
                ui.label(RichText::new(w).font(font(12.)).color(
                    if todo.date.is_some_and(|d| d < today) && !todo.done {
                        t.error
                    } else {
                        t.secondary
                    },
                ));
            }
        });
    }
    fn todo_quadrants(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
        today: u32,
    ) {
        let order = todo::display_order(&state.todos, today);
        // Equal fixed-height cards: the list scrolls inside and the add row stays at the bottom.
        // Rows are as far apart as the columns (one item spacing).
        const PAD: egui::Margin = egui::Margin::symmetric(14, 10);
        ui.spacing_mut().item_spacing.y = 8.;
        let card = ((ui.available_height() - ui.spacing().item_spacing.y - 8.) / 2.).max(240.);
        // The usual Eisenhower layout: important on top, urgent on the right.
        const GRID: [[u8; 2]; 2] = [[1, 0], [3, 2]];
        for row in GRID {
            ui.columns(2, |cols| {
                for (&q, ui) in row.iter().zip(cols.iter_mut()) {
                    let dragging =
                        egui::DragAndDrop::payload::<Dragged>(ui.ctx()).map(|d| d.0.clone());
                    let card_rect = theme::surface(t)
                        .inner_margin(PAD)
                        .show(ui, |ui| {
                            ui.set_height(card - PAD.sum().y);
                            ui.spacing_mut().item_spacing.y = 4.;
                            ui.horizontal(|ui| {
                                let (dot, _) =
                                    ui.allocate_exact_size(egui::vec2(12., 12.), Sense::hover());
                                ui.painter().circle_filled(
                                    dot.center(),
                                    6.,
                                    COLORS[usize::from(q)],
                                );
                                ui.label(
                                    RichText::new(todo::QUADRANTS[usize::from(q)])
                                        .font(bold(15.))
                                        .color(COLORS[usize::from(q)]),
                                );
                                ui.label(
                                    RichText::new(todo::HINTS[usize::from(q)])
                                        .font(font(12.))
                                        .color(t.muted),
                                );
                                let open = order.iter().filter(|t| t.quadrant == q).count();
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(open.to_string())
                                                .font(font(13.))
                                                .color(t.secondary),
                                        );
                                    },
                                );
                            });
                            ui.separator();
                            let items: Vec<Todo> = order
                                .iter()
                                .filter(|t| t.quadrant == q)
                                .map(|t| (*t).clone())
                                .collect();
                            let finished: Vec<Todo> = state
                                .todos
                                .iter()
                                .filter(|t| t.quadrant == q && t.done)
                                .cloned()
                                .collect();
                            // Separator and add row below the list.
                            let list = (ui.available_height()
                                - theme::COMPACT_ROW
                                - 2. * ui.spacing().item_spacing.y
                                - 6.)
                                .max(40.);
                            egui::ScrollArea::vertical()
                                .id_salt(("todo.list", q))
                                .max_height(list)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let show_done = self.todo_show_done;
                                    if items.is_empty() && (finished.is_empty() || !show_done) {
                                        ui.label(
                                            RichText::new("还没有待办，在下面添加或把待办拖进来。")
                                                .font(font(13.))
                                                .color(t.muted),
                                        );
                                    }
                                    // Done items, when shown, follow the open ones.
                                    for item in
                                        items.iter().chain(finished.iter().filter(|_| show_done))
                                    {
                                        self.todo_row(ui, item, t, intents, today, false);
                                    }
                                });
                            ui.separator();
                            // Buttons first from the right, so the field takes exactly what is left.
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let more = ui
                                        .add(theme::compact_button("详细", t, false))
                                        .on_hover_text("设置日期、时间和详情后再添加");
                                    self.target(&format!("todo.quick.more.{q}"), more.rect);
                                    let add = ui.add(theme::compact_button("+", t, false));
                                    let field = ui.add(
                                        theme::compact_field(&mut self.todo_quick[usize::from(q)])
                                            .hint_text("添加待办，回车保存")
                                            .desired_width(ui.available_width())
                                            .char_limit(60),
                                    );
                                    self.target(&format!("todo.quick.{q}"), field.rect);
                                    let submit = field.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if (submit || add.clicked())
                                        && todo::valid_title(&self.todo_quick[usize::from(q)])
                                    {
                                        let title =
                                            std::mem::take(&mut self.todo_quick[usize::from(q)]);
                                        intents.push(Intent::Todo(Command::Save(Todo::new(
                                            &title, q,
                                        ))));
                                    } else if more.clicked() {
                                        let title =
                                            std::mem::take(&mut self.todo_quick[usize::from(q)]);
                                        self.edit_todo(Todo::new(&title, q));
                                    }
                                },
                            );
                        })
                        .response
                        .rect;
                    if let Some(id) = dragging
                        && ui.rect_contains_pointer(card_rect)
                    {
                        let target = moved(&state.todos, &id, q);
                        if target.is_some() {
                            ui.painter().rect_stroke(
                                card_rect,
                                theme::SURFACE_RADIUS,
                                egui::Stroke::new(2., COLORS[usize::from(q)]),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if ui.input(|i| i.pointer.any_released()) {
                            egui::DragAndDrop::clear_payload(ui.ctx());
                            if let Some(todo) = target {
                                intents.push(Intent::Todo(Command::Save(todo)));
                            }
                        }
                    }
                }
            });
        }
    }
    fn todo_timeline(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
        today: u32,
    ) {
        ui.horizontal(|ui| {
            if ui.add(theme::compact_button("‹", t, false)).clicked() {
                self.todo_day = shift(self.todo_day, -1);
            }
            if ui.add(theme::compact_button("今天", t, false)).clicked() {
                self.todo_day = today;
            }
            if ui.add(theme::compact_button("›", t, false)).clicked() {
                self.todo_day = shift(self.todo_day, 1);
            }
            if let Some(d) = date(self.todo_day) {
                const WEEK: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
                ui.label(
                    RichText::new(format!(
                        "{}月{}日 {}",
                        d.month(),
                        d.day(),
                        WEEK[d.weekday().num_days_from_monday() as usize]
                    ))
                    .font(bold(16.))
                    .color(t.text),
                );
            }
        });
        ui.add_space(8.);
        let day = self.todo_day;
        let mut items: Vec<Todo> = state
            .todos
            .iter()
            .filter(|t| t.date == Some(day) && (self.todo_show_done || !t.done))
            .cloned()
            .collect();
        items.sort_by_key(|t| (t.done, t.time.unwrap_or(u16::MAX), t.quadrant));
        let all_day: Vec<Todo> = items.iter().filter(|t| t.time.is_none()).cloned().collect();
        theme::surface(t).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("全天").font(bold(14.)).color(t.secondary));
            if all_day.is_empty() {
                ui.label(RichText::new("—").color(t.muted));
            }
            for item in &all_day {
                self.todo_row(ui, item, t, intents, today, true);
            }
        });
        ui.add_space(8.);
        let hour_height = 34.;
        let first = items
            .iter()
            .filter_map(|t| t.time)
            .map(|m| m / 60)
            .min()
            .unwrap_or(8)
            .min(8);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), hour_height * f32::from(24 - first)),
            Sense::click(),
        );
        let painter = ui.painter_at(rect);
        for h in first..24 {
            let y = rect.top() + f32::from(h - first) * hour_height;
            painter.line_segment(
                [
                    egui::pos2(rect.left() + 48., y),
                    egui::pos2(rect.right(), y),
                ],
                egui::Stroke::new(1., t.line),
            );
            painter.text(
                egui::pos2(rect.left() + 4., y + 2.),
                egui::Align2::LEFT_TOP,
                format!("{h:02}:00"),
                font(12.),
                t.muted,
            );
        }
        if day == today {
            use chrono::Timelike;
            let now = chrono::Utc::now() + chrono::Duration::hours(8);
            let minutes = now.hour() as f32 * 60. + now.minute() as f32;
            let y = rect.top() + (minutes / 60. - f32::from(first)) * hour_height;
            if y >= rect.top() {
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + 48., y),
                        egui::pos2(rect.right(), y),
                    ],
                    egui::Stroke::new(2., t.accent),
                );
            }
        }
        let mut clicked_item = None;
        for (i, item) in items.iter().filter(|t| t.time.is_some()).enumerate() {
            let start = f32::from(item.time.unwrap_or(0)) / 60. - f32::from(first);
            let length = f32::from(item.minutes.unwrap_or(30)) / 60.;
            let block = egui::Rect::from_min_size(
                egui::pos2(
                    rect.left() + 56. + (i % 2) as f32 * 8.,
                    rect.top() + start * hour_height + 1.,
                ),
                egui::vec2(rect.width() - 64., (length * hour_height).max(22.) - 2.),
            );
            let color = COLORS[usize::from(item.quadrant.min(3))];
            painter.rect_filled(
                block,
                6.,
                color.gamma_multiply(if item.done { 0.25 } else { 0.55 }),
            );
            let heading = painter.text(
                block.left_top() + egui::vec2(8., 4.),
                egui::Align2::LEFT_TOP,
                format!("{}  {}", when(item, today), item.title),
                font(13.),
                if item.done { t.muted } else { t.text },
            );
            // Details: their own line in tall blocks, after the title in short ones.
            if let Some(line) = item.notes.lines().next() {
                let (at, width) = if block.height() >= 38. {
                    (block.left_top() + egui::vec2(8., 22.), block.width() - 16.)
                } else {
                    (
                        egui::pos2(heading.right() + 12., heading.top() + 1.),
                        block.right() - heading.right() - 20.,
                    )
                };
                if width > 24. {
                    let galley = theme::fitted_text(&painter, line, font(12.), t.secondary, width);
                    painter.galley(at, galley, t.secondary);
                }
            }
            self.target(&format!("todo.block.{}", item.id), block);
            if response.clicked()
                && response
                    .interact_pointer_pos()
                    .is_some_and(|p| block.contains(p))
            {
                clicked_item = Some(item.clone());
            }
        }
        if let Some(item) = clicked_item {
            self.edit_todo(item);
        } else if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            // Clicking an empty slot starts a todo at that hour.
            let hour = first + ((p.y - rect.top()) / hour_height) as u16;
            let mut fresh = Todo::new("", 0);
            fresh.date = Some(day);
            fresh.time = Some(hour.min(23) * 60);
            fresh.minutes = Some(60);
            self.edit_todo(fresh);
        }
    }
    fn todo_calendar(&mut self, ui: &mut egui::Ui, state: &State, t: Tokens, today: u32) {
        if self.todo_month == 0 {
            self.todo_month = today / 100;
        }
        let (year, month) = ((self.todo_month / 100) as i32, self.todo_month % 100);
        ui.horizontal(|ui| {
            if ui.add(theme::compact_button("‹", t, false)).clicked() {
                self.todo_month = if month == 1 {
                    (year as u32 - 1) * 100 + 12
                } else {
                    self.todo_month - 1
                };
            }
            if ui.add(theme::compact_button("本月", t, false)).clicked() {
                self.todo_month = today / 100;
            }
            if ui.add(theme::compact_button("›", t, false)).clicked() {
                self.todo_month = if month == 12 {
                    (year as u32 + 1) * 100 + 1
                } else {
                    self.todo_month + 1
                };
            }
            ui.label(
                RichText::new(format!("{year}年{month}月"))
                    .font(bold(16.))
                    .color(t.text),
            );
        });
        ui.add_space(8.);
        let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
            return;
        };
        let offset = first.weekday().num_days_from_monday() as i64;
        let width = ui.available_width();
        let cell = egui::vec2(width / 7., 84.);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, 22. + cell.y * 6.), Sense::click());
        let painter = ui.painter_at(rect);
        for (i, name) in ["一", "二", "三", "四", "五", "六", "日"]
            .iter()
            .enumerate()
        {
            painter.text(
                egui::pos2(rect.left() + cell.x * (i as f32 + 0.5), rect.top() + 10.),
                egui::Align2::CENTER_CENTER,
                *name,
                font(12.),
                t.muted,
            );
        }
        let mut clicked_day = None;
        for index in 0..42 {
            let d = first + chrono::Duration::days(index - offset);
            let id = day_id(d);
            let cell_rect = egui::Rect::from_min_size(
                egui::pos2(
                    rect.left() + cell.x * (index % 7) as f32,
                    rect.top() + 22. + cell.y * (index / 7) as f32,
                ),
                cell,
            )
            .shrink(2.);
            let in_month = d.month() == month;
            painter.rect_filled(
                cell_rect,
                6.,
                if id == today {
                    t.accent.gamma_multiply(0.25)
                } else {
                    t.field
                },
            );
            painter.text(
                cell_rect.left_top() + egui::vec2(6., 4.),
                egui::Align2::LEFT_TOP,
                d.day().to_string(),
                bold(13.),
                if in_month { t.text } else { t.muted },
            );
            let mut day_items: Vec<&Todo> =
                state.todos.iter().filter(|t| t.date == Some(id)).collect();
            day_items.sort_by_key(|t| (t.done, t.time.unwrap_or(u16::MAX)));
            for (row, item) in day_items.iter().take(3).enumerate() {
                let y = cell_rect.top() + 24. + row as f32 * 17.;
                painter.circle_filled(
                    egui::pos2(cell_rect.left() + 9., y + 7.),
                    3.,
                    COLORS[usize::from(item.quadrant.min(3))],
                );
                let galley = theme::fitted_text(
                    &painter,
                    &item.title,
                    font(11.),
                    if item.done { t.muted } else { t.secondary },
                    cell_rect.width() - 22.,
                );
                painter.galley(egui::pos2(cell_rect.left() + 16., y), galley, t.secondary);
            }
            if day_items.len() > 3 {
                painter.text(
                    cell_rect.right_bottom() - egui::vec2(6., 4.),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("+{}", day_items.len() - 3),
                    font(11.),
                    t.muted,
                );
            }
            if response.clicked()
                && response
                    .interact_pointer_pos()
                    .is_some_and(|p| cell_rect.contains(p))
            {
                clicked_day = Some(id);
            }
        }
        if let Some(day) = clicked_day {
            self.todo_day = day;
            self.todo_view = TodoView::Timeline;
        }
    }
    /// Home card: the first open todos in display order; opens the todo page.
    pub(super) fn todo_card(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        state: &State,
        t: Tokens,
    ) {
        let response = ui.interact(rect, ui.id().with("home.todos"), Sense::click());
        theme::paint_surface(ui.painter(), rect, t, response.hovered());
        let today = todo::today();
        let order = todo::display_order(&state.todos, today);
        let painter = ui.painter();
        painter.text(
            rect.min + egui::vec2(18., 22.),
            egui::Align2::LEFT_CENTER,
            "待办",
            bold(15.),
            t.text,
        );
        painter.text(
            rect.right_top() + egui::vec2(-18., 22.),
            egui::Align2::RIGHT_CENTER,
            format!("{} 项", order.len()),
            font(12.),
            t.secondary,
        );
        if order.is_empty() {
            painter.text(
                rect.min + egui::vec2(18., 56.),
                egui::Align2::LEFT_CENTER,
                "没有未完成的待办",
                font(14.),
                t.muted,
            );
        }
        for (i, item) in order.iter().take(3).enumerate() {
            let y = rect.top() + 50. + i as f32 * 22.;
            painter.circle_filled(
                egui::pos2(rect.left() + 22., y),
                4.,
                COLORS[usize::from(item.quadrant.min(3))],
            );
            let w = when(item, today);
            let galley =
                theme::fitted_text(painter, &item.title, font(14.), t.text, rect.width() - 130.);
            painter.galley(egui::pos2(rect.left() + 34., y - 9.), galley, t.text);
            painter.text(
                egui::pos2(rect.right() - 18., y),
                egui::Align2::RIGHT_CENTER,
                w,
                font(12.),
                t.secondary,
            );
        }
        self.target("home.todos", rect);
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.nav = super::model::Nav::Todo;
        }
    }
    pub(super) fn todo_editor(&mut self, ui: &mut egui::Ui, t: Tokens, intents: &mut Vec<Intent>) {
        let Some(todo) = self.todo_draft.todo.as_mut() else {
            self.close_sheet();
            return;
        };
        ui.label(RichText::new("标题").size(12.).color(t.sheet_muted));
        let title = ui.add(
            theme::text_field(&mut todo.title)
                .desired_width(f32::INFINITY)
                .char_limit(60),
        );
        self.targets.insert(
            "todo.edit.title".into(),
            [
                title.rect.min.x,
                title.rect.min.y,
                title.rect.width(),
                title.rect.height(),
            ],
        );
        ui.add_space(8.);
        ui.label(RichText::new("详情（可选）").size(12.).color(t.sheet_muted));
        let title_empty = todo.title.trim().is_empty();
        let notes = ui.add(
            theme::text_area(&mut todo.notes)
                .hint_text("地点、链接、要带的东西…")
                .desired_width(f32::INFINITY)
                .char_limit(todo::NOTES_CHARS),
        );
        self.targets.insert(
            "todo.edit.notes".into(),
            [
                notes.rect.min.x,
                notes.rect.min.y,
                notes.rect.width(),
                notes.rect.height(),
            ],
        );
        if std::mem::take(&mut self.todo_draft.focus) {
            if title_empty {
                title.request_focus();
            } else {
                notes.request_focus();
            }
        }
        ui.add_space(8.);
        ui.label(RichText::new("象限").size(12.).color(t.sheet_muted));
        ui.horizontal_wrapped(|ui| {
            for q in 0..4u8 {
                let selected = todo.quadrant == q;
                let label = RichText::new(todo::QUADRANTS[usize::from(q)])
                    .font(bold(13.))
                    .color(if selected { t.ink } else { t.sheet_muted });
                if ui
                    .add(
                        egui::Button::new(label)
                            .fill(if selected {
                                COLORS[usize::from(q)].gamma_multiply(0.6)
                            } else {
                                t.field
                            })
                            .corner_radius(theme::CONTROL_RADIUS),
                    )
                    .clicked()
                {
                    todo.quadrant = q;
                }
            }
        });
        ui.add_space(8.);
        ui.label(
            RichText::new("日期与时间（可选）")
                .size(12.)
                .color(t.sheet_muted),
        );
        let today = todo::today();
        ui.horizontal_wrapped(|ui| {
            ui.add(
                theme::text_field(&mut self.todo_draft.date)
                    .hint_text("2026-10-27")
                    .desired_width(120.)
                    .char_limit(10),
            );
            for (label, value) in [
                ("今天", Some(today)),
                ("明天", Some(shift(today, 1))),
                ("不设", None),
            ] {
                if ui.add(theme::secondary_button(label, t)).clicked() {
                    self.todo_draft.date = value
                        .and_then(date)
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default();
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(
                theme::text_field(&mut self.todo_draft.time)
                    .hint_text("09:30")
                    .desired_width(70.)
                    .char_limit(5),
            );
            let current = parse_time(&self.todo_draft.time).flatten();
            for (label, minutes) in TIMES {
                let selected = current == Some(minutes);
                if ui.selectable_label(selected, label).clicked() {
                    self.todo_draft.time = if selected {
                        String::new()
                    } else {
                        hhmm(minutes)
                    };
                    if !selected && self.todo_draft.date.trim().is_empty() {
                        self.todo_draft.date = date(today)
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_default();
                    }
                }
            }
            if current.is_some() && ui.selectable_label(false, "清除").clicked() {
                self.todo_draft.time.clear();
            }
        });
        let todo = self.todo_draft.todo.as_mut().expect("draft present");
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("时长").size(12.).color(t.sheet_muted));
            for minutes in [15u16, 30, 60, 90, 120] {
                let selected = todo.minutes == Some(minutes);
                if ui
                    .selectable_label(selected, format!("{minutes} 分钟"))
                    .clicked()
                {
                    todo.minutes = if selected { None } else { Some(minutes) };
                }
            }
        });
        if todo.updated > 0 {
            ui.checkbox(&mut todo.done, "已完成");
        }
        let parsed = (
            parse_date(&self.todo_draft.date),
            parse_time(&self.todo_draft.time),
        );
        let valid = todo::valid_title(&todo.title) && parsed.0.is_some() && parsed.1.is_some();
        if !valid {
            ui.label(
                RichText::new("标题 1–60 字；日期如 2026-10-27，时间如 09:30。")
                    .font(font(12.))
                    .color(t.error),
            );
        }
        ui.add_space(10.);
        ui.horizontal(|ui| {
            let save = ui.add_enabled(valid, theme::primary_button("保存", t));
            self.targets.insert(
                "todo.edit.save".into(),
                [
                    save.rect.min.x,
                    save.rect.min.y,
                    save.rect.width(),
                    save.rect.height(),
                ],
            );
            if save.clicked()
                && let (Some(date), Some(time)) = parsed
                && let Some(mut todo) = self.todo_draft.todo.take()
            {
                todo.title = todo.title.trim().to_owned();
                todo.notes = todo::tidy_notes(&todo.notes);
                todo.date = date;
                todo.time = time;
                if todo.time.is_none() {
                    todo.minutes = None;
                }
                intents.push(Intent::Todo(Command::Save(todo)));
                self.close_sheet();
            }
            let exists = self.todo_draft.todo.as_ref().is_some_and(|t| t.updated > 0);
            if exists
                && ui.add(theme::secondary_button("删除", t)).clicked()
                && let Some(todo) = self.todo_draft.todo.take()
            {
                intents.push(Intent::Todo(Command::Delete(todo.id)));
                self.close_sheet();
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_and_times_parse_strictly() {
        assert_eq!(parse_time("09:30"), Some(Some(570)));
        assert_eq!(parse_time("9：05"), Some(Some(545)));
        assert_eq!(parse_time(""), Some(None));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_date("2026-10-27"), Some(Some(20261027)));
        assert_eq!(parse_date("2026-02-30"), None);
        assert_eq!(parse_date(""), Some(None));
        assert_eq!(shift(20261231, 1), 20270101);
        let mut todo = Todo::new("开会", 0);
        todo.date = Some(20261027);
        todo.time = Some(570);
        assert_eq!(when(&todo, 20261027), "今天 09:30");
        todo.date = Some(20261020);
        todo.time = None;
        assert_eq!(when(&todo, 20261027), "逾期 10月20日");
    }
    #[test]
    fn dropping_on_another_quadrant_moves_only_that_todo() {
        let a = Todo::new("开会", 0);
        let mut gone = Todo::new("旧事", 1);
        gone.deleted = true;
        let all = vec![a.clone(), gone.clone()];
        let to = moved(&all, &a.id, 2).unwrap();
        assert_eq!(
            (to.id.as_str(), to.quadrant, to.title.as_str()),
            (a.id.as_str(), 2, "开会")
        );
        assert!(
            moved(&all, &a.id, 0).is_none(),
            "same quadrant is not a change"
        );
        assert!(moved(&all, &gone.id, 0).is_none());
        assert!(moved(&all, "missing", 3).is_none());
        assert!(moved(&all, &a.id, 4).is_none());
    }
}
