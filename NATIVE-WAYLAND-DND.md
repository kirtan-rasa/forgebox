# Native Wayland drag-and-drop compatibility patch

ForgeBox currently uses `eframe 0.29` / `winit 0.30`, whose native Wayland backend does not forward external file drops. `scripts/enable-native-wayland-dnd.sh` fetches the `patches/winit` directory from the following pinned upstream commit and installs it as a local Cargo patch:

- Project: https://github.com/MixaDoDs/photocraft
- Commit: `798bb7b6afa343ed34d19040db48248ce14d2108`
- Original upstream proposal: https://github.com/storytold/photocraft/pull/7828

The compatibility implementation uses Wayland's `wl_data_device`, accepts local `text/uri-list` file offers, requests copy-only actions, reads the transfer asynchronously, and exposes the resulting paths through the legacy `winit 0.30` events that `egui-winit` already understands.

This integration is temporary and intentionally pinned for reproducible builds. Revisit it when `eframe`/`egui-winit` officially adopts `winit 0.31`. The upstream proposal has known edge cases involving overlapping or unusually slow transfers; ordinary one-off file and multi-file drops are the intended path. Do not treat this as a security boundary: dropped paths still pass through ForgeBox's existing file-processing validation.
