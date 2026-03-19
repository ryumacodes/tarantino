#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub accessibility_granted: bool,
    pub screen_recording_granted: bool,
    pub can_request_accessibility: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionError {
    NotSupported,
    SystemError(String),
    UserDenied,
}

impl std::fmt::Display for PermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionError::NotSupported => write!(f, "Permission checking not supported on this platform"),
            PermissionError::SystemError(msg) => write!(f, "System error: {}", msg),
            PermissionError::UserDenied => write!(f, "User denied permission request"),
        }
    }
}

impl std::error::Error for PermissionError {}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use core_foundation::base::{CFRelease, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::string::CFStringRef;
    use std::ffi::c_void;
    

    // External function declarations for macOS accessibility APIs
    extern "C" {
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
    extern "C" {
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
            return Err(PermissionError::SystemError(format!("Failed to open accessibility preferences: {}", error_msg)).into());
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
                        println!("=== PERMISSIONS: Screen recording permission: {} ===", perms.screen_recording);
                        perms.screen_recording
                    },
                    Err(e) => {
                        println!("=== PERMISSIONS: Failed to check permissions: {} ===", e);
                        false
                    }
                }
            },
            Err(e) => {
                println!("=== PERMISSIONS: Failed to create capture backend: {} ===", e);
                false
            }
        }
    }
    
    pub fn request_screen_recording_permission() -> Result<bool> {
        // Use native ScreenCaptureKit to request screen recording permission
        use crate::capture::backends::CaptureBackendFactory;

        println!("=== PERMISSIONS: Requesting screen recording permission ===");

        // Create native backend
        let backend = CaptureBackendFactory::create_backend()
            .map_err(|e| PermissionError::SystemError(format!("Failed to create capture backend: {}", e)))?;

        // Request permissions using native API
        match futures::executor::block_on(backend.request_permissions()) {
            Ok(perms) => {
                println!("=== PERMISSIONS: Screen recording permission: {} ===", perms.screen_recording);
                Ok(perms.screen_recording)
            },
            Err(e) => {
                Err(PermissionError::SystemError(format!("Failed to request permissions: {}", e)).into())
            }
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
            return Err(PermissionError::SystemError(format!("Failed to open screen recording preferences: {}", error_msg)).into());
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
            Ok(backend) => {
                match futures::executor::block_on(backend.enumerate_sources()) {
                    Ok(sources) => {
                        let displays: Vec<_> = sources.iter()
                            .filter(|s| matches!(s.source_type, crate::capture::CaptureSourceType::Display))
                            .collect();

                        diagnostics.push(format!("✅ Found {} display(s)", displays.len()));
                        for (i, display) in displays.iter().enumerate() {
                            diagnostics.push(format!("  Display {}: {} ({}x{})",
                                i + 1, display.name, display.width, display.height));
                        }
                    },
                    Err(e) => diagnostics.push(format!("❌ Failed to enumerate displays: {}", e)),
                }
            },
            Err(e) => diagnostics.push(format!("❌ Failed to create capture backend: {}", e)),
        }

        Ok(diagnostics.join("\n"))
    }
}

#[cfg(not(target_os = "macos"))]
mod other_platforms {
    use super::*;
    
    pub fn check_accessibility_permission() -> bool {
        // On other platforms, assume permissions are granted
        true
    }
    
    pub fn request_accessibility_permission() -> Result<bool> {
        // On other platforms, no permission request needed
        Ok(true)
    }
    
    pub fn open_accessibility_preferences() -> Result<()> {
        // On other platforms, no preferences to open
        Err(PermissionError::NotSupported.into())
    }
    
    pub fn check_screen_recording_permission() -> bool {
        // On other platforms, assume permissions are granted
        true
    }
}

/// Check current accessibility and screen recording permission status
#[command]
pub fn check_permissions() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        PermissionStatus {
            accessibility_granted: macos::check_accessibility_permission(),
            screen_recording_granted: macos::check_screen_recording_permission(),
            can_request_accessibility: true,
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        PermissionStatus {
            accessibility_granted: other_platforms::check_accessibility_permission(),
            screen_recording_granted: other_platforms::check_screen_recording_permission(),
            can_request_accessibility: false,
        }
    }
}

/// Request accessibility permission (shows system dialog on macOS)
#[command]
pub fn request_accessibility_permission() -> Result<bool, PermissionError> {
    #[cfg(target_os = "macos")]
    {
        macos::request_accessibility_permission()
            .map_err(|e| PermissionError::SystemError(e.to_string()))
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        other_platforms::request_accessibility_permission()
            .map_err(|e| PermissionError::SystemError(e.to_string()))
    }
}

/// Open system accessibility preferences (macOS only)
#[command]
pub fn open_accessibility_preferences() -> Result<(), PermissionError> {
    #[cfg(target_os = "macos")]
    {
        macos::open_accessibility_preferences()
            .map_err(|e| PermissionError::SystemError(e.to_string()))
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        other_platforms::open_accessibility_preferences()
    }
}

