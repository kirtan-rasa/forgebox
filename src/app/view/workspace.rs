use super::ForgeApp;
use super::super::theme::*;
use super::files::{file_glyph, reorder_item, reorder_item_to_index};
use super::icons::{glyph_text, icon_font};
use super::parameters::parameter_ui;
use super::widgets::{paint_check_mark, reorder_arrow_button, section_header};
use crate::catalog::{info, ToolId};
use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, Stroke, Vec2};

impl ForgeApp {
    pub(super) fn render_workspace(
        &mut self,
        ctx: &egui::Context,
        content_width: f32,
        compact_ui: bool,
        dragging_external_files: bool,
    ) {
        let page_margin = if content_width < 620.0 { 18.0 } else if content_width < 920.0 { 22.0 } else { 28.0 };
        egui::CentralPanel::default().frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(page_margin, if compact_ui { 10.0 } else { 14.0 }))).show(ctx, |ui| {
            // Keep title, stepper and navigation outside the scroll area. Only the active step content scrolls.
            // This prevents egui from restoring a stale scroll offset above the upload panel.
            ui.set_width(ui.available_width());
            let tool = info(self.selected_tool);
            // Compact, centered tool heading: constrain the card width as well as
            // centering its text. Use the standard UI font; hierarchy comes from size/weight.
            let full_header_width = ui.available_width();
            let header_width = full_header_width.min(if compact_ui { 520.0 } else { 840.0 });
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                egui::Frame::none()
                    .fill(Color32::from_rgb(8, 29, 46))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(19, 59, 81)))
                    .rounding(6.0)
                    .inner_margin(egui::Margin { left: 12.0, right: 12.0, top: 7.0, bottom: 7.0 })
                    .show(ui, |ui| {
                        ui.set_width((header_width - 24.0).max(1.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(tool.name)
                                    .size(if compact_ui { 20.0 } else { 23.0 })
                                    .strong()
                                    .color(TEXT),
                            );
                            ui.add_space(1.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(tool.description)
                                        .size(12.5)
                                        .color(Color32::from_rgb(169, 204, 224)),
                                )
                                .halign(Align::Center)
                                .wrap(),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("Requires: {}", tool.dependencies))
                                        .size(10.5)
                                        .color(MUTED),
                                )
                                .halign(Align::Center)
                                .wrap(),
                            );
                        });
                    });
            });
            ui.add_space(7.0);

            // One connected wizard bar. Tools without options expose only Files and Save.
            let visible_steps = self.wizard_step_sequence();
            let first_label = if self.selected_tool == ToolId::GenerateQr { "Content" } else { "Files" };
            egui::Frame::none()
                .fill(PANEL)
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(24, 76, 106)))
                .rounding(6.0)
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let full_width = ui.available_width();
                    let small_controls = compact_ui || full_width < 680.0;
                    // Preserve full step names on narrow layouts by placing navigation
                    // on a separate row instead of crushing the progress buttons.
                    let controls_width = if small_controls { 94.0 } else { 206.0 };
                    // Allocate the whole row, then position the stepper and navigation
                    // explicitly. No implicit horizontal spacing can push Back/Next inward.
                    let nav_gap = 16.0_f32;
                    let flow_width = (full_width - controls_width - nav_gap).max(0.0);
                    // Hide labels only when the stepper itself has too little room.
                    // This keeps numbered steps readable instead of squeezing their captions.
                    let show_step_labels = flow_width >= 340.0;
                    ui.allocate_ui_with_layout(Vec2::new(full_width, 40.0), Layout::left_to_right(Align::Center), |row_ui| {
                        row_ui.spacing_mut().item_spacing.x = 0.0;
                        let row_rect = row_ui.max_rect();
                        row_ui.allocate_ui_with_layout(Vec2::new(flow_width, 40.0), Layout::left_to_right(Align::Center), |ui| {
                            let spacing = ui.spacing().item_spacing.x;
                            let base_node_width = |step_id: usize| -> f32 {
                                match (step_id, small_controls) {
                                    (0, true) => 94.0,
                                    (1, true) => 112.0,
                                    (2, true) => 86.0,
                                    (0, false) => 100.0,
                                    (1, false) => 136.0,
                                    (2, false) => 144.0,
                                    _ => 100.0,
                                }
                            };
                            let arrow_count = visible_steps.len().saturating_sub(1);
                            let item_count = visible_steps.len() + arrow_count;
                            let total_spacing = spacing * item_count.saturating_sub(1) as f32;
                            // Reserve a little room for the arrows, then shrink step
                            // buttons proportionally when the available width is tight.
                            let base_total_node_width: f32 = visible_steps.iter().map(|step_id| base_node_width(*step_id)).sum();
                            let available_for_nodes = (ui.available_width() - total_spacing - arrow_count as f32 * 8.0).max(0.0);
                            let node_scale = if base_total_node_width > 0.0 {
                                (available_for_nodes / base_total_node_width).min(1.0)
                            } else {
                                1.0
                            };
                            let node_width = |step_id: usize| -> f32 { base_node_width(step_id) * node_scale };
                            let total_node_width: f32 = visible_steps.iter().map(|step_id| node_width(*step_id)).sum();
                            let arrow_width = if arrow_count > 0 {
                                ((ui.available_width() - total_node_width - total_spacing) / arrow_count as f32).max(0.0)
                            } else {
                                0.0
                            };
                            let current_index = visible_steps.iter().position(|step_id| *step_id == self.wizard_step).unwrap_or(0);

                            for (visible_index, step_id) in visible_steps.iter().copied().enumerate() {
                                let label = match step_id {
                                    0 => first_label,
                                    1 => "Options",
                                    _ => "Save",
                                };
                                let active = self.wizard_step == step_id;
                                let complete = current_index > visible_index;
                                let fill = if active { ACCENT } else if complete { Color32::from_rgb(20, 75, 75) } else { Color32::from_rgb(11, 31, 48) };
                                let edge = if active { ACCENT } else if complete { GREEN.linear_multiply(0.75) } else { Color32::from_rgb(30, 72, 96) };
                                // Keep completion marks independent of fonts; some systems render ✓ as a missing-glyph square.
                                let caption = if show_step_labels {
                                    if complete { label.to_owned() } else { format!("{}  {label}", visible_index + 1) }
                                } else {
                                    (visible_index + 1).to_string()
                                };
                                let step_response = ui.add_sized(
                                    [node_width(step_id), 34.0],
                                    egui::Button::new(RichText::new(caption).size(if small_controls { 11.5 } else { 13.0 }).strong().color(TEXT))
                                        .fill(fill)
                                        .stroke(Stroke::new(1.0_f32, edge)),
                                );
                                if complete && show_step_labels {
                                    paint_check_mark(
                                        ui.painter(),
                                        egui::pos2(step_response.rect.left() + 14.0, step_response.rect.center().y),
                                        1.7,
                                        TEXT,
                                    );
                                }
                                if visible_index + 1 < visible_steps.len() {
                                    let arrow_color = if current_index > visible_index { GREEN } else { Color32::from_rgb(45, 103, 134) };
                                    let (rect, _) = ui.allocate_exact_size(Vec2::new(arrow_width, 24.0), egui::Sense::hover());
                                    let y = rect.center().y;
                                    let start = egui::pos2(rect.left() + 2.0, y);
                                    let tip = egui::pos2(rect.right() - 4.0, y);
                                    ui.painter().line_segment([start, tip], Stroke::new(1.5_f32, arrow_color));
                                    ui.painter().line_segment([egui::pos2(tip.x - 6.0, y - 4.0), tip], Stroke::new(1.5_f32, arrow_color));
                                    ui.painter().line_segment([egui::pos2(tip.x - 6.0, y + 4.0), tip], Stroke::new(1.5_f32, arrow_color));
                                }
                            }
                        });
                        let divider_x = row_rect.left() + flow_width + nav_gap * 0.5;
                        row_ui.painter().line_segment(
                            [egui::pos2(divider_x, row_rect.center().y - 12.0),
                             egui::pos2(divider_x, row_rect.center().y + 12.0)],
                            Stroke::new(1.0_f32, Color32::from_rgb(24, 76, 106)),
                        );
                        row_ui.allocate_ui_with_layout(Vec2::new(controls_width, 40.0), Layout::right_to_left(Align::Center), |ui| {
                            let can_next = match self.wizard_step {
                                0 => self.can_advance_from_files(),
                                1 => self.selected_tool != ToolId::GenerateQr || !self.text.trim().is_empty(),
                                _ => false,
                            };
                            let current_index = visible_steps.iter().position(|step_id| *step_id == self.wizard_step).unwrap_or(0);
                            let back_enabled = current_index > 0 && !self.running;
                            let next_enabled = current_index + 1 < visible_steps.len() && can_next && !self.running;
                            if small_controls {
                                if ui.add_enabled(next_enabled, egui::Button::new("›").fill(if next_enabled { ACCENT } else { Color32::from_rgb(15, 35, 52) }).min_size(Vec2::new(36.0, 34.0))).on_hover_text("Next").clicked() {
                                    if let Some(next_step) = visible_steps.get(current_index + 1) { self.wizard_step = *next_step; }
                                }
                                if ui.add_enabled(back_enabled, egui::Button::new("‹").min_size(Vec2::new(36.0, 34.0))).on_hover_text("Back").clicked() {
                                    if current_index > 0 { self.wizard_step = visible_steps[current_index - 1]; }
                                }
                            } else {
                                if ui.add_enabled(next_enabled, egui::Button::new(RichText::new("Next  ›").size(13.0)).fill(if next_enabled { ACCENT } else { Color32::from_rgb(15, 35, 52) }).min_size(Vec2::new(88.0, 34.0))).clicked() {
                                    if let Some(next_step) = visible_steps.get(current_index + 1) { self.wizard_step = *next_step; }
                                }
                                if ui.add_enabled(back_enabled, egui::Button::new(RichText::new("‹  Back").size(13.0)).min_size(Vec2::new(88.0, 34.0))).clicked() {
                                    if current_index > 0 { self.wizard_step = visible_steps[current_index - 1]; }
                                }
                            }
                        });
                    });
                });
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt(("main-content-scroll-v3", self.selected_tool, self.wizard_step))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
            if self.dialog_open {
                ui.add_space(5.0);
                ui.label(RichText::new("Waiting for the system dialog…").size(11.0).color(MUTED));
            }
            if dragging_external_files && !self.running {
                ui.add_space(5.0);
                ui.label(RichText::new("Drop files to add them to the list").strong().color(CYAN));
            }
            ui.add_space(8.0);

            egui::Frame::none().fill(PANEL).stroke(Stroke::new(1.0_f32, Color32::from_rgb(24, 76, 106))).rounding(7.0).inner_margin(egui::Margin::same(15.0)).show(ui, |ui| {
                // Keep the configuration card aligned with the full content panel rather than
                // sizing it to whichever control happens to be the narrowest.
                ui.set_width(ui.available_width());
                match self.wizard_step {
                    0 => {
                        if self.selected_tool == ToolId::GenerateQr {
                            section_header(ui, "01", "Generate a QR code", "Enter the input on the next step");
                            ui.add_space(12.0);
                            ui.label(RichText::new("This tool creates a PNG from text or a URL and does not require input files.").size(13.0).color(MUTED));
                            ui.add_space(8.0);
                            if ui.button("Go to options →").clicked() { self.wizard_step = 1; }
                        } else {
                        section_header(ui, "01", "Select files", "Select multiple files or drag and drop them here");
                        ui.add_space(14.0);
                        ui.horizontal_wrapped(|ui| {
                            if ui.add_enabled(!self.running && !self.dialog_open, egui::Button::new(RichText::new(format!("{}  Choose files", glyph_text(self.icons_loaded, "󰈔"))).font(icon_font(14.0)).strong()).fill(ACCENT).min_size(Vec2::new(160.0, 40.0))).clicked() { self.choose_files(); }
                            if ui.add_enabled(!self.running && !self.files.is_empty(), egui::Button::new("Clear list")).clicked() { self.files.clear(); self.secondary_file = None; self.selection_notice = None; self.dragging_file = None; self.drag_insert_index = None; self.drag_card_size = None; }
                        });
                        ui.add_space(12.0);
                        // Persistent drop target: keep it visible even after files have been added.
                        let drop = egui::Frame::none().fill(if dragging_external_files { Color32::from_rgb(10, 39, 55) } else { Color32::from_rgb(8, 24, 39) }).stroke(Stroke::new(1.0_f32, if dragging_external_files { CYAN } else { Color32::from_rgb(26, 80, 112) })).rounding(6.0).inner_margin(egui::Margin::same(16.0)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.set_min_height(if self.files.is_empty() { 142.0 } else { 76.0 });
                            ui.vertical_centered(|ui| {
                                ui.add_space(if self.files.is_empty() { 10.0 } else { 0.0 });
                                ui.label(RichText::new(glyph_text(self.icons_loaded, "󰇘")).font(icon_font(if self.files.is_empty() { 28.0 } else { 20.0 })).color(CYAN));
                                ui.label(RichText::new(if dragging_external_files { "Drop files to add them" } else { "Drag files here to add them" }).size(if self.files.is_empty() { 15.0 } else { 13.0 }).strong());
                                if self.files.is_empty() { ui.label(RichText::new("or click “Choose files” above").size(12.0).color(MUTED)); }
                            });
                        });
                        let _ = drop;
                        ui.add_space(8.0);
                        if let Some(notice) = &self.selection_notice {
                            ui.label(RichText::new(notice).size(11.5).color(CYAN));
                        }
                        ui.add_space(8.0);
                        if !self.files.is_empty() {
                            let mut remove_index = None;
                            let mut reorder_request: Option<(usize, usize)> = None;
                            if self.selected_tool == ToolId::ImagesToPdf {
                                ui.label(RichText::new("PDF pages · drag a card from anywhere except its buttons").size(12.0).color(MUTED));
                                ui.add_space(10.0);
                                let available = ui.available_width().max(190.0);
                                let cols = if available >= 2200.0 { 5usize }
                                    else if available >= 1400.0 { 4usize }
                                    else if available >= 1040.0 { 3usize }
                                    else if available >= 420.0 { 2usize }
                                    else { 1usize };
                                let gap = 10.0;
                                let card_width = ((available - gap * (cols.saturating_sub(1) as f32)) / cols as f32).clamp(190.0, 480.0);
                                let inner_width = (card_width - 22.0).max(168.0);
                                let preview_height = (inner_width * 0.78).clamp(160.0, 270.0);
                                // Keep source data unchanged while dragging; render a projected order
                                // with a real placeholder slot. The card body is draggable except for
                                // the separate functional-control row.
                                let active_drag_path = self.dragging_file.clone()
                                    .filter(|path| self.files.iter().any(|item| item == path));
                                let remaining_files: Vec<PathBuf> = self.files.iter()
                                    .filter(|path| active_drag_path.as_ref() != Some(*path))
                                    .cloned()
                                    .collect();
                                let default_insert_index = active_drag_path.as_ref()
                                    .and_then(|path| self.files.iter().position(|item| item == path))
                                    .unwrap_or(remaining_files.len())
                                    .min(remaining_files.len());
                                let insert_index = if active_drag_path.is_some() {
                                    self.drag_insert_index.unwrap_or(default_insert_index).min(remaining_files.len())
                                } else {
                                    0
                                };
                                let mut layout_items: Vec<Option<PathBuf>> = Vec::with_capacity(self.files.len());
                                if active_drag_path.is_some() {
                                    for slot in 0..=remaining_files.len() {
                                        if slot == insert_index {
                                            layout_items.push(None);
                                        }
                                        if let Some(path) = remaining_files.get(slot) {
                                            layout_items.push(Some(path.clone()));
                                        }
                                    }
                                } else {
                                    layout_items.extend(self.files.iter().cloned().map(Some));
                                }

                                // Geometry collected from the actual layout is used for hit-testing.
                                // Unlike egui's floating dnd_drag_source response, these rects always
                                // belong to the visible cards in the grid.
                                let mut rendered_cards: Vec<(PathBuf, egui::Rect)> = Vec::new();
                                let mut insertion_slot_rect: Option<(usize, egui::Rect)> = None;
                                let mut drag_started: Option<PathBuf> = None;
                                {
                                    let mut index = 0usize;
                                    while index < layout_items.len() {
                                        ui.horizontal_top(|ui| {
                                            for _ in 0..cols {
                                                if index >= layout_items.len() { break; }
                                                let visual_index = index;
                                                match layout_items[index].clone() {
                                                    None => {
                                                        if active_drag_path.is_some() {
                                                            let outer_size = self.drag_card_size.unwrap_or(Vec2::new(card_width, preview_height + 140.0));
                                                            let (_, slot_rect) = ui.allocate_space(outer_size);
                                                            ui.painter().rect_filled(
                                                                slot_rect.shrink(1.0),
                                                                12.0,
                                                                Color32::from_rgb(11, 31, 49),
                                                            );
                                                            ui.painter().rect_stroke(
                                                                slot_rect.shrink(1.0),
                                                                12.0,
                                                                Stroke::new(1.6_f32, ACCENT),
                                                            );
                                                            ui.painter().text(
                                                                slot_rect.center(),
                                                                egui::Align2::CENTER_CENTER,
                                                                "Drop here",
                                                                FontId::proportional(14.0),
                                                                CYAN,
                                                            );
                                                            insertion_slot_rect = Some((insert_index, slot_rect));
                                                        }
                                                    }
                                                    Some(path) => {
                                                        let current = self.files.iter().position(|item| item == &path).unwrap_or(visual_index);
                                                        let card_frame = egui::Frame::none()
                                                            .fill(Color32::from_rgb(8, 24, 39))
                                                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(24, 76, 106)))
                                                            .rounding(6.0)
                                                            .inner_margin(egui::Margin::same(10.0));
                                                        let mut controls_rect: Option<egui::Rect> = None;
                                                        let card_response = card_frame.show(ui, |ui| {
                                                            ui.set_width(inner_width);
                                                            ui.vertical_centered(|ui| {
                                                                egui::Frame::none()
                                                                    .fill(Color32::from_rgb(5, 16, 28))
                                                                    .rounding(8.0)
                                                                    .inner_margin(egui::Margin::same(4.0))
                                                                    .show(ui, |ui| {
                                                                        ui.allocate_ui_with_layout(
                                                                            Vec2::new(inner_width - 8.0, preview_height),
                                                                            Layout::centered_and_justified(egui::Direction::TopDown),
                                                                            |ui| {
                                                                                if let Some(texture) = self.thumbnails.get(&path) {
                                                                                    ui.add(egui::Image::new(texture).max_size(Vec2::new(inner_width - 16.0, preview_height - 8.0)).fit_to_fraction(Vec2::splat(1.0)));
                                                                                } else if self.thumbnail_pending.contains(&path) {
                                                                                    ui.spinner();
                                                                                    ui.label(RichText::new("Loading preview…").size(12.0).color(MUTED));
                                                                                } else if self.thumbnail_errors.contains(&path) {
                                                                                    ui.label(RichText::new("Preview unavailable").size(12.0).color(MUTED));
                                                                                } else {
                                                                                    ui.spinner();
                                                                                }
                                                                            },
                                                                        );
                                                                    });
                                                                ui.add_space(8.0);
                                                                ui.horizontal(|ui| {
                                                                    // Decorative grip remains a hover target, while the full non-button
                                                                    // card body receives the drag interaction after layout is complete.
                                                                    let handle_sense = egui::Sense::hover();
                                                                    let (handle_rect, handle_response) = ui.allocate_exact_size(Vec2::new(34.0, 34.0), handle_sense);
                                                                    let handle_fill = if handle_response.hovered() { Color32::from_rgb(20, 55, 78) } else { Color32::from_rgb(10, 29, 46) };
                                                                    ui.painter().rect_filled(handle_rect, 6.0, handle_fill);
                                                                    // Six-dot grip, drawn directly so it works with any configured font.
                                                                    for row in 0..3 {
                                                                        for col in 0..2 {
                                                                            ui.painter().circle_filled(
                                                                                egui::pos2(handle_rect.center().x + (col as f32 - 0.5) * 6.0, handle_rect.center().y + (row as f32 - 1.0) * 6.0),
                                                                                1.5,
                                                                                if handle_response.hovered() { CYAN } else { MUTED },
                                                                            );
                                                                        }
                                                                    }
                                                                    let _ = handle_response.on_hover_text("Drag the card from anywhere except its buttons");
                                                                    ui.vertical(|ui| {
                                                                        ui.label(RichText::new(format!("Page {}", visual_index + 1)).strong().size(14.0));
                                                                        ui.add(egui::Label::new(RichText::new(path.file_name().and_then(|s| s.to_str()).unwrap_or("Image")).size(12.0).color(MUTED)).truncate())
                                                                            .on_hover_text(path.display().to_string());
                                                                    });
                                                                });
                                                                ui.add_space(6.0);
                                                                let controls_row = ui.horizontal(|ui| {
                                                                    if reorder_arrow_button(ui, current > 0 && !self.running && self.dragging_file.is_none(), true, "Move page up") {
                                                                        reorder_request = Some((current, current.saturating_sub(1)));
                                                                    }
                                                                    if ui.add_enabled(!self.running && self.dragging_file.is_none(), egui::Button::new("Remove").min_size(Vec2::new((inner_width - 104.0).max(64.0), 34.0))).clicked() { remove_index = Some(current); }
                                                                    if reorder_arrow_button(ui, current + 1 < self.files.len() && !self.running && self.dragging_file.is_none(), false, "Move page down") {
                                                                        reorder_request = Some((current, current + 2));
                                                                    }
                                                                });
                                                                controls_rect = Some(controls_row.response.rect);
                                                            });
                                                        });
                                                        let card_rect = card_response.response.rect;
                                                        // Make nearly the entire card draggable, excluding the functional
                                                        // button row. This avoids a global overlay over the buttons while
                                                        // letting image, filename, page number, grip, and blank card space
                                                        // all act as the drag source.
                                                        let drag_bottom = controls_rect
                                                            .map(|rect| rect.min.y - 2.0)
                                                            .unwrap_or(card_rect.max.y - 42.0)
                                                            .clamp(card_rect.min.y, card_rect.max.y);
                                                        let drag_rect = egui::Rect::from_min_max(
                                                            card_rect.min,
                                                            egui::pos2(card_rect.max.x, drag_bottom),
                                                        );
                                                        let drag_sense = if !self.running && self.dragging_file.is_none() {
                                                            egui::Sense::click_and_drag()
                                                        } else {
                                                            egui::Sense::hover()
                                                        };
                                                        let drag_response = ui.interact(
                                                            drag_rect,
                                                            ui.id().with(("image-page-drag-zone", path.clone())),
                                                            drag_sense,
                                                        );
                                                        if drag_response.drag_started() && drag_started.is_none() {
                                                            drag_started = Some(path.clone());
                                                        }
                                                        rendered_cards.push((path.clone(), card_rect));
                                                    }
                                                }
                                                index += 1;
                                            }
                                        });
                                        ui.add_space(10.0);
                                    }
                                }

                                // Start the custom drag only after the layout pass, so all widgets in this
                                // frame use one consistent order and one consistent set of hit rectangles.
                                if active_drag_path.is_none() {
                                    if let Some(path) = drag_started {
                                        if let Some((_, rect)) = rendered_cards.iter().find(|(item, _)| item == &path) {
                                            self.drag_card_size = Some(rect.size());
                                        }
                                        let from = self.files.iter().position(|item| item == &path).unwrap_or(0);
                                        self.drag_insert_index = Some(from.min(self.files.len().saturating_sub(1)));
                                        self.dragging_file = Some(path);
                                        ctx.request_repaint();
                                    }
                                }

                                // Pick the insertion slot from card rectangles, not egui's drag-source
                                // responses. The preview is paint-only and therefore cannot block this test.
                                if active_drag_path.is_some() {
                                    if let (Some(pointer), true) = (
                                        ui.input(|i| i.pointer.interact_pos()),
                                        ui.input(|i| i.pointer.primary_down()),
                                    ) {
                                        let mut next_insert = None;
                                        if let Some((slot_index, slot_rect)) = insertion_slot_rect {
                                            if slot_rect.contains(pointer) {
                                                next_insert = Some(slot_index);
                                            }
                                        }
                                        if next_insert.is_none() {
                                            if let Some((target_path, target_rect)) = rendered_cards.iter().find(|(_, rect)| rect.contains(pointer)) {
                                                if let Some(target_position) = remaining_files.iter().position(|item| item == target_path) {
                                                    let after = if cols == 1 {
                                                        pointer.y >= target_rect.center().y
                                                    } else {
                                                        pointer.x >= target_rect.center().x
                                                    };
                                                    next_insert = Some((target_position + usize::from(after)).min(remaining_files.len()));
                                                }
                                            }
                                        }
                                        if let Some(next_insert) = next_insert {
                                            if self.drag_insert_index != Some(next_insert) {
                                                self.drag_insert_index = Some(next_insert);
                                                ctx.request_repaint();
                                            }
                                        }
                                    }
                                }

                                // Draw a compact, non-interactive preview. Painting cannot intercept pointer hits.
                                if let (Some(path), Some(pointer)) = (
                                    active_drag_path.as_ref(),
                                    ctx.input(|i| i.pointer.interact_pos()),
                                ) {
                                    let outer_size = self.drag_card_size.unwrap_or(Vec2::new(card_width, preview_height + 140.0));
                                    let preview_scale = 0.58;
                                    let preview_size = outer_size * preview_scale;
                                    let preview_rect = egui::Rect::from_min_size(pointer + Vec2::new(14.0, 14.0), preview_size);
                                    let painter = ctx.layer_painter(egui::LayerId::new(
                                        egui::Order::Tooltip,
                                        egui::Id::new("forgebox-image-page-drag-preview"),
                                    ));
                                    painter.rect_filled(preview_rect, 12.0, Color32::from_rgba_unmultiplied(8, 24, 39, 242));
                                    painter.rect_stroke(preview_rect, 12.0, Stroke::new(1.6_f32, ACCENT));
                                    let thumb_rect = egui::Rect::from_min_size(
                                        preview_rect.min + Vec2::new(9.0, 9.0),
                                        Vec2::new((preview_rect.width() - 18.0).max(1.0), (preview_height * preview_scale).min(preview_rect.height() * 0.66).max(1.0)),
                                    );
                                    painter.rect_filled(thumb_rect, 8.0, Color32::from_rgb(5, 16, 28));
                                    if let Some(texture) = self.thumbnails.get(path) {
                                        let texture_size = texture.size_vec2();
                                        if texture_size.x > 0.0 && texture_size.y > 0.0 {
                                            let scale = ((thumb_rect.width() - 12.0).max(1.0) / texture_size.x)
                                                .min((thumb_rect.height() - 12.0).max(1.0) / texture_size.y);
                                            let image_size = texture_size * scale;
                                            let image_rect = egui::Rect::from_center_size(thumb_rect.center(), image_size);
                                            painter.image(
                                                texture.id(),
                                                image_rect,
                                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                                Color32::from_white_alpha(235),
                                            );
                                        }
                                    }
                                    let dragged_page = self.files.iter().position(|item| item == path).unwrap_or(0);
                                    painter.text(
                                        egui::pos2(preview_rect.center().x, thumb_rect.bottom() + 17.0),
                                        egui::Align2::CENTER_CENTER,
                                        format!("Page {}", dragged_page + 1),
                                        FontId::proportional(13.0),
                                        TEXT,
                                    );
                                    let raw_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("Image");
                                    let file_name = if raw_name.chars().count() > 32 {
                                        format!("{}…", raw_name.chars().take(31).collect::<String>())
                                    } else {
                                        raw_name.to_owned()
                                    };
                                    painter.text(
                                        egui::pos2(preview_rect.center().x, thumb_rect.bottom() + 36.0),
                                        egui::Align2::CENTER_CENTER,
                                        file_name,
                                        FontId::proportional(11.5),
                                        MUTED,
                                    );
                                }

                                // Commit only when the pointer is released. During the drag, the cards
                                // above are a projected ordering; the actual paths remain untouched.
                                if self.dragging_file.is_some() {
                                    let cancel_drag = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                    let pointer_released = !ui.input(|i| i.pointer.primary_down());
                                    if cancel_drag || pointer_released {
                                        if !cancel_drag {
                                            if let Some(dragged_path) = self.dragging_file.clone() {
                                                if let Some(from) = self.files.iter().position(|path| path == &dragged_path) {
                                                    let target = self.drag_insert_index.unwrap_or(from).min(self.files.len().saturating_sub(1));
                                                    reorder_item_to_index(&mut self.files, from, target);
                                                }
                                            }
                                        }
                                        self.dragging_file = None;
                                        self.drag_insert_index = None;
                                        self.drag_card_size = None;
                                        ctx.request_repaint();
                                    }
                                }
                            } else {
                                // Keep one vertical scroller for the whole page; nested lists steal wheel events.
                                for (index, path) in self.files.iter().enumerate() {
                                        egui::Frame::none().fill(Color32::from_rgb(8, 24, 39)).rounding(8.0).inner_margin(egui::Margin::symmetric(11.0, 8.0)).show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(glyph_text(self.icons_loaded, file_glyph(path))).font(icon_font(17.0)).color(CYAN));
                                                ui.vertical(|ui| {
                                                    ui.add(egui::Label::new(RichText::new(path.file_name().and_then(|s| s.to_str()).unwrap_or("File")).size(13.0).strong()).truncate())
                                                        .on_hover_text(path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default());
                                                    ui.add(egui::Label::new(RichText::new(path.parent().unwrap_or(path).display().to_string()).size(10.5).color(MUTED)).truncate())
                                                        .on_hover_text(path.display().to_string());
                                                });
                                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                    if ui.add_enabled(!self.running, egui::Button::new("×")).on_hover_text("Remove file from list").clicked() { remove_index = Some(index); }
                                                    if self.selected_tool == ToolId::PdfMerge {
                                                        if reorder_arrow_button(ui, index + 1 < self.files.len() && !self.running, false, "Move PDF down") { reorder_request = Some((index, index + 2)); }
                                                        if reorder_arrow_button(ui, index > 0 && !self.running, true, "Move PDF up") { reorder_request = Some((index, index - 1)); }
                                                    }
                                                });
                                            });
                                        });
                                        ui.add_space(5.0);
                                }
                            }
                            if let Some((from, insertion_index)) = reorder_request {
                                if !self.running { reorder_item(&mut self.files, from, insertion_index); }
                            }
                            if let Some(index) = remove_index { if index < self.files.len() && !self.running { self.files.remove(index); } }
                            ui.add_space(5.0);
                            ui.label(RichText::new(format!("Files selected: {}", self.files.len())).size(12.0).color(MUTED));
                            if self.selected_tool == ToolId::PdfWatermark {
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.label("Watermark PDF:");
                                    if ui.add_enabled(!self.dialog_open && !self.running, egui::Button::new("Browse…")).clicked() { self.choose_secondary_file(); }
                                });
                                if let Some(path) = &self.secondary_file { ui.label(RichText::new(path.display().to_string()).size(11.0).color(CYAN)); }
                            }
                        }
                        }
                    }
                    1 => {
                        section_header(ui, "02", "Configure task", "Only options relevant to the selected tool");
                        ui.add_space(8.0);
                        ui.add_enabled_ui(!self.running, |ui| parameter_ui(ui, self));
                        ui.add_space(8.0);
                        if self.running { ui.label(RichText::new("Settings are locked while processing.").size(11.0).color(MUTED)); }
                    }
                    _ => {
                        let save_step_number = visible_steps.iter().position(|step_id| *step_id == 2)
                            .map(|visible_index| visible_index + 1)
                            .unwrap_or(visible_steps.len());
                        section_header(ui, &format!("{save_step_number:02}"), "Save output", "Choose an output folder, then start processing");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Output folder").strong().size(13.0));
                        ui.add_space(7.0);
                        egui::Frame::none().fill(Color32::from_rgb(8, 24, 39)).rounding(5.0).inner_margin(egui::Margin::same(12.0)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(glyph_text(self.icons_loaded, "󰉋")).font(icon_font(17.0)).color(CYAN));
                                ui.add(egui::Label::new(RichText::new(self.output_dir.display().to_string()).size(13.0)).truncate());
                            });
                        });
                        ui.add_space(8.0);
                        if ui.add_enabled(!self.dialog_open && !self.running, egui::Button::new("Choose folder…").min_size(Vec2::new(150.0, 36.0))).clicked() {
                            self.choose_output_folder();
                        }
                        ui.add_space(10.0);
                        let can_run = self.can_run();
                        if !self.running && ui.add_enabled(can_run, egui::Button::new(RichText::new("▶  Start conversion").size(15.0).strong()).fill(if can_run { ACCENT } else { Color32::from_rgb(15, 35, 52) }).min_size(Vec2::new(ui.available_width(), 46.0))).clicked() {
                            self.start_job();
                        }
                        if !self.running && !can_run {
                            let reason = match self.selected_tool {
                                ToolId::GenerateQr => "Enter text or a URL in the Options step first.",
                                ToolId::PdfMerge if self.files.len() < 2 => "Select at least two PDF files to merge.",
                                ToolId::PdfWatermark if self.secondary_file.is_none() => "Select the watermark PDF in the first step.",
                                _ => "Add at least one input file in the first step.",
                            };
                            ui.add_space(4.0);
                            ui.label(RichText::new(reason).size(11.5).color(MUTED));
                        }

                        // One live status area for running, success or error; never duplicate status below the card.
                        if self.running || !self.logs.is_empty() {
                            ui.add_space(12.0);
                            let has_error = self.logs.iter().any(|(ok, _)| !ok);
                            let has_warning = self.logs.iter().any(|(ok, message)| *ok && message.starts_with("Warning:"));
                            let success_count = self.logs.iter().filter(|(ok, message)| *ok && message.starts_with("Done:")).count();
                            let (status_fill, status_edge) = if self.running {
                                (Color32::from_rgb(19, 35, 48), Color32::from_rgb(35, 119, 160))
                            } else if has_error {
                                (Color32::from_rgb(47, 23, 39), Color32::from_rgb(139, 60, 83))
                            } else if has_warning {
                                (Color32::from_rgb(48, 39, 23), Color32::from_rgb(139, 108, 51))
                            } else {
                                (Color32::from_rgb(11, 43, 43), Color32::from_rgb(37, 103, 87))
                            };
                            egui::Frame::none()
                                .fill(status_fill)
                                .stroke(Stroke::new(1.0_f32, status_edge))
                                .rounding(10.0)
                                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        if self.running {
                                            ui.spinner();
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(format!("Processing: {}", info(self.selected_tool).name)).strong().color(CYAN));
                                                ui.label(RichText::new("ForgeBox will show the result here when the task finishes.").size(11.5).color(MUTED));
                                            });
                                        } else if has_error {
                                            ui.label(RichText::new("✕").size(18.0).strong().color(RED));
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new("Conversion failed").strong().color(RED));
                                                for (_, message) in self.logs.iter().filter(|(ok, _)| !*ok) {
                                                    ui.add(egui::Label::new(RichText::new(message).size(12.0).color(TEXT).strong()).wrap());
                                                }
                                            });
                                        } else {
                                            if has_warning {
                                                let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 22.0), egui::Sense::hover());
                                                ui.painter().text(icon_rect.center(), egui::Align2::CENTER_CENTER, "!", FontId::proportional(18.0), WARNING);
                                            } else {
                                                let (check_rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 22.0), egui::Sense::hover());
                                                paint_check_mark(ui.painter(), check_rect.center(), 2.0, GREEN);
                                            }
                                            ui.vertical(|ui| {
                                                let summary = if has_warning {
                                                    format!("Done with warnings · outputs saved: {success_count}")
                                                } else if success_count > 0 {
                                                    format!("Done · outputs saved: {success_count}")
                                                } else {
                                                    self.logs.first().map(|(_, message)| message.clone()).unwrap_or_else(|| "Task completed".into())
                                                };
                                                ui.label(RichText::new(summary).strong().color(if has_warning { WARNING } else { GREEN }));
                                                if let Some((_, message)) = self.logs.iter().find(|(ok, message)| *ok && message.starts_with("Done:")) {
                                                    ui.add(egui::Label::new(RichText::new(message.trim_start_matches("Done:").trim()).size(11.5).color(MUTED)).truncate())
                                                        .on_hover_text(message.trim_start_matches("Done:").trim());
                                                }
                                                for (_, warning) in self.logs.iter().filter(|(ok, message)| *ok && message.starts_with("Warning:")) {
                                                    ui.add(egui::Label::new(RichText::new(warning).size(11.5).color(WARNING)).wrap());
                                                }
                                            });
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                if ui.button("Open folder").clicked() {
                                                    #[cfg(target_os = "linux")]
                                                    { let _ = std::process::Command::new("xdg-open").arg(&self.output_dir).spawn(); }
                                                    #[cfg(target_os = "macos")]
                                                    { let _ = std::process::Command::new("open").arg(&self.output_dir).spawn(); }
                                                }
                                            });
                                        }
                                    });
                                });
                        }
                    }
                }
            });

            });
        });
    }
}
