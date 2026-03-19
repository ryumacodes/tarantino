

//! FFI declarations for VideoToolbox encoder

use std::os::raw::{c_char, c_void};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VTEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,        // in bps
    pub keyframe_interval: u32, // in frames
    pub quality: u32,        // 0-100
    pub enable_realtime: bool,
    pub profile: *const c_char, // "baseline", "main", "high"
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VTEncodedFrame {
    pub data: *const u8,
    pub data_len: usize,
    pub timestamp_us: u64,
    pub is_keyframe: bool,
    pub pts: i64,
    pub dts: i64,
    pub sps_data: *const u8,
    pub sps_len: usize,
    pub pps_data: *const u8,
    pub pps_len: usize,
}

/// Frame callback function pointer type
pub type VTFrameCallbackFn = extern "C" fn(context: *mut c_void, frame: VTEncodedFrame);

#[link(name = "VideoToolbox", kind = "framework")]
#[link(name = "CoreVideo", kind = "framework")]
#[link(name = "CoreMedia", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    /// Check if VideoToolbox is available
    pub fn vt_encoder_is_available() -> bool;

    /// Create and configure VideoToolbox encoder
    pub fn vt_encoder_create(
        config: VTEncoderConfig,
        rust_context: *mut c_void,
        callback: VTFrameCallbackFn,
    ) -> *mut c_void;

    /// Encode a frame
    pub fn vt_encoder_encode_frame(
        instance: *mut c_void,
        frame_data: *const u8,
        data_len: usize,
        width: u32,
        height: u32,
        stride: u32,
        pixel_format: *const c_char,
        timestamp_us: u64,
    ) -> bool;

    /// Flush pending frames
    pub fn vt_encoder_flush(instance: *mut c_void) -> bool;

    /// Destroy encoder and release resources
    pub fn vt_encoder_destroy(instance: *mut c_void);
}
