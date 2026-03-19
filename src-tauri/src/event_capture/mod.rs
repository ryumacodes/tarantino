use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::mouse_tracking::MouseEvent;

/// Enhanced mouse event with additional context for auto-zoom analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedMouseEvent {
    pub base: MouseEvent,
    pub window_id: Option<String>,
    pub app_name: Option<String>,
    pub is_double_click: bool,
    pub cluster_id: Option<String>,
}

/// Keyboard event for shortcut detection and typing analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardEvent {
    pub timestamp: u64,
    pub key: String,
    pub modifiers: Vec<String>,
    pub combo: Option<String>, // e.g., "⌘⇧K"
    pub is_shortcut: bool,
    pub is_typing: bool,
}

/// Window bounds tracking for safe zoom constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowEvent {
    pub timestamp: u64,
    pub app_name: String,
    pub window_title: String,
    pub bounds: (i32, i32, u32, u32), // x, y, w, h
    pub is_focused: bool,
}

/// Audio source mapping for system audio processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSourceEvent {
    pub timestamp: u64,
    pub app_name: String,
    pub source_type: AudioSourceType,
    pub volume_level: f32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioSourceType {
    SystemAudio,
    Microphone,
    AppAudio { app_bundle_id: String },
}

/// Combined event data structure for comprehensive recording metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSession {
    pub session_id: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub mouse_events: Vec<EnhancedMouseEvent>,
    pub keyboard_events: Vec<KeyboardEvent>,
    pub window_events: Vec<WindowEvent>,
    pub audio_events: Vec<AudioSourceEvent>,
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub display_id: String,
    pub display_resolution: (u32, u32),
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f32,
    pub capture_region: Option<(i32, i32, u32, u32)>,
    #[serde(default)]
    pub has_microphone: bool,
    #[serde(default)]
    pub has_system_audio: bool,
    #[serde(default = "default_fps")]
    pub recording_fps: u32,
    #[serde(default = "default_quality")]
    pub recording_quality: f32,
}

fn default_scale_factor() -> f32 { 1.0 }
fn default_fps() -> u32 { 60 }
fn default_quality() -> f32 { 1.0 }

#[allow(dead_code)]
impl CaptureSession {
    pub fn new() -> Self {
        use uuid::Uuid;

        Self {
            session_id: Uuid::new_v4().to_string(),
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            end_time: None,
            mouse_events: Vec::new(),
            keyboard_events: Vec::new(),
            window_events: Vec::new(),
            audio_events: Vec::new(),
            metadata: SessionMetadata {
                display_id: "main".to_string(),
                display_resolution: (1920, 1080),
                scale_factor: 1.0,
                capture_region: None,
                has_microphone: false,
                has_system_audio: false,
                recording_fps: 60,
                recording_quality: 1.0,
            },
        }
    }

    pub fn start(&mut self) -> Result<()> {
        // Initialize event capture modules
        println!("Starting enhanced event capture session: {}", self.session_id);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.end_time = Some(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64);
        println!("Stopped enhanced event capture session: {}", self.session_id);
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        println!("Saved event capture session to: {}", path);
        Ok(())
    }
}
