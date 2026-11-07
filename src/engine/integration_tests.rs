//! End-to-end tests for the real conversion engines used by ForgeBox.
//! The dedicated CI job installs all mandatory utilities. Local test runs skip
//! a suite when one of its external commands is unavailable.

use super::{
    execute, image_magick_program, os, pdf_page_count, run_magick_owned, temp_dir, Job,
    JobOptions, PdfCompressionProfile, PdfImageFormat, PdfRotation, ResolveProfile,
};
use crate::catalog::ToolId;
use image::{Rgba, RgbaImage};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Workspace(PathBuf);

impl Workspace {
    fn new(name: &str) -> Self {
        Self(temp_dir(&format!("integration-{name}")).expect("create test workspace"))
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn available(command: &str) -> bool {
    Command::new("sh")
        .args(["-c", "command -v \"$1\" >/dev/null 2>&1", "forgebox-test", command])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn require(commands: &[&str], suite: &str) -> bool {
    let missing: Vec<_> = commands.iter().copied().filter(|c| !available(c)).collect();
    if missing.is_empty() {
        true
    } else {
        eprintln!("Skipping {suite} integration suite; missing: {}", missing.join(", "));
        false
    }
}

fn job(tool: ToolId, files: Vec<PathBuf>, output: PathBuf, options: JobOptions) -> Job {
    Job::new(tool, files, output, options).expect("valid integration job")
}

fn outputs(paths: &[PathBuf], count: usize) {
    assert_eq!(paths.len(), count, "unexpected output list: {paths:?}");
    for path in paths {
        let metadata = fs::metadata(path)
            .unwrap_or_else(|error| panic!("output {} missing: {error}", path.display()));
        assert!(metadata.is_file() && metadata.len() > 0, "invalid output: {}", path.display());
    }
}

fn image_fixture(path: &Path, width: u32, height: u32) {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([
            ((x * 7 + y * 3) % 255) as u8,
            ((x * 2 + y * 11) % 255) as u8,
            ((x * 13 + y) % 255) as u8,
            255,
        ])
    });
    image.save(path).expect("write image fixture");
}

fn blank_pdf(pages: u32) -> Vec<u8> {
    use std::io::Write;
    let mut objects = vec!["<< /Type /Catalog /Pages 2 0 R >>".to_string()];
    let kids = (0..pages)
        .map(|i| format!("{} 0 R", 3 + i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>"));
    for i in 0..pages {
        let page = 3 + i * 2;
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R >>",
            page + 1
        ));
        objects.push("<< /Length 0 >>\nstream\n\nendstream".into());
    }
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0usize];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(&mut pdf, "{} 0 obj\n{}\nendobj", i + 1, object).unwrap();
    }
    let xref = pdf.len();
    writeln!(&mut pdf, "xref\n0 {}\n0000000000 65535 f ", offsets.len()).unwrap();
    for offset in offsets.iter().skip(1) {
        writeln!(&mut pdf, "{offset:010} 00000 n ").unwrap();
    }
    writeln!(
        &mut pdf,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
        offsets.len()
    )
    .unwrap();
    pdf
}

fn write_pdf(path: &Path, pages: u32) {
    fs::write(path, blank_pdf(pages)).expect("write PDF fixture");
}

