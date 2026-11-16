use super::super::theme::*;
use super::icons::{glyph_text, icon_font};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};

pub(super) fn reorder_arrow_button(ui: &mut egui::Ui, enabled: bool, up: bool, tooltip: &str) -> bool {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(40.0, 34.0), sense);
    let fill = if !enabled {
        DISABLED_BG
    } else if response.is_pointer_button_down_on() {
        ACTIVE_BG
    } else if response.hovered() {
        HOVER_BG
    } else {
        CARD
    };
    ui.painter().rect_filled(rect, 5.0, fill);
    if response.hovered() && enabled {
        ui.painter().rect_stroke(rect, 5.0, Stroke::new(1.0_f32, ACCENT));
    }
    let center = rect.center();
    let color = if enabled { TEXT } else { MUTED };
    let (shaft_start, shaft_end, head_left, head_right, tip) = if up {
        (
            egui::pos2(center.x, center.y + 5.0),
            egui::pos2(center.x, center.y - 4.0),
            egui::pos2(center.x - 4.5, center.y),
            egui::pos2(center.x + 4.5, center.y),
            egui::pos2(center.x, center.y - 4.0),
        )
    } else {
        (
            egui::pos2(center.x, center.y - 5.0),
            egui::pos2(center.x, center.y + 4.0),
            egui::pos2(center.x - 4.5, center.y),
            egui::pos2(center.x + 4.5, center.y),
            egui::pos2(center.x, center.y + 4.0),
        )
    };
    let stroke = Stroke::new(1.8_f32, color);
    ui.painter().line_segment([shaft_start, shaft_end], stroke);
    ui.painter().line_segment([head_left, tip], stroke);
    ui.painter().line_segment([head_right, tip], stroke);
    response.on_hover_text(tooltip).clicked()
}

pub(super) fn paint_check_mark(painter: &egui::Painter, center: egui::Pos2, stroke_width: f32, color: Color32) {
    let start = center + Vec2::new(-5.0, 0.5);
    let joint = center + Vec2::new(-1.5, 4.0);
    let end = center + Vec2::new(5.5, -4.5);
    let stroke = Stroke::new(stroke_width, color);
    painter.line_segment([start, joint], stroke);
    painter.line_segment([joint, end], stroke);
}
pub(super) fn section_header(ui: &mut egui::Ui, number: &str, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([32.0, 28.0], egui::Label::new(RichText::new(number).font(FontId::monospace(11.0)).strong().color(CYAN)).selectable(false));
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(16.5).strong());
            ui.label(RichText::new(subtitle).size(12.0).color(MUTED));
        });
    });
}

pub(super) fn nav_button(ui: &mut egui::Ui, glyph: &str, label: &str, active: bool, icons_loaded: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 29.0), egui::Sense::click());
    if active {
        ui.painter().rect_filled(rect, 3.0, ACTIVE_BG);
        ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())), 1.0, ACCENT);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 3.0, HOVER_BG);
    }
    ui.painter().text(rect.left_center() + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER,
        glyph_text(icons_loaded, glyph), icon_font(13.0),
        if active { CYAN } else { MUTED });
    ui.painter().text(rect.left_center() + Vec2::new(34.0, 0.0), egui::Align2::LEFT_CENTER,
        label, egui::FontId::proportional(12.5),
        if active { TEXT } else { TEXT_SECONDARY });
    response.clicked()
}
