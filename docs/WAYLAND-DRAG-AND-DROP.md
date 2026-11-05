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
