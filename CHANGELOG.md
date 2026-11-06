# Changelog

## 0.4.0

### Added
- Add a one-command POSIX shell installer for Linux, deployed through GitHub Pages.
- Offer optional installation of the rembg CPU backend and Russian OCR language data.

### Improved
- Refactor the application UI and conversion engine into focused modules.
- Strengthen conversion integration tests and improve Linux packaging.
- Improve Fedora dependency setup and release build infrastructure.
- Require FFmpeg for audio and video conversion, including automatic RPM Fusion Free setup on Fedora.


## 0.3.0-beta.1

### Fixed

- Validate PDF page ranges against the actual page count before splitting or reordering.
- Rotate every page in a multi-page PDF and preserve page counts during PDF operations.
- Treat recoverable qpdf warnings as warnings instead of false conversion failures.
- Isolate LibreOffice conversions from active desktop sessions and other conversions.

### Improved

- Add Fedora dependency setup for OCR and an isolated optional rembg CPU environment.
- Improve native Wayland drag-and-drop compatibility across compositors.
- Validate dropped file types, improve output-directory selection, and reduce thumbnail queue overhead.
- Remove obsolete platform packaging and setup paths and simplify the project around Linux releases.

- Improve native Wayland file-drop compatibility by refreshing `text/uri-list` acceptance during pointer motion and flushing receive requests.
- Avoid restarting copy-action negotiation during an active drag.

## 0.2.0-beta.2

### Added

- Native Wayland external-file drag and drop through a pinned compatibility patch for `winit 0.30.13`.
- Live drag-to-reorder for Images-to-PDF pages, with a visible insertion slot and Escape to cancel.

### Improved

- Enable Wayland and X11 backends together; the desktop entry prefers native Wayland when available.
- Keep an explicit XWayland launcher as a fallback.
- Unify the wizard steps, connecting arrows, and Back/Next controls in one responsive navigation bar.
- Use one dynamic status area in the Save card instead of a separate results panel and duplicate running labels.
- Slightly enlarge body/help typography, correct visible step numbering when Parameters is skipped, and use font-independent vector icons for reorder controls.

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
