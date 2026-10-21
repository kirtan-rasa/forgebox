# ForgeBox on Windows: conversion engines

The ForgeBox installer installs the desktop application. Several conversion backends are separate command-line programs; bundling all of them into one installer would make the download much larger and create separate licensing and update obligations.

On Windows 10/11 with WinGet available, run this script from PowerShell to install the common backends:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
& "$env:LOCALAPPDATA\Programs\ForgeBox\scripts\install-windows-deps.ps1"
```

If ForgeBox was installed for a different user or folder, use the corresponding `scripts\install-windows-deps.ps1` inside that folder. The script installs ImageMagick, Poppler, Ghostscript, qpdf, Tesseract OCR, ExifTool, LibreOffice, and FFmpeg. LibreOffice and FFmpeg are optional in the script and can be skipped with `-SkipOffice` or `-SkipVideo`.

Close and reopen ForgeBox after installing the tools. Some installers update `PATH`, and already-running applications do not automatically receive the new environment.

## Features that need additional setup

- **PDF watermarking:** requires `pdftk.exe` on `PATH`. A consistently available WinGet package was not verified for this release kit, so the script does not silently install a third-party package from an unverified source.
- **Background removal:** requires the `rembg` command and its Python/model environment.
- **Video/audio conversion:** requires FFmpeg with the needed H.264/AAC or VP9/Opus codecs.
- **HEIC/HEIF:** support depends on the ImageMagick build and its format delegates.

When a backend is unavailable, ForgeBox displays the missing command in its error message. The Windows command resolver knows the common Ghostscript and LibreOffice executable names, but custom installs may still need their `bin`/`program` directory added to the user or system `PATH`.
