use super::{check_cancelled, report_status, soffice_convert, Job};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn processed_image_extension(input: &Path) -> String {
    let extension = input.extension().and_then(|value| value.to_str()).unwrap_or("png");
    match extension.to_ascii_lowercase().as_str() {
        // ImageMagick can decode HEIC/HEIF on supported builds, but HEIF
        // encoders are commonly absent. Produce a portable PNG instead.
        "heic" | "heif" => "png".into(),
        other => other.into(),
    }
}

fn output_details(job: &Job, input: &Path) -> (&'static str, String) {
    use super::JobOptions as Options;
    use crate::catalog::ToolId as Tool;

    let source_extension = || {
        input.extension().and_then(|value| value.to_str()).unwrap_or("png").to_lowercase()
    };
    match (&job.tool, &job.options) {
        (Tool::ImageConvert, Options::ImageConvert { format }) => ("converted", format.as_str().to_owned()),
        (Tool::ImageResize, Options::ImageResize { .. }) => ("resized", processed_image_extension(input)),
        (Tool::ImageCompress, Options::ImageCompress { .. }) => ("compressed", processed_image_extension(input)),
        (Tool::ImageGrayscale, Options::None) => ("grayscale", processed_image_extension(input)),
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

fn check_batch_cancelled(outputs: &[PathBuf], output_dir: &Path) -> Result<(), String> {
    check_cancelled().map_err(|error| {
        if outputs.is_empty() {
            error
        } else {
            format!(
                "{error} {} completed output(s) were kept in {}.",
                outputs.len(),
                output_dir.display()
            )
        }
    })
}

fn ensure_output_file(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("The converter did not create the expected output file: {error}"))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("The converter did not create a non-empty output file.".into());
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
        check_batch_cancelled(&outputs, &job.output_dir)?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("file");
        report_status(format!("Processing {name} ({}/{})", index + 1, job.files.len()));
        let (suffix, extension) = output_details(job, input);
        let reservation = reserve_output_for_input(job, input, suffix, &extension)?;
        let out = reservation.path().to_path_buf();
        if let Err(error) = operation(input, &out) {
            let _ = fs::remove_file(&out);
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            return Err(if outputs.is_empty() {
                format!(r#"Failed to process "{name}": {error}"#)
            } else {
                format!(r#"Failed to process "{name}": {error}. {} earlier completed output(s) were kept in {}."#, outputs.len(), job.output_dir.display())
            });
        }
        if let Err(error) = ensure_output_file(&out) {
            let _ = fs::remove_file(&out);
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            return Err(format!(r#"Failed to process "{name}": {error}. Earlier completed outputs, if any, were kept."#));
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
        check_batch_cancelled(&outputs, &job.output_dir)?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("file");
        report_status(format!("Reading metadata from {name} ({}/{})", index + 1, job.files.len()));
        let reservation = reserve_output_for_input(job, input, "exif", "json")?;
        let out = reservation.path().to_path_buf();
        if let Err(error) = operation(input, &out) {
            let _ = fs::remove_file(&out);
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            return Err(if outputs.is_empty() {
                format!(r#"Failed to read metadata from "{name}": {error}"#)
            } else {
                format!(r#"Failed to read metadata from "{name}": {error}. {} earlier completed output(s) were kept in {}."#, outputs.len(), job.output_dir.display())
            });
        }
        if let Err(error) = ensure_output_file(&out) {
            let _ = fs::remove_file(&out);
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            return Err(format!(r#"Failed to read metadata from "{name}": {error}. Earlier completed outputs, if any, were kept."#));
        }
        outputs.push(out);
    }
    Ok(outputs)
}

pub(super) fn process_each_convert(job: &Job, target_ext: &str) -> Result<Vec<PathBuf>, String> {
    require_files(job, 1, "Add documents first.")?;
    let mut outputs: Vec<PathBuf> = Vec::new();
    for (index, input) in job.files.iter().enumerate() {
        check_batch_cancelled(&outputs, &job.output_dir)?;
        let name = input.file_name().and_then(|value| value.to_str()).unwrap_or("document");
        report_status(format!("Converting {name} ({}/{})", index + 1, job.files.len()));
        let reservation = reserve_output_for_input(job, input, "converted", target_ext)?;
        let out = reservation.path().to_path_buf();
        if let Err(error) = soffice_convert(input, target_ext, &out) {
            let _ = fs::remove_file(&out);
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("document");
            return Err(if outputs.is_empty() {
                format!(r#"Failed to convert "{name}": {error}"#)
            } else {
                format!(r#"Failed to convert "{name}": {error}. {} earlier completed output(s) were kept in {}."#, outputs.len(), job.output_dir.display())
            });
        }
        if let Err(error) = ensure_output_file(&out) {
            let _ = fs::remove_file(&out);
            let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("document");
            return Err(format!(r#"Failed to convert "{name}": {error}. Earlier completed outputs, if any, were kept."#));
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
        #[cfg(target_os = "linux")]
        if let Some(parent) = self.lock_path.parent() {
            remove_reservation_lock(parent, &self.lock_path);
        }
        #[cfg(not(target_os = "linux"))]
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
        let file_name = path.file_name().ok_or_else(|| "Could not construct an output filename.".to_string())?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        file_name.hash(&mut hasher);
        let hash = hasher.finish();

        #[cfg(target_os = "linux")]
        {
            let _gate = reservation_gate(dir)?;
            match fs::symlink_metadata(&path) {
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("Could not inspect output path {}: {error}", path.display())),
            }
            let lock_prefix = format!(".forgebox-output-{hash:016x}-");
            let legacy_lock = dir.join(format!(".forgebox-output-{hash:016x}.lock"));
            let Some(lock_path) = reserve_lock_under_gate(dir, &lock_prefix, &legacy_lock)? else {
                continue;
            };
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

        #[cfg(not(target_os = "linux"))]
        {
            match fs::symlink_metadata(&path) {
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("Could not inspect output path {}: {error}", path.display())),
            }
            let lock_path = dir.join(format!(".forgebox-output-{hash:016x}.lock"));
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

pub(super) struct PrefixReservation {
    path: PathBuf,
    lock_path: PathBuf,
}

impl PrefixReservation {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrefixReservation {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(parent) = self.lock_path.parent() {
            remove_reservation_lock(parent, &self.lock_path);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = fs::remove_file(&self.lock_path);
    }
}

pub(super) fn reserve_unique_prefix(
    dir: &Path,
    stem: &str,
) -> Result<PrefixReservation, String> {
    for index in 1..=10_000 {
        let candidate = if index == 1 { stem.to_owned() } else { format!("{stem}-{index}") };
        let path = dir.join(&candidate);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("prefix:{candidate}").hash(&mut hasher);
        let hash = hasher.finish();

        #[cfg(target_os = "linux")]
        {
            let _gate = reservation_gate(dir)?;
            if fs::symlink_metadata(&path).is_ok() || !files_with_prefix(dir, &candidate).is_empty() {
                continue;
            }
            let lock_prefix = format!(".forgebox-prefix-{hash:016x}-");
            let legacy_lock = dir.join(format!(".forgebox-prefix-{hash:016x}.lock"));
            let Some(lock_path) = reserve_lock_under_gate(dir, &lock_prefix, &legacy_lock)? else {
                continue;
            };
            if fs::symlink_metadata(&path).is_ok() || !files_with_prefix(dir, &candidate).is_empty() {
                let _ = fs::remove_file(&lock_path);
                continue;
            }
            return Ok(PrefixReservation { path, lock_path });
        }

        #[cfg(not(target_os = "linux"))]
        {
            if fs::symlink_metadata(&path).is_ok() || !files_with_prefix(dir, &candidate).is_empty() {
                continue;
            }
            let lock_path = dir.join(format!(".forgebox-prefix-{hash:016x}.lock"));
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
                Err(error) => return Err(format!("Could not reserve output prefix {candidate}: {error}")),
            }
            if fs::symlink_metadata(&path).is_ok() || !files_with_prefix(dir, &candidate).is_empty() {
                let _ = fs::remove_file(&lock_path);
                continue;
            }
            return Ok(PrefixReservation { path, lock_path });
        }
    }
    Err(format!("Could not reserve a unique output prefix for {stem} after 10,000 attempts."))
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
    temp_dir_in(&std::env::temp_dir(), prefix)
}

pub(super) fn temp_dir_for_output(output: &Path, prefix: &str) -> Result<PathBuf, String> {
    let base = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    temp_dir_in(base, prefix)
}

pub(super) fn temp_dir_in(base: &Path, prefix: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(base).map_err(display_io)?;
    #[cfg(target_os = "linux")]
    remove_stale_temp_dirs(base);

    // Include the process start time as well as its PID. A reused PID must not
    // make an abandoned workspace look active after an application crash.
    let process_start = current_process_start_time().map_err(display_io)?;
    for _ in 0..128 {
        let sequence = TEMP_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!(
            ".forgebox-{prefix}-{}-{process_start}-{}-{sequence}",
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

const STALE_ARTIFACT_GRACE_MS: u128 = 24 * 60 * 60 * 1_000;

#[cfg(target_os = "linux")]
fn process_identity(pid: u32) -> io::Result<Option<(char, u64)>> {
    let stat = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    // The command name inside /proc/<pid>/stat is parenthesized and may itself
    // contain spaces or parentheses. Parse fields only after its final ')'.
    let fields = stat
        .rsplit_once(')')
        .map(|(_, fields)| fields)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Malformed process stat record"))?;
    let mut fields = fields.split_whitespace();
    let state = fields
        .next()
        .and_then(|field| field.chars().next())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing process state"))?;
    // After consuming field 3 (state), field 22 (starttime) is 18 positions on.
    let start_time = fields
        .nth(18)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing process start time"))?
        .parse::<u64>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some((state, start_time)))
}

#[cfg(target_os = "linux")]
fn current_process_start_time() -> io::Result<u64> {
    process_identity(std::process::id())?
        .map(|(_, start_time)| start_time)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Current process is missing from /proc"))
}

#[cfg(not(target_os = "linux"))]
fn current_process_start_time() -> io::Result<u64> {
    Ok(0)
}

#[cfg(target_os = "linux")]
fn process_instance_is_alive(pid: u32, expected_start_time: u64) -> io::Result<bool> {
    Ok(matches!(
        process_identity(pid)?,
        Some((state, start_time)) if start_time == expected_start_time && state != 'Z' && state != 'X'
    ))
}

#[cfg(target_os = "linux")]
fn parse_reservation_owner(name: &str) -> Option<(u32, u64, u128)> {
    let stem = name.strip_suffix(".lock")?;
    let mut parts = stem.rsplitn(4, '-');
    let created_at = parts.next()?.parse::<u128>().ok()?;
    let start_time = parts.next()?.parse::<u64>().ok()?;
    let pid = parts.next()?.parse::<u32>().ok()?;
    let prefix = parts.next()?;
    (prefix.starts_with(".forgebox-output-") || prefix.starts_with(".forgebox-prefix-"))
        .then_some((pid, start_time, created_at))
}

#[cfg(target_os = "linux")]
struct ReservationGate {
    file: File,
}

#[cfg(target_os = "linux")]
impl Drop for ReservationGate {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(target_os = "linux")]
fn reservation_gate(dir: &Path) -> Result<ReservationGate, String> {
    use std::os::unix::fs::OpenOptionsExt;

    let path = dir.join(".forgebox-reservations.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    let file = options.open(&path)
        .map_err(|error| format!("Could not open output reservation gate {}: {error}", path.display()))?;
    if !file.metadata().map_err(|error| error.to_string())?.is_file() {
        return Err(format!("Output reservation gate {} is not a regular file.", path.display()));
    }

    loop {
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result == 0 {
            return Ok(ReservationGate { file });
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("Could not lock output reservation gate {}: {error}", path.display()));
        }
    }
}

#[cfg(target_os = "linux")]
fn remove_stale_reservation_locks(dir: &Path, lock_prefix: &str) -> Result<bool, String> {
    let now = timestamp();
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("Could not inspect output reservation folder {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not inspect output reservation entry: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(lock_prefix) || !name.ends_with(".lock") {
            continue;
        }
        if !entry.file_type().map_err(|error| error.to_string())?.is_file() {
            // Never follow or remove unexpected links/directories in a user-selected folder.
            return Ok(false);
        }
        let Some((pid, start_time, created_at)) = parse_reservation_owner(name) else {
            // Legacy or malformed lock: its owner cannot be proven dead, so preserve it.
            return Ok(false);
        };
        if now.saturating_sub(created_at) < STALE_ARTIFACT_GRACE_MS {
            return Ok(false);
        }
        let alive = process_instance_is_alive(pid, start_time)
            .unwrap_or(true); // Uncertainty means keep the reservation.
        if alive {
            return Ok(false);
        }
        fs::remove_file(entry.path())
            .map_err(|error| format!("Could not remove stale reservation {}: {error}", entry.path().display()))?;
    }
    Ok(true)
}

#[cfg(target_os = "linux")]
fn reserve_lock_under_gate(
    dir: &Path,
    lock_prefix: &str,
    legacy_lock_path: &Path,
) -> Result<Option<PathBuf>, String> {
    if fs::symlink_metadata(legacy_lock_path).is_ok() {
        // The old lock format has no process identity. Leave it alone rather than
        // risking deletion while an older ForgeBox process still owns it.
        return Ok(None);
    }
    if !remove_stale_reservation_locks(dir, lock_prefix)? {
        return Ok(None);
    }

    use std::os::unix::fs::OpenOptionsExt;
    let pid = std::process::id();
    let start_time = current_process_start_time().map_err(|error| {
        format!("Could not identify the current process for output reservation: {error}")
    })?;
    let created_at = timestamp();
    let path = dir.join(format!("{lock_prefix}{pid}-{start_time}-{created_at}.lock"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    match options.open(&path) {
        Ok(file) => {
            drop(file);
            Ok(Some(path))
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(format!("Could not reserve an output name with {}: {error}", path.display())),
    }
}

#[cfg(target_os = "linux")]
fn remove_reservation_lock(dir: &Path, path: &Path) {
    if let Ok(_gate) = reservation_gate(dir) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(target_os = "linux")]
fn parse_temp_dir_owner(name: &str) -> Option<(u32, u64, u128)> {
    let stem = name.strip_prefix(".forgebox-")?;
    let mut parts = stem.rsplitn(5, '-');
    let _sequence = parts.next()?.parse::<u64>().ok()?;
    let created_at = parts.next()?.parse::<u128>().ok()?;
    let start_time = parts.next()?.parse::<u64>().ok()?;
    let pid = parts.next()?.parse::<u32>().ok()?;
    let prefix = parts.next()?;
    (!prefix.is_empty()).then_some((pid, start_time, created_at))
}

#[cfg(target_os = "linux")]
fn remove_stale_temp_dirs(base: &Path) {
    let now = timestamp();
    let Ok(entries) = fs::read_dir(base) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some((pid, start_time, created_at)) = parse_temp_dir_owner(name) else { continue };
        if now.saturating_sub(created_at) < STALE_ARTIFACT_GRACE_MS {
            continue;
        }
        let Ok(file_type) = entry.file_type() else { continue };
        if !file_type.is_dir() {
            continue;
        }
        if process_instance_is_alive(pid, start_time).unwrap_or(true) {
            continue;
        }
        // The 24-hour grace period also gives orphaned converter helpers time
        // to exit after their parent application was forcibly terminated.
        let _ = fs::remove_dir_all(entry.path());
    }
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
    fn cancelling_between_files_keeps_completed_outputs() {
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
        let outputs = fs::read_dir(&output_dir).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(outputs.len(), 1, "the first completed output must be preserved");
        assert_eq!(fs::read(outputs[0].path()).unwrap(), b"completed output");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_later_file_failure_keeps_earlier_completed_outputs() {
        let directory = temp_dir("batch-failure-preserve-test").unwrap();
        let first_input = directory.join("first.png");
        let second_input = directory.join("second.png");
        fs::write(&first_input, b"first").unwrap();
        fs::write(&second_input, b"second").unwrap();

        let output_dir = directory.join("output");
        fs::create_dir_all(&output_dir).unwrap();
        let job = Job::new(
            ToolId::ImageConvert,
            vec![first_input.clone(), second_input.clone()],
            output_dir.clone(),
            JobOptions::ImageConvert { format: ImageFormat::Png },
        ).unwrap();

        let result = process_each(&job, |input, output| {
            if input == second_input {
                return Err("simulated second-file failure".into());
            }
            fs::write(output, b"first completed output").map_err(|error| error.to_string())
        });

        assert!(result.unwrap_err().contains("simulated second-file failure"));
        let outputs = fs::read_dir(&output_dir).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(outputs.len(), 1, "the first output should remain after a later failure");
        assert_eq!(fs::read(outputs[0].path()).unwrap(), b"first completed output");

        fs::remove_dir_all(directory).unwrap();
    }
}



#[cfg(test)]
mod output_reservation_tests {
    use super::{reserve_unique_path, temp_dir};
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn reservation_entry_count(directory: &std::path::Path) -> usize {
        fs::read_dir(directory).unwrap().filter_map(Result::ok)
            .filter(|entry| entry.file_name() != ".forgebox-reservations.lock")
            .count()
    }

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
        assert_eq!(reservation_entry_count(&directory), 8);

        drop(reservations);
        assert_eq!(reservation_entry_count(&directory), 0);
        fs::remove_dir_all(directory).unwrap();
    }


    #[cfg(target_os = "linux")]
    #[test]
    fn abandoned_reservation_lock_is_reclaimed_after_owner_exits() {
        use std::hash::{Hash, Hasher};

        let directory = temp_dir("stale-output-reservation-test").unwrap();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        "result.pdf".hash(&mut hasher);
        let hash = hasher.finish();
        let old = super::super::timestamp().saturating_sub(
            super::super::STALE_ARTIFACT_GRACE_MS + 1,
        );
        let stale = directory.join(format!(
            ".forgebox-output-{hash:016x}-{}-1-{old}.lock",
            u32::MAX,
        ));
        fs::write(&stale, b"").unwrap();

        let reservation = reserve_unique_path(&directory, "result", "pdf").unwrap();
        assert!(!stale.exists(), "a dead owner's aged reservation should be removed");
        assert_eq!(reservation.path().file_name().unwrap(), "result.pdf");
        drop(reservation);
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


#[cfg(test)]
mod prefix_reservation_tests {
    use super::{reserve_unique_prefix, temp_dir};
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn reservation_entry_count(directory: &std::path::Path) -> usize {
        fs::read_dir(directory).unwrap().filter_map(Result::ok)
            .filter(|entry| entry.file_name() != ".forgebox-reservations.lock")
            .count()
    }

    #[test]
    fn concurrent_multi_output_prefixes_are_reserved_independently() {
        let directory = temp_dir("prefix-reservation-test").unwrap();
        let barrier = Arc::new(Barrier::new(6));
        let workers = (0..6)
            .map(|_| {
                let directory = directory.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    let reservation = reserve_unique_prefix(&directory, "pages").unwrap();
                    let path = reservation.path().to_path_buf();
                    (reservation, path)
                })
            })
            .collect::<Vec<_>>();

        let mut reservations = Vec::new();
        let mut prefixes = Vec::new();
        for worker in workers {
            let (reservation, prefix) = worker.join().unwrap();
            reservations.push(reservation);
            prefixes.push(prefix);
        }

        prefixes.sort();
        prefixes.dedup();
        assert_eq!(prefixes.len(), 6);
        assert_eq!(reservation_entry_count(&directory), 6);

        drop(reservations);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        fs::remove_dir_all(directory).unwrap();
    }
}





#[cfg(test)]
mod temp_dir_in_tests {
    use super::{temp_dir_for_output, temp_dir_in};
    use std::fs;


    #[cfg(target_os = "linux")]
    #[test]
    fn abandoned_temporary_workspace_is_reclaimed_after_owner_exits() {
        use super::super::{temp_dir_in, timestamp, STALE_ARTIFACT_GRACE_MS};
        use std::fs;

        let base = std::env::temp_dir().join(format!(
            "forgebox-stale-workspace-test-{}",
            std::process::id(),
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let old = timestamp().saturating_sub(STALE_ARTIFACT_GRACE_MS + 1);
        let stale = base.join(format!(".forgebox-abandoned-{}-1-{old}-0", u32::MAX));
        fs::create_dir(&stale).unwrap();
        fs::write(stale.join("partial.dat"), b"orphaned").unwrap();

        let current = temp_dir_in(&base, "cleanup-trigger").unwrap();
        assert!(!stale.exists(), "a dead owner's aged workspace should be removed");
        assert!(current.is_dir());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn private_workspace_can_be_created_under_destination_directory() {
        let base = std::env::temp_dir().join(format!("forgebox-temp-in-test-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let workspace = temp_dir_in(&base, "images-pdf").unwrap();
        assert_eq!(workspace.parent(), Some(base.as_path()));
        assert!(workspace.file_name().unwrap().to_string_lossy().starts_with(".forgebox-images-pdf-"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&workspace).unwrap().permissions().mode() & 0o077, 0);
        }
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn output_workspace_is_created_beside_the_destination() {
        let base = std::env::temp_dir().join(format!("forgebox-output-temp-test-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let output = base.join("converted.xlsx");
        let workspace = temp_dir_for_output(&output, "office-convert").unwrap();
        assert_eq!(workspace.parent(), Some(base.as_path()));
        assert!(workspace.file_name().unwrap().to_string_lossy().starts_with(".forgebox-office-convert-"));
        fs::remove_dir_all(&base).unwrap();
    }
}


#[cfg(test)]
mod processed_image_extension_tests {
    use super::processed_image_extension;
    use std::path::Path;

    #[test]
    fn heif_sources_are_saved_as_portable_png_for_in_place_image_tools() {
        assert_eq!(processed_image_extension(Path::new("photo.heic")), "png");
        assert_eq!(processed_image_extension(Path::new("photo.HEIF")), "png");
    }

    #[test]
    fn other_image_formats_keep_their_extension() {
        assert_eq!(processed_image_extension(Path::new("photo.jpg")), "jpg");
        assert_eq!(processed_image_extension(Path::new("scan.tiff")), "tiff");
    }
}
