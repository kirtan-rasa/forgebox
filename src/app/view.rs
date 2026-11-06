use super::ForgeApp;
use super::theme::*;
use crate::catalog::{info, Category, ToolId, TOOLS};
use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, Stroke, Vec2};
use image::ImageDecoder;
use std::path::{Path, PathBuf};

impl eframe::App for ForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_results();
        self.poll_dialogs();
        self.poll_thumbnails(ctx);
        self.queue_thumbnails();
        let dropped = ctx.input(|input| input.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>());
        if !dropped.is_empty() && !self.running {
            self.add_files(dropped);
            self.wizard_step = 0;
        }
        let dragging_external_files = ctx.input(|input| !input.raw.hovered_files.is_empty());

        let screen_width = ctx.screen_rect().width();
        let sidebar_width = if screen_width < 900.0 { 186.0 } else if screen_width < 1120.0 { 218.0 } else { 258.0 };
        // Responsive decisions must use the width that remains after the sidebar.
        // Using the full window width keeps the desktop layout active for a narrow
        // central panel, which can force the wizard controls beyond the right edge.
        let content_width = (screen_width - sidebar_width).max(0.0);
        let compact_ui = content_width < 760.0;
        egui::SidePanel::left("forge-sidebar")
            .exact_width(sidebar_width)
            .resizable(false)
            .frame(egui::Frame::none().fill(PANEL).inner_margin(egui::Margin::same(12.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([38.0, 38.0], egui::Label::new(RichText::new(glyph_text(self.icons_loaded, "󰉋")).font(icon_font(25.0)).strong().color(CYAN)).selectable(false));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("FORGEBOX").strong().size(18.0));
                        ui.label(RichText::new("BETA · FILE TOOLS").size(10.0).color(MUTED));
                    });
                });
                ui.add_space(12.0);
                ui.label(RichText::new("TOOLS").size(11.0).strong().color(MUTED));
                ui.add_space(5.0);
                if nav_button(ui, "󰍉", "All tools", self.active_category.is_none(), self.icons_loaded) { self.active_category = None; }
                for category in Category::ALL {
                    if nav_button(ui, category.glyph(), category.title(), self.active_category == Some(category), self.icons_loaded) {
                        self.active_category = Some(category);
                    }
                }
                ui.add_space(10.0);
                // Keep search to one compact row: explicitly allocate its bounds so
                // the text edit cannot expand into the tool list's layout area.
                let search_width = ui.available_width().max(80.0);
                let (search_rect, _) = ui.allocate_exact_size(
                    Vec2::new(search_width, 29.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(
                    search_rect,
                    5.0,
                    Color32::from_rgb(5, 17, 29),
                );
                ui.painter().rect_stroke(
                    search_rect,
                    5.0,
                    Stroke::new(1.0_f32, Color32::from_rgb(22, 69, 95)),
                );
                let search_content = search_rect.shrink2(Vec2::new(7.0, 2.0));
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(search_content), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.horizontal_centered(|ui| {
                        ui.add_sized(
                            [16.0, 22.0],
                            egui::Label::new(
                                RichText::new(glyph_text(self.icons_loaded, "󰍉"))
                                    .font(icon_font(13.0))
                                    .color(Color32::from_rgb(69, 145, 184)),
                            ).selectable(false),
                        );
                        let text_width = ui.available_width().max(24.0);
                        ui.add_sized(
                            [text_width, 22.0],
                            egui::TextEdit::singleline(&mut self.search)
                                .frame(false)
                                .margin(egui::Margin::symmetric(1.0, 3.0))
                                .hint_text(RichText::new("Search tools…").size(11.5).color(MUTED))
                                .desired_width(text_width),
                        );
                    });
                });
                ui.add_space(9.0);
                let tool_list_height = (ui.available_height() - 12.0).max(120.0);
                egui::ScrollArea::vertical().id_salt("tool-list").max_height(tool_list_height).auto_shrink([false, false]).show(ui, |ui| {
                    let query = self.search.to_lowercase();
                    let active_category = self.active_category;
                    let filtered: Vec<_> = TOOLS.iter().filter(|tool| {
                        active_category.map(|c| c == tool.category).unwrap_or(true)
                            && (query.is_empty() || tool.name.to_lowercase().contains(&query) || tool.subtitle.to_lowercase().contains(&query) || tool.description.to_lowercase().contains(&query))
                    }).collect();
                    if filtered.is_empty() {
                        ui.label(RichText::new("No tools found. Try another search or category.").size(11.5).color(MUTED));
                    }
                    for tool in filtered {
                        let selected = self.selected_tool == tool.id;
                        let fill = if selected { Color32::from_rgb(10, 39, 61) } else { PANEL };
                        let stroke = if selected { Stroke::new(1.0_f32, ACCENT) } else { Stroke::new(1.0_f32, Color32::TRANSPARENT) };
                        let response = egui::Frame::none().fill(fill).stroke(stroke).rounding(5.0).inner_margin(egui::Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.add_sized([25.0, 28.0], egui::Label::new(RichText::new(glyph_text(self.icons_loaded, tool.glyph)).font(icon_font(17.0)).color(if selected { CYAN } else { MUTED })).selectable(false));
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(tool.name).size(13.5).strong().color(if selected { TEXT } else { Color32::from_rgb(183, 220, 241) }));
                                    if !compact_ui { ui.label(RichText::new(tool.subtitle).size(10.5).color(MUTED)); }
                                });
                            });
                        }).response;
                        if response.interact(egui::Sense::click()).clicked() && !self.running && !self.dialog_open { self.select_tool(tool.id); }
                        ui.add_space(4.0);
                    }
                });
            });

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
        if self.running || self.dialog_open || !self.thumbnail_pending.is_empty() { ctx.request_repaint_after(std::time::Duration::from_millis(100)); }
    }
}

