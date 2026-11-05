# ForgeBox on Fedora

The RPM declares the common conversion engines as package dependencies. Install it with DNF so the package manager can resolve those requirements:

```bash
sudo dnf install ./forgebox-*.rpm
```

The RPM requires Tesseract and its English language data because English is the default OCR language. Russian OCR data and FFmpeg are weak/recommended dependencies, not hard requirements. FFmpeg may require RPM Fusion to be enabled.

Do not use `rpm -i` when you expect dependencies to be installed automatically: RPM itself does not perform DNF's dependency resolution. AppImage also does not install host conversion engines.

## Background removal

Background removal is optional because `rembg` is a Python application with its own runtime and model. The current upstream release supports Python 3.11 through 3.13, while Fedora 45's system Python is 3.14. ForgeBox therefore does not install `rembg` into the system Python or declare it as a normal RPM requirement.

The RPM includes a helper that creates a dedicated CPU environment using Python 3.13:

```bash
bash /usr/share/doc/forgebox/install-rembg-fedora.sh
```

The same helper is available in the source tree at `scripts/install-rembg-fedora.sh`. It installs `rembg[cpu,cli]` into `~/.local/share/forgebox/rembg-venv`; ForgeBox searches that path automatically. The AI model is downloaded separately on first use.

This separation keeps the system Python untouched and avoids making a large optional AI backend mandatory for every ForgeBox installation.
