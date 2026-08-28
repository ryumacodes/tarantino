//! Miscellaneous commands

use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::state::UnifiedAppState;

/// Restore the capture controls with the same foreground guarantees used at
/// startup. KDE can discard the keep-above state while a window is hidden.
pub fn bring_capture_bar_to_front(app: &AppHandle) -> Result<(), String> {
    let Some(bar) = app.get_webview_window("capture-bar") else {
        return Err("Capture bar window not found".to_string());
    };

    bar.unminimize().map_err(|e| e.to_string())?;
    bar.show().map_err(|e| e.to_string())?;
    // Apply this after mapping as well as in the static window configuration;
    // X11/KWin may recreate the window-manager state after hide/show.
    bar.set_always_on_top(true).map_err(|e| e.to_string())?;
    bar.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn log_to_terminal(message: String, level: Option<String>) {
    let prefix = match level.as_deref() {
        Some("error") => "[FRONTEND ERROR]",
        Some("warn") => "[FRONTEND WARN]",
        Some("info") => "[FRONTEND INFO]",
        _ => "[FRONTEND LOG]",
    };
    println!("{} {}", prefix, message);
}

#[tauri::command]
pub async fn show_capture_bar(app: AppHandle) -> Result<(), String> {
    bring_capture_bar_to_front(&app)?;
    println!("Capture bar shown in front and focused");
    Ok(())
}

#[tauri::command]
pub async fn reset_tray_to_idle(app: AppHandle) -> Result<(), String> {
    crate::commands::tray::reset_tray_to_idle_impl(app).await
}

#[tauri::command]
pub async fn exit(app: AppHandle, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    crate::commands::lifecycle::release_recording_surfaces(&app, Some(state.inner()), "exit");
    state.shutdown().await.map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
