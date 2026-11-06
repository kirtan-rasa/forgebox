use super::ForgeApp;
use super::super::theme::*;
use super::icons::{glyph_text, icon_font};
use super::widgets::nav_button;
use crate::catalog::{Category, TOOLS};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

impl ForgeApp {
    pub(super) fn render_sidebar(
        &mut self,
        ctx: &egui::Context,
        sidebar_width: f32,
        compact_ui: bool,
    ) {
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
    }
}
