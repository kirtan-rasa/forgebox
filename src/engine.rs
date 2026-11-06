mod common;
mod data;
mod images;
mod media;
mod pdf;
mod command;
mod control;
mod job;

pub(crate) use self::command::{image_magick_program, image_magick_thread_limit};
use self::command::{
    command_version_succeeds, configure_command_environment, recommended_image_magick_threads,
    run_capture, run_magick_owned, run_ok, run_qpdf,
};
use self::common::{
    display_io, files_with_prefix, first_file, normalized_ext, os, output_for_input, positive,
    process_each, process_each_capture, process_each_convert, require_files,
    sort_numbered_paths, temp_dir, unique_path, unique_pattern, unique_prefix,
};
use self::data::{csv_to_json, data_convert, encode_file_url_path, soffice_convert};
use self::images::{images_to_pdf, long_png_to_pdf};
use self::media::{convert_video, extract_audio};
use self::pdf::{parse_pdf_page_order, parse_pdf_page_range, pdf_page_count};
pub use self::job::{
    AudioFormat, DataFormat, ImageFormat, Job, JobOptions, PdfCompressionProfile,
    PdfImageFormat, PdfRotation, VideoFormat, VideoPreset,
};
pub use self::control::ExecutionControl;

use qrcode::QrCode;
use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

thread_local! {
    static EXECUTION_WARNINGS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static EXECUTION_CONTROL: RefCell<Option<ExecutionControl>> = const { RefCell::new(None) };
}

pub(super) fn with_execution_control<R>(
    control: ExecutionControl,
    operation: impl FnOnce() -> R,
) -> R {
    let previous = EXECUTION_CONTROL.with(|current| current.replace(Some(control)));
    let result = operation();
    EXECUTION_CONTROL.with(|current| {
        current.replace(previous);
    });
    result
}

pub(super) fn check_cancelled() -> Result<(), String> {
    let cancelled = EXECUTION_CONTROL.with(|current| {
        current.borrow().as_ref().map(ExecutionControl::is_cancelled).unwrap_or(false)
    });
    if cancelled {
        Err("Operation cancelled by user.".into())
    } else {
        Ok(())
    }
}

pub(super) fn execution_cancelled() -> bool {
    EXECUTION_CONTROL.with(|current| {
        current.borrow().as_ref().map(ExecutionControl::is_cancelled).unwrap_or(false)
    })
}

