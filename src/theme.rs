use egui::{Color32, FontFamily, FontId, Rounding, Stroke, Style, TextStyle, Visuals};

/// Accent used for primary actions and focus rings.
pub const ACCENT: Color32 = Color32::from_rgb(94, 186, 167);

pub const BG_DEEP: Color32 = Color32::from_rgb(18, 20, 24);
pub const BG_PANEL: Color32 = Color32::from_rgb(26, 29, 36);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(34, 38, 48);
pub const BG_HOVER: Color32 = Color32::from_rgb(42, 48, 60);

pub const TEXT: Color32 = Color32::from_rgb(232, 236, 242);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 156, 172);
pub const TEXT_DIM: Color32 = Color32::from_rgb(100, 108, 124);

pub const BORDER: Color32 = Color32::from_rgb(48, 54, 68);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(40, 44, 56);

pub const DANGER: Color32 = Color32::from_rgb(232, 112, 112);

/// Apply a modern dark theme tuned for dense data-vis work.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.dark_mode = true;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = Color32::from_rgb(240, 180, 90);
    visuals.error_fg_color = DANGER;
    visuals.extreme_bg_color = BG_DEEP;
    visuals.faint_bg_color = BG_ELEVATED;
    visuals.code_bg_color = BG_DEEP;
    visuals.window_fill = BG_PANEL;
    visuals.panel_fill = BG_PANEL;
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 12.0,
        spread: 0.0,
        color: Color32::from_black_alpha(80),
    };
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(90),
    };
    visuals.window_rounding = Rounding::same(10.0);
    visuals.menu_rounding = Rounding::same(8.0);
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(94, 186, 167, 90);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    // Non-interactive widgets (labels, frames).
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = BG_ELEVATED;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
    visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

    // Idle interactive.
    visuals.widgets.inactive.bg_fill = BG_ELEVATED;
    visuals.widgets.inactive.weak_bg_fill = BG_ELEVATED;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.rounding = Rounding::same(7.0);

    // Hovered.
    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BG_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.rounding = Rounding::same(7.0);

    // Active / pressed.
    visuals.widgets.active.bg_fill = Color32::from_rgb(50, 58, 72);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(50, 58, 72);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.active.rounding = Rounding::same(7.0);

    // Open menus / comboboxes.
    visuals.widgets.open.bg_fill = BG_HOVER;
    visuals.widgets.open.weak_bg_fill = BG_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.rounding = Rounding::same(7.0);

    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;

    ctx.set_visuals(visuals);

    let mut style = Style {
        visuals: ctx.style().visuals.clone(),
        ..(*ctx.style()).clone()
    };
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    style.spacing.indent = 16.0;
    style.spacing.slider_width = 140.0;
    style.spacing.interact_size = egui::vec2(40.0, 22.0);
    style.spacing.scroll = egui::style::ScrollStyle::solid();

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );

    ctx.set_style(style);
}

/// Compact section label used in the sidebar.
pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(TEXT_DIM)
            .extra_letter_spacing(1.2),
    );
    ui.add_space(2.0);
}

/// Primary filled action button.
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(BG_DEEP).strong())
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(8.0))
            .min_size(egui::vec2(0.0, 32.0)),
    )
}

/// Secondary outline-style button.
pub fn secondary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(TEXT))
            .fill(BG_ELEVATED)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(Rounding::same(8.0))
            .min_size(egui::vec2(0.0, 32.0)),
    )
}

/// Subtle danger / remove button.
pub fn danger_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).small().color(DANGER))
            .fill(Color32::from_rgba_unmultiplied(232, 112, 112, 28))
            .stroke(Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(232, 112, 112, 80),
            ))
            .rounding(Rounding::same(6.0)),
    )
}
