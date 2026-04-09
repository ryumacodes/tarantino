//! Input configuration commands (camera, mic, system audio, webcam)

use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use serde::Deserialize;
use crate::state::UnifiedAppState;

/// Grant camera/microphone permission to a WKWebView on macOS.
/// Returns true if the delegate was successfully installed.
#[cfg(target_os = "macos")]
fn grant_media_capture_permission(win: &tauri::WebviewWindow) -> bool {
    use objc::runtime::{Object, Sel, Class};
    use objc::{msg_send, sel, sel_impl, declare::ClassDecl};
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

        let ns_window = win.ns_window().unwrap() as *mut Object;
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
            let _: () = msg_send![delegate, retain];
            let _: () = msg_send![wk_webview, setUIDelegate: delegate];
            println!("Webcam: Media capture permission delegate installed on WKWebView");
            true
        } else {
            println!("Webcam: Could not find WKWebView in view hierarchy");
            false
        }
    }
}

#[tauri::command]
pub async fn input_set_camera(enabled: bool, device_id: Option<String>, shape: String, _app: AppHandle, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_camera_input(enabled, device_id.clone(), shape).await.map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        if enabled {
            // Start capture session + show native preview window
            let did = state.camera_device_id.lock().clone();
            if state.webcam_capture.lock().is_none() {
                if let Some(capture) = crate::webcam::WebcamCapture::start(did.as_deref(), 30) {
                    *state.webcam_capture.lock() = Some(capture);
                    println!("Camera input: enabled — preview showing");
                } else {
                    println!("Camera input: failed to start camera");
                    state.camera_enabled.store(false, std::sync::atomic::Ordering::SeqCst);
                    return Err("Failed to start camera".to_string());
                }
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

#[allow(dead_code)]
async fn _input_set_camera_with_window(enabled: bool, device_id: Option<String>, _shape: String, app: AppHandle, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    if enabled {
        if app.get_webview_window("webcam-preview").is_none() {
            use tauri::{WebviewWindowBuilder, WebviewUrl, LogicalSize, Size};

            let url = if let Some(ref did) = device_id {
                format!("webcam.html?deviceId={}", urlencoding::encode(did))
            } else {
                "webcam.html".to_string()
            };

            let win = WebviewWindowBuilder::new(&app, "webcam-preview", WebviewUrl::App(url.into()))
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

            win.set_size(Size::Logical(LogicalSize::new(180.0, 180.0))).map_err(|e| e.to_string())?;
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
pub async fn input_set_mic(enabled: bool, device_id: Option<String>, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_mic_input(enabled, device_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn input_set_system_audio(enabled: bool, source_id: Option<String>, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_system_audio(enabled, source_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webcam_set_transform(x_norm: f32, y_norm: f32, size_norm: f32, shape: String, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_webcam_transform(x_norm, y_norm, size_norm, shape).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webcam_set_autododge(enabled: bool, radius_norm: f32, strength: f32, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_webcam_autododge(enabled, radius_norm, strength).await.map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct WebcamPosition { pub x: f32, pub y: f32 }

#[tauri::command]
pub async fn save_webcam_recording(data: Vec<u8>, _position: WebcamPosition, _size: f32, _shape: String, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    use std::fs;
    use std::io::Write;

    // Get the recording output path and save webcam alongside it
    // e.g., /tmp/recording.mp4 → /tmp/recording.webcam.webm
    let config = state.recording.get_current_config()
        .ok_or_else(|| "No active recording session".to_string())?;
    let base = config.output_path.trim_end_matches(".mp4");

    let webcam_path = format!("{}.webcam.webm", base);
    let mut file = fs::File::create(&webcam_path).map_err(|e| format!("Failed to create webcam file: {}", e))?;
    file.write_all(&data).map_err(|e| format!("Failed to write webcam data: {}", e))?;

    println!("Webcam recording saved: {} ({} bytes)", webcam_path, data.len());
    Ok(())
}
