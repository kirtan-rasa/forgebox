use super::{
    normalized_ext, os, positive, record_execution_warning, report_status, run_ok,
    run_ok_with_timeout, ResolveProfile,
};
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

const MEDIA_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

fn run_ffprobe(args: Vec<std::ffi::OsString>) -> Result<std::process::Output, String> {
    run_ok_with_timeout("ffprobe", args, MEDIA_PROBE_TIMEOUT)
}

fn is_hdr_transfer(transfer: Option<&str>) -> bool {
    matches!(transfer, Some("smpte2084" | "arib-std-b67"))
}

fn probe_video_conversion(input: &Path) -> Result<(u8, Option<String>), String> {
    let output = run_ffprobe(vec![
        os("-v"), os("error"),
        os("-select_streams"), os("v:0"),
        os("-show_entries"), os("stream=pix_fmt,bits_per_raw_sample,color_transfer"),
        os("-of"), os("json"),
        os(input),
    ]).map_err(|error| format!("Could not inspect the video bit depth and HDR transfer: {error}"))?;
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("FFprobe returned invalid video metadata: {error}"))?;
    let stream = json.get("streams").and_then(Value::as_array)
        .and_then(|streams| streams.first())
        .ok_or_else(|| "FFprobe did not find a video stream in the selected file.".to_string())?;
    let pixel_format = value_string(stream, "pix_fmt").unwrap_or("unknown");
    let raw_depth = stream.get("bits_per_raw_sample")
        .and_then(|value| value.as_str().and_then(|text| text.parse::<u8>().ok())
            .or_else(|| value.as_u64().map(|number| number as u8)))
        .unwrap_or(0);
    let bit_depth = raw_depth.max(inferred_pixel_depth(pixel_format));
    Ok((bit_depth, parse_optional_string(stream, "color_transfer")))
}

fn ffmpeg_supports_filter(filter: &str) -> bool {
    run_ok_with_timeout(
        "ffmpeg",
        vec![os("-hide_banner"), os("-h"), os(&format!("filter={filter}"))],
        MEDIA_PROBE_TIMEOUT,
    ).is_ok()
}

pub(super) fn convert_video(input: &Path, output: &Path, target_format: &str, preset: &str, quality: &str) -> Result<(), String> {
    let format = normalized_ext(target_format);
    if !["mp4", "webm", "mkv", "mov"].contains(&format.as_str()) {
        return Err(format!(
            "Video format \"{format}\" is not supported. Choose MP4, WebM, MKV, or MOV."
        ));
    }
    let crf = positive(quality, 23).clamp(18, 30).to_string();
    let (preset_label, x264_preset, vp9_cpu_used) = match preset {
        "balanced" => ("balanced", "medium", "4"),
        "smaller" => ("smaller", "slow", "2"),
        _ => ("fast", "veryfast", "6"),
    };
    let (source_bit_depth, source_transfer) = probe_video_conversion(input)?;
    let source_is_hdr = is_hdr_transfer(source_transfer.as_deref());
    if source_is_hdr && !ffmpeg_supports_filter("zscale") {
        return Err(
            "The source uses HDR (PQ/HLG), but this FFmpeg build does not include the zscale filter.              Install an FFmpeg build with libzimg support; conversion was stopped to avoid damaging HDR highlights."
                .into()
        );
    }
    if source_bit_depth > 8 && !source_is_hdr && format != "webm" {
        record_execution_warning(format!(
            "The source is {source_bit_depth}-bit SDR, while the selected H.264 output is 8-bit. Some source precision will be reduced."
        ));
    }
    if source_is_hdr {
        record_execution_warning(
            "The HDR source was tone-mapped to SDR BT.709 for compatibility with the selected output format."
                .into()
        );
    }

    let video_filter = if source_is_hdr {
        "zscale=transfer=linear:npl=100,format=gbrpf32le,tonemap=tonemap=hable:desat=0,zscale=transfer=bt709:primaries=bt709:matrix=bt709,scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p"
    } else {
        "scale=trunc(iw/2)*2:trunc(ih/2)*2"
    };
    let mut args = vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"),
        os("-i"), os(input),
        os("-vf"), os(video_filter),
        os("-map"), os("0:v:0"), os("-map"), os("0:a?"),
        // Conversion is privacy-first: do not carry source metadata (including GPS).
        os("-map_metadata"), os("-1"),
        os("-map_metadata:s:v"), os("-1"),
        os("-map_metadata:s:a"), os("-1"),
        os("-map_chapters"), os("-1"),
        os("-threads"), os("2"),
    ];
    if source_is_hdr {
        args.extend([
            os("-color_primaries"), os("bt709"),
            os("-color_trc"), os("bt709"),
            os("-colorspace"), os("bt709"),
        ]);
    }
    match format.as_str() {
        "mp4" | "mov" | "mkv" => {
            args.extend([
                os("-c:v"), os("libx264"), os("-preset"), os(x264_preset),
                os("-crf"), os(&crf), os("-pix_fmt"), os("yuv420p"),
                os("-c:a"), os("aac"), os("-b:a"), os("160k"),
            ]);
            if format == "mp4" || format == "mov" {
                args.extend([os("-movflags"), os("+faststart")]);
            }
        }
        "webm" => {
            let vp9_crf = (positive(quality, 23) + 9).clamp(24, 39).to_string();
            let pixel_format = if source_bit_depth > 8 && !source_is_hdr {
                "yuv420p10le"
            } else {
                "yuv420p"
            };
            args.extend([
                os("-c:v"), os("libvpx-vp9"), os("-deadline"), os("good"),
                os("-cpu-used"), os(vp9_cpu_used), os("-crf"), os(&vp9_crf), os("-b:v"), os("0"),
                os("-pix_fmt"), os(pixel_format),
                os("-c:a"), os("libopus"), os("-b:a"), os("128k"),
            ]);
        }
        _ => return Err(format!("Video format \"{format}\" is not supported. Choose MP4, WebM, MKV, or MOV.")),
    }
    args.push(os(output));
    run_ok("ffmpeg", args).map(|_| ()).map_err(|error| {
        if error.contains("Unknown encoder") || error.contains("Encoder not found") {
            format!("This FFmpeg build does not include a codec for {}. Install an FFmpeg build with {} support.", format.to_uppercase(), if format == "webm" { "VP9 and Opus" } else { "H.264/AAC" })
        } else if error.contains("does not contain any stream") || error.contains("matches no streams") {
            format!("No video stream found in {}.", input.file_name().and_then(|v| v.to_str()).unwrap_or("file"))
        } else {
            format!("FFmpeg error while encoding {} ({preset_label}): {error}", format.to_uppercase())
        }
    })
}

