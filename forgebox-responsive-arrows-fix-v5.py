#!/usr/bin/env python3
"""Pin ForgeBox wizard navigation to the right edge of its bar.

Supports main.rs after the responsive-layout v3 script, whether or not the
previous arrows-fix v4 script was applied. Makes a backup before writing.
Run from the ForgeBox project root.
"""
from pathlib import Path
import sys

source_path = Path("src/main.rs")
if not source_path.is_file():
    sys.exit("ERROR: src/main.rs not found. Run this script from the ForgeBox project root.")
source = source_path.read_text(encoding="utf-8")

# Normalize either of the two layouts produced by v3/v4 into an explicitly
# sized row. Setting the row's implicit spacing to zero prevents egui's
# horizontal layout from shifting the navigation group away from the right edge.
v4_prefix = '''                    let controls_width = if small_controls { 94.0 } else { 206.0 };
                    // Keep Back/Next beside the stepper. Reserve enough room for both
                    // horizontal gaps and the separator so egui does not wrap navigation.
                    let flow_width = (full_width - controls_width - 36.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(Vec2::new(flow_width, 40.0), Layout::left_to_right(Align::Center), |ui| {'''
v3_prefix = '''                    let controls_below = full_width < 620.0;
                    let controls_width = if small_controls { 94.0 } else { 206.0 };
                    let flow_width = if controls_below {
                        (full_width - 12.0).max(0.0)
                    } else {
                        (full_width - controls_width - 22.0).max(0.0)
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.allocate_ui_with_layout(Vec2::new(flow_width, 40.0), Layout::left_to_right(Align::Center), |ui| {'''
new_prefix = '''                    let controls_width = if small_controls { 94.0 } else { 206.0 };
                    // Allocate the whole row, then position the stepper and navigation
                    // explicitly. No implicit horizontal spacing can push Back/Next inward.
                    let nav_gap = 16.0_f32;
                    let flow_width = (full_width - controls_width - nav_gap).max(0.0);
                    ui.allocate_ui_with_layout(Vec2::new(full_width, 40.0), Layout::left_to_right(Align::Center), |row_ui| {
                        row_ui.spacing_mut().item_spacing.x = 0.0;
                        let row_rect = row_ui.max_rect();
                        row_ui.allocate_ui_with_layout(Vec2::new(flow_width, 40.0), Layout::left_to_right(Align::Center), |ui| {'''

updated = source
prefix_matches = sum(updated.count(prefix) for prefix in (v4_prefix, v3_prefix))
if prefix_matches != 1:
    sys.exit(
        f"ERROR: expected exactly one recognized v3/v4 stepper layout, found {prefix_matches}. "
        "No files changed. Keep the backup/source and send this message with the current main.rs."
    )
if v4_prefix in updated:
    updated = updated.replace(v4_prefix, new_prefix, 1)
else:
    updated = updated.replace(v3_prefix, new_prefix, 1)

v4_nav = '''                        });
                        ui.separator();
                        ui.allocate_ui_with_layout(Vec2::new(controls_width, 40.0), Layout::right_to_left(Align::Center), |ui| {'''
v3_nav = '''                        });
                        if !controls_below {
                            ui.separator();
                        }
                        let navigation_width = if controls_below { full_width } else { controls_width };
                        ui.allocate_ui_with_layout(Vec2::new(navigation_width, 40.0), Layout::right_to_left(Align::Center), |ui| {'''
new_nav = '''                        });
                        let divider_x = row_rect.left() + flow_width + nav_gap * 0.5;
                        row_ui.painter().line_segment(
                            [egui::pos2(divider_x, row_rect.center().y - 12.0),
                             egui::pos2(divider_x, row_rect.center().y + 12.0)],
                            Stroke::new(1.0_f32, Color32::from_rgb(24, 76, 106)),
                        );
                        row_ui.allocate_ui_with_layout(Vec2::new(controls_width, 40.0), Layout::right_to_left(Align::Center), |ui| {'''
nav_matches = sum(updated.count(fragment) for fragment in (v4_nav, v3_nav))
if nav_matches != 1:
    sys.exit(
        f"ERROR: expected exactly one recognized navigation block, found {nav_matches}. "
        "No files changed. The source has not been modified."
    )
if v4_nav in updated:
    updated = updated.replace(v4_nav, new_nav, 1)
else:
    updated = updated.replace(v3_nav, new_nav, 1)

# A final guard: the new explicit layout must appear once and the old row wrappers must be gone.
if updated.count('let nav_gap = 16.0_f32;') != 1 or 'let controls_below = full_width < 620.0;' in updated:
    sys.exit("ERROR: post-patch validation failed. No files changed.")

backup = source_path.with_name("main.rs.pre-arrows-fix-v5.bak")
suffix = 1
while backup.exists():
    backup = source_path.with_name(f"main.rs.pre-arrows-fix-v5-{suffix}.bak")
    suffix += 1
backup.write_text(source, encoding="utf-8")
source_path.write_text(updated, encoding="utf-8")
print("Pinned the stepper and Back/Next navigation into a single fixed-width row.")
print("Removed implicit horizontal spacing from that row and retained a divider between the groups.")
print(f"Backup: {backup}")
print("Run: cargo fmt -- --check && cargo check")
