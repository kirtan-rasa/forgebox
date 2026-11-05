use crate::catalog::ToolId;
use csv::{ReaderBuilder, WriterBuilder};
use serde_json::{Map, Value};
use qrcode::QrCode;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(windows))]
static IMAGEMAGICK_PROGRAM: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

thread_local! {
    static EXECUTION_WARNINGS: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

pub fn take_execution_warnings() -> Vec<String> {
    EXECUTION_WARNINGS.with(|warnings| std::mem::take(&mut *warnings.borrow_mut()))
}

fn clear_execution_warnings() {
    EXECUTION_WARNINGS.with(|warnings| warnings.borrow_mut().clear());
}

fn record_execution_warning(message: String) {
    let message = message.trim().to_string();
    if message.is_empty() {
        return;
    }
    EXECUTION_WARNINGS.with(|warnings| {
        let mut warnings = warnings.borrow_mut();
        if !warnings.contains(&message) {
            warnings.push(message);
        }
    });
}

#[derive(Clone, Debug)]
pub struct Job {
    pub tool: ToolId,
    pub files: Vec<PathBuf>,
    pub secondary_file: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub target_format: String,
    pub text: String,
    pub width: String,
    pub height: String,
    pub quality: String,
    pub compress_pdf: bool,
    pub pdf_profile: String,
    pub rotation: String,
    pub pages: String,
    pub fps: String,
    pub video_preset: String,
    pub language: String,
    pub dpi: String,
}

pub fn execute(job: Job) -> Result<Vec<PathBuf>, String> {
    clear_execution_warnings();
    fs::create_dir_all(&job.output_dir)
        .map_err(|e| format!("Failed to create the output folder: {e}"))?;

    use ToolId::*;
    match job.tool {
        ImagesToPdf => images_to_pdf(&job),
        ImageConvert => process_each(&job, |input, out| {
            let target = normalized_ext(&job.target_format);
            let mut args = vec![os(input), os("-auto-orient")];
            if target == "jpg" || target == "jpeg" {
                // Transparent pixels have no direct representation in JPEG; use a predictable white background.
                args.extend([os("-background"), os("white"), os("-alpha"), os("remove"), os("-alpha"), os("off")]);
            }
            args.push(os(out));
            run_magick_owned(args)
        }),
        ImageResize => process_each(&job, |input, out| {
            let size = format!("{}x{}", positive(&job.width, 1600), positive(&job.height, 1200));
            let args = vec![os(input), os("-auto-orient"), os("-resize"), os(&size), os(out)];
            run_magick_owned(args)
        }),
        ImageCompress => process_each(&job, |input, out| {
            let quality = positive(&job.quality, 82).clamp(1, 100).to_string();
            run_magick_owned(vec![os(input), os("-auto-orient"), os("-quality"), os(&quality), os(out)])
        }),
        ImageGrayscale => process_each(&job, |input, out| {
            run_magick_owned(vec![os(input), os("-auto-orient"), os("-colorspace"), os("Gray"), os(out)])
        }),
        RemoveBackground => process_each(&job, |input, out| {
            run_ok("rembg", vec![os("i"), os(input), os(out)]).map(|_| ())
        }),
        ExtractExif => process_each_capture(&job, |input, out| {
            let output = run_capture("exiftool", vec![os("-json"), os(input)])?;
            fs::write(out, output).map_err(|e| format!("Failed to write JSON: {e}"))
        }),
        ImageOcr => process_each(&job, |input, out| {
            let base = out.with_extension("");
            let lang = if job.language.trim().is_empty() { "eng" } else { job.language.trim() };
            run_ok("tesseract", vec![os(input), os(&base), os("-l"), os(lang), os("txt")]).map(|_| ())
        }),
        GenerateQr => {
            if job.text.trim().is_empty() {
                return Err("Enter text or a URL for the QR code.".into());
            }
            let out = unique_path(&job.output_dir, "qr-code", "png");
            let code = QrCode::new(job.text.as_bytes())
                .map_err(|error| format!("Could not generate the QR code: {error}"))?;
            let qr_image = code
                .render::<image::Luma<u8>>()
                .min_dimensions(320, 320)
                .build();
            qr_image.save(&out).map_err(|error| format!("Failed to save the QR code: {error}"))?;
            Ok(vec![out])
        }
        LongPngToPdf => long_png_to_pdf(&job),
        PdfMerge => {
            require_files(&job, 2, "Select at least two PDF files to merge.")?;
            let out = unique_path(&job.output_dir, "merged", "pdf");
            let mut args: Vec<OsString> = job.files.iter().map(|p| os(p)).collect();
            args.push(os(&out));
            if let Err(error) = run_ok("pdfunite", args) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
        PdfSplit => {
            let input = first_file(&job)?;
            let page_count = pdf_page_count(input)?;
            let (first, end) = parse_pdf_page_range(&job.pages, page_count)?;
            let pattern = unique_pattern(&job.output_dir, &input.file_stem().and_then(|s| s.to_str()).unwrap_or("document"));
            let pattern_prefix = pattern.file_name().and_then(|s| s.to_str()).unwrap_or("split-%d.pdf").split("%d").next().unwrap_or("split-").to_string();
            if let Err(error) = run_ok("pdfseparate", vec![os("-f"), os(&first.to_string()), os("-l"), os(&end.to_string()), os(input), os(&pattern)]) {
                for created in files_with_prefix(&job.output_dir, &pattern_prefix) { let _ = fs::remove_file(created); }
                return Err(error);
            }
            let mut outputs = files_with_prefix(&job.output_dir, &pattern_prefix);
            outputs.retain(|p| p.extension().and_then(|s| s.to_str()).map(|ext| ext.eq_ignore_ascii_case("pdf")).unwrap_or(false));
            sort_numbered_paths(&mut outputs);
            if outputs.is_empty() {
                for created in files_with_prefix(&job.output_dir, &pattern_prefix) { let _ = fs::remove_file(created); }
                Err("The PDF was processed, but no output pages were found. Check the page range.".into())
            } else { Ok(outputs) }
        }
        PdfCompress => {
            let input = first_file(&job)?;
            let out = output_for_input(&job, input, "compressed", "pdf");
            let profile = match job.pdf_profile.as_str() {
                "screen" => "screen",
                "printer" => "printer",
                "prepress" => "prepress",
                _ => "ebook",
            };
            let result = run_ok("gs", vec![
                os("-sDEVICE=pdfwrite"), os("-dCompatibilityLevel=1.4"),
                os(&format!("-dPDFSETTINGS=/{profile}")),
                os("-dNOPAUSE"), os("-dQUIET"), os("-dBATCH"),
                os(&format!("-sOutputFile={}", out.display())), os(input),
            ]);
            if let Err(error) = result { let _ = fs::remove_file(&out); return Err(error); }
            Ok(vec![out])
        }
        PdfRotate => {
            let input = first_file(&job)?;
            let angle = match job.rotation.as_str() { "180" => "180", "270" => "270", _ => "90" };
            let out = output_for_input(&job, input, "rotated", "pdf");
            if let Err(error) = run_qpdf(vec![os(input), os(format!("--rotate=+{}:1-z", angle)), os(&out)]) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
        PdfReorder => {
            let input = first_file(&job)?;
            let order = job.pages.trim();
            if order.is_empty() {
                return Err("Enter a page order, for example 3,1,2,4.".into());
            }
            let page_count = pdf_page_count(input)?;
            let pages = parse_pdf_page_order(order, page_count)?;
            let normalized_order = pages.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
            let out = output_for_input(&job, input, "reordered", "pdf");
            if let Err(error) = run_qpdf(vec![os(input), os("--pages"), os("."), os(&normalized_order), os("--"), os(&out)]) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
        PdfToImages => {
            let input = first_file(&job)?;
            let dpi = positive(&job.dpi, 150).clamp(36, 600).to_string();
            let prefix = unique_prefix(&job.output_dir, &format!("{}-page", input.file_stem().and_then(|s| s.to_str()).unwrap_or("pdf")));
            let format = if job.target_format.eq_ignore_ascii_case("jpg") { "-jpeg" } else { "-png" };
            let prefix_name = prefix.file_name().and_then(|s| s.to_str()).unwrap_or("pdf-page").to_string();
            if let Err(error) = run_ok("pdftoppm", vec![os(format), os("-r"), os(&dpi), os(input), os(&prefix)]) {
                for created in files_with_prefix(&job.output_dir, &prefix_name) { let _ = fs::remove_file(created); }
                return Err(error);
            }
            let mut found = files_with_prefix(&job.output_dir, &prefix_name);
            let expected_ext = if format == "-jpeg" { "jpg" } else { "png" };
            found.retain(|path| path.extension().and_then(|s| s.to_str()).map(|ext| ext.eq_ignore_ascii_case(expected_ext)).unwrap_or(false));
            sort_numbered_paths(&mut found);
            if found.is_empty() {
                for created in files_with_prefix(&job.output_dir, &prefix_name) { let _ = fs::remove_file(created); }
                Err("Poppler finished, but no images were found.".into())
            } else { Ok(found) }
        }
        PdfWatermark => {
            let input = first_file(&job)?;
            let mark = job.secondary_file.as_ref().ok_or("Select the second PDF to use as a watermark.")?;
            let out = output_for_input(&job, input, "watermarked", "pdf");
            if let Err(error) = run_ok("pdftk", vec![os(input), os("stamp"), os(mark), os("output"), os(&out)]) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
        DocToPdf => process_each_convert(&job, "pdf"),
        PdfToDoc => process_each_convert(&job, "docx"),
        DataConvert => data_convert(&job),
        ExtractAudio => process_each(&job, |input, out| extract_audio(input, out, &job.target_format)),
        VideoConvert => process_each(&job, |input, out| convert_video(input, out, &job.target_format, &job.video_preset, &job.quality)),
        VideoToGif => process_each(&job, |input, out| {
            let fps = positive(&job.fps, 12).clamp(1, 30).to_string();
            let width = positive(&job.width, 480).clamp(64, 1280).to_string();
            // Palette generation dramatically improves GIF color quality without adding a second encode pass.
            let filter = format!("fps={fps},scale={width}:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer");
            run_ok("ffmpeg", vec![os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"), os("-i"), os(input), os("-filter_complex"), os(&filter), os("-threads"), os("2"), os("-loop"), os("0"), os(out)]).map(|_| ())
        }),
    }
}

fn convert_video(input: &Path, output: &Path, target_format: &str, preset: &str, quality: &str) -> Result<(), String> {
    let format = normalized_ext(target_format);
    let crf = positive(quality, 23).clamp(18, 30).to_string();
    let (preset_label, x264_preset, vp9_cpu_used) = match preset {
        "balanced" => ("balanced", "medium", "4"),
        "smaller" => ("smaller", "slow", "2"),
        _ => ("fast", "veryfast", "6"),
    };
    let mut args = vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"),
        os("-i"), os(input), os("-map"), os("0:v:0"), os("-map"), os("0:a?"),
        os("-map_metadata"), os("0"), os("-threads"), os("2"),
    ];
    match format.as_str() {
        "mp4" | "mov" | "mkv" => {
            args.extend([
                os("-c:v"), os("libx264"), os("-preset"), os(x264_preset),
                os("-crf"), os(&crf), os("-pix_fmt"), os("yuv420p"),
                os("-c:a"), os("aac"), os("-b:a"), os("160k"),
            ]);
            if format == "mp4" || format == "mov" {
                args.extend([os("-movflags"), os("+faststart")]);
            }
        }
        "webm" => {
            let vp9_crf = (positive(quality, 23) + 9).clamp(24, 39).to_string();
            args.extend([
                os("-c:v"), os("libvpx-vp9"), os("-deadline"), os("good"),
                os("-cpu-used"), os(vp9_cpu_used), os("-crf"), os(&vp9_crf), os("-b:v"), os("0"),
                os("-c:a"), os("libopus"), os("-b:a"), os("128k"),
            ]);
        }
        _ => return Err(format!("Video format \"{format}\" is not supported. Choose MP4, WebM, MKV, or MOV.")),
    }
    args.push(os(output));
    run_ok("ffmpeg", args).map(|_| ()).map_err(|error| {
        if error.contains("Unknown encoder") || error.contains("Encoder not found") {
            format!("This FFmpeg build does not include a codec for {}. Install an FFmpeg build with {} support.", format.to_uppercase(), if format == "webm" { "VP9 and Opus" } else { "H.264/AAC" })
        } else if error.contains("does not contain any stream") || error.contains("matches no streams") {
            format!("No video stream found in {}.", input.file_name().and_then(|v| v.to_str()).unwrap_or("file"))
        } else {
            format!("FFmpeg error while encoding {} ({preset_label}): {error}", format.to_uppercase())
        }
    })
}

fn extract_audio(input: &Path, output: &Path, target_format: &str) -> Result<(), String> {
    let format = normalized_ext(target_format);
    let mut args = vec![os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"), os("-i"), os(input), os("-map"), os("0:a:0"), os("-vn")];
    match format.as_str() {
        "mp3" => args.extend([os("-c:a"), os("libmp3lame"), os("-b:a"), os("192k")]),
        "wav" => args.extend([os("-c:a"), os("pcm_s16le")]),
        "aac" => args.extend([os("-c:a"), os("aac"), os("-b:a"), os("192k")]),
        "ogg" => args.extend([os("-c:a"), os("libvorbis"), os("-q:a"), os("5")]),
        "flac" => args.extend([os("-c:a"), os("flac")]),
        "m4a" => args.extend([os("-c:a"), os("aac"), os("-b:a"), os("192k")]),
        _ => return Err(format!(r#"Audio format "{format}" is not supported."#)),
    }
    args.push(os(output));
    run_ok("ffmpeg", args).map(|_| ()).map_err(|error| {
        if error.contains("matches no streams") || error.contains("does not contain any stream") {
            "No audio stream was found in the selected file.".into()
        } else { error }
    })
}

fn process_each<F>(job: &Job, mut operation: F) -> Result<Vec<PathBuf>, String>
where
    F: FnMut(&Path, &Path) -> Result<(), String>,
{
    require_files(job, 1, "Add files to process first.")?;
    let mut outputs: Vec<PathBuf> = Vec::new();
    for input in &job.files {
        let ext = match job.tool {
            ToolId::ImageConvert => normalized_ext(&job.target_format),
            ToolId::RemoveBackground => "png".to_string(),
            ToolId::ImageOcr => "txt".to_string(),
            ToolId::ExtractExif => "json".to_string(),
            ToolId::ExtractAudio => normalized_ext(&job.target_format),
            ToolId::VideoConvert => normalized_ext(&job.target_format),
            ToolId::VideoToGif => "gif".to_string(),
            _ => input.extension().and_then(|s| s.to_str()).unwrap_or("png").to_lowercase(),
        };
        let suffix = match job.tool {
            ToolId::ImageConvert => "converted",
            ToolId::ImageResize => "resized",
            ToolId::ImageCompress => "compressed",
            ToolId::ImageGrayscale => "grayscale",
            ToolId::RemoveBackground => "no-background",
            ToolId::ExtractExif => "exif",
            ToolId::ImageOcr => "ocr",
            ToolId::ExtractAudio => "audio",
            ToolId::VideoConvert => "converted",
            ToolId::VideoToGif => "animation",
            _ => "output",
        };
        let out = output_for_input(job, input, suffix, &ext);
        if let Err(error) = operation(input, &out) {
            let _ = fs::remove_file(&out);
            for created in &outputs { let _ = fs::remove_file(created); }
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            return Err(format!(r#"Failed to process "{name}": {error}"#));
        }
        outputs.push(out);
    }
    Ok(outputs)
}

fn process_each_capture<F>(job: &Job, mut operation: F) -> Result<Vec<PathBuf>, String>
where
    F: FnMut(&Path, &Path) -> Result<(), String>,
{
    require_files(job, 1, "Add files to process first.")?;
    let mut outputs: Vec<PathBuf> = Vec::new();
    for input in &job.files {
        let out = output_for_input(job, input, "exif", "json");
        if let Err(error) = operation(input, &out) {
            let _ = fs::remove_file(&out);
            for created in &outputs { let _ = fs::remove_file(created); }
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            return Err(format!(r#"Failed to read metadata from "{name}": {error}"#));
        }
        outputs.push(out);
    }
    Ok(outputs)
}

fn process_each_convert(job: &Job, target_ext: &str) -> Result<Vec<PathBuf>, String> {
    require_files(job, 1, "Add documents first.")?;
    let mut outputs: Vec<PathBuf> = Vec::new();
    for input in &job.files {
        let out = output_for_input(job, input, "converted", target_ext);
        if let Err(error) = soffice_convert(input, target_ext, &out) {
            let _ = fs::remove_file(&out);
            for created in &outputs { let _ = fs::remove_file(created); }
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("document");
            return Err(format!(r#"Failed to convert "{name}": {error}"#));
        }
        outputs.push(out);
    }
    Ok(outputs)
}

fn images_to_pdf(job: &Job) -> Result<Vec<PathBuf>, String> {
    require_files(job, 1, "Add at least one image to create a PDF.")?;
    let allowed = ["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "heic", "avif", "gif"];
    for input in &job.files {
        let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !allowed.contains(&ext.as_str()) {
            return Err(format!(r#"File "{}" does not appear to be a supported image."#, input.file_name().and_then(|s| s.to_str()).unwrap_or("unnamed file")));
        }
    }

    let out = unique_path(&job.output_dir, "images", "pdf");
    let temp = temp_dir("images-pdf")?;
    let result = (|| -> Result<Vec<PathBuf>, String> {
        let mut normalized: Vec<PathBuf> = Vec::with_capacity(job.files.len());
        for (index, input) in job.files.iter().enumerate() {
            // Normalize each file separately before assembling the PDF. This applies EXIF
            // orientation exactly once and writes standalone PNG pages so the orientation
            // cannot be applied again by the PDF encoder. Preserve density metadata so physical
            // page dimensions remain consistent with the source image.
            let page = temp.join(format!("page-{:05}.png", index + 1));
            let mut input_arg = input.as_os_str().to_os_string();
            if input.extension().and_then(|ext| ext.to_str()).map(|ext| ext.eq_ignore_ascii_case("gif")).unwrap_or(false) {
                input_arg.push("[0]");
            }
            if let Err(error) = run_magick_owned(vec![input_arg, os("-auto-orient"), os("+repage"), os(&page)]) {
                let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("image");
                return Err(format!(r#"Failed to prepare "{name}": {error}"#));
            }
            normalized.push(page);
        }

        let mut args: Vec<OsString> = normalized.iter().map(|p| os(p)).collect();
        if job.compress_pdf {
            args.extend([os("-background"), os("white"), os("-alpha"), os("remove"), os("-alpha"), os("off"), os("-compress"), os("JPEG"), os("-quality"), os(&positive(&job.quality, 82).clamp(20, 100).to_string())]);
        }
        args.push(os(&out));
        run_magick_owned(args)?;
        Ok(vec![out.clone()])
    })();
    let _ = fs::remove_dir_all(&temp);
    if result.is_err() { let _ = fs::remove_file(&out); }
    result
}

fn long_png_to_pdf(job: &Job) -> Result<Vec<PathBuf>, String> {
    let input = first_file(job)?;
    let temp = temp_dir("long-png")?;
    let result = (|| -> Result<Vec<PathBuf>, String> {
        let pattern = temp.join("page-%05d.png");
        run_magick_owned(vec![os(input), os("-auto-orient"), os("-crop"), os("100%x3508+0+0"), os("+repage"), os(&pattern)])?;
        let mut pages = fs::read_dir(&temp).map_err(display_io)?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().and_then(|s| s.to_str()).map(|ext| ext.eq_ignore_ascii_case("png")).unwrap_or(false)).collect::<Vec<_>>();
        sort_numbered_paths(&mut pages);
        if pages.is_empty() { return Err("Could not create pages from the image.".into()); }
        let out = output_for_input(job, input, "paginated", "pdf");
        let mut args: Vec<OsString> = pages.iter().map(|p| os(p)).collect();
        args.push(os(&out));
        if let Err(error) = run_magick_owned(args) { let _ = fs::remove_file(&out); return Err(error); }
        Ok(vec![out])
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

fn data_convert(job: &Job) -> Result<Vec<PathBuf>, String> {
    let input = first_file(job)?;
    let source_ext = input.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let target_ext = normalized_ext(&job.target_format);
    if !["csv", "json", "xlsx"].contains(&source_ext.as_str()) {
        return Err("Supported input formats are CSV, JSON, and XLSX.".into());
    }
    if !["csv", "json", "xlsx"].contains(&target_ext.as_str()) {
        return Err("The output format must be CSV, JSON, or XLSX.".into());
    }
    if source_ext == target_ext {
        return Err("The input and output formats are the same.".into());
    }

    let out = output_for_input(job, input, "converted", &target_ext);
    let result = (|| -> Result<(), String> {
        match (source_ext.as_str(), target_ext.as_str()) {
            ("csv", "json") => csv_to_json(input, &out)?,
            ("json", "csv") => json_to_csv(input, &out)?,
            ("csv", "xlsx") => soffice_convert(input, "xlsx", &out)?,
            ("xlsx", "csv") => soffice_convert(input, "csv", &out)?,
            ("json", "xlsx") => {
                let temp = temp_dir("json-xlsx")?;
                let result = (|| {
                    let temp_csv = temp.join("table.csv");
                    json_to_csv(input, &temp_csv)?;
                    soffice_convert(&temp_csv, "xlsx", &out)
                })();
                let _ = fs::remove_dir_all(&temp);
                result?;
            }
            ("xlsx", "json") => {
                let temp = temp_dir("xlsx-json")?;
                let result = (|| {
                    let temp_csv = temp.join("data.csv");
                    soffice_convert(input, "csv", &temp_csv)?;
                    csv_to_json(&temp_csv, &out)
                })();
                let _ = fs::remove_dir_all(&temp);
                result?;
            }
            _ => return Err("This format combination is not supported.".into()),
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&out);
        return Err(error);
    }
    Ok(vec![out])
}

fn csv_to_json(input: &Path, output: &Path) -> Result<(), String> {
    let mut reader = ReaderBuilder::new().flexible(true).from_path(input).map_err(|e| format!("Could not read CSV: {e}"))?;
    let headers = reader.headers().map_err(|e| format!("Failed to read CSV headers: {e}"))?.clone();
    let mut rows = Vec::new();
    for row in reader.records() {
        let record = row.map_err(|e| format!("CSV row error: {e}"))?;
        let mut object = Map::new();
        for (i, header) in headers.iter().enumerate() {
            object.insert(header.to_string(), Value::String(record.get(i).unwrap_or("").to_string()));
        }
        rows.push(Value::Object(object));
    }
    fs::write(output, serde_json::to_vec_pretty(&Value::Array(rows)).map_err(|e| e.to_string())?).map_err(display_io)
}

fn json_to_csv(input: &Path, output: &Path) -> Result<(), String> {
    let text = fs::read_to_string(input).map_err(display_io)?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {e}"))?;
    let rows = value.as_array().ok_or("JSON must contain an array of objects, for example [{\"name\":\"Ada\"}].")?;
    let mut header_set = BTreeSet::new();
    for row in rows {
        let object = row.as_object().ok_or("Each JSON row must be an object.")?;
        header_set.extend(object.keys().cloned());
    }
    let headers: Vec<String> = header_set.into_iter().collect();
    let mut writer = WriterBuilder::new().from_path(output).map_err(|e| format!("Failed to create CSV: {e}"))?;
    writer.write_record(&headers).map_err(|e| e.to_string())?;
    for row in rows {
        let object = row.as_object().expect("validated above");
        let record = headers.iter().map(|key| object.get(key).map(json_cell).unwrap_or_default()).collect::<Vec<_>>();
        writer.write_record(record).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(display_io)
}

fn json_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(_) | Value::Bool(_) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn soffice_convert(input: &Path, extension: &str, output: &Path) -> Result<(), String> {
    let temp = temp_dir("office-convert")?;
    let result = (|| -> Result<(), String> {
        run_ok("libreoffice", vec![os("--headless"), os("--convert-to"), os(extension), os("--outdir"), os(&temp), os(input)])?;
        let generated = temp.join(format!("{}.{}", input.file_stem().and_then(|s| s.to_str()).unwrap_or("output"), extension));
        if !generated.exists() {
            return Err(format!("LibreOffice did not create the expected file {}. Check whether the input format is supported.", generated.display()));
        }
        fs::copy(&generated, output).map_err(|e| format!("Failed to save the output: {e}"))?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&temp);
    if result.is_err() { let _ = fs::remove_file(output); }
    result
}

fn run_magick_owned(args: Vec<OsString>) -> Result<(), String> {
    #[cfg(windows)]
    {
        // Windows also ships a system convert.exe, unrelated to ImageMagick.
        // Never accidentally invoke that utility as a conversion backend.
        run_ok("magick", args).map(|_| ())
    }
    #[cfg(not(windows))]
    {
        let program = *IMAGEMAGICK_PROGRAM.get_or_init(|| {
            if has_command("magick") { "magick" } else { "convert" }
        });
        run_ok(program, args).map(|_| ())
    }
}

fn command_candidates(program: &str) -> Vec<OsString> {
    #[cfg(windows)]
    {
        let mut candidates: Vec<OsString> = Vec::new();
        let roots = [std::env::var_os("ProgramFiles"), std::env::var_os("ProgramFiles(x86)")];

        for root in roots.into_iter().flatten().map(PathBuf::from) {
            match program {
                "libreoffice" => {
                    for executable in ["soffice.com", "soffice.exe"] {
                        let path = root.join("LibreOffice").join("program").join(executable);
                        if path.is_file() { candidates.push(path.into_os_string()); }
                    }
                }
                "gs" => {
                    let install_root = root.join("gs");
                    if let Ok(entries) = fs::read_dir(install_root) {
                        let mut version_dirs: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
                        version_dirs.sort();
                        version_dirs.reverse();
                        for version_dir in version_dirs {
                            for executable in ["gswin64c.exe", "gswin32c.exe"] {
                                let path = version_dir.join("bin").join(executable);
                                if path.is_file() { candidates.push(path.into_os_string()); }
                            }
                        }
                    }
                }
                "magick" => {
                    if let Ok(entries) = fs::read_dir(&root) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let label = path.file_name().map(|name| name.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                            if path.is_dir() && label.starts_with("imagemagick") {
                                for executable in [path.join("magick.exe"), path.join("bin").join("magick.exe")] {
                                    if executable.is_file() { candidates.push(executable.into_os_string()); }
                                }
                            }
                        }
                    }
                }
                "tesseract" => {
                    let path = root.join("Tesseract-OCR").join("tesseract.exe");
                    if path.is_file() { candidates.push(path.into_os_string()); }
                }
                "qpdf" => {
                    for path in [root.join("qpdf").join("bin").join("qpdf.exe"), root.join("qpdf.exe")] {
                        if path.is_file() { candidates.push(path.into_os_string()); }
                    }
                    if let Ok(entries) = fs::read_dir(&root) {
                        for entry in entries.flatten() {
                            let folder = entry.path();
                            let label = folder.file_name().map(|name| name.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                            if folder.is_dir() && label.starts_with("qpdf") {
                                for path in [folder.join("bin").join("qpdf.exe"), folder.join("qpdf.exe")] {
                                    if path.is_file() { candidates.push(path.into_os_string()); }
                                }
                            }
                        }
                    }
                }
                "pdftk" => {
                    for path in [root.join("PDFtk Server").join("bin").join("pdftk.exe"), root.join("pdftk").join("pdftk.exe")] {
                        if path.is_file() { candidates.push(path.into_os_string()); }
                    }
                }
                _ => {}
            }
        }

        let aliases: &[&str] = match program {
            "gs" => &["gswin64c.exe", "gswin32c.exe", "gs.exe", "gs"],
            "libreoffice" => &["soffice.com", "soffice.exe", "libreoffice.exe", "libreoffice"],
            _ => &[],
        };
        for alias in aliases { candidates.push(OsString::from(alias)); }
        if !program.to_ascii_lowercase().ends_with(".exe") {
            candidates.push(OsString::from(format!("{program}.exe")));
        }
        candidates.push(OsString::from(program));

        let mut unique = Vec::new();
        for candidate in candidates {
            if !unique.contains(&candidate) { unique.push(candidate); }
        }
        unique
    }
    #[cfg(not(windows))]
    {
        if program == "rembg" {
            let mut candidates = Vec::new();
            let data_home = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty())
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .map(|home| home.join(".local/share"))
                });
            if let Some(data_home) = data_home {
                candidates.push(
                    data_home.join("forgebox/rembg-venv/bin/rembg").into_os_string()
                );
            }
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(
                    PathBuf::from(home).join(".local/bin/rembg").into_os_string()
                );
            }
            candidates.push(OsString::from(program));
            candidates
        } else {
            vec![OsString::from(program)]
        }
    }
}

fn run_ok(program: &str, args: Vec<OsString>) -> Result<Output, String> {
    let candidates = command_candidates(program);
    let mut tried: Vec<String> = Vec::new();
    for candidate in candidates {
        let label = candidate.to_string_lossy().into_owned();
        match Command::new(&candidate).args(args.iter()).output() {
            Ok(output) if output.status.success() => return Ok(output),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let detail = if !stderr.is_empty() { stderr } else { stdout };
                return Err(if detail.is_empty() {
                    format!("`{program}` exited with code {}.", output.status)
                } else {
                    format!("`{program}`: {detail}")
                });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => tried.push(label),
            Err(error) => return Err(format!("Failed to launch `{program}` via `{label}`: {error}")),
        }
    }
    let install_hint = match (program, std::env::consts::OS) {
        ("tesseract", "linux") => {
            " Install Tesseract and the matching language data with your distribution's package manager. On Fedora: sudo dnf install tesseract tesseract-langpack-eng."
        }
        ("tesseract", "windows") => {
            " Install Tesseract for Windows and add its executable directory to PATH."
        }
        ("tesseract", "macos") => {
            " Install Tesseract and make sure its executable is available on PATH."
        }
        ("rembg", "linux") => {
            " Install the optional rembg CPU backend in a Python 3.11–3.13 environment. Fedora RPM users can run: bash /usr/share/doc/forgebox/install-rembg-fedora.sh."
        }
        ("rembg", "windows") => {
            " Install the optional rembg CPU backend in a Python 3.11–3.13 virtual environment and add that environment's Scripts directory to PATH."
        }
        ("rembg", "macos") => {
            " Install the optional rembg CPU backend in a Python 3.11–3.13 virtual environment and make its bin directory available on PATH."
        }
        _ => " See the Installation section in the README for setup instructions.",
    };
    Err(format!(
        "Dependency `{program}` was not found. Tried: {}.{}",
        tried.join(", "),
        install_hint
    ))
}

fn run_capture(program: &str, args: Vec<OsString>) -> Result<Vec<u8>, String> {
    Ok(run_ok(program, args)?.stdout)
}

/// qpdf uses exit code 3 for recoverable warnings. Treat warnings as a
/// successful operation while leaving their messages available on stderr.
fn run_qpdf(args: Vec<OsString>) -> Result<Output, String> {
    let mut qpdf_args = Vec::with_capacity(args.len() + 1);
    qpdf_args.push(os("--warning-exit-0"));
    qpdf_args.extend(args);

    let output = run_ok("qpdf", qpdf_args)?;
    if !output.stderr.is_empty() {
        record_execution_warning(format!(
            "qpdf: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

fn pdf_page_count(input: &Path) -> Result<u32, String> {
    let output = run_qpdf(vec![os("--show-npages"), os(input)])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let page_count = stdout.trim().parse::<u32>().map_err(|_| {
        format!("Could not read the page count from this PDF. qpdf returned: {}", stdout.trim())
    })?;
    if page_count == 0 {
        return Err("This PDF contains no pages.".into());
    }
    Ok(page_count)
}

fn parse_pdf_page_range(value: &str, page_count: u32) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err("Enter a page range as start,end. Use last for the final page, for example 1,last.".into());
    }

    let first = parts[0].parse::<u32>().ok().filter(|page| *page > 0)
        .ok_or_else(|| "The start page must be a positive number.".to_string())?;
    let end = if parts[1].eq_ignore_ascii_case("last") {
        page_count
    } else {
        parts[1].parse::<u32>().ok().filter(|page| *page > 0)
            .ok_or_else(|| "The end page must be a positive number or last.".to_string())?
    };

    if first > page_count {
        return Err(format!("Page {first} does not exist. This PDF has {page_count} pages."));
    }
    if end > page_count {
        return Err(format!("Page {end} does not exist. This PDF has {page_count} pages. Use last for the final page."));
    }
    if end < first {
        return Err("The end page must not be less than the start page.".into());
    }
    Ok((first, end))
}

fn parse_pdf_page_order(value: &str, page_count: u32) -> Result<Vec<u32>, String> {
    if value.trim().is_empty() {
        return Err("Enter a page order, for example 3,1,2,4.".into());
    }

    value.split(',').map(str::trim).map(|part| {
        let page = part.parse::<u32>().ok().filter(|page| *page > 0)
            .ok_or_else(|| "The order must contain positive page numbers separated by commas.".to_string())?;
        if page > page_count {
            return Err(format!("Page {page} does not exist. This PDF has {page_count} pages."));
        }
        Ok(page)
    }).collect()
}

fn has_command(program: &str) -> bool {
    command_candidates(program).iter().any(|candidate| Command::new(candidate).arg("--version").output().is_ok())
}

fn require_files(job: &Job, min: usize, message: &str) -> Result<(), String> {
    if job.files.len() < min { Err(message.into()) } else { Ok(()) }
}

fn first_file(job: &Job) -> Result<&Path, String> {
    job.files.first().map(PathBuf::as_path).ok_or_else(|| "Add a file first.".to_string())
}

fn output_for_input(job: &Job, input: &Path, suffix: &str, extension: &str) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    unique_path(&job.output_dir, &format!("{stem}-{suffix}"), extension)
}

fn unique_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let ext = normalized_ext(extension);
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() { return first; }
    for index in 2..10_000 {
        let candidate = dir.join(format!("{stem}-{index}.{ext}"));
        if !candidate.exists() { return candidate; }
    }
    dir.join(format!("{stem}-{}.{}", timestamp(), ext))
}

fn unique_prefix(dir: &Path, stem: &str) -> PathBuf {
    let base = dir.join(stem);
    if !base.exists() && files_with_prefix(dir, stem).is_empty() { return base; }
    for index in 2..10_000 {
        let candidate = dir.join(format!("{stem}-{index}"));
        if !candidate.exists() && files_with_prefix(dir, candidate.file_name().and_then(|s| s.to_str()).unwrap_or(stem)).is_empty() { return candidate; }
    }
    dir.join(format!("{stem}-{}", timestamp()))
}

fn unique_pattern(dir: &Path, stem: &str) -> PathBuf {
    let name = unique_prefix(dir, &format!("{stem}-split"));
    dir.join(format!("{}-%d.pdf", name.file_stem().and_then(|s| s.to_str()).unwrap_or("split")))
}

fn sort_numbered_paths(paths: &mut [PathBuf]) {
    fn trailing_number(path: &Path) -> Option<u64> {
        let stem = path.file_stem()?.to_str()?;
        let digits: String = stem.chars().rev().take_while(|ch| ch.is_ascii_digit()).collect::<String>().chars().rev().collect();
        if digits.is_empty() { None } else { digits.parse().ok() }
    }
    paths.sort_by(|left, right| {
        match (trailing_number(left), trailing_number(right)) {
            (Some(a), Some(b)) => a.cmp(&b).then_with(|| left.file_name().cmp(&right.file_name())),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.file_name().cmp(&right.file_name()),
        }
    });
}

fn files_with_prefix(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    fs::read_dir(dir).ok().into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| {
        p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with(prefix)).unwrap_or(false)
    }).collect()
}

fn temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!("forgebox-{prefix}-{}-{}", std::process::id(), timestamp()));
    fs::create_dir_all(&path).map_err(display_io)?;
    Ok(path)
}

fn timestamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

fn positive(value: &str, fallback: i32) -> i32 {
    value.trim().parse::<i32>().ok().filter(|n| *n > 0).unwrap_or(fallback)
}

fn normalized_ext(value: &str) -> String {
    value.trim().trim_start_matches('.').to_lowercase()
}

fn os<T: AsRef<std::ffi::OsStr>>(value: T) -> OsString {
    value.as_ref().to_os_string()
}

fn display_io(error: io::Error) -> String { error.to_string() }

#[cfg(test)]
mod tests {
    use super::sort_numbered_paths;
    use std::path::PathBuf;

    #[test]
    fn numbered_outputs_are_sorted_numerically_not_lexicographically() {
        let mut files = vec![
            PathBuf::from("page-10.png"),
            PathBuf::from("page-2.png"),
            PathBuf::from("page-1.png"),
        ];
        sort_numbered_paths(&mut files);
        assert_eq!(files, vec![PathBuf::from("page-1.png"), PathBuf::from("page-2.png"), PathBuf::from("page-10.png")]);
    }
}


#[cfg(test)]
mod pdf_page_tests {
    use super::{parse_pdf_page_order, parse_pdf_page_range};

    #[test]
    fn split_range_accepts_last_page_keyword() {
        assert_eq!(parse_pdf_page_range("1,last", 8).unwrap(), (1, 8));
        assert_eq!(parse_pdf_page_range(" 2, 5 ", 8).unwrap(), (2, 5));
    }

    #[test]
    fn split_range_rejects_pages_outside_document() {
        let error = parse_pdf_page_range("1,12", 8).unwrap_err();
        assert!(error.contains("has 8 pages"));
        assert!(parse_pdf_page_range("9,last", 8).is_err());
    }

    #[test]
    fn split_range_rejects_reversed_or_malformed_ranges() {
        assert!(parse_pdf_page_range("5,2", 8).is_err());
        assert!(parse_pdf_page_range("1", 8).is_err());
        assert!(parse_pdf_page_range("1,0", 8).is_err());
    }

    #[test]
    fn page_order_accepts_valid_pages_and_repeats() {
        assert_eq!(parse_pdf_page_order("3,1,2,1", 4).unwrap(), vec![3, 1, 2, 1]);
    }

    #[test]
    fn page_order_rejects_missing_pages_and_empty_input() {
        assert!(parse_pdf_page_order("1,5", 4).unwrap_err().contains("has 4 pages"));
        assert!(parse_pdf_page_order("", 4).is_err());
        assert!(parse_pdf_page_order("1,,2", 4).is_err());
    }
}

#[cfg(test)]
mod execution_warning_tests {
    use super::{clear_execution_warnings, record_execution_warning, take_execution_warnings};

    #[test]
    fn execution_warnings_are_trimmed_and_deduplicated() {
        clear_execution_warnings();
        record_execution_warning("  qpdf: recovered object table  ".into());
        record_execution_warning("qpdf: recovered object table".into());
        assert_eq!(
            take_execution_warnings(),
            vec!["qpdf: recovered object table".to_string()]
        );
    }
}

#[cfg(test)]
mod video_tests {
    use super::{convert_video, extract_audio};
    use std::path::Path;

    #[test]
    fn unsupported_video_output_format_is_rejected_with_supported_choices() {
        let error = convert_video(
            Path::new("input.mp4"),
            Path::new("output.avi"),
            "avi",
            "fast",
            "23",
        ).expect_err("AVI is not a supported output target in the UI");
        assert!(error.contains("MP4, WebM, MKV, or MOV"));
    }

    #[test]
    fn unsupported_audio_output_format_is_rejected() {
        let error = extract_audio(
            Path::new("input.mp4"),
            Path::new("output.wma"),
            "wma",
        ).expect_err("WMA is not a supported audio output target");
        assert!(error.contains("is not supported"));
    }
}
