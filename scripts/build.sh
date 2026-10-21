#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# Prepare the pinned native Wayland file-drop backend before Cargo resolves local patches.
if [[ ! -f "patches/winit/src/platform_impl/linux/wayland/seat/dnd.rs" ]]; then
  bash scripts/enable-native-wayland-dnd.sh
fi

# Formatting is fast and catches many syntax errors before the compiler starts.
cargo fmt

# Full dev/test builds are opt-in so low-power CPUs do not compile several profiles on every build.
if [[ "${FORGEBOX_RUN_CHECKS:-0}" == "1" ]]; then
  cargo check
  cargo test
fi

cargo build --release
printf '\nBuilt binary: %s\n' "$PWD/target/release/forgebox"
