//! Platform-neutral capture contracts.

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Raw video frame produced by a native capture backend.
#[derive(Clone, Debug)]
pub struct CapturedFrame {
    pub data: Bytes,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub timestamp_us: u64,
    pub stride: u32,
}

/// Raw audio packet produced by a native capture backend.
#[derive(Clone, Debug)]
pub struct CapturedAudio {
    pub data: Bytes,
    pub sample_rate: u32,
    pub channels: u32,
    pub _timestamp_us: u64,
}

/// A display or window that can be selected for capture.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureSourceInfo {
    pub id: u64,
    pub name: String,
    pub source_type: CaptureSourceType,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale_factor: f64,
    pub is_primary: bool,
    pub owner_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureSourceType {
    Display,
    Window,
}

/// Settings shared by all native capture backends.
#[derive(Clone, Debug)]
pub struct CaptureConfig {
    pub source_id: u64,
    pub source_type: CaptureSourceType,
    pub fps: u32,
    pub include_cursor: bool,
    pub include_audio: bool,
    pub region: Option<CaptureRegion>,
    pub output_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub screen_recording: bool,
    pub microphone: bool,
    pub camera: bool,
}