fn fixture_command(program: &str, args: Vec<OsString>) {
    let result = Command::new(program).args(args).output().expect("launch fixture command");
    assert!(
        result.status.success(),
        "{program} fixture command failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn image_tool_paths_work_with_real_imagemagick_exiftool_and_tesseract() {
    if !require(&[image_magick_program(), "img2pdf", "exiftool", "tesseract", "qpdf"], "images") {
        return;
    }
    let ws = Workspace::new("images");
    let input = ws.path("input.png");
    let second = ws.path("second.png");
    image_fixture(&input, 96, 64);
    image_fixture(&second, 80, 60);

    let converted = execute(job(
        ToolId::ImageConvert,
        vec![input.clone()],
        ws.path("convert"),
        JobOptions::ImageConvert { format: super::ImageFormat::Jpeg },
    )).expect("PNG to JPEG");
    outputs(&converted, 1);
    image::open(&converted[0]).expect("JPEG decodes");

    let resized = execute(job(
        ToolId::ImageResize,
        vec![input.clone()],
        ws.path("resize"),
        JobOptions::ImageResize { width: 32, height: 32 },
    )).expect("resize image");
    outputs(&resized, 1);
    assert_eq!(image::image_dimensions(&resized[0]).unwrap(), (32, 21));

    let compressed = execute(job(
        ToolId::ImageCompress,
        vec![input.clone()],
        ws.path("compress"),
        JobOptions::ImageCompress { quality: 55 },
    )).expect("compress image");
    outputs(&compressed, 1);
    image::open(&compressed[0]).expect("compressed image decodes");

    let gray = execute(job(
        ToolId::ImageGrayscale,
        vec![input.clone()],
        ws.path("gray"),
        JobOptions::None,
    )).expect("grayscale image");
    outputs(&gray, 1);
    assert!(image::open(&gray[0]).unwrap().to_rgba8().pixels()
        .all(|p| p[0] == p[1] && p[1] == p[2]));

    let exif = execute(job(
        ToolId::ExtractExif,
        converted.clone(),
        ws.path("exif"),
        JobOptions::None,
    )).expect("extract EXIF");
    outputs(&exif, 1);
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&exif[0]).unwrap())
        .unwrap().is_array());

    let ocr_image = ws.path("ocr.png");
    run_magick_owned(vec![
        os("-size"), os("900x180"), os("xc:white"), os("-font"), os("DejaVu-Sans"),
        os("-pointsize"), os("72"), os("-fill"), os("black"), os("-gravity"),
        os("center"), os("-annotate"), os("+0+0"), os("FORGEBOX OCR TEST"), os(&ocr_image),
    ]).expect("create OCR fixture");
    let ocr = execute(job(
        ToolId::ImageOcr,
        vec![ocr_image],
        ws.path("ocr"),
        JobOptions::ImageOcr { language: "eng".into() },
    )).expect("Tesseract OCR");
    outputs(&ocr, 1);
    assert!(fs::read_to_string(&ocr[0]).unwrap().to_ascii_uppercase().contains("FORGEBOX"));

    let qr = execute(job(
        ToolId::GenerateQr,
        Vec::new(),
        ws.path("qr"),
        JobOptions::GenerateQr { text: "https://example.com/forgebox".into() },
    )).expect("generate QR");
    outputs(&qr, 1);
    image::open(&qr[0]).expect("QR is a valid PNG");

    let pdf = execute(job(
        ToolId::ImagesToPdf,
        vec![input.clone(), second],
        ws.path("images-pdf"),
        JobOptions::ImagesToPdf { compress: false, quality: 82 },
    )).expect("images to PDF");
    outputs(&pdf, 1);
    assert_eq!(pdf_page_count(&pdf[0]).unwrap(), 2);

    let compressed_pdf = execute(job(
        ToolId::ImagesToPdf,
        vec![input.clone()],
        ws.path("images-pdf-compressed"),
        JobOptions::ImagesToPdf { compress: true, quality: 65 },
    )).expect("compressed images to PDF");
    outputs(&compressed_pdf, 1);
    assert_eq!(pdf_page_count(&compressed_pdf[0]).unwrap(), 1);

    let tall = ws.path("tall.png");
    image_fixture(&tall, 64, 4_000);
    let paginated = execute(job(
        ToolId::LongPngToPdf,
        vec![tall],
        ws.path("long-pdf"),
        JobOptions::None,
    )).expect("long image to PDF");
    outputs(&paginated, 1);
    assert_eq!(pdf_page_count(&paginated[0]).unwrap(), 2);
}