/// Request screen recording permission (shows system dialog on macOS)
#[command]
pub fn request_screen_recording_permission() -> Result<bool, PermissionError> {
    #[cfg(target_os = "macos")]
    {
        macos::request_screen_recording_permission()
            .map_err(|e| PermissionError::SystemError(e.to_string()))
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true) // Always granted on other platforms
    }
}

/// Open screen recording preferences (macOS only)
#[command]
pub fn open_screen_recording_preferences() -> Result<(), PermissionError> {
    #[cfg(target_os = "macos")]
    {
        macos::open_screen_recording_preferences()
            .map_err(|e| PermissionError::SystemError(e.to_string()))
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Err(PermissionError::NotSupported)
    }
}

/// Diagnose screen capture issues and provide troubleshooting info
#[command]
pub fn diagnose_screen_capture() -> Result<String, PermissionError> {
    #[cfg(target_os = "macos")]
    {
        macos::diagnose_screen_capture()
            .map_err(|e| PermissionError::SystemError(e.to_string()))
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok("Screen capture should work on this platform without special permissions.".to_string())
    }
}

/// Check if accessibility permissions are required for mouse tracking
pub fn are_accessibility_permissions_required() -> bool {
    // On macOS, accessibility permissions are required for reliable mouse tracking
    // On other platforms, they might not be required
    cfg!(target_os = "macos")
}

/// Validate that all required permissions are granted for mouse tracking.
/// If not granted, requests permission (shows macOS system dialog).
pub fn validate_mouse_tracking_permissions() -> Result<(), PermissionError> {
    if !are_accessibility_permissions_required() {
        return Ok(());
    }

    let status = check_permissions();

    if !status.accessibility_granted {
        // Request permission, which shows the macOS system prompt
        #[cfg(target_os = "macos")]
        {
            println!("=== PERMISSIONS: Accessibility not granted, requesting permission ===");
            match macos::request_accessibility_permission() {
                Ok(granted) if granted => return Ok(()),
                Ok(_) => {
                    println!("=== PERMISSIONS: Accessibility permission still not granted after request ===");
                    return Err(PermissionError::UserDenied);
                }
                Err(e) => {
                    println!("=== PERMISSIONS: Failed to request accessibility permission: {} ===", e);
                    return Err(PermissionError::SystemError(format!("{}", e)));
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        return Err(PermissionError::UserDenied);
    }

    Ok(())
}

/// Validate that all required permissions are granted for recording
#[command]
pub fn validate_recording_permissions() -> Result<PermissionStatus, PermissionError> {
    let status = check_permissions();

    // Check screen recording permission
    if !status.screen_recording_granted {
        return Err(PermissionError::UserDenied);
    }

    // Check accessibility permission for mouse tracking
    if are_accessibility_permissions_required() && !status.accessibility_granted {
        return Err(PermissionError::UserDenied);
    }

    Ok(status)
}

/// Request all required permissions for recording
#[command]
pub fn request_all_recording_permissions() -> Result<PermissionStatus, PermissionError> {
    #[cfg(target_os = "macos")]
    {
        println!("Requesting all recording permissions on macOS");

        // First check current status
        let mut status = check_permissions();

        // Request screen recording permission if needed
        if !status.screen_recording_granted {
            println!("Requesting screen recording permission...");
            let granted = macos::request_screen_recording_permission()
                .map_err(|e| PermissionError::SystemError(e.to_string()))?;
            status.screen_recording_granted = granted;
        }

        // Request accessibility permission if needed
        if !status.accessibility_granted {
            println!("Requesting accessibility permission...");
            let granted = macos::request_accessibility_permission()
                .map_err(|e| PermissionError::SystemError(e.to_string()))?;
            status.accessibility_granted = granted;
        }

        Ok(status)
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On other platforms, assume permissions are granted
        Ok(PermissionStatus {
            accessibility_granted: true,
            screen_recording_granted: true,
            can_request_accessibility: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_permission_status_creation() {
        let status = PermissionStatus {
            accessibility_granted: true,
            screen_recording_granted: true,
            can_request_accessibility: true,
        };
        
        assert!(status.accessibility_granted);
        assert!(status.screen_recording_granted);
        assert!(status.can_request_accessibility);
    }
    
    #[test]
    fn test_permission_error_display() {
        let error = PermissionError::UserDenied;
        assert_eq!(error.to_string(), "User denied permission request");
        
        let error = PermissionError::SystemError("Test error".to_string());
        assert_eq!(error.to_string(), "System error: Test error");
    }
    
    #[test]
    fn test_permissions_required() {
        // Test that the function returns expected values based on platform
        let required = are_accessibility_permissions_required();
        
        #[cfg(target_os = "macos")]
        assert!(required);
        
        #[cfg(not(target_os = "macos"))]
        assert!(!required);
    }
}