fn reorder_item<T>(items: &mut Vec<T>, from: usize, insertion_index: usize) {
    if from >= items.len() { return; }
    let item = items.remove(from);
    let target = if from < insertion_index { insertion_index.saturating_sub(1) } else { insertion_index };
    items.insert(target.min(items.len()), item);
}

fn reorder_item_to_index<T>(items: &mut Vec<T>, from: usize, target_index: usize) {
    if from >= items.len() { return; }
    let item = items.remove(from);
    items.insert(target_index.min(items.len()), item);
}

pub(super) fn load_thumbnail(path: &Path) -> Result<egui::ColorImage, String> {
    let mut decoded = (|| -> Result<image::DynamicImage, String> {
        let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
        let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
        let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
        let mut decoded = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        decoded.apply_orientation(orientation);
        Ok(decoded)
    })();

    // Formats supported by ImageMagick (for example HEIC) may not be enabled in
    // the image crate. Fall back to a temporary PNG while keeping work off the UI thread.
    if decoded.is_err() {
        let suffix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let temp = std::env::temp_dir().join(format!("forgebox-thumb-{}-{suffix}.png", std::process::id()));
        // Share the engine's cached ImageMagick probe instead of spawning a
        // separate --version process for every unsupported preview.
        let program = crate::engine::image_magick_program();
        let thread_limit = crate::engine::image_magick_thread_limit().to_string();
        let output = std::process::Command::new(program)
            .args(["-limit", "thread", thread_limit.as_str()])
            .arg(path).args(["-auto-orient", "-thumbnail", "600x420"])
            .arg(&temp).output();
        match output {
            Ok(result) if result.status.success() => {
                decoded = image::open(&temp).map_err(|e| e.to_string());
                let _ = std::fs::remove_file(&temp);
            }
            Ok(result) => {
                let _ = std::fs::remove_file(&temp);
                let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
                return Err(if detail.is_empty() { "Preview format is not supported".into() } else { detail });
            }
            Err(error) => { let _ = std::fs::remove_file(&temp); return Err(error.to_string()); }
        }
    }
    let thumb = decoded.map_err(|e| e.to_string())?.thumbnail(600, 420).to_rgba8();
    let size = [thumb.width() as usize, thumb.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, thumb.as_raw()))
}

// Draw reorder arrows into an explicitly allocated hit rectangle. This avoids tiny or
// ambiguous hit zones and leaves the controls independent from the drag handle.
fn reorder_arrow_button(ui: &mut egui::Ui, enabled: bool, up: bool, tooltip: &str) -> bool {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(40.0, 34.0), sense);
    let fill = if !enabled {
        Color32::from_rgb(13, 36, 54)
    } else if response.is_pointer_button_down_on() {
        Color32::from_rgb(23, 70, 99)
    } else if response.hovered() {
        Color32::from_rgb(17, 55, 80)
    } else {
        Color32::from_rgb(13, 41, 61)
    };
    ui.painter().rect_filled(rect, 5.0, fill);
    if response.hovered() && enabled {
        ui.painter().rect_stroke(rect, 5.0, Stroke::new(1.0_f32, ACCENT));
    }
    let center = rect.center();
    let color = if enabled { Color32::from_rgb(211, 236, 249) } else { Color32::from_rgb(74, 116, 141) };
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

