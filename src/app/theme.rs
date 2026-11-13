use eframe::egui::{self, Color32, Vec2};

pub(super) const BG: Color32 = Color32::from_rgb(10, 25, 42);
pub(super) const PANEL: Color32 = Color32::from_rgb(7, 18, 31);
pub(super) const CARD: Color32 = Color32::from_rgb(13, 31, 49);
pub(super) const MUTED: Color32 = Color32::from_rgb(99, 151, 180);
pub(super) const TEXT: Color32 = Color32::from_rgb(216, 235, 248);
pub(super) const ACCENT: Color32 = Color32::from_rgb(54, 165, 234);
pub(super) const CYAN: Color32 = Color32::from_rgb(87, 190, 247);
pub(super) const GREEN: Color32 = Color32::from_rgb(87, 190, 145);
pub(super) const RED: Color32 = Color32::from_rgb(235, 105, 123);
pub(super) const WARNING: Color32 = Color32::from_rgb(231, 177, 83);

pub(super) fn apply(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.style_mut(|style| {
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = Color32::from_rgb(5, 13, 23);
        style.visuals.faint_bg_color = Color32::from_rgb(10, 25, 42);
        style.visuals.widgets.noninteractive.bg_fill = BG;
        style.visuals.widgets.inactive.bg_fill = CARD;
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(17, 45, 67);
        style.visuals.widgets.active.bg_fill = ACCENT;
        style.visuals.selection.bg_fill = ACCENT;
        style.spacing.item_spacing = Vec2::new(9.0, 8.0);
        style.spacing.button_padding = Vec2::new(11.0, 7.0);
        style.visuals.override_text_color = Some(TEXT);
    });
}
