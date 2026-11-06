//! Application-wide project navigation, available even when only capture UI is open.
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, Submenu},
};

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::default(app)?;
    let open = MenuItem::with_id(
        app,
        "file_open_projects",
        "Open Project…",
        true,
        Some("CmdOrCtrl+O"),
    )?;
    let save = MenuItem::with_id(app, "file_save_project", "Save Project", true, Some("CmdOrCtrl+S"))?;
    let stop = MenuItem::with_id(app, "file_stop_recording", "Stop Recording", false, None::<&str>)?;
    let file = menu.items()?.into_iter().find_map(|item| {
        item.as_submenu()
            .filter(|submenu| submenu.text().ok().as_deref() == Some("File"))
            .cloned()
    });
    if let Some(file) = file {
        file.prepend(&open)?;
        file.insert(&save, 1)?;
        file.append(&stop)?;
    } else {
        menu.prepend(&Submenu::with_items(app, "File", true, &[&open, &save, &stop])?)?;
    }
    // Native undo: targets WebKit's text history, not the editor document.
    // Route these accelerators to the focused webview; it retains text undo.
    for item in menu.items()? {
        let Some(edit) = item.as_submenu() else { continue; };
        if edit.text()?.as_str() != "Edit" { continue; }
        for (index, item) in edit.items()?.into_iter().enumerate() {
            let Some(predefined) = item.as_predefined_menuitem() else { continue; };
            let (id, label, accelerator) = match predefined.text()?.as_str() {
                "Undo" => ("editor_undo", "Undo", "CmdOrCtrl+Z"),
                "Redo" => ("editor_redo", "Redo", "CmdOrCtrl+Shift+Z"),
                _ => continue,
            };
            edit.remove_at(index)?;
            edit.insert(&MenuItem::with_id(app, id, label, true, Some(accelerator))?, index)?;
        }
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id.as_ref() == "file_stop_recording" {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<std::sync::Arc<crate::state::UnifiedAppState>>();
                if !state.recording.is_recording() { return; }
                if let Err(error) = crate::commands::record_stop_instant(app.clone(), state).await {
                    eprintln!("Could not stop recording: {error}");
                }
            });
        }
        if let Some(action) = match event.id.as_ref() {
            "editor_undo" => Some("undo"), "editor_redo" => Some("redo"), _ => None,
        } {
            if let Some(window) = app.webview_windows().values().find(|w| w.is_focused().unwrap_or(false)) {
                let script = format!(
                    "if (window.dispatchEvent(new CustomEvent('tarantino-history', {{detail:'{action}',cancelable:true}}))) document.execCommand('{action}');"
                );
                if let Err(error) = window.eval(script) { eprintln!("Could not dispatch history action: {error}"); }
            }
        }
        if event.id.as_ref() == "file_save_project" {
            if let Some(window) = app.webview_windows().values().find(|w| w.is_focused().unwrap_or(false)) {
                if let Err(error) = window.eval("window.dispatchEvent(new Event('tarantino-save'));") {
                    eprintln!("Could not dispatch save action: {error}");
                }
            }
        }
        if event.id.as_ref() == "file_open_projects" {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = crate::commands::projects::open_project_library(app).await {
                    eprintln!("Could not open projects: {error}");
                }
            });
        }
    });
    Ok(())
}

/// Keep the native fallback available even when the capture window is hidden.
pub fn set_recording(app: &AppHandle, recording: bool) {
    let item = app.menu().and_then(|menu| menu.items().ok()).and_then(|items| {
        items.into_iter().find_map(|item| {
            item.as_submenu().and_then(|menu| menu.get("file_stop_recording"))
        })
    });
    if let Some(item) = item {
        if let Some(item) = item.as_menuitem() {
            if let Err(error) = item.set_enabled(recording) {
                eprintln!("Could not update recording menu: {error}");
            }
        }
    }
}
