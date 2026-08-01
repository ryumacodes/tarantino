//! Cross-platform permission contracts and platform-specific implementations.

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
            PermissionError::NotSupported => {
                write!(f, "Permission checking not supported on this platform")
            }
            PermissionError::SystemError(msg) => write!(f, "System error: {}", msg),
            PermissionError::UserDenied => write!(f, "User denied permission request"),
        }
    }
}

impl std::error::Error for PermissionError {}

#[cfg(target_os = "macos")]
mod macos;

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

    pub fn open_camera_preferences() -> Result<()> {
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

/// Open camera preferences (macOS only)
#[command]
pub fn open_camera_preferences() -> Result<(), PermissionError> {
    #[cfg(target_os = "macos")]
    {
        macos::open_camera_preferences().map_err(|e| PermissionError::SystemError(e.to_string()))
    }

    #[cfg(not(target_os = "macos"))]
    {
        other_platforms::open_camera_preferences()
    }
}

/// Diagnose screen capture issues and provide troubleshooting info
#[command]
pub fn diagnose_screen_capture() -> Result<String, PermissionError> {
    #[cfg(target_os = "macos")]
    {
        macos::diagnose_screen_capture().map_err(|e| PermissionError::SystemError(e.to_string()))
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
                    println!(
                        "=== PERMISSIONS: Accessibility permission still not granted after request ==="
                    );
                    return Err(PermissionError::UserDenied);
                }
                Err(e) => {
                    println!(
                        "=== PERMISSIONS: Failed to request accessibility permission: {} ===",
                        e
                    );
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
