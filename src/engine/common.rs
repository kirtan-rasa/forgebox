use super::{soffice_convert, Job};
use crate::catalog::ToolId;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn process_each<F>(job: &Job, mut operation: F) -> Result<Vec<PathBuf>, String>
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

pub(super) fn process_each_capture<F>(job: &Job, mut operation: F) -> Result<Vec<PathBuf>, String>
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

pub(super) fn process_each_convert(job: &Job, target_ext: &str) -> Result<Vec<PathBuf>, String> {
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

pub(super) fn require_files(job: &Job, min: usize, message: &str) -> Result<(), String> {
    if job.files.len() < min { Err(message.into()) } else { Ok(()) }
}

pub(super) fn first_file(job: &Job) -> Result<&Path, String> {
    job.files.first().map(PathBuf::as_path).ok_or_else(|| "Add a file first.".to_string())
}

pub(super) fn output_for_input(job: &Job, input: &Path, suffix: &str, extension: &str) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    unique_path(&job.output_dir, &format!("{stem}-{suffix}"), extension)
}

pub(super) fn unique_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let ext = normalized_ext(extension);
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() { return first; }
    for index in 2..10_000 {
        let candidate = dir.join(format!("{stem}-{index}.{ext}"));
        if !candidate.exists() { return candidate; }
    }
    dir.join(format!("{stem}-{}.{}", timestamp(), ext))
}

pub(super) fn unique_prefix(dir: &Path, stem: &str) -> PathBuf {
    let base = dir.join(stem);
    if !base.exists() && files_with_prefix(dir, stem).is_empty() { return base; }
    for index in 2..10_000 {
        let candidate = dir.join(format!("{stem}-{index}"));
        if !candidate.exists() && files_with_prefix(dir, candidate.file_name().and_then(|s| s.to_str()).unwrap_or(stem)).is_empty() { return candidate; }
    }
    dir.join(format!("{stem}-{}", timestamp()))
}

pub(super) fn unique_pattern(dir: &Path, stem: &str) -> PathBuf {
    let name = unique_prefix(dir, &format!("{stem}-split"));
    dir.join(format!("{}-%d.pdf", name.file_stem().and_then(|s| s.to_str()).unwrap_or("split")))
}

pub(super) fn sort_numbered_paths(paths: &mut [PathBuf]) {
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

pub(super) fn files_with_prefix(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    fs::read_dir(dir).ok().into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| {
        p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with(prefix)).unwrap_or(false)
    }).collect()
}

pub(super) fn temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!("forgebox-{prefix}-{}-{}", std::process::id(), timestamp()));
    fs::create_dir_all(&path).map_err(display_io)?;
    Ok(path)
}

fn timestamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

pub(super) fn positive(value: &str, fallback: i32) -> i32 {
    value.trim().parse::<i32>().ok().filter(|n| *n > 0).unwrap_or(fallback)
}

pub(super) fn normalized_ext(value: &str) -> String {
    value.trim().trim_start_matches('.').to_lowercase()
}

pub(super) fn os<T: AsRef<std::ffi::OsStr>>(value: T) -> OsString {
    value.as_ref().to_os_string()
}

pub(super) fn display_io(error: io::Error) -> String { error.to_string() }
