//! Runtime video acceleration and FFmpeg codec configuration.

#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};

use super::types::ExportSettings;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExportAcceleration {
    VideoToolbox,
    Vaapi { device: PathBuf },
    Nvidia,
    AmdAmf,
    Software,
}

impl ExportAcceleration {
    pub fn label(&self) -> &'static str {
        match self {
            Self::VideoToolbox => "VideoToolbox",
            Self::Vaapi { .. } => "VA-API",
            Self::Nvidia => "NVENC/CUDA",
            Self::AmdAmf => "AMD AMF",
            Self::Software => "software",
        }
    }

    pub fn append_decoder_args(&self, args: &mut Vec<String>) {
        match self {
            Self::VideoToolbox => args.extend(["-hwaccel".into(), "videotoolbox".into()]),
            Self::Vaapi { device } => {
                args.extend(["-hwaccel".into(), "vaapi".into()]);
                args.extend([
                    "-hwaccel_device".into(),
                    device.to_string_lossy().into_owned(),
                ]);
            }
            Self::Nvidia => args.extend(["-hwaccel".into(), "cuda".into()]),
            Self::AmdAmf => args.extend(["-hwaccel".into(), "auto".into()]),
            Self::Software => {}
        }
    }

    pub fn append_encoder_input_args(&self, args: &mut Vec<String>) {
        if let Self::Vaapi { device } = self {
            args.extend([
                "-vaapi_device".into(),
                device.to_string_lossy().into_owned(),
            ]);
        }
    }

    pub fn encoder_filter(&self, target_fps: u32) -> String {
        match self {
            Self::Vaapi { .. } => format!("fps={target_fps},format=nv12,hwupload"),
            _ => format!("fps={target_fps},format=yuv420p"),
        }
    }
}

pub fn detect_export_acceleration(settings: &ExportSettings) -> ExportAcceleration {
    #[cfg(target_os = "macos")]
    {
        let _ = settings;
        return ExportAcceleration::VideoToolbox;
    }

    #[cfg(target_os = "linux")]
    {
        return detect_linux_acceleration(settings);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = settings;
        ExportAcceleration::Software
    }
}

#[cfg(target_os = "linux")]
fn detect_linux_acceleration(settings: &ExportSettings) -> ExportAcceleration {
    if settings.format.as_deref().unwrap_or("mp4") != "mp4" {
        return ExportAcceleration::Software;
    }

    let codec = settings.codec.as_deref().unwrap_or("h264");
    let encoders = ffmpeg_encoder_list();
    let requested = std::env::var("TARANTINO_VIDEO_BACKEND")
        .unwrap_or_else(|_| "auto".into())
        .to_ascii_lowercase();
    select_linux_acceleration(
        &requested,
        codec,
        &encoders,
        find_vaapi_device(),
        Path::new("/dev/nvidia0").exists(),
        |candidate| verify_linux_encoder(candidate, codec),
    )
}

#[cfg(target_os = "linux")]
fn select_linux_acceleration(
    requested: &str,
    codec: &str,
    encoders: &str,
    va_device: Option<PathBuf>,
    nvidia_device: bool,
    mut verify: impl FnMut(&ExportAcceleration) -> bool,
) -> ExportAcceleration {
    let has = |name: &str| encoders.split_whitespace().any(|item| item == name);
    let nvenc = if codec == "h265" {
        "hevc_nvenc"
    } else {
        "h264_nvenc"
    };
    let vaapi = if codec == "h265" {
        "hevc_vaapi"
    } else {
        "h264_vaapi"
    };
    let amf = if codec == "h265" {
        "hevc_amf"
    } else {
        "h264_amf"
    };

    let mut choose = |backend: &str| {
        match backend {
            "vaapi" if va_device.is_some() && has(vaapi) => Some(ExportAcceleration::Vaapi {
                device: va_device.clone().unwrap(),
            }),
            "nvenc" | "nvidia" if nvidia_device && has(nvenc) => Some(ExportAcceleration::Nvidia),
            "amf" if va_device.is_some() && has(amf) => Some(ExportAcceleration::AmdAmf),
            "software" => Some(ExportAcceleration::Software),
            _ => None,
        }
        .filter(|candidate| verify(candidate))
    };

    if requested != "auto" {
        if let Some(backend) = choose(&requested) {
            return backend;
        }
        eprintln!(
            "Requested Linux video backend '{requested}' is unavailable; selecting automatically"
        );
    }
    for backend in ["nvenc", "vaapi", "amf"] {
        if let Some(selected) = choose(backend) {
            return selected;
        }
    }
    ExportAcceleration::Software
}

