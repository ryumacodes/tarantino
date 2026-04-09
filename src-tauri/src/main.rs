#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auto_zoom;
mod capture;
mod capture_commands;
mod commands;
mod cursor_engine;
mod cursor_renderer;
mod encoder;
mod event_capture;
mod export;
mod ffmpeg;
mod ffmpeg_manager;
mod mouse_tracking;
mod muxer;
mod permissions;
mod post_processing;
mod preview;
mod recording;
mod recording_commands;
mod sidecar;
mod state;
mod video_processing;
#[cfg(target_os = "macos")]
mod webcam;
mod zoom_preview;

use anyhow::Result;
use state::UnifiedAppState;
use std::sync::Arc;
use tauri::{
    tray::TrayIconBuilder,
    Manager,
};

// Re-export CaptureMode from commands::capture for external use
pub use commands::capture::CaptureMode;

//=============================================================================
// System Tray Setup
//=============================================================================

fn setup_tray(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = commands::create_idle_tray_menu(app)?;
    let app_handle = app.clone();

    println!("Setting up system tray with event handlers...");
    let _tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Tarantino - Ready to Record")
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            println!("Tray menu event: {:?}", event.id);
            match event.id.as_ref() {
                "stop_recording" => {
                    println!("=== STOP RECORDING CLICKED ===");
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_clone.state::<Arc<UnifiedAppState>>();
                        match commands::record_stop_instant(app_clone.clone(), state).await {
                            Ok(temp_path) => println!("Instant stop signaled: {}", temp_path),
                            Err(e) => {
                                println!("Instant stop failed: {}", e);
                                let _ = commands::reset_tray_to_idle(app_clone).await;
                            }
                        }
                    });
                }
                "quit" => {
                    println!("Quit clicked from tray");
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        // Yield so the menu event handler finishes before tearing down the app
                        tokio::task::yield_now().await;
                        handle.exit(0);
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|_tray, event| {
            println!("Tray icon event: {:?}", event);
        })
        .build(app)?;

    println!("System tray setup completed successfully");
    Ok(())
}

//=============================================================================
// Main Entry Point
//=============================================================================

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app_state = Arc::new(UnifiedAppState::new().expect("Failed to create app state"));
    if let Err(e) = app_state.initialize().await {
        eprintln!("Failed to initialize app state: {}", e);
        std::process::exit(1);
    }

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Capture configuration (from commands module)
            commands::capture::capture_set_mode,
            commands::capture::capture_select_display,
            commands::capture::capture_select_window,
            commands::capture::capture_select_area,
            commands::capture::capture_select_device,
            // Input configuration (from commands module)
            commands::input::input_set_camera,
            commands::input::input_set_mic,
            commands::input::input_set_system_audio,
            commands::input::webcam_set_transform,
            commands::input::webcam_set_autododge,
            commands::input::save_webcam_recording,
            // Recording control (from commands module)
            commands::recording_control::record_start,
            commands::recording_control::record_pause,
            commands::recording_control::record_resume,
            commands::recording_control::record_stop,
            commands::recording_control::record_stop_instant,
            commands::recording_control::hud_stop,
            // Legacy recording commands
            recording_commands::record_start_new,
            recording_commands::record_pause_new,
            recording_commands::record_resume_new,
            recording_commands::record_stop_instant_new,
            recording_commands::get_recording_status,
            recording_commands::update_recording_duration,
            // Native capture API
            capture_commands::record_start_native,
            capture_commands::record_toggle_mic,
            capture_commands::record_toggle_system_audio,
            capture_commands::record_toggle_camera,
            capture_commands::record_add_break,
            capture_commands::record_stop_native,
            capture_commands::record_get_state,
            // Permission management
            permissions::check_permissions,
            permissions::request_accessibility_permission,
            permissions::open_accessibility_preferences,
            permissions::request_screen_recording_permission,
            permissions::open_screen_recording_preferences,
            permissions::diagnose_screen_capture,
            permissions::validate_recording_permissions,
            permissions::request_all_recording_permissions,
            // Preview zoom indicators
            zoom_preview::get_preview_zoom_indicators,
            zoom_preview::has_mouse_data_for_preview,
            // Editor commands
            capture_commands::project_open,
            capture_commands::editor_play,
            capture_commands::editor_pause,
            capture_commands::editor_seek,
            capture_commands::editor_add_zoom,
            capture_commands::editor_update_zoom,
            capture_commands::editor_delete_zoom,
            capture_commands::export_run,
            // Device enumeration (from commands module)
            commands::device::get_displays,
            commands::device::get_displays_with_thumbnails,
            commands::device::get_selected_display,
            commands::device::get_display_bounds,
            commands::device::get_windows,
            commands::device::get_devices,
            commands::device::get_audio_devices,
            // Display preview
            commands::display_preview::show_display_preview,
            commands::display_preview::hide_display_preview,
            // HUD
            commands::hud::hide_recording_hud,
            commands::hud::hud_move,
            commands::hud::hud_query_capture_region,
            // Mouse tracking
            commands::mouse::start_mouse_tracking,
            commands::mouse::stop_mouse_tracking,
            commands::mouse::get_mouse_events,
            commands::mouse::get_mouse_tracking_stats,
            // Video processing
            commands::video::get_video_info,
            commands::video::get_video_metadata,
            commands::video::extract_video_thumbnails,
            commands::video::export_video,
            commands::video::extract_audio_waveform,
            commands::video::read_sidecar_file,
            commands::video::compute_cursor_trajectory,
            // Auto-zoom
            commands::zoom::load_auto_zoom_data,
            commands::zoom::save_auto_zoom_data,
            // Cursor engine
            commands::cursor::cursor_process_event,
            commands::cursor::cursor_get_state,
            commands::cursor::cursor_reset_engine,
            commands::cursor::cursor_update_config,
            commands::cursor::cursor_get_metrics,
            // Export pipeline
            commands::export_pipeline::export_start_pipeline,
            commands::export_pipeline::export_get_progress,
            commands::export_pipeline::export_cancel,
            commands::export_pipeline::export_create_config,
            // Preview
            commands::preview::preview_load_project,
            commands::preview::preview_play,
            commands::preview::preview_pause,
            commands::preview::preview_seek,
            commands::preview::preview_set_speed,
            commands::preview::preview_get_frame,
            commands::preview::preview_update_options,
            // Misc
            commands::misc::log_to_terminal,
            commands::misc::reset_tray_to_idle,
            commands::misc::show_capture_bar,
            commands::misc::exit,
        ])
        .setup(|app| {
            if let Err(e) = setup_tray(app.handle()) {
                eprintln!("Failed to setup system tray: {}", e);
            }

            if let Some(capture_bar) = app.get_webview_window("capture-bar") {
                capture_bar.show().ok();
                capture_bar.set_focus().ok();
            }

            let state = app.state::<Arc<UnifiedAppState>>();
            let mouse_tracker = state.get_mouse_tracker();
            if let Err(e) = crate::mouse_tracking::create_mouse_listener(mouse_tracker) {
                eprintln!("Failed to create mouse listener: {}", e);
            }

            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("capture-bar") {
                    window.open_devtools();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
