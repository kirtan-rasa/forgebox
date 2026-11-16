use super::ForgeApp;
use super::super::theme::*;
use super::icons::{glyph_text, icon_font};
use super::widgets::nav_button;
use crate::catalog::{Category, TOOLS};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

impl ForgeApp {
    pub(super) fn render_sidebar(&mut self, ctx: &egui::Context, sidebar_width: f32, compact_ui: bool) {
        egui::SidePanel::left("forge-sidebar")
            .exact_width(sidebar_width)
            .resizable(false)
            .frame(egui::Frame::none().fill(PANEL)
                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                .inner_margin(egui::Margin { left: 10.0, right: 10.0, top: 12.0, bottom: 10.0 }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([25.0, 30.0], egui::Label::new(
                        RichText::new(glyph_text(self.icons_loaded, "󰉋")).font(icon_font(19.0)).color(CYAN)
                    ).selectable(false));
                    ui.label(RichText::new("ForgeBox").font(egui::FontId::proportional(17.0)).strong().color(CYAN));
                });
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Search follows the same restrained outlined-field treatment as the workspace.
                egui::Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(5.0)
                    .inner_margin(egui::Margin::symmetric(7.0, 3.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized([18.0, 25.0], egui::Label::new(
                                RichText::new(glyph_text(self.icons_loaded, "󰍉")).font(icon_font(13.0)).color(MUTED)
                            ).selectable(false));
                            ui.add(egui::TextEdit::singleline(&mut self.search).frame(false)
                                .margin(egui::Margin::symmetric(2.0, 3.0))
                                .hint_text(RichText::new("Search tools").size(12.0).color(MUTED))
                                .desired_width(ui.available_width()));
                        });
                    });
                ui.add_space(8.0);

                ui.label(RichText::new("CATEGORIES").font(egui::FontId::monospace(9.5)).color(MUTED));
                ui.add_space(4.0);
                if nav_button(ui, "󰍉", "All tools", self.active_category.is_none(), self.icons_loaded) {
                    self.active_category = None;
                }
                for category in Category::ALL {
                    if nav_button(ui, category.glyph(), category.title(), self.active_category == Some(category), self.icons_loaded) {
                        self.active_category = Some(category);
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(RichText::new("TOOLS").font(egui::FontId::monospace(9.5)).color(MUTED));
                ui.add_space(4.0);

                egui::ScrollArea::vertical().id_salt("tool-list").auto_shrink([false, false]).show(ui, |ui| {
                    let query = self.search.to_lowercase();
                    let active_category = self.active_category;
                    let filtered: Vec<_> = TOOLS.iter().filter(|tool| {
                        active_category.map(|c| c == tool.category).unwrap_or(true)
                            && (query.is_empty() || tool.name.to_lowercase().contains(&query)
                                || tool.subtitle.to_lowercase().contains(&query)
                                || tool.description.to_lowercase().contains(&query))
                    }).collect();

                    if filtered.is_empty() {
                        ui.label(RichText::new("No tools found").size(11.5).color(MUTED));
                    }
                    for tool in filtered {
                        let selected = self.selected_tool == tool.id;
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), if compact_ui { 30.0 } else { 32.0 }),
                            egui::Sense::click(),
                        );
                        if selected {
                            ui.painter().rect_filled(rect, 3.0, ACTIVE_BG);
                            ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())), 1.0, ACCENT);
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 3.0, HOVER_BG);
                        }
                        ui.painter().text(rect.left_center() + Vec2::new(13.0, 0.0), egui::Align2::LEFT_CENTER,
                            glyph_text(self.icons_loaded, tool.glyph), icon_font(14.0),
                            if selected { CYAN } else { MUTED });
                        ui.painter().text(rect.left_center() + Vec2::new(37.0, 0.0), egui::Align2::LEFT_CENTER,
                            tool.name, egui::FontId::proportional(12.5),
                            if selected { TEXT } else { TEXT_SECONDARY });
                        if response.clicked() && !self.running && !self.dialog_open {
                            self.select_tool(tool.id);
                        }
                    }
                });
            });
    }
}
