//! Native webcam capture using AVFoundation (macOS)
//!
//! Two-phase lifecycle:
//! 1. Toggle camera ON → start AVCaptureSession + show preview window
//! 2. Start recording → hide preview, begin encoding frames to MP4
//! 3. Stop recording → stop encoding, finalize MP4
//! 4. Toggle camera OFF → stop capture session entirely

use std::os::raw::{c_char, c_void};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::broadcast;

/// Frame data from AVFoundation camera callback
#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct AVCFrameData {
    data: *const u8,
    data_len: usize,
    width: u32,
    height: u32,
    pixel_format: *const c_char,
    timestamp_us: u64,
    stride: u32,
}

extern "C" {
    fn avc_start_webcam(
        device_id: *const c_char,
        fps: i32,
        width: i32,
        height: i32,
        rust_context: *mut c_void,
        frame_callback: extern "C" fn(*mut c_void, AVCFrameData),
    ) -> *mut c_void;

    fn avc_start_recording(
        session_ptr: *mut c_void,
        rust_context: *mut c_void,
        frame_callback: extern "C" fn(*mut c_void, AVCFrameData),
    );

    fn avc_stop_recording(session_ptr: *mut c_void);
    fn avc_stop_webcam(session_ptr: *mut c_void);
    fn avc_check_camera_permission() -> bool;
    fn avc_request_camera_permission() -> bool;
}

#[derive(Clone, Debug)]
pub struct WebcamFrame {
    pub data: Bytes,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub timestamp_us: u64,
}

struct WebcamCallbackContext {
    sender: broadcast::Sender<WebcamFrame>,
}

extern "C" fn webcam_frame_callback(context: *mut c_void, frame: AVCFrameData) {
    unsafe {
        let ctx = &*(context as *const WebcamCallbackContext);
        let data = std::slice::from_raw_parts(frame.data, frame.data_len);
        let _ = ctx.sender.send(WebcamFrame {
            data: Bytes::copy_from_slice(data),
            width: frame.width,
            height: frame.height,
            stride: frame.stride,
            timestamp_us: frame.timestamp_us,
        });
    }
}

/// Active webcam capture session (preview + optional recording)
pub struct WebcamCapture {
    session_ptr: *mut c_void,
    /// Held alive while recording to keep the channel open
    recording_context: Option<Box<WebcamCallbackContext>>,
}

unsafe impl Send for WebcamCapture {}

impl WebcamCapture {
    /// Start the camera and show preview. No frames are sent to Rust yet.
    pub fn start(device_id: Option<&str>, fps: u32) -> Option<Self> {
        let has_permission = unsafe { avc_check_camera_permission() };
        if !has_permission {
            println!("[Webcam] Requesting camera permission...");
            let granted = unsafe { avc_request_camera_permission() };
            if !granted {
                println!("[Webcam] Camera permission denied");
                return None;
            }
        }

        let device_cstr = device_id.and_then(|d| std::ffi::CString::new(d).ok());
        let device_ptr = device_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());

        let session_ptr = unsafe {
            avc_start_webcam(
                device_ptr,
                fps as i32,
                1280, 720,
                std::ptr::null_mut(),
                webcam_frame_callback, // not used until start_recording
            )
        };

        if session_ptr.is_null() {
            println!("[Webcam] Failed to start camera capture");
            return None;
        }

        println!("[Webcam] Camera started with preview");
        Some(Self { session_ptr, recording_context: None })
    }

    /// Begin recording frames. Returns a receiver for encoded frame data.
    /// Hides the preview window.
    pub fn start_recording(&mut self) -> broadcast::Receiver<WebcamFrame> {
        let (tx, rx) = broadcast::channel(30);
        let context = Box::new(WebcamCallbackContext { sender: tx });
        let context_ptr = &*context as *const WebcamCallbackContext as *mut c_void;

        unsafe {
            avc_start_recording(self.session_ptr, context_ptr, webcam_frame_callback);
        }

        self.recording_context = Some(context);
        println!("[Webcam] Recording started (preview hidden)");
        rx
    }

    /// Stop recording frames. The capture session stays alive.
    pub fn stop_recording(&mut self) {
        if self.recording_context.is_some() {
            unsafe { avc_stop_recording(self.session_ptr) };
            self.recording_context = None;
            println!("[Webcam] Recording stopped");
        }
    }

    /// Stop the camera entirely (capture + preview).
    pub fn stop(&mut self) {
        if !self.session_ptr.is_null() {
            unsafe { avc_stop_webcam(self.session_ptr) };
            self.session_ptr = std::ptr::null_mut();
            self.recording_context = None;
            println!("[Webcam] Camera stopped");
        }
    }
}

