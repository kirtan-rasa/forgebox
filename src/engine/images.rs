use super::{
    display_io, first_file, os, output_for_input, require_files, run_magick_owned, run_ok,
    sort_numbered_paths, temp_dir, unique_path, Job,
};
use std::fs;
use std::path::PathBuf;

pub(super) fn images_to_pdf(job: &Job, compress: bool, quality: u8) -> Result<Vec<PathBuf>, String> {
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
        let mut args = vec![os("--output"), os(&out)];
        args.extend(pdf_pages.iter().map(|page| os(page)));
        run_ok("img2pdf", args)?;
        Ok(vec![out.clone()])
    })();
    let _ = fs::remove_dir_all(&temp);
    if result.is_err() { let _ = fs::remove_file(&out); }
    result
}

pub(super) fn long_png_to_pdf(job: &Job) -> Result<Vec<PathBuf>, String> {
    let input = first_file(job)?;
    let temp = temp_dir("long-png")?;
    let result = (|| -> Result<Vec<PathBuf>, String> {
        let pattern = temp.join("page-%05d.png");
        run_magick_owned(vec![os(input), os("-auto-orient"), os("-crop"), os("100%x3508"), os("+repage"), os(&pattern)])?;
        let mut pages = fs::read_dir(&temp).map_err(display_io)?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().and_then(|s| s.to_str()).map(|ext| ext.eq_ignore_ascii_case("png")).unwrap_or(false)).collect::<Vec<_>>();
        sort_numbered_paths(&mut pages);
        if pages.is_empty() { return Err("Could not create pages from the image.".into()); }
        let out = output_for_input(job, input, "paginated", "pdf");
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
