use eframe::egui::{self, Color32, Vec2};

pub(super) const BG: Color32 = Color32::from_rgb(7, 24, 39);
pub(super) const PANEL: Color32 = Color32::from_rgb(7, 19, 32);
pub(super) const CARD: Color32 = Color32::from_rgb(10, 27, 44);
pub(super) const MUTED: Color32 = Color32::from_rgb(83, 143, 176);
pub(super) const TEXT: Color32 = Color32::from_rgb(220, 239, 250);
pub(super) const ACCENT: Color32 = Color32::from_rgb(49, 165, 235);
pub(super) const CYAN: Color32 = Color32::from_rgb(71, 190, 250);
pub(super) const GREEN: Color32 = Color32::from_rgb(87, 190, 145);
pub(super) const RED: Color32 = Color32::from_rgb(235, 105, 123);
pub(super) const WARNING: Color32 = Color32::from_rgb(231, 177, 83);

pub(super) fn apply(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.style_mut(|style| {
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = Color32::from_rgb(4, 12, 21);
        style.visuals.widgets.noninteractive.bg_fill = CARD;
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(13, 35, 53);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(18, 52, 75);
        style.visuals.widgets.active.bg_fill = ACCENT;
        style.visuals.selection.bg_fill = ACCENT;
        style.spacing.item_spacing = Vec2::new(9.0, 9.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.visuals.override_text_color = Some(TEXT);
    });
}
