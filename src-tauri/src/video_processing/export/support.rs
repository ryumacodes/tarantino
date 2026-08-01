use std::path::Path;

use super::super::types::ExportSettings;
use crate::cursor_renderer::{CursorEvent, SpringConfig, parse_cursor_events};

/// Find the cursor sidecar, including the original recording fallback used by
/// post-processed media.
pub(super) fn resolve_mouse_events_path(input_path: &Path) -> std::path::PathBuf {
    let direct = crate::recording::artifacts::RecordingArtifacts::new(input_path).mouse();
    if direct.exists() {
        return direct;
    }

    let Some(file_name) = input_path.file_name().and_then(|name| name.to_str()) else {
        return direct;
    };
    if !file_name.starts_with("processed_") {
        return direct;
    }

    let original = input_path.with_file_name(file_name.replacen("processed_", "", 1));
    let fallback = crate::recording::artifacts::RecordingArtifacts::new(original).mouse();
    if fallback.exists() { fallback } else { direct }
}

/// Load raw cursor events from sidecar for zoom trajectory simulation.
pub fn load_raw_cursor_events(
    mouse_events_path: &Path,
    source_width: u32,
    source_height: u32,
) -> Vec<CursorEvent> {
    let content = match std::fs::read_to_string(mouse_events_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let sidecar: serde_json::Value = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let (events, _sf, ex, ey, ew, eh) = if let Some(mouse_events) = sidecar.get("mouse_events") {
        let dw = sidecar
            .get("display_width")
            .and_then(|v| v.as_f64())
            .unwrap_or(source_width as f64);
        let dh = sidecar
            .get("display_height")
            .and_then(|v| v.as_f64())
            .unwrap_or(source_height as f64);
        let sf = sidecar
            .get("scale_factor")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let recording_area = sidecar.get("recording_area");
        let (ex, ey, ew, eh) = if let Some(area) = recording_area {
            (
                area.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                area.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
                area.get("width").and_then(|v| v.as_f64()).unwrap_or(dw),
                area.get("height").and_then(|v| v.as_f64()).unwrap_or(dh),
            )
        } else {
            (0.0, 0.0, dw, dh)
        };
        (
            mouse_events.as_array().cloned().unwrap_or_default(),
            sf,
            ex,
            ey,
            ew,
            eh,
        )
    } else if let Some(arr) = sidecar.as_array() {
        (
            arr.clone(),
            1.0,
            0.0,
            0.0,
            source_width as f64,
            source_height as f64,
        )
    } else {
        return Vec::new();
    };

    parse_cursor_events(&events, 1.0, ex, ey, ew, eh)
}

pub(super) fn get_zoom_spring_config(settings: &ExportSettings) -> SpringConfig {
    if let (Some(t), Some(f), Some(m)) = (
        settings.zoom_spring_tension,
        settings.zoom_spring_friction,
        settings.zoom_spring_mass,
    ) {
        return SpringConfig {
            tension: t,
            friction: f,
            mass: m,
        };
    }
    let name = settings.animation_speed.as_deref().unwrap_or("mellow");
    resolve_spring_preset(name)
}

pub(super) fn get_cursor_spring_config(settings: &ExportSettings) -> SpringConfig {
    if let Some(ref cursor) = settings.cursor_settings {
        if let (Some(t), Some(f), Some(m)) = (
            cursor.spring_tension,
            cursor.spring_friction,
            cursor.spring_mass,
        ) {
            return SpringConfig {
                tension: t,
                friction: f,
                mass: m,
            };
        }
        if let Some(ref preset) = cursor.speed_preset {
            return resolve_spring_preset(preset);
        }
    }
    SpringConfig {
        tension: 170.0,
        friction: 30.0,
        mass: 1.0,
    }
}

pub fn resolve_spring_preset(name: &str) -> SpringConfig {
    match name {
        "slow" => SpringConfig {
            tension: 120.0,
            friction: 28.0,
            mass: 1.0,
        },
        "mellow" => SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        },
        "quick" => SpringConfig {
            tension: 280.0,
            friction: 38.0,
            mass: 1.0,
        },
        "rapid" => SpringConfig {
            tension: 400.0,
            friction: 44.0,
            mass: 1.0,
        },
        _ => SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        },
    }
}
