//! Command modules for Tauri IPC
//!
//! This module organizes all Tauri commands into logical groups:
//! - capture: Capture source configuration
//! - input: Input device configuration (mic, camera, webcam)
//! - device: Display, window, and audio device enumeration
//! - video: Video processing, thumbnails, waveforms
//! - hud: Recording HUD overlay
//! - mouse: Mouse tracking
//! - zoom: Auto-zoom data management
//! - display_preview: Display selection preview overlay
//! - misc: Logging, capture bar, exit
//! - tray: System tray management
//! - recording_control: Recording start/stop/pause commands
//! - processing: Recording processing and editor helpers

pub mod capture;
pub mod audio_assets;
pub mod transcription;
pub mod device;
pub mod display_preview;
mod editor_ready;
pub mod hud;
pub mod input;
pub mod lifecycle;
pub mod misc;
pub mod mouse;
pub mod motion;
pub mod processing;
pub mod projects;
pub mod presets;
pub mod recording_control;
pub mod tray;
pub mod video;
pub mod video_validation;
pub mod zoom;
pub mod zoom_rebuild;

// Re-export only items that are used via `commands::` path
pub use misc::reset_tray_to_idle;
pub use recording_control::{STOPPING_RECORDING, record_stop_instant};
pub use tray::create_idle_tray_menu;

pub mod export_jobs;

pub mod loupe;

pub mod camera_opacity;
