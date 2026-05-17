//! Input configuration commands (camera, mic, system audio, webcam)

use crate::state::UnifiedAppState;
use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

/// Grant camera/microphone permission to a WKWebView on macOS.
/// Returns true if the delegate was successfully installed.
#[cfg(target_os = "macos")]
fn grant_media_capture_permission(win: &tauri::WebviewWindow) -> bool {
    use objc::runtime::{Class, Object, Sel};
    use objc::{declare::ClassDecl, msg_send, sel, sel_impl};
    use std::sync::Once;

    static REGISTER: Once = Once::new();
    static mut DELEGATE_CLASS: Option<&'static Class> = None;

    REGISTER.call_once(|| {
        let superclass = Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("TarantinoMediaDelegate", superclass).unwrap();

        extern "C" fn handle_media_permission(
            _this: &Object,
            _sel: Sel,
            _webview: *mut Object,
            _origin: *mut Object,
            _frame: *mut Object,
            _media_type: usize,
            decision_handler: *mut std::ffi::c_void,
        ) {
            println!("Webcam: Media capture permission requested — auto-granting");
            unsafe {
                // ObjC block layout: isa(8) + flags(4) + reserved(4) + invoke(8)
                // invoke is at byte offset 16 = pointer offset 2
                let invoke_ptr = *(decision_handler as *const *const std::ffi::c_void).add(2);
                let invoke: extern "C" fn(*mut std::ffi::c_void, i64) =
                    std::mem::transmute(invoke_ptr);
                invoke(decision_handler, 1); // 1 = WKPermissionDecision.grant
            }
        }

        unsafe {
            let sel = sel!(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:);
            decl.add_method(
                sel,
                handle_media_permission as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object, usize, *mut std::ffi::c_void),
            );
            DELEGATE_CLASS = Some(decl.register());
        }
    });

    unsafe {
        let cls = DELEGATE_CLASS.unwrap();
        let delegate: *mut Object = msg_send![cls, new];

        let ns_window = match win.ns_window() {
            Ok(handle) => handle as *mut Object,
            Err(error) => {
                println!(
                    "Webcam: Could not access native window for media delegate: {}",
                    error
                );
                return false;
            }
        };
        let content_view: *mut Object = msg_send![ns_window, contentView];

        // Walk view hierarchy to find WKWebView or any subclass of it
        fn find_wkwebview(view: *mut Object) -> Option<*mut Object> {
            unsafe {
                // Check if this view is a WKWebView or subclass
                let wk_class = Class::get("WKWebView");
                if let Some(wk_cls) = wk_class {
                    let is_wk: bool = msg_send![view, isKindOfClass: wk_cls];
                    if is_wk {
                        return Some(view);
                    }
                }
                let subviews: *mut Object = msg_send![view, subviews];
                let count: usize = msg_send![subviews, count];
                for i in 0..count {
                    let subview: *mut Object = msg_send![subviews, objectAtIndex: i];
                    if let Some(wk) = find_wkwebview(subview) {
                        return Some(wk);
                    }
                }
                None
            }
        }

        if let Some(wk_webview) = find_wkwebview(content_view) {
            let config: *mut Object = msg_send![wk_webview, configuration];
            let preferences: *mut Object = msg_send![config, preferences];
            if !preferences.is_null() {
                let ns_number = Class::get("NSNumber").unwrap();
                let enabled: *mut Object = msg_send![ns_number, numberWithBool: true];
                let ns_string = Class::get("NSString").unwrap();
                let key_cstr = std::ffi::CString::new("mediaDevicesEnabled").unwrap();
                let key: *mut Object =
                    msg_send![ns_string, stringWithUTF8String: key_cstr.as_ptr()];
                let _: () = msg_send![preferences, setValue: enabled forKey: key];
                println!("Webcam: Enabled WKWebView mediaDevicesEnabled preference");
            }

            let sel = sel!(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:);
            let delegate_responds: bool = msg_send![delegate, respondsToSelector: sel];
            let _: () = msg_send![delegate, retain];
            let _: () = msg_send![wk_webview, setUIDelegate: delegate];
            let current_delegate: *mut Object = msg_send![wk_webview, UIDelegate];
            let current_responds: bool = if current_delegate.is_null() {
                false
            } else {
                msg_send![current_delegate, respondsToSelector: sel]
            };
            println!(
                "Webcam: Media capture permission delegate installed on WKWebView (delegate_responds={}, current_is_ours={}, current_responds={})",
                delegate_responds,
                current_delegate == delegate,
                current_responds
            );
            true
        } else {
            println!("Webcam: Could not find WKWebView in view hierarchy");
            false
        }
    }
}