pub(super) fn convert_audio(
    input: &Path,
    output: &Path,
    target_format: &str,
    bitrate: u16,
) -> Result<(), String> {
    let format = normalized_ext(target_format);
    let bitrate = bitrate.clamp(64, 320).to_string();
    let mut args = vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"),
        os("-i"), os(input),
        os("-map"), os("0:a:0"),
        // Avoid carrying source tags (including location metadata) into the audio output.
        os("-map_metadata"), os("-1"),
        os("-map_chapters"), os("-1"),
        os("-vn"), os("-sn"), os("-dn"),
        os("-threads"), os("2"),
    ];

    let encoder = match format.as_str() {
        "mp3" => {
            args.extend([os("-c:a"), os("libmp3lame"), os("-b:a"), os(&format!("{bitrate}k"))]);
            "libmp3lame"
        }
        "wav" => {
            args.extend([os("-c:a"), os("pcm_s16le")]);
            "pcm_s16le"
        }
        "aac" => {
            args.extend([os("-c:a"), os("aac"), os("-b:a"), os(&format!("{bitrate}k"))]);
            "aac"
        }
        "ogg" => {
            args.extend([os("-c:a"), os("libvorbis"), os("-b:a"), os(&format!("{bitrate}k"))]);
            "libvorbis"
        }
        "flac" => {
            args.extend([os("-c:a"), os("flac")]);
            "flac"
        }
        "m4a" => {
            args.extend([
                os("-c:a"), os("aac"), os("-b:a"), os(&format!("{bitrate}k")),
                os("-movflags"), os("+faststart"),
            ]);
            "aac"
        }
        "opus" => {
            args.extend([
                os("-c:a"), os("libopus"), os("-b:a"), os(&format!("{bitrate}k")),
                os("-vbr"), os("on"),
            ]);
            "libopus"
        }
        _ => return Err(format!(
            "Audio format \"{format}\" is not supported. Choose MP3, WAV, AAC, OGG, FLAC, M4A, or Opus."
        )),
    };

    args.push(os(output));
    run_ok("ffmpeg", args).map(|_| ()).map_err(|error| {
        if error.contains("matches no streams") || error.contains("does not contain any stream") {
            "No audio stream was found in the selected file.".into()
        } else if error.contains("Unknown encoder") || error.contains("Encoder not found") {
            format!(
                "This FFmpeg build does not include the {encoder} encoder required for {} output. Install a full FFmpeg build with that codec.",
                format.to_uppercase()
            )
        } else {
            format!("FFmpeg audio conversion to {} failed: {error}", format.to_uppercase())
        }
    })
}

const TELEGRAM_STICKER_MAX_BYTES: u64 = 256 * 1024;

#[derive(Clone, Copy, Debug)]
struct StickerVideoInfo {
    width: u32,
    height: u32,
}

fn sticker_video_info(input: &Path) -> Result<StickerVideoInfo, String> {
    let output = run_ffprobe(vec![
        os("-v"), os("error"),
        os("-select_streams"), os("v:0"),
        // Older FFprobe releases reject stream_side_data in -show_entries.
        // show_streams includes side_data_list in the stream JSON on versions
        // that expose display-matrix rotation metadata.
        os("-show_streams"),
        os("-of"), os("json"),
        os(input),
    ]).map_err(|error| format!("Could not inspect the source video: {error}"))?;
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("FFprobe returned invalid source-video data: {error}"))?;
    let stream = json.get("streams")
        .and_then(Value::as_array)
        .and_then(|streams| streams.first())
        .ok_or_else(|| "No video stream was found in the selected file.".to_string())?;
    let width = stream.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
    let height = stream.get("height").and_then(Value::as_u64).unwrap_or(0) as u32;
    let rotation = stream.get("side_data_list")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find_map(|item| item.get("rotation").and_then(Value::as_f64)))
        .or_else(|| stream.get("tags").and_then(|tags| tags.get("rotate"))
            .and_then(Value::as_str).and_then(|value| value.parse::<f64>().ok()));
    let (width, height) = display_oriented_dimensions(width, height, rotation);
    if width < 2 || height < 2 {
        return Err("The selected video has invalid dimensions.".into());
    }
    Ok(StickerVideoInfo { width, height })
}

