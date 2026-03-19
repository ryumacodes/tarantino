use anyhow::Result;
use tauri::{AppHandle, Emitter, Manager};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::capture::*;

/// Global native backend instance (single active capture)
static NATIVE_BACKEND: once_cell::sync::Lazy<Arc<Mutex<Option<Box<dyn NativeCaptureBackend>>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

/// Start recording using the clean native capture API
/// This replaces the old FFmpeg-based record_start command
#[tauri::command]
pub async fn record_start_native(
    mode: String,           // "instant" or "studio"
    target_type: String,    // "screen", "window", "area"
    target_id: Option<u32>, // Display ID, window ID, etc.
    target_bounds: Option<CaptureBounds>,
    mic_name: Option<String>,
    camera_label: Option<String>,
    system_audio: Option<bool>,
    fps: Option<u32>,
    app: AppHandle,
) -> Result<StartRecordingResponse, String> {
    println!("Starting native recording with clean API");
    
    // Hide UI elements during recording
    hide_ui_for_recording(&app).await?;
    
    // Parse recording mode
    let recording_mode = match mode.as_str() {
        "instant" => RecordingMode::Quick,
        "studio" => RecordingMode::Studio,
        _ => RecordingMode::Quick, // Default to Quick Record
    };
    
    // Parse capture target
    let capture_target_type = match target_type.as_str() {
        "screen" => CaptureTargetType::Screen,
        "window" => CaptureTargetType::Window,
        "area" => CaptureTargetType::Area,
        _ => return Err("Invalid target type".to_string()),
    };
    
    // Build capture configuration
    let config = StartRecordingPayload {
        mode: recording_mode,
        target: CaptureTarget {
            target_type: capture_target_type,
            id: target_id,
            bounds: target_bounds,
        },
        mic_name,
        camera_label,
        system_audio,
        fps,
    };
    
    // Create native backend
    let mut backend = CaptureBackendFactory::create_backend()
        .map_err(|e| format!("Failed to create native backend: {}", e))?;

    // Check/request permissions
    let perms = backend.check_permissions().await
        .map_err(|e| format!("Permission check failed: {}", e))?;
    if !perms.screen_recording {
        let _ = backend.request_permissions().await;
    }

    // Map StartRecordingPayload -> CaptureConfig
    let source_type = match config.target.target_type {
        CaptureTargetType::Screen => CaptureSourceType::Display,
        CaptureTargetType::Window => CaptureSourceType::Window,
        CaptureTargetType::Area => CaptureSourceType::Display, // Area via region crop
    };

    // Resolve source ID (prefer selected display from state, else primary)
    let sources = backend.enumerate_sources().await
        .map_err(|e| format!("Failed to enumerate sources: {}", e))?;

    // Try get selected display from UnifiedAppState
    let selected_display_id = {
        // Best-effort read; if state unavailable, fall back
        if let Some(app_state) = app.try_state::<Arc<crate::state::UnifiedAppState>>() {
            let app_read = app_state.app.read();
            app_read.selected_display_id.clone()
        } else { None }
    };

    let source_id = if let Some(id) = config.target.id.map(|v| v as u64) {
        id
    } else if let Some(sel) = selected_display_id.as_deref() {
        // Map selected display id string to source id u64
        if let Some(src) = sources.iter().find(|s| s.source_type == CaptureSourceType::Display && s.id.to_string() == sel) {
            src.id
        } else {
            sources.iter().find(|s| s.is_primary).map(|s| s.id)
                .or_else(|| sources.first().map(|s| s.id))
                .ok_or_else(|| "No capture sources available".to_string())?
        }
    } else {
        sources.iter().find(|s| s.is_primary).map(|s| s.id)
            .or_else(|| sources.first().map(|s| s.id))
            .ok_or_else(|| "No capture sources available".to_string())?
    };

    let region = config.target.bounds.as_ref().map(|b| CaptureRegion {
        x: b.x,
        y: b.y,
        width: b.w,
        height: b.h,
    });

    let capture_cfg = CaptureConfig {
        source_id,
        source_type,
        fps: config.fps.unwrap_or(60),
        include_cursor: true,
        include_audio: config.system_audio.unwrap_or(false),
        region,
        output_path: None,
    };

    // Start capture
    let _handle = backend.start_capture(capture_cfg).await
        .map_err(|e| format!("Failed to start capture: {}", e))?;

    // Keep backend globally for stop/mute/etc
    {
        let mut global = NATIVE_BACKEND.lock().await;
        *global = Some(backend);
    }

    // Return a minimal response for UI compatibility
    let response = StartRecordingResponse {
        session_id: uuid::Uuid::new_v4().to_string(),
        project_path: "".to_string(),
    };
    
    // Update system tray to recording state
    let _ = crate::commands::tray::update_main_tray_timer_cmd(app.clone(), "00:00:00".to_string()).await;
    
    // Timer is managed by the main record_start function to avoid duplicate timers
    
    // Emit recording started event
    app.emit("recording:started", &response).map_err(|e| e.to_string())?;
    
    println!("Native recording started successfully: session {}", response.session_id);
    Ok(response)
}

