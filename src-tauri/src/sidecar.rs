use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use crate::mouse_tracking::MouseEvent;

// Sidecar v1.1 schema matching the spec
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sidecar {
    pub v: u32,
    pub media: MediaInfo,
    pub trim: TrimRange,
    #[serde(default)]
    pub cuts: Vec<CutRange>,
    #[serde(default)]
    pub zoom: Vec<ZoomKeyframe>,
    #[serde(default)]
    pub webcam: Vec<WebcamKeyframe>,
    #[serde(default)]
    pub overlays: Vec<Overlay>,
    #[serde(default)]
    pub mouse_events: Vec<MouseEvent>,
    pub audio: AudioSettings,
    pub preview: PreviewSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaInfo {
    pub path: String,
    pub fps: u32,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrimRange {
    #[serde(rename = "in")]
    pub in_point: u64,
    pub out: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CutRange {
    pub start: u64,
    pub end: u64,
    pub keep: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZoomKeyframe {
    pub t: u64,
    pub cx: f32,
    pub cy: f32,
    pub scale: f32,
    pub dur: u64,
    pub ease: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebcamKeyframe {
    pub t: u64,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub shape: String,
    pub visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Overlay {
    #[serde(rename = "ring")]
    Ring {
        t0: u64,
        t1: u64,
        x: f32,
        y: f32,
        inner: f32,
        outer: f32,
        color: String,
    },
    #[serde(rename = "arrow")]
    Arrow {
        t0: u64,
        t1: u64,
        x: f32,
        y: f32,
        x2: f32,
        y2: f32,
        color: String,
        stroke_width: f32,
    },
    #[serde(rename = "label")]
    Label {
        t0: u64,
        t1: u64,
        x: f32,
        y: f32,
        text: String,
        style: String,
    },
    #[serde(rename = "box")]
    Box {
        t0: u64,
        t1: u64,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: String,
        stroke_width: f32,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioSettings {
    pub dual_track: bool,
    pub mic_gain_db: f32,
    pub sys_gain_db: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ducking: Option<DuckingSettings>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GateSettings {
    pub enabled: bool,
    pub threshold_db: f32,
    pub release_ms: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DuckingSettings {
    pub enabled: bool,
    pub amount_db: f32,
    pub attack_ms: u32,
    pub release_ms: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreviewSettings {
    pub resolution_scale: f32,
}

impl Sidecar {
    /// Create a new sidecar with default values for a recording session
    pub fn new_for_recording(
        media_path: impl AsRef<Path>,
        fps: u32,
        duration_ms: u64,
        width: u32,
        height: u32,
    ) -> Self {
        let path = media_path.as_ref();
        let path_str = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("recording.mp4")
            .to_string();

        Self {
            v: 110,
            media: MediaInfo {
                path: path_str,
                fps,
                duration_ms,
                width,
                height,
            },
            trim: TrimRange {
                in_point: 0,
                out: duration_ms,
            },
            cuts: Vec::new(),
            zoom: Vec::new(),
            webcam: vec![
                // Default webcam position if camera was enabled
                WebcamKeyframe {
                    t: 0,
                    x: 0.85,
                    y: 0.15,
                    size: 0.12,
                    shape: "circle".to_string(),
                    visible: false,
                    corner_radius: None,
                    opacity: None,
                }
            ],
            overlays: Vec::new(),
            mouse_events: Vec::new(),
            audio: AudioSettings {
                dual_track: true,
                mic_gain_db: 0.0,
                sys_gain_db: 0.0,
                gate: None,
                ducking: None,
            },
            preview: PreviewSettings {
                resolution_scale: 1.0,
            },
        }
    }

    /// Save sidecar to disk
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Load sidecar from disk
    #[allow(dead_code)]
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let json = fs::read_to_string(path)?;
        let sidecar = serde_json::from_str(&json)?;
        Ok(sidecar)
    }

    /// Generate sidecar path from media path
    #[allow(dead_code)]
    pub fn sidecar_path_for_media(media_path: impl AsRef<Path>) -> PathBuf {
        let path = media_path.as_ref();
        let stem = path.file_stem().unwrap_or_default();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        
        parent.join(format!("{}.sidecar.json", stem.to_string_lossy()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidecar_serialization() {
        let sidecar = Sidecar::new_for_recording(
            "/tmp/test.mp4",
            60,
            120000,
            1920,
            1080,
        );

        let json = serde_json::to_string_pretty(&sidecar).unwrap();
        let parsed: Sidecar = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.v, 110);
        assert_eq!(parsed.media.fps, 60);
        assert_eq!(parsed.media.width, 1920);
        assert_eq!(parsed.media.height, 1080);
    }
}