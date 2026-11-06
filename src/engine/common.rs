use super::{check_cancelled, report_status, soffice_convert, Job};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn output_details(job: &Job, input: &Path) -> (&'static str, String) {
    use super::JobOptions as Options;
    use crate::catalog::ToolId as Tool;

    let source_extension = || {
        input.extension().and_then(|value| value.to_str()).unwrap_or("png").to_lowercase()
    };
    match (&job.tool, &job.options) {
        (Tool::ImageConvert, Options::ImageConvert { format }) => ("converted", format.as_str().to_owned()),
        (Tool::ImageResize, Options::ImageResize { .. }) => ("resized", source_extension()),
        (Tool::ImageCompress, Options::ImageCompress { .. }) => ("compressed", source_extension()),
        (Tool::ImageGrayscale, Options::None) => ("grayscale", source_extension()),
        (Tool::RemoveBackground, Options::None) => ("no-background", "png".into()),
        (Tool::ExtractExif, Options::None) => ("exif", "json".into()),
        (Tool::ImageOcr, Options::ImageOcr { .. }) => ("ocr", "txt".into()),
        (Tool::ExtractAudio, Options::ExtractAudio { format }) => ("audio", format.as_str().to_owned()),
        (Tool::VideoConvert, Options::VideoConvert { format, .. }) => ("converted", format.as_str().to_owned()),
        (Tool::VideoToGif, Options::VideoToGif { .. }) => ("animation", "gif".into()),
        _ => ("output", source_extension()),
    }
}

pub(super) fn process_each<F>(job: &Job, mut operation: F) -> Result<Vec<PathBuf>, String>
where
    F: FnMut(&Path, &Path) -> Result<(), String>,
{
    require_files(job, 1, "Add files to process first.")?;
    let mut outputs: Vec<PathBuf> = Vec::new();
    for (index, input) in job.files.iter().enumerate() {
        check_cancelled()?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("file");
        report_status(format!("Processing {name} ({}/{})", index + 1, job.files.len()));
        let (suffix, extension) = output_details(job, input);
        let out = output_for_input(job, input, suffix, &extension);
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
    for (index, input) in job.files.iter().enumerate() {
        check_cancelled()?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("file");
        report_status(format!("Reading metadata from {name} ({}/{})", index + 1, job.files.len()));
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
    for (index, input) in job.files.iter().enumerate() {
        check_cancelled()?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("document");
        report_status(format!("Converting {name} ({}/{})", index + 1, job.files.len()));
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
    let base = std::env::temp_dir();

    // A millisecond timestamp alone can collide when two conversions create
    // temporary directories at nearly the same time. Use an in-process
    // sequence and create the directory atomically rather than accepting an
    // already-existing path.
    for _ in 0..128 {
        let sequence = TEMP_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!(
            "forgebox-{prefix}-{}-{}-{sequence}",
            std::process::id(),
            timestamp()
        ));

        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(display_io(error)),
        }
    }

    Err("Could not create a unique temporary directory after 128 attempts.".into())
}

#[cfg(test)]
mod temp_dir_tests {
    use super::temp_dir;
    use std::fs;

    #[test]
    fn temporary_directories_with_same_prefix_are_unique() {
        let first = temp_dir("unique-test").unwrap();
        let second = temp_dir("unique-test").unwrap();

        assert_ne!(first, second);
        assert!(first.is_dir());
        assert!(second.is_dir());

        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }
}

fn timestamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

pub(super) fn positive(value: &str, fallback: i32) -> i32 {
    value.trim().parse::<i32>().ok().filter(|number| *number > 0).unwrap_or(fallback)
}

pub(super) fn normalized_ext(value: &str) -> String {
    value.trim().trim_start_matches('.').to_lowercase()
}

pub(super) fn os<T: AsRef<std::ffi::OsStr>>(value: T) -> OsString {
    value.as_ref().to_os_string()
}

pub(super) fn display_io(error: io::Error) -> String { error.to_string() }