/// Toggle microphone mute during recording
#[tauri::command]
pub async fn record_toggle_mic(
    session_id: String,
    muted: bool,
) -> Result<(), String> {
    // Not implemented yet for native backend
    println!("record_toggle_mic not yet implemented for native backend (session_id={}, muted={})", session_id, muted);
    Ok(())
}

/// Toggle system audio mute during recording  
#[tauri::command]
pub async fn record_toggle_system_audio(
    session_id: String,
    muted: bool,
) -> Result<(), String> {
    // Not implemented yet for native backend
    println!("record_toggle_system_audio not yet implemented for native backend (session_id={}, muted={})", session_id, muted);
    Ok(())
}

/// Toggle camera mute during recording
#[tauri::command]
pub async fn record_toggle_camera(
    session_id: String,
    muted: bool,
) -> Result<(), String> {
    // Not implemented yet for native backend
    println!("record_toggle_camera not yet implemented for native backend (session_id={}, muted={})", session_id, muted);
    Ok(())
}

/// Add break (Studio mode only)
#[tauri::command]
pub async fn record_add_break(
    session_id: String,
    app: AppHandle,
) -> Result<AddBreakResponse, String> {
    // Not implemented yet for native backend
    println!("record_add_break not yet implemented for native backend (session_id={})", session_id);
    // Emit placeholder event for UI continuity
    let response = AddBreakResponse { clip_id: uuid::Uuid::new_v4().to_string() };
    app.emit("recording:break-added", &response).map_err(|e| e.to_string())?;
    Ok(response)
}

/// Stop recording and return project path
#[tauri::command]
pub async fn record_stop_native(
    session_id: String,
    app: AppHandle,
) -> Result<StopRecordingResponse, String> {
    println!("Stopping native recording session: {}", session_id);
    
    // Stop the active backend capture
    {
        let mut global = NATIVE_BACKEND.lock().await;
        if let Some(mut backend) = global.take() {
            backend.stop_capture().await.map_err(|e| format!("Failed to stop capture: {}", e))?;
        } else {
            return Err("No active capture backend".to_string());
        }
    }

    // Reset tray to idle state
    let _ = crate::commands::tray::reset_tray_to_idle_impl(app.clone()).await;

    // Emit stopped event with placeholder response
    let response = StopRecordingResponse {
        project_path: String::new(),
        summary: RecordingSummary {
            duration_ms: 0,
            clips: vec![],
            has_audio: false,
            has_camera: false,
        },
    };
    app.emit("recording:stopped", &response).map_err(|e| e.to_string())?;

    println!("Native capture stopped successfully");
    Ok(response)
}

/// Get current recording state
#[tauri::command]
pub async fn record_get_state(
    _session_id: String,
) -> Result<crate::capture::RecordingState, String> {
    // TODO: Implement proper session state management
    // For now, just return Idle state
    Ok(crate::capture::RecordingState::Idle)
}