fn paint_check_mark(painter: &egui::Painter, center: egui::Pos2, stroke_width: f32, color: Color32) {
    let start = center + Vec2::new(-5.0, 0.5);
    let joint = center + Vec2::new(-1.5, 4.0);
    let end = center + Vec2::new(5.5, -4.5);
    let stroke = Stroke::new(stroke_width, color);
    painter.line_segment([start, joint], stroke);
    painter.line_segment([joint, end], stroke);
}

fn icon_font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("forgebox-icons".into()))
}


fn collect_nerd_fonts(root: &PathBuf, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 { return; }
    let Ok(entries) = std::fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_nerd_fonts(&path, out, depth + 1);
        } else if path.file_name().and_then(|n| n.to_str()).map(|n| {
            let n = n.to_lowercase();
            n.contains("nerdfont") && (n.ends_with(".ttf") || n.ends_with(".otf"))
        }).unwrap_or(false) {
            out.push(path);
        }
    }
}

fn install_nerd_font(ctx: &egui::Context, bytes: Option<Vec<u8>>) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let loaded = if let Some(bytes) = bytes {
        fonts.font_data.insert("forgebox-nerd".to_owned(), egui::FontData::from_owned(bytes));
        let mut icon_family = vec!["forgebox-nerd".to_owned()];
        if let Some(proportional) = fonts.families.get(&egui::FontFamily::Proportional) {
            icon_family.extend(proportional.iter().cloned());
        }
        fonts.families.insert(egui::FontFamily::Name("forgebox-icons".into()), icon_family);
        true
    } else {
        // Use readable Unicode substitutes instead of displaying missing-glyph boxes.
        let fallback = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
        fonts.families.insert(egui::FontFamily::Name("forgebox-icons".into()), fallback);
        false
    };

    ctx.set_fonts(fonts);
    loaded
}

pub(super) fn load_nerd_font(ctx: &egui::Context) -> bool {
    // Fontconfig knows the active font directories. Prefer its direct answer and
    // avoid recursively scanning every system/user font folder on normal startup.
    if let Ok(output) = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "JetBrainsMono Nerd Font"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.to_lowercase().contains("nerd") {
                if let Ok(bytes) = std::fs::read(path) {
                    return install_nerd_font(ctx, Some(bytes));
                }
            }
        }
    }

    // Fallback for systems without Fontconfig or for fonts outside its database.
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/fonts/nerd-fonts/JetBrainsMonoNerdFont-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/JetBrainsMonoNerdFont-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/nerd-fonts/JetBrainsMonoNerdFont-Regular.ttf"),
    ];
    let home = std::env::var_os("HOME");

    if let Some(home) = home {
        let home = PathBuf::from(home);
        candidates.push(home.join(".local/share/fonts/JetBrainsMonoNerdFont-Regular.ttf"));
        candidates.push(home.join(".local/share/fonts/JetBrainsMono/JetBrainsMonoNerdFont-Regular.ttf"));
        collect_nerd_fonts(&home.join(".local/share/fonts"), &mut candidates, 0);
        collect_nerd_fonts(&home.join(".fonts"), &mut candidates, 0);
    }
    for root in ["/usr/share/fonts", "/usr/local/share/fonts"] {
        collect_nerd_fonts(&PathBuf::from(root), &mut candidates, 0);
    }

    install_nerd_font(ctx, candidates.iter().find_map(|path| std::fs::read(path).ok()))
}

