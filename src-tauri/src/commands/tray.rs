//! Tray menu management commands

use tauri::menu::{MenuBuilder, MenuItemBuilder, MenuItem};
use std::sync::Mutex;

/// Holds the recording status menu item so we can update its text without rebuilding the menu.
static RECORDING_STATUS_ITEM: Mutex<Option<MenuItem<tauri::Wry>>> = Mutex::new(None);

/// Create idle state tray menu
pub fn create_idle_tray_menu(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("status", "Ready to Record").enabled(false).build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Tarantino").build(app)?)
        .build()?;
    Ok(menu)
}

/// Create recording state tray menu with elapsed time
pub fn create_recording_tray_menu_with_time(app: &tauri::AppHandle, elapsed: Option<&str>) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
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
        .item(&MenuItemBuilder::with_id("stop_recording", "Stop Recording").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Tarantino").build(app)?)
        .build()?;

    Ok(menu)
}

/// Update tray with recording timer
pub async fn update_main_tray_timer_cmd(app: tauri::AppHandle, elapsed_time: String) -> Result<(), String> {
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

        let tooltip_text = format!("Recording: {}", elapsed_time);
        let _ = tray.set_tooltip(Some(&tooltip_text));
    } else {
        return Err("No main tray found to update".to_string());
    }

    Ok(())
}

/// Reset tray to idle state (helper function - actual command in main.rs)
pub async fn reset_tray_to_idle_impl(app: tauri::AppHandle) -> Result<(), String> {
    // Clear the stored recording menu item
    *RECORDING_STATUS_ITEM.lock().unwrap() = None;

    if let Some(tray) = app.tray_by_id("main") {
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

