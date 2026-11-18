use super::record_execution_warning;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

fn os<T: AsRef<OsStr>>(value: T) -> OsString {
    value.as_ref().to_os_string()
}

pub(crate) fn image_magick_program() -> &'static str {
    // Probe each time so an installation or repair made while ForgeBox is
    // running is picked up without requiring an application restart.
    if has_command("magick") { "magick" } else { "convert" }
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

fn run_cancellable(command: &mut Command, timeout: Option<Duration>) -> io::Result<Output> {
    super::check_cancelled()
        .map_err(|message| io::Error::new(io::ErrorKind::Interrupted, message))?;

    // Isolate each external utility in its own process group. Some tools
    // launch helpers (LibreOffice, ImageMagick delegates); killing only the
    // direct child can leave descendants holding our output pipes open.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("stdout was configured as piped");
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    let process_id = child.id();
    let stdout_reader = read_pipe(stdout);
    let stderr_reader = read_pipe(stderr);
    let started = Instant::now();

    let status = loop {
        if timeout.map(|limit| started.elapsed() >= limit).unwrap_or(false) {
            stop_child(&mut child);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "External command exceeded its time limit."));
        }
        if super::execution_cancelled() {
            stop_child(&mut child);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Operation cancelled by user."));
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                stop_child(&mut child);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(error);
            }
        }
    };

    // Normally both pipes reach EOF immediately after the command exits.
    // A helper that inherited stdout/stderr can otherwise keep our reader
    // threads blocked forever even though the main child has already exited.
    let pipe_drain_started = Instant::now();
    while !stdout_reader.is_finished() || !stderr_reader.is_finished() {
        let timed_out = match timeout {
            Some(limit) => started.elapsed() >= limit,
            None => pipe_drain_started.elapsed() >= Duration::from_secs(2),
        };
        if timed_out {
            if timeout.is_some() {
                stop_child(&mut child);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "External command exceeded its time limit."));
            }

            #[cfg(unix)]
            {
                stop_process_group(process_id);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
            }
            #[cfg(not(unix))]
            {
                // The main process has exited, but this platform has no
                // process-tree termination in the standard library. Detach
                // the readers rather than blocking the application forever.
                drop(stdout_reader);
                drop(stderr_reader);
            }
            return Err(io::Error::other(
                "A helper process kept the command output pipes open after the main process exited.",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let stdout = join_pipe(stdout_reader)?;
    let stderr = join_pipe(stderr_reader)?;
    Ok(Output { status, stdout, stderr })
}

fn read_pipe<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_pipe(reader: JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    reader.join().map_err(|_| io::Error::new(io::ErrorKind::Other, "Process output reader panicked"))?
}

fn stop_process_group(process_id: u32) {
    #[cfg(unix)]
    {
        // process_group(0) above makes the child PID its process-group ID.
        // Signal the entire group so delegated helpers cannot outlive a cancel.
        let process_group = -(process_id as libc::pid_t);
        let _ = unsafe { libc::kill(process_group, libc::SIGKILL) };
    }
    #[cfg(not(unix))]
    let _ = process_id;
}

fn stop_child(child: &mut Child) {
    #[cfg(unix)]
    stop_process_group(child.id());

    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }

    let _ = child.wait();
}

pub(super) fn run_ok(program: &str, args: Vec<OsString>) -> Result<Output, String> {
    run_ok_internal(program, args, None)
}

pub(super) fn run_ok_with_timeout(
    program: &str,
    args: Vec<OsString>,
    timeout: Duration,
) -> Result<Output, String> {
    run_ok_internal(program, args, Some(timeout))
}

