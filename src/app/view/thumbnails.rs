use eframe::egui;
use image::ImageDecoder;
use std::path::Path;

const MAX_IN_MEMORY_THUMBNAIL_PIXELS: u64 = 8_000_000;

fn requires_external_thumbnail(width: u32, height: u32) -> bool {
    u64::from(width).saturating_mul(u64::from(height)) > MAX_IN_MEMORY_THUMBNAIL_PIXELS
}

pub(in crate::app) fn load_thumbnail(path: &Path) -> Result<egui::ColorImage, String> {
    let mut decoded = (|| -> Result<image::DynamicImage, String> {
        let reader = image::ImageReader::open(path).map_err(|error| error.to_string())?;
        let mut decoder = reader.into_decoder().map_err(|error| error.to_string())?;
        let (width, height) = decoder.dimensions();
        if requires_external_thumbnail(width, height) {
            return Err(format!(
                "Image dimensions {width}×{height} are too large for in-memory preview decoding."
            ));
        }

        let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
        let mut decoded = image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
        decoded.apply_orientation(orientation);
        Ok(decoded)
    })();

    // Very large images and formats unsupported by the image crate are reduced by
    // ImageMagick before their pixels enter the Rust process.
    if decoded.is_err() {
        let program = crate::engine::image_magick_program();
        let thread_limit = crate::engine::image_magick_thread_limit().to_string();
        let output = std::process::Command::new(program)
            .args([
                "-limit", "thread", thread_limit.as_str(),
                "-define", "jpeg:size=1200x840",
            ])
            .arg(path)
            .args(["-auto-orient", "-thumbnail", "600x420", "png:-"])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                decoded = image::load_from_memory(&result.stdout).map_err(|error| error.to_string());
            }
            Ok(result) => {
                let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
                return Err(if detail.is_empty() {
                    "Preview format is not supported".into()
                } else {
                    detail
                });
            }
            Err(error) => return Err(error.to_string()),
        }
    }

    let thumb = decoded.map_err(|error| error.to_string())?.thumbnail(600, 420).to_rgba8();
    let size = [thumb.width() as usize, thumb.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, thumb.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::requires_external_thumbnail;

    #[test]
    fn large_images_are_delegated_before_full_decode() {
        assert!(!requires_external_thumbnail(4000, 2000));
        assert!(requires_external_thumbnail(4000, 2001));
        assert!(!requires_external_thumbnail(0, u32::MAX));
    }
}