fn display_oriented_dimensions(width: u32, height: u32, rotation: Option<f64>) -> (u32, u32) {
    let rotation = rotation.unwrap_or(0.0).round() as i32;
    if rotation.rem_euclid(180) == 90 { (height, width) } else { (width, height) }
}

fn sticker_video_filter(
    info: StickerVideoInfo,
    fit: super::VideoStickerFit,
    crop_left: u16,
    crop_top: u16,
    zoom_milli: u16,
) -> Result<String, String> {
    use super::VideoStickerFit;

    match fit {
        VideoStickerFit::Fit => Ok(
            "scale=512:512:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=lanczos,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=black@0,setsar=1,fps=30,format=yuva420p"
                .into(),
        ),
        VideoStickerFit::Crop => {
            let min_side = info.width.min(info.height) & !1;
            if min_side < 2 {
                return Err("The source video is too small to create a square sticker.".into());
            }
            let zoom = u32::from(zoom_milli.clamp(1_000, 4_000));
            let crop_side = ((u64::from(min_side) * 1_000 / u64::from(zoom)) as u32 & !1).max(2);
            let available_x = info.width.saturating_sub(crop_side);
            let available_y = info.height.saturating_sub(crop_side);
            let x = (available_x * u32::from(crop_left.min(1_000)) / 1_000) & !1;
            let y = (available_y * u32::from(crop_top.min(1_000)) / 1_000) & !1;
            Ok(format!(
                "crop={crop_side}:{crop_side}:{x}:{y},scale=512:512:flags=lanczos,setsar=1,fps=30,format=yuva420p"
            ))
        }
    }
}

fn encode_sticker_pass(
    input: &Path,
    output: &Path,
    start_ms: u32,
    duration_ms: u32,
    filter: &str,
    bitrate_kbps: u32,
    pass: u8,
    passlog: &Path,
) -> Result<(), String> {
    let seek = format!("{:.3}", start_ms as f64 / 1_000.0);
    let duration = format!("{:.3}", duration_ms as f64 / 1_000.0);
    let mut args = vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"),
        os("-ss"), os(&seek),
        os("-i"), os(input),
        os("-t"), os(&duration),
        os("-map"), os("0:v:0"),
        os("-vf"), os(filter),
        os("-an"), os("-sn"), os("-dn"),
        os("-c:v"), os("libvpx-vp9"),
        os("-pix_fmt"), os("yuva420p"),
        os("-metadata:s:v:0"), os("alpha_mode=1"),
        os("-auto-alt-ref"), os("0"),
        os("-deadline"), os("good"),
        os("-cpu-used"), os("4"),
        os("-threads"), os("2"),
        os("-b:v"), os(&format!("{bitrate_kbps}k")),
        os("-pass"), os(pass.to_string()),
        os("-passlogfile"), os(passlog),
        os("-map_metadata"), os("-1"),
        os("-map_chapters"), os("-1"),
    ];

    if pass == 1 {
        args.extend([os("-f"), os("null"), os("-")]);
    } else {
        args.push(os(output));
    }
    run_ok("ffmpeg", args).map(|_| ()).map_err(|error| {
        if error.contains("Unknown encoder") || error.contains("Encoder not found") {
            "This FFmpeg build does not include the libvpx-vp9 encoder required for Telegram video stickers.".into()
        } else {
            format!("VP9 sticker encoding failed: {error}")
        }
    })
}

fn validate_sticker_output(path: &Path) -> Result<(), String> {
    let size = std::fs::metadata(path)
        .map_err(|error| format!("Could not read the exported sticker: {error}"))?
        .len();
    if size > TELEGRAM_STICKER_MAX_BYTES {
        return Err(format!(
            "The exported sticker is {} KB, above Telegram's 256 KB limit. Try a shorter or less detailed clip.",
            size.div_ceil(1_024)
        ));
    }

    let output = run_ffprobe(vec![
        os("-v"), os("error"),
        os("-show_entries"), os("stream=codec_type,codec_name,width,height:format=duration"),
        os("-of"), os("json"),
        os(path),
    ]).map_err(|error| format!("Could not verify the exported WebM sticker: {error}"))?;
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("FFprobe returned invalid sticker data: {error}"))?;
    let streams = json.get("streams").and_then(Value::as_array)
        .ok_or_else(|| "The exported WebM does not contain a readable video stream.".to_string())?;
    if streams.len() != 1
        || streams.first().and_then(|stream| stream.get("codec_type")).and_then(Value::as_str) != Some("video")
        || streams.first().and_then(|stream| stream.get("codec_name")).and_then(Value::as_str) != Some("vp9")
    {
        return Err("The exported sticker must contain one VP9 video stream and no audio.".into());
    }
    let stream = &streams[0];
    let width = stream.get("width").and_then(Value::as_u64).unwrap_or(0);
    let height = stream.get("height").and_then(Value::as_u64).unwrap_or(0);
    if width != 512 || height != 512 {
        return Err(format!("The exported sticker has unexpected dimensions: {width}×{height}; expected 512×512."));
    }
    let duration = json.get("format")
        .and_then(|format| format.get("duration"))
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    if duration <= 0.0 || duration > 3.01 {
        return Err(format!("The exported sticker duration is {duration:.2} seconds; Telegram allows at most 3 seconds."));
    }
    Ok(())
}