#[cfg(target_os = "linux")]
fn verify_linux_encoder(acceleration: &ExportAcceleration, codec: &str) -> bool {
    if matches!(acceleration, ExportAcceleration::Software) {
        return true;
    }

    let mut args = vec!["-v".to_string(), "error".to_string()];
    acceleration.append_encoder_input_args(&mut args);
    args.extend([
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        "color=size=64x64:rate=1".to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
    ]);
    let settings: ExportSettings = serde_json::from_value(serde_json::json!({
        "format": "mp4", "codec": codec, "quality": "low"
    }))
    .expect("minimal export settings are valid");
    args.extend(build_codec_args(&settings, acceleration));
    args.extend([
        "-vf".to_string(),
        acceleration.encoder_filter(1),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ]);

    Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_os = "linux")]
fn ffmpeg_encoder_list() -> String {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn find_vaapi_device() -> Option<PathBuf> {
    if let Some(device) = std::env::var_os("TARANTINO_VAAPI_DEVICE") {
        let path = PathBuf::from(device);
        if path.exists() {
            return Some(path);
        }
    }
    let mut devices = std::fs::read_dir("/dev/dri")
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        })
        .collect::<Vec<_>>();
    devices.sort();
    devices.into_iter().next()
}

pub fn add_trim_settings(args: &mut Vec<String>, settings: &ExportSettings) {
    if let Some(trim_start) = settings.trim_start {
        args.extend(["-ss".into(), format!("{:.3}", trim_start as f64 / 1000.0)]);
    }
    if let (Some(trim_start), Some(trim_end)) = (settings.trim_start, settings.trim_end) {
        let duration = (trim_end - trim_start) as f64 / 1000.0;
        if duration > 0.0 {
            args.extend(["-t".into(), format!("{duration:.3}")]);
        }
    }
}

pub fn build_codec_args(
    settings: &ExportSettings,
    acceleration: &ExportAcceleration,
) -> Vec<String> {
    let format = settings.format.as_deref().unwrap_or("mp4");
    let quality = settings.quality.as_deref().unwrap_or("high");
    let codec = settings.codec.as_deref().unwrap_or("h264");
    let mut args = Vec::new();

    match format {
        "mp4" => append_mp4_codec_args(&mut args, acceleration, codec, quality),
        "mov" => {
            if matches!(acceleration, ExportAcceleration::VideoToolbox) {
                args.extend(["-c:v".into(), "prores_videotoolbox".into()]);
            } else {
                args.extend(["-c:v".into(), "prores_ks".into()]);
            }
            let profile = match quality {
                "low" => "1",
                "medium" => "2",
                _ => "3",
            };
            args.extend(["-profile:v".into(), profile.into()]);
        }
        "webm" => {
            args.extend(["-c:v".into(), "libvpx-vp9".into()]);
            let crf = match quality {
                "low" => "40",
                "medium" => "32",
                _ => "24",
            };
            args.extend(["-crf".into(), crf.into(), "-b:v".into(), "0".into()]);
            args.extend([
                "-row-mt".into(),
                "1".into(),
                "-tile-columns".into(),
                "2".into(),
            ]);
            let speed = match quality {
                "low" => "4",
                "medium" => "2",
                _ => "1",
            };
            args.extend(["-speed".into(), speed.into()]);
        }
        _ => append_mp4_codec_args(&mut args, acceleration, "h264", "medium"),
    }
    args
}

