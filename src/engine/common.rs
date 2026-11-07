use super::{check_cancelled, report_status, soffice_convert, Job};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
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
        (Tool::PdfToDoc, Options::None) => ("converted", "docx".into()),
        (Tool::ImageOcr, Options::ImageOcr { .. }) => ("ocr", "txt".into()),
        (Tool::AudioConvert, Options::AudioConvert { format, .. }) => ("converted", format.as_str().to_owned()),
        (Tool::ExtractAudio, Options::ExtractAudio { format }) => ("audio", format.as_str().to_owned()),
        (Tool::VideoConvert, Options::VideoConvert { format, .. }) => ("converted", format.as_str().to_owned()),
        (Tool::DavinciResolve, Options::DavinciResolve { .. }) => ("davinci-resolve", "mov".into()),
        (Tool::VideoToGif, Options::VideoToGif { .. }) => ("animation", "gif".into()),
        (Tool::VideoSticker, Options::VideoSticker { .. }) => ("sticker", "webm".into()),
        _ => ("output", source_extension()),
    }
}

fn check_cancelled_or_remove_outputs(outputs: &[PathBuf]) -> Result<(), String> {
    if let Err(error) = check_cancelled() {
        for output in outputs {
            let _ = fs::remove_file(output);
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn process_each<F>(job: &Job, mut operation: F) -> Result<Vec<PathBuf>, String>
where
    F: FnMut(&Path, &Path) -> Result<(), String>,
{
    require_files(job, 1, "Add files to process first.")?;
    let mut outputs: Vec<PathBuf> = Vec::new();
    for (index, input) in job.files.iter().enumerate() {
        check_cancelled_or_remove_outputs(&outputs)?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("file");
        report_status(format!("Processing {name} ({}/{})", index + 1, job.files.len()));
        let (suffix, extension) = output_details(job, input);
        let reservation = reserve_output_for_input(job, input, suffix, &extension)?;
        let out = reservation.path().to_path_buf();
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
        check_cancelled_or_remove_outputs(&outputs)?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("file");
        report_status(format!("Reading metadata from {name} ({}/{})", index + 1, job.files.len()));
        let reservation = reserve_output_for_input(job, input, "exif", "json")?;
        let out = reservation.path().to_path_buf();
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
        check_cancelled_or_remove_outputs(&outputs)?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("document");
        report_status(format!("Converting {name} ({}/{})", index + 1, job.files.len()));
        let reservation = reserve_output_for_input(job, input, "converted", target_ext)?;
        let out = reservation.path().to_path_buf();
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

pub(super) struct OutputReservation {
    path: PathBuf,
    lock_path: PathBuf,
}

impl OutputReservation {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for OutputReservation {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

pub(super) fn reserve_output_for_input(
    job: &Job,
    input: &Path,
    suffix: &str,
    extension: &str,
) -> Result<OutputReservation, String> {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    reserve_unique_path(&job.output_dir, &format!("{stem}-{suffix}"), extension)
}

pub(super) fn reserve_unique_path(
    dir: &Path,
    stem: &str,
    extension: &str,
) -> Result<OutputReservation, String> {
    let ext = normalized_ext(extension);
    for index in 1..=10_000 {
        let candidate_stem = if index == 1 { stem.to_owned() } else { format!("{stem}-{index}") };
        let path = dir.join(format!("{candidate_stem}.{ext}"));
        match fs::symlink_metadata(&path) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect output path {}: {error}", path.display())),
        }

        let file_name = path.file_name().ok_or_else(|| "Could not construct an output filename.".to_string())?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        file_name.hash(&mut hasher);
        let lock_path = dir.join(format!(".forgebox-output-{:016x}.lock", hasher.finish()));

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&lock_path) {
            Ok(lock) => drop(lock),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Could not reserve output name {}: {error}", path.display())),
        }

        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let _ = fs::remove_file(&lock_path);
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                let _ = fs::remove_file(&lock_path);
                return Err(format!("Could not inspect output path {}: {error}", path.display()));
            }
        }
        return Ok(OutputReservation { path, lock_path });
    }
    Err(format!("Could not reserve a unique output filename for {stem}.{ext} after 10,000 attempts."))
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
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

        match create_private_dir(&path) {
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&first).unwrap().permissions().mode() & 0o077, 0);
        }

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


#[cfg(test)]
mod cancellation_cleanup_tests {
    use super::{process_each, temp_dir};
    use crate::catalog::ToolId;
    use crate::engine::{
        with_execution_control, ExecutionControl, ImageFormat, Job, JobOptions,
    };
    use std::fs;
    use std::sync::mpsc;

    #[test]
    fn cancelling_between_files_removes_outputs_from_the_current_batch() {
        let directory = temp_dir("batch-cancel-cleanup-test").unwrap();
        let first_input = directory.join("first.png");
        let second_input = directory.join("second.png");
        fs::write(&first_input, b"first").unwrap();
        fs::write(&second_input, b"second").unwrap();

        let output_dir = directory.join("output");
        fs::create_dir_all(&output_dir).unwrap();
        let job = Job::new(
            ToolId::ImageConvert,
            vec![first_input, second_input],
            output_dir.clone(),
            JobOptions::ImageConvert {
                format: ImageFormat::Png,
            },
        )
        .unwrap();

        let (status_tx, _status_rx) = mpsc::channel();
        let control = ExecutionControl::new(status_tx);
        let cancel_handle = control.clone();
        let result = with_execution_control(control, || {
            process_each(&job, |_input, output| {
                fs::write(output, b"completed output").map_err(|error| error.to_string())?;
                cancel_handle.cancel();
                Ok(())
            })
        });

        assert!(result.unwrap_err().contains("cancelled"));
        assert_eq!(
            fs::read_dir(&output_dir).unwrap().count(),
            0,
            "a cancelled batch must not leave files from earlier inputs"
        );

        fs::remove_dir_all(directory).unwrap();
    }
}



#[cfg(test)]
mod output_reservation_tests {
    use super::{reserve_unique_path, temp_dir};
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn concurrent_reservations_never_return_the_same_path() {
        let directory = temp_dir("output-reservation-test").unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let workers = (0..8)
            .map(|_| {
                let directory = directory.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    let reservation = reserve_unique_path(&directory, "result", "pdf").unwrap();
                    let path = reservation.path().to_path_buf();
                    (reservation, path)
                })
            })
            .collect::<Vec<_>>();

        let mut reservations = Vec::new();
        let mut paths = Vec::new();
        for worker in workers {
            let (reservation, path) = worker.join().unwrap();
            reservations.push(reservation);
            paths.push(path);
        }

        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), 8);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 8);

        drop(reservations);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn existing_files_and_symlinks_are_never_reserved() {
        let directory = temp_dir("output-reservation-existing-test").unwrap();
        fs::write(directory.join("result.pdf"), b"keep").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink("missing-target", directory.join("result-2.pdf")).unwrap();

        let reservation = reserve_unique_path(&directory, "result", "pdf").unwrap();
        #[cfg(unix)]
        assert_eq!(reservation.path().file_name().unwrap(), "result-3.pdf");
        #[cfg(not(unix))]
        assert_eq!(reservation.path().file_name().unwrap(), "result-2.pdf");
        assert_eq!(fs::read(directory.join("result.pdf")).unwrap(), b"keep");

        drop(reservation);
        fs::remove_dir_all(directory).unwrap();
    }
}