#[test]
fn optional_rembg_backend_reports_missing_setup_without_model_download() {
    let result = super::run_ok("rembg", vec![os("--help")]);
    if let Err(error) = result {
        assert!(
            error.contains("rembg") && error.contains("not found"),
            "missing optional backend needs an actionable error: {error}"
        );
    } else {
        eprintln!("rembg is installed; skipped model inference to avoid downloading a model in CI.");
    }
}

#[test]
fn pdf_tool_paths_produce_valid_documents_and_page_images() {
    if !require(&["qpdf", "pdfunite", "pdfseparate", "pdftoppm", "gs", "pdftk"], "PDF") {
        return;
    }
    let ws = Workspace::new("pdf");
    let source = ws.path("source.pdf");
    let second = ws.path("second.pdf");
    let watermark = ws.path("watermark.pdf");
    write_pdf(&source, 3);
    write_pdf(&second, 2);
    write_pdf(&watermark, 1);

    let merged = execute(job(ToolId::PdfMerge, vec![source.clone(), second], ws.path("merge"), JobOptions::None))
        .expect("merge PDFs");
    outputs(&merged, 1);
    assert_eq!(pdf_page_count(&merged[0]).unwrap(), 5);

    let split = execute(job(
        ToolId::PdfSplit, vec![source.clone()], ws.path("split"),
        JobOptions::PdfSplit { range: "1,last".into() },
    )).expect("split PDF");
    outputs(&split, 3);
    assert!(split.iter().all(|p| pdf_page_count(p).unwrap() == 1));

    let compressed = execute(job(
        ToolId::PdfCompress, vec![source.clone()], ws.path("compress"),
        JobOptions::PdfCompress { profile: PdfCompressionProfile::Ebook },
    )).expect("compress PDF");
    outputs(&compressed, 1);
    assert_eq!(pdf_page_count(&compressed[0]).unwrap(), 3);

    let rotated = execute(job(
        ToolId::PdfRotate, vec![source.clone()], ws.path("rotate"),
        JobOptions::PdfRotate { rotation: PdfRotation::Degrees90 },
    )).expect("rotate PDF");
    outputs(&rotated, 1);
    assert_eq!(pdf_page_count(&rotated[0]).unwrap(), 3);

    let reordered = execute(job(
        ToolId::PdfReorder, vec![source.clone()], ws.path("reorder"),
        JobOptions::PdfReorder { order: "3,1,2".into() },
    )).expect("reorder PDF pages");
    outputs(&reordered, 1);
    assert_eq!(pdf_page_count(&reordered[0]).unwrap(), 3);

    let rendered = execute(job(
        ToolId::PdfToImages, vec![source.clone()], ws.path("render"),
        JobOptions::PdfToImages { format: PdfImageFormat::Png, dpi: 72 },
    )).expect("PDF to images");
    outputs(&rendered, 3);
    assert!(rendered.iter().all(|p| image::open(p).is_ok()));

    let stamped = execute(job(
        ToolId::PdfWatermark, vec![source], ws.path("stamp"),
        JobOptions::PdfWatermark { watermark_file: watermark },
    )).expect("PDF watermark");
    outputs(&stamped, 1);
    assert_eq!(pdf_page_count(&stamped[0]).unwrap(), 3);
}

