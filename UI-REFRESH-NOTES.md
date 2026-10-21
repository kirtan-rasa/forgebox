ForgeBox UI refresh: typography, visible step numbering, reorder icons
======================================================================

Changes
-------
- Increased explicit UI text sizes by approximately 8-11%, with matching increases for the main title and conditional step/title text. This includes helper descriptions, card subtitles, file metadata and similar secondary text.
- The stepper labels use visible positions rather than fixed internal step IDs. Tools that skip Parameters show Save as step 2 in the navigation.
- The Save card heading uses the same dynamic visible step number, so it displays 02 when the Parameters step is hidden and 03 otherwise.
- Replaced Unicode up/down arrows in image-page and PDF reorder controls with vector-drawn arrows. The icons remain legible regardless of the active UI font or Nerd Font availability; tooltips and disabled states are retained.
- Previous workflow changes are preserved: connected navigation bar, optional Parameters step per tool, and a single dynamic status area inside the Save card.

Validation status
-----------------
- Static checks confirm that the stepper number is derived from visible_index, the Save heading derives its visible ordinal from wizard_step_sequence, and no Unicode arrow glyphs remain in the reorder buttons.
- ZIP/TAR archives are validated for integrity after packaging.
- Rust compilation could not be run in this packaging environment because cargo/rustc are not installed. In the existing Fedora checkout, replace src/main.rs and run `cargo check`; this should reuse the already-created target cache and Cargo.lock.
