## Unreleased
- Add native Wayland external-file drag-and-drop through a pinned compatibility patch for winit 0.30.13.
- Enable Wayland and X11 backends together; the desktop entry now prefers native Wayland when available.
- Keep an explicit XWayland launcher as a fallback.

# Changelog

## 0.2.0-beta.1

### Added
- Video conversion to MP4, WebM, MKV and MOV using the installed FFmpeg codecs.
- Video quality control and three speed/size presets, with a low-CPU preset selected by default.
- Video-input filtering for conversion and GIF tools, including drag-and-drop validation.

### Improved
- Audio extraction now selects an encoder appropriate to MP3, WAV, AAC, OGG, FLAC or M4A instead of applying one bitrate option to every format.
- GIF export uses a generated palette for cleaner colors and limits processing to two threads.
- Removed nested vertical scroll areas from file lists and image-page previews so mouse-wheel/touchpad behavior is more predictable.
- Reduced release-build work by disabling full LTO and allowing multiple codegen units; the build script no longer builds check/test profiles on every normal release build.
- Tightened page padding and labeled the application as beta.
- Unified the wizard steps, connecting arrows and back/next controls into one responsive navigation bar; arrows expand to fill the space between steps.
- Replaced the separate results panel and duplicate running labels with one dynamic status area inside the save card; moved the output-folder action there and removed the redundant “choose other files” button.

### Validation
- Exercised FFmpeg command combinations against a generated sample clip for MP4, MOV, MKV, WebM, GIF, MP3, WAV, AAC, OGG, FLAC and M4A output.
- Static consistency checks confirmed all tool IDs are catalogued and `Job` fields match their construction site.
- The user confirmed `cargo check` and `cargo run --release` succeeded on Fedora for the original beta.1 tree. The UI refresh itself has not been compiled in the packaging environment because Rust/Cargo are unavailable here; run `cargo check` after applying the source update. `cargo test` has not been confirmed.
## UI refinements (unreleased)

- Slightly enlarged body/help typography throughout the interface.
- Renumbered the visible Save step correctly when Parameters is skipped.
- Replaced reorder-button arrow glyphs with font-independent vector icons.