#[tauri::command]
pub async fn input_set_camera(
    enabled: bool,
    device_id: Option<String>,
    shape: String,
    _app: AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let raw_dev_mode = std::env::var("TARANTINO_DEV_LAUNCH_MODE")
            .map(|mode| mode == "raw")
            .unwrap_or(false);

        if raw_dev_mode {
            state
                .set_camera_input(enabled, device_id.clone(), shape.clone())
                .await
                .map_err(|e| e.to_string())?;
            println!("Camera input: using native AVFoundation camera path in raw dev mode");
            return input_set_camera_native(enabled, device_id, shape, state).await;
        }

        if enabled {
            if let Err(error) = crate::webcam::ensure_camera_permission() {
                println!(
                    "Camera input: native permission preflight failed: {}",
                    error
                );
                state
                    .camera_enabled
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                return Err(error);
            }
        }
    }

    state
        .set_camera_input(enabled, device_id.clone(), shape.clone())
        .await
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        return input_set_camera_with_window(enabled, device_id, shape, _app, state).await;
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[tauri::command]
pub fn webcam_log(level: String, message: String) {
    println!("[WebcamWebView][{}] {}", level, message);
}

#[allow(dead_code)]
async fn input_set_camera_native(
    enabled: bool,
    _device_id: Option<String>,
    shape: String,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if enabled {
            // Start capture session + show native preview window
            let did = state.camera_device_id.lock().clone();
            if let Some(ref mut capture) = *state.webcam_capture.lock() {
                capture.set_shape(&shape);
                println!("Camera input: updated native preview shape={}", shape);
            } else {
                let mut capture = crate::webcam::WebcamCapture::start(did.as_deref(), &shape, 30)
                    .map_err(|error| {
                    println!("Camera input: failed to start camera: {}", error);
                    state
                        .camera_enabled
                        .store(false, std::sync::atomic::Ordering::SeqCst);
                    error
                })?;
                let current_did = state.camera_device_id.lock().clone();
                let still_current = state
                    .camera_enabled
                    .load(std::sync::atomic::Ordering::SeqCst)
                    && current_did == did;
                if !still_current {
                    capture.stop();
                    println!("Camera input: discarded stale native camera start");
                    return Ok(());
                }
                *state.webcam_capture.lock() = Some(capture);
                println!("Camera input: enabled — preview showing");
            }
        } else {
            // Stop capture session + close preview
            if let Some(mut capture) = state.webcam_capture.lock().take() {
                capture.stop();
            }
            println!("Camera input: disabled — preview closed");
        }
    }

    Ok(())
}