fn parameter_ui(ui: &mut egui::Ui, app: &mut ForgeApp) {
    match app.selected_tool {
        ToolId::ImagesToPdf => {
            ui.label(RichText::new("One PDF, with a separate page for each image.").size(12.0));
            ui.label(RichText::new("Reorder pages by dragging files in the first step.").size(11.0).color(MUTED));
            ui.add_space(12.0);
            ui.checkbox(&mut app.compress_pdf, "Compress images inside the PDF");
            if app.compress_pdf {
                ui.label(RichText::new(format!("Image quality: {}%", app.quality)).size(12.0));
                let mut quality = app.quality.parse::<f32>().unwrap_or(82.0).clamp(1.0, 100.0);
                if ui.add(egui::Slider::new(&mut quality, 20.0..=100.0).integer()).changed() { app.quality = (quality as i32).to_string(); }
                ui.label(RichText::new("Lower quality produces a smaller PDF. For photos, 70–85% is usually a good range.").size(11.0).color(MUTED));
            }
        }
        ToolId::ImageConvert => {
            ui.label(RichText::new("Target format").size(12.0));
            format_combo(ui, &mut app.target_format, &["png", "jpg", "webp", "tiff", "heic"]);
        }
        ToolId::ImageResize => {
            ui.horizontal_wrapped(|ui| {
                labeled_text(ui, "Width", &mut app.width, "1600");
                labeled_text(ui, "Height", &mut app.height, "1200");
            });
            ui.label(RichText::new("Image aspect ratio is preserved.").size(11.0).color(MUTED));
        }
        ToolId::ImageCompress => {
            ui.label(RichText::new(format!("Quality: {}%", app.quality)).size(12.0));
            let mut quality = app.quality.parse::<f32>().unwrap_or(82.0).clamp(1.0, 100.0);
            if ui.add(egui::Slider::new(&mut quality, 1.0..=100.0).integer()).changed() { app.quality = (quality as i32).to_string(); }
            ui.label(RichText::new("For JPEG/WebP, lower values usually produce smaller files.").size(11.0).color(MUTED));
        }
        ToolId::ImageOcr => {
            ui.label(RichText::new("Tesseract language").size(12.0));
            ui.text_edit_singleline(&mut app.language);
            ui.label(RichText::new("Examples: eng, rus, eng+rus. The corresponding Tesseract language data must be installed.").size(11.0).color(MUTED));
        }
        ToolId::GenerateQr => {
            ui.label(RichText::new("Text or URL").size(12.0));
            ui.add(egui::TextEdit::multiline(&mut app.text).desired_rows(3).hint_text("https://example.com"));
        }
        ToolId::PdfRotate => {
            ui.label(RichText::new("Rotation angle").size(12.0));
            egui::ComboBox::from_id_salt("rotation").selected_text(format!("{}°", app.rotation)).show_ui(ui, |ui| {
                for angle in ["90", "180", "270"] { ui.selectable_value(&mut app.rotation, angle.to_string(), format!("{angle}°")); }
            });
        }
        ToolId::PdfCompress => {
            ui.label(RichText::new("Quality profile").size(12.0));
            egui::ComboBox::from_id_salt("pdf-compression-profile")
                .selected_text(match app.pdf_profile.as_str() {
                    "screen" => "Screen · smallest file",
                    "printer" => "Print · high quality",
                    "prepress" => "Prepress · maximum quality",
                    _ => "E-book · balanced size and quality",
                })
                .width(ui.available_width().min(360.0))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.pdf_profile, "screen".to_owned(), "Screen · compact");
                    ui.selectable_value(&mut app.pdf_profile, "ebook".to_owned(), "E-book · balanced");
                    ui.selectable_value(&mut app.pdf_profile, "printer".to_owned(), "Print · high quality");
                    ui.selectable_value(&mut app.pdf_profile, "prepress".to_owned(), "Prepress · maximum quality");
                });
            ui.label(RichText::new("The Screen profile compresses images more aggressively. Print and Prepress usually produce larger files.").size(11.0).color(MUTED));
        }
        ToolId::PdfSplit => {
            ui.label(RichText::new("Page range: start,end").size(12.0));
            ui.text_edit_singleline(&mut app.pages);
            ui.label(RichText::new("Creates one PDF per page. Use last to include through the final page, for example 1,last.").size(11.0).color(MUTED));
        }
        ToolId::PdfReorder => {
            ui.label(RichText::new("New page order").size(12.0));
            ui.text_edit_singleline(&mut app.pages);
            ui.label(RichText::new("Only listed pages are kept; omitted pages are removed. Example: 3,1,2,4.").size(11.0).color(MUTED));
        }
        ToolId::PdfToImages => {
            ui.label(RichText::new("Image format").size(12.0));
            format_combo(ui, &mut app.target_format, &["png", "jpg"]);
            ui.add_space(5.0);
            labeled_text(ui, "Resolution (DPI)", &mut app.dpi, "150");
        }
        ToolId::DataConvert => {
            ui.label(RichText::new("Output format").size(12.0));
            format_combo(ui, &mut app.target_format, &["csv", "json", "xlsx"]);
            ui.label(RichText::new("CSV → JSON treats all values as text. JSON input for CSV must be an array of objects.").size(11.0).color(MUTED));
        }
        ToolId::ExtractAudio => {
            ui.label(RichText::new("Audio format").size(12.0));
            format_combo(ui, &mut app.target_format, &["mp3", "wav", "aac", "ogg", "flac", "m4a"]);
        }
        ToolId::VideoConvert => {
            ui.label(RichText::new("Output format").size(12.0));
            format_combo(ui, &mut app.target_format, &["mp4", "webm", "mkv", "mov"]);
            ui.add_space(8.0);
            ui.label(RichText::new("Speed/size balance").size(12.0));
            egui::ComboBox::from_id_salt("video-preset")
                .selected_text(match app.video_preset.as_str() {
                    "balanced" => "Balanced quality and speed",
                    "smaller" => "Smaller file · slower",
                    _ => "Fast · best for low-end CPUs",
                })
                .width(ui.available_width().min(340.0))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.video_preset, "fast".to_owned(), "Fast · best for low-end CPUs");
                    ui.selectable_value(&mut app.video_preset, "balanced".to_owned(), "Balanced quality and speed");
                    ui.selectable_value(&mut app.video_preset, "smaller".to_owned(), "Smaller file · slower");
                });
            let mut quality = app.quality.parse::<f32>().unwrap_or(23.0).clamp(18.0, 30.0);
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Quality level: {}", quality as i32)).size(12.0)).on_hover_text("CRF setting: lower values produce better quality and larger files.");
            if ui.add(egui::Slider::new(&mut quality, 18.0..=30.0).integer().show_value(false)).changed() {
                app.quality = (quality as i32).to_string();
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("18 · higher quality, larger file").size(11.0).color(MUTED));
                ui.label(RichText::new("30 · smaller file, lower quality").size(11.0).color(MUTED));
            });
            ui.label(RichText::new("Default: fast mode and up to two threads to keep the system responsive.").size(11.0).color(MUTED));
            ui.label(RichText::new("Requires FFmpeg with H.264/AAC for MP4/MOV/MKV or VP9/Opus for WebM.").size(11.0).color(MUTED));
        }
        ToolId::VideoToGif => {
            ui.horizontal_wrapped(|ui| {
                labeled_text(ui, "Width", &mut app.width, "480");
                labeled_text(ui, "Frames per second", &mut app.fps, "12");
            });
            ui.label(RichText::new("A palette is generated automatically for cleaner colors. GIF works best for short clips.").size(11.0).color(MUTED));
        }
        ToolId::PdfWatermark => {
            ui.label(RichText::new("Select the main PDF on the left and a second PDF to use as a watermark.").size(12.0));
        }
        ToolId::ImageGrayscale | ToolId::RemoveBackground | ToolId::ExtractExif | ToolId::LongPngToPdf | ToolId::PdfMerge | ToolId::DocToPdf | ToolId::PdfToDoc => {
            ui.label(RichText::new("No additional options are required.").size(13.0));
            ui.label(RichText::new("Choose a file and an output folder, then start the conversion.").size(11.0).color(MUTED));
        }
    }
}