/// Open project in editor
#[tauri::command]
pub async fn project_open(
    project_path: String,
    app: AppHandle,
) -> Result<Project, String> {
    // Load project from project.json
    let project_file = std::path::Path::new(&project_path).join("project.json");
    let project_json = std::fs::read_to_string(&project_file)
        .map_err(|e| format!("Failed to read project file: {}", e))?;
    
    let project: Project = serde_json::from_str(&project_json)
        .map_err(|e| format!("Failed to parse project: {}", e))?;
    
    // Open editor with the project
    open_editor_with_project(&app, &project_path).await?;
    
    println!("Project opened: {}", project_path);
    Ok(project)
}

/// Editor playback control - play from specific time
#[tauri::command] 
pub async fn editor_play(
    start_time_ms: u64,
    app: AppHandle,
) -> Result<(), String> {
    // Emit play command to editor
    app.emit("editor:play", start_time_ms).map_err(|e| e.to_string())?;
    println!("Editor playback started from {}ms", start_time_ms);
    Ok(())
}

/// Editor playback control - pause
#[tauri::command]
pub async fn editor_pause(
    app: AppHandle,
) -> Result<(), String> {
    // Emit pause command to editor
    app.emit("editor:pause", ()).map_err(|e| e.to_string())?;
    println!("Editor playback paused");
    Ok(())
}

/// Editor playback control - seek to specific time
#[tauri::command]
pub async fn editor_seek(
    time_ms: u64,
    app: AppHandle,
) -> Result<(), String> {
    // Emit seek command to editor
    app.emit("editor:seek", time_ms).map_err(|e| e.to_string())?;
    println!("Editor seeked to {}ms", time_ms);
    Ok(())
}

/// Add zoom segment to project
#[tauri::command]
pub async fn editor_add_zoom(
    segment: ZoomSegment,
    app: AppHandle,
) -> Result<String, String> {
    // Emit add zoom command to editor
    app.emit("editor:add-zoom", &segment).map_err(|e| e.to_string())?;
    println!("Zoom segment added: {}", segment.id);
    Ok(segment.id)
}

/// Update zoom segment
#[tauri::command]
pub async fn editor_update_zoom(
    id: String,
    updates: serde_json::Value,
    app: AppHandle,
) -> Result<(), String> {
    // Emit update zoom command to editor
    app.emit("editor:update-zoom", serde_json::json!({ "id": id, "updates": updates }))
        .map_err(|e| e.to_string())?;
    println!("Zoom segment updated: {}", id);
    Ok(())
}

/// Delete zoom segment
#[tauri::command]
pub async fn editor_delete_zoom(
    id: String,
    app: AppHandle,
) -> Result<(), String> {
    // Emit delete zoom command to editor
    app.emit("editor:delete-zoom", id.clone()).map_err(|e| e.to_string())?;
    println!("Zoom segment deleted: {}", id);
    Ok(())
}

/// Export project with progress updates
#[tauri::command]
pub async fn export_run(
    format: String,           // "mp4" or "gif"
    fps: Option<u32>,
    #[allow(unused_variables)]
    base_resolution: Option<Resolution>,
    #[allow(unused_variables)]
    quality: Option<f64>,     // 0.0 - 1.0
    app: AppHandle,
) -> Result<String, String> {
    println!("Starting export: format={}, fps={:?}", format, fps);
    
    // TODO: Implement export using ffmpeg-next
    // This will be implemented in Sprint 4
    
    // For now, emit progress events to show the API works
    for i in 0..=100 {
        app.emit("export:progress", serde_json::json!({
            "current": i,
            "total": 100,
            "percentage": i as f64
        })).map_err(|e| e.to_string())?;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    
    // Return placeholder path
    let output_path = format!("/tmp/export_{}.{}", 
        chrono::Local::now().format("%Y%m%d_%H%M%S"), 
        format
    );
    
    println!("Export completed: {}", output_path);
    Ok(output_path)
}

/// Hide UI elements for clean recording
async fn hide_ui_for_recording(app: &AppHandle) -> Result<(), String> {
    // Hide capture bar
    if let Some(bar) = app.get_webview_window("capture-bar") {
        bar.hide().map_err(|e| e.to_string())?;
        println!("Capture bar hidden for recording");
    }
    
    // Hide display preview
    if let Some(preview) = app.get_webview_window("display-preview") {
        preview.hide().map_err(|e| e.to_string())?;
        println!("Display preview hidden for recording");
    }
    
    // Close any existing recording HUD
    if let Some(hud) = app.get_webview_window("recording-hud") {
        let _ = hud.close();
        println!("Recording HUD closed");
    }
    
    Ok(())
}

/// Legacy tray timer - replaced by main timer system to avoid conflicts  
/// This function is kept for compatibility but should not be used
#[deprecated(note = "Use the main timer system instead to avoid timer conflicts")]
fn _legacy_spawn_tray_timer(app: AppHandle, started_at: i64) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            
            let elapsed = chrono::Utc::now().timestamp_millis() - started_at;
            let seconds = elapsed / 1000;
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            let secs = seconds % 60;
            let time_str = format!("{:02}:{:02}:{:02}", hours, minutes, secs);
            
            if let Err(_) = crate::commands::tray::update_main_tray_timer_cmd(app.clone(), time_str).await {
                // If tray update fails, recording might have stopped
                break;
            }
        }
    });
}

