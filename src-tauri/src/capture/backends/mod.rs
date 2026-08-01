//! Native capture backends
//!
//! This module provides platform-specific native capture implementations:
//! - macOS: ScreenCaptureKit (SCK)
//! - Windows: Desktop Duplication API (DXGI)
//! - Linux: PipeWire via xdg-desktop-portal
//!
//! Each backend implements the `NativeCaptureBackend` trait for a unified interface.

use anyhow::Result;
use tokio::sync::broadcast;

pub use super::types::*;

// Platform-specific backend modules
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

// Optional xcap fallback (disabled; remove cfg until feature is added)
// pub mod xcap_fallback;

/// Unified trait for native capture backends
#[async_trait::async_trait]
pub trait NativeCaptureBackend: Send + Sync {
    /// Enumerate available capture sources (displays + windows)
    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>>;

    /// Check if required permissions are granted
    async fn check_permissions(&self) -> Result<PermissionStatus>;

    /// Request permissions (may show system prompt)
    async fn request_permissions(&self) -> Result<PermissionStatus>;

    /// Start capture session with given configuration
    async fn start_capture(&mut self, config: CaptureConfig) -> Result<()>;

    /// Stop active capture session
    async fn stop_capture(&mut self) -> Result<()>;

    /// Get frame stream (broadcast channel receiver)
    fn frame_receiver(&self) -> Option<broadcast::Receiver<CapturedFrame>>;

    /// Get audio stream (broadcast channel receiver)
    fn audio_receiver(&self) -> Option<broadcast::Receiver<CapturedAudio>>;
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
            println!("Using the XDG desktop portal and PipeWire backend");
            return Ok(Box::new(linux::PipeWireBackend::new()?));
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

        if let Ok(output) = Command::new("sw_vers").arg("-productVersion").output() {
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
}
