//! Codec configuration for FFmpeg encoding.

use super::types::ExportSettings;

/// Add trim settings to FFmpeg args
pub fn add_trim_settings(args: &mut Vec<String>, settings: &ExportSettings) {
    if let Some(trim_start) = settings.trim_start {
        let start_seconds = trim_start as f64 / 1000.0;
        args.push("-ss".to_string());
        args.push(format!("{:.3}", start_seconds));
    }

    if let (Some(trim_start), Some(trim_end)) = (settings.trim_start, settings.trim_end) {
        let duration = (trim_end - trim_start) as f64 / 1000.0;
        if duration > 0.0 {
            args.push("-t".to_string());
            args.push(format!("{:.3}", duration));
        }
    }
}

/// Build codec arguments for export
pub fn build_codec_args(settings: &ExportSettings) -> Vec<String> {
    let format = settings.format.as_deref().unwrap_or("mp4");
    let quality = settings.quality.as_deref().unwrap_or("high");
    let codec = settings.codec.as_deref().unwrap_or("h264");
    let mut codec_args = Vec::new();

    match format {
        "mp4" => {
            #[cfg(target_os = "macos")]
            {
                codec_args.push("-c:v".to_string());
                codec_args.push(if codec == "h265" { "hevc_videotoolbox" } else { "h264_videotoolbox" }.to_string());
                let (vt_quality, bitrate) = match quality {
                    "low" => ("40", "2M"),
                    "medium" => ("60", "5M"),
                    "high" | _ => ("80", "10M"),
                };
                codec_args.extend(["-q:v".to_string(), vt_quality.to_string()]);
                codec_args.extend(["-b:v".to_string(), bitrate.to_string()]);
                codec_args.extend(["-bf".to_string(), "3".to_string()]);
            }
            #[cfg(not(target_os = "macos"))]
            {
                codec_args.push("-c:v".to_string());
                codec_args.push(if codec == "h265" { "libx265" } else { "libx264" }.to_string());
                let (preset, crf) = match quality {
                    "low" => ("faster", "28"),
                    "medium" => ("medium", "23"),
                    "high" | _ => ("slow", "18"),
                };
                codec_args.extend(["-preset".to_string(), preset.to_string()]);
                codec_args.extend(["-crf".to_string(), crf.to_string()]);
            }
        }
        "mov" => {
            #[cfg(target_os = "macos")]
            codec_args.extend(["-c:v".to_string(), "prores_videotoolbox".to_string()]);
            #[cfg(not(target_os = "macos"))]
            codec_args.extend(["-c:v".to_string(), "prores_ks".to_string()]);
            let profile = match quality {
                "low" => "1",
                "medium" => "2",
                "high" | _ => "3",
            };
            codec_args.extend(["-profile:v".to_string(), profile.to_string()]);
        }
        "webm" => {
            codec_args.extend(["-c:v".to_string(), "libvpx-vp9".to_string()]);
            let crf = match quality {
                "low" => "40",
                "medium" => "32",
                "high" | _ => "24",
            };
            codec_args.extend(["-crf".to_string(), crf.to_string()]);
            codec_args.extend(["-b:v".to_string(), "0".to_string()]);
            codec_args.extend(["-row-mt".to_string(), "1".to_string()]);
            codec_args.extend(["-tile-columns".to_string(), "2".to_string()]);
            let speed = match quality {
                "low" => "4",
                "medium" => "2",
                "high" | _ => "1",
            };
            codec_args.extend(["-speed".to_string(), speed.to_string()]);
        }
        _ => {
            #[cfg(target_os = "macos")]
            {
                codec_args.extend(["-c:v".to_string(), "h264_videotoolbox".to_string()]);
                codec_args.extend(["-q:v".to_string(), "60".to_string()]);
                codec_args.extend(["-b:v".to_string(), "5M".to_string()]);
            }
            #[cfg(not(target_os = "macos"))]
            {
                codec_args.extend(["-c:v".to_string(), "libx264".to_string()]);
                codec_args.extend(["-preset".to_string(), "medium".to_string()]);
                codec_args.extend(["-crf".to_string(), "23".to_string()]);
            }
        }
    }

    codec_args
}
