//! Video processing module
//!
//! Provides video processing, thumbnail generation, export, and cursor compositing.
//! All per-frame compositing is GPU-accelerated via wgpu compute shaders.

pub mod codec_config;
pub mod cursor_compositing;
pub mod export;
pub mod gpu_compositor;
pub mod metadata;
pub mod processor;
pub mod thumbnails;
pub mod types;
pub mod visual_effects;
pub mod zoom_trajectory;

// Re-export types used by other modules
pub use types::{VideoInfo, CursorSettings, ExportSettings, ProcessingProgress};

// Re-export processor
pub use processor::VideoProcessor;
