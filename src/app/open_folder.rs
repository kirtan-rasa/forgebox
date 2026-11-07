use std::path::Path;
use std::process::Command;

#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStrExt;

#[cfg(target_os = "linux")]
fn file_uri(path: &Path) -> Result<String, String> {
    let absolute = path.canonicalize()
        .map_err(|error| format!("Could not resolve the output folder: {error}"))?;
    let mut uri = String::from("file://");
    for byte in absolute.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            uri.push(*byte as char);
        } else {
            use std::fmt::Write as _;
            write!(&mut uri, "%{:02X}", *byte).expect("writing to a String cannot fail");
        }
    }
    Ok(uri)
}

#[cfg(target_os = "linux")]
fn folders_argument(uri: &str) -> String {
    format!("[ '{uri}' ]")
}

#[cfg(target_os = "linux")]
pub(super) fn open_folder(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("The output folder does not exist: {}", path.display()));
    }
    let uri = file_uri(path)?;
    let dbus_result = Command::new("gdbus")
        .args([
            "call", "--session", "--dest", "org.freedesktop.FileManager1",
            "--object-path", "/org/freedesktop/FileManager1",
            "--method", "org.freedesktop.FileManager1.ShowFolders",
        ])
        .arg(folders_argument(&uri))
        .arg("")
        .output();

    if let Ok(output) = &dbus_result {
        if output.status.success() {
            return Ok(());
        }
    }

    // Lightweight desktops may not export FileManager1. Launch a file manager
    // executable directly instead of falling back to gio/xdg-open MIME handlers.
    for manager in [
        "nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm",
        "pcmanfm-qt", "pantheon-files", "cosmic-files", "deepin-file-manager", "spacefm",
    ] {
        if Command::new(manager).arg(path).spawn().is_ok() {
            return Ok(());
        }
    }

    let detail = match dbus_result {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if stderr.is_empty() { format!("gdbus exited with {}", output.status) } else { stderr }
        }
        Err(error) => format!("could not run gdbus: {error}"),
    };
    Err(format!("Could not contact the desktop file manager ({detail}), and no supported file manager executable was found."))
}

#[cfg(target_os = "macos")]
pub(super) fn open_folder(path: &Path) -> Result<(), String> {
    Command::new("open").arg(path).spawn().map(|_| ())
        .map_err(|error| format!("Could not open the output folder: {error}"))
}

#[cfg(target_os = "windows")]
pub(super) fn open_folder(path: &Path) -> Result<(), String> {
    Command::new("explorer.exe").arg(path).spawn().map(|_| ())
        .map_err(|error| format!("Could not open the output folder: {error}"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{file_uri, folders_argument};

    #[test]
    fn file_uri_encodes_spaces_unicode_and_quotes() {
        let root = std::env::temp_dir().join(format!("forgebox-uri-test-{}", std::process::id()));
        let target = root.join("Music & Mix").join("Песня's.flac");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"fixture").unwrap();

        let uri = file_uri(&target).unwrap();
        assert!(uri.ends_with("/Music%20%26%20Mix/%D0%9F%D0%B5%D1%81%D0%BD%D1%8F%27s.flac"));
        assert_eq!(folders_argument(&uri), format!("[ '{uri}' ]"));

        std::fs::remove_dir_all(root).unwrap();
    }
}
