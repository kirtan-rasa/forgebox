#!/usr/bin/env python3
"""Safely apply the ForgeBox responsive gutters and wizard-label fix.
Run from the ForgeBox project root: python3 forgebox-responsive-gutters-steps-v3.py
The script aborts without writing anything if any expected source snippet is missing
or ambiguous, and keeps a backup of src/main.rs before making changes.
"""
from pathlib import Path
import sys

source_path = Path("src/main.rs")
if not source_path.is_file():
    sys.exit("ERROR: src/main.rs not found. Run this script from the ForgeBox project root.")
source = source_path.read_text(encoding="utf-8")
changes = [
    ('layout change 1',
     '            });\n\n        let page_margin = if content_width < 620.0 { 10.0 } else if content_width < 920.0 { 16.0 } else { 22.0 };\n        egui::CentralPanel::default().frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(page_margin, if compact_ui { 10.0 } else { 14.0 }))).show(ctx, |ui| {',
     '            });\n\n        let page_margin = if content_width < 620.0 { 18.0 } else if content_width < 920.0 { 22.0 } else { 28.0 };\n        egui::CentralPanel::default().frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(page_margin, if compact_ui { 10.0 } else { 14.0 }))).show(ctx, |ui| {'),
    ('layout change 2',
     '            // This prevents egui from restoring a stale scroll offset above the upload panel.\n            ui.set_min_width(ui.available_width());\n            let tool = info(self.selected_tool);',
     '            // This prevents egui from restoring a stale scroll offset above the upload panel.\n            ui.set_width(ui.available_width());\n            let tool = info(self.selected_tool);'),
    ('layout change 3',
     '            let full_header_width = ui.available_width();\n            let header_width = full_header_width.min(if compact_ui { 520.0 } else { 840.0 });\n            let header_side_space = ((full_header_width - header_width) * 0.5).max(0.0);\n            ui.horizontal(|ui| {\n                ui.add_space(header_side_space);\n                egui::Frame::none()\n                    .fill(Color32::from_rgb(8, 29, 46))\n                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(19, 59, 81)))\n                    .rounding(6.0)\n                    .inner_margin(egui::Margin { left: 12.0, right: 12.0, top: 7.0, bottom: 7.0 })\n                    .show(ui, |ui| {\n                        ui.set_width((header_width - 24.0).max(1.0));\n                        ui.vertical_centered(|ui| {\n                            ui.label(\n                                RichText::new(tool.name)\n                                    .size(if compact_ui { 20.0 } else { 23.0 })\n                                    .strong()\n                                    .color(TEXT),\n                            );\n                            ui.add_space(1.0);\n                            ui.add(\n                                egui::Label::new(\n                                    RichText::new(tool.description)\n                                        .size(12.5)\n                                        .color(Color32::from_rgb(169, 204, 224)),\n                                )\n                                .halign(Align::Center)\n                                .wrap(),\n                            );\n                        });\n                    });\n                ui.add_space(header_side_space);\n            });\n            ui.add_space(7.0);',
     '            let full_header_width = ui.available_width();\n            let header_width = full_header_width.min(if compact_ui { 520.0 } else { 840.0 });\n            ui.with_layout(Layout::top_down(Align::Center), |ui| {\n                egui::Frame::none()\n                    .fill(Color32::from_rgb(8, 29, 46))\n                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(19, 59, 81)))\n                    .rounding(6.0)\n                    .inner_margin(egui::Margin { left: 12.0, right: 12.0, top: 7.0, bottom: 7.0 })\n                    .show(ui, |ui| {\n                        ui.set_width((header_width - 24.0).max(1.0));\n                        ui.vertical_centered(|ui| {\n                            ui.label(\n                                RichText::new(tool.name)\n                                    .size(if compact_ui { 20.0 } else { 23.0 })\n                                    .strong()\n                                    .color(TEXT),\n                            );\n                            ui.add_space(1.0);\n                            ui.add(\n                                egui::Label::new(\n                                    RichText::new(tool.description)\n                                        .size(12.5)\n                                        .color(Color32::from_rgb(169, 204, 224)),\n                                )\n                                .halign(Align::Center)\n                                .wrap(),\n                            );\n                        });\n                    });\n            });\n            ui.add_space(7.0);'),
    ('layout change 4',
     '                    ui.set_width(ui.available_width());\n                    let full_width = ui.available_width();\n                    let small_controls = compact_ui || full_width < 680.0;\n                    let controls_width = if small_controls { 94.0 } else { 206.0 };\n                    // Never reserve more horizontal space than actually exists.\n                    // The old 220 px floor could make this row wider than its card.\n                    let flow_width = (full_width - controls_width - 22.0).max(0.0);\n                    ui.horizontal(|ui| {',
     '                    ui.set_width(ui.available_width());\n                    let full_width = ui.available_width();\n                    let small_controls = compact_ui || full_width < 680.0;\n                    // Preserve full step names on narrow windows by placing navigation\n                    // on a separate row instead of crushing the progress buttons.\n                    let controls_below = full_width < 620.0;\n                    let controls_width = if small_controls { 94.0 } else { 206.0 };\n                    let flow_width = if controls_below {\n                        (full_width - 12.0).max(0.0)\n                    } else {\n                        (full_width - controls_width - 22.0).max(0.0)\n                    };\n                    ui.horizontal_wrapped(|ui| {'),
    ('layout change 5',
     '                                match (step_id, small_controls) {\n                                    (0, true) => 80.0,\n                                    (1, true) => 96.0,\n                                    (2, true) => 84.0,\n                                    (0, false) => 100.0,',
     '                                match (step_id, small_controls) {\n                                    (0, true) => 94.0,\n                                    (1, true) => 112.0,\n                                    (2, true) => 86.0,\n                                    (0, false) => 100.0,'),
    ('layout change 6',
     '                        ui.separator();\n                        ui.allocate_ui_with_layout(Vec2::new(controls_width, 40.0), Layout::right_to_left(Align::Center), |ui| {',
     '                        if !controls_below {\n                            ui.separator();\n                        }\n                        let navigation_width = if controls_below { full_width } else { controls_width };\n                        ui.allocate_ui_with_layout(Vec2::new(navigation_width, 40.0), Layout::right_to_left(Align::Center), |ui| {'),
    ('layout change 7',
     '                .show(ui, |ui| {\n                    ui.set_min_width(ui.available_width());\n            if self.dialog_open {',
     '                .show(ui, |ui| {\n                    ui.set_width(ui.available_width());\n            if self.dialog_open {'),
]

# Validate every expected snippet before changing the file. No half-applied patch jobs.
for label, old, _new in changes:
    count = source.count(old)
    if count != 1:
        sys.exit(f"ERROR: {label}: expected one matching source block, found {count}. No files changed.")

updated = source
for _label, old, new in changes:
    updated = updated.replace(old, new, 1)

backup = source_path.with_name("main.rs.pre-responsive-layout.bak")
suffix = 1
while backup.exists():
    backup = source_path.with_name(f"main.rs.pre-responsive-layout-{suffix}.bak")
    suffix += 1
backup.write_text(source, encoding="utf-8")
source_path.write_text(updated, encoding="utf-8")
print(f"Applied {len(changes)} responsive-layout changes (v3).")
print(f"Backup: {backup}")
print("Next: cargo fmt -- --check, then cargo check. Visual layout still needs to be tested in the app.")
