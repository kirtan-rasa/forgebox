use super::super::{ForgeApp, QueueStatus};
use crate::catalog::ToolId;
use super::widgets::reorder_arrow_button;
use eframe::egui::{self, Color32, RichText, Stroke};

impl ForgeApp {
    pub(super) fn render_queue(&mut self, ui: &mut egui::Ui) {
        if !self.queue_mode || self.queue.is_empty() {
            return;
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Task queue").strong().size(13.0));
            ui.label(RichText::new(format!("{} task(s)", self.queue.len())).size(11.0).color(Color32::GRAY));
        });
        ui.add_space(6.0);

        let mut remove_index = None;
        let mut move_request = None;
        for (index, entry) in self.queue.iter().enumerate() {
            let (label, tint) = match entry.status {
                QueueStatus::Pending => ("Queued", Color32::from_rgb(145, 164, 180)),
                QueueStatus::Running => ("Running", Color32::from_rgb(70, 180, 220)),
                QueueStatus::Completed => ("Done", Color32::from_rgb(80, 190, 145)),
                QueueStatus::Failed => ("Failed", Color32::from_rgb(220, 100, 120)),
                QueueStatus::Cancelled => ("Cancelled", Color32::from_rgb(190, 150, 90)),
            };

            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(5.0, 7.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(format!("{:02}. {}", index + 1, entry.tool_name)).strong().size(12.0));
                            let input_label = if entry.job.tool == ToolId::GenerateQr {
                                "Generated from text".to_owned()
                            } else if entry.file_count == 0 {
                                "No input files".to_owned()
                            } else if entry.file_count == 1 {
                                "1 input file".to_owned()
                            } else {
                                format!("{} input files", entry.file_count)
                            };
                            ui.label(RichText::new(entry.summary.as_deref().unwrap_or(&input_label)).size(10.5).color(Color32::GRAY));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(label).size(10.5).color(tint));
                            if !self.running {
                                if ui.small_button("×").on_hover_text("Remove task").clicked() {
                                    remove_index = Some(index);
                                }
                                if reorder_arrow_button(ui, index + 1 < self.queue.len(), false, "Move down") {
                                    move_request = Some((index, index + 1));
                                }
                                if reorder_arrow_button(ui, index > 0, true, "Move up") {
                                    move_request = Some((index, index - 1));
                                }
                            }
                        });
                    });
                });
            ui.add_space(4.0);
        }

        let pending_count = self.queue.iter()
            .filter(|entry| entry.status == QueueStatus::Pending)
            .count();
        if !self.running && pending_count > 0 {
            ui.add_space(8.0);
            if ui.add_sized(
                [ui.available_width(), 40.0],
                egui::Button::new(RichText::new(format!("▶  Run queue ({pending_count})")).strong())
                    .fill(Color32::from_rgb(15, 73, 85)),
            ).clicked() {
                self.start_queue();
            }
        }

        if !self.running {
            ui.horizontal(|ui| {
                if ui.small_button("Clear queue").clicked() {
                    remove_index = None;
                    self.clear_queue();
                }
                if self.queue.iter().any(|entry| entry.status == QueueStatus::Pending) {
                    ui.label(RichText::new("Tasks run in order. Failed tasks do not stop the rest of the queue.").size(10.5).color(Color32::GRAY));
                }
            });
        }

        if let Some((from, to)) = move_request {
            if from < self.queue.len() && to < self.queue.len() {
                let item = self.queue.remove(from);
                self.queue.insert(to, item);
            }
        }
        if let Some(index) = remove_index {
            if index < self.queue.len() {
                self.queue.remove(index);
            }
        }
    }
}
