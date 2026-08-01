use std::sync::Arc;

use tauri::State;

use crate::recording::types::*;
use crate::state::UnifiedAppState;

pub(super) fn resolve_recording_target(
    state: &UnifiedAppState,
    frontend_target_type: String,
    frontend_target_id: String,
) -> Result<(String, String), String> {
    let app_read = state.app.read();
    let backend_mode = app_read.capture_mode.as_str();
    let resolved = match frontend_target_type.as_str() {
        "desktop" if frontend_target_id != "0" => {
            ("desktop".to_string(), frontend_target_id.clone())
        }
        "window" if frontend_target_id != "0" => ("window".to_string(), frontend_target_id.clone()),
        "device" if frontend_target_id != "0" => ("device".to_string(), frontend_target_id.clone()),
        _ => match backend_mode {
            "desktop" => {
                let id = app_read
                    .selected_display_id
                    .clone()
                    .or_else(|| app_read.displays.first().map(|display| display.id.clone()))
                    .or_else(|| {
                        if frontend_target_type == "desktop" && frontend_target_id != "0" {
                            Some(frontend_target_id.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| {
                        "No display is selected yet. Wait for displays to load, then try again."
                            .to_string()
                    })?;
                ("desktop".to_string(), id)
            }
            "window" => {
                let id = app_read
                    .selected_window_id
                    .clone()
                    .or_else(|| {
                        if frontend_target_type == "window" && frontend_target_id != "0" {
                            Some(frontend_target_id.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| {
                        "No window is selected yet. Pick a window after the list loads.".to_string()
                    })?;
                ("window".to_string(), id)
            }
            "device" => {
                let id = app_read
                    .selected_device_id
                    .clone()
                    .unwrap_or_else(|| frontend_target_id.clone());
                ("device".to_string(), id)
            }
            _ => (frontend_target_type.clone(), frontend_target_id.clone()),
        },
    };

    println!(
        "Recording target resolved: frontend={}:{} backend_mode={} => {}:{}",
        frontend_target_type, frontend_target_id, backend_mode, resolved.0, resolved.1
    );
    Ok(resolved)
}

/// Build recording configuration from parameters
pub(super) fn build_recording_config(
    target_type: String,
    target_id: String,
    quality: String,
    include_cursor: bool,
    include_microphone: bool,
    include_system_audio: bool,
    output_path: Option<String>,
) -> Result<RecordingConfig, String> {
    // Parse target
    let target = match target_type.as_str() {
        "desktop" => {
            // Support optional area selection via target_id syntax: "<display_id>:x,y,w,h"
            let mut parts = target_id.split(':');
            let display_id_str = parts.next().unwrap_or("");
            let display_id = display_id_str
                .parse::<u32>()
                .map_err(|_| "Invalid display ID")?;

            let area = if let Some(area_str) = parts.next() {
                // Expect x,y,w,h
                let nums: Vec<&str> = area_str.split(',').collect();
                if nums.len() == 4 {
                    let x = nums[0].parse::<i32>().map_err(|_| "Invalid area x")?;
                    let y = nums[1].parse::<i32>().map_err(|_| "Invalid area y")?;
                    let w = nums[2].parse::<u32>().map_err(|_| "Invalid area width")?;
                    let h = nums[3].parse::<u32>().map_err(|_| "Invalid area height")?;
                    Some(RecordingArea {
                        x,
                        y,
                        width: w,
                        height: h,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            RecordingTarget::Desktop { display_id, area }
        }
        "window" => {
            let window_id = target_id.parse::<u64>().map_err(|_| "Invalid window ID")?;
            RecordingTarget::Window {
                window_id,
                include_shadow: true,
            }
        }
        "device" => {
            RecordingTarget::Device {
                device_id: target_id,
                device_type: DeviceType::Camera, // Default to camera for now
            }
        }
        _ => return Err("Invalid target type".to_string()),
    };

    // Parse quality preset
    let quality_preset = match quality.as_str() {
        "Lossless" => QualityPreset::Lossless,
        "High" => QualityPreset::High,
        "Medium" => QualityPreset::Medium,
        "Low" => QualityPreset::Low,
        _ => QualityPreset::High, // Default to high
    };

    // Generate output path if not provided
    let final_output_path = output_path.unwrap_or_else(|| {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
        #[cfg(target_os = "linux")]
        let media_dir = format!("{}/Videos", home_dir);
        #[cfg(not(target_os = "linux"))]
        let media_dir = format!("{}/Movies", home_dir);
        format!("{}/Tarantino/Recording_{}.mp4", media_dir, timestamp)
    });

    Ok(RecordingConfig {
        target,
        quality: quality_preset,
        output_format: OutputFormat {
            container: Container::MP4,
            codec: VideoCodec::H264,
            audio_codec: Some(AudioCodec::AAC),
        },
        output_path: final_output_path,
        // Linux/Wayland blocks global cursor tracking, so the portal must embed
        // the cursor when the user asks for it. macOS keeps the editable overlay.
        include_cursor: cfg!(target_os = "linux") && include_cursor,
        cursor_size: 1.0,
        highlight_clicks: false,
        include_microphone,
        microphone_device: None, // TODO: Support device selection
        include_system_audio,
    })
}

pub(super) fn validate_recording_target(
    state: &State<'_, Arc<UnifiedAppState>>,
    config: &RecordingConfig,
) -> Result<(), String> {
    match &config.target {
        RecordingTarget::Desktop { display_id, .. } => {
            let cached_displays = state.cached_displays();
            if cached_displays
                .iter()
                .any(|display| display.id == display_id.to_string())
            {
                Ok(())
            } else {
                Err("Selected display is not available yet. Try again after the display picker loads.".to_string())
            }
        }
        RecordingTarget::Window { window_id, .. } => {
            let cached_windows = state.cached_windows();
            let window = cached_windows
                .iter()
                .find(|window| window.id == window_id.to_string())
                .ok_or_else(|| {
                    "Selected window is stale. Pick a window again after the list loads."
                        .to_string()
                })?;
            let app_name = window.app_name.to_lowercase();
            let title = window.title.to_lowercase();
            if app_name == "tarantino" || title == "tarantino" || title.contains("web inspector") {
                return Err("Cannot record Tarantino's own windows. Choose another window or switch to display capture.".to_string());
            }
            Ok(())
        }
        RecordingTarget::Device { .. } => Ok(()),
    }
}

pub(super) fn normalize_webcam_shape(shape: &str) -> String {
    match shape {
        "roundrect" | "rounded" => "roundrect".to_string(),
        _ => "circle".to_string(),
    }
}
