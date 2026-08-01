//! macOS privacy and accessibility permission implementation.

use super::*;
use core_foundation::base::{CFRelease, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFStringRef;
use std::ffi::c_void;

// External function declarations for macOS accessibility APIs
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        numValues: isize,
        keyCallbacks: *const c_void,
        valueCallbacks: *const c_void,
    ) -> *const c_void;
}

// External constants
unsafe extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

pub fn check_accessibility_permission() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn request_accessibility_permission() -> Result<bool> {
    unsafe {
        // Create options dictionary to show the prompt
        let prompt_key = kAXTrustedCheckOptionPrompt;
        let prompt_value = CFBoolean::true_value().as_concrete_TypeRef();

        let keys: [*const c_void; 1] = [prompt_key as *const c_void];
        let values: [*const c_void; 1] = [prompt_value as *const c_void];

        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::null(),
            std::ptr::null(),
        );

        let result = AXIsProcessTrustedWithOptions(options);

        if !options.is_null() {
            CFRelease(options);
        }

        Ok(result)
    }
}

pub fn open_accessibility_preferences() -> Result<()> {
    use std::process::Command;

    let output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .output()
        .map_err(|e| PermissionError::SystemError(format!("Failed to open preferences: {}", e)))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(PermissionError::SystemError(format!(
            "Failed to open accessibility preferences: {}",
            error_msg
        ))
        .into());
    }

    Ok(())
}

pub fn check_screen_recording_permission() -> bool {
    // Use native ScreenCaptureKit to check screen recording permission
    use crate::capture::backends::CaptureBackendFactory;

    println!("=== PERMISSIONS: Testing screen recording permission ===");

    // Create native backend
    match CaptureBackendFactory::create_backend() {
        Ok(backend) => {
            // Check permissions using native API
            match futures::executor::block_on(backend.check_permissions()) {
                Ok(perms) => {
                    println!(
                        "=== PERMISSIONS: Screen recording permission: {} ===",
                        perms.screen_recording
                    );
                    perms.screen_recording
                }
                Err(e) => {
                    println!("=== PERMISSIONS: Failed to check permissions: {} ===", e);
                    false
                }
            }
        }
        Err(e) => {
            println!(
                "=== PERMISSIONS: Failed to create capture backend: {} ===",
                e
            );
            false
        }
    }
}

pub fn request_screen_recording_permission() -> Result<bool> {
    // Use native ScreenCaptureKit to request screen recording permission
    use crate::capture::backends::CaptureBackendFactory;

    println!("=== PERMISSIONS: Requesting screen recording permission ===");

    // Create native backend
    let backend = CaptureBackendFactory::create_backend().map_err(|e| {
        PermissionError::SystemError(format!("Failed to create capture backend: {}", e))
    })?;

    // Request permissions using native API
    match futures::executor::block_on(backend.request_permissions()) {
        Ok(perms) => {
            println!(
                "=== PERMISSIONS: Screen recording permission: {} ===",
                perms.screen_recording
            );
            Ok(perms.screen_recording)
        }
        Err(e) => Err(PermissionError::SystemError(format!(
            "Failed to request permissions: {}",
            e
        ))
        .into()),
    }
}

pub fn open_screen_recording_preferences() -> Result<()> {
    use std::process::Command;

    let output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .output()
        .map_err(|e| PermissionError::SystemError(format!("Failed to open preferences: {}", e)))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(PermissionError::SystemError(format!(
            "Failed to open screen recording preferences: {}",
            error_msg
        ))
        .into());
    }

    Ok(())
}

pub fn open_camera_preferences() -> Result<()> {
    use std::process::Command;

    let output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Camera")
        .output()
        .map_err(|e| PermissionError::SystemError(format!("Failed to open preferences: {}", e)))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(PermissionError::SystemError(format!(
            "Failed to open camera preferences: {}",
            error_msg
        ))
        .into());
    }

    Ok(())
}

pub fn diagnose_screen_capture() -> Result<String> {
    use crate::capture::backends::CaptureBackendFactory;

    let mut diagnostics = Vec::new();

    // Check screen recording permission
    if check_screen_recording_permission() {
        diagnostics.push("✅ Screen recording permission: GRANTED".to_string());
    } else {
        diagnostics.push("❌ Screen recording permission: DENIED".to_string());
    }

    // Test display enumeration using native backend
    match CaptureBackendFactory::create_backend() {
        Ok(backend) => match futures::executor::block_on(backend.enumerate_sources()) {
            Ok(sources) => {
                let displays: Vec<_> = sources
                    .iter()
                    .filter(|s| matches!(s.source_type, crate::capture::CaptureSourceType::Display))
                    .collect();

                diagnostics.push(format!("✅ Found {} display(s)", displays.len()));
                for (i, display) in displays.iter().enumerate() {
                    diagnostics.push(format!(
                        "  Display {}: {} ({}x{})",
                        i + 1,
                        display.name,
                        display.width,
                        display.height
                    ));
                }
            }
            Err(e) => diagnostics.push(format!("❌ Failed to enumerate displays: {}", e)),
        },
        Err(e) => diagnostics.push(format!("❌ Failed to create capture backend: {}", e)),
    }

    Ok(diagnostics.join("\n"))
}