fn format_combo(ui: &mut egui::Ui, selected: &mut String, formats: &[&str]) {
    egui::ComboBox::from_id_salt("target-format").selected_text(selected.to_uppercase()).show_ui(ui, |ui| {
        for format in formats { ui.selectable_value(selected, (*format).to_string(), format.to_uppercase()); }
    });
}

fn labeled_text(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(11.5).color(MUTED));
        ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(95.0));
    });
}

fn section_header(ui: &mut egui::Ui, number: &str, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([32.0, 28.0], egui::Label::new(RichText::new(number).font(FontId::monospace(11.0)).strong().color(CYAN)).selectable(false));
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(16.0).strong());
            ui.label(RichText::new(subtitle).size(11.5).color(MUTED));
        });
    });
}

fn nav_button(ui: &mut egui::Ui, glyph: &str, label: &str, active: bool, icons_loaded: bool) -> bool {
    let fill = if active { Color32::from_rgb(10, 39, 61) } else { PANEL };
    let stroke = if active { Stroke::new(1.0_f32, Color32::from_rgb(30, 107, 153)) } else { Stroke::new(1.0_f32, Color32::TRANSPARENT) };
    let response = egui::Frame::none().fill(fill).stroke(stroke).rounding(4.0).inner_margin(egui::Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.add_sized([21.0, 18.0], egui::Label::new(RichText::new(glyph_text(icons_loaded, glyph)).font(icon_font(15.0)).color(if active { CYAN } else { MUTED })).selectable(false));
            ui.label(RichText::new(label).size(13.0).color(if active { TEXT } else { Color32::from_rgb(144, 198, 226) }));
        });
    }).response;
    response.interact(egui::Sense::click()).clicked()
}

