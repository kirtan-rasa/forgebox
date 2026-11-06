#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# The setup helper is idempotent: it validates an existing patch and
# repairs missing state before Cargo resolves the local dependency.
bash scripts/enable-native-wayland-dnd.sh

# Formatting is fast and catches many syntax errors before the compiler starts.
cargo fmt

# Full dev/test builds are opt-in so low-power CPUs do not compile several profiles on every build.
if [[ "${FORGEBOX_RUN_CHECKS:-0}" == "1" ]]; then
  cargo check
  cargo test
fi

cargo build --release
printf '\nBuilt binary: %s\n' "$PWD/target/release/forgebox"
