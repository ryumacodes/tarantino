//! Native capture backends
//!
//! This module provides platform-specific native capture implementations:
//! - macOS: ScreenCaptureKit (SCK)
//! - Windows: Desktop Duplication API (DXGI)
//! - Linux: PipeWire via xdg-desktop-portal
//!
//! Each backend implements the `NativeCaptureBackend` trait for a unified interface.

use anyhow::Result;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// Platform-specific backend modules
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod linux;

// Optional xcap fallback (disabled; remove cfg until feature is added)
// pub mod xcap_fallback;

/// Captured frame data from any backend
#[derive(Clone, Debug)]
pub struct CapturedFrame {
    /// Raw pixel data (format specified in pixel_format)
    pub data: Bytes,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Pixel format (e.g., "BGRA", "RGBA", "NV12")
    pub pixel_format: String,
    /// Timestamp in microseconds since epoch
    pub timestamp_us: u64,
    /// Bytes per row (stride)
    pub stride: u32,
}

/// Captured audio data from any backend
/// Note: Audio capture is implemented but not yet integrated
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct CapturedAudio {
    /// Raw audio sample data (typically PCM)
    pub data: Bytes,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u32,
    /// Timestamp in microseconds since epoch
    pub timestamp_us: u64,
}

/// Information about a capture source (display or window)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureSourceInfo {
    /// Source ID (display index or window handle)
    pub id: u64,
    /// Human-readable name
    pub name: String,
    /// Source type
    pub source_type: CaptureSourceType,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Scale factor (for HiDPI displays)
    pub scale_factor: f64,
    /// Whether this is the primary display
    pub is_primary: bool,
    /// Owning application name (for windows)
    pub owner_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureSourceType {
    Display,
    Window,
}

/// Backend capabilities - what features are supported
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendCapabilities {
    /// Can capture display/screen
    pub display_capture: bool,
    /// Can capture individual windows
    pub window_capture: bool,
    /// Can capture specific regions
    pub region_capture: bool,
    /// Can include cursor in capture
    pub cursor_capture: bool,
    /// Supports HDR capture
    pub hdr_support: bool,
    /// Can capture system audio
    pub system_audio: bool,
    /// Can capture per-application audio
    pub app_audio: bool,
    /// Hardware acceleration available
    pub hardware_acceleration: bool,
    /// Supported pixel formats
    pub pixel_formats: Vec<String>,
}

/// Configuration for starting a capture session
#[derive(Clone, Debug)]
pub struct CaptureConfig {
    /// Source to capture (display ID or window handle)
    pub source_id: u64,
    /// Source type
    pub source_type: CaptureSourceType,
    /// Target frame rate
    pub fps: u32,
    /// Include cursor in capture
    pub include_cursor: bool,
    /// Include system audio (if supported)
    pub include_audio: bool,
    /// Specific region to capture (None = full source)
    pub region: Option<CaptureRegion>,
    /// Optional output path; if provided, backend should write encoded video here
    #[allow(dead_code)] // Reserved for future use - direct-to-file capture mode
    pub output_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Unified trait for native capture backends
/// Note: Some trait methods are unused but kept for cross-platform architecture
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait NativeCaptureBackend: Send + Sync {
    /// Get backend name for debugging/logging
    fn backend_name(&self) -> &'static str;

    /// Get backend capabilities
    fn capabilities(&self) -> BackendCapabilities;

    /// Enumerate available capture sources (displays + windows)
    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>>;

    /// Check if required permissions are granted
    async fn check_permissions(&self) -> Result<PermissionStatus>;

    /// Request permissions (may show system prompt)
    async fn request_permissions(&self) -> Result<PermissionStatus>;

    /// Start capture session with given configuration
    async fn start_capture(&mut self, config: CaptureConfig) -> Result<CaptureSessionHandle>;

    /// Stop active capture session
    async fn stop_capture(&mut self) -> Result<()>;

    /// Get frame stream (broadcast channel receiver)
    fn frame_receiver(&self) -> Option<broadcast::Receiver<CapturedFrame>>;

    /// Get audio stream (broadcast channel receiver)
    fn audio_receiver(&self) -> Option<broadcast::Receiver<CapturedAudio>>;

    /// Check if capture is currently active
    fn is_active(&self) -> bool;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub screen_recording: bool,
    pub microphone: bool,
    pub camera: bool,
}

/// Handle to an active capture session
#[derive(Clone, Debug)]
pub struct CaptureSessionHandle {
    #[allow(dead_code)] // Reserved for future use - session management features
    pub session_id: String,
    #[allow(dead_code)] // Reserved for future use - session management features
    pub source_info: CaptureSourceInfo,
}

/// Factory for creating the appropriate native backend for the current platform
pub struct CaptureBackendFactory;

impl CaptureBackendFactory {
    /// Create the best available capture backend for the current platform
    pub fn create_backend() -> Result<Box<dyn NativeCaptureBackend>> {
        #[cfg(target_os = "macos")]
        {
            // Try ScreenCaptureKit first (requires macOS 12.3+)
            if Self::is_sck_available() {
                println!("Using native ScreenCaptureKit backend");
                return Ok(Box::new(macos::ScreenCaptureKitBackend::new()?));
            }

            // Fallback path disabled until xcap feature is added
            anyhow::bail!("ScreenCaptureKit requires macOS 12.3 or later");
        }

        #[cfg(target_os = "windows")]
        {
            // Try DXGI Desktop Duplication (Windows 8+)
            if Self::is_dxgi_available() {
                println!("Using native DXGI Desktop Duplication backend");
                return Ok(Box::new(windows::DXGIBackend::new()?));
            }

            // Fallback path disabled until xcap feature is added
            anyhow::bail!("DXGI Desktop Duplication requires Windows 8 or later");
        }

        #[cfg(target_os = "linux")]
        {
            // Try PipeWire via xdg-desktop-portal
            if Self::is_pipewire_available() {
                println!("Using native PipeWire backend");
                return Ok(Box::new(linux::PipeWireBackend::new()?));
            }

            // Fallback path disabled until xcap feature is added
            anyhow::bail!("PipeWire is required for screen capture on Linux");
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            anyhow::bail!("Unsupported platform for native capture")
        }
    }

    /// Check if ScreenCaptureKit is available on macOS
    #[cfg(target_os = "macos")]
    fn is_sck_available() -> bool {
        // Check macOS version >= 12.3
        use std::process::Command;

        if let Ok(output) = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            if let Ok(version) = String::from_utf8(output.stdout) {
                let version = version.trim();
                // Parse version (format: "14.1.0" or "12.3")
                if let Some(major) = version.split('.').next() {
                    if let Ok(major_num) = major.parse::<u32>() {
                        return major_num >= 12;
                    }
                }
            }
        }

        false
    }

    /// Check if DXGI is available on Windows
    #[cfg(target_os = "windows")]
    fn is_dxgi_available() -> bool {
        // Windows 8+ has DXGI Desktop Duplication
        // We can do a more sophisticated check later
        true
    }

    /// Check if PipeWire is available on Linux
    #[cfg(target_os = "linux")]
    fn is_pipewire_available() -> bool {
        // Check if PipeWire is running
        use std::process::Command;

        Command::new("pw-cli")
            .arg("info")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