fn glyph_text(icons_loaded: bool, glyph: &str) -> &str {
    if icons_loaded { return glyph; }
    match glyph {
        "󰋩" => "▧",
        "󰈙" => "▤",
        "󰈔" => "▣",
        "󰘦" | "󰓦" => "▦",
        "󰎆" => "♫",
        "󰍉" => "⌕",
        "󰉋" => "▰",
        "󰄬" => "✓",
        "󰅖" => "×",
        "󰇘" => "⇩",
        "󰩨" => "↗",
        "󰍛" | "󰍹" => "⇩",
        "󰔐" => "◐",
        "󰟖" => "◌",
        "󰋼" => "ⓘ",
        "󰕥" => "▦",
        "󰐕" => "+",
        "󰑓" => "↻",
        "󰒺" => "↕",
        "󰏘" => "◈",
        "󰐊" => "▶",
        _ => glyph,
    }
}

fn file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_video_file(path: &Path) -> bool {
    matches!(file_extension(path).as_str(),
        "mp4" | "mkv" | "mov" | "webm" | "avi" | "m4v" | "mpeg" | "mpg" | "3gp" | "ogv" | "flv" | "mts" | "m2ts")
}

fn is_image_file(path: &Path) -> bool {
    matches!(file_extension(path).as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff" | "bmp" | "heic" | "avif" | "gif")
}

fn is_audio_file(path: &Path) -> bool {
    matches!(file_extension(path).as_str(),
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac")
}

pub(super) fn is_supported_input_file(tool: ToolId, path: &Path) -> bool {
    match tool {
        ToolId::GenerateQr => false,
        ToolId::PdfMerge | ToolId::PdfSplit | ToolId::PdfCompress | ToolId::PdfRotate
        | ToolId::PdfReorder | ToolId::PdfToImages | ToolId::PdfWatermark | ToolId::PdfToDoc => {
            file_extension(path) == "pdf"
        }
        ToolId::DocToPdf => {
            matches!(file_extension(path).as_str(), "doc" | "docx" | "odt" | "rtf")
        }
        ToolId::DataConvert => {
            matches!(file_extension(path).as_str(), "csv" | "json" | "xlsx")
        }
        ToolId::ExtractAudio => is_video_file(path) || is_audio_file(path),
        ToolId::VideoConvert | ToolId::VideoToGif => is_video_file(path),
        ToolId::ImagesToPdf | ToolId::ImageConvert | ToolId::ImageResize | ToolId::ImageCompress
        | ToolId::ImageGrayscale | ToolId::RemoveBackground | ToolId::ExtractExif
        | ToolId::ImageOcr | ToolId::LongPngToPdf => is_image_file(path),
    }
}

pub(super) fn accepted_input_description(tool: ToolId) -> &'static str {
    match tool {
        ToolId::GenerateQr => "text",
        ToolId::PdfMerge | ToolId::PdfSplit | ToolId::PdfCompress | ToolId::PdfRotate
        | ToolId::PdfReorder | ToolId::PdfToImages | ToolId::PdfWatermark | ToolId::PdfToDoc => "PDF files",
        ToolId::DocToPdf => "DOC, DOCX, ODT, or RTF files",
        ToolId::DataConvert => "CSV, JSON, or XLSX files",
        ToolId::ExtractAudio => "audio or video files",
        ToolId::VideoConvert | ToolId::VideoToGif => "video files",
        _ => "image files",
    }
}

fn file_glyph(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase().as_str() {
        "pdf" => "󰈙",
        "png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff" | "heic" | "avif" | "gif" | "bmp" => "󰋩",
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "mpeg" | "mpg" | "3gp" | "ogv" | "flv" | "mts" | "m2ts" => "▶",
        "csv" | "json" | "xlsx" => "󰘦",
        _ => "󰈔",
    }
}

fn output_dir_for_home(home: &Path) -> PathBuf {
    let downloads = home.join("Downloads");
    if downloads.is_dir() {
        downloads
    } else {
        home.to_path_buf()
    }
}

pub(super) fn default_output_dir() -> PathBuf {
    let home = std::env::var_os("HOME");

    if let Some(home) = home.map(PathBuf::from).filter(|path| path.is_dir()) {
        return output_dir_for_home(&home);
    }

    std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
}

#[cfg(test)]
mod tests {
    use super::reorder_item;

    #[test]
    fn reorder_item_moves_up_and_down_without_off_by_one() {
        let mut values = vec!["A", "B", "C", "D"];
        reorder_item(&mut values, 0, 3);
        assert_eq!(values, vec!["B", "C", "A", "D"]);
        reorder_item(&mut values, 2, 4);
        assert_eq!(values, vec!["B", "C", "D", "A"]);
        reorder_item(&mut values, 3, 0);
        assert_eq!(values, vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn reorder_item_ignores_invalid_source() {
        let mut values = vec![1, 2, 3];
        reorder_item(&mut values, 9, 0);
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn reorder_item_to_index_moves_item_and_keeps_stable_order() {
        let mut values = vec!["A", "B", "C", "D"];
        super::reorder_item_to_index(&mut values, 0, 2);
        assert_eq!(values, vec!["B", "C", "A", "D"]);
        super::reorder_item_to_index(&mut values, 3, 0);
        assert_eq!(values, vec!["D", "B", "C", "A"]);
        super::reorder_item_to_index(&mut values, 1, 1);
        assert_eq!(values, vec!["D", "B", "C", "A"]);
    }


    #[test]
    fn output_dir_prefers_downloads_when_it_exists() {
        let home = std::env::temp_dir().join(format!("forgebox-output-downloads-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("Downloads")).unwrap();
        assert_eq!(super::output_dir_for_home(&home), home.join("Downloads"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn output_dir_falls_back_to_home_when_downloads_is_missing() {
        let home = std::env::temp_dir().join(format!("forgebox-output-home-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(super::output_dir_for_home(&home), home);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn input_file_filters_match_each_tool_category() {
        use crate::catalog::ToolId;

        assert!(super::is_supported_input_file(ToolId::PdfSplit, std::path::Path::new("input.PDF")));
        assert!(!super::is_supported_input_file(ToolId::PdfSplit, std::path::Path::new("input.png")));
        assert!(super::is_supported_input_file(ToolId::ImageConvert, std::path::Path::new("input.HEIC")));
        assert!(!super::is_supported_input_file(ToolId::ImageConvert, std::path::Path::new("input.docx")));
        assert!(super::is_supported_input_file(ToolId::ExtractAudio, std::path::Path::new("track.FLAC")));
        assert!(super::is_supported_input_file(ToolId::ExtractAudio, std::path::Path::new("clip.mkv")));
        assert!(!super::is_supported_input_file(ToolId::ExtractAudio, std::path::Path::new("table.xlsx")));
        assert!(super::is_supported_input_file(ToolId::DocToPdf, std::path::Path::new("report.docx")));
        assert!(super::is_supported_input_file(ToolId::DataConvert, std::path::Path::new("data.csv")));
    }

    #[test]
    fn video_extension_filter_recognizes_common_containers() {
        assert!(super::is_video_file(std::path::Path::new("holiday.MP4")));
        assert!(super::is_video_file(std::path::Path::new("clip.webm")));
        assert!(!super::is_video_file(std::path::Path::new("track.mp3")));
        assert!(!super::is_video_file(std::path::Path::new("picture.png")));
    }

    #[test]
    fn thumbnail_respects_exif_orientation() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/exif-orientation-6.jpg");
        let image = super::load_thumbnail(&fixture).expect("EXIF fixture should load");
        assert!(image.size[1] > image.size[0], "EXIF orientation should display this image in portrait orientation");
    }
}
