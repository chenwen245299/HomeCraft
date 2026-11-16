//! Measured from the supplied Home reference: 52pt toolbar, 222pt sidebar,
//! 32pt navigation rows and 154pt accessory tiles. All visual tokens live here.
use eframe::egui::{self, Color32, FontFamily, FontId};
use std::sync::Arc;
pub const SIDEBAR: f32 = 222.0;
pub const TOOLBAR: f32 = 52.0;
pub const ROW: f32 = 34.0;
pub const TILE: egui::Vec2 = egui::vec2(174.0, 148.0);
pub const SURFACE_RADIUS: u8 = 20;
pub const MODAL_RADIUS: u8 = 24;
pub const CONTROL_RADIUS: u8 = 10;
pub const ICON_RADIUS: u8 = 7;
/// Every font renders at this fraction of the size the code asks for; one knob for overall text size.
pub const TEXT_SCALE: f32 = 0.9;

/// A single toolbar row. Metadata yields space to actions, never pushes them offscreen.
pub fn action_row<R>(
    ui: &mut egui::Ui,
    metadata: Option<(&str, &str)>,
    t: Tokens,
    actions: impl FnOnce(&mut egui::Ui) -> R,
) -> (R, Option<egui::Rect>) {
    ui.horizontal(|ui| {
        let result = actions(ui);
        let rect = metadata.and_then(|(text, detail)| {
            let width = ui.available_width();
            (width >= 24.).then(|| {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 34.),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(text).size(12.).color(t.muted))
                                .truncate(),
                        )
                        .on_hover_text(detail)
                        .rect
                    },
                )
                .inner
            })
        });
        (result, rect)
    })
    .inner
}
/// Single-line text in painted controls, with reliable ellipsis for long names.
pub fn fitted_text(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = width.max(0.);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    painter.layout_job(job)
}
pub fn section(ui: &mut egui::Ui, title: &str, t: Tokens) {
    ui.add_space(4.);
    ui.label(egui::RichText::new(title).font(bold(16.)).color(t.ink));
}
pub fn settings_link(ui: &mut egui::Ui, title: &str, detail: &str, t: Tokens) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.), egui::Sense::click());
    ui.painter().rect_filled(
        rect,
        CONTROL_RADIUS,
        if response.hovered() || response.has_focus() {
            t.line
        } else {
            t.field
        },
    );
    let heading = fitted_text(ui.painter(), title, bold(14.), t.text, rect.width() - 54.);
    ui.painter()
        .galley(rect.min + egui::vec2(14., 8.), heading, t.text);
    let subtitle = fitted_text(
        ui.painter(),
        detail,
        font(12.),
        t.secondary,
        rect.width() - 54.,
    );
    ui.painter()
        .galley(rect.min + egui::vec2(14., 29.), subtitle, t.secondary);
    super::icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(
            rect.right_center() - egui::vec2(21., 0.),
            egui::vec2(15., 15.),
        ),
        super::icons::Icon::Chevron,
        t.secondary,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[derive(Clone, Copy)]
pub struct Tokens {
    pub text: Color32,
    pub secondary: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub climate: Color32,
    pub usage: Color32,
    pub safe: Color32,
    pub sidebar: Color32,
    pub tile: Color32,
    pub tile_hover: Color32,
    pub glass: Color32,
    pub banner: Color32,
    pub line: Color32,
    pub sheet: Color32,
    pub ink: Color32,
    pub sheet_muted: Color32,
    pub field: Color32,
    pub card: Color32,
    pub error: Color32,
    pub backdrop: Color32,
    pub on_accent: Color32,
}
impl Tokens {
    pub fn home() -> Self {
        Self {
            text: Color32::from_rgb(255, 255, 255),
            secondary: Color32::from_white_alpha(218),
            muted: Color32::from_white_alpha(150),
            accent: Color32::from_rgb(255, 151, 41),
            climate: Color32::from_rgb(51, 207, 239),
            usage: Color32::from_rgb(255, 221, 62),
            safe: Color32::from_rgb(32, 218, 185),
            sidebar: Color32::from_rgba_unmultiplied(35, 47, 50, 100),
            tile: Color32::from_rgba_unmultiplied(24, 39, 48, 180),
            tile_hover: Color32::from_rgba_unmultiplied(43, 60, 70, 200),
            glass: Color32::from_rgba_unmultiplied(24, 39, 48, 180),
            banner: Color32::from_rgba_unmultiplied(24, 39, 48, 180),
            line: Color32::from_white_alpha(35),
            sheet: Color32::from_rgba_unmultiplied(24, 39, 48, 232),
            ink: Color32::from_rgb(248, 250, 252),
            sheet_muted: Color32::from_rgb(194, 207, 217),
            field: Color32::from_white_alpha(20),
            card: Color32::from_rgba_unmultiplied(24, 39, 48, 180),
            error: Color32::from_rgb(255, 202, 132),
            backdrop: Color32::from_black_alpha(115),
            on_accent: Color32::from_rgb(45, 27, 9),
        }
    }
}

