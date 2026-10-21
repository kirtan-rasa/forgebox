# ForgeBox Beta

**Your local file toolkit.** One interface for everyday image, PDF, document, spreadsheet, audio, and video tasks. ForgeBox works locally and calls utilities installed on your computer to process files.

## Features

### Images
- Convert PNG, JPG, WebP, TIFF, HEIC, and other formats supported by ImageMagick.
- Resize images while preserving aspect ratio.
- Compress images, convert to grayscale, and remove backgrounds.
- Export EXIF metadata to JSON.
- OCR to TXT with selectable Tesseract language.
- Create a multi-page PDF from multiple images, with page previews, reordering, and optional compression.
- Split a tall image across PDF pages.

### PDF and Documents
- Merge PDFs and reorder input files.
- Split by page range, compress with a selectable profile, rotate, and reorder pages.
- Render PDF pages to PNG or JPG.
- Apply a PDF watermark.
- DOC/DOCX → PDF and attempted PDF → DOCX conversion through LibreOffice.

### Spreadsheets and Media
- CSV ↔ JSON, CSV ↔ XLSX, and JSON ↔ XLSX.
- Convert video between MP4, WebM, MKV, and MOV, with quality and encoding-speed controls. The default fast preset is designed to be gentler on slower CPUs.
- Extract audio from video to MP3, WAV, AAC, OGG, FLAC, or M4A.
- Convert video to GIF with an automatically generated palette and adjustable FPS and width.
- Generate QR codes from text or URLs.

## Interface and Behavior

The workflow has three steps: choose files, configure options, and save the output. The step indicators, connecting arrows, and Back/Next buttons share a single responsive navigation bar. Navigation controls stay in place even when a step cannot be opened. A single dynamic status area inside the save card shows progress, success, or errors. The separate results panel and redundant shortcut for choosing another set of files have been removed. The workspace header stays fixed while the tool list and main area scroll independently. Image previews scroll with the main content rather than using a nested scroll area. Fields and cards adapt to the window width.

File dialogs and thumbnail generation run in the background so reading large photos should not block the interface. In Images to PDF, page cards show previews corrected for EXIF orientation; orientation is also normalized before PDF creation, and page order follows the cards. Reorder pages by dragging cards or using the arrow buttons. Input PDFs can also be reordered with arrows in Merge PDFs.

Source files are never overwritten: ForgeBox chooses an unused output name. If processing one file in a batch fails, the app reports the problematic filename and removes partial outputs from that batch where it is safe to do so.

## Build

You need stable Rust, Cargo, FFmpeg for video/audio, and system libraries for eframe/winit. On Fedora, install the main dependencies from the project root:

```bash
bash scripts/install-deps-fedora.sh
```

Before the first build, prepare the pinned native Wayland file-drop backend (details below), then build and run:

```bash
bash scripts/enable-native-wayland-dnd.sh
cargo run --release
```

The bootstrap is one-time per source checkout. `scripts/build.sh` invokes it automatically when needed.

`cargo build --release` creates the binary `target/release/forgebox`. The release profile disables full LTO and uses multiple codegen units to reduce link times on slower CPUs. The binary may be slightly larger, but repeat builds are faster. The build script formats the code and builds only the release profile instead of needlessly compiling dev/test profiles:

```bash
bash scripts/build.sh
```

To run `cargo check` and tests separately, use `FORGEBOX_RUN_CHECKS=1 bash scripts/build.sh`. This is useful before a release, but not necessary for every small change.

## Native Wayland and Drag-and-Drop

The `winit 0.30` backend used by this version of `eframe` does not implement receiving external file drops on native Wayland. ForgeBox therefore includes a one-time bootstrap script that vendors a pinned compatibility patch for `winit 0.30.13`. The patch receives `text/uri-list` file offers through `wl_data_device` and forwards file-hover/drop events to the existing `egui` import flow. It requests copy-only transfers and reads the URI list asynchronously.

Before your first build, from the project root run:

```bash
bash scripts/enable-native-wayland-dnd.sh
cargo run --release
```

The bootstrap fetches only the `patches/winit` subtree at a pinned upstream commit, then keeps that source inside this project. It requires Git and an internet connection once. The application itself continues to process files locally. `scripts/build.sh` runs the bootstrap automatically if the vendored patch is missing.

The normal desktop entry now launches ForgeBox without unsetting `WAYLAND_DISPLAY`, so Hyprland can run it as a native Wayland client. The older `scripts/forgebox-x11` launcher remains available as a fallback if a particular compositor or file manager has compatibility issues.

This is a compatibility patch based on an open upstream proposal, not yet the official `winit 0.30` implementation. Upstream `winit 0.31` has introduced native file-drop events, but `eframe`/`egui-winit` must migrate to that API together before ForgeBox can remove the local patch. Multi-file drops should work, but the upstream patch is still under review for uncommon overlapping/slow drag-transfer cases.

## Tool Dependencies

The `scripts/install-deps-fedora.sh` script installs the main dependencies. FFmpeg may require RPM Fusion, and `rembg` is installed separately in a Python environment. ImageMagick must be built with HEIC/HEIF support to read and write those formats.

| Feature | Dependency |
|---|---|
| Conversion, resize, grayscale, images → PDF, and tall image → PDF | `magick` (ImageMagick; falls back to `convert`) |
| Background removal | `rembg` CLI |
| EXIF | `exiftool` |
| OCR | `tesseract` and language data (`eng`, `rus`, etc.) |
| QR codes | Built into ForgeBox (`qrcode`) |
| PDF merge/split/rasterize | `pdfunite`, `pdfseparate`, `pdftoppm` from Poppler |
| PDF compression / rotate / reorder / watermark | `gs`, `qpdf`, `pdftk` |
| DOCX/PDF and XLSX | `libreoffice` |
| Video conversion, audio extraction, and GIF | `ffmpeg` with H.264/AAC for MP4/MOV/MKV; VP9/Opus for WebM |

External programs are called only by the features that need them. A “dependency not found” error means the program is not installed or is missing from `PATH`. ForgeBox passes arguments directly without a shell, so spaces in paths should not break processing.

## Known Limitations

- Tall-image PDF conversion uses a fixed page-fragment height of 3508 px; paper-size and page-overlap settings are not available yet.
- PDF to DOCX depends on the PDF import capabilities of the installed LibreOffice version; complex layouts may change.
- PDF Split expects a range in `start,end` format; PDF page reordering expects a list such as `3,1,2,4`.
- Background removal depends on the `rembg` setup and model; `rembg` may download its model on first use.
- Excel operations require LibreOffice. JSON input for CSV/XLSX conversion must be an array of objects rather than an arbitrary nested JSON structure.
- Video conversion depends on the codecs included in the installed FFmpeg build; ForgeBox reports an FFmpeg error if a required codec is missing.
- External utilities have their own format limitations and security policies. For example, system ImageMagick policy may block PDF creation.

## License

MIT. See [`LICENSE`](LICENSE).
