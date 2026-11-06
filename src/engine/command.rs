use super::record_execution_warning;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn os<T: AsRef<OsStr>>(value: T) -> OsString {
    value.as_ref().to_os_string()
}

static IMAGEMAGICK_PROGRAM: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

pub(crate) fn image_magick_program() -> &'static str {
    *IMAGEMAGICK_PROGRAM.get_or_init(|| {
        if has_command("magick") { "magick" } else { "convert" }
    })
}

/// Reserve at least one available CPU thread for the desktop UI and the rest
/// of the system. Cap ImageMagick's parallelism to avoid saturating low-core CPUs.
pub(crate) fn image_magick_thread_limit() -> usize {
    recommended_image_magick_threads(
        std::thread::available_parallelism()
            .map(|parallelism| parallelism.get())
            .unwrap_or(1),
    )
}

pub(super) fn recommended_image_magick_threads(available: usize) -> usize {
    available.saturating_sub(1).clamp(1, 4)
}

pub(super) fn run_magick_owned(args: Vec<OsString>) -> Result<(), String> {
    let mut limited_args = Vec::with_capacity(args.len() + 3);
    limited_args.extend([
        os("-limit"),
        os("thread"),
        os(image_magick_thread_limit().to_string()),
    ]);
    limited_args.extend(args);
    run_ok(image_magick_program(), limited_args).map(|_| ())
}

fn command_candidates(program: &str) -> Vec<OsString> {
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
            candidates.push(PathBuf::from(home).join(".local/bin/rembg").into_os_string());
        }
        candidates.push(OsString::from(program));
        candidates
    } else {
        vec![OsString::from(program)]
    }
}

/// Do not leak an AppImage's bundled Python paths into the optional
/// rembg virtual environment. Python uses PYTHONHOME/PYTHONPATH before the
/// venv's own stdlib and can fail before importing the CLI.
pub(super) fn configure_command_environment(program: &str, command: &mut Command) {
    if program == "rembg" {
        command.env_remove("PYTHONHOME");
        command.env_remove("PYTHONPATH");
    }
}

pub(super) fn run_ok(program: &str, args: Vec<OsString>) -> Result<Output, String> {
    let candidates = command_candidates(program);
    let mut tried: Vec<String> = Vec::new();
    for candidate in candidates {
        let label = candidate.to_string_lossy().into_owned();
        let mut command = Command::new(&candidate);
        configure_command_environment(program, &mut command);
        match command.args(args.iter()).output() {
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
        ("tesseract", "macos") => {
            " Install Tesseract and make sure its executable is available on PATH."
        }
        ("rembg", "linux") => {
            " Install the optional rembg CPU backend in a Python 3.11–3.13 environment. Fedora RPM users can run: bash /usr/share/doc/forgebox/install-rembg-fedora.sh."
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

pub(super) fn run_capture(program: &str, args: Vec<OsString>) -> Result<Vec<u8>, String> {
    Ok(run_ok(program, args)?.stdout)
}

/// qpdf uses exit code 3 for recoverable warnings. Treat warnings as a
/// successful operation while leaving their messages available on stderr.
pub(super) fn run_qpdf(args: Vec<OsString>) -> Result<Output, String> {
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

pub(super) fn command_version_succeeds(candidate: &OsStr) -> bool {
    Command::new(candidate)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn has_command(program: &str) -> bool {
    command_candidates(program)
        .iter()
        .any(|candidate| command_version_succeeds(candidate))
}

