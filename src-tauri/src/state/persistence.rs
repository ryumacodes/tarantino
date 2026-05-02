use anyhow::Result;
use std::path::PathBuf;

use super::UnifiedAppState;

/// Persistence-related methods for UnifiedAppState
impl UnifiedAppState {
    /// Get configuration directory path
    pub fn get_config_directory(&self) -> Result<PathBuf> {
        let home_dir = std::env::var("HOME")
            .map_err(|_| anyhow::anyhow!("HOME environment variable not set"))?;

        let config_dir = std::path::Path::new(&home_dir)
            .join(".config")
            .join("tarantino");

        Ok(config_dir)
    }

    /// Load saved application settings from disk
    pub async fn load_app_settings(&self) -> Result<()> {
        let config_dir = self.get_config_directory()?;
        let settings_path = config_dir.join("settings.json");

        if settings_path.exists() {
            self.app.load_from_file(settings_path.to_str().unwrap())?;
            println!("Loaded app settings from: {}", settings_path.display());
        }

        Ok(())
    }

    /// Save application settings to disk
    pub async fn save_app_settings(&self) -> Result<()> {
        let config_dir = self.get_config_directory()?;
        std::fs::create_dir_all(&config_dir)?;

        let settings_path = config_dir.join("settings.json");
        self.app.save_to_file(settings_path.to_str().unwrap())?;

        println!("Saved app settings to: {}", settings_path.display());
        Ok(())
    }

    /// Get the current recording's sidecar path
    /// The sidecar folder contains metadata files like .mouse.json, .auto_zoom.json, webcam.webm, etc.
    pub fn get_current_sidecar_path(&self) -> Option<String> {
        let config = self.recording.get_current_config()?;
        let output_path = std::path::Path::new(&config.output_path);

        // Sidecar folder is the output file's stem + ".sidecar"
        // e.g., /tmp/recording.mp4 -> /tmp/recording.sidecar/
        let parent = output_path.parent()?;
        let stem = output_path.file_stem()?.to_str()?;
        let sidecar_path = parent.join(format!("{}.sidecar", stem));

        // Create the sidecar folder if it doesn't exist
        if !sidecar_path.exists() {
            if let Err(e) = std::fs::create_dir_all(&sidecar_path) {
                println!("Failed to create sidecar folder: {}", e);
                return None;
            }
        }

        sidecar_path.to_str().map(String::from)
    }
}

/// Save zoom analysis and mouse events to sidecar files
pub fn save_zoom_sidecar(
    video_path: &str,
    analysis: &crate::auto_zoom::ZoomAnalysis,
    mouse_events: &[crate::event_capture::EnhancedMouseEvent],
    display_info: (u32, u32, f32, Option<crate::recording::types::RecordingArea>),
    capture_mode: &str,
    screen_dims: Option<(u32, u32)>,
) -> Result<()> {
    use crate::auto_zoom::save_analysis;

    let (width, height, scale_factor, recording_area) = display_info;

    // Save zoom analysis (use consistent naming: strip .mp4 before adding extension)
    let base_path = video_path.trim_end_matches(".mp4");
    let zoom_path = format!("{}.auto_zoom.json", base_path);
    save_analysis(analysis, &zoom_path)?;
    println!("Zoom analysis saved to: {}", zoom_path);

    // Save raw mouse events for preview zoom indicators
    // Include display resolution, scale factor, and recording area for proper coordinate normalization
    let mouse_path = format!("{}.mouse.json", base_path);
    let mouse_sidecar = serde_json::json!({
        "capture_mode": capture_mode,
        "display_width": width,
        "display_height": height,
        // For window recordings, the host display dimensions (used for proportional sizing in editor)
        "screen_width": screen_dims.map(|(w, _)| w),
        "screen_height": screen_dims.map(|(_, h)| h),
        "scale_factor": scale_factor,
        "recording_area": recording_area.as_ref().map(|area| serde_json::json!({
            "x": area.x,
            "y": area.y,
            "width": area.width,
            "height": area.height
        })),
        "mouse_events": mouse_events,
    });
    let mouse_json = serde_json::to_string_pretty(&mouse_sidecar)?;
    std::fs::write(&mouse_path, &mouse_json)?;
    println!("Mouse events saved to {} (display: {}x{}, scale: {})", mouse_path, width, height, scale_factor);

    Ok(())
}
