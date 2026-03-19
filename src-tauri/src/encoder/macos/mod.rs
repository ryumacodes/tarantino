//! macOS VideoToolbox encoder implementation
//!
//! This module provides:
//! - Hardware-accelerated H.264 encoding using Apple's VideoToolbox framework

mod ffi;

use anyhow::Result;
use bytes::Bytes;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use super::{EncoderConfig, VideoCodec};
use ffi::*;

/// Encoded frame output from VideoToolbox
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    /// Compressed frame data (H.264 NAL units)
    pub data: Bytes,
    /// Presentation timestamp in microseconds
    pub timestamp_us: u64,
    /// Whether this is a keyframe (IDR frame)
    pub is_keyframe: bool,
    /// Presentation timestamp (in encoder timescale)
    pub pts: i64,
    /// Decode timestamp (in encoder timescale)
    pub dts: i64,
    /// SPS (Sequence Parameter Set) data - extracted from format description
    pub sps: Option<Bytes>,
    /// PPS (Picture Parameter Set) data - extracted from format description
    pub pps: Option<Bytes>,
}

/// VideoToolbox encoder wrapper
#[allow(dead_code)]
pub struct VideoToolboxEncoder {
    /// Native encoder instance pointer
    encoder_ptr: *mut c_void,
    /// Configuration
    config: EncoderConfig,
    /// Channel for receiving encoded frames
    encoded_frame_rx: Arc<Mutex<mpsc::UnboundedReceiver<EncodedFrame>>>,
    /// Channel for sending encoded frames (held to keep channel alive)
    _encoded_frame_tx: Arc<Mutex<mpsc::UnboundedSender<EncodedFrame>>>,
    /// Profile string (kept alive for FFI)
    _profile_cstring: CString,
    /// Track whether encoder has been flushed (to prevent double-flush in Drop)
    flushed: bool,
}

unsafe impl Send for VideoToolboxEncoder {}
unsafe impl Sync for VideoToolboxEncoder {}

impl VideoToolboxEncoder {
    /// Create a new VideoToolbox encoder
    pub fn new(config: EncoderConfig) -> Result<Self> {
        // Verify VideoToolbox is available
        if !unsafe { vt_encoder_is_available() } {
            anyhow::bail!("VideoToolbox is not available on this system");
        }

        // Create channel for encoded frames
        let (tx, rx) = mpsc::unbounded_channel();
        let tx = Arc::new(Mutex::new(tx));
        let rx = Arc::new(Mutex::new(rx));

        // Convert profile to C string
        let profile_str = match config.codec {
            VideoCodec::H264 => "main", // Default to main profile
            VideoCodec::H265 => "main", // HEVC main profile
            _ => "baseline",
        };
        let profile_cstring = CString::new(profile_str)?;

        // Calculate bitrate based on resolution and quality
        let bitrate = Self::calculate_bitrate(&config);

        // Build VT config
        let vt_config = VTEncoderConfig {
            width: config.width,
            height: config.height,
            fps: config.fps,
            bitrate,
            keyframe_interval: config.fps, // Keyframe every 1 second for better error recovery and seeking
            quality: 80, // High quality (0-100 scale)
            enable_realtime: !config.hardware_accel, // Use realtime mode for lower latency
            profile: profile_cstring.as_ptr(),
        };

        // Create context for callback
        let context = Box::into_raw(Box::new(tx.clone())) as *mut c_void;

        // Create encoder
        let encoder_ptr = unsafe {
            vt_encoder_create(
                vt_config,
                context,
                Self::frame_callback,
            )
        };

        if encoder_ptr.is_null() {
            // Clean up context
            unsafe { drop(Box::from_raw(context as *mut Arc<Mutex<mpsc::UnboundedSender<EncodedFrame>>>)); }
            anyhow::bail!("Failed to create VideoToolbox encoder");
        }

        println!("VideoToolbox encoder created: {}x{} @ {} fps, {} bps",
                config.width, config.height, config.fps, bitrate);

        Ok(Self {
            encoder_ptr,
            config,
            encoded_frame_rx: rx,
            _encoded_frame_tx: tx,
            _profile_cstring: profile_cstring,
            flushed: false,
        })
    }

