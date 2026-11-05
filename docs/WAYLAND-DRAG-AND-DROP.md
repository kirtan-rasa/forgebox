# Native Wayland drag and drop

ForgeBox currently uses `eframe 0.29` and `winit 0.30`. The native Wayland backend in that stack does not forward external file-drop events to egui, so the project pins a compatibility version of `winit 0.30.13` under `patches/winit`.

## Implementation

The compatibility code uses the core Wayland `wl_data_device` protocol:

- Accepts local file offers using the `text/uri-list` MIME type.
- Requests copy-only transfers so the source file manager is never asked to move or delete the original.
- Flushes the receive request so the data source can begin writing.
- Reads the URI list asynchronously through calloop rather than blocking the event loop.
- Emits the legacy `HoveredFile`, `HoveredFileCancelled`, and `DroppedFile` events consumed by egui-winit.

Some compositors require the accepted MIME type to be reaffirmed as the pointer moves over the target. The `motion` handler therefore refreshes the `text/uri-list` acceptance and flushes the protocol request. It does **not** renegotiate DnD actions on every motion event: restarting that negotiation can interrupt a quick drop. The selected-action handler also reaffirms the accepted MIME type for the active offer.

These protocol details are intended to improve compatibility across native Wayland compositors. They still need to be verified on the affected GNOME setup; successful operation on one compositor does not prove equivalent behavior on all others.

## GNOME clipboard / drag-and-drop interaction

The eframe 0.29.1 dependency graph enables the `egui-winit` clipboard feature. Upstream `egui-winit` implements its Wayland clipboard backend with `smithay-clipboard 0.7.x`. That backend creates a second event queue and a second `wl_data_device` for the same Wayland seat. Its data-device drag-and-drop callbacks are empty.

On the affected GNOME setup, the Wayland trace showed the Winit device receiving no DnD events while the device created by the clipboard worker received `data_offer`, `enter`, `motion`, and `drop`. The latter cannot forward the drag to Winit, so the application never receives `HoveredFile` or `DroppedFile`.

ForgeBox therefore vendors the matching `egui-winit 0.29.1` sources under `patches/egui-winit` and changes the clipboard backend: it retains `arboard`, but does not initialize `smithay-clipboard`. On Linux, this means OS clipboard access uses the X11/XWayland backend when available. Without a usable X11 display, egui falls back to its in-process clipboard. The intent is to leave one Wayland data device, owned by Winit, responsible for native file drops.

This is a targeted workaround for the duplicate data-device behavior observed on GNOME. It is not considered verified until a build containing this patch has been tested on GNOME.

## Pinned source

The bootstrap script fetches the vendored crate from this pinned source:

- Project: https://github.com/MixaDoDs/photocraft
- Commit: `798bb7b6afa343ed34d19040db48248ce14d2108`
- Original proposal: https://github.com/storytold/photocraft/pull/7828

Run the bootstrap manually from the repository root when needed:

```bash
bash scripts/enable-native-wayland-dnd.sh
```

The normal build script invokes the bootstrap automatically if the expected vendored source is absent. The patch remains local to the build and does not change how ForgeBox processes the dropped files.

This is a compatibility integration, not a security boundary. File paths received through drag and drop still pass through ForgeBox's normal file-processing and validation logic. Revisit the patch when the eframe/egui-winit dependency stack can migrate to an upstream winit API with native Wayland file-drop support.
