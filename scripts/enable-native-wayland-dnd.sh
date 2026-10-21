#!/usr/bin/env bash
set -euo pipefail

# Bootstrap a pinned, minimal checkout of the winit 0.30.13 Wayland DnD patch.
# This is a temporary compatibility patch until egui-winit adopts upstream winit 0.31.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="${FORGEBOX_WAYLAND_DND_COMMIT:-798bb7b6afa343ed34d19040db48248ce14d2108}"
REPO="${FORGEBOX_WAYLAND_DND_REPO:-https://github.com/MixaDoDs/photocraft.git}"
BRANCH="${FORGEBOX_WAYLAND_DND_BRANCH:-wayland-dnd-main}"
DEST="$ROOT/patches/winit"

if [[ ! -f "$ROOT/Cargo.toml" || ! -d "$ROOT/src" ]]; then
  echo "Run this script from inside the ForgeBox source tree." >&2
  exit 2
fi
if ! command -v git >/dev/null 2>&1; then
  echo "Git is required to fetch the pinned Wayland drop implementation." >&2
  echo "Install it with: sudo dnf install git" >&2
  exit 2
fi

is_ready() {
  [[ -f "$DEST/Cargo.toml" && -f "$DEST/src/platform_impl/linux/wayland/seat/dnd.rs" ]] \
    && grep -q 'version = "0.30.13"' "$DEST/Cargo.toml" \
    && grep -q 'DataDeviceHandler' "$DEST/src/platform_impl/linux/wayland/seat/dnd.rs"
}

if is_ready; then
  echo "Native Wayland drag-and-drop support is already prepared."
  exit 0
fi
if [[ -e "$DEST" ]]; then
  echo "Found $DEST, but it does not look like the expected winit 0.30.13 source." >&2
  echo "Move that directory aside and rerun this script; refusing to overwrite it." >&2
  exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
REPO_DIR="$TMP/photocraft"

echo "Fetching the pinned winit Wayland drag-and-drop implementation..."
git clone --quiet --depth=1 --filter=blob:none --no-checkout --branch "$BRANCH" "$REPO" "$REPO_DIR"
# Do not rely on FETCH_HEAD here: some Git versions/clone configurations do
# not leave it available after a --no-checkout clone. Test the pinned commit
# object directly and fetch it only when it is not already present locally.
if ! git -C "$REPO_DIR" cat-file -e "$COMMIT^{commit}" 2>/dev/null; then
  git -C "$REPO_DIR" fetch --quiet --depth=1 --filter=blob:none origin "$COMMIT"
fi
git -C "$REPO_DIR" sparse-checkout init --cone
git -C "$REPO_DIR" sparse-checkout set patches/winit
git -C "$REPO_DIR" checkout --quiet --detach "$COMMIT"
ACTUAL="$(git -C "$REPO_DIR" rev-parse HEAD)"
if [[ "$ACTUAL" != "$COMMIT" ]]; then
  echo "Unexpected source revision: $ACTUAL (expected $COMMIT)." >&2
  exit 3
fi
if [[ ! -f "$REPO_DIR/patches/winit/src/platform_impl/linux/wayland/seat/dnd.rs" ]]; then
  echo "The pinned source does not contain the Wayland DnD implementation." >&2
  exit 3
fi
mkdir -p "$ROOT/patches"
cp -a "$REPO_DIR/patches/winit" "$DEST"

# Cargo.toml already routes winit to patches/winit and enables both Linux backends.
echo "Installed winit 0.30.13 Wayland DnD patch from commit $COMMIT."
echo "Next: cargo run --release"
echo "The initial build may take longer because Cargo must compile the patched windowing backend."