    /// Calculate bitrate based on resolution and quality preset
    fn calculate_bitrate(config: &EncoderConfig) -> u32 {
        let pixels = config.width * config.height;
        let base_bitrate = match pixels {
            ..=921600 => 2_000_000,      // 720p or lower: 2 Mbps
            ..=2073600 => 5_000_000,     // 1080p: 5 Mbps
            ..=3686400 => 10_000_000,    // 1440p: 10 Mbps
            _ => 20_000_000,              // 4K+: 20 Mbps
        };

        // Adjust for quality preset (bitrate override if set)
        if config.bitrate > 0 {
            config.bitrate
        } else {
            base_bitrate
        }
    }

    /// Validate encoded frame before passing to muxer
    fn validate_encoded_frame(frame: &EncodedFrame) -> Result<()> {
        // Check if frame data is empty
        if frame.data.is_empty() {
            anyhow::bail!("Encoded frame has no data");
        }

        // Check minimum frame size (should be at least a few bytes for a valid NAL unit)
        if frame.data.len() < 4 {
            anyhow::bail!("Encoded frame is too small ({} bytes)", frame.data.len());
        }

        // Validate AVCC format (4-byte length prefix)
        if frame.data.len() >= 4 {
            let nal_len = u32::from_be_bytes([
                frame.data[0],
                frame.data[1],
                frame.data[2],
                frame.data[3],
            ]) as usize;

            // NAL length should be reasonable (not larger than frame data minus the length prefix)
            if nal_len > frame.data.len() - 4 {
                anyhow::bail!(
                    "Invalid NAL unit length: {} exceeds frame data size {} - 4",
                    nal_len,
                    frame.data.len()
                );
            }

            // NAL length should not be zero
            if nal_len == 0 {
                anyhow::bail!("NAL unit has zero length");
            }

            // Check NAL unit type (5 bits, should be valid H.264 type)
            if frame.data.len() >= 5 {
                let nal_type = frame.data[4] & 0x1F;
                // Valid H.264 NAL unit types: 1-12, 14-18, 19-20
                // 0, 13, 21-31 are invalid or reserved
                if nal_type == 0 || nal_type == 13 || nal_type > 20 {
                    anyhow::bail!("Invalid NAL unit type: {}", nal_type);
                }
            }
        }

        // Validate timestamps
        if frame.pts < 0 || frame.dts < 0 {
            anyhow::bail!("Invalid timestamps: PTS={}, DTS={}", frame.pts, frame.dts);
        }

        Ok(())
    }

    /// Encode a single frame
    pub fn encode_frame(&mut self, frame_data: &[u8], width: u32, height: u32, stride: u32, pixel_format: &str, timestamp_us: u64) -> Result<()> {
        let format_cstring = CString::new(pixel_format)?;

        let success = unsafe {
            vt_encoder_encode_frame(
                self.encoder_ptr,
                frame_data.as_ptr(),
                frame_data.len(),
                width,
                height,
                stride,
                format_cstring.as_ptr(),
                timestamp_us,
            )
        };

        if !success {
            anyhow::bail!("Failed to encode frame");
        }

        Ok(())
    }

    /// Flush any pending frames
    pub fn flush(&mut self) -> Result<()> {
        // Skip if already flushed
        if self.flushed {
            println!("VideoToolbox encoder already flushed, skipping");
            return Ok(());
        }

        let success = unsafe { vt_encoder_flush(self.encoder_ptr) };

        if !success {
            anyhow::bail!("Failed to flush encoder");
        }

        // Mark as flushed to prevent double-flush in Drop
        self.flushed = true;
        println!("VideoToolbox encoder flushed successfully");

        Ok(())
    }

    /// Get the next encoded frame (non-blocking)
    pub fn try_receive_frame(&self) -> Option<EncodedFrame> {
        self.encoded_frame_rx.lock().unwrap().try_recv().ok()
    }

