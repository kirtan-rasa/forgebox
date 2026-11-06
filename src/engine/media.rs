use super::{normalized_ext, os, positive, run_ok};
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
