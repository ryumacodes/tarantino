//! Type definitions for the preview compositor
//!
//! Contains data structures for frame data, video formats, and cursor/zoom data.

/// Frame data resolved from the timeline
#[derive(Debug)]
pub struct ResolvedFrameData {
    /// Display video frame (always present)
    pub display_frame: Option<VideoFrameData>,

    /// Camera PIP frame (optional)
    pub camera_frame: Option<VideoFrameData>,

    /// Cursor data at this timestamp
    pub cursor_data: Option<CursorData>,

    /// Active zoom effect at this timestamp
    pub zoom_data: Option<ZoomData>,

    /// Timestamp of this frame
    pub timestamp_ms: u64,
}

#[derive(Debug)]
pub struct VideoFrameData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA8 or YUV420 data
    pub format: VideoFormat,
}

#[derive(Debug)]
pub enum VideoFormat {
    Rgba8,
    Yuv420,
    Nv12,
}

#[derive(Debug)]
pub struct CursorData {
    pub x: f32,         // Normalized coordinates (0.0-1.0)
    pub y: f32,
    pub width: u32,     // Cursor image dimensions
    pub height: u32,
    pub image: Vec<u8>, // RGBA8 cursor image
    pub visible: bool,
}

#[derive(Debug)]
pub struct ZoomData {
    pub focus_x: f32,     // Focus point in normalized coordinates
    pub focus_y: f32,
    pub zoom_factor: f32, // 1.0 = no zoom, 2.0 = 2x zoom
    pub progress: f32,    // Animation progress (0.0-1.0) for easing
}
