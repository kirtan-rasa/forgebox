use raw_window_handle::RawDisplayHandle;

/// Handles interfacing with the OS clipboard.
///
/// On Linux, use arboard's X11 backend instead of creating a second Wayland
/// data device. The second device created by smithay-clipboard can receive
/// GNOME drag-and-drop events without forwarding them to winit.
///
/// If arboard cannot connect to the OS clipboard, a fallback clipboard that
/// just works within the same app is used instead.
pub struct Clipboard {
    #[cfg(all(feature = "arboard", not(target_os = "android")))]
    arboard: Option<arboard::Clipboard>,

    /// Fallback manual clipboard.
    clipboard: String,
}

impl Clipboard {
    /// Construct a new instance.
    pub fn new(_raw_display_handle: Option<RawDisplayHandle>) -> Self {
        Self {
            #[cfg(all(feature = "arboard", not(target_os = "android")))]
            arboard: init_arboard(),

            clipboard: Default::default(),
        }
    }

    pub fn get(&mut self) -> Option<String> {
        #[cfg(all(feature = "arboard", not(target_os = "android")))]
        if let Some(clipboard) = &mut self.arboard {
            return match clipboard.get_text() {
                Ok(text) => Some(text),
                Err(err) => {
                    log::error!("arboard paste error: {err}");
                    None
                }
            };
        }

        Some(self.clipboard.clone())
    }

    pub fn set(&mut self, text: String) {
        #[cfg(all(feature = "arboard", not(target_os = "android")))]
        if let Some(clipboard) = &mut self.arboard {
            if let Err(err) = clipboard.set_text(text) {
                log::error!("arboard copy/cut error: {err}");
            }
            return;
        }

        self.clipboard = text;
    }
}

#[cfg(all(feature = "arboard", not(target_os = "android")))]
fn init_arboard() -> Option<arboard::Clipboard> {
    crate::profile_function!();

    log::trace!("Initializing arboard clipboard…");
    match arboard::Clipboard::new() {
        Ok(clipboard) => Some(clipboard),
        Err(err) => {
            log::warn!("Failed to initialize arboard clipboard: {err}");
            None
        }
    }
}
