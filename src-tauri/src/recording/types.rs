use serde::{Deserialize, Serialize};

/// Recording configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub target: RecordingTarget,
    pub quality: QualityPreset,
    pub output_format: OutputFormat,
    pub output_path: String,
    pub include_cursor: bool,
    pub cursor_size: f32,
    pub highlight_clicks: bool,
    pub include_microphone: bool,
    pub microphone_device: Option<String>,
    pub include_system_audio: bool,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            target: RecordingTarget::Desktop {
                display_id: 0,
                area: None
            },
            quality: QualityPreset::High,
            output_format: OutputFormat {
                container: Container::MP4,
                codec: VideoCodec::H264,
                audio_codec: Some(AudioCodec::AAC),
            },
            output_path: "/tmp/recording.mp4".to_string(),
            include_cursor: true,
            cursor_size: 1.0,
            highlight_clicks: false,
            include_microphone: false,
            microphone_device: None,
            include_system_audio: false,
        }
    }
}

/// Recording target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingTarget {
    Desktop {
        display_id: u32,
        area: Option<RecordingArea>,
    },
    Window {
        window_id: u64,
        include_shadow: bool,
    },
    Device {
        device_id: String,
        device_type: DeviceType,
    },
}

/// Recording area for partial capture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Device type for device capture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    Camera,
    IOSDevice,
    AndroidDevice,
}

/// Quality preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityPreset {
    Lossless,
    High,
    Medium,
    Low,
}

/// Output format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputFormat {
    pub container: Container,
    pub codec: VideoCodec,
    pub audio_codec: Option<AudioCodec>,
}

/// Container format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Container {
    MP4,
    MOV,
    MKV,
    WebM,
}

/// Video codec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    H265,
    VP9,
    ProRes,
}

/// Audio codec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioCodec {
    AAC,
    Opus,
    PCM,
}