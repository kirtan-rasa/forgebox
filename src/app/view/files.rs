use crate::catalog::ToolId;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};


pub(in crate::app) fn sort_image_paths_naturally(paths: &mut [PathBuf]) {
    paths.sort_by(|left, right| {
        let left_name = left.file_name().unwrap_or_default().to_string_lossy();
        let right_name = right.file_name().unwrap_or_default().to_string_lossy();
        natural_name_cmp(&left_name, &right_name).then_with(|| left.cmp(right))
    });
}

fn natural_name_cmp(left: &str, right: &str) -> Ordering {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let (left, right) = (left.as_bytes(), right.as_bytes());
    let (mut li, mut ri) = (0, 0);

    while li < left.len() && ri < right.len() {
        if left[li].is_ascii_digit() && right[ri].is_ascii_digit() {
            let (ls, rs) = (li, ri);
            while li < left.len() && left[li].is_ascii_digit() { li += 1; }
            while ri < right.len() && right[ri].is_ascii_digit() { ri += 1; }

            let (mut ln, mut rn) = (ls, rs);
            while ln < li && left[ln] == b'0' { ln += 1; }
            while rn < ri && right[rn] == b'0' { rn += 1; }
            let numeric_order = (li - ln).cmp(&(ri - rn))
                .then_with(|| left[ln..li].cmp(&right[rn..ri]));
            if numeric_order != Ordering::Equal { return numeric_order; }

            let width_order = (li - ls).cmp(&(ri - rs));
            if width_order != Ordering::Equal { return width_order; }
        } else {
            let order = left[li].cmp(&right[ri]);
            if order != Ordering::Equal { return order; }
            li += 1;
            ri += 1;
        }
    }

    (left.len() - li).cmp(&(right.len() - ri))
}

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
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac" | "opus" | "aif" | "aiff" | "wma" | "amr" | "ac3")
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
        ToolId::AudioConvert => is_audio_file(path),
        ToolId::ExtractAudio => is_video_file(path) || is_audio_file(path),
        ToolId::VideoConvert | ToolId::VideoToGif | ToolId::DavinciResolve | ToolId::VideoSticker => is_video_file(path),
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
        ToolId::AudioConvert => "audio files",
        ToolId::ExtractAudio => "audio or video files",
        ToolId::VideoConvert | ToolId::VideoToGif | ToolId::DavinciResolve | ToolId::VideoSticker => "video files",
        _ => "image files",
    }
}

pub(super) fn file_glyph(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase().as_str() {
        "pdf" => "󰈙",
        "png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff" | "heic" | "avif" | "gif" | "bmp" => "󰋩",
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "mpeg" | "mpg" | "3gp" | "ogv" | "flv" | "mts" | "m2ts" => "▶",
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac" | "opus" | "aif" | "aiff" | "wma" | "amr" | "ac3" => "♫",
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


#[cfg(test)]
mod natural_sort_tests {
    use super::sort_image_paths_naturally;
    use std::path::PathBuf;

    #[test]
    fn numbered_image_names_sort_numerically() {
        let mut paths = vec![
            PathBuf::from("/pages/PDF10.png"),
            PathBuf::from("/pages/PDF2.png"),
            PathBuf::from("/pages/PDF1.png"),
        ];
        sort_image_paths_naturally(&mut paths);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/pages/PDF1.png"),
                PathBuf::from("/pages/PDF2.png"),
                PathBuf::from("/pages/PDF10.png"),
            ]
        );
    }

    #[test]
    fn natural_sort_is_case_insensitive_and_stable_for_zero_padded_numbers() {
        let mut paths = vec![
            PathBuf::from("/pages/page10.png"),
            PathBuf::from("/pages/Page002.png"),
            PathBuf::from("/pages/page2.png"),
            PathBuf::from("/pages/PAGE1.png"),
        ];
        sort_image_paths_naturally(&mut paths);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/pages/PAGE1.png"),
                PathBuf::from("/pages/page2.png"),
                PathBuf::from("/pages/Page002.png"),
                PathBuf::from("/pages/page10.png"),
            ]
        );
    }
}
