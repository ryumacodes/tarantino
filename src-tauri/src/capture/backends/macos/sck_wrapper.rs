//! Safe Rust wrappers around the C FFI for ScreenCaptureKit

use super::ffi::*;
use anyhow::Result;
use std::os::raw::c_void;
use std::ffi::CStr;

/// Rust-owned display info (with owned strings instead of pointers)
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub display_id: u64,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

/// Rust-owned window info (with owned strings instead of pointers)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WindowInfo {
    pub window_id: u64,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub owner_name: String,
}

/// Safe wrapper to get shareable displays
pub fn get_shareable_displays() -> Result<Vec<DisplayInfo>> {
    unsafe {
        let mut displays_ptr: *mut SCKDisplay = std::ptr::null_mut();
        let mut count: usize = 0;

        let success = sck_get_shareable_displays(&mut displays_ptr, &mut count);

        if !success || displays_ptr.is_null() {
            anyhow::bail!("Failed to get shareable displays");
        }

        // Convert C array to Rust Vec with owned strings
        let displays_slice = std::slice::from_raw_parts(displays_ptr, count);
        let mut result = Vec::with_capacity(count);

        for display in displays_slice {
            // Convert C string to Rust String BEFORE freeing
            let name = if !display.name.is_null() {
                CStr::from_ptr(display.name)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Display".to_string()
            };

            result.push(DisplayInfo {
                display_id: display.display_id,
                name,
                width: display.width,
                height: display.height,
                scale_factor: display.scale_factor,
                is_primary: display.is_primary,
            });
        }

        // Free the C array (after converting strings)
        sck_free_displays(displays_ptr, count);

        Ok(result)
    }
}

/// Safe wrapper to get shareable windows
pub fn get_shareable_windows() -> Result<Vec<WindowInfo>> {
    unsafe {
        let mut windows_ptr: *mut SCKWindow = std::ptr::null_mut();
        let mut count: usize = 0;

        let success = sck_get_shareable_windows(&mut windows_ptr, &mut count);

        if !success || windows_ptr.is_null() {
            anyhow::bail!("Failed to get shareable windows");
        }

        // Convert C array to Rust Vec with owned strings
        let windows_slice = std::slice::from_raw_parts(windows_ptr, count);
        let mut result = Vec::with_capacity(count);

        for window in windows_slice {
            // Convert C strings to Rust Strings BEFORE freeing
            let title = if !window.title.is_null() {
                CStr::from_ptr(window.title)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Untitled".to_string()
            };

            let owner_name = if !window.owner_name.is_null() {
                CStr::from_ptr(window.owner_name)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown".to_string()
            };

            result.push(WindowInfo {
                window_id: window.window_id,
                title,
                width: window.width,
                height: window.height,
                owner_name,
            });
        }

        // Free the C array (after converting strings)
        sck_free_windows(windows_ptr, count);

        Ok(result)
    }
}

/// Safe wrapper to check permissions
pub fn check_permissions() -> SCKPermissionStatus {
    unsafe { sck_check_permissions() }
}

/// Safe wrapper to request permissions
pub fn request_permissions() -> SCKPermissionStatus {
    unsafe { sck_request_permissions() }
}

/// Context for frame and audio callbacks
struct CallbackContext {
    frame_callback: Box<dyn FnMut(SCKFrameData) + Send>,
    audio_callback: Box<dyn FnMut(SCKAudioData) + Send>,
}

/// C callback function that bridges to Rust closure for frames
extern "C" fn frame_callback_trampoline(context: *mut c_void, frame: SCKFrameData) {
    if context.is_null() {
        return;
    }

    unsafe {
        let ctx = &mut *(context as *mut CallbackContext);
        (ctx.frame_callback)(frame);
    }
}

/// C callback function that bridges to Rust closure for audio
extern "C" fn audio_callback_trampoline(context: *mut c_void, audio: SCKAudioData) {
    if context.is_null() {
        return;
    }

    unsafe {
        let ctx = &mut *(context as *mut CallbackContext);
        (ctx.audio_callback)(audio);
    }
}

/// Safe wrapper to start capture
pub fn start_capture<F, A>(config: SCKCaptureConfig, frame_callback: F, audio_callback: A) -> Result<*mut c_void>
where
    F: FnMut(SCKFrameData) + Send + 'static,
    A: FnMut(SCKAudioData) + Send + 'static,
{
    unsafe {
        // Box the callback context
        let ctx = Box::new(CallbackContext {
            frame_callback: Box::new(frame_callback),
            audio_callback: Box::new(audio_callback),
        });

        let ctx_ptr = Box::into_raw(ctx) as *mut c_void;

        // Start capture with the callback trampolines
        let instance = sck_start_capture(
            config,
            ctx_ptr,
            frame_callback_trampoline,
            audio_callback_trampoline
        );

        if instance.is_null() {
            // Clean up context if capture failed
            let _ = Box::from_raw(ctx_ptr as *mut CallbackContext);
            anyhow::bail!("Failed to start ScreenCaptureKit capture");
        }

        Ok(instance)
    }
}

/// Safe wrapper to stop capture
pub fn stop_capture(instance: *mut c_void) -> Result<()> {
    if instance.is_null() {
        anyhow::bail!("Invalid capture instance");
    }

    unsafe {
        let mut rust_context: *mut c_void = std::ptr::null_mut();
        let success = sck_stop_capture(instance, &mut rust_context);

        // Clean up the Rust callback context if it was returned
        if !rust_context.is_null() {
            // Reconstruct the Box and let it drop to free the memory
            let _ = Box::from_raw(rust_context as *mut CallbackContext);
        }

        if !success {
            anyhow::bail!("Failed to stop capture");
        }

        Ok(())
    }
}
