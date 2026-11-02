//! Format-aware encoding of composited RGBA frames.
use super::super::{
    audio_export::{AudioTrackFiles, append_audio_encode_args, append_audio_inputs},
    codec_config::{ExportAcceleration, build_codec_args},
    types::ExportSettings,
};
use std::path::Path;

/// Per-frame palettes keep streaming memory bounded: a whole-video palette
/// branch would hold decoded RGBA frames until the final histogram is ready.
pub fn gif_filter(quality: Option<&str>) -> String {
    let (colors, dither) = match quality.unwrap_or("high") {
        "low" => (64, "bayer:bayer_scale=5"),
        "medium" => (128, "bayer:bayer_scale=3"),
        _ => (256, "sierra2_4a"),
    };
    format!(
        "[0:v]split[frames][colors];[colors]palettegen=stats_mode=single:max_colors={colors}:reserve_transparent=0[palette];[frames][palette]paletteuse=new=1:dither={dither}[gif]"
    )
}

pub fn encoder_args(
    settings: &ExportSettings,
    acceleration: &ExportAcceleration,
    tracks: &AudioTrackFiles,
    audio_settings: &ExportSettings,
    dimensions: (u32, u32),
    fps: u32,
    output: &Path,
) -> Vec<String> {
    let format = settings.format.as_deref().unwrap_or("mp4");
    let mut args = vec!["-y".into()];
    if format != "gif" {
        acceleration.append_encoder_input_args(&mut args);
    }
    args.extend([
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-s".into(),
        format!("{}x{}", dimensions.0, dimensions.1),
        "-r".into(),
        fps.to_string(),
        "-threads".into(),
        "0".into(),
        "-i".into(),
        "-".into(),
    ]);
    if format == "gif" {
        args.extend([
            "-filter_complex".into(),
            gif_filter(settings.quality.as_deref()),
            "-map".into(),
            "[gif]".into(),
        ]);
    } else {
        if tracks.has_any() {
            append_audio_inputs(&mut args, tracks, audio_settings);
            append_audio_encode_args(&mut args, settings, tracks);
        }
        args.extend(["-vf".into(), acceleration.encoder_filter(fps)]);
        if matches!(format, "mp4" | "mov") {
            args.extend(["-movflags".into(), "+faststart".into()]);
        }
    }
    args.extend(build_codec_args(settings, acceleration));
    args.push(output.to_string_lossy().into_owned());
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gif_uses_palettes_and_never_reads_audio_or_uploads_encoder_frames() {
        let settings: ExportSettings =
            serde_json::from_value(serde_json::json!({"format":"gif"})).unwrap();
        let tracks = AudioTrackFiles {
            background: None,
            mic: Some("unused.wav".into()),
            system: None,
        };
        let args = encoder_args(
            &settings,
            &ExportAcceleration::Vaapi {
                device: "/dev/dri/renderD128".into(),
            },
            &tracks,
            &settings,
            (64, 64),
            30,
            Path::new("out.gif"),
        );
        assert!(!args.iter().any(|a| matches!(
            a.as_str(),
            "unused.wav" | "-vaapi_device" | "-movflags" | "h264_vaapi"
        )));
        assert!(args.windows(2).any(|a| a == ["-c:v", "gif"]));
        assert!(args.windows(2).any(|a| a == ["-loop", "0"]));
        assert!(args.contains(&gif_filter(None)));
    }
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod audio_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "timing_tests.rs"]
mod timing_tests;

#[cfg(test)]
#[path = "background_audio_tests.rs"]
mod background_audio_tests;