    /// Get the next encoded frame (async, blocking)
    pub async fn receive_frame(&self) -> Option<EncodedFrame> {
        // Try to get frame without blocking
        let result = {
            let mut rx_guard = self.encoded_frame_rx.lock().unwrap();
            rx_guard.try_recv().ok()
        }; // Guard dropped here

        if result.is_some() {
            return result;
        }

        // If no frame available, sleep briefly and retry
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

        // Try one more time after sleep
        {
            let mut rx_guard = self.encoded_frame_rx.lock().unwrap();
            rx_guard.try_recv().ok()
        } // Guard dropped here
    }

    /// Callback from VideoToolbox when a frame is encoded
    extern "C" fn frame_callback(context: *mut c_void, frame: VTEncodedFrame) {
        if context.is_null() {
            return;
        }

        // Reconstruct sender from context
        let tx = unsafe { &*(context as *const Arc<Mutex<mpsc::UnboundedSender<EncodedFrame>>>) };

        // Copy frame data (VideoToolbox owns the buffer, so we must copy)
        let data = unsafe {
            std::slice::from_raw_parts(frame.data, frame.data_len)
        };

        // Copy SPS and PPS if provided
        let sps = if !frame.sps_data.is_null() && frame.sps_len > 0 {
            let sps_slice = unsafe {
                std::slice::from_raw_parts(frame.sps_data, frame.sps_len)
            };
            Some(Bytes::copy_from_slice(sps_slice))
        } else {
            None
        };

        let pps = if !frame.pps_data.is_null() && frame.pps_len > 0 {
            let pps_slice = unsafe {
                std::slice::from_raw_parts(frame.pps_data, frame.pps_len)
            };
            Some(Bytes::copy_from_slice(pps_slice))
        } else {
            None
        };

        let encoded_frame = EncodedFrame {
            data: Bytes::copy_from_slice(data),
            timestamp_us: frame.timestamp_us,
            is_keyframe: frame.is_keyframe,
            pts: frame.pts,
            dts: frame.dts,
            sps,
            pps,
        };

        // Validate frame before sending to prevent corrupted frames from reaching the muxer
        if let Err(e) = Self::validate_encoded_frame(&encoded_frame) {
            println!("[ENCODER ERROR] Dropping invalid encoded frame: {}", e);
            println!("[ENCODER ERROR] Frame details - size: {}, keyframe: {}, pts: {}, dts: {}",
                     encoded_frame.data.len(), encoded_frame.is_keyframe,
                     encoded_frame.pts, encoded_frame.dts);
            return; // Drop this frame
        }

        // Send to channel (ignore errors if receiver is dropped)
        let _ = tx.lock().unwrap().send(encoded_frame);
    }

    /// Get encoder configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &EncoderConfig {
        &self.config
    }
}

impl Drop for VideoToolboxEncoder {
    fn drop(&mut self) {
        // Destroy encoder (flush only if not already flushed)
        if !self.encoder_ptr.is_null() {
            unsafe {
                // Only flush if not already flushed
                // This prevents double-flush which can cause hangs
                if !self.flushed {
                    println!("VideoToolbox encoder: flushing in Drop (not explicitly flushed)");
                    vt_encoder_flush(self.encoder_ptr);
                } else {
                    println!("VideoToolbox encoder: skipping flush in Drop (already flushed)");
                }
                vt_encoder_destroy(self.encoder_ptr);
            }
            self.encoder_ptr = ptr::null_mut();
        }

        println!("VideoToolbox encoder destroyed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitrate_calculation() {
        let config_720p = EncoderConfig {
            width: 1280,
            height: 720,
            fps: 30,
            bitrate: 0,
            codec: VideoCodec::H264,
            container: super::super::Container::Mp4,
            hardware_accel: true,
        };

        let bitrate = VideoToolboxEncoder::calculate_bitrate(&config_720p);
        assert_eq!(bitrate, 2_000_000);

        let config_1080p = EncoderConfig {
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 0,
            codec: VideoCodec::H264,
            container: super::super::Container::Mp4,
            hardware_accel: true,
        };

        let bitrate = VideoToolboxEncoder::calculate_bitrate(&config_1080p);
        assert_eq!(bitrate, 5_000_000);
    }
}
