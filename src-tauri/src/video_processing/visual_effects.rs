//! Visual effects configuration helpers.
//!
//! Provides output dimension calculation, cursor config defaults,
//! output path determination, and webcam info loading.
//! All per-frame visual effects are now GPU-accelerated via gpu_compositor.rs.

use std::path::{Path, PathBuf};
use anyhow::Result;

use super::types::{ExportSettings, CursorSettings};

/// Get output dimensions based on settings
pub fn get_output_dimensions(settings: &ExportSettings) -> (u32, u32) {
    let aspect_ratio = settings.visual_settings.as_ref()
        .and_then(|v| v.aspect_ratio.as_deref())
        .unwrap_or("auto");

    if let Some(res) = settings.resolution.as_ref() {
        (res.width, res.height)
    } else {
        match aspect_ratio {
            "16:9" => (1920, 1080),
            "9:16" => (1080, 1920),
            "4:3" => (1440, 1080),
            "1:1" => (1080, 1080),
            "21:9" => (2560, 1080),
            _ => (1920, 1080),
        }
    }
}

/// Get cursor configuration with defaults
pub fn get_cursor_config(settings: &ExportSettings) -> CursorSettings {
    settings.cursor_settings.clone().unwrap_or_else(|| CursorSettings {
        enabled: Some(true),
        size: Some(1.0),
        highlight_clicks: Some(true),
        smoothing: Some(0.15),
        style: Some("pointer".to_string()),
        always_use_pointer: Some(false),
        color: Some("#ffffff".to_string()),
        highlight_color: Some("#ff6b6b".to_string()),
        ripple_color: Some("#64b4ff".to_string()),
        shadow_intensity: Some(30.0),
        trail_enabled: Some(false),
        trail_length: Some(10),
        trail_opacity: Some(0.5),
        click_effect: Some("ripple".to_string()),
        speed_preset: Some("mellow".to_string()),
        spring_tension: Some(170.0),
        spring_friction: Some(30.0),
        spring_mass: Some(1.0),
        rotation: Some(0.0),
        rotate_while_moving: Some(false),
        rotation_intensity: Some(50.0),
        hide_when_idle: Some(true),
        idle_timeout: Some(3000),
    })
}

/// Determine output path for export
pub fn determine_output_path(settings: &ExportSettings, input_path: &Path) -> Result<PathBuf> {
    if let Some(ref path) = settings.output_path {
        Ok(PathBuf::from(path))
    } else {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let movies_dir = PathBuf::from(format!("{}/Movies/Tarantino", home_dir));
        std::fs::create_dir_all(&movies_dir)?;

        let format = settings.format.as_deref().unwrap_or("mp4");
        let base_name = settings.project_title
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                input_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("export")
            });
        let sanitized_name: String = base_name
            .chars()
            .map(|c| if c == '/' || c == '\\' || c == ':' || c == '*' || c == '?' || c == '"' || c == '<' || c == '>' || c == '|' { '_' } else { c })
            .collect();

        let mut output_path = movies_dir.join(format!("{}.{}", sanitized_name, format));

        let same_file = input_path.canonicalize().ok() == output_path.canonicalize().ok()
            || input_path == output_path;
        if same_file {
            output_path = movies_dir.join(format!("processed_{}.{}", sanitized_name, format));
        }

        Ok(output_path)
    }
}

/// Get webcam info — looks for {recording}.webcam.mp4 alongside the video.
/// Falls back to old sidecar/webcam.webm format for compatibility.
pub fn get_webcam_info(input_path: &Path) -> Option<(PathBuf, f64, f64, f64, String)> {
    let dir = input_path.parent()?;
    let stem = input_path.file_stem()?.to_str()?;

    // Strip processed_ prefix to find the original webcam file
    let base_stem = if stem.starts_with("processed_") {
        &stem["processed_".len()..]
    } else {
        stem
    };

    // New format: {base}.webcam.mp4 alongside the video
    let webcam_mp4 = dir.join(format!("{}.webcam.mp4", base_stem));
    if webcam_mp4.exists() {
        // Use default position/size — editor settings are applied via GPU config
        println!("Found webcam overlay: {}", webcam_mp4.display());
        return Some((webcam_mp4, 0.85, 0.85, 0.15, "circle".to_string()));
    }

    // Legacy: sidecar folder format
    let sidecar_folder = dir.join(format!("{}.sidecar", base_stem));
    let webcam_webm = sidecar_folder.join("webcam.webm");
    if webcam_webm.exists() {
        let webcam_metadata_path = sidecar_folder.join("webcam.json");
        if let Ok(metadata_str) = std::fs::read_to_string(&webcam_metadata_path) {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&metadata_str) {
                let pos_x = meta["position"]["x"].as_f64().unwrap_or(0.86);
                let pos_y = meta["position"]["y"].as_f64().unwrap_or(0.14);
                let size = meta["size"].as_f64().unwrap_or(0.12);
                let shape = meta["shape"].as_str().unwrap_or("circle").to_string();
                return Some((webcam_webm, pos_x, pos_y, size, shape));
            }
        }
    }

    None
}
