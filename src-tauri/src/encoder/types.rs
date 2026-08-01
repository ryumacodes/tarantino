//! Platform-neutral video encoder contracts.

use bytes::Bytes;

#[derive(Clone)]
pub struct EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub hardware_accel: bool,
}

/// Compressed video frame produced by any platform encoder.
///
/// The current MP4 pipeline expects H.264 data in AVCC form. Keeping this
/// contract outside the VideoToolbox module prevents recording and muxing from
/// depending on a macOS implementation type.
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    pub data: Bytes,
    pub timestamp_us: u64,
    pub is_keyframe: bool,
    pub pts: i64,
    pub dts: i64,
    pub sps: Option<Bytes>,
    pub pps: Option<Bytes>,
}
