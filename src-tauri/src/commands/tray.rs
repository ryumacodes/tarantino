//! Tray menu management commands

use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};

/// Holds the recording status menu item so we can update its text without rebuilding the menu.
static RECORDING_STATUS_ITEM: Mutex<Option<MenuItem<tauri::Wry>>> = Mutex::new(None);

/// Create idle state tray menu
pub fn create_idle_tray_menu(
    app: &tauri::AppHandle,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("new_recording", "New Recording…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("record_display", "Record Display…").build(app)?)
        .item(&MenuItemBuilder::with_id("record_window", "Record Window…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("capture_settings", "Settings…").build(app)?)
        .item(&MenuItemBuilder::with_id("capture_shortcuts", "Keyboard Shortcuts…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("open_projects", "Open Project…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Tarantino").build(app)?)
        .build()?;
    Ok(menu)
}

/// Create recording state tray menu with elapsed time
pub fn create_recording_tray_menu_with_time(
    app: &tauri::AppHandle,
    elapsed: Option<&str>,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    crate::app_menu::set_recording(app, true);
    let status_text = match elapsed {
        Some(t) => format!("🔴 Recording — {}", t),
        None => "🔴 Recording".to_string(),
    };

    let status_item = MenuItemBuilder::with_id("recording_status", &status_text)
        .enabled(false)
        .build(app)?;

    // Store the item so we can update its text later
    *RECORDING_STATUS_ITEM.lock().unwrap() = Some(status_item.clone());

    let menu = MenuBuilder::new(app)
        .item(&status_item)
        .separator()
        .item(&MenuItemBuilder::with_id("restart_recording", "Restart Recording").build(app)?)
        .item(&MenuItemBuilder::with_id("stop_recording", "Stop Recording").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Tarantino").build(app)?)
        .build()?;

    Ok(menu)
}

/// Update tray with recording timer
pub async fn update_main_tray_timer_cmd(
    app: tauri::AppHandle,
    elapsed_time: String,
) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        let status_text = format!("🔴 Recording — {}", elapsed_time);

        // Try to update the existing menu item text in-place (no menu rebuild, no flicker)
        let updated = {
            let guard = RECORDING_STATUS_ITEM.lock().unwrap();
            if let Some(ref item) = *guard {
                item.set_text(&status_text).is_ok()
            } else {
                false
            }
        };

        if !updated {
            // First tick — build and set the recording menu
            let menu = create_recording_tray_menu_with_time(&app, Some(&elapsed_time))
                .map_err(|e| format!("Failed to create recording menu: {}", e))?;
            tray.set_menu(Some(menu))
                .map_err(|e| format!("Failed to set recording menu: {}", e))?;
        }

        #[cfg(target_os = "macos")]
        tray.set_title(Some(elapsed_time.strip_prefix("00:").unwrap_or(&elapsed_time)))
            .map_err(|error| format!("Failed to update menu-bar timer: {error}"))?;
        let tooltip_text = format!("Recording: {}", elapsed_time);
        let _ = tray.set_tooltip(Some(&tooltip_text));
    } else {
        return Err("No main tray found to update".to_string());
    }

    Ok(())
}

/// Reset tray to idle state (helper function - actual command in main.rs)
pub async fn reset_tray_to_idle_impl(app: tauri::AppHandle) -> Result<(), String> {
    crate::app_menu::set_recording(&app, false);
    // Clear the stored recording menu item
    *RECORDING_STATUS_ITEM.lock().unwrap() = None;

    if let Some(tray) = app.tray_by_id("main") {
        #[cfg(target_os = "macos")]
        tray.set_title(None::<&str>).map_err(|error| error.to_string())?;
        // Reset tooltip
        let _ = tray.set_tooltip(Some("Tarantino - Ready to Record"));

        // Set idle menu
        let menu = create_idle_tray_menu(&app)
            .map_err(|e| format!("Failed to create idle tray menu: {}", e))?;

        tray.set_menu(Some(menu))
            .map_err(|e| format!("Failed to set idle menu: {}", e))?;

        Ok(())
    } else {
        Err("Failed to get main tray".to_string())
    }
}

/// Only actions backed by a capture-bar workflow are exposed in the idle menu.
pub fn capture_action_for_menu(id: &str) -> Option<&'static str> {
    match id {
        "new_recording" => Some("new"),
        "record_display" => Some("display"),
        "record_window" => Some("window"),
        "capture_settings" => Some("settings"),
        "capture_shortcuts" => Some("shortcuts"),
        _ => None,
    }
}

pub fn dispatch_capture_action(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    use tauri::Emitter;
    let action = capture_action_for_menu(id).ok_or("Unknown capture menu action")?;
    super::misc::bring_capture_bar_to_front(app)?;
    app.emit_to("capture-bar", "capture-menu-action", action)
        .map_err(|error| error.to_string())
}
