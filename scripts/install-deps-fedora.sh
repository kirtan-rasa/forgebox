#!/usr/bin/env bash
set -euo pipefail

if ! command -v dnf >/dev/null 2>&1; then
  echo "This script is for Fedora (dnf)." >&2
  exit 1
fi

sudo dnf install -y \
  gcc gcc-c++ make pkgconf-pkg-config git rust cargo rustfmt \
  libxkbcommon-devel wayland-devel libX11-devel libXrandr-devel \
  libXi-devel libXcursor-devel libXinerama-devel mesa-libGL-devel \
  ImageMagick poppler-utils ghostscript qpdf pdftk-java libreoffice \
  tesseract tesseract-langpack-eng tesseract-langpack-rus perl-Image-ExifTool

cat <<'MSG'
ForgeBox build dependencies and common file utilities installed.

Optional dependencies:
  - ffmpeg: enable video/audio conversion and GIF export. Codec support for H.264/VP9 varies by build; Fedora may require RPM Fusion.
  - rembg: enable background removal; install separately in a Python environment.

Prepare native Wayland drag-and-drop once, then build and run:
  bash scripts/enable-native-wayland-dnd.sh
  cargo run --release
MSG
