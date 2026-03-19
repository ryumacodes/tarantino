//! Device and display enumeration commands

use std::sync::Arc;
use tauri::State;
use crate::state::{self, UnifiedAppState};

#[derive(serde::Serialize)]
pub struct DisplayBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

#[tauri::command]
pub async fn get_displays(state: State<'_, Arc<UnifiedAppState>>) -> Result<Vec<state::Display>, String> {
    state.get_displays().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_selected_display(state: State<'_, Arc<UnifiedAppState>>) -> Result<Option<state::Display>, String> {
    let app = state.app.read();
    let sel = app.selected_display_id.clone();
    let display = sel.and_then(|id| app.displays.iter().find(|d| d.id == id).cloned());
    Ok(display)
}

#[tauri::command]
pub async fn get_displays_with_thumbnails(
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<Vec<state::Display>, String> {
    state.get_displays_with_thumbnails().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_display_bounds(display_id: String, state: State<'_, Arc<UnifiedAppState>>) -> Result<DisplayBounds, String> {
    let displays = state.get_displays().await.map_err(|e| e.to_string())?;

    let display = displays.iter()
        .find(|d| d.id == display_id)
        .ok_or_else(|| format!("Display with id {} not found", display_id))?;

    Ok(DisplayBounds {
        x: 0,
        y: 0,
        width: display.width,
        height: display.height,
        scale_factor: display.scale_factor,
    })
}

#[tauri::command]
pub async fn get_windows(state: State<'_, Arc<UnifiedAppState>>) -> Result<Vec<state::Window>, String> {
    state.get_windows().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_devices(state: State<'_, Arc<UnifiedAppState>>) -> Result<Vec<state::Device>, String> {
    state.get_capture_devices().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_audio_devices(
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<state::AudioDevices, String> {
    state.get_audio_devices().await.map_err(|e| e.to_string())
}
