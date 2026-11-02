//! Video processing module
//!
//! Provides video processing, thumbnail generation, and export.
//! Scene compositing uses wgpu; small shortcut tiles blend into the readback.

pub mod audio_export;
pub mod background_audio;
pub mod codec_config;
pub mod export;
pub mod shortcut_overlay;
mod caption_overlay;
mod caption_text_gpu;
mod caption_background;
mod caption_corner;
mod mask_cover;
mod mask_sampling;
mod mask_stage;
pub mod mask_export;
mod mask_composite;
mod caption_stage;
mod shortcut_motion;
mod shortcut_gpu;
mod shortcut_stage;
pub mod export_control;
pub mod export_range;
pub mod gpu_compositor;
pub mod processor;
pub mod thumbnails;
pub mod types;
pub mod timeline;
pub mod timeline_decode;
pub mod visual_effects;
pub mod zoom_trajectory;
pub(crate) mod camera_activity;
pub mod webcam_timeline;

#[cfg(test)]
mod performance_tests;

// Re-export types used by other modules
pub use types::{CursorSettings, ExportSettings, ProcessingProgress, VideoInfo};

// Re-export processor
pub use processor::VideoProcessor;

#[cfg(test)]
mod trim_integration_tests;

#[cfg(test)]
mod cursor_visibility_integration_tests;

pub mod source_crop;
pub mod loupe;

#[cfg(test)]
mod loupe_export_tests;

#[cfg(test)]
mod source_crop_export_tests;

pub mod camera_motion;

#[cfg(test)]
mod local_export_tests;

#[cfg(test)]
mod caption_export_tests;

#[cfg(test)]
mod mask_export_tests;

#[cfg(test)]
mod recorded_cursor_export_tests;

#[cfg(test)]
mod camera_activity_export_tests;
