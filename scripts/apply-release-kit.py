#!/usr/bin/env python3
"""Apply ForgeBox packaging and Windows-runtime compatibility changes safely."""
from __future__ import annotations

from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parent.parent


def need_file(name: str) -> Path:
    path = ROOT / name
    if not path.is_file():
        raise RuntimeError(f"Required project file is missing: {name}")
    return path


def replace_once_or_already(text: str, old: str, new: str, marker: str, label: str) -> str:
    if marker in text:
        return text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(
            f"Could not safely patch {label}: expected exactly one matching block, found {count}. "
            "No project files have been written."
        )
    return text.replace(old, new, 1)


def main() -> int:
    cargo_path = need_file("Cargo.toml")
    engine_path = need_file("src/engine.rs")
    main_path = need_file("src/main.rs")
    readme_path = need_file("README.md")
    fedora_deps_path = need_file("scripts/install-deps-fedora.sh")
    build_path = need_file("build.rs")
    icon_ico = need_file("assets/forgebox.ico")
    icon_png = need_file("assets/forgebox.png")
    desktop_path = need_file("share/forgebox.desktop")

    cargo = cargo_path.read_text(encoding="utf-8")
    engine = engine_path.read_text(encoding="utf-8")
    app_main = main_path.read_text(encoding="utf-8")
    readme = readme_path.read_text(encoding="utf-8")
    fedora_deps = fedora_deps_path.read_text(encoding="utf-8")

    # Cargo additions are made in-memory first so a mismatch never leaves a half-applied setup.
    if 'qrcode = "0.14"' not in cargo:
        dependency_anchor = 'serde_json = "1.0"'
        if cargo.count(dependency_anchor) != 1:
            raise RuntimeError("Could not find the serde_json dependency line in Cargo.toml; no files were written.")
        cargo = cargo.replace(dependency_anchor, dependency_anchor + '\nqrcode = "0.14"', 1)
    if "[target.'cfg(windows)'.build-dependencies]" not in cargo:
        cargo = cargo.rstrip() + '\n\n[target.\'cfg(windows)\'.build-dependencies]\nwinres = "0.1"\n'

    # Generate QR images inside ForgeBox instead of requiring qrencode to be installed.
    old_qr = '''        GenerateQr => {
            if job.text.trim().is_empty() {
                return Err("Enter text or a URL for the QR code.".into());
            }
            let out = unique_path(&job.output_dir, "qr-code", "png");
            if let Err(error) = run_ok("qrencode", vec![os("-o"), os(&out), os("-s"), os("8"), os(&job.text)]) {
                let _ = fs::remove_file(&out);
                return Err(error);
            }
            Ok(vec![out])
        }
'''
    new_qr = '''        GenerateQr => {
            if job.text.trim().is_empty() {
                return Err("Enter text or a URL for the QR code.".into());
            }
            let out = unique_path(&job.output_dir, "qr-code", "png");
            let code = QrCode::new(job.text.as_bytes())
                .map_err(|error| format!("Could not generate the QR code: {error}"))?;
            let qr_image = code
                .render::<image::Luma<u8>>()
                .min_dimensions(320, 320)
                .build();
            qr_image.save(&out).map_err(|error| format!("Failed to save the QR code: {error}"))?;
            Ok(vec![out])
        }
'''
    engine = replace_once_or_already(engine, old_qr, new_qr, "QrCode::new(job.text.as_bytes())", "src/engine.rs QR generation")
    if "use qrcode::QrCode;" not in engine:
        import_anchor = "use serde_json::{Map, Value};"
        if engine.count(import_anchor) != 1:
            raise RuntimeError("Could not find the serde_json import in src/engine.rs; no files were written.")
        engine = engine.replace(import_anchor, import_anchor + "\nuse qrcode::QrCode;", 1)

    old_magick = '''fn run_magick_owned(args: Vec<OsString>) -> Result<(), String> {
    if has_command("magick") {
        run_ok("magick", args).map(|_| ())
    } else {
        run_ok("convert", args).map(|_| ())
    }
}
'''
    new_magick = '''fn run_magick_owned(args: Vec<OsString>) -> Result<(), String> {
    #[cfg(windows)]
    {
        // Windows also ships a system convert.exe, unrelated to ImageMagick.
        // Never accidentally invoke that utility as a conversion backend.
        run_ok("magick", args).map(|_| ())
    }
    #[cfg(not(windows))]
    {
        if has_command("magick") {
            run_ok("magick", args).map(|_| ())
        } else {
            run_ok("convert", args).map(|_| ())
        }
    }
}
'''
    engine = replace_once_or_already(engine, old_magick, new_magick, "// Windows also ships a system convert.exe", "src/engine.rs ImageMagick fallback")

    old_commands = '''fn run_ok(program: &str, args: Vec<OsString>) -> Result<Output, String> {
    let output = Command::new(program).args(args.iter()).output().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            format!("Dependency `{program}` was not found. See the Installation section in the README.")
        } else {
            format!("Failed to launch `{program}`: {e}")
        }
    })?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(if detail.is_empty() { format!("`{program}` exited with code {}.", output.status) } else { format!("`{program}`: {detail}") })
    }
}

fn run_capture(program: &str, args: Vec<OsString>) -> Result<Vec<u8>, String> {
    Ok(run_ok(program, args)?.stdout)
}

fn has_command(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}
'''
    new_commands = r'''fn command_candidates(program: &str) -> Vec<OsString> {
    #[cfg(windows)]
    {
        let mut candidates: Vec<OsString> = Vec::new();
        let roots = [std::env::var_os("ProgramFiles"), std::env::var_os("ProgramFiles(x86)")];

        for root in roots.into_iter().flatten().map(PathBuf::from) {
            match program {
                "libreoffice" => {
                    for executable in ["soffice.com", "soffice.exe"] {
                        let path = root.join("LibreOffice").join("program").join(executable);
                        if path.is_file() { candidates.push(path.into_os_string()); }
                    }
                }
                "gs" => {
                    let install_root = root.join("gs");
                    if let Ok(entries) = fs::read_dir(install_root) {
                        let mut version_dirs: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
                        version_dirs.sort();
                        version_dirs.reverse();
                        for version_dir in version_dirs {
                            for executable in ["gswin64c.exe", "gswin32c.exe"] {
                                let path = version_dir.join("bin").join(executable);
                                if path.is_file() { candidates.push(path.into_os_string()); }
                            }
                        }
                    }
                }
                "magick" => {
                    if let Ok(entries) = fs::read_dir(&root) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let label = path.file_name().map(|name| name.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                            if path.is_dir() && label.starts_with("imagemagick") {
                                for executable in [path.join("magick.exe"), path.join("bin").join("magick.exe")] {
                                    if executable.is_file() { candidates.push(executable.into_os_string()); }
                                }
                            }
                        }
                    }
                }
                "tesseract" => {
                    let path = root.join("Tesseract-OCR").join("tesseract.exe");
                    if path.is_file() { candidates.push(path.into_os_string()); }
                }
                "qpdf" => {
                    for path in [root.join("qpdf").join("bin").join("qpdf.exe"), root.join("qpdf.exe")] {
                        if path.is_file() { candidates.push(path.into_os_string()); }
                    }
                    if let Ok(entries) = fs::read_dir(&root) {
                        for entry in entries.flatten() {
                            let folder = entry.path();
                            let label = folder.file_name().map(|name| name.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                            if folder.is_dir() && label.starts_with("qpdf") {
                                for path in [folder.join("bin").join("qpdf.exe"), folder.join("qpdf.exe")] {
                                    if path.is_file() { candidates.push(path.into_os_string()); }
                                }
                            }
                        }
                    }
                }
                "pdftk" => {
                    for path in [root.join("PDFtk Server").join("bin").join("pdftk.exe"), root.join("pdftk").join("pdftk.exe")] {
                        if path.is_file() { candidates.push(path.into_os_string()); }
                    }
                }
                _ => {}
            }
        }

        let aliases: &[&str] = match program {
            "gs" => &["gswin64c.exe", "gswin32c.exe", "gs.exe", "gs"],
            "libreoffice" => &["soffice.com", "soffice.exe", "libreoffice.exe", "libreoffice"],
            _ => &[],
        };
        for alias in aliases { candidates.push(OsString::from(alias)); }
        if !program.to_ascii_lowercase().ends_with(".exe") {
            candidates.push(OsString::from(format!("{program}.exe")));
        }
        candidates.push(OsString::from(program));

        let mut unique = Vec::new();
        for candidate in candidates {
            if !unique.contains(&candidate) { unique.push(candidate); }
        }
        unique
    }
    #[cfg(not(windows))]
    {
        vec![OsString::from(program)]
    }
}

fn run_ok(program: &str, args: Vec<OsString>) -> Result<Output, String> {
    let candidates = command_candidates(program);
    let mut tried: Vec<String> = Vec::new();
    for candidate in candidates {
        let label = candidate.to_string_lossy().into_owned();
        match Command::new(&candidate).args(args.iter()).output() {
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
    Err(format!(
        "Dependency `{program}` was not found. Tried: {}. See the Installation section in the README.",
        tried.join(", ")
    ))
}

fn run_capture(program: &str, args: Vec<OsString>) -> Result<Vec<u8>, String> {
    Ok(run_ok(program, args)?.stdout)
}

fn has_command(program: &str) -> bool {
    command_candidates(program).iter().any(|candidate| Command::new(candidate).arg("--version").output().is_ok())
}
'''
    engine = replace_once_or_already(engine, old_commands, new_commands, "fn command_candidates(program: &str)", "src/engine.rs Windows command lookup")

    old_thumbnail = '''        let program = match std::process::Command::new("magick").arg("-version").output() {
            Ok(output) if output.status.success() => "magick",
            _ => "convert",
        };
'''
    new_thumbnail = '''        let program = match std::process::Command::new("magick").arg("-version").output() {
            Ok(output) if output.status.success() => "magick",
            _ if cfg!(target_os = "windows") => {
                return Err("ImageMagick (`magick.exe`) was not found. Install it and ensure it is available in PATH.".into());
            }
            _ => "convert",
        };
'''
    app_main = replace_once_or_already(app_main, old_thumbnail, new_thumbnail, "ImageMagick (`magick.exe`) was not found", "src/main.rs thumbnail fallback")

    old_open_folder = '''                                                    #[cfg(target_os = "linux")]
                                                    { let _ = std::process::Command::new("xdg-open").arg(&self.output_dir).spawn(); }
'''
    new_open_folder = '''                                                    #[cfg(target_os = "linux")]
                                                    { let _ = std::process::Command::new("xdg-open").arg(&self.output_dir).spawn(); }
                                                    #[cfg(target_os = "windows")]
                                                    { let _ = std::process::Command::new("explorer.exe").arg(&self.output_dir).spawn(); }
'''
    app_main = replace_once_or_already(app_main, old_open_folder, new_open_folder, 'Command::new("explorer.exe").arg(&self.output_dir)', "src/main.rs Open folder action")

    if "| QR codes | Built into ForgeBox (`qrcode`) |" not in readme:
        old_qr_row = "| QR codes | `qrencode` |"
        if old_qr_row not in readme:
            raise RuntimeError("Could not find the QR dependency row in README.md; no files were written.")
        readme = readme.replace(old_qr_row, "| QR codes | Built into ForgeBox (`qrcode`) |", 1)
    if " qrencode" in fedora_deps:
        fedora_deps = fedora_deps.replace(" perl-Image-ExifTool qrencode", " perl-Image-ExifTool")
    if "tesseract-langpack-eng" not in fedora_deps:
        fedora_deps = fedora_deps.replace(
            "tesseract tesseract-langpack-rus",
            "tesseract tesseract-langpack-eng tesseract-langpack-rus",
            1,
        )

    if not (build_path.stat().st_size and icon_ico.stat().st_size and icon_png.stat().st_size and desktop_path.stat().st_size):
        raise RuntimeError("A release resource is empty; check the extracted release-kit folder.")

    # Every validation above completes before backups or writes begin.
    updates = {
        cargo_path: cargo,
        engine_path: engine,
        main_path: app_main,
        readme_path: readme,
        fedora_deps_path: fedora_deps,
    }
    for path, new_text in updates.items():
        current = path.read_text(encoding="utf-8")
        if current == new_text:
            continue
        backup = path.with_name(path.name + ".release-kit.bak")
        if not backup.exists():
            shutil.copy2(path, backup)
    for path, new_text in updates.items():
        current = path.read_text(encoding="utf-8")
        if current != new_text:
            path.write_text(new_text, encoding="utf-8", newline="")

    print("ForgeBox release kit applied.")
    print("Updated: Cargo.toml, src/engine.rs, src/main.rs, README.md, scripts/install-deps-fedora.sh")
    print("Created backups with the suffix .release-kit.bak before modifying each file.")
    print("Next: run `cargo test` and `cargo check` before committing. Run `cargo fmt` only if you intend to normalize the existing source formatting too.")
    print("The workflow builds the Windows installer and Fedora RPM on GitHub Actions.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"Release-kit setup failed: {error}", file=sys.stderr)
        raise SystemExit(2)