fn run_ok_internal(
    program: &str,
    args: Vec<OsString>,
    timeout: Option<Duration>,
) -> Result<Output, String> {
    super::check_cancelled()?;
    super::report_status(format!("Running {program}…"));
    let candidates = command_candidates(program);
    let mut tried: Vec<String> = Vec::new();
    for candidate in candidates {
        let label = candidate.to_string_lossy().into_owned();
        let mut command = Command::new(&candidate);
        configure_command_environment(program, &mut command);
        command.args(args.iter());
        match run_cancellable(&mut command, timeout) {
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
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                return Err("Operation cancelled by user.".into());
            }
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                return Err(format!("Command `{program}` timed out after {} seconds. The process was stopped.", timeout.map(|value| value.as_secs()).unwrap_or_default()));
            }
            Err(error) => return Err(format!("Failed to run `{program}` via `{label}`: {error}")),
        }
    }
    let install_hint = match (program, std::env::consts::OS) {
        ("tesseract", "linux") => {
            " Install Tesseract and the matching language data with your distribution's package manager. On Fedora: sudo dnf install tesseract tesseract-langpack-eng."
        }
        ("img2pdf", "linux") => {
            " Install img2pdf with your distribution's package manager. Fedora: sudo dnf install python3-img2pdf; Debian/Ubuntu: sudo apt install img2pdf; Arch: sudo pacman -S img2pdf."
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

const COMMAND_VERSION_TIMEOUT: Duration = Duration::from_secs(3);

fn command_version_succeeds_with_timeout(candidate: &OsStr, timeout: Duration) -> bool {
    let mut command = Command::new(candidate);
    command.arg("--version");
    run_cancellable(&mut command, Some(timeout))
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub(super) fn command_version_succeeds(candidate: &OsStr) -> bool {
    command_version_succeeds_with_timeout(candidate, COMMAND_VERSION_TIMEOUT)
}

fn has_command(program: &str) -> bool {
    command_candidates(program)
        .iter()
        .any(|candidate| command_version_succeeds(candidate))
}

#[cfg(all(test, unix))]
mod cancellation_tests {
    use super::{os, run_ok};
    use crate::engine::{with_execution_control, ExecutionControl};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn cancellation_kills_descendants_that_inherit_process_pipes() {
        let (status_tx, _status_rx) = mpsc::channel();
        let control = ExecutionControl::new(status_tx);
        let cancel_handle = control.clone();

        let worker = thread::spawn(move || {
            with_execution_control(control, || {
                run_ok("sh", vec![os("-c"), os("sleep 2 & wait")])
            })
        });

        thread::sleep(Duration::from_millis(150));
        let started = Instant::now();
        cancel_handle.cancel();
        let result = worker.join().expect("command worker should not panic");

        assert!(result.unwrap_err().contains("cancelled"));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "cancellation should also close pipes held by child processes"
        );
    }

    #[test]
    fn cancellation_stops_a_running_external_command() {
        let (status_tx, _status_rx) = mpsc::channel();
        let control = ExecutionControl::new(status_tx);
        let cancel_handle = control.clone();

        let worker = thread::spawn(move || {
            with_execution_control(control, || run_ok("sleep", vec![os("15")]))
        });

        thread::sleep(Duration::from_millis(150));
        let started = Instant::now();
        cancel_handle.cancel();
        let result = worker.join().expect("command worker should not panic");

        assert!(result.unwrap_err().contains("cancelled"));
        assert!(started.elapsed() < Duration::from_secs(3), "cancellation should stop the command promptly");
    }
}



#[cfg(all(test, unix))]
mod timeout_tests {
    use super::{os, run_ok_with_timeout};
    use std::time::{Duration, Instant};

    #[test]
    fn timeout_terminates_a_running_process_group() {
        let started = Instant::now();
        let error = run_ok_with_timeout(
            "sh",
            vec![os("-c"), os("sleep 10")],
            Duration::from_millis(150),
        ).unwrap_err();

        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}


#[cfg(all(test, unix))]
mod inherited_pipe_tests {
    use super::{os, run_ok};
    use std::time::{Duration, Instant};

    #[test]
    fn background_helper_cannot_keep_command_waiting_forever() {
        let started = Instant::now();
        let error = run_ok("sh", vec![os("-c"), os("sleep 20 & exit 0")]).unwrap_err();

        assert!(error.contains("kept the command output pipes open"));
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}


#[cfg(all(test, unix))]
mod command_version_timeout_tests {
    use super::command_version_succeeds_with_timeout;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    #[test]
    fn version_probe_stops_a_hung_executable() {
        let path = std::env::temp_dir().join(format!(
            "forgebox-hung-version-probe-{}",
            std::process::id()
        ));
        fs::write(&path, "#!/bin/sh\nsleep 10\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();

        let started = Instant::now();
        let succeeded = command_version_succeeds_with_timeout(
            path.as_os_str(),
            Duration::from_millis(150),
        );
        let elapsed = started.elapsed();

        let _ = fs::remove_file(&path);
        assert!(!succeeded, "a timed-out version probe must not report success");
        assert!(
            elapsed < Duration::from_secs(2),
            "a hung version probe should stop promptly, elapsed: {elapsed:?}"
        );
    }
}