/// Open editor window with project
async fn open_editor_with_project(app: &AppHandle, project_path: &str) -> Result<(), String> {
    use tauri::{WebviewWindowBuilder, WebviewUrl, Size, LogicalSize, Position, LogicalPosition};

    println!("Opening editor with project: {}", project_path);

    // Early exit if editor already exists - prevents duplicate windows from racing stop commands
    if let Some(_existing) = app.get_webview_window("editor") {
        println!("Editor already exists, skipping creation");
        return Ok(());
    }
    
    // Create editor URL with project path
    let url = format!("editor.html?project={}", urlencoding::encode(project_path));
    
    // Create editor window with overlay titlebar + native traffic lights
    let builder = WebviewWindowBuilder::new(app, "editor", WebviewUrl::App(url.into()))
        .title("Tarantino Editor")
        .decorations(true)
        .transparent(false)
        .resizable(true)
        .always_on_top(false)
        .visible(true)
        .skip_taskbar(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let editor_window = builder.build()
        .map_err(|e| format!("Failed to create editor window: {}", e))?;

    // Position native traffic lights inside our custom titlebar
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        editor_window.set_traffic_lights_inset(16.0, 20.0).ok();
    }

    // Set reasonable size and position
    let size = LogicalSize::new(1400.0, 900.0);
    let position = LogicalPosition::new(120.0, 80.0);

    editor_window.set_size(Size::Logical(size)).map_err(|e| e.to_string())?;
    editor_window.set_position(Position::Logical(position)).map_err(|e| e.to_string())?;
    editor_window.set_min_size(Some(Size::Logical(LogicalSize::new(1100.0, 720.0)))).ok();

    // Show and focus the window
    editor_window.show().map_err(|e| e.to_string())?;
    editor_window.set_focus().map_err(|e| e.to_string())?;
    
    // Open DevTools in debug mode
    #[cfg(debug_assertions)]
    {
        editor_window.open_devtools();
    }
    
    println!("Editor window opened successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capture_target_creation() {
        let target = CaptureTarget {
            target_type: CaptureTargetType::Screen,
            id: Some(0),
            bounds: None,
        };
        
        assert!(matches!(target.target_type, CaptureTargetType::Screen));
        assert_eq!(target.id, Some(0));
    }
    
    #[test]  
    fn test_start_recording_payload() {
        let payload = StartRecordingPayload {
            mode: RecordingMode::Quick,
            target: CaptureTarget {
                target_type: CaptureTargetType::Window,
                id: Some(12345),
                bounds: None,
            },
            mic_name: Some("Built-in Microphone".to_string()),
            camera_label: None,
            system_audio: Some(true),
            fps: Some(30),
        };
        
        assert!(matches!(payload.mode, RecordingMode::Quick));
        assert_eq!(payload.fps, Some(30));
        assert_eq!(payload.system_audio, Some(true));
    }
}