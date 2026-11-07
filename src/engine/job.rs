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
    Opus => "opus",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveProfile {
    Auto,
    Hq,
    Hqx,
    Sq,
    Lb,
    Dnxhr444,
}

impl ResolveProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Hq => "hq",
            Self::Hqx => "hqx",
            Self::Sq => "sq",
            Self::Lb => "lb",
            Self::Dnxhr444 => "444",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "hq" => Ok(Self::Hq),
            "hqx" => Ok(Self::Hqx),
            "sq" => Ok(Self::Sq),
            "lb" => Ok(Self::Lb),
            "444" => Ok(Self::Dnxhr444),
            _ => Err(format!("Unsupported DaVinci Resolve DNxHR profile: {value:?}.")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoStickerFit {
    Crop,
    Fit,
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
    AudioConvert { format: AudioFormat, bitrate: u16 },
    ExtractAudio { format: AudioFormat },
    VideoConvert { format: VideoFormat, preset: VideoPreset, quality: u8 },
    DavinciResolve { profile: ResolveProfile },
    VideoToGif { width: u16, fps: u8 },
    VideoSticker { start_ms: u32, end_ms: u32, fit: VideoStickerFit, crop_left: u16, crop_top: u16, zoom_milli: u16 },
}

impl JobOptions {
    pub fn matches_tool(&self, tool: ToolId) -> bool {
        matches!(
            (tool, self),
            (ToolId::ImageConvert, Self::ImageConvert { .. })
                | (ToolId::ImageResize, Self::ImageResize { .. })
                | (ToolId::ImageCompress, Self::ImageCompress { .. })
                | (ToolId::ImagesToPdf, Self::ImagesToPdf { .. })
                | (ToolId::ImageOcr, Self::ImageOcr { .. })
                | (ToolId::GenerateQr, Self::GenerateQr { .. })
                | (ToolId::PdfSplit, Self::PdfSplit { .. })
                | (ToolId::PdfCompress, Self::PdfCompress { .. })
                | (ToolId::PdfRotate, Self::PdfRotate { .. })
                | (ToolId::PdfReorder, Self::PdfReorder { .. })
                | (ToolId::PdfToImages, Self::PdfToImages { .. })
                | (ToolId::PdfWatermark, Self::PdfWatermark { .. })
                | (ToolId::DataConvert, Self::DataConvert { .. })
                | (ToolId::AudioConvert, Self::AudioConvert { .. })
                | (ToolId::ExtractAudio, Self::ExtractAudio { .. })
                | (ToolId::VideoConvert, Self::VideoConvert { .. })
                | (ToolId::DavinciResolve, Self::DavinciResolve { .. })
                | (ToolId::VideoToGif, Self::VideoToGif { .. })
                | (ToolId::VideoSticker, Self::VideoSticker { .. })
                | (ToolId::ImageGrayscale, Self::None)
                | (ToolId::RemoveBackground, Self::None)
                | (ToolId::ExtractExif, Self::None)
                | (ToolId::LongPngToPdf, Self::None)
                | (ToolId::PdfMerge, Self::None)
                | (ToolId::DocToPdf, Self::None)
                | (ToolId::PdfToDoc, Self::None)
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

const MAX_IMAGE_RESIZE_DIMENSION: u32 = 16_384;
const MAX_IMAGE_RESIZE_PIXELS: u64 = 64_000_000;

impl Job {
    pub fn new(tool: ToolId, files: Vec<PathBuf>, output_dir: PathBuf, options: JobOptions) -> Result<Self, String> {
        if !options.matches_tool(tool) {
            return Err(format!("The settings do not match the selected tool ({tool:?})."));
        }

        if let JobOptions::VideoSticker { start_ms, end_ms, crop_left, crop_top, zoom_milli, .. } = &options {
            let duration_ms = end_ms.saturating_sub(*start_ms);
            if duration_ms == 0 || duration_ms > 3_000 {
                return Err("A Telegram video sticker must be between 1 ms and 3 seconds long.".into());
            }
            if *crop_left > 1_000 || *crop_top > 1_000 {
                return Err("Video sticker crop position must be between 0 and 1000.".into());
            }
            if !(1_000..=4_000).contains(zoom_milli) {
                return Err("Video sticker crop zoom must be between 1× and 4×.".into());
            }
        }

        if let JobOptions::AudioConvert { bitrate, .. } = &options {
            if !(64..=320).contains(bitrate) {
                return Err("Audio bitrate must be between 64 and 320 kb/s.".into());
            }
        }

        if let JobOptions::ImageResize { width, height } = &options {
            let requested_pixels = u64::from(*width) * u64::from(*height);
            if *width == 0 || *height == 0 {
                return Err("Image resize width and height must be greater than zero.".into());
            }
            if *width > MAX_IMAGE_RESIZE_DIMENSION
                || *height > MAX_IMAGE_RESIZE_DIMENSION
                || requested_pixels > MAX_IMAGE_RESIZE_PIXELS
            {
                return Err(
                    "Image resize bounds are limited to 16,384 px per side and 64 megapixels in total."
                        .into(),
                );
            }
        }

        Ok(Self { tool, files, output_dir, options })
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioFormat, ImageFormat, JobOptions, PdfRotation, ResolveProfile, VideoFormat, VideoStickerFit};
    use crate::catalog::ToolId;

    #[test]
    fn job_options_reject_a_mismatched_tool() {
        let options = JobOptions::PdfSplit { range: "1,last".into() };
        assert!(!options.matches_tool(ToolId::PdfReorder));
        assert!(options.matches_tool(ToolId::PdfSplit));
    }

    #[test]
    fn audio_formats_include_opus_and_bitrate_is_bounded() {
        use super::{Job, JobOptions};
        use std::path::PathBuf;

        assert_eq!(AudioFormat::parse("OPUS").unwrap().as_str(), "opus");
        let make_job = |bitrate| Job::new(
            ToolId::AudioConvert,
            vec![PathBuf::from("track.flac")],
            PathBuf::from("/tmp"),
            JobOptions::AudioConvert { format: AudioFormat::Mp3, bitrate },
        );
        assert!(make_job(64).is_ok());
        assert!(make_job(320).is_ok());
        assert!(make_job(63).is_err());
        assert!(make_job(321).is_err());
    }

    #[test]
    fn video_sticker_job_enforces_three_second_limit_and_square_crop_bounds() {
        use super::{Job, JobOptions, VideoStickerFit};
        use std::path::PathBuf;
        let make = |start_ms, end_ms, crop_left, zoom_milli| Job::new(
            ToolId::VideoSticker,
            vec![PathBuf::from("clip.mp4")],
            PathBuf::from("/tmp"),
            JobOptions::VideoSticker {
                start_ms, end_ms, fit: VideoStickerFit::Crop,
                crop_left, crop_top: 500, zoom_milli,
            },
        );
        assert!(make(0, 3_000, 500, 1_000).is_ok());
        assert!(make(500, 3_500, 0, 4_000).is_ok());
        assert!(make(0, 3_001, 500, 1_000).is_err());
        assert!(make(0, 0, 500, 1_000).is_err());
        assert!(make(0, 1_000, 1_001, 1_000).is_err());
        assert!(make(0, 1_000, 500, 4_001).is_err());
    }

    #[test]
    fn image_resize_rejects_zero_and_excessive_bounds() {
        use super::{Job, JobOptions};
        use std::path::PathBuf;

        let make_job = |width, height| {
            Job::new(
                ToolId::ImageResize,
                Vec::new(),
                PathBuf::from("/tmp"),
                JobOptions::ImageResize { width, height },
            )
        };

        assert!(make_job(0, 100).is_err());
        assert!(make_job(16_385, 1).is_err());
        assert!(make_job(8_001, 8_000).is_err());
        assert!(make_job(8_000, 8_000).is_ok());
    }

    #[test]
    fn fixed_format_values_are_parsed_and_normalized() {
        assert_eq!(ImageFormat::parse("jpeg").unwrap(), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::Jpeg.as_str(), "jpg");
        assert_eq!(VideoFormat::parse("WEBM").unwrap().as_str(), "webm");
        assert_eq!(PdfRotation::parse("270").unwrap().degrees(), 270);
        assert!(ImageFormat::parse("bmp").is_err());
        assert_eq!(ResolveProfile::parse("AUTO").unwrap(), ResolveProfile::Auto);
        assert_eq!(ResolveProfile::parse("444").unwrap().as_str(), "444");
        assert!(ResolveProfile::parse("dnxhd").is_err());
    }
}