pub(super) fn report_status(status: impl Into<String>) {
    EXECUTION_CONTROL.with(|current| {
        if let Some(control) = current.borrow().as_ref() {
            control.report_status(status);
        }
    });
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

pub(crate) fn image_thumbnail_png(
    path: &Path,
    control: ExecutionControl,
) -> Result<Vec<u8>, String> {
    let args = vec![
        os("-limit"),
        os("thread"),
        os(image_magick_thread_limit().to_string()),
        os("-define"),
        os("jpeg:size=1200x840"),
        os(path),
        os("-auto-orient"),
        os("-thumbnail"),
        os("600x420"),
        os("png:-"),
    ];

    with_execution_control(control, || run_capture(image_magick_program(), args))
}

pub fn execute(job: Job) -> Result<Vec<PathBuf>, String> {
    execute_with_control(job, ExecutionControl::silent())
}

pub fn execute_with_control(job: Job, control: ExecutionControl) -> Result<Vec<PathBuf>, String> {
    clear_execution_warnings();
    with_execution_control(control, || execute_job(job))
}

fn execute_job(job: Job) -> Result<Vec<PathBuf>, String> {
    check_cancelled()?;
    fs::create_dir_all(&job.output_dir)
        .map_err(|error| format!("Failed to create the output folder: {error}"))?;

    use self::job::JobOptions as Options;
    use crate::catalog::ToolId as Tool;

    match (&job.tool, &job.options) {
        (Tool::ImagesToPdf, Options::ImagesToPdf { compress, quality }) => {
            images_to_pdf(&job, *compress, *quality)
        }
        (Tool::ImageConvert, Options::ImageConvert { format }) => process_each(&job, |input, out| {
            let mut args = vec![os(input), os("-auto-orient")];
            if matches!(format, ImageFormat::Jpeg) {
                args.extend([os("-background"), os("white"), os("-alpha"), os("remove"), os("-alpha"), os("off")]);
            }
            args.push(os(out));
            run_magick_owned(args)
        }),
        (Tool::ImageResize, Options::ImageResize { width, height }) => process_each(&job, |input, out| {
            let size = format!("{width}x{height}");
            run_magick_owned(vec![os(input), os("-auto-orient"), os("-resize"), os(&size), os(out)])
        }),
        (Tool::ImageCompress, Options::ImageCompress { quality }) => process_each(&job, |input, out| {
            let quality = quality.to_string();
            run_magick_owned(vec![os(input), os("-auto-orient"), os("-quality"), os(&quality), os(out)])
        }),
        (Tool::ImageGrayscale, Options::None) => process_each(&job, |input, out| {
            run_magick_owned(vec![os(input), os("-auto-orient"), os("-colorspace"), os("Gray"), os(out)])
        }),
        (Tool::RemoveBackground, Options::None) => process_each(&job, |input, out| {
            run_ok("rembg", vec![os("i"), os(input), os(out)]).map(|_| ())
        }),
        (Tool::ExtractExif, Options::None) => process_each_capture(&job, |input, out| {
            let output = run_capture("exiftool", vec![os("-json"), os(input)])?;
            fs::write(out, output).map_err(|error| format!("Failed to write JSON: {error}"))
        }),
        (Tool::ImageOcr, Options::ImageOcr { language }) => process_each(&job, |input, out| {
            let base = out.with_extension("");
            let lang = if language.trim().is_empty() { "eng" } else { language.trim() };
            run_ok("tesseract", vec![os(input), os(&base), os("-l"), os(lang), os("txt")])
                .map(|_| ())
                .map_err(|error| {
                    if error.contains("Error opening data file")
                        || error.contains("Failed loading language")
                    {
                        let languages = lang
                            .split('+')
                            .map(str::trim)
                            .filter(|language| !language.is_empty())
                            .collect::<Vec<_>>();
                        let fedora_packages = languages
                            .iter()
                            .map(|language| format!("tesseract-langpack-{language}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let debian_packages = languages
                            .iter()
                            .map(|language| format!("tesseract-ocr-{language}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let arch_packages = languages
                            .iter()
                            .map(|language| format!("tesseract-data-{language}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        format!(
                            "Tesseract language data for `{lang}` is missing. Install the matching language package and try again. Examples: Fedora: `sudo dnf install {fedora_packages}`; Debian/Ubuntu: `sudo apt install {debian_packages}`; Arch: `sudo pacman -S {arch_packages}`."
                        )
                    } else {
                        error
                    }
                })
        }),
        (Tool::GenerateQr, Options::GenerateQr { text }) => {
            if text.trim().is_empty() {
                return Err("Enter text or a URL for the QR code.".into());
            }
            let out = unique_path(&job.output_dir, "qr-code", "png");
            let code = QrCode::new(text.as_bytes())
                .map_err(|error| format!("Could not generate the QR code: {error}"))?;
            let qr_image = code
                .render::<image::Luma<u8>>()
                .min_dimensions(320, 320)
                .build();
            if let Err(error) = qr_image.save(&out) {
                let _ = fs::remove_file(&out);
                return Err(format!("Failed to save the QR code: {error}"));
            }
            Ok(vec![out])
        }
        (Tool::LongPngToPdf, Options::None) => long_png_to_pdf(&job),
        (Tool::PdfMerge, Options::None) => {
            require_files(&job, 2, "Select at least two PDF files to merge.")?;
            let out = unique_path(&job.output_dir, "merged", "pdf");
            let mut args: Vec<OsString> = job.files.iter().map(|path| os(path)).collect();
            args.push(os(&out));
            if let Err(error) = run_ok("pdfunite", args) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
        (Tool::PdfSplit, Options::PdfSplit { range }) => {
            let input = first_file(&job)?;
            let page_count = pdf_page_count(input)?;
            let (first, end) = parse_pdf_page_range(range, page_count)?;
            let pattern = unique_pattern(&job.output_dir, &input.file_stem().and_then(|value| value.to_str()).unwrap_or("document"));
            let pattern_prefix = pattern.file_name().and_then(|value| value.to_str()).unwrap_or("split-%d.pdf")
                .split("%d").next().unwrap_or("split-").to_string();
            if let Err(error) = run_ok("pdfseparate", vec![os("-f"), os(&first.to_string()), os("-l"), os(&end.to_string()), os(input), os(&pattern)]) {
                for created in files_with_prefix(&job.output_dir, &pattern_prefix) { let _ = fs::remove_file(created); }
                return Err(error);
            }
            let mut outputs = files_with_prefix(&job.output_dir, &pattern_prefix);
            outputs.retain(|path| path.extension().and_then(|value| value.to_str()).map(|extension| extension.eq_ignore_ascii_case("pdf")).unwrap_or(false));
            sort_numbered_paths(&mut outputs);
            if outputs.is_empty() {
                for created in files_with_prefix(&job.output_dir, &pattern_prefix) { let _ = fs::remove_file(created); }
                Err("The PDF was processed, but no output pages were found. Check the page range.".into())
            } else {
                Ok(outputs)
            }
        }
        (Tool::PdfCompress, Options::PdfCompress { profile }) => {
            let input = first_file(&job)?;
            let out = output_for_input(&job, input, "compressed", "pdf");
            let mut output_argument = OsString::from("-sOutputFile=");
            // Preserve non-UTF-8 Linux path bytes instead of round-tripping
            // through Path::display(), which is intentionally lossy.
            output_argument.push(out.as_os_str());
            let result = run_ok("gs", vec![
                os("-sDEVICE=pdfwrite"), os("-dCompatibilityLevel=1.4"),
                os(&format!("-dPDFSETTINGS=/{}", profile.as_str())),
                os("-dNOPAUSE"), os("-dQUIET"), os("-dBATCH"),
                output_argument, os(input),
            ]);
            if let Err(error) = result { let _ = fs::remove_file(&out); return Err(error); }
            Ok(vec![out])
        }
        (Tool::PdfRotate, Options::PdfRotate { rotation }) => {
            let input = first_file(&job)?;
            let out = output_for_input(&job, input, "rotated", "pdf");
            if let Err(error) = run_qpdf(vec![os(input), os(format!("--rotate=+{}:1-z", rotation.degrees())), os(&out)]) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
        (Tool::PdfReorder, Options::PdfReorder { order }) => {
            let input = first_file(&job)?;
            if order.trim().is_empty() {
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
        (Tool::PdfToImages, Options::PdfToImages { format, dpi }) => {
            let input = first_file(&job)?;
            let dpi = dpi.to_string();
            let prefix = unique_prefix(&job.output_dir, &format!("{}-page", input.file_stem().and_then(|value| value.to_str()).unwrap_or("pdf")));
            let format_arg = if matches!(format, PdfImageFormat::Jpeg) { "-jpeg" } else { "-png" };
            let prefix_name = prefix.file_name().and_then(|value| value.to_str()).unwrap_or("pdf-page").to_string();
            if let Err(error) = run_ok("pdftoppm", vec![os(format_arg), os("-r"), os(&dpi), os(input), os(&prefix)]) {
                for created in files_with_prefix(&job.output_dir, &prefix_name) { let _ = fs::remove_file(created); }
                return Err(error);
            }
            let mut found = files_with_prefix(&job.output_dir, &prefix_name);
            let expected_ext = format.as_str();
            found.retain(|path| path.extension().and_then(|value| value.to_str()).map(|extension| extension.eq_ignore_ascii_case(expected_ext)).unwrap_or(false));
            sort_numbered_paths(&mut found);
            if found.is_empty() {
                for created in files_with_prefix(&job.output_dir, &prefix_name) { let _ = fs::remove_file(created); }
                Err("Poppler finished, but no images were found.".into())
            } else {
                Ok(found)
            }
        }
        (Tool::PdfWatermark, Options::PdfWatermark { watermark_file }) => {
            let input = first_file(&job)?;
            let out = output_for_input(&job, input, "watermarked", "pdf");
            if let Err(error) = run_ok("pdftk", vec![os(input), os("stamp"), os(watermark_file), os("output"), os(&out)]) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
        (Tool::DocToPdf, Options::None) => process_each_convert(&job, "pdf"),
        (Tool::PdfToDoc, Options::None) => process_each_convert(&job, "docx"),
        (Tool::DataConvert, Options::DataConvert { format }) => data_convert(&job, format.as_str()),
        (Tool::ExtractAudio, Options::ExtractAudio { format }) => process_each(&job, |input, out| {
            extract_audio(input, out, format.as_str())
        }),
        (Tool::VideoConvert, Options::VideoConvert { format, preset, quality }) => process_each(&job, |input, out| {
            convert_video(input, out, format.as_str(), preset.as_str(), &quality.to_string())
        }),
        (Tool::VideoToGif, Options::VideoToGif { width, fps }) => process_each(&job, |input, out| {
            let filter = format!("fps={fps},scale={width}:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer");
            run_ok("ffmpeg", vec![os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"), os("-i"), os(input), os("-filter_complex"), os(&filter), os("-threads"), os("2"), os("-loop"), os("0"), os(out)]).map(|_| ())
        }),
        _ => Err(format!("The settings do not match the selected tool ({:?}).", job.tool)),
    }
}

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
mod pdf_integration_tests {
    use super::{execute, pdf_page_count, temp_dir, Job, JobOptions, PdfRotation};
    use crate::catalog::ToolId;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;

    fn make_blank_pdf(page_count: u32) -> Vec<u8> {
        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        ];
        let kids = (0..page_count)
            .map(|index| format!("{} 0 R", 3 + index * 2))
            .collect::<Vec<_>>()
            .join(" ");
        objects.push(format!(
            "<< /Type /Pages /Kids [{kids}] /Count {page_count} >>"
        ));

        for index in 0..page_count {
            let page_id = 3 + index * 2;
            let content_id = page_id + 1;
            objects.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_id} 0 R >>"
            ));
            objects.push("<< /Length 0 >>\nstream\n\nendstream".into());
        }

        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0usize];
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            writeln!(&mut pdf, "{} 0 obj\n{}\nendobj", index + 1, object)
                .expect("writing to a Vec cannot fail");
        }

        let xref_offset = pdf.len();
        writeln!(&mut pdf, "xref\n0 {}\n0000000000 65535 f ", offsets.len())
            .expect("writing to a Vec cannot fail");
        for offset in offsets.iter().skip(1) {
            writeln!(&mut pdf, "{offset:010} 00000 n ")
                .expect("writing to a Vec cannot fail");
        }
        writeln!(
            &mut pdf,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF",
            offsets.len(),
            xref_offset
        )
        .expect("writing to a Vec cannot fail");
        pdf
    }

    fn make_job(tool: ToolId, input: PathBuf, output_dir: PathBuf) -> Job {
        let options = match tool {
            ToolId::PdfSplit => JobOptions::PdfSplit { range: "1,last".into() },
            ToolId::PdfReorder => JobOptions::PdfReorder { order: "3,1,2,4".into() },
            ToolId::PdfRotate => JobOptions::PdfRotate { rotation: PdfRotation::Degrees90 },
            _ => JobOptions::None,
        };
        Job::new(tool, vec![input], output_dir, options).expect("test job settings should match the tool")
    }

    fn qpdf_available() -> bool {
        Command::new("qpdf").arg("--version").output()
            .map(|output| output.status.success()).unwrap_or(false)
    }

    fn pdfseparate_available() -> bool {
        Command::new("pdfseparate").arg("-v").output()
            .map(|output| output.status.success()).unwrap_or(false)
    }

    #[test]
    fn multi_page_pdf_split_rotate_and_reorder_work() {
        if !qpdf_available() || !pdfseparate_available() {
            eprintln!("Skipping PDF integration test: qpdf and pdfseparate are required.");
            return;
        }

        let directory = temp_dir("pdf-integration").unwrap();
        let input = directory.join("eight-pages.pdf");
        fs::write(&input, make_blank_pdf(8)).unwrap();

        let split = execute({
            make_job(
                ToolId::PdfSplit,
                input.clone(),
                directory.join("split"),
            )
        }).expect("splitting all eight pages should succeed");
        assert_eq!(split.len(), 8);
        for page in &split {
            assert_eq!(pdf_page_count(page).unwrap(), 1);
        }

        let rotated = execute(make_job(
            ToolId::PdfRotate,
            input.clone(),
            directory.join("rotated"),
        )).expect("rotating a multi-page PDF should succeed");
        assert_eq!(rotated.len(), 1);
        assert_eq!(pdf_page_count(&rotated[0]).unwrap(), 8);

        let reordered = execute({
            make_job(
                ToolId::PdfReorder,
                input,
                directory.join("reordered"),
            )
        }).expect("reordering pages should succeed");
        assert_eq!(reordered.len(), 1);
        assert_eq!(pdf_page_count(&reordered[0]).unwrap(), 4);

        let _ = fs::remove_dir_all(directory);
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
mod file_url_tests {
    use super::encode_file_url_path;

    #[test]
    fn file_url_paths_escape_spaces_and_uri_delimiters() {
        assert_eq!(
            encode_file_url_path("/tmp/ForgeBox profile/#1"),
            "/tmp/ForgeBox%20profile/%231"
        );
        assert_eq!(
            encode_file_url_path("C:/Users/Example User/AppData/Temp"),
            "C:/Users/Example%20User/AppData/Temp"
        );
    }
}

#[cfg(test)]
mod csv_conversion_tests {
    use super::{csv_to_json, temp_dir};
    use serde_json::Value;
    use std::fs;

    #[test]
    fn csv_to_json_fills_missing_cells_but_rejects_extra_fields() {
        let directory = temp_dir("csv-tests").unwrap();
        let input = directory.join("input.csv");
        let output = directory.join("output.json");

        fs::write(&input, "name,city\nAda\n").unwrap();
        csv_to_json(&input, &output).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(value[0]["name"], "Ada");
        assert_eq!(value[0]["city"], "");

        fs::write(&input, "name,city\nAda,London,extra\n").unwrap();
        let error = csv_to_json(&input, &output).unwrap_err();
        assert!(error.contains("Extra fields would be lost"));

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn csv_to_json_rejects_duplicate_headers() {
        let directory = temp_dir("csv-duplicate-headers").unwrap();
        let input = directory.join("input.csv");
        let output = directory.join("output.json");

        fs::write(&input, "name,name\nAda,Byron\n").unwrap();
        let error = csv_to_json(&input, &output).unwrap_err();
        assert!(error.contains("duplicate column name"));

        let _ = fs::remove_dir_all(directory);
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

#[cfg(test)]
mod image_magick_resource_tests {
    use super::recommended_image_magick_threads;

    #[test]
    fn image_magick_reserves_a_thread_and_caps_parallelism() {
        assert_eq!(recommended_image_magick_threads(1), 1);
        assert_eq!(recommended_image_magick_threads(2), 1);
        assert_eq!(recommended_image_magick_threads(4), 3);
        assert_eq!(recommended_image_magick_threads(32), 4);
    }
}

#[cfg(test)]
mod command_detection_tests {
    use super::command_version_succeeds;
    use std::ffi::OsStr;

    #[test]
    fn command_detection_requires_a_successful_exit_status() {
        assert!(command_version_succeeds(OsStr::new("/bin/true")));
        assert!(!command_version_succeeds(OsStr::new("/bin/false")));
        assert!(!command_version_succeeds(OsStr::new(
            "/definitely/missing/forgebox-command"
        )));
    }
}


#[cfg(test)]
mod subprocess_environment_tests {
    use super::configure_command_environment;
    use std::ffi::OsStr;
    use std::process::Command;

    #[test]
    fn rembg_does_not_inherit_appimage_python_paths() {
        let mut command = Command::new("true");
        configure_command_environment("rembg", &mut command);

        for variable in ["PYTHONHOME", "PYTHONPATH"] {
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == OsStr::new(variable) && value.is_none()),
                "{variable} must be removed from the rembg subprocess"
            );
        }
    }

    #[test]
    fn other_tools_keep_their_python_environment() {
        let mut command = Command::new("true");
        configure_command_environment("ffmpeg", &mut command);

        assert!(
            !command
                .get_envs()
                .any(|(key, _)| key == OsStr::new("PYTHONHOME"))
        );
        assert!(
            !command
                .get_envs()
                .any(|(key, _)| key == OsStr::new("PYTHONPATH"))
        );
    }
}

#[cfg(all(test, unix))]
mod ghostscript_path_tests {
    use super::OsString;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;

    #[test]
    fn ghostscript_output_argument_preserves_non_utf8_path_bytes() {
        let path = PathBuf::from(OsString::from_vec(
            b"/tmp/forgebox-output-\xff.pdf".to_vec(),
        ));
        let mut argument = OsString::from("-sOutputFile=");
        argument.push(path.as_os_str());

        let expected = [
            &b"-sOutputFile="[..],
            path.as_os_str().as_bytes(),
        ]
        .concat();
        assert_eq!(argument.as_os_str().as_bytes(), expected);
    }
}
