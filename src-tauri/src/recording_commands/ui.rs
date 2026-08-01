use anyhow::Result;
use tauri::{AppHandle, Manager};

/// Hide UI elements during recording
pub(super) async fn hide_ui_elements(app: &AppHandle) -> Result<(), String> {
    // Hide webcam preview FIRST so it doesn't appear in the screen recording
    if let Some(wc) = app.get_webview_window("webcam-preview") {
        wc.hide().map_err(|e| e.to_string())?;
        println!("Webcam preview hidden for recording");
    }

    // Hide preview windows
    if let Some(preview) = app.get_webview_window("display-preview") {
        preview.hide().map_err(|e| e.to_string())?;
    }

    // Hide capture bar
    if let Some(bar) = app.get_webview_window("capture-bar") {
        bar.hide().map_err(|e| e.to_string())?;
    }

    println!("UI elements hidden for recording");
    Ok(())
}

pub(super) async fn restore_ui_elements(app: &AppHandle) {
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.show();
        let _ = bar.set_focus();
    }

    if let Some(preview) = app.get_webview_window("display-preview") {
        let _ = preview.hide();
    }

    if let Some(wc) = app.get_webview_window("webcam-preview") {
        let _ = wc.show();
    }
}
