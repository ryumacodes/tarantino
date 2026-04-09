//! Recording processing and editor commands
//!
//! Handles background processing of recordings, post-processing effects,
//! and launching the editor window.

use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use tauri::{Emitter, Manager};

use crate::auto_zoom::ZoomProcessor;
use crate::event_capture::{CaptureSession, EnhancedMouseEvent};
use crate::state::UnifiedAppState;

/// Open the editor window with the given media file
pub async fn open_editor(
    app: &tauri::AppHandle,
    media_path: &str,
    sidecar_path: &str,
    has_webcam: bool,
    has_mic: bool,
    has_system_audio: bool,
) -> Result<(), String> {
    use tauri::{LogicalPosition, LogicalSize, Position, Size, WebviewUrl, WebviewWindowBuilder};

    if app.get_webview_window("editor").is_some() {
        println!("Editor already exists");
        return Ok(());
    }

    let _ = crate::commands::tray::reset_tray_to_idle_impl(app.clone()).await;

    if let Some(hud) = app.get_webview_window("recording-hud") {
        let _ = hud.close();
    }
    if let Some(ov) = app.get_webview_window("display-preview") {
        let _ = ov.close();
    }
    if let Some(wc) = app.get_webview_window("webcam-preview") {
        let _ = wc.close();
    }
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.hide();
    }

    let url = format!(
        "editor.html?media={}&sidecar={}&webcam={}&mic={}&system_audio={}",
        urlencoding::encode(media_path),
        urlencoding::encode(sidecar_path),
        has_webcam,
        has_mic,
        has_system_audio
    );

    let builder = WebviewWindowBuilder::new(app, "editor", WebviewUrl::App(url.into()))
        .title("Tarantino — Editor")
        .decorations(true)
        .transparent(false)
        .resizable(true)
        .always_on_top(false)
        .skip_taskbar(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let win = builder.build()
        .map_err(|e| format!("Failed to create editor: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        win.set_traffic_lights_inset(16.0, 20.0).ok();
    }

    #[cfg(debug_assertions)]
    {
        win.open_devtools();
    }

    win.set_size(Size::Logical(LogicalSize::new(1280.0, 840.0)))
        .ok();
    let _ = win.set_min_size(Some(Size::Logical(LogicalSize::new(1100.0, 720.0))));
    win.set_position(Position::Logical(LogicalPosition::new(120.0, 80.0)))
        .ok();
    win.show().ok();
    win.set_focus().ok();

    Ok(())
}

/// Open the editor with a loading state while processing continues
pub async fn open_editor_with_loading(app: &tauri::AppHandle, temp_path: &str, has_webcam: bool) -> Result<()> {
    if app.get_webview_window("editor").is_some() {
        return Ok(());
    }

    let url = format!(
        "editor.html?loading=true&temp_path={}&webcam={}",
        urlencoding::encode(temp_path),
        has_webcam
    );

    if let Some(preview) = app.get_webview_window("display-preview") {
        let _ = preview.close();
    }
    if let Some(wc) = app.get_webview_window("webcam-preview") {
        let _ = wc.close();
    }
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.hide();
    }

    let builder = tauri::WebviewWindowBuilder::new(
        app,
        "editor",
        tauri::WebviewUrl::App(url.parse().unwrap()),
    )
    .title("Tarantino — Editor")
    .decorations(true)
    .inner_size(1400.0, 900.0)
    .center();
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let _editor = builder.build()?;

    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        _editor.set_traffic_lights_inset(16.0, 20.0).ok();
    }

    update_editor_status(app, "Processing recording...").await?;
    Ok(())
}