#[test]
fn document_and_spreadsheet_tools_convert_all_supported_pairs() {
    if !require(&["libreoffice", "qpdf"], "document/data") {
        return;
    }
    let ws = Workspace::new("documents-data");
    let rtf = ws.path("report.rtf");
    fs::write(
        &rtf,
        r"{\rtf1\ansi\deff0 {\fonttbl {\f0 DejaVu Sans;}}\f0\fs28 ForgeBox document integration test.}",
    ).unwrap();

    let pdf = execute(job(ToolId::DocToPdf, vec![rtf], ws.path("doc-pdf"), JobOptions::None))
        .expect("document to PDF");
    outputs(&pdf, 1);
    assert_eq!(pdf_page_count(&pdf[0]).unwrap(), 1);

    let docx = execute(job(ToolId::PdfToDoc, pdf.clone(), ws.path("pdf-docx"), JobOptions::None))
        .expect("PDF to DOCX");
    outputs(&docx, 1);
    assert_eq!(docx[0].extension().and_then(|e| e.to_str()), Some("docx"));

    let scanned_pdf = ws.path("scanned.pdf");
    write_pdf(&scanned_pdf, 1);
    let no_text_error = execute(job(
        ToolId::PdfToDoc,
        vec![scanned_pdf],
        ws.path("scanned-docx"),
        JobOptions::None,
    )).expect_err("blank/scanned PDFs must not produce an empty DOCX");
    assert!(no_text_error.contains("No selectable text"));

    let csv = ws.path("table.csv");
    let json = ws.path("table.json");
    fs::write(&csv, "name,score\nAda,10\nGrace,20\n").unwrap();
    fs::write(&json, r#"[{"name":"Ada","score":10},{"name":"Grace","score":20}]"#).unwrap();

    let csv_json = execute(job(
        ToolId::DataConvert, vec![csv.clone()], ws.path("csv-json"),
        JobOptions::DataConvert { format: super::DataFormat::Json },
    )).expect("CSV to JSON");
    outputs(&csv_json, 1);
    let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&csv_json[0]).unwrap()).unwrap();
    assert_eq!(parsed[1]["name"], "Grace");

    let json_csv = execute(job(
        ToolId::DataConvert, vec![json.clone()], ws.path("json-csv"),
        JobOptions::DataConvert { format: super::DataFormat::Csv },
    )).expect("JSON to CSV");
    outputs(&json_csv, 1);
    assert!(fs::read_to_string(&json_csv[0]).unwrap().contains("Grace,20"));

    let csv_xlsx = execute(job(
        ToolId::DataConvert, vec![csv], ws.path("csv-xlsx"),
        JobOptions::DataConvert { format: super::DataFormat::Xlsx },
    )).expect("CSV to XLSX");
    outputs(&csv_xlsx, 1);

    let xlsx_csv = execute(job(
        ToolId::DataConvert, csv_xlsx.clone(), ws.path("xlsx-csv"),
        JobOptions::DataConvert { format: super::DataFormat::Csv },
    )).expect("XLSX to CSV");
    outputs(&xlsx_csv, 1);

    let json_xlsx = execute(job(
        ToolId::DataConvert, vec![json], ws.path("json-xlsx"),
        JobOptions::DataConvert { format: super::DataFormat::Xlsx },
    )).expect("JSON to XLSX");
    outputs(&json_xlsx, 1);

    let xlsx_json = execute(job(
        ToolId::DataConvert, csv_xlsx, ws.path("xlsx-json"),
        JobOptions::DataConvert { format: super::DataFormat::Json },
    )).expect("XLSX to JSON");
    outputs(&xlsx_json, 1);
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&xlsx_json[0]).unwrap())
        .unwrap().is_array());
}