pub(super) fn convert_video_sticker(
    input: &Path,
    output: &Path,
    start_ms: u32,
    end_ms: u32,
    fit: super::VideoStickerFit,
    crop_left: u16,
    crop_top: u16,
    zoom_milli: u16,
) -> Result<(), String> {
    let duration_ms = end_ms.saturating_sub(start_ms);
    if duration_ms == 0 || duration_ms > 3_000 {
        return Err("Choose a video segment between 1 ms and 3 seconds.".into());
    }
    let info = sticker_video_info(input)?;
    let filter = sticker_video_filter(info, fit, crop_left, crop_top, zoom_milli)?;

    let work_dir = super::temp_dir("sticker")
        .map_err(|error| format!("Could not prepare temporary sticker-encoding files: {error}"))?;
    let passlog = work_dir.join("vp9-pass");

    // Leave room for WebM container overhead and metadata, then lower the
    // bitrate and repeat the two-pass encode if a complex scene exceeds 256 KiB.
    let target_payload = TELEGRAM_STICKER_MAX_BYTES.saturating_sub(8 * 1_024);
    let mut bitrate = ((target_payload * 8 * 1_000 / u64::from(duration_ms)) * 85 / 100)
        .clamp(64_000, 1_800_000) as u32 / 1_000;
    let result = (|| {
        for attempt in 0..5 {
            let _ = std::fs::remove_file(output);
            encode_sticker_pass(input, output, start_ms, duration_ms, &filter, bitrate, 1, &passlog)?;
            encode_sticker_pass(input, output, start_ms, duration_ms, &filter, bitrate, 2, &passlog)?;
            let size = std::fs::metadata(output)
                .map_err(|error| format!("Could not inspect the encoded sticker size: {error}"))?
                .len();
            if size <= TELEGRAM_STICKER_MAX_BYTES {
                validate_sticker_output(output)?;
                return Ok(());
            }
            if attempt == 4 {
                break;
            }
            let reduced = (f64::from(bitrate) * TELEGRAM_STICKER_MAX_BYTES as f64 / size as f64 * 0.88) as u32;
            bitrate = reduced.max(64);
            report_status(format!(
                "Reducing sticker bitrate to {bitrate} kb/s to meet Telegram's 256 KB limit…"
            ));
        }
        Err("Could not bring this video below Telegram's 256 KB sticker limit. Choose a shorter segment or a less detailed scene.".into())
    })();

    let _ = std::fs::remove_dir_all(work_dir);
    result
}

