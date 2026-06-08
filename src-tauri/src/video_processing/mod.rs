//! Video processing module
//!
//! Provides video processing, thumbnail generation, and export.
//! All per-frame compositing is GPU-accelerated via wgpu compute shaders.

pub mod audio_export;
pub mod codec_config;
pub mod export;
pub mod gpu_compositor;
pub mod processor;
pub mod thumbnails;
pub mod types;
pub mod visual_effects;
pub mod zoom_trajectory;

// Re-export types used by other modules
pub use types::{CursorSettings, ExportSettings, ProcessingProgress, VideoInfo};

// Re-export processor
pub use processor::VideoProcessor;
