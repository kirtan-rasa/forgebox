# ForgeBox on Fedora

The RPM declares the common conversion engines as package dependencies. Install it with DNF so the package manager can resolve those requirements:

```bash
sudo dnf install ./forgebox-*.rpm
```

The RPM requires Tesseract and its English language data because English is the default OCR language. It also requires LibreOffice for document and spreadsheet conversion, FFmpeg for audio/video tools, and `python3-img2pdf` for image-to-PDF assembly. Russian OCR data is optional and can be installed with the post-install prompt or `sudo dnf install tesseract-langpack-rus`.

ForgeBox uses the `libx264` encoder for MP4/MOV/MKV output. Fedora's `ffmpeg-free` build does not include that encoder, so Fedora users need the full `ffmpeg` package from RPM Fusion Free. The quick installer enables RPM Fusion Free automatically. For manual installation, run:

```bash
sudo dnf install https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm
sudo dnf swap ffmpeg-free ffmpeg --allowerasing
```

If `ffmpeg-free` is not installed, install the full package directly with `sudo dnf install ffmpeg --allowerasing`.

Do not use `rpm -i` when you expect dependencies to be installed automatically: RPM itself does not perform DNF's dependency resolution. AppImage also does not install host conversion engines.

## Background removal

Background removal is optional because `rembg` is a Python application with its own runtime and model. The current Fedora helper uses a dedicated Python 3.13 environment for the optional rembg CPU backend instead of changing the system Python or declaring rembg as a normal RPM requirement.

The source setup script installs required tools, including img2pdf and Tesseract with its English/Russian language data. In an interactive terminal, DNF asks you to confirm the package transaction; afterward, the script separately asks whether to install optional `rembg`. Declining it does not affect the other conversion tools.

The RPM includes a helper that creates a dedicated CPU environment using Python 3.13:

```bash
bash /usr/share/doc/forgebox/install-rembg-fedora.sh
```

The same helper is available in the source tree at `scripts/install-rembg-fedora.sh`. It installs `rembg[cpu,cli]` into `~/.local/share/forgebox/rembg-venv`; ForgeBox searches that path automatically. The AI model is downloaded separately on first use.

This separation keeps the system Python untouched and avoids making a large optional AI backend mandatory for every ForgeBox installation.
