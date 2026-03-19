//! Uniform buffer management for the compositor
//!
//! Contains uniform struct definitions and helper functions for building
//! shader uniform data.

use bytemuck::{Pod, Zeroable};

use super::{CameraPipOptions, PipPosition, PipSize, ResolvedFrameData, ZoomData};

/// Uniform data for camera PIP rendering
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraUniforms {
    pub position: [f32; 2],       // Position in normalized coordinates
    pub size: [f32; 2],           // Size in normalized coordinates
    pub roundness: f32,           // Corner roundness (0.0-1.0)
    pub shadow_blur: f32,         // Shadow blur radius
    pub shadow_offset: [f32; 2],  // Shadow offset
    pub shadow_opacity: f32,      // Shadow opacity
}

/// Uniform data for zoom effects
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct ZoomUniforms {
    pub focus_point: [f32; 2], // Focus point in normalized coordinates
    pub zoom_factor: f32,      // Current zoom factor
    pub progress: f32,         // Animation progress for easing
}

/// Uniform data for background effects
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct BackgroundUniforms {
    pub blur_radius: f32,           // Background blur radius
    pub background_color: [f32; 4], // RGBA background color
}

impl CameraUniforms {
    /// Build disabled camera uniforms
    pub fn disabled() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            roundness: 0.0,
            shadow_blur: 0.0,
            shadow_offset: [0.0, 0.0],
            shadow_opacity: 0.0,
        }
    }

    /// Build camera uniforms from options
    pub fn from_options(
        camera_options: &CameraPipOptions,
        output_width: u32,
        output_height: u32,
    ) -> Self {
        if !camera_options.enabled {
            return Self::disabled();
        }

        // Calculate position and size based on options
        let (norm_width, norm_height) = match &camera_options.size {
            PipSize::Small => (0.15, 0.11),  // 160x120 at 1080p
            PipSize::Medium => (0.3, 0.22), // 320x240 at 1080p
            PipSize::Large => (0.6, 0.44),  // 640x480 at 1080p
            PipSize::Custom(w, h) => {
                (*w as f32 / output_width as f32, *h as f32 / output_height as f32)
            }
        };

        let position = match &camera_options.position {
            PipPosition::TopLeft => [0.02, 0.02],
            PipPosition::TopRight => [0.98 - norm_width, 0.02],
            PipPosition::BottomLeft => [0.02, 0.98 - norm_height],
            PipPosition::BottomRight => [0.98 - norm_width, 0.98 - norm_height],
            PipPosition::Custom(x, y) => [*x, *y],
        };

        Self {
            position,
            size: [norm_width, norm_height],
            roundness: camera_options.roundness,
            shadow_blur: camera_options.shadow.blur_radius,
            shadow_offset: [camera_options.shadow.offset_x, camera_options.shadow.offset_y],
            shadow_opacity: camera_options.shadow.opacity,
        }
    }
}

impl ZoomUniforms {
    /// Build zoom uniforms from frame data
    pub fn from_frame_data(frame_data: &ResolvedFrameData) -> Self {
        if let Some(zoom_data) = &frame_data.zoom_data {
            Self::from_zoom_data(zoom_data)
        } else {
            Self::default_no_zoom()
        }
    }

    /// Build zoom uniforms from zoom data
    pub fn from_zoom_data(zoom_data: &ZoomData) -> Self {
        Self {
            focus_point: [zoom_data.focus_x, zoom_data.focus_y],
            zoom_factor: zoom_data.zoom_factor,
            progress: zoom_data.progress,
        }
    }

    /// Build default zoom uniforms (no zoom)
    pub fn default_no_zoom() -> Self {
        Self {
            focus_point: [0.5, 0.5],
            zoom_factor: 1.0,
            progress: 0.0,
        }
    }
}

impl BackgroundUniforms {
    /// Build background uniforms
    pub fn new(blur_radius: f32) -> Self {
        Self {
            blur_radius,
            background_color: [0.0, 0.0, 0.0, 1.0], // Black background
        }
    }
}
