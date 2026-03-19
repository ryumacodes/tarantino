//! Mouse tracking commands

use std::sync::Arc;
use tauri::State;
use crate::mouse_tracking::{MouseEvent, MouseTrackingStats};
use crate::state::UnifiedAppState;

#[tauri::command]
pub async fn start_mouse_tracking(state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.start_mouse_tracking().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_mouse_tracking(state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.stop_mouse_tracking().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_mouse_events(state: State<'_, Arc<UnifiedAppState>>) -> Result<Vec<MouseEvent>, String> {
    state.get_mouse_events().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_mouse_tracking_stats(state: State<'_, Arc<UnifiedAppState>>) -> Result<MouseTrackingStats, String> {
    state.get_mouse_tracking_stats().await.map_err(|e| e.to_string())
}
