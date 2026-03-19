//! Display preview overlay commands

#![allow(unexpected_cfgs)]

use std::sync::Arc;
use tauri::{AppHandle, State, Position, Size, Manager};
use crate::state::UnifiedAppState;

#[cfg(target_os = "macos")]
use core_graphics::geometry::CGRect;

#[cfg(target_os = "macos")]
pub fn ns_screen_for_cg_display_id(target: u32) -> Option<(cocoa::base::id, CGRect)> {
    use cocoa::{appkit::NSScreen, base::{id, nil}};
    use cocoa::foundation::NSString;
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let screens: id = NSScreen::screens(nil);
        let count: u64 = msg_send![screens, count];
        for i in 0..count {
            let s: id = msg_send![screens, objectAtIndex: i];
            let desc: id = msg_send![s, deviceDescription];
            let key = NSString::alloc(nil).init_str("NSScreenNumber");
            let num: id = msg_send![desc, objectForKey: key];
            let cg_id: u32 = msg_send![num, unsignedIntValue];
            if cg_id == target {
                let frame: CGRect = msg_send![s, frame];
                return Some((s, frame));
            }
        }
        None
    }
}

#[tauri::command]
pub async fn show_display_preview(display_id: String, app: AppHandle, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    println!("show_display_preview called with display_id: {}", display_id);

    let displays = state.get_displays().await.map_err(|e| e.to_string())?;
    println!("Available displays: {:?}", displays.iter().map(|d| &d.id).collect::<Vec<_>>());

    let display = displays.iter()
        .find(|d| d.id == display_id)
        .ok_or_else(|| format!("Display with id {} not found", display_id))?;

    let preview_url = format!(
        "preview.html?name={}&width={}&height={}&fps={}",
        urlencoding::encode(&display.name),
        display.width,
        display.height,
        display.refresh_rate
    );

    // Close any existing preview window
    if let Some(existing_window) = app.get_webview_window("display-preview") {
        println!("Closing existing preview window");
        let _ = existing_window.close();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Get screen dimensions
    #[cfg(target_os = "macos")]
    let (logical_pos, logical_size) = {
        match ns_screen_for_cg_display_id(display.cg_display_id) {
            Some((_screen_ptr, frame)) => {
                if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
                    return Err(format!("Invalid screen dimensions for display {}: {}x{}",
                                     display_id, frame.size.width, frame.size.height));
                }

                let pos = tauri::LogicalPosition::new(frame.origin.x, frame.origin.y);
                let size = tauri::LogicalSize::new(frame.size.width, frame.size.height);

                println!("Found NSScreen for display {} (CGDirectDisplayID {}): frame origin({}, {}), size({} x {})",
                         display_id, display.cg_display_id, frame.origin.x, frame.origin.y, frame.size.width, frame.size.height);

                (pos, size)
            }
            None => {
                println!("Could not find NSScreen for display {}, falling back to primary monitor", display_id);
                let monitor = app.primary_monitor()
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "No primary monitor found".to_string())?;

                let scale_factor = monitor.scale_factor();
                let physical_size = monitor.size();
                let physical_pos = monitor.position();

                let logical_size: tauri::LogicalSize<f64> = physical_size.to_logical(scale_factor);
                let logical_pos: tauri::LogicalPosition<f64> = physical_pos.to_logical(scale_factor);

                (logical_pos, logical_size)
            }
        }
    };

    #[cfg(not(target_os = "macos"))]
    let (logical_pos, logical_size) = {
        let monitor = app.primary_monitor()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "No primary monitor found".to_string())?;

        let scale_factor = monitor.scale_factor();
        let physical_size = monitor.size();
        let physical_pos = monitor.position();

        let logical_size: tauri::LogicalSize<f64> = physical_size.to_logical(scale_factor);
        let logical_pos: tauri::LogicalPosition<f64> = physical_pos.to_logical(scale_factor);

        println!("Monitor info - Scale: {}, Physical: {:?}@{:?}, Logical: {:?}@{:?}",
                 scale_factor, physical_size, physical_pos, logical_size, logical_pos);

        (logical_pos, logical_size)
    };

    println!("Using full screen rect - Pos: {:?}, Size: {:?}", logical_pos, logical_size);

    println!("Creating new preview window");
    let preview = tauri::WebviewWindowBuilder::new(
        &app,
        "display-preview",
        tauri::WebviewUrl::App(preview_url.into())
    )
    .title("Display Preview")
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .visible_on_all_workspaces(true)
    .build()
    .map_err(|e| e.to_string())?;

    preview.set_size(Size::Logical(logical_size)).map_err(|e| e.to_string())?;
    preview.set_position(Position::Logical(logical_pos)).map_err(|e| e.to_string())?;
    preview.set_ignore_cursor_events(true).map_err(|e| e.to_string())?;
    preview.set_always_on_top(true).map_err(|e| e.to_string())?;
    preview.show().map_err(|e| e.to_string())?;
    preview.set_focus().ok();

    println!("Preview window positioned and shown at {:?} with size {:?}", logical_pos, logical_size);

    // Ensure capture bar stays on top
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.set_always_on_top(true);
        let _ = bar.set_focus();
    }

    // Auto-close after 5 seconds
    let preview_clone = preview.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        println!("Auto-closing preview window after timeout");
        let _ = preview_clone.close();
    });

    Ok(())
}

#[tauri::command]
pub async fn hide_display_preview(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("display-preview") {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