impl Drop for WebcamCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Spawn the webcam encoding task. Encodes frames and writes to MP4.
pub fn spawn_webcam_task(
    mut frame_rx: broadcast::Receiver<WebcamFrame>,
    output_path: PathBuf,
    fps: u32,
    stop_signal: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<Result<String, String>> {
    tokio::task::spawn_blocking(move || {
        use crate::encoder::{Encoder, EncoderConfig, VideoCodec, Container};
        use crate::muxer::Mp4Muxer;

        let mut encoder: Option<Encoder> = None;
        let mut muxer: Option<Mp4Muxer> = None;
        let mut frame_count: u64 = 0;
        let mut prev_pts_us: Option<u64> = None;
        let out_str = output_path.to_str().unwrap_or("/tmp/webcam.mp4").to_string();

        println!("[Webcam] Encoding task started → {}", out_str);

        loop {
            if stop_signal.load(Ordering::Relaxed) {
                println!("[Webcam] Stop signal after {} frames", frame_count);
                break;
            }

            let frame = match frame_rx.try_recv() {
                Ok(f) => f,
                Err(broadcast::error::TryRecvError::Empty) => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(broadcast::error::TryRecvError::Closed) => break,
            };

            if encoder.is_none() {
                let config = EncoderConfig {
                    width: frame.width,
                    height: frame.height,
                    fps,
                    bitrate: 2_000_000,
                    codec: VideoCodec::H264,
                    container: Container::Mp4,
                    hardware_accel: true,
                };
                let mut enc = Encoder::new(config, &out_str)
                    .map_err(|e| format!("Encoder init: {}", e))?;
                enc.start().map_err(|e| format!("Encoder start: {}", e))?;
                encoder = Some(enc);

                let mux = Mp4Muxer::new(&out_str, frame.width, frame.height, fps)
                    .map_err(|e| format!("Muxer init: {}", e))?;
                muxer = Some(mux);

                println!("[Webcam] Pipeline: {}x{} @ {}fps", frame.width, frame.height, fps);
            }

            let enc = encoder.as_mut().unwrap();
            let mux = muxer.as_mut().unwrap();

            if let Err(e) = enc.encode_frame(
                &frame.data, frame.width, frame.height, frame.stride, "BGRA", frame.timestamp_us,
            ) {
                if frame_count < 5 { println!("[Webcam] Encode err: {}", e); }
                continue;
            }

            while let Some(encoded) = enc.try_receive_frame() {
                let duration_ms = if let Some(prev) = prev_pts_us {
                    ((encoded.timestamp_us.saturating_sub(prev)) / 1000) as u32
                } else {
                    1000 / fps
                };
                prev_pts_us = Some(encoded.timestamp_us);
                let _ = mux.write_frame(&encoded, duration_ms.max(1));
            }

            frame_count += 1;
            if frame_count % 300 == 0 {
                println!("[Webcam] {} frames encoded", frame_count);
            }
        }

        // Drain remaining
        if let Some(enc) = encoder.as_ref() {
            if let Some(mux) = muxer.as_mut() {
                while let Some(encoded) = enc.try_receive_frame() {
                    let duration_ms = prev_pts_us.map(|prev| {
                        ((encoded.timestamp_us.saturating_sub(prev)) / 1000) as u32
                    }).unwrap_or(1000 / fps);
                    prev_pts_us = Some(encoded.timestamp_us);
                    let _ = mux.write_frame(&encoded, duration_ms.max(1));
                }
            }
        }

        if let Some(mux) = muxer {
            if let Err(e) = mux.finish() {
                println!("[Webcam] Muxer error: {}", e);
            }
        }

        println!("[Webcam] Done: {} ({} frames)", out_str, frame_count);
        Ok(out_str)
    })
}
