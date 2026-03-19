//! HUD (Heads-Up Display) commands for recording overlay

use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use crate::state::UnifiedAppState;

#[tauri::command]
pub async fn hide_recording_hud(app: AppHandle) -> Result<(), String> {
    if let Some(hud) = app.get_webview_window("recording-hud") {
        hud.close().map_err(|e| e.to_string())?;
        println!("Recording HUD window closed");
    }
    Ok(())
}

#[tauri::command]
pub async fn hud_move(x: f64, y: f64, _state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    println!("HUD moved to position: ({}, {})", x, y);
    // TODO: Save position to persistent storage
    Ok(())
}

#[tauri::command]
pub async fn hud_query_capture_region(_state: State<'_, Arc<UnifiedAppState>>) -> Result<serde_json::Value, String> {
    println!("HUD querying capture region for auto-dodge");

    // TODO: Return the current capture region bounds
    Ok(serde_json::json!({
        "x": 0,
        "y": 0,
        "width": 1920,
        "height": 1080
    }))
}
