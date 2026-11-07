# Changelog

## Unreleased

### Improved
- Run conversions immediately by default; make multi-task queueing an explicit optional mode.
- Detect the installed FFmpeg frame-sync options, preferring `-fps_mode passthrough` on newer builds and retaining `-vsync 0` for older builds.
- Open the actual output directory through the freedesktop FileManager1 D-Bus interface, with direct file-manager fallbacks instead of default file-opening handlers.

### Added
- Add a Telegram video sticker editor with trim handles, movable square framing, Crop/Fit modes, and VP9 WebM export under the 256 KB limit.
- Add an audio converter for MP3, WAV, FLAC, AAC, M4A, OGG, and Opus with selectable lossy-output bitrates.
- Add a DaVinci Resolve Linux export tool that creates DNxHR MOV files with 24-bit Linear PCM audio.
- Automatically select DNxHR HQ, HQX, or 444 from the source pixel format and bit depth.
- Verify exported DNxHR profile, pixel format, and PCM audio with FFprobe.
- Warn when subtitles, alpha, data streams, attachments, or additional video tracks are not preserved.

### Improved
- Document DaVinci Resolve export profiles, Linux requirements, and media-preservation limits.
- Exercise DNxHR HQ, HQX, YUV 4:4:4, and RGB 4:4:4 encoding in conversion integration tests.

## 0.4.1

### Added
- Offer optional setup of the rembg CPU backend and Russian Tesseract language data from the post-install flow.

### Improved
- Require FFmpeg for audio, video, and GIF features across RPM, DEB, and Arch packages.
- Configure RPM Fusion Free automatically on Fedora and switch from ffmpeg-free to the full FFmpeg build when needed.
- Keep public GitHub Releases focused on installable packages and the RPM verification key; retain intermediate build outputs only as CI artifacts.
- Validate the exact public release asset set before publishing.

## 0.4.0

### Added
- Add a one-command POSIX shell installer for Linux, deployed through GitHub Pages.

### Improved
- Refactor the application UI and conversion engine into focused modules.
- Strengthen conversion integration tests and improve Linux packaging.
- Improve Fedora dependency setup and release build infrastructure.


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
