use super::{
    display_io, first_file, os, reserve_output_for_input, reserve_unique_path, require_files,
    run_magick_owned, run_ok, sort_numbered_paths, temp_dir_in, Job,
};
use std::fs;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn first_frame_input_arg(input: &Path) -> OsString {
    let mut argument = input.as_os_str().to_os_string();
    let extension = input.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if ["gif", "tif", "tiff"].iter().any(|value| extension.eq_ignore_ascii_case(value)) {
        // Each selected source file contributes exactly one PDF page.
        // GIF and multi-page TIFF inputs must not fan out into extra files.
        argument.push("[0]");
    }
    argument
}

pub(super) fn images_to_pdf(job: &Job, combine: bool, compress: bool, quality: u8) -> Result<Vec<PathBuf>, String> {
    require_files(job, 1, "Add at least one image to create a PDF.")?;
    let allowed = ["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "heic", "heif", "avif", "gif"];
    for input in &job.files {
        let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !allowed.contains(&ext.as_str()) {
            return Err(format!(r#"File "{}" does not appear to be a supported image."#, input.file_name().and_then(|s| s.to_str()).unwrap_or("unnamed file")));
        }
    }

    let temp = temp_dir_in(&job.output_dir, "images-pdf")?;
    let result = (|| -> Result<Vec<PathBuf>, String> {
        let mut normalized: Vec<PathBuf> = Vec::with_capacity(job.files.len());
        for (index, input) in job.files.iter().enumerate() {
            // Normalize each file separately before assembling the PDF. This applies EXIF
            // orientation exactly once and writes standalone PNG pages so the orientation
            // cannot be applied again by the PDF encoder. Preserve density metadata so physical
            // page dimensions remain consistent with the source image.
            let page = temp.join(format!("page-{:05}.png", index + 1));
            let input_arg = first_frame_input_arg(input);
            if let Err(error) = run_magick_owned(vec![input_arg, os("-auto-orient"), os("+repage"), os(&page)]) {
                let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("image");
                return Err(format!(r#"Failed to prepare "{name}": {error}"#));
            }
            normalized.push(page);
        }

        // ImageMagick's PDF coder is disabled by default on several Linux
        // distributions. Use img2pdf for PDF assembly instead of weakening the
        // host security policy.
        let mut pdf_pages = Vec::with_capacity(normalized.len());
        if compress {
            for (index, page) in normalized.iter().enumerate() {
                let jpeg = temp.join(format!("page-{:05}.jpg", index + 1));
                run_magick_owned(vec![
                    os(page), os("-background"), os("white"), os("-alpha"), os("remove"),
                    os("-alpha"), os("off"), os("-quality"), os(&quality.to_string()), os(&jpeg),
                ])?;
                pdf_pages.push(jpeg);
            }
        } else {
            pdf_pages = normalized;
        }
        if combine {
            let reservation = reserve_unique_path(&job.output_dir, "images", "pdf")?;
            let out = reservation.path().to_path_buf();
            let mut args = vec![os("--output"), os(&out)];
            args.extend(pdf_pages.iter().map(|page| os(page)));
            if let Err(error) = run_ok("img2pdf", args) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        } else {
            let mut outputs = Vec::with_capacity(job.files.len());
            for (input, page) in job.files.iter().zip(pdf_pages.iter()) {
                if let Err(error) = super::check_cancelled() {
                    return Err(format!(
                        "{error} {} completed PDF(s) were kept in {}.",
                        outputs.len(),
                        job.output_dir.display()
                    ));
                }
                let reservation = reserve_output_for_input(job, input, "converted", "pdf")?;
                let out = reservation.path().to_path_buf();
                let args = vec![os("--output"), os(&out), os(page)];
                if let Err(error) = run_ok("img2pdf", args) {
                    let _ = fs::remove_file(&out);
                    let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("image");
                    return Err(if outputs.is_empty() {
                        format!(r#"Failed to create a PDF for "{name}": {error}"#)
                    } else {
                        format!(r#"Failed to create a PDF for "{name}": {error}. {} earlier completed PDF(s) were kept in {}."#, outputs.len(), job.output_dir.display())
                    });
                }
                outputs.push(out);
            }
            Ok(outputs)
        }
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

pub(super) fn long_png_to_pdf(job: &Job) -> Result<Vec<PathBuf>, String> {
    let input = first_file(job)?;
    let temp = temp_dir("long-png")?;
    let result = (|| -> Result<Vec<PathBuf>, String> {
        let pattern = temp.join("page-%05d.png");
        // Omitting the width makes ImageMagick tile the full source width into
        // vertical strips. A percentage width is treated as one crop rectangle.
        run_magick_owned(vec![os(input), os("-auto-orient"), os("-crop"), os("x3508"), os("+repage"), os(&pattern)])?;
        let mut pages = fs::read_dir(&temp).map_err(display_io)?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().and_then(|s| s.to_str()).map(|ext| ext.eq_ignore_ascii_case("png")).unwrap_or(false)).collect::<Vec<_>>();
        sort_numbered_paths(&mut pages);
        if pages.is_empty() { return Err("Could not create pages from the image.".into()); }
        let reservation = reserve_output_for_input(job, input, "paginated", "pdf")?;
        let out = reservation.path().to_path_buf();
        let mut args = vec![os("--output"), os(&out)];
        args.extend(pages.iter().map(|page| os(page)));
        if let Err(error) = run_ok("img2pdf", args) {
            let _ = fs::remove_file(&out);
            return Err(error);
        }
        Ok(vec![out])
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}


#[cfg(test)]
mod first_frame_input_tests {
    use super::first_frame_input_arg;
    use std::path::Path;

    #[test]
    fn multi_frame_sources_are_limited_to_one_pdf_page() {
        for path in ["animation.gif", "scan.tif", "archive.TIFF"] {
            assert_eq!(
                first_frame_input_arg(Path::new(path)).to_string_lossy(),
                format!("{path}[0]")
            );
        }
    }

    #[test]
    fn ordinary_images_keep_their_original_path() {
        assert_eq!(
            first_frame_input_arg(Path::new("photo.png")).to_string_lossy(),
            "photo.png"
        );
    }
}