fn append_mp4_codec_args(
    args: &mut Vec<String>,
    acceleration: &ExportAcceleration,
    codec: &str,
    quality: &str,
) {
    let h265 = codec == "h265";
    let bitrate = match quality {
        "low" => "2M",
        "medium" => "5M",
        _ => "10M",
    };
    match acceleration {
        ExportAcceleration::VideoToolbox => {
            args.extend([
                "-c:v".into(),
                if h265 {
                    "hevc_videotoolbox"
                } else {
                    "h264_videotoolbox"
                }
                .into(),
            ]);
            let quality_value = match quality {
                "low" => "40",
                "medium" => "60",
                _ => "80",
            };
            args.extend([
                "-q:v".into(),
                quality_value.into(),
                "-b:v".into(),
                bitrate.into(),
            ]);
            args.extend(["-bf".into(), "3".into()]);
        }
        ExportAcceleration::Vaapi { .. } => args.extend([
            "-c:v".into(),
            if h265 { "hevc_vaapi" } else { "h264_vaapi" }.into(),
            "-b:v".into(),
            bitrate.into(),
        ]),
        ExportAcceleration::Nvidia => args.extend([
            "-c:v".into(),
            if h265 { "hevc_nvenc" } else { "h264_nvenc" }.into(),
            "-preset".into(),
            "p5".into(),
            "-b:v".into(),
            bitrate.into(),
        ]),
        ExportAcceleration::AmdAmf => args.extend([
            "-c:v".into(),
            if h265 { "hevc_amf" } else { "h264_amf" }.into(),
            "-quality".into(),
            if quality == "low" {
                "speed"
            } else {
                "balanced"
            }
            .into(),
            "-b:v".into(),
            bitrate.into(),
        ]),
        ExportAcceleration::Software => {
            args.extend([
                "-c:v".into(),
                if h265 { "libx265" } else { "libx264" }.into(),
            ]);
            let (preset, crf) = match quality {
                "low" => ("faster", "28"),
                "medium" => ("medium", "23"),
                _ => ("slow", "18"),
            };
            args.extend(["-preset".into(), preset.into(), "-crf".into(), crf.into()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vaapi_uses_gpu_upload_filter_and_hardware_codec() {
        let acceleration = ExportAcceleration::Vaapi {
            device: PathBuf::from("/dev/dri/renderD128"),
        };
        assert_eq!(
            acceleration.encoder_filter(60),
            "fps=60,format=nv12,hwupload"
        );
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "format": "mp4", "codec": "h264", "quality": "high"
        }))
        .unwrap();
        let args = build_codec_args(&settings, &acceleration);
        assert!(args.iter().any(|arg| arg == "h264_vaapi"));
        assert!(!args.iter().any(|arg| arg == "libx264"));
    }

    #[test]
    fn software_backend_remains_available() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "format": "mp4", "codec": "h265", "quality": "medium"
        }))
        .unwrap();
        let args = build_codec_args(&settings, &ExportAcceleration::Software);
        assert!(args.iter().any(|arg| arg == "libx265"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_prefers_discrete_nvidia_then_vaapi() {
        let encoders = "h264_nvenc h264_vaapi h264_amf";
        let selected = select_linux_acceleration(
            "auto",
            "h264",
            encoders,
            Some(PathBuf::from("/dev/dri/renderD128")),
            true,
            |_| true,
        );
        assert_eq!(selected, ExportAcceleration::Nvidia);

        let selected = select_linux_acceleration(
            "auto",
            "h264",
            encoders,
            Some(PathBuf::from("/dev/dri/renderD128")),
            false,
            |_| true,
        );
        assert!(matches!(selected, ExportAcceleration::Vaapi { .. }));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_rejects_a_reported_backend_when_probe_fails() {
        let selected = select_linux_acceleration(
            "auto",
            "h264",
            "h264_vaapi h264_amf",
            Some(PathBuf::from("/dev/dri/renderD128")),
            false,
            |candidate| !matches!(candidate, ExportAcceleration::Vaapi { .. }),
        );
        assert_eq!(selected, ExportAcceleration::AmdAmf);
    }
}
