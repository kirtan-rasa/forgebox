use super::{normalized_ext, os, positive, record_execution_warning, report_status, run_ok, ResolveProfile};
use serde_json::Value;
use std::path::Path;

pub(super) fn convert_video(input: &Path, output: &Path, target_format: &str, preset: &str, quality: &str) -> Result<(), String> {
    let format = normalized_ext(target_format);
    let crf = positive(quality, 23).clamp(18, 30).to_string();
    let (preset_label, x264_preset, vp9_cpu_used) = match preset {
        "balanced" => ("balanced", "medium", "4"),
        "smaller" => ("smaller", "slow", "2"),
        _ => ("fast", "veryfast", "6"),
    };
    let mut args = vec![
        os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"),
        os("-i"), os(input), os("-map"), os("0:v:0"), os("-map"), os("0:a?"),
        os("-map_metadata"), os("0"), os("-threads"), os("2"),
    ];
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
            args.extend([
                os("-c:v"), os("libvpx-vp9"), os("-deadline"), os("good"),
                os("-cpu-used"), os(vp9_cpu_used), os("-crf"), os(&vp9_crf), os("-b:v"), os("0"),
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

pub(super) fn extract_audio(input: &Path, output: &Path, target_format: &str) -> Result<(), String> {
    let format = normalized_ext(target_format);
    let mut args = vec![os("-y"), os("-hide_banner"), os("-loglevel"), os("error"), os("-nostdin"), os("-i"), os(input), os("-map"), os("0:a:0"), os("-vn")];
    match format.as_str() {
        "mp3" => args.extend([os("-c:a"), os("libmp3lame"), os("-b:a"), os("192k")]),
        "wav" => args.extend([os("-c:a"), os("pcm_s16le")]),
        "aac" => args.extend([os("-c:a"), os("aac"), os("-b:a"), os("192k")]),
        "ogg" => args.extend([os("-c:a"), os("libvorbis"), os("-q:a"), os("5")]),
        "flac" => args.extend([os("-c:a"), os("flac")]),
        "m4a" => args.extend([os("-c:a"), os("aac"), os("-b:a"), os("192k")]),
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
    for depth in [16_u8, 14, 12, 10, 9] {
        if pixel_format.contains(&depth.to_string()) {
            return depth;
        }
    }
    if pixel_format.contains("p010") { 10 } else { 8 }
}

fn probe_video(input: &Path) -> Result<VideoProbe, String> {
    report_status(format!("Analyzing {} with FFprobe…", input.file_name().and_then(|name| name.to_str()).unwrap_or("video")));
    let output = run_ok("ffprobe", vec![
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
    let result = run_ok("ffprobe", vec![
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
        os("-map_metadata"), os("0"),
        os("-map_chapters"), os("0"),
        // -vsync 0 is supported by FFmpeg 4.4 and newer; it passes source frame timestamps through.
        os("-vsync"), os("0"),
        os("-c:v"), os("dnxhd"),
        os("-profile:v"), os(encoding.profile),
        os("-pix_fmt"), os(encoding.pixel_format),
        os("-threads"), os("2"),
        os("-c:a"), os("pcm_s24le"),
        os("-ar"), os("48000"),
        os("-movflags"), os("+faststart"),
    ];
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

    verify_resolve_output(output, source.has_audio, encoding.label, encoding.pixel_format)?;
    Ok(())
}

#[cfg(test)]
mod davinci_tests {
    use super::{ffmpeg_color_space_tag, is_rgb_pixel_format, pixel_format_has_alpha, select_resolve_encoding, ResolveProfile, VideoProbe};

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