async fn input_set_camera_with_window(
    enabled: bool,
    device_id: Option<String>,
    shape: String,
    app: AppHandle,
    _state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    if enabled {
        if let Some(win) = app.get_webview_window("webcam-preview") {
            win.emit("webcam:set-shape", shape.clone())
                .map_err(|e| e.to_string())?;
        } else {
            use tauri::{LogicalSize, Size, WebviewUrl, WebviewWindowBuilder};

            let url = if let Some(ref did) = device_id {
                format!(
                    "webcam.html?deviceId={}&shape={}",
                    urlencoding::encode(did),
                    urlencoding::encode(&shape)
                )
            } else {
                format!("webcam.html?shape={}", urlencoding::encode(&shape))
            };

            let win =
                WebviewWindowBuilder::new(&app, "webcam-preview", WebviewUrl::App(url.into()))
                    .title("Webcam Preview")
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .resizable(false)
                    .visible(false)
                    .inner_size(180.0, 180.0)
                    .build()
                    .map_err(|e| e.to_string())?;

            win.set_size(Size::Logical(LogicalSize::new(180.0, 180.0)))
                .map_err(|e| e.to_string())?;
            win.show().map_err(|e| e.to_string())?;

            // Grant camera permission to the WKWebView after showing
            #[cfg(target_os = "macos")]
            {
                let win_clone = win.clone();
                // Retry with increasing delay — the WKWebView view hierarchy
                // takes time to build after the window is shown
                tokio::spawn(async move {
                    for attempt in 1..=5 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(200 * attempt)).await;
                        if grant_media_capture_permission(&win_clone) {
                            if let Err(error) = win_clone.emit("webcam:ready-to-start", ()) {
                                println!("Webcam: Failed to notify WebView readiness: {}", error);
                            }
                            break;
                        }
                        println!("Webcam: Retry {} to find WKWebView...", attempt);
                    }
                });
            }

            println!("Webcam preview window created");
        }
    } else {
        // Close webcam preview window
        if let Some(win) = app.get_webview_window("webcam-preview") {
            let _ = win.close();
            println!("Webcam preview window closed");
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn input_set_mic(
    enabled: bool,
    device_id: Option<String>,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state
        .set_mic_input(enabled, device_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn input_set_system_audio(
    enabled: bool,
    source_id: Option<String>,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state
        .set_system_audio(enabled, source_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webcam_set_transform(
    x_norm: f32,
    y_norm: f32,
    size_norm: f32,
    shape: String,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state
        .set_webcam_transform(x_norm, y_norm, size_norm, shape)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webcam_set_autododge(
    enabled: bool,
    radius_norm: f32,
    strength: f32,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state
        .set_webcam_autododge(enabled, radius_norm, strength)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct WebcamPosition {
    pub x: f32,
    pub y: f32,
}

#[tauri::command]
pub async fn save_webcam_recording(
    data: Option<Vec<u8>>,
    _position: WebcamPosition,
    _size: f32,
    _shape: String,
    output_path: Option<String>,
    data_base64: Option<String>,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    use base64::Engine;
    use std::fs;
    use std::io::Write;

    let data = if let Some(data_base64) = data_base64 {
        base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .map_err(|e| format!("Failed to decode webcam data: {}", e))?
    } else {
        data.ok_or_else(|| "No webcam data provided".to_string())?
    };

    let recording_path = if let Some(path) = output_path {
        path
    } else {
        state
            .recording
            .get_current_config()
            .ok_or_else(|| "No active recording session".to_string())?
            .output_path
    };
    let base = recording_path.trim_end_matches(".mp4");

    let webcam_path = format!("{}.webcam.webm", base);
    let mut file = fs::File::create(&webcam_path)
        .map_err(|e| format!("Failed to create webcam file: {}", e))?;
    file.write_all(&data)
        .map_err(|e| format!("Failed to write webcam data: {}", e))?;

    println!(
        "Webcam recording saved: {} ({} bytes)",
        webcam_path,
        data.len()
    );
    Ok(())
}

pub async fn start_webview_webcam_recording(
    app: &AppHandle,
    output_path: &str,
    state: &UnifiedAppState,
) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("webcam-preview") {
        let (webcam_x, webcam_y, webcam_size, webcam_shape) = state.webcam_transform();
        win.emit(
            "recording:started",
            serde_json::json!({
                "output_path": output_path,
                "webcam_x": webcam_x,
                "webcam_y": webcam_y,
                "webcam_size": webcam_size,
                "webcam_shape": webcam_shape,
            }),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub async fn stop_webview_webcam_recording(
    app: &AppHandle,
    output_path: &str,
    state: &UnifiedAppState,
) -> Result<(), String> {
    let Some(win) = app.get_webview_window("webcam-preview") else {
        return Ok(());
    };

    sync_webcam_window_transform(&win, state).await;
    win.emit("webcam:stop", ()).map_err(|e| e.to_string())?;

    let webcam_path = format!("{}.webcam.webm", output_path.trim_end_matches(".mp4"));
    let path = std::path::Path::new(&webcam_path);
    let mut saved = false;
    for _ in 0..150 {
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.len() > 0 {
                println!("Webcam WebView recording saved: {}", webcam_path);
                saved = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    if !saved {
        println!(
            "Warning: timed out waiting for WebView webcam recording at {}",
            webcam_path
        );
    }

    let _ = win.emit("webcam:close", ());
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    let _ = win.close();
    Ok(())
}

async fn sync_webcam_window_transform(win: &tauri::WebviewWindow, state: &UnifiedAppState) {
    let Ok(position) = win.outer_position() else {
        return;
    };
    let Ok(size) = win.outer_size() else {
        return;
    };
    let Ok(Some(monitor)) = win.current_monitor() else {
        return;
    };

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    if monitor_size.width == 0 || monitor_size.height == 0 {
        return;
    }

    let center_x = position.x - monitor_position.x + (size.width as i32 / 2);
    let center_y = position.y - monitor_position.y + (size.height as i32 / 2);
    let x_norm = (center_x as f32 / monitor_size.width as f32).clamp(0.0, 1.0);
    let y_norm = (center_y as f32 / monitor_size.height as f32).clamp(0.0, 1.0);

    let (_, _, _, shape) = state.webcam_transform();
    let size_norm = (size.width as f32 / monitor_size.width as f32).clamp(0.08, 0.25);
    if let Err(error) = state
        .set_webcam_transform(x_norm, y_norm, size_norm, shape)
        .await
    {
        println!("Warning: failed to sync webcam window transform: {}", error);
    }
}