/// Update the editor status message
pub async fn update_editor_status(app: &tauri::AppHandle, status: &str) -> Result<()> {
    if let Some(editor) = app.get_webview_window("editor") {
        editor
            .emit("processing-status", status)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    Ok(())
}

/// Notify the editor that the recording is ready
pub async fn notify_editor_ready(app: &tauri::AppHandle, final_path: &str) -> Result<()> {
    if let Some(editor) = app.get_webview_window("editor") {
        editor
            .emit("recording-ready", final_path)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    Ok(())
}

/// Spawn background processing for a recording
///
/// NOTE: `recording_start_time` must be captured BEFORE calling `stop_mouse_tracking()`,
/// because `stop_mouse_tracking()` resets the start time to None. This fixes the race
/// condition where all click events would appear at time=0ms.
pub fn spawn_background_recording_processing(
    app: tauri::AppHandle,
    state: Arc<UnifiedAppState>,
    temp_path: String,
    recording_start_time: Option<SystemTime>,
) {
    println!("🎬 [BG_PROCESS] Spawning background recording processing");
    println!("🎬 [BG_PROCESS] Temp path: {}", temp_path);
    println!("🎬 [BG_PROCESS] Recording start time (passed): {:?}", recording_start_time);

    tokio::spawn(async move {
        let _ = update_editor_status(&app, "Finalizing recording...").await;

        match state.wait_for_recording_completion().await {
            Ok(_path) => {
                let _ = update_editor_status(&app, "Processing mouse events...").await;

                println!("🖱️ [BG_PROCESS] Retrieving mouse events from tracker...");
                let mouse_events = {
                    let tracker = state.get_mouse_tracker();
                    let guard = tracker.lock();
                    let events = guard.get_events();
                    println!("🖱️ [BG_PROCESS] Tracker state:");
                    println!("   - is_tracking: {}", guard.is_tracking);
                    println!("   - events count: {}", events.len());
                    println!("   - recording_start_time (from tracker, expected None): {:?}", guard.recording_start_time);
                    println!("   - recording_start_time (passed, should be Some): {:?}", recording_start_time);

                    // Log event type breakdown
                    let clicks = events.iter().filter(|e| matches!(e.event_type, crate::mouse_tracking::MouseEventType::ButtonPress { .. })).count();
                    let moves = events.iter().filter(|e| matches!(e.event_type, crate::mouse_tracking::MouseEventType::Move)).count();
                    println!("   - clicks (ButtonPress): {}", clicks);
                    println!("   - moves: {}", moves);

                    events
                };

                // Use the passed recording_start_time instead of reading from tracker
                // (tracker's recording_start_time is already None at this point)
                let start_time = recording_start_time;

                {
                    println!("🖱️ [BG_PROCESS] Stopping mouse tracking...");
                    state.get_mouse_tracker().lock().stop_tracking();
                }

                let _ = update_editor_status(&app, "Processing video file...").await;
                println!("📹 [BG_PROCESS] Processing video file with {} mouse events", mouse_events.len());
                println!("📹 [BG_PROCESS] Using start_time: {:?}", start_time);
                let final_path = process_recorded_file(&temp_path, mouse_events, start_time)
                    .await
                    .unwrap_or(temp_path.clone());

                let _ = update_editor_status(&app, "Applying effects...").await;
                let processed_path = apply_post_processing(&final_path)
                    .await
                    .unwrap_or(final_path);

                let processed_path_buf = std::path::Path::new(&processed_path);
                if crate::commands::video_validation::wait_for_file_ready(
                    processed_path_buf,
                    tokio::time::Duration::from_secs(10),
                )
                .await
                {
                    let _ = update_editor_status(&app, "Ready!").await;
                    let _ = notify_editor_ready(&app, &processed_path).await;
                } else {
                    let _ =
                        update_editor_status(&app, "Error: Recording failed to finalize").await;
                }

                let _ = app.emit("recording-stopped", processed_path);
            }
            Err(e) => {
                let _ = update_editor_status(&app, &format!("Error: {}", e)).await;
            }
        }
    });
}

/// Process a recorded file: move to permanent location and generate auto-zoom data
pub async fn process_recorded_file(
    temp_path: &str,
    mouse_events: Vec<crate::mouse_tracking::MouseEvent>,
    start_time: Option<std::time::SystemTime>,
) -> Result<String> {
    println!("📁 [PROCESS] Starting process_recorded_file");
    println!("📁 [PROCESS] Temp path: {}", temp_path);
    println!("📁 [PROCESS] Mouse events received: {}", mouse_events.len());

    // Count click events specifically
    let click_count = mouse_events.iter().filter(|e| {
        matches!(e.event_type, crate::mouse_tracking::MouseEventType::ButtonPress { .. })
    }).count();
    println!("🖱️ [PROCESS] Click events (ButtonPress): {}", click_count);

    if mouse_events.is_empty() {
        println!("⚠️ [PROCESS] WARNING: No mouse events received! Mouse tracking may not be working.");
    }

    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let movies_dir = std::path::PathBuf::from(&home_dir)
        .join("Movies")
        .join("Tarantino");
    std::fs::create_dir_all(&movies_dir)?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let final_path = movies_dir.join(format!("recording_{}.mp4", timestamp));

    let mut media_path = temp_path.to_string();
    let mut file_found = false;

    for attempt in 1..=10 {
        if std::path::Path::new(&temp_path).exists() {
            if let Ok(metadata) = std::fs::metadata(&temp_path) {
                if metadata.len() > 1024 {
                    if let Ok(true) =
                        crate::commands::video_validation::validate_video_file(&temp_path)
                    {
                        file_found = true;
                        println!("✅ [PROCESS] Video file validated on attempt {}", attempt);
                        break;
                    }
                }
            }
        }
        if attempt < 10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    if file_found {
        if std::fs::rename(&temp_path, &final_path).is_ok() {
            media_path = final_path.to_str().unwrap_or("").to_string();
            println!("📁 [PROCESS] Moved video to: {}", media_path);

            // Also move sidecar files (mouse.json and auto_zoom.json) that were saved by generate_zoom_analysis
            let temp_base = temp_path.trim_end_matches(".mp4");
            let final_base = media_path.trim_end_matches(".mp4");

            // Move mouse.json
            let temp_mouse = format!("{}.mouse.json", temp_base);
            let final_mouse = format!("{}.mouse.json", final_base);
            if std::path::Path::new(&temp_mouse).exists() {
                if let Err(e) = std::fs::rename(&temp_mouse, &final_mouse) {
                    println!("⚠️ [PROCESS] Failed to move mouse.json: {}", e);
                } else {
                    println!("📁 [PROCESS] Moved mouse.json to: {}", final_mouse);
                }
            }

            // Move auto_zoom.json
            let temp_zoom = format!("{}.auto_zoom.json", temp_base);
            let final_zoom = format!("{}.auto_zoom.json", final_base);
            if std::path::Path::new(&temp_zoom).exists() {
                if let Err(e) = std::fs::rename(&temp_zoom, &final_zoom) {
                    println!("⚠️ [PROCESS] Failed to move auto_zoom.json: {}", e);
                } else {
                    println!("📁 [PROCESS] Moved auto_zoom.json to: {}", final_zoom);
                }
            }

            // Move webcam.mp4 (native AVFoundation capture)
            let temp_webcam = format!("{}.webcam.mp4", temp_base);
            let final_webcam = format!("{}.webcam.mp4", final_base);
            if std::path::Path::new(&temp_webcam).exists() {
                if let Err(e) = std::fs::rename(&temp_webcam, &final_webcam) {
                    println!("⚠️ [PROCESS] Failed to move webcam.mp4: {}", e);
                } else {
                    println!("📁 [PROCESS] Moved webcam.mp4 to: {}", final_webcam);
                }
            }
        }
    }

    // Calculate start time for timestamp normalization
    let start_ms = start_time
        .unwrap_or_else(std::time::SystemTime::now)
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    println!("⏱️ [PROCESS] Recording start time (ms since epoch): {}", start_ms);

    // Convert to enhanced events with normalized timestamps
    let enhanced_events: Vec<EnhancedMouseEvent> = mouse_events
        .iter()
        .map(|e| {
            let mut normalized = e.clone();
            normalized.timestamp = e.timestamp.saturating_sub(start_ms);
            EnhancedMouseEvent {
                base: normalized,
                window_id: None,
                app_name: None,
                is_double_click: false,
                cluster_id: None,
            }
        })
        .collect();

    println!("🔄 [PROCESS] Enhanced events created: {}", enhanced_events.len());

    // Note: mouse.json is already saved by generate_zoom_analysis() with correct display dimensions
    // We don't save it here to avoid overwriting with hardcoded values

    // Auto-zoom generation
    let auto_zoom_path = format!("{}.auto_zoom.json", media_path.trim_end_matches(".mp4"));
    println!("🔍 [PROCESS] Auto-zoom path: {}", auto_zoom_path);

    if !std::path::Path::new(&auto_zoom_path).exists() {
        println!("🔍 [PROCESS] Generating auto-zoom analysis...");

        let mut session = CaptureSession::new();
        // Events are already normalized to relative time (0 = recording start)
        // So we must set start_time to 0 to match
        session.start_time = 0;
        session.mouse_events = enhanced_events.clone();

        // Set end_time from last event or fallback - required for zoom block duration calculation
        if let Some(last_event) = session.mouse_events.last() {
            session.end_time = Some(last_event.base.timestamp + 1000); // Add 1s buffer
        } else {
            session.end_time = Some(30000); // Default 30s fallback
        }

        println!("🔍 [PROCESS] Session created with {} mouse events, start_time: {}, end_time: {:?}",
            session.mouse_events.len(), session.start_time, session.end_time);

        let processor = ZoomProcessor::with_default_config();
        match processor.analyze_session(&session, &[]) {
            Ok(analysis) => {
                println!("✅ [PROCESS] Zoom analysis complete:");
                println!("   - Total clicks processed: {}", analysis.total_clicks);
                println!("   - Zoom blocks generated: {}", analysis.zoom_blocks.len());
                println!("   - Session duration: {}ms", analysis.session_duration);

                for (i, block) in analysis.zoom_blocks.iter().enumerate() {
                    println!("   - Block {}: {}ms-{}ms, {:.1}x zoom at ({:.2}, {:.2})",
                        i, block.start_time, block.end_time, block.zoom_factor, block.center_x, block.center_y);
                }

                match crate::auto_zoom::save_analysis(&analysis, &auto_zoom_path) {
                    Ok(_) => println!("✅ [PROCESS] Auto-zoom saved to: {}", auto_zoom_path),
                    Err(e) => println!("❌ [PROCESS] Failed to save auto-zoom: {}", e),
                }
            }
            Err(e) => {
                println!("❌ [PROCESS] Zoom analysis failed: {}", e);
            }
        }
    } else {
        println!("ℹ️ [PROCESS] Auto-zoom file already exists, skipping generation");
    }

    println!("✅ [PROCESS] Processing complete. Final path: {}", media_path);
    Ok(media_path)
}

/// Apply post-processing effects to a video
pub async fn apply_post_processing(video_path: &str) -> Result<String> {
    let processor = crate::post_processing::create_default_processor();
    let zoom_analysis_path = format!("{}.auto_zoom.json", video_path.trim_end_matches(".mp4"));
    let zoom_analysis = if std::path::Path::new(&zoom_analysis_path).exists() {
        crate::auto_zoom::load_analysis(&zoom_analysis_path).ok()
    } else {
        None
    };

    let input_path = std::path::Path::new(video_path);
    let output_path = input_path.with_file_name(format!(
        "processed_{}",
        input_path.file_name().unwrap().to_string_lossy()
    ));
    processor
        .process_video(input_path, &output_path, zoom_analysis.as_ref())
        .await?;

    // Copy sidecar files (auto_zoom.json and mouse.json) to processed_ path
    let original_zoom_path = format!("{}.auto_zoom.json", video_path.trim_end_matches(".mp4"));
    let processed_zoom_path = format!("{}.auto_zoom.json", output_path.to_string_lossy().trim_end_matches(".mp4"));

    if std::path::Path::new(&original_zoom_path).exists() {
        if let Err(e) = std::fs::copy(&original_zoom_path, &processed_zoom_path) {
            println!("⚠️ [POST_PROCESS] Failed to copy auto_zoom.json: {}", e);
        } else {
            println!("✅ [POST_PROCESS] Copied auto_zoom.json to: {}", processed_zoom_path);
        }
    }

    let original_mouse_path = format!("{}.mouse.json", video_path.trim_end_matches(".mp4"));
    let processed_mouse_path = format!("{}.mouse.json", output_path.to_string_lossy().trim_end_matches(".mp4"));

    if std::path::Path::new(&original_mouse_path).exists() {
        if let Err(e) = std::fs::copy(&original_mouse_path, &processed_mouse_path) {
            println!("⚠️ [POST_PROCESS] Failed to copy mouse.json: {}", e);
        } else {
            println!("✅ [POST_PROCESS] Copied mouse.json to: {}", processed_mouse_path);
        }
    }

    Ok(output_path.to_string_lossy().to_string())
}
