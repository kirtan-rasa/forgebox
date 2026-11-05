# Local egui-winit compatibility patch

This directory vendors the `egui-winit 0.29.1` source from the egui `0.29.1` tag.

ForgeBox keeps the upstream event integration and window settings code. The clipboard module is intentionally adjusted to use `arboard` without `smithay-clipboard`. The latter creates an additional Wayland data device whose empty DnD handlers can intercept file-drop events on the affected GNOME setup.

Do not re-enable the `smithay-clipboard` Wayland backend without re-testing native Wayland file drops on GNOME.

Upstream: https://github.com/emilk/egui/tree/0.29.1/crates/egui-winit
