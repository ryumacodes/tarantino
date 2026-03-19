//! Native screen capture for Tarantino
//!
//! # Architecture Migration (v0.2.0)
//!
//! The capture system has been redesigned to use native OS APIs instead of FFmpeg:
//!
//! ## Old Architecture (REMOVED)
//! - FFmpeg CLI-based capture via AVFoundation (macOS)
//! - `CaptureSession` trait with FFmpeg implementation
//! - High latency, process management issues
//!
//! ## New Architecture (CURRENT)
//! - **Native backends** per OS:
//!   - macOS: ScreenCaptureKit (SCK) - `src/capture/backends/macos/`
//!   - Windows: DXGI Desktop Duplication (stub) - `src/capture/backends/windows/`
//!   - Linux: PipeWire (stub) - `src/capture/backends/linux/`
//! - Low-level frame capture via `NativeCaptureBackend` trait
//! - Low latency (< 33ms), native cursor, proper permissions
//!
//! ## Migration Guide
//!
//! ### For Low-Level Frame Capture:
//! ```rust
//! // OLD (removed):
//! let session = CaptureSessionFactory::create_session()?;
//!
//! // NEW:
//! use crate::capture::backends::{CaptureBackendFactory, CaptureConfig, CaptureSourceType};
//!
//! let mut backend = CaptureBackendFactory::create_backend()?;
//! let sources = backend.enumerate_sources().await?;
//!
//! // Start capture
//! let config = CaptureConfig {
//!     source_id: sources[0].id,
//!     source_type: CaptureSourceType::Display,
//!     fps: 60,
//!     include_cursor: true,
//!     include_audio: false,
//!     region: None,
//! };
//! let handle = backend.start_capture(config).await?;
//!
//! // Receive frames
//! if let Some(mut rx) = backend.frame_receiver() {
//!     while let Ok(frame) = rx.recv().await {
//!         // Process frame...
//!     }
//! }
//! ```
//!
//! ### For High-Level Recording (Project-Based):
//! The high-level recording session manager needs to be reimplemented using the native
//! backends. The old FFmpeg-based implementation has been removed. Consider implementing
//! a new `RecordingSessionManager` that uses `NativeCaptureBackend` + encoding.

use serde::{Deserialize, Serialize};

// Native capture backends
pub mod backends;

// Re-export new backend architecture as primary API
pub use backends::{
    CaptureBackendFactory, NativeCaptureBackend, CaptureConfig, CaptureSourceType,
    CaptureRegion,
};

/// Clean recording API matching the spec
/// 
/// This replaces the complex FFmpeg-based recording system with native APIs:
/// - macOS: ScreenCaptureKit for screen/audio, AVFoundation for camera
/// - Windows: Windows Graphics Capture (WGC) for screen, Media Foundation for audio/camera  
/// - Linux: PipeWire/GStreamer for screen capture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRecordingPayload {
    pub mode: RecordingMode,
    pub target: CaptureTarget,
    #[serde(rename = "micName")]
    pub mic_name: Option<String>,
    #[serde(rename = "cameraLabel")]
    pub camera_label: Option<String>,
    #[serde(rename = "systemAudio")]
    pub system_audio: Option<bool>,
    pub fps: Option<u32>, // 30 or 60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingMode {
    /// Quick Record (single continuous take) - default mode
    #[serde(rename = "instant")]
    Quick,
    /// Studio mode (segmented, pause/resume)
    Studio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureTarget {
    #[serde(rename = "type")]
    pub target_type: CaptureTargetType,
    pub id: Option<u32>,
    pub bounds: Option<CaptureBounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureTargetType {
    Screen,
    Window, 
    Area,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureBounds {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Response when recording starts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRecordingResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "projectPath")]
    pub project_path: String,
}

/// Response when recording stops
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopRecordingResponse {
    #[serde(rename = "projectPath")]
    pub project_path: String,
    pub summary: RecordingSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSummary {
    pub duration_ms: u64,
    pub clips: Vec<ClipInfo>,
    pub has_audio: bool,
    pub has_camera: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipInfo {
    pub id: String,
    #[serde(rename = "startTime")]
    pub start_time: u64,
    pub duration: u64,
}

/// Response when break is added (Studio mode)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddBreakResponse {
    #[serde(rename = "clipId")]
    pub clip_id: String,
}

/// Project structure matching the spec
/// 
/// /clips/clip_0001/
/// ├── display.mp4
/// ├── camera.mp4?  
/// ├── mic.wav?
/// ├── system.wav?
/// └── cursor.json
/// /project.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub fps: u32, // 30 or 60
    #[serde(rename = "baseResolution")]
    pub base_resolution: Resolution,
    pub clips: Vec<Clip>,
    pub tracks: ProjectTracks,
    #[serde(rename = "cursorEvents")]
    pub cursor_events: Vec<CursorEvent>,
    pub effects: ProjectEffects,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    #[serde(rename = "startTime")]
    pub start_time: u64, // milliseconds
    pub duration: u64,   // milliseconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTracks {
    pub display: Vec<VideoTrack>,
    pub camera: Option<Vec<VideoTrack>>,
    pub mic: Option<Vec<AudioTrack>>,
    pub system: Option<Vec<AudioTrack>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoTrack {
    #[serde(rename = "clipId")]
    pub clip_id: String,
    pub path: String,
    #[serde(rename = "timeOffset")]
    pub time_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    #[serde(rename = "clipId")]
    pub clip_id: String,
    pub path: String,
    #[serde(rename = "timeOffset")]
    pub time_offset: u64,
    #[serde(rename = "channelMode")]
    pub channel_mode: ChannelMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelMode {
    Stereo,
    #[serde(rename = "monoL")]
    MonoLeft,
    #[serde(rename = "monoR")]
    MonoRight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEffects {
    pub zoom: Vec<ZoomSegment>,
}

/// Cursor event from native tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorEvent {
    pub t: u64, // timestamp in ms
    pub x: f64, // normalized 0.0-1.0
    pub y: f64, // normalized 0.0-1.0
    #[serde(rename = "type")]
    pub event_type: CursorEventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorEventType {
    Move,
    Down,
    Up,
    #[serde(rename = "dblclick")]
    DoubleClick,
}

/// Zoom segment for Smart Zoom system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomSegment {
    pub id: String,
    pub start: u64, // project time in ms
    pub end: u64,   // project time in ms
    pub mode: ZoomMode,
    pub strength: f64,  // 1.0 - 2.5
    pub easing: ZoomEasing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZoomMode {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "spot")]
    Spot { x: f64, y: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoomEasing {
    EaseInOut,
    Linear,
}


/// Lightweight recording state for UI polling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingState {
    Idle,
    Recording,
    Paused,
    Processing,
    Completed,
    Failed { error: String },
}

/// Safe area cropping for full screen recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeAreaCrop {
    /// Auto-detect menu bar height and notch
    pub auto_detect: bool,
    /// Manual crop values (pixels from each edge)
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl Default for SafeAreaCrop {
    fn default() -> Self {
        Self {
            auto_detect: true,
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        }
    }
}