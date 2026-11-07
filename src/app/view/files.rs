use crate::catalog::ToolId;
use std::path::{Path, PathBuf};

pub(super) fn reorder_item<T>(items: &mut Vec<T>, from: usize, insertion_index: usize) {
    if from >= items.len() { return; }
    let item = items.remove(from);
    let target = if from < insertion_index { insertion_index.saturating_sub(1) } else { insertion_index };
    items.insert(target.min(items.len()), item);
}

pub(super) fn reorder_item_to_index<T>(items: &mut Vec<T>, from: usize, target_index: usize) {
    if from >= items.len() { return; }
    let item = items.remove(from);
    items.insert(target_index.min(items.len()), item);
}

pub(super) fn file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub(super) fn is_video_file(path: &Path) -> bool {
    matches!(file_extension(path).as_str(),
        "mp4" | "mkv" | "mov" | "webm" | "avi" | "m4v" | "mpeg" | "mpg" | "3gp" | "ogv" | "flv" | "mts" | "m2ts")
}

pub(super) fn is_image_file(path: &Path) -> bool {
    matches!(file_extension(path).as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff" | "bmp" | "heic" | "avif" | "gif")
}

pub(super) fn is_audio_file(path: &Path) -> bool {
    matches!(file_extension(path).as_str(),
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac")
}

pub(in crate::app) fn is_supported_input_file(tool: ToolId, path: &Path) -> bool {
    match tool {
        ToolId::GenerateQr => false,
        ToolId::PdfMerge | ToolId::PdfSplit | ToolId::PdfCompress | ToolId::PdfRotate
        | ToolId::PdfReorder | ToolId::PdfToImages | ToolId::PdfWatermark | ToolId::PdfToDoc => {
            file_extension(path) == "pdf"
        }
        ToolId::DocToPdf => {
            matches!(file_extension(path).as_str(), "doc" | "docx" | "odt" | "rtf")
        }
        ToolId::DataConvert => {
            matches!(file_extension(path).as_str(), "csv" | "json" | "xlsx")
        }
        ToolId::ExtractAudio => is_video_file(path) || is_audio_file(path),
        ToolId::VideoConvert | ToolId::VideoToGif | ToolId::DavinciResolve => is_video_file(path),
        ToolId::ImagesToPdf | ToolId::ImageConvert | ToolId::ImageResize | ToolId::ImageCompress
        | ToolId::ImageGrayscale | ToolId::RemoveBackground | ToolId::ExtractExif
        | ToolId::ImageOcr | ToolId::LongPngToPdf => is_image_file(path),
    }
}

pub(in crate::app) fn accepted_input_description(tool: ToolId) -> &'static str {
    match tool {
        ToolId::GenerateQr => "text",
        ToolId::PdfMerge | ToolId::PdfSplit | ToolId::PdfCompress | ToolId::PdfRotate
        | ToolId::PdfReorder | ToolId::PdfToImages | ToolId::PdfWatermark | ToolId::PdfToDoc => "PDF files",
        ToolId::DocToPdf => "DOC, DOCX, ODT, or RTF files",
        ToolId::DataConvert => "CSV, JSON, or XLSX files",
        ToolId::ExtractAudio => "audio or video files",
        ToolId::VideoConvert | ToolId::VideoToGif | ToolId::DavinciResolve => "video files",
        _ => "image files",
    }
}

pub(super) fn file_glyph(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase().as_str() {
        "pdf" => "󰈙",
        "png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff" | "heic" | "avif" | "gif" | "bmp" => "󰋩",
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "mpeg" | "mpg" | "3gp" | "ogv" | "flv" | "mts" | "m2ts" => "▶",
        "csv" | "json" | "xlsx" => "󰘦",
        _ => "󰈔",
    }
}

pub(super) fn output_dir_for_home(home: &Path) -> PathBuf {
    let downloads = home.join("Downloads");
    if downloads.is_dir() {
        downloads
    } else {
        home.to_path_buf()
    }
}

pub(in crate::app) fn default_output_dir() -> PathBuf {
    let home = std::env::var_os("HOME");

    if let Some(home) = home.map(PathBuf::from).filter(|path| path.is_dir()) {
        return output_dir_for_home(&home);
    }

    std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
}
