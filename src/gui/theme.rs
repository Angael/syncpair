use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme,
    ThemePreference, Visuals,
};

pub const ACCENT: Color32 = Color32::from_rgb(0x4f, 0x8c, 0xff);
pub const SUCCESS: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x7a);
pub const WARNING: Color32 = Color32::from_rgb(0xe8, 0xa3, 0x3d);
pub const DANGER: Color32 = Color32::from_rgb(0xe5, 0x5b, 0x5b);

pub const RADIUS: u8 = 10;

pub fn install(ctx: &egui::Context) {
    ctx.options_mut(|options| options.theme_preference = ThemePreference::System);

    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();

        let spacing = &mut style.spacing;
        spacing.item_spacing = egui::vec2(8.0, 8.0);
        spacing.button_padding = egui::vec2(14.0, 7.0);
        spacing.interact_size.y = 32.0;
        spacing.window_margin = Margin::same(16);
        spacing.menu_margin = Margin::same(8);
        spacing.icon_width = 18.0;
        spacing.icon_width_inner = 10.0;
        spacing.text_edit_width = 260.0;

        style.animation_time = 0.12;
    });

    ctx.style_mut_of(Theme::Dark, |style| style.visuals = visuals(true));
    ctx.style_mut_of(Theme::Light, |style| style.visuals = visuals(false));
}

fn visuals(dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let radius = CornerRadius::same(RADIUS);
    let small = CornerRadius::same(8);

    if dark {
        v.panel_fill = Color32::from_rgb(0x16, 0x18, 0x1d);
        v.window_fill = Color32::from_rgb(0x1e, 0x21, 0x28);
        v.faint_bg_color = Color32::from_rgb(0x1e, 0x21, 0x28);
        v.extreme_bg_color = Color32::from_rgb(0x10, 0x12, 0x16);
        v.code_bg_color = Color32::from_rgb(0x10, 0x12, 0x16);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x2a, 0x2e, 0x37));
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x26, 0x2a, 0x33);
        v.widgets.inactive.bg_fill = Color32::from_rgb(0x2a, 0x2e, 0x38);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x30, 0x35, 0x40);
        v.widgets.hovered.bg_fill = Color32::from_rgb(0x34, 0x39, 0x45);
        v.widgets.active.weak_bg_fill = Color32::from_rgb(0x3a, 0x40, 0x4c);
    } else {
        v.panel_fill = Color32::from_rgb(0xf4, 0xf5, 0xf8);
        v.window_fill = Color32::WHITE;
        v.faint_bg_color = Color32::WHITE;
        v.extreme_bg_color = Color32::from_rgb(0xfb, 0xfb, 0xfd);
        v.code_bg_color = Color32::from_rgb(0xee, 0xf0, 0xf4);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xe2, 0xe5, 0xeb));
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xea, 0xec, 0xf1);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xdf, 0xe3, 0xea);
        v.widgets.active.weak_bg_fill = Color32::from_rgb(0xd4, 0xd9, 0xe2);
    }

    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = small;
        widget.expansion = 0.0;
    }
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.6));
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);

    v.selection.bg_fill = ACCENT.gamma_multiply(if dark { 0.55 } else { 0.35 });
    v.selection.stroke = Stroke::new(1.0, if dark { Color32::WHITE } else { ACCENT });
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARNING;
    v.error_fg_color = DANGER;
    v.window_corner_radius = radius;
    v.menu_corner_radius = radius;
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 30 }),
    };
    v.window_stroke = v.widgets.noninteractive.bg_stroke;
    v.striped = true;
    v
}

/// Rounded surface used for every content block.
pub fn card(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::new()
        .fill(visuals.faint_bg_color)
        .stroke(visuals.widgets.noninteractive.bg_stroke)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::same(16))
}

pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong())
        .fill(ACCENT)
        .min_size(egui::vec2(0.0, 36.0))
}

pub fn danger_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong())
        .fill(DANGER)
        .min_size(egui::vec2(0.0, 36.0))
}

/// iOS-style switch; returns a response that is `changed()` when flipped.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, enabled: bool) -> egui::Response {
    let size = egui::vec2(42.0, 24.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, mut response) = ui.allocate_exact_size(size, sense);
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let off_fill = ui.visuals().widgets.inactive.bg_fill;
        let mut fill = Color32::from_rgba_unmultiplied(
            lerp_u8(off_fill.r(), ACCENT.r(), t),
            lerp_u8(off_fill.g(), ACCENT.g(), t),
            lerp_u8(off_fill.b(), ACCENT.b(), t),
            255,
        );
        if !enabled {
            fill = fill.gamma_multiply(0.5);
        }
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, fill);
        let knob_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        ui.painter()
            .circle_filled(egui::pos2(knob_x, rect.center().y), radius - 3.0, Color32::WHITE);
    }
    response
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}