pub fn surface(t: Tokens) -> egui::Frame {
    egui::Frame::new()
        .fill(t.card)
        .stroke(egui::Stroke::new(1., t.line))
        .corner_radius(SURFACE_RADIUS)
        .inner_margin(18.)
}
/// The same shared surface for fixed-size dashboard cards and accessory controls.
pub fn paint_surface(painter: &egui::Painter, rect: egui::Rect, t: Tokens, hovered: bool) {
    painter.rect_filled(
        rect,
        SURFACE_RADIUS,
        if hovered { t.tile_hover } else { t.card },
    );
    painter.rect_stroke(
        rect,
        SURFACE_RADIUS,
        egui::Stroke::new(1., t.line),
        egui::StrokeKind::Inside,
    );
}
pub fn modal(t: Tokens) -> egui::Frame {
    egui::Frame::new()
        .fill(t.sheet)
        .stroke(egui::Stroke::new(1., t.line))
        .corner_radius(MODAL_RADIUS)
        .inner_margin(26.)
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
}
pub fn primary_button(label: &str, t: Tokens) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label.to_owned())
            .font(bold(14.))
            .color(t.on_accent),
    )
    .fill(t.accent)
    .corner_radius(CONTROL_RADIUS)
    .min_size(egui::vec2(0., 34.))
}
pub fn secondary_button(label: &str, t: Tokens) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label.to_owned())
            .font(bold(14.))
            .color(t.ink),
    )
    .fill(t.field)
    .corner_radius(CONTROL_RADIUS)
    .min_size(egui::vec2(0., 34.))
}
/// Denser controls for list pages such as the todo quadrants: 28 pt tall with a 13 pt label.
/// Pair with `button_padding` (10, 4) and `interact_size.y` = COMPACT_ROW in that scope.
pub const COMPACT_ROW: f32 = 28.0;
pub fn compact_button(label: &str, t: Tokens, primary: bool) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label.to_owned())
            .font(bold(13.))
            .color(if primary { t.on_accent } else { t.ink }),
    )
    .fill(if primary { t.accent } else { t.field })
    .corner_radius(CONTROL_RADIUS)
    .min_size(egui::vec2(0., COMPACT_ROW))
}
pub fn compact_field(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .font(font(13.))
        .margin(egui::Margin::symmetric(8, 5))
        .min_size(egui::vec2(0., COMPACT_ROW))
}
pub fn text_field(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .font(font(14.))
        .margin(egui::Margin::symmetric(10, 8))
        .min_size(egui::vec2(0., 34.))
}
pub fn text_area(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(text)
        .font(font(14.))
        .margin(egui::Margin::symmetric(10, 8))
        .desired_rows(3)
}

