use super::super::theme::MUTED;
use super::ForgeApp;
use crate::catalog::ToolId;
use eframe::egui::{self, RichText};

pub(super) fn parameter_ui(ui: &mut egui::Ui, app: &mut ForgeApp) {
    match app.selected_tool {
        ToolId::ImagesToPdf => {
            ui.label(RichText::new("One PDF, with a separate page for each image.").size(12.0));
            ui.label(RichText::new("Reorder pages by dragging files in the first step.").size(11.0).color(MUTED));
            ui.add_space(12.0);
            ui.checkbox(&mut app.compress_pdf, "Compress images inside the PDF");
            if app.compress_pdf {
                ui.label(RichText::new(format!("Image quality: {}%", app.quality)).size(12.0));
                let mut quality = app.quality.parse::<f32>().unwrap_or(82.0).clamp(1.0, 100.0);
                if ui.add(egui::Slider::new(&mut quality, 20.0..=100.0).integer()).changed() { app.quality = (quality as i32).to_string(); }
                ui.label(RichText::new("Lower quality produces a smaller PDF. For photos, 70–85% is usually a good range.").size(11.0).color(MUTED));
            }
        }
        ToolId::ImageConvert => {
            ui.label(RichText::new("Target format").size(12.0));
            format_combo(ui, &mut app.target_format, &["png", "jpg", "webp", "tiff", "heic"]);
        }
        ToolId::ImageResize => {
            ui.horizontal_wrapped(|ui| {
                labeled_text(ui, "Width", &mut app.width, "1600");
                labeled_text(ui, "Height", &mut app.height, "1200");
            });
            ui.label(RichText::new("Image aspect ratio is preserved. Resize bounds are limited to 16,384 px per side and 64 MP total.").size(11.0).color(MUTED));
        }
        ToolId::ImageCompress => {
            ui.label(RichText::new(format!("Quality: {}%", app.quality)).size(12.0));
            let mut quality = app.quality.parse::<f32>().unwrap_or(82.0).clamp(1.0, 100.0);
            if ui.add(egui::Slider::new(&mut quality, 1.0..=100.0).integer()).changed() { app.quality = (quality as i32).to_string(); }
            ui.label(RichText::new("For JPEG/WebP, lower values usually produce smaller files.").size(11.0).color(MUTED));
        }
        ToolId::ImageOcr => {
            ui.label(RichText::new("Tesseract language").size(12.0));
            ui.text_edit_singleline(&mut app.language);
            ui.label(RichText::new("Examples: eng, rus, eng+rus. The corresponding Tesseract language data must be installed.").size(11.0).color(MUTED));
        }
        ToolId::GenerateQr => {
            ui.label(RichText::new("Text or URL").size(12.0));
            ui.add(egui::TextEdit::multiline(&mut app.text).desired_rows(3).hint_text("https://example.com"));
        }
        ToolId::PdfRotate => {
            ui.label(RichText::new("Rotation angle").size(12.0));
            egui::ComboBox::from_id_salt("rotation").selected_text(format!("{}°", app.rotation)).show_ui(ui, |ui| {
                for angle in ["90", "180", "270"] { ui.selectable_value(&mut app.rotation, angle.to_string(), format!("{angle}°")); }
            });
        }
        ToolId::PdfCompress => {
            ui.label(RichText::new("Quality profile").size(12.0));
            egui::ComboBox::from_id_salt("pdf-compression-profile")
                .selected_text(match app.pdf_profile.as_str() {
                    "screen" => "Screen · smallest file",
                    "printer" => "Print · high quality",
                    "prepress" => "Prepress · maximum quality",
                    _ => "E-book · balanced size and quality",
                })
                .width(ui.available_width().min(360.0))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.pdf_profile, "screen".to_owned(), "Screen · compact");
                    ui.selectable_value(&mut app.pdf_profile, "ebook".to_owned(), "E-book · balanced");
                    ui.selectable_value(&mut app.pdf_profile, "printer".to_owned(), "Print · high quality");
                    ui.selectable_value(&mut app.pdf_profile, "prepress".to_owned(), "Prepress · maximum quality");
                });
            ui.label(RichText::new("The Screen profile compresses images more aggressively. Print and Prepress usually produce larger files.").size(11.0).color(MUTED));
        }
        ToolId::PdfSplit => {
            ui.label(RichText::new("Page range: start,end").size(12.0));
            ui.text_edit_singleline(&mut app.pages);
            ui.label(RichText::new("Creates one PDF per page. Use last to include through the final page, for example 1,last.").size(11.0).color(MUTED));
        }
        ToolId::PdfReorder => {
            ui.label(RichText::new("New page order").size(12.0));
            ui.text_edit_singleline(&mut app.pages);
            ui.label(RichText::new("Only listed pages are kept; omitted pages are removed. Example: 3,1,2,4.").size(11.0).color(MUTED));
        }
        ToolId::PdfToImages => {
            ui.label(RichText::new("Image format").size(12.0));
            format_combo(ui, &mut app.target_format, &["png", "jpg"]);
            ui.add_space(5.0);
            labeled_text(ui, "Resolution (DPI)", &mut app.dpi, "150");
        }
        ToolId::DataConvert => {
            ui.label(RichText::new("Output format").size(12.0));
            format_combo(ui, &mut app.target_format, &["csv", "json", "xlsx"]);
            ui.label(RichText::new("CSV → JSON treats all values as text. JSON input for CSV must be an array of objects.").size(11.0).color(MUTED));
        }
        ToolId::AudioConvert => {
            ui.label(RichText::new("Output format").size(12.0));
            format_combo(ui, &mut app.target_format, &["mp3", "wav", "flac", "aac", "m4a", "ogg", "opus"]);
            let lossless = matches!(app.target_format.as_str(), "wav" | "flac");
            ui.add_space(8.0);
            ui.add_enabled_ui(!lossless, |ui| {
                ui.label(RichText::new("Audio bitrate").size(12.0));
                egui::ComboBox::from_id_salt("audio-convert-bitrate")
                    .selected_text(format!("{} kb/s", app.audio_bitrate))
                    .width(ui.available_width().min(220.0))
                    .show_ui(ui, |ui| {
                        for bitrate in ["96", "128", "160", "192", "256", "320"] {
                            ui.selectable_value(&mut app.audio_bitrate, bitrate.to_owned(), format!("{bitrate} kb/s"));
                        }
                    });
            });
            ui.label(RichText::new(if lossless {
                "WAV and FLAC use lossless encoding. The bitrate setting is not used."
            } else {
                "Bitrate applies to MP3, AAC, M4A, OGG, and Opus. Higher values usually produce larger files."
            }).size(11.0).color(MUTED));
            ui.label(RichText::new("The first audio stream is converted and source metadata is copied when supported. Original files are kept unchanged.").size(11.0).color(MUTED));
        }
        ToolId::ExtractAudio => {
            ui.label(RichText::new("Audio format").size(12.0));
            format_combo(ui, &mut app.target_format, &["mp3", "wav", "aac", "ogg", "flac", "m4a", "opus"]);
        }
        ToolId::VideoConvert => {
            ui.label(RichText::new("Output format").size(12.0));
            format_combo(ui, &mut app.target_format, &["mp4", "webm", "mkv", "mov"]);
            ui.add_space(8.0);
            ui.label(RichText::new("Speed/size balance").size(12.0));
            egui::ComboBox::from_id_salt("video-preset")
                .selected_text(match app.video_preset.as_str() {
                    "balanced" => "Balanced quality and speed",
                    "smaller" => "Smaller file · slower",
                    _ => "Fast · best for low-end CPUs",
                })
                .width(ui.available_width().min(340.0))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.video_preset, "fast".to_owned(), "Fast · best for low-end CPUs");
                    ui.selectable_value(&mut app.video_preset, "balanced".to_owned(), "Balanced quality and speed");
                    ui.selectable_value(&mut app.video_preset, "smaller".to_owned(), "Smaller file · slower");
                });
            let mut quality = app.quality.parse::<f32>().unwrap_or(23.0).clamp(18.0, 30.0);
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Quality level: {}", quality as i32)).size(12.0)).on_hover_text("CRF setting: lower values produce better quality and larger files.");
            if ui.add(egui::Slider::new(&mut quality, 18.0..=30.0).integer().show_value(false)).changed() {
                app.quality = (quality as i32).to_string();
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("18 · higher quality, larger file").size(11.0).color(MUTED));
                ui.label(RichText::new("30 · smaller file, lower quality").size(11.0).color(MUTED));
            });
            ui.label(RichText::new("Default: fast mode and up to two threads to keep the system responsive.").size(11.0).color(MUTED));
            ui.label(RichText::new("Requires FFmpeg with H.264/AAC for MP4/MOV/MKV or VP9/Opus for WebM.").size(11.0).color(MUTED));
        }
        ToolId::DavinciResolve => {
            ui.label(RichText::new("DaVinci Resolve Linux export profile").size(12.0));
            egui::ComboBox::from_id_salt("davinci-resolve-profile")
                .selected_text(match app.resolve_profile.as_str() {
                    "hq" => "DNxHR HQ · 8-bit 4:2:2",
                    "hqx" => "DNxHR HQX · 10-bit 4:2:2",
                    "sq" => "DNxHR SQ · smaller 8-bit file",
                    "lb" => "DNxHR LB · low-bitrate editing copy",
                    "444" => "DNxHR 444 · 10-bit 4:4:4",
                    _ => "Automatic · recommended",
                })
                .width(ui.available_width().min(380.0))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.resolve_profile, "auto".to_owned(), "Automatic · recommended");
                    ui.selectable_value(&mut app.resolve_profile, "hq".to_owned(), "DNxHR HQ · 8-bit 4:2:2");
                    ui.selectable_value(&mut app.resolve_profile, "hqx".to_owned(), "DNxHR HQX · 10-bit 4:2:2");
                    ui.selectable_value(&mut app.resolve_profile, "sq".to_owned(), "DNxHR SQ · smaller 8-bit file");
                    ui.selectable_value(&mut app.resolve_profile, "lb".to_owned(), "DNxHR LB · low-bitrate editing copy");
                    ui.selectable_value(&mut app.resolve_profile, "444".to_owned(), "DNxHR 444 · 10-bit 4:4:4");
                });
            ui.add_space(8.0);
            ui.label(RichText::new("Output: QuickTime MOV · DNxHR video · Linear PCM audio at 48 kHz / 24-bit. Original resolution and timestamps are retained; source color tags are copied when available.").size(11.5).color(MUTED));
            ui.add_space(4.0);
            ui.label(RichText::new("Automatic selects HQ for 8-bit 4:2:0/4:2:2, HQX for 10-bit 4:2:0/4:2:2, and DNxHR 444 for 4:4:4/RGB sources. It does not turn 8-bit footage into real 10-bit footage.").size(11.0).color(MUTED));
            ui.add_space(4.0);
            ui.label(RichText::new("Subtitle streams are not embedded in the MOV. ForgeBox reports discarded tracks and checks the encoded file with FFprobe.").size(11.0).color(MUTED));
        }
        ToolId::VideoToGif => {
            ui.horizontal_wrapped(|ui| {
                labeled_text(ui, "Width", &mut app.width, "480");
                labeled_text(ui, "Frames per second", &mut app.fps, "12");
            });
            ui.label(RichText::new("A palette is generated automatically for cleaner colors. GIF works best for short clips.").size(11.0).color(MUTED));
        }
        ToolId::PdfWatermark => {
            ui.label(RichText::new("Select the main PDF on the left and a second PDF to use as a watermark.").size(12.0));
        }
        ToolId::ImageGrayscale | ToolId::RemoveBackground | ToolId::ExtractExif | ToolId::LongPngToPdf | ToolId::PdfMerge | ToolId::DocToPdf | ToolId::PdfToDoc => {
            ui.label(RichText::new("No additional options are required.").size(13.0));
            ui.label(RichText::new("Choose a file and an output folder, then start the conversion.").size(11.0).color(MUTED));
        }
    }
}

pub(super) fn format_combo(ui: &mut egui::Ui, selected: &mut String, formats: &[&str]) {
    egui::ComboBox::from_id_salt("target-format").selected_text(selected.to_uppercase()).show_ui(ui, |ui| {
        for format in formats { ui.selectable_value(selected, (*format).to_string(), format.to_uppercase()); }
    });
}

pub(super) fn labeled_text(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(11.5).color(MUTED));
        ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(95.0));
    });
}
