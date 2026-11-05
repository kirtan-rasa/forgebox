# Changelog

## Unreleased

- Improve native Wayland file-drop compatibility by refreshing `text/uri-list` acceptance during pointer motion and flushing receive requests.
- Avoid restarting copy-action negotiation during an active drag.

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
- Slightly enlarged body/help typography, corrected visible step numbering when Parameters is skipped, and replaced text arrow glyphs with font-independent vector icons.

### Validation

- Exercised FFmpeg command combinations against a generated sample clip for MP4, MOV, MKV, WebM, GIF, MP3, WAV, AAC, OGG, FLAC and M4A output.
- Static consistency checks confirmed all tool IDs are catalogued and `Job` fields match their construction site.