fn visuals(t: Tokens) -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(t.ink);
    v.weak_text_color = Some(t.sheet_muted);
    v.hyperlink_color = t.error;
    v.faint_bg_color = t.field;
    v.extreme_bg_color = t.field;
    v.text_edit_bg_color = Some(t.field);
    v.code_bg_color = t.field;
    v.warn_fg_color = t.error;
    v.error_fg_color = t.error;
    v.window_fill = t.sheet;
    v.window_stroke = egui::Stroke::new(1., t.line);
    v.window_corner_radius = MODAL_RADIUS.into();
    v.menu_corner_radius = CONTROL_RADIUS.into();
    v.panel_fill = Color32::TRANSPARENT;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(255, 151, 41, 64);
    v.selection.stroke = egui::Stroke::new(1., t.text);
    v.collapsing_header_frame = true;
    for (widget, opacity) in [
        (&mut v.widgets.noninteractive, 12),
        (&mut v.widgets.inactive, 20),
        (&mut v.widgets.hovered, 35),
        (&mut v.widgets.active, 48),
        (&mut v.widgets.open, 35),
    ] {
        widget.bg_fill = Color32::from_white_alpha(opacity);
        widget.weak_bg_fill = widget.bg_fill;
        widget.bg_stroke = egui::Stroke::new(
            1.,
            Color32::from_white_alpha(if opacity >= 35 { 88 } else { 35 }),
        );
        widget.fg_stroke = egui::Stroke::new(1.5, t.ink);
        widget.corner_radius = CONTROL_RADIUS.into();
        widget.expansion = 0.;
    }
    v
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn install(ctx: &egui::Context) {
    use egui::epaint::text::VariationCoords;
    let mut defs = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        let mut regular = egui::FontData::from_owned(bytes);
        regular.tweak.coords = VariationCoords::new([(b"opsz", 17.0)]);
        let mut bold = regular.clone();
        bold.tweak.coords = VariationCoords::new([(b"opsz", 17.0), (b"wght", 600.0)]);
        defs.font_data.insert("sf".into(), Arc::new(regular));
        defs.font_data.insert("sf-bold".into(), Arc::new(bold));
        defs.families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "sf".into());
    }
    if let Some(bytes) = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ]
    .iter()
    .find_map(|path| std::fs::read(path).ok())
    {
        defs.font_data
            .insert("cjk".into(), Arc::new(egui::FontData::from_owned(bytes)));
        defs.families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("cjk".into());
    }
    let mut semibold = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if defs.font_data.contains_key("sf-bold") {
        semibold.insert(0, "sf-bold".into());
    }
    defs.families
        .insert(FontFamily::Name("semibold".into()), semibold);
    // Scales glyphs and row metrics alike, so layout follows the smaller text.
    for data in defs.font_data.values_mut() {
        Arc::make_mut(data).tweak.scale = TEXT_SCALE;
    }
    ctx.set_fonts(defs);
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = visuals(Tokens::home());
    style.text_styles.insert(egui::TextStyle::Body, font(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, bold(14.0));
    style.text_styles.insert(egui::TextStyle::Small, font(12.0));
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.interact_size.y = 34.;
    style.spacing.text_edit_width = 220.;
    style.spacing.scroll.bar_width = 8.;
    style.spacing.scroll.floating = true;
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
}

/// Original procedural seaside wallpaper. Generated once, no Apple image assets.
pub fn wallpaper(ctx: &egui::Context, dusk: bool) -> (egui::TextureHandle, egui::TextureHandle) {
    let (w, h) = (480usize, 360usize);
    let mut pixels = Vec::with_capacity(w * h);
    for row in 0..h {
        for col in 0..w {
            let x = col as f32 / (w - 1) as f32;
            let y = row as f32 / (h - 1) as f32;
            let sky = [116.0, 166.0, 196.0];
            let sand = [222.0, 206.0, 157.0];
            let blend = ((y - 0.14) / 0.8).clamp(0.0, 1.0);
            let mut rgb = [0.0; 3];
            for i in 0..3 {
                rgb[i] = sky[i] * (1.0 - blend) + sand[i] * blend;
            }
            let cloud = (-((x - 0.72) / 0.44).powi(2) - ((y - 0.43) / 0.34).powi(2)).exp() * 0.36;
            let dune = (-((x - 0.28) / 0.29).powi(2) - ((y - 1.03) / 0.22).powi(2)).exp() * 0.85;
            let gold = (-((x - 0.97) / 0.4).powi(2) - ((y - 0.91) / 0.30).powi(2)).exp() * 0.82;
            for i in 0..3 {
                rgb[i] = rgb[i] * (1.0 - cloud) + [235.0, 229.0, 208.0][i] * cloud;
                rgb[i] = rgb[i] * (1.0 - dune) + [106.0, 88.0, 61.0][i] * dune;
                rgb[i] = rgb[i] * (1.0 - gold) + [200.0, 146.0, 67.0][i] * gold;
                if dusk {
                    rgb[i] = rgb[i] * [0.63, 0.55, 0.65][i] + [23.0, 10.0, 13.0][i];
                }
            }
            pixels.push(Color32::from_rgb(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8));
        }
    }
    let source = egui::ColorImage::new([w, h], pixels);
    let bytes: Vec<u8> = source.pixels.iter().flat_map(|p| p.to_array()).collect();
    let rgba = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
        let i = (y as usize * w + x as usize) * 4;
        image::Rgba([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
    });
    let blurred = image::imageops::blur(&rgba, 12.);
    let frosted = ctx.load_texture(
        "HomeCraft frosted wallpaper",
        egui::ColorImage::from_rgba_unmultiplied([w, h], &blurred),
        egui::TextureOptions::LINEAR,
    );
    let wallpaper = ctx.load_texture(
        "EPaper original coastal glass",
        source,
        egui::TextureOptions::LINEAR,
    );
    (wallpaper, frosted)
}
