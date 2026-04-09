//! Video processing types and data structures

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bitrate: u64,
    pub format: String,
    pub size_bytes: u64,
    pub frame_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    pub color: String,
    pub position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualSettings {
    pub background_type: Option<String>,
    pub background_color: Option<String>,
    pub gradient_direction: Option<String>,
    pub gradient_stops: Option<Vec<GradientStop>>,
    pub padding: Option<f64>,
    pub corner_radius: Option<f64>,
    pub shadow_enabled: Option<bool>,
    pub shadow_intensity: Option<f64>,
    pub shadow_blur: Option<f64>,
    pub shadow_offset_x: Option<f64>,
    pub shadow_offset_y: Option<f64>,
    pub aspect_ratio: Option<String>,
    // Motion blur settings (3 channels, Screen Studio style)
    pub motion_blur_enabled: Option<bool>,
    pub motion_blur_pan_intensity: Option<f64>, // 0.0-1.0, blur during camera pans
    pub motion_blur_zoom_intensity: Option<f64>, // 0.0-1.0, blur during zoom in/out
    pub motion_blur_cursor_intensity: Option<f64>, // 0.0-1.0, blur on cursor movement
    // Device frame settings
    pub device_frame: Option<String>, // none, iphone-15-pro, iphone-15, ipad-pro, macbook-pro, browser
    pub device_frame_color: Option<String>, // black, silver, gold, blue
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorSettings {
    pub enabled: Option<bool>,
    pub size: Option<f64>,
    pub highlight_clicks: Option<bool>,
    pub smoothing: Option<f64>,
    // Visual style settings
    pub style: Option<String>,            // "pointer" | "circle" | "filled" | "outline" | "dotted"
    pub always_use_pointer: Option<bool>, // Force pointer style
    pub color: Option<String>,            // hex color e.g. "#ffffff"
    pub highlight_color: Option<String>,  // hex color for click highlight
    pub ripple_color: Option<String>,     // hex color for ripple effect
    pub shadow_intensity: Option<f64>,    // 0-100
    // Trail settings
    pub trail_enabled: Option<bool>,
    pub trail_length: Option<u32>,        // 5-30 positions
    pub trail_opacity: Option<f64>,       // 0-1
    // Click effect
    pub click_effect: Option<String>,     // "none" | "circle" | "ripple"
    // Spring physics — actual values from frontend SPRING_PRESETS (source of truth)
    pub speed_preset: Option<String>,     // "slow" | "mellow" | "quick" | "rapid"
    pub spring_tension: Option<f64>,
    pub spring_friction: Option<f64>,
    pub spring_mass: Option<f64>,
    // Rotation
    pub rotation: Option<f64>,            // 0-360 degrees
    pub rotate_while_moving: Option<bool>,
    pub rotation_intensity: Option<f64>,  // 0-100
    // Idle behavior
    pub hide_when_idle: Option<bool>,
    pub idle_timeout: Option<u64>,        // ms
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettingsExport {
    pub mic_gain: Option<f64>,
    pub system_gain: Option<f64>,
    pub noise_gate: Option<bool>,
    pub dual_track: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomBlock {
    pub start_time_ms: u64,
    pub end_time_ms: u64,
    pub zoom_level: f64,
    pub center_x: f64,
    pub center_y: f64,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub zoom_in_speed: Option<String>,
    #[serde(default)]
    pub zoom_out_speed: Option<String>,
}

/// Cursor frame data for export rendering (per-frame cursor position)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CursorFrame {
    pub frame: u64,
    pub x: f64,  // Normalized 0-1
    pub y: f64,  // Normalized 0-1
    pub is_click: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    // Output path (optional - will generate default if null)
    pub output_path: Option<String>,
    // Project title (used for output filename if output_path is null)
    pub project_title: Option<String>,
    // Resolution settings
    pub resolution: Option<Resolution>,
    // Frame rate (24, 30, 60)
    pub frame_rate: Option<u32>,
    // Quality preset (low, medium, high)
    pub quality: Option<String>,
    // Output format (mp4, mov, webm, gif)
    pub format: Option<String>,
    // Codec (h264, prores, vp9)
    pub codec: Option<String>,
    // Trim settings
    pub trim_start: Option<u64>,
    pub trim_end: Option<u64>,
    // Visual settings for effects
    pub visual_settings: Option<VisualSettings>,
    // Cursor settings
    pub cursor_settings: Option<CursorSettings>,
    // Audio settings
    pub audio_settings: Option<AudioSettingsExport>,
    // Zoom blocks for effects
    pub zoom_blocks: Option<Vec<ZoomBlock>>,
    // Source video dimensions for aspect-correct scaling
    pub source_width: Option<u32>,
    pub source_height: Option<u32>,
    // Animation speed preset + actual spring values from frontend (source of truth)
    pub animation_speed: Option<String>,
    pub zoom_spring_tension: Option<f64>,
    pub zoom_spring_friction: Option<f64>,
    pub zoom_spring_mass: Option<f64>,
    // Webcam overlay settings
    pub webcam_corner: Option<String>,  // "top-left", "top-right", "bottom-left", "bottom-right"
    pub webcam_size: Option<f64>,       // fraction of output width (0.08-0.25)
    pub webcam_shape: Option<String>,   // "circle" or "roundrect"
    // Capture mode: "display" or "window" (affects how zoom is applied in export)
    pub capture_mode: Option<String>,
    // Legacy fields for compatibility
    #[serde(default)]
    pub zoom_keyframes: Option<serde_json::Value>,
    #[serde(default)]
    pub zoom_analysis: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingProgress {
    pub current_frame: u64,
    pub total_frames: u64,
    pub percentage: f64,
    pub estimated_remaining_ms: Option<u64>,
}

/// Validate zoom blocks: sort by time, clamp to duration, zero-gap truncation for overlaps
pub fn validate_export_zoom_blocks(blocks: &mut Vec<ZoomBlock>, duration_ms: u64) {
    if blocks.is_empty() {
        return;
    }

    blocks.sort_by_key(|b| b.start_time_ms);

    for block in blocks.iter_mut() {
        block.end_time_ms = block.end_time_ms.min(duration_ms);
    }

    // Zero-gap: truncate A.end to B.start (no merge, no gap)
    for i in 1..blocks.len() {
        if blocks[i - 1].end_time_ms > blocks[i].start_time_ms {
            blocks[i - 1].end_time_ms = blocks[i].start_time_ms;
        }
    }

    // Remove blocks shorter than 500ms
    blocks.retain(|b| b.end_time_ms > b.start_time_ms + 500);

    println!("Validated {} zoom blocks (duration: {}ms)", blocks.len(), duration_ms);
}

/// Load zoom blocks from sidecar file
pub fn load_zoom_blocks_from_sidecar(video_path: &std::path::Path) -> Option<Vec<ZoomBlock>> {
    use crate::auto_zoom;

    let video_str = video_path.to_str()?;
    let base_path = video_str.trim_end_matches(".mp4");
    let zoom_path = format!("{}.auto_zoom.json", base_path);

    if !std::path::Path::new(&zoom_path).exists() {
        return None;
    }

    match auto_zoom::load_analysis(&zoom_path) {
        Ok(mut analysis) => {
            // Validate blocks loaded from sidecar
            auto_zoom::validate_zoom_blocks(&mut analysis.zoom_blocks, analysis.session_duration);

            let blocks: Vec<ZoomBlock> = analysis.zoom_blocks
                .into_iter()
                .map(|b| ZoomBlock {
                    start_time_ms: b.start_time,
                    end_time_ms: b.end_time,
                    zoom_level: b.zoom_factor as f64,
                    center_x: b.center_x,
                    center_y: b.center_y,
                    kind: Some(b.kind),
                    zoom_in_speed: b.zoom_in_speed,
                    zoom_out_speed: b.zoom_out_speed,
                })
                .collect();
            Some(blocks)
        }
        Err(_) => None,
    }
}
