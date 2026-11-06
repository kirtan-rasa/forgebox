use crate::catalog::ToolId;
use std::path::PathBuf;

macro_rules! string_option {
    ($name:ident, $label:literal, { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant,)+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value,)+
                }
            }

            pub fn parse(value: &str) -> Result<Self, String> {
                match value.trim().to_ascii_lowercase().as_str() {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(format!("Unsupported {}: `{}`.", $label, value)),
                }
            }
        }
    };
}

string_option!(DataFormat, "data format", {
    Csv => "csv",
    Json => "json",
    Xlsx => "xlsx",
});

string_option!(AudioFormat, "audio format", {
    Mp3 => "mp3",
    Wav => "wav",
    Aac => "aac",
    Ogg => "ogg",
    Flac => "flac",
    M4a => "m4a",
});

string_option!(VideoFormat, "video format", {
    Mp4 => "mp4",
    Webm => "webm",
    Mkv => "mkv",
    Mov => "mov",
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Tiff,
    Heic,
}

impl ImageFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Tiff => "tiff",
            Self::Heic => "heic",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            "webp" => Ok(Self::Webp),
            "tif" | "tiff" => Ok(Self::Tiff),
            "heic" => Ok(Self::Heic),
            _ => Err(format!("Unsupported image format: `{value}`.")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdfImageFormat {
    Png,
    Jpeg,
}

impl PdfImageFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            _ => Err(format!("Unsupported PDF image format: `{value}`.")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdfCompressionProfile {
    Screen,
    Ebook,
    Printer,
    Prepress,
}

impl PdfCompressionProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Ebook => "ebook",
            Self::Printer => "printer",
            Self::Prepress => "prepress",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "screen" => Ok(Self::Screen),
            "ebook" => Ok(Self::Ebook),
            "printer" => Ok(Self::Printer),
            "prepress" => Ok(Self::Prepress),
            _ => Err(format!("Unsupported PDF compression profile: `{value}`.")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdfRotation {
    Degrees90,
    Degrees180,
    Degrees270,
}

impl PdfRotation {
    pub const fn degrees(self) -> u16 {
        match self {
            Self::Degrees90 => 90,
            Self::Degrees180 => 180,
            Self::Degrees270 => 270,
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "90" => Ok(Self::Degrees90),
            "180" => Ok(Self::Degrees180),
            "270" => Ok(Self::Degrees270),
            _ => Err(format!("Unsupported PDF rotation angle: `{value}`.")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoPreset {
    Fast,
    Balanced,
    Smaller,
}

impl VideoPreset {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Smaller => "smaller",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fast" => Ok(Self::Fast),
            "balanced" => Ok(Self::Balanced),
            "smaller" => Ok(Self::Smaller),
            _ => Err(format!("Unsupported video preset: `{value}`.")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobOptions {
    None,
    ImageConvert { format: ImageFormat },
    ImageResize { width: u32, height: u32 },
    ImageCompress { quality: u8 },
    ImagesToPdf { compress: bool, quality: u8 },
    ImageOcr { language: String },
    GenerateQr { text: String },
    PdfSplit { range: String },
    PdfCompress { profile: PdfCompressionProfile },
    PdfRotate { rotation: PdfRotation },
    PdfReorder { order: String },
    PdfToImages { format: PdfImageFormat, dpi: u16 },
    PdfWatermark { watermark_file: PathBuf },
    DataConvert { format: DataFormat },
    ExtractAudio { format: AudioFormat },
    VideoConvert { format: VideoFormat, preset: VideoPreset, quality: u8 },
    VideoToGif { width: u16, fps: u8 },
}

impl JobOptions {
    pub fn matches_tool(&self, tool: ToolId) -> bool {
        matches!(
            (tool, self),
            (ToolId::ImageConvert, Self::ImageConvert { .. })
                | (ToolId::ImageResize, Options::ImageResize { .. })
                | (Tool::ImageCompress, Options::ImageCompress { .. })
                | (Tool::ImagesToPdf, Options::ImagesToPdf { .. })
                | (Tool::ImageOcr, Options::ImageOcr { .. })
                | (Tool::GenerateQr, Options::GenerateQr { .. })
                | (Tool::PdfSplit, Options::PdfSplit { .. })
                | (Tool::PdfCompress, Options::PdfCompress { .. })
                | (Tool::PdfRotate, Options::PdfRotate { .. })
                | (Tool::PdfReorder, Options::PdfReorder { .. })
                | (Tool::PdfToImages, Options::PdfToImages { .. })
                | (Tool::PdfWatermark, Options::PdfWatermark { .. })
                | (Tool::DataConvert, Options::DataConvert { .. })
                | (Tool::ExtractAudio, Options::ExtractAudio { .. })
                | (Tool::VideoConvert, Options::VideoConvert { .. })
                | (Tool::VideoToGif, Options::VideoToGif { .. })
                | (Tool::ImageGrayscale, Options::None)
                | (Tool::RemoveBackground, Options::None)
                | (Tool::ExtractExif, Options::None)
                | (Tool::LongPngToPdf, Options::None)
                | (Tool::PdfMerge, Options::None)
                | (Tool::DocToPdf, Options::None)
                | (Tool::PdfToDoc, Options::None)
        )
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub tool: ToolId,
    pub files: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub options: JobOptions,
}

impl Job {
    pub fn new(tool: ToolId, files: Vec<PathBuf>, output_dir: PathBuf, options: JobOptions) -> Result<Self, String> {
        if !options.matches_tool(tool) {
            return Err(format!("The settings do not match the selected tool ({tool:?})."));
        }
        Ok(Self { tool, files, output_dir, options })
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageFormat, JobOptions, PdfRotation, VideoFormat};
    use crate::catalog::ToolId;

    #[test]
    fn job_options_reject_a_mismatched_tool() {
        let options = JobOptions::PdfSplit { range: "1,last".into() };
        assert!(!options.matches_tool(ToolId::PdfReorder));
        assert!(options.matches_tool(ToolId::PdfSplit));
    }

    #[test]
    fn fixed_format_values_are_parsed_and_normalized() {
        assert_eq!(ImageFormat::parse("jpeg").unwrap(), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::Jpeg.as_str(), "jpg");
        assert_eq!(VideoFormat::parse("WEBM").unwrap().as_str(), "webm");
        assert_eq!(PdfRotation::parse("270").unwrap().degrees(), 270);
        assert!(ImageFormat::parse("bmp").is_err());
    }
}
