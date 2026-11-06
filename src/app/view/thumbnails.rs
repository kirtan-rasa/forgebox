use super::theme::*;
use eframe::egui;
use image::ImageDecoder;
use std::path::Path;

pub(super) fn load_thumbnail(path: &Path) -> Result<egui::ColorImage, String> {
    let mut decoded = (|| -> Result<image::DynamicImage, String> {
        let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
        let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
        let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
        let mut decoded = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        decoded.apply_orientation(orientation);
        Ok(decoded)
    })();

    // Formats supported by ImageMagick (for example HEIC) may not be enabled in
    // the image crate. Fall back to a temporary PNG while keeping work off the UI thread.
    if decoded.is_err() {
        let suffix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let temp = std::env::temp_dir().join(format!("forgebox-thumb-{}-{suffix}.png", std::process::id()));
        // Share the engine's cached ImageMagick probe instead of spawning a
        // separate --version process for every unsupported preview.
        let program = crate::engine::image_magick_program();
        let thread_limit = crate::engine::image_magick_thread_limit().to_string();
        let output = std::process::Command::new(program)
            .args(["-limit", "thread", thread_limit.as_str()])
            .arg(path).args(["-auto-orient", "-thumbnail", "600x420"])
            .arg(&temp).output();
        match output {
            Ok(result) if result.status.success() => {
                decoded = image::open(&temp).map_err(|e| e.to_string());
                let _ = std::fs::remove_file(&temp);
            }
            Ok(result) => {
                let _ = std::fs::remove_file(&temp);
                let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
                return Err(if detail.is_empty() { "Preview format is not supported".into() } else { detail });
            }
            Err(error) => { let _ = std::fs::remove_file(&temp); return Err(error.to_string()); }
        }
    }
    let thumb = decoded.map_err(|e| e.to_string())?.thumbnail(600, 420).to_rgba8();
    let size = [thumb.width() as usize, thumb.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, thumb.as_raw()))
}

// Draw reorder arrows into an explicitly allocated hit rectangle. This avoids tiny or
// ambiguous hit zones and leaves the controls independent from the drag handle.
