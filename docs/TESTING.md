# ForgeBox testing guide

## Automated checks

The Linux GitHub Actions workflow has two test layers:

1. **Source tests** run Rust unit tests and cargo check without requiring every optional conversion engine.
2. **Conversion integration tests** install the real Linux conversion utilities and execute every supported tool path that can be exercised without an external AI model.

The integration suite covers:

| Tool | Integration check |
|---|---|
| Image Converter | PNG to JPEG and decode the result |
| Images to PDF | Two-page combined PDF, separate one-page PDFs, compressed-JPEG path, and output page count |
| Resize Image | Output dimensions and aspect ratio |
| Compress Image | Output exists and decodes |
| Grayscale | RGB channels are equal |
| Remove Background | Missing optional backend produces an installation hint; model inference is intentionally not downloaded in CI |
| EXIF Metadata | ExifTool output parses as JSON |
| Extract Text (OCR) | Tesseract recognizes generated English text |
| QR Code Generator | Creates a valid PNG |
| Long Image to PDF | Tall image becomes a multi-page PDF |
| Merge PDFs | Combined page count |
| Split PDF | One output per requested page |
| Compress PDF | Ghostscript output remains a valid PDF |
| Rotate PDF | Output remains a valid multi-page PDF |
| Reorder PDF Pages | Output contains the requested page count |
| PDF to Images | Every page becomes a readable PNG |
| PDF Watermark | pdftk produces a valid stamped PDF |
| DOCX to PDF | LibreOffice converts a source document to PDF |
| PDF to DOCX | Poppler extracts selectable text and LibreOffice creates an editable DOCX; scanned PDFs produce a clear OCR hint |
| CSV / JSON / Excel | CSV ↔ JSON, CSV ↔ XLSX and JSON ↔ XLSX |
| Extract Audio | MP4 audio stream to MP3 |
| Video Converter | MP4 to WebM |
| Video to GIF | MP4 to animated GIF |
| Video to Sticker | Crop and Fit VP9 WebM outputs, 512×512 dimensions, duration and 256 KB size limit |
| DaVinci Resolve Linux | DNxHR HQ, HQX, YUV 4:4:4 and RGB 4:4:4 with PCM audio verification |

The optional rembg check never downloads a model. Actual background-removal quality and first-run model acquisition require a separate manual test.

## Run locally

Run the ordinary tests:

~~~bash
cargo clippy --locked --all-targets
cargo test --locked
cargo check --locked
~~~

Install the external tools used by the integration suite on Fedora:

~~~bash
sudo dnf install ImageMagick python3-img2pdf ghostscript qpdf poppler-utils pdftk-java \
  libreoffice tesseract tesseract-langpack-eng perl-Image-ExifTool ffmpeg
~~~

Then run the conversion suite:

~~~bash
cargo test --locked integration_tests -- --test-threads=1
~~~

A test skips when its required system command is unavailable. The dedicated CI job verifies the command set before running the suite, so missing integration dependencies fail CI rather than silently reducing coverage.

## Manual desktop checks

Automated command-line tests do not replace compositor and visual checks. Before a release, perform these checks on real desktops:

- Fedora GNOME on Wayland: external drag-and-drop, focus while dragging, selection outline, and dropping several files.
- Hyprland on Wayland: drag-and-drop, workspace switching, window focus, and file drops while the window is partially covered.
- X11: external file drops and native file picker behavior.
- Narrow window sizes: sidebar collapse/compact layout, wizard labels, horizontal margins, scrolling, and queue monitor.
- Images to PDF: import numbered files in multiple batches, confirm natural sorting before manual edits, then reorder by drag, remove a page during thumbnail loading, and export both combined and separate PDFs.
- Video Sticker: switch input videos during metadata/frame previews, close the editor while a preview is running, and verify that FFprobe/FFmpeg preview processes stop.
- Queue: add several different tools, edit the next task after queuing, stop an active task, close the window during a task, and continue after one task fails.
- Optional backends: install rembg and verify first-run model acquisition separately; test OCR with each language pack the user intends to support.

Record the distribution, desktop environment, display protocol, package format, and ForgeBox commit for any manual failure.