pub(super) fn extract_audio(input: &Path, output: &Path, target_format: &str) -> Result<(), String> {
    let format = normalized_ext(target_format);
    let mut args = vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"),
        os("-i"), os(input), os("-map"), os("0:a:0"),
        os("-map_metadata"), os("-1"), os("-map_chapters"), os("-1"),
        os("-vn"),
    ];
    match format.as_str() {
        "mp3" => args.extend([os("-c:a"), os("libmp3lame"), os("-b:a"), os("192k")]),
        "wav" => args.extend([os("-c:a"), os("pcm_s16le")]),
        "aac" => args.extend([os("-c:a"), os("aac"), os("-b:a"), os("192k")]),
        "ogg" => args.extend([os("-c:a"), os("libvorbis"), os("-q:a"), os("5")]),
        "flac" => args.extend([os("-c:a"), os("flac")]),
        "m4a" => args.extend([os("-c:a"), os("aac"), os("-b:a"), os("192k")]),
        "opus" => args.extend([os("-c:a"), os("libopus"), os("-b:a"), os("192k")]),
        _ => return Err(format!(r#"Audio format "{format}" is not supported."#)),
    }
    args.push(os(output));
    run_ok("ffmpeg", args).map(|_| ()).map_err(|error| {
        if error.contains("matches no streams") || error.contains("does not contain any stream") {
            "No audio stream was found in the selected file.".into()
        } else { error }
    })
}

#[derive(Clone, Debug)]
struct VideoProbe {
    is_interlaced: bool,
    bit_depth: u8,
    is_444: bool,
    is_rgb: bool,
    has_alpha: bool,
    has_audio: bool,
    has_subtitles: bool,
    additional_video_streams: usize,
    has_data_streams: bool,
    has_attachments: bool,
    color_range: Option<String>,
    color_space: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
}

fn value_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn parse_optional_string(value: &Value, key: &str) -> Option<String> {
    value_string(value, key)
        .filter(|text| !text.is_empty() && *text != "unknown")
        .map(str::to_owned)
}

fn is_rgb_pixel_format(pixel_format: &str) -> bool {
    ["rgb", "bgr", "gbr", "argb", "abgr", "0rgb", "0bgr"]
        .iter()
        .any(|prefix| pixel_format.starts_with(prefix))
}

fn pixel_format_has_alpha(pixel_format: &str) -> bool {
    ["rgba", "bgra", "argb", "abgr", "yuva", "gbrap", "gbra"]
        .iter()
        .any(|prefix| pixel_format.starts_with(prefix))
}

fn inferred_pixel_depth(pixel_format: &str) -> u8 {
    let format = pixel_format.to_ascii_lowercase();

    // Packed RGB formats encode total bits per pixel in the suffix.
    if ["rgb48", "bgr48", "rgba64", "bgra64", "argb64", "abgr64"]
        .iter()
        .any(|prefix| format.starts_with(prefix))
    {
        return 16;
    }

    // Planar high-bit-depth formats carry component depth after p,
    // e.g. yuv420p10le, yuva444p12be, gbrp16le.
    if format.contains("p010") {
        return 10;
    }
    for depth in [16_u8, 14, 12, 10, 9] {
        if format.contains(&format!("p{depth}")) {
            return depth;
        }
    }
    if format.starts_with("gray16") || format.starts_with("ya16") {
        return 16;
    }
    8
}

fn probe_video(input: &Path) -> Result<VideoProbe, String> {
    report_status(format!("Analyzing {} with FFprobe…", input.file_name().and_then(|name| name.to_str()).unwrap_or("video")));
    let output = run_ffprobe(vec![
        os("-v"), os("error"),
        os("-show_entries"),
        os("stream=codec_type,codec_name,width,height,pix_fmt,bits_per_raw_sample,field_order,color_range,color_space,color_transfer,color_primaries,sample_rate,channels,channel_layout:format=duration"),
        os("-of"), os("json"),
        os(input),
    ]).map_err(|error| {
        if error.contains("not found") {
            "FFprobe is required for DaVinci Resolve export. Install it together with FFmpeg (it is normally included in the same package).".to_string()
        } else {
            format!("Could not inspect the source video with FFprobe: {error}")
        }
    })?;

    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("FFprobe returned invalid JSON: {error}"))?;
    let streams = json.get("streams").and_then(Value::as_array)
        .ok_or_else(|| "FFprobe did not return any media streams.".to_string())?;
    let video = streams.iter().find(|stream| value_string(stream, "codec_type") == Some("video"))
        .ok_or_else(|| "No video stream was found in the selected file.".to_string())?;
    let pixel_format = value_string(video, "pix_fmt").unwrap_or("unknown").to_owned();
    let width = video.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
    let height = video.get("height").and_then(Value::as_u64).unwrap_or(0) as u32;
    if width < 256 || height < 120 {
        return Err(format!("DNxHR requires video dimensions of at least 256×120 pixels. The selected file is {width}×{height}."));
    }
    let field_order = value_string(video, "field_order").unwrap_or("unknown");
    let is_interlaced = matches!(field_order, "tt" | "bb" | "tb" | "bt");
    let raw_depth = video.get("bits_per_raw_sample")
        .and_then(|value| value.as_str().and_then(|s| s.parse::<u8>().ok()).or_else(|| value.as_u64().map(|v| v as u8)))
        .unwrap_or(0);
    let bit_depth = raw_depth.max(inferred_pixel_depth(&pixel_format));
    let is_rgb = is_rgb_pixel_format(&pixel_format);
    let is_444 = pixel_format.contains("444") || is_rgb;
    let has_alpha = pixel_format_has_alpha(&pixel_format);
    Ok(VideoProbe {
        is_interlaced,
        bit_depth,
        is_444,
        is_rgb,
        has_alpha,
        has_audio: streams.iter().any(|stream| value_string(stream, "codec_type") == Some("audio")),
        has_subtitles: streams.iter().any(|stream| value_string(stream, "codec_type") == Some("subtitle")),
        additional_video_streams: streams.iter()
            .filter(|stream| value_string(stream, "codec_type") == Some("video"))
            .count().saturating_sub(1),
        has_data_streams: streams.iter().any(|stream| value_string(stream, "codec_type") == Some("data")),
        has_attachments: streams.iter().any(|stream| value_string(stream, "codec_type") == Some("attachment")),
        color_range: parse_optional_string(video, "color_range"),
        color_space: parse_optional_string(video, "color_space"),
        color_transfer: parse_optional_string(video, "color_transfer"),
        color_primaries: parse_optional_string(video, "color_primaries"),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResolveEncoding {
    profile: &'static str,
    pixel_format: &'static str,
    label: &'static str,
}

fn select_resolve_encoding(profile: ResolveProfile, source: &VideoProbe) -> ResolveEncoding {
    let selected = match profile {
        ResolveProfile::Auto if source.is_444 => ResolveProfile::Dnxhr444,
        ResolveProfile::Auto if source.bit_depth > 8 => ResolveProfile::Hqx,
        ResolveProfile::Auto => ResolveProfile::Hq,
        explicit => explicit,
    };
    match selected {
        ResolveProfile::Auto | ResolveProfile::Hq => ResolveEncoding { profile: "dnxhr_hq", pixel_format: "yuv422p", label: "DNxHR HQ" },
        ResolveProfile::Hqx => ResolveEncoding { profile: "dnxhr_hqx", pixel_format: "yuv422p10le", label: "DNxHR HQX" },
        ResolveProfile::Sq => ResolveEncoding { profile: "dnxhr_sq", pixel_format: "yuv422p", label: "DNxHR SQ" },
        ResolveProfile::Lb => ResolveEncoding { profile: "dnxhr_lb", pixel_format: "yuv422p", label: "DNxHR LB" },
        ResolveProfile::Dnxhr444 => ResolveEncoding { profile: "dnxhr_444", pixel_format: if source.is_rgb { "gbrp10le" } else { "yuv444p10le" }, label: "DNxHR 444" },
    }
}

fn video_sync_args_from_help(help: &str) -> Result<Vec<&'static str>, String> {
    let has_option = |name: &str| {
        help.lines().any(|line| {
            line.trim_start()
                .split_whitespace()
                .next()
                .map(|option| option.starts_with(name))
                .unwrap_or(false)
        })
    };

    if has_option("-fps_mode") {
        Ok(vec!["-fps_mode", "passthrough"])
    } else if has_option("-vsync") {
        Ok(vec!["-vsync", "0"])
    } else {
        Err("This FFmpeg build exposes neither -fps_mode nor the legacy -vsync option. Update FFmpeg and try again.".into())
    }
}

fn ffmpeg_video_sync_args() -> Result<Vec<std::ffi::OsString>, String> {
    static CACHED_ARGS: OnceLock<Vec<std::ffi::OsString>> = OnceLock::new();
    if let Some(args) = CACHED_ARGS.get() {
        return Ok(args.clone());
    }

    // Cache only success: FFmpeg may be installed or repaired while ForgeBox is open.
    let output = run_ok_with_timeout(
        "ffmpeg",
        vec![os("-hide_banner"), os("-h"), os("full")],
        MEDIA_PROBE_TIMEOUT,
    ).map_err(|error| format!("Could not inspect FFmpeg options: {error}"))?;
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let options = video_sync_args_from_help(&help)?
        .into_iter()
        .map(os)
        .collect::<Vec<_>>();
    let _ = CACHED_ARGS.set(options.clone());
    Ok(CACHED_ARGS.get().cloned().unwrap_or(options))
}

fn ffmpeg_color_space_tag(source: &VideoProbe) -> Option<&str> {
    // FFprobe reports RGB's matrix coefficients as "gbr", but FFmpeg's
    // -colorspace option accepts YUV matrix names and rejects "gbr".
    source.color_space.as_deref().filter(|value| *value != "gbr")
}

fn copy_color_metadata(args: &mut Vec<std::ffi::OsString>, source: &VideoProbe) {
    for (option, value) in [
        ("-color_range", source.color_range.as_deref()),
        ("-colorspace", ffmpeg_color_space_tag(source)),
        ("-color_trc", source.color_transfer.as_deref()),
        ("-color_primaries", source.color_primaries.as_deref()),
    ] {
        if let Some(value) = value {
            args.push(os(option));
            args.push(os(value));
        }
    }
}

fn verify_resolve_output(output: &Path, source_had_audio: bool, expected_profile: &str, expected_pixel_format: &str) -> Result<(), String> {
    let result = run_ffprobe(vec![
        os("-v"), os("error"),
        os("-show_entries"), os("stream=codec_type,codec_name,profile,pix_fmt,sample_rate,channels"),
        os("-of"), os("json"),
        os(output),
    ]).map_err(|error| format!("The MOV was encoded, but FFprobe could not verify it: {error}"))?;
    let json: Value = serde_json::from_slice(&result.stdout)
        .map_err(|error| format!("The MOV was encoded, but FFprobe returned invalid output metadata: {error}"))?;
    let streams = json.get("streams").and_then(Value::as_array)
        .ok_or_else(|| "The MOV was encoded, but it contains no readable streams.".to_string())?;
    let video = streams.iter().find(|stream| value_string(stream, "codec_type") == Some("video"))
        .ok_or_else(|| "The MOV was encoded, but its video stream is missing.".to_string())?;
    if value_string(video, "codec_name") != Some("dnxhd") {
        return Err("The output file does not contain the expected DNxHR video stream.".into());
    }
    if !value_string(video, "profile").map(|actual| actual.eq_ignore_ascii_case(expected_profile)).unwrap_or(false) {
        return Err(format!("The output DNxHR profile could not be verified (expected {expected_profile})."));
    }
    if value_string(video, "pix_fmt") != Some(expected_pixel_format) {
        return Err(format!("The output pixel format could not be verified (expected {expected_pixel_format})."));
    }
    let audio = streams.iter().find(|stream| value_string(stream, "codec_type") == Some("audio"));
    if source_had_audio {
        let audio = audio.ok_or_else(|| "The source had audio, but the exported MOV does not contain an audio stream.".to_string())?;
        if value_string(audio, "codec_name") != Some("pcm_s24le") || value_string(audio, "sample_rate") != Some("48000") {
            return Err("The output audio does not match the expected 24-bit Linear PCM at 48 kHz.".into());
        }
    }
    Ok(())
}

pub(super) fn convert_davinci_resolve(input: &Path, output: &Path, requested_profile: ResolveProfile) -> Result<(), String> {
    let source = probe_video(input)?;
    let encoding = select_resolve_encoding(requested_profile, &source);

    if source.is_interlaced {
        record_execution_warning("Interlaced footage was detected. ForgeBox will deinterlace it with BWDIF and output progressive frames at the source frame rate; field-rate motion detail is not retained.".into());
    }
    if source.has_subtitles {
        record_execution_warning("Subtitle tracks were found in the source but are not embedded in the DNxHR MOV. Keep the original file if you need them.".into());
    }
    if source.additional_video_streams > 0 {
        record_execution_warning(format!(
            "The source contains {} additional video stream(s). Only the first video stream is exported; alternate angles and embedded previews are not included.",
            source.additional_video_streams
        ));
    }
    if source.has_data_streams {
        record_execution_warning("The source contains data streams (for example, timed metadata or timecode tracks). These are not copied to the DNxHR MOV.".into());
    }
    if source.has_attachments {
        record_execution_warning("The source contains embedded attachments. They are not copied to the DNxHR MOV.".into());
    }
    if source.has_alpha {
        record_execution_warning("The source contains an alpha channel. The selected DNxHR output is opaque and will not preserve transparency.".into());
    }
    if source.is_444 && encoding.profile != "dnxhr_444" {
        record_execution_warning("The source uses 4:4:4/RGB chroma. The selected profile converts it to 4:2:2; choose DNxHR 444 to retain 4:4:4 sampling.".into());
    }
    let output_bit_depth = if encoding.pixel_format.contains("10le") { 10 } else { 8 };
    if source.bit_depth > output_bit_depth {
        record_execution_warning(format!(
            "The source appears to be {}-bit, but {} stores {}-bit data. Some source precision will be reduced.",
            source.bit_depth, encoding.label, output_bit_depth
        ));
    }
    if source.color_transfer.as_deref().map(|value| value == "smpte2084" || value == "arib-std-b67").unwrap_or(false) {
        record_execution_warning("HDR transfer metadata was detected and will be copied. Check the Resolve project's color-management settings before grading.".into());
    }

    report_status(format!("Encoding {} as {}…", input.file_name().and_then(|name| name.to_str()).unwrap_or("video"), encoding.label));
    let mut args = vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"),
        os("-i"), os(input),
        os("-map"), os("0:v:0"),
        os("-map"), os("0:a?"),
        // Copy only the explicit color tags below, not arbitrary source metadata.
        os("-map_metadata"), os("-1"),
        os("-map_chapters"), os("-1"),
        os("-c:v"), os("dnxhd"),
        os("-profile:v"), os(encoding.profile),
        os("-pix_fmt"), os(encoding.pixel_format),
        os("-threads"), os("2"),
        os("-c:a"), os("pcm_s24le"),
        os("-ar"), os("48000"),
        os("-movflags"), os("+faststart"),
    ];
    // FFmpeg 7+ may reject -vsync; use -fps_mode when advertised, with a
    // legacy -vsync fallback for older builds such as FFmpeg 4.4.
    args.extend(ffmpeg_video_sync_args()?);
    if source.is_interlaced {
        args.extend([os("-vf"), os("bwdif=mode=send_frame:parity=auto:deint=all")]);
    }
    copy_color_metadata(&mut args, &source);
    args.push(os(output));

    run_ok("ffmpeg", args).map_err(|error| {
        if error.contains("Unknown encoder") || error.contains("Encoder not found") {
            "This FFmpeg build does not include the DNxHR encoder. Install a full FFmpeg build with the dnxhd encoder enabled.".to_string()
        } else if error.contains("not supported") && error.contains("pixel format") {
            format!("The installed FFmpeg build cannot encode the required pixel format for {}. Try Automatic or DNxHR HQ.", encoding.label)
        } else {
            format!("DNxHR encoding failed: {error}")
        }
    })?;

    if let Err(error) = verify_resolve_output(output, source.has_audio, encoding.label, encoding.pixel_format) {
        record_execution_warning(format!(
            "The DNxHR MOV was encoded but could not be fully verified: {error}. The output was retained for inspection."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod sticker_orientation_tests {
    use super::display_oriented_dimensions;

    #[test]
    fn portrait_display_matrix_swaps_encoded_dimensions() {
        assert_eq!(display_oriented_dimensions(1920, 1080, Some(90.0)), (1080, 1920));
        assert_eq!(display_oriented_dimensions(1920, 1080, Some(-90.0)), (1080, 1920));
        assert_eq!(display_oriented_dimensions(1920, 1080, Some(180.0)), (1920, 1080));
        assert_eq!(display_oriented_dimensions(1920, 1080, None), (1920, 1080));
    }
}

#[cfg(test)]
mod davinci_tests {
    use super::{ffmpeg_color_space_tag, inferred_pixel_depth, is_rgb_pixel_format, pixel_format_has_alpha, select_resolve_encoding, video_sync_args_from_help, ResolveProfile, VideoProbe};

    fn source(pixel_format: &str, bit_depth: u8) -> VideoProbe {
        VideoProbe {
            is_interlaced: false,
            bit_depth,
            is_444: pixel_format.contains("444") || is_rgb_pixel_format(pixel_format),
            is_rgb: is_rgb_pixel_format(pixel_format),
            has_alpha: pixel_format_has_alpha(pixel_format),
            has_audio: true,
            has_subtitles: false,
            additional_video_streams: 0,
            has_data_streams: false,
            has_attachments: false,
            color_range: None,
            color_space: None,
            color_transfer: None,
            color_primaries: None,
        }
    }

    #[test]
    fn pixel_depth_ignores_chroma_subsampling_digits() {
        assert_eq!(inferred_pixel_depth("nv12"), 8);
        assert_eq!(inferred_pixel_depth("yuv410p"), 8);
        assert_eq!(inferred_pixel_depth("yuv420p"), 8);
    }

    #[test]
    fn pixel_depth_detects_component_depth_and_packed_rgb() {
        assert_eq!(inferred_pixel_depth("yuv420p10le"), 10);
        assert_eq!(inferred_pixel_depth("yuva444p12be"), 12);
        assert_eq!(inferred_pixel_depth("gbrp16le"), 16);
        assert_eq!(inferred_pixel_depth("rgb48le"), 16);
        assert_eq!(inferred_pixel_depth("rgba64le"), 16);
    }
    #[test]
    fn modern_ffmpeg_uses_fps_mode_passthrough() {
        let help = "  -fps_mode[:stream_specifier]  set framerate mode\n  -vsync parameter  legacy sync mode";
        assert_eq!(video_sync_args_from_help(help).unwrap(), vec!["-fps_mode", "passthrough"]);
    }

    #[test]
    fn legacy_ffmpeg_falls_back_to_numeric_vsync() {
        let help = "  -vsync parameter  video sync method";
        assert_eq!(video_sync_args_from_help(help).unwrap(), vec!["-vsync", "0"]);
    }

    #[test]
    fn ffmpeg_without_a_frame_sync_option_returns_a_clear_error() {
        assert!(video_sync_args_from_help("  -threads number  set thread count").unwrap_err().contains("neither"));
    }

    #[test]
    fn rgb_gbr_matrix_tag_is_not_passed_to_ffmpeg_colorspace_option() {
        let mut rgb = source("gbrp10le", 10);
        rgb.color_space = Some("gbr".into());
        assert_eq!(ffmpeg_color_space_tag(&rgb), None);

        let mut yuv = source("yuv422p10le", 10);
        yuv.color_space = Some("bt709".into());
        assert_eq!(ffmpeg_color_space_tag(&yuv), Some("bt709"));
    }

    #[test]
    fn automatic_profile_selects_hq_for_eight_bit_video() {
        let selected = select_resolve_encoding(ResolveProfile::Auto, &source("yuv420p", 8));
        assert_eq!(selected.profile, "dnxhr_hq");
        assert_eq!(selected.pixel_format, "yuv422p");
    }

    #[test]
    fn automatic_profile_selects_hqx_for_ten_bit_video() {
        let selected = select_resolve_encoding(ResolveProfile::Auto, &source("yuv420p10le", 10));
        assert_eq!(selected.profile, "dnxhr_hqx");
        assert_eq!(selected.pixel_format, "yuv422p10le");
    }

    #[test]
    fn automatic_profile_selects_444_for_rgb_and_four_four_four_sources() {
        let selected = select_resolve_encoding(ResolveProfile::Auto, &source("yuv444p10le", 10));
        assert_eq!(selected.profile, "dnxhr_444");
        assert_eq!(selected.pixel_format, "yuv444p10le");
        let rgb = select_resolve_encoding(ResolveProfile::Auto, &source("gbrp", 8));
        assert_eq!(rgb.profile, "dnxhr_444");
        assert_eq!(rgb.pixel_format, "gbrp10le");
        let bgr = select_resolve_encoding(ResolveProfile::Auto, &source("bgra", 8));
        assert_eq!(bgr.profile, "dnxhr_444");
        assert_eq!(bgr.pixel_format, "gbrp10le");
        assert!(source("bgra", 8).has_alpha);
        assert!(source("yuva420p", 8).has_alpha);
    }
}


#[cfg(test)]
mod hdr_transfer_tests {
    use super::is_hdr_transfer;

    #[test]
    fn recognizes_hdr_transfer_functions_only() {
        assert!(is_hdr_transfer(Some("smpte2084")));
        assert!(is_hdr_transfer(Some("arib-std-b67")));
        assert!(!is_hdr_transfer(Some("bt709")));
        assert!(!is_hdr_transfer(Some("unknown")));
        assert!(!is_hdr_transfer(None));
    }
}
