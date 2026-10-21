#!/usr/bin/env python3
"""Show only step numbers when the ForgeBox wizard stepper becomes too narrow.

Run this from the ForgeBox project root after applying responsive-arrows-fix-v5:
    python3 forgebox-responsive-step-numbers-v6.py

The script validates all target fragments before changing src/main.rs and creates
an adjacent backup first. It leaves the arrow/navigation layout untouched.
"""
from pathlib import Path
import sys

source_path = Path("src/main.rs")
if not source_path.is_file():
    sys.exit("ERROR: src/main.rs not found. Run this script from the ForgeBox project root.")
source = source_path.read_text(encoding="utf-8")

changes = [
    (
        "responsive label threshold",
        "                    let flow_width = (full_width - controls_width - nav_gap).max(0.0);\n",
        "                    let flow_width = (full_width - controls_width - nav_gap).max(0.0);\n                    // Hide labels only when the stepper itself has too little room.\n                    // This keeps numbered steps readable instead of squeezing their captions.\n                    let show_step_labels = flow_width >= 340.0;\n",
    ),
    (
        "step caption",
        '                                let caption = if complete { label.to_owned() } else { format!("{}  {label}", visible_index + 1) };\n',
        '                                let caption = if show_step_labels {\n                                    if complete { label.to_owned() } else { format!("{}  {label}", visible_index + 1) }\n                                } else {\n                                    (visible_index + 1).to_string()\n                                };\n',
    ),
    (
        "completion tick",
        "                                if complete {\n                                    paint_check_mark(\n",
        "                                if complete && show_step_labels {\n                                    paint_check_mark(\n",
    ),
]

# Check every target before changing anything, so this can never half-apply.
for label, old, _new in changes:
    count = source.count(old)
    if count != 1:
        sys.exit(
            f"ERROR: {label}: expected exactly one matching source fragment, found {count}. "
            "No files changed. This script is intended for the source after responsive-arrows-fix-v5."
        )

updated = source
for _label, old, new in changes:
    updated = updated.replace(old, new, 1)

if updated.count("let show_step_labels = flow_width >= 340.0;") != 1:
    sys.exit("ERROR: post-patch validation failed. No files changed.")
if updated.count("if complete && show_step_labels {") != 1:
    sys.exit("ERROR: completion-state validation failed. No files changed.")

backup = source_path.with_name("main.rs.pre-step-numbers-v6.bak")
suffix = 1
while backup.exists():
    backup = source_path.with_name(f"main.rs.pre-step-numbers-v6-{suffix}.bak")
    suffix += 1
backup.write_text(source, encoding="utf-8")
source_path.write_text(updated, encoding="utf-8")

print("Applied responsive step labels: names remain visible when there is room; narrow layouts show step numbers only.")
print("The Back/Next and arrow geometry was not changed.")
print(f"Backup: {backup}")
print("Run: cargo fmt -- --check && cargo check, then launch the app to verify the breakpoint visually.")
