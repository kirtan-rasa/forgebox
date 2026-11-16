use eframe::egui::{self, Color32, Stroke, Vec2};

pub(super) const BG: Color32 = Color32::from_rgb(6, 22, 36);
pub(super) const PANEL: Color32 = Color32::from_rgb(7, 21, 35);
pub(super) const CARD: Color32 = Color32::from_rgb(10, 28, 45);
pub(super) const INPUT: Color32 = Color32::from_rgb(7, 25, 42);
pub(super) const PREVIEW: Color32 = Color32::from_rgb(4, 14, 24);

pub(super) const BORDER: Color32 = Color32::from_rgb(29, 88, 120);
pub(super) const BORDER_SUBTLE: Color32 = Color32::from_rgb(22, 60, 81);
pub(super) const BORDER_MUTED: Color32 = Color32::from_rgb(72, 100, 121);
pub(super) const ACTIVE_BG: Color32 = Color32::from_rgb(13, 39, 58);
pub(super) const HOVER_BG: Color32 = Color32::from_rgb(12, 35, 53);
pub(super) const DISABLED_BG: Color32 = Color32::from_rgb(11, 31, 48);

pub(super) const MUTED: Color32 = Color32::from_rgb(76, 142, 171);
pub(super) const TEXT_MUTED: Color32 = Color32::from_rgb(150, 179, 197);
pub(super) const TEXT_SECONDARY: Color32 = Color32::from_rgb(176, 201, 216);
pub(super) const TEXT: Color32 = Color32::from_rgb(210, 232, 246);
pub(super) const ACCENT: Color32 = Color32::from_rgb(49, 169, 224);
pub(super) const CYAN: Color32 = Color32::from_rgb(70, 190, 238);
pub(super) const CYAN_SOFT: Color32 = Color32::from_rgb(99, 207, 234);
pub(super) const GREEN: Color32 = Color32::from_rgb(87, 190, 145);
pub(super) const RED: Color32 = Color32::from_rgb(235, 105, 123);
pub(super) const WARNING: Color32 = Color32::from_rgb(231, 177, 83);

pub(super) const STATUS_RUNNING: Color32 = Color32::from_rgb(13, 31, 45);
pub(super) const STATUS_RUNNING_BORDER: Color32 = Color32::from_rgb(35, 119, 160);
pub(super) const STATUS_WARNING: Color32 = Color32::from_rgb(43, 35, 23);
pub(super) const STATUS_WARNING_BORDER: Color32 = Color32::from_rgb(139, 108, 51);
pub(super) const STATUS_ERROR: Color32 = Color32::from_rgb(43, 22, 36);
pub(super) const STATUS_ERROR_BORDER: Color32 = Color32::from_rgb(139, 60, 83);
pub(super) const STATUS_SUCCESS: Color32 = Color32::from_rgb(10, 38, 39);
pub(super) const STATUS_SUCCESS_BORDER: Color32 = Color32::from_rgb(37, 103, 87);

pub(super) fn apply(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.style_mut(|style| {
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = PREVIEW;
        style.visuals.faint_bg_color = CARD;
        style.visuals.code_bg_color = INPUT;
        style.visuals.hyperlink_color = CYAN;
        style.visuals.warn_fg_color = WARNING;
        style.visuals.error_fg_color = RED;
        style.visuals.window_rounding = egui::Rounding::same(8.0);
        style.visuals.menu_rounding = egui::Rounding::same(6.0);
        style.visuals.window_stroke = Stroke::new(1.0, BORDER);

        style.visuals.widgets.noninteractive.bg_fill = PANEL;
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
        style.visuals.widgets.noninteractive.fg_stroke.color = TEXT_SECONDARY;
        style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(5.0);

        style.visuals.widgets.inactive.bg_fill = INPUT;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        style.visuals.widgets.inactive.fg_stroke.color = TEXT;
        style.visuals.widgets.inactive.rounding = egui::Rounding::same(5.0);

        style.visuals.widgets.hovered.bg_fill = HOVER_BG;
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, CYAN);
        style.visuals.widgets.hovered.fg_stroke.color = TEXT;
        style.visuals.widgets.hovered.rounding = egui::Rounding::same(5.0);

        style.visuals.widgets.active.bg_fill = ACTIVE_BG;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        style.visuals.widgets.active.fg_stroke.color = TEXT;
        style.visuals.widgets.active.rounding = egui::Rounding::same(5.0);

        style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.42);
        style.visuals.selection.stroke = Stroke::new(1.0, CYAN);
        style.visuals.text_cursor = Stroke::new(1.5, CYAN);

        style.spacing.item_spacing = Vec2::new(9.0, 7.0);
        style.spacing.button_padding = Vec2::new(11.0, 7.0);
        style.spacing.interact_size = Vec2::new(40.0, 28.0);
        style.spacing.slider_width = 180.0;

        style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
        style.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        style.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        style.visuals.override_text_color = Some(TEXT);
    });
}
