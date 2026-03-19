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
//! - cursor: Cursor intelligence engine
//! - export_pipeline: Export pipeline commands
//! - preview: Video preview playback
//! - display_preview: Display selection preview overlay
//! - misc: Logging, capture bar, exit
//! - tray: System tray management
//! - recording_control: Recording start/stop/pause commands
//! - processing: Recording processing and editor helpers

pub mod capture;
pub mod cursor;
pub mod device;
pub mod display_preview;
pub mod export_pipeline;
pub mod hud;
pub mod input;
pub mod misc;
pub mod mouse;
pub mod preview;
pub mod processing;
pub mod recording_control;
pub mod tray;
pub mod video;
pub mod video_validation;
pub mod zoom;

// Re-export only items that are used via `commands::` path
pub use misc::reset_tray_to_idle;
pub use tray::create_idle_tray_menu;
pub use recording_control::{record_stop_instant, STOPPING_RECORDING};
