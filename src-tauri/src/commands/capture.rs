//! Capture configuration commands

use crate::state::UnifiedAppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

/// Capture mode enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    Desktop,
    Window,
    Device,
}

#[tauri::command]
pub async fn capture_set_mode(
    mode: CaptureMode,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state
        .set_capture_mode(mode)
        .await
        .map_err(|e| e.to_string())
}

/// On Wayland, window identities are private until the user approves one in
/// the desktop portal. Open that chooser now so recording can start from the
/// already-approved source later.
#[tauri::command]
pub async fn capture_prepare_window() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        crate::recording::linux::prepare_window_source()
            .await
            .map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "linux"))]
    Ok(())
}

#[tauri::command]
pub async fn capture_select_display(
    id: String,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state.select_display(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn capture_select_window(
    id: String,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state.select_window(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn capture_select_area(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state
        .select_area(x.into(), y.into(), width.into(), height.into())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn capture_select_device(
    id: String,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state.select_device(id).await.map_err(|e| e.to_string())
}
