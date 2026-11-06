#!/usr/bin/env bash
set -euo pipefail

if ! command -v dnf >/dev/null 2>&1; then
  echo "This script is for Fedora (dnf)." >&2
  exit 1
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
DNF_FLAGS=()
if [[ ! -t 0 ]]; then
  DNF_FLAGS+=(--assumeyes)
fi

FEDORA_VERSION="$(rpm -E %fedora)"
case "$FEDORA_VERSION" in
  ''|*[!0-9]*) echo "Could not determine the Fedora release version." >&2; exit 1 ;;
esac

# ForgeBox uses libx264, which is not included in Fedora's ffmpeg-free build.
if ! rpm -q rpmfusion-free-release >/dev/null 2>&1; then
  echo "Enabling RPM Fusion Free for the full FFmpeg package..."
  sudo dnf install "${DNF_FLAGS[@]}" \
    "https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-${FEDORA_VERSION}.noarch.rpm"
fi

if rpm -q ffmpeg-free >/dev/null 2>&1; then
  sudo dnf swap "${DNF_FLAGS[@]}" ffmpeg-free ffmpeg --allowerasing
else
  sudo dnf install "${DNF_FLAGS[@]}" ffmpeg --allowerasing
fi

sudo dnf install "${DNF_FLAGS[@]}" \
  gcc gcc-c++ make pkgconf-pkg-config git rust cargo rustfmt \
  libxkbcommon-devel wayland-devel libX11-devel libXrandr-devel \
  libXi-devel libXcursor-devel libXinerama-devel mesa-libGL-devel \
  ImageMagick python3-img2pdf poppler-utils ghostscript qpdf pdftk-java libreoffice \
  tesseract tesseract-langpack-eng perl-Image-ExifTool

if tesseract --list-langs 2>/dev/null | grep -qx 'rus'; then
  echo "Russian OCR language data is already installed."
elif [[ -t 0 ]]; then
  read -r -p "Install Russian language data for Tesseract OCR? [y/N] " install_russian
  case "$install_russian" in
    y|Y|yes|YES|Yes)
      sudo dnf install "${DNF_FLAGS[@]}" tesseract-langpack-rus
      ;;
    *)
      echo "Skipping Russian OCR language data."
      ;;
  esac
else
  echo "Skipping optional Russian OCR language data in non-interactive mode."
fi

find_working_rembg() {
  local data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
  local candidate

  # Keep this search order aligned with ForgeBox's runtime command lookup.
  for candidate in \
    "$data_home/forgebox/rembg-venv/bin/rembg" \
    "$HOME/.local/bin/rembg"
  do
    if [[ -x "$candidate" ]] && "$candidate" --help >/dev/null 2>&1; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  if command -v rembg >/dev/null 2>&1; then
    candidate="$(command -v rembg)"
    if "$candidate" --help >/dev/null 2>&1; then
      printf '%s\n' "$candidate"
      return 0
    fi
  fi

  return 1
}

if rembg_path="$(find_working_rembg)"; then
  echo
  echo "A working rembg installation was found at: $rembg_path"
  echo "Skipping the optional rembg installation prompt."
elif [[ -t 0 ]]; then
  echo
  read -r -p "Set up the optional rembg CPU backend now? [y/N] " install_rembg
  case "$install_rembg" in
    y|Y|yes|YES|Yes)
      bash "$SCRIPT_DIR/install-rembg-fedora.sh"
      ;;
    *)
      echo "Skipping rembg. You can install it later with: bash \"$SCRIPT_DIR/install-rembg-fedora.sh\""
      ;;
  esac
else
  echo "Skipping the optional rembg prompt in non-interactive mode."
  echo "To install it later, run: bash \"$SCRIPT_DIR/install-rembg-fedora.sh\""
fi

cat <<'MSG'
ForgeBox build dependencies and common file utilities installed.

Optional components:
  - Russian OCR data: sudo dnf install tesseract-langpack-rus
  - rembg: optional background-removal backend. For an installed Fedora RPM, run:
      bash /usr/share/doc/forgebox/install-rembg-fedora.sh
    From a source checkout, run:
      bash scripts/install-rembg-fedora.sh

Prepare native Wayland drag-and-drop once, then build and run:
  bash scripts/enable-native-wayland-dnd.sh
  cargo run --release
MSG
