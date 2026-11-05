#!/usr/bin/env bash
set -euo pipefail

if ! command -v dnf >/dev/null 2>&1; then
  echo "This helper is for Fedora (dnf)." >&2
  exit 1
fi

PYTHON_BIN="python3.13"
if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  echo "Installing Python 3.13 for the isolated rembg environment…"
  sudo dnf install -y python3.13
fi

if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  echo "Python 3.13 was not found after the DNF installation." >&2
  exit 2
fi

"$PYTHON_BIN" - <<'PY'
import sys
if not ((3, 11) <= sys.version_info[:2] < (3, 14)):
    raise SystemExit(
        f"rembg currently requires Python >=3.11 and <3.14; found {sys.version.split()[0]}"
    )
PY

DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
VENV_DIR="$DATA_HOME/forgebox/rembg-venv"
mkdir -p "$(dirname "$VENV_DIR")"

if [[ ! -x "$VENV_DIR/bin/python" ]]; then
  echo "Creating isolated environment at: $VENV_DIR"
  "$PYTHON_BIN" -m venv "$VENV_DIR"
fi

if ! "$VENV_DIR/bin/python" -m pip --version >/dev/null 2>&1; then
  echo "The Python 3.13 environment has no pip bootstrap. Trying ensurepip…"
  "$PYTHON_BIN" -m ensurepip --user --upgrade
  "$PYTHON_BIN" -m venv --clear "$VENV_DIR"
fi

if ! "$VENV_DIR/bin/python" -m pip --version >/dev/null 2>&1; then
  echo "Could not initialise pip for Python 3.13." >&2
  echo "Check the Fedora Python 3.13/ensurepip packages, then run this helper again." >&2
  exit 3
fi

echo "Installing the CPU version of rembg and its CLI…"
"$VENV_DIR/bin/python" -m pip install --upgrade pip
"$VENV_DIR/bin/python" -m pip install --upgrade "rembg[cpu,cli]"
"$VENV_DIR/bin/rembg" --help >/dev/null

echo
echo "rembg is ready for ForgeBox."
echo "Environment: $VENV_DIR"
echo "The AI model may be downloaded the first time you remove a background."