#[test]
fn audio_and_video_tools_extract_convert_and_generate_gif() {
    if !require(&["ffmpeg"], "audio/video") {
        return;
    }
    let ws = Workspace::new("media");
    let source = ws.path("sample.mp4");
    fixture_command("ffmpeg", vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"),
        os("-f"), os("lavfi"), os("-i"), os("color=c=blue:s=160x120:r=10:d=1"),
        os("-f"), os("lavfi"), os("-i"), os("sine=frequency=880:duration=1"),
        os("-shortest"), os("-c:v"), os("libx264"), os("-pix_fmt"), os("yuv420p"),
        os("-c:a"), os("aac"), source.as_os_str().to_os_string(),
    ]);

    let audio = execute(job(
        ToolId::ExtractAudio, vec![source.clone()], ws.path("audio"),
        JobOptions::ExtractAudio { format: super::AudioFormat::Mp3 },
    )).expect("extract audio");
    outputs(&audio, 1);
    assert_eq!(audio[0].extension().and_then(|e| e.to_str()), Some("mp3"));

    let video = execute(job(
        ToolId::VideoConvert, vec![source.clone()], ws.path("video"),
        JobOptions::VideoConvert {
            format: super::VideoFormat::Webm,
            preset: super::VideoPreset::Fast,
            quality: 28,
        },
    )).expect("convert video to WebM");
    outputs(&video, 1);

    let gif = execute(job(
        ToolId::VideoToGif, vec![source], ws.path("gif"),
        JobOptions::VideoToGif { width: 128, fps: 6 },
    )).expect("video to GIF");
    outputs(&gif, 1);
    let decoded = image::ImageReader::open(&gif[0]).unwrap()
        .with_guessed_format().unwrap().decode().unwrap();
    assert_eq!(decoded.width(), 128);
}


#[test]
fn davinci_resolve_profile_creates_verified_dnxhr_mov_with_pcm_audio() {
    if !require(&["ffmpeg", "ffprobe"], "DaVinci Resolve") {
        return;
    }

    let encoder_list = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .expect("query FFmpeg encoders");
    let encoder_text = format!("{}\\n{}", String::from_utf8_lossy(&encoder_list.stdout), String::from_utf8_lossy(&encoder_list.stderr));
    if !encoder_text.lines().any(|line| line.contains("dnxhd")) {
        eprintln!("Skipping DaVinci Resolve integration test: this FFmpeg build has no DNxHR encoder.");
        return;
    }

    let ws = Workspace::new("davinci-resolve");
    let input = ws.path("source.mkv");
    fixture_command("ffmpeg", vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"),
        os("-f"), os("lavfi"), os("-i"), os("testsrc2=size=320x240:rate=25:duration=1"),
        os("-f"), os("lavfi"), os("-i"), os("sine=frequency=1000:sample_rate=48000:duration=1"),
        os("-c:v"), os("mpeg4"), os("-q:v"), os("5"), os("-pix_fmt"), os("yuv420p"),
        os("-c:a"), os("pcm_s16le"), os("-shortest"), os(&input),
    ]);

    let result = execute(job(
        ToolId::DavinciResolve,
        vec![input],
        ws.path("resolve-output"),
        JobOptions::DavinciResolve { profile: ResolveProfile::Auto },
    )).expect("DaVinci Resolve export should succeed");
    outputs(&result, 1);
    assert_eq!(result[0].extension().and_then(|ext| ext.to_str()), Some("mov"));

    let probe = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "stream=codec_type,codec_name,profile,pix_fmt,sample_rate",
            "-of", "json",
        ])
        .arg(&result[0])
        .output()
        .expect("probe exported MOV");
    assert!(probe.status.success(), "FFprobe failed: {}", String::from_utf8_lossy(&probe.stderr));
    let metadata: serde_json::Value = serde_json::from_slice(&probe.stdout).expect("parse output metadata");
    let streams = metadata["streams"].as_array().expect("output stream list");
    let video = streams.iter().find(|stream| stream["codec_type"] == "video").expect("video stream");
    assert_eq!(video["codec_name"], "dnxhd");
    assert!(video["profile"].as_str().map(|profile| profile.eq_ignore_ascii_case("DNxHR HQ")).unwrap_or(false), "unexpected DNxHR profile: {}", video["profile"]);
    assert_eq!(video["pix_fmt"], "yuv422p");
    let audio = streams.iter().find(|stream| stream["codec_type"] == "audio").expect("audio stream");
    assert_eq!(audio["codec_name"], "pcm_s24le");
    assert_eq!(audio["sample_rate"], "48000");
}
