//! FFI bindings for ScreenCaptureKit Objective-C wrapper
//!
//! This module defines the C-compatible interface between Rust and the
//! Objective-C++ ScreenCaptureKit wrapper.

use std::os::raw::{c_char, c_void};

/// Display information from SCShareableContent
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SCKDisplay {
    pub display_id: u64,
    pub name: *const c_char,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

/// Window information from SCShareableContent
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SCKWindow {
    pub window_id: u64,
    pub title: *const c_char,
    pub width: u32,
    pub height: u32,
    pub owner_name: *const c_char,
}

/// Permission status from TCC
#[repr(C)]
#[derive(Debug, Clone)]
pub struct SCKPermissionStatus {
    pub screen_recording: bool,
    pub microphone: bool,
    pub camera: bool,
}

/// Capture configuration
#[repr(C)]
#[derive(Debug, Clone)]
pub struct SCKCaptureConfig {
    /// Source ID (display ID or window CGWindowID)
    pub source_id: u64,
    /// True for display, false for window
    pub is_display: bool,
    /// Target frame rate
    pub fps: u32,
    /// Include cursor in capture
    pub include_cursor: bool,
    /// Include system/app audio
    pub include_audio: bool,
    /// Crop region (0 = no crop)
    pub crop_x: u32,
    pub crop_y: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    /// Optional output path for encoded file (UTF-8 string)
    pub output_path: *const c_char,
}

/// Frame data delivered from SCK
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SCKFrameData {
    /// Raw pixel data pointer
    pub data: *const u8,
    /// Data length in bytes
    pub data_len: usize,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Pixel format ("BGRA", "NV12", etc.)
    pub pixel_format: *const c_char,
    /// Timestamp in microseconds
    pub timestamp_us: u64,
    /// Bytes per row
    pub stride: u32,
}

/// Audio data delivered from SCK
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SCKAudioData {
    /// Raw audio data pointer
    pub data: *const u8,
    /// Data length in bytes
    pub data_len: usize,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u32,
    /// Timestamp in microseconds
    pub timestamp_us: u64,
}

// External functions implemented in Objective-C++ (sck_wrapper.mm)
extern "C" {
    /// Check if ScreenCaptureKit is available
    pub fn sck_is_available() -> bool;

    /// Get all shareable displays
    /// Returns true on success, populates out_displays and out_count
    pub fn sck_get_shareable_displays(
        out_displays: *mut *mut SCKDisplay,
        out_count: *mut usize,
    ) -> bool;

    /// Get all shareable windows
    /// Returns true on success, populates out_windows and out_count
    pub fn sck_get_shareable_windows(
        out_windows: *mut *mut SCKWindow,
        out_count: *mut usize,
    ) -> bool;

    /// Check current permission status
    pub fn sck_check_permissions() -> SCKPermissionStatus;

    /// Request permissions (may trigger system prompt)
    pub fn sck_request_permissions() -> SCKPermissionStatus;

    /// Start capture session
    /// Returns opaque pointer to capture instance, or null on failure
    pub fn sck_start_capture(
        config: SCKCaptureConfig,
        context: *mut c_void,
        frame_callback: extern "C" fn(*mut c_void, SCKFrameData),
        audio_callback: extern "C" fn(*mut c_void, SCKAudioData),
    ) -> *mut c_void;

    /// Stop capture session and return the callback context for cleanup
    /// Returns the rust_context pointer via out_rust_context that needs to be freed by Rust
    /// Returns true on success
    pub fn sck_stop_capture(instance: *mut c_void, out_rust_context: *mut *mut c_void) -> bool;

    /// Free display array allocated by sck_get_shareable_displays
    pub fn sck_free_displays(displays: *mut SCKDisplay, count: usize);

    /// Free window array allocated by sck_get_shareable_windows
    pub fn sck_free_windows(windows: *mut SCKWindow, count: usize);
}
