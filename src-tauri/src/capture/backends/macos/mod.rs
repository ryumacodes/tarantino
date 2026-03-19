//! macOS ScreenCaptureKit backend
//!
//! This module provides native screen capture using ScreenCaptureKit (SCK),
//! available on macOS 12.3+. SCK provides:
//! - Low-latency screen capture
//! - Native cursor compositing
//! - System audio capture (macOS 13+)
//! - Per-application audio
//! - Efficient frame delivery via CMSampleBuffer
//! - Hardware-accelerated encoding support

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use super::*;

mod ffi;
mod sck_wrapper;

use ffi::*;

/// ScreenCaptureKit backend for macOS
pub struct ScreenCaptureKitBackend {
    /// Native SCK capture instance (Objective-C object)
    capture_instance: Arc<Mutex<Option<*mut std::ffi::c_void>>>,
    /// Frame broadcast channel
    frame_sender: Arc<Mutex<Option<broadcast::Sender<CapturedFrame>>>>,
    /// Audio broadcast channel
    audio_sender: Arc<Mutex<Option<broadcast::Sender<CapturedAudio>>>>,
    /// Whether capture is currently active
    is_active: Arc<Mutex<bool>>,
    /// Current session handle
    session_handle: Arc<Mutex<Option<CaptureSessionHandle>>>,
}

unsafe impl Send for ScreenCaptureKitBackend {}
unsafe impl Sync for ScreenCaptureKitBackend {}

impl ScreenCaptureKitBackend {
    /// Create a new ScreenCaptureKit backend
    pub fn new() -> Result<Self> {
        // Verify SCK is available
        if !Self::verify_sck_available() {
            anyhow::bail!("ScreenCaptureKit requires macOS 12.3 or later");
        }

        Ok(Self {
            capture_instance: Arc::new(Mutex::new(None)),
            frame_sender: Arc::new(Mutex::new(None)),
            audio_sender: Arc::new(Mutex::new(None)),
            is_active: Arc::new(Mutex::new(false)),
            session_handle: Arc::new(Mutex::new(None)),
        })
    }

    /// Verify ScreenCaptureKit is available
    fn verify_sck_available() -> bool {
        unsafe { ffi::sck_is_available() }
    }

    /// Convert native display info to CaptureSourceInfo
    fn convert_display_info(display: sck_wrapper::DisplayInfo) -> CaptureSourceInfo {
        CaptureSourceInfo {
            id: display.display_id,
            name: display.name,
            source_type: CaptureSourceType::Display,
            width: display.width,
            height: display.height,
            scale_factor: display.scale_factor,
            is_primary: display.is_primary,
        }
    }

    /// Convert native window info to CaptureSourceInfo
    fn convert_window_info(window: sck_wrapper::WindowInfo) -> CaptureSourceInfo {
        CaptureSourceInfo {
            id: window.window_id,
            name: window.title,
            source_type: CaptureSourceType::Window,
            width: window.width,
            height: window.height,
            scale_factor: 1.0, // Windows don't have independent scale
            is_primary: false,
        }
    }
}

#[async_trait::async_trait]
impl NativeCaptureBackend for ScreenCaptureKitBackend {
    fn backend_name(&self) -> &'static str {
        "ScreenCaptureKit"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            display_capture: true,
            window_capture: true,
            region_capture: true,
            cursor_capture: true,
            hdr_support: true, // SCK supports HDR on capable displays
            system_audio: Self::system_audio_supported(),
            app_audio: Self::system_audio_supported(), // Same version requirement
            hardware_acceleration: true,
            pixel_formats: vec![
                "BGRA".to_string(),
                "NV12".to_string(),
                "420v".to_string(), // YUV420
            ],
        }
    }

    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>> {
        let displays = sck_wrapper::get_shareable_displays()?;
        let windows = sck_wrapper::get_shareable_windows()?;

        let mut sources = Vec::new();

        // Add displays
        for display in displays {
            sources.push(Self::convert_display_info(display));
        }

        // Add windows
        for window in windows {
            sources.push(Self::convert_window_info(window));
        }

        // Sort: primary display first, then by ID
        sources.sort_by(|a, b| {
            match (a.is_primary, b.is_primary) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.id.cmp(&b.id),
            }
        });

        Ok(sources)
    }

    async fn check_permissions(&self) -> Result<PermissionStatus> {
        let status = sck_wrapper::check_permissions();
        Ok(PermissionStatus {
            screen_recording: status.screen_recording,
            microphone: status.microphone,
            camera: status.camera,
        })
    }

    async fn request_permissions(&self) -> Result<PermissionStatus> {
        let status = sck_wrapper::request_permissions();
        Ok(PermissionStatus {
            screen_recording: status.screen_recording,
            microphone: status.microphone,
            camera: status.camera,
        })
    }

    async fn start_capture(&mut self, config: CaptureConfig) -> Result<CaptureSessionHandle> {
        // Check permissions first
        let perms = self.check_permissions().await?;
        if !perms.screen_recording {
            anyhow::bail!("Screen recording permission not granted. Please enable in System Settings > Privacy & Security > Screen Recording");
        }

        // Create broadcast channels for frames and audio
        let (frame_sender, _) = broadcast::channel(100);
        let (audio_sender, _) = broadcast::channel(100);
        *self.frame_sender.lock().unwrap() = Some(frame_sender.clone());
        *self.audio_sender.lock().unwrap() = Some(audio_sender.clone());

        // Will build SCK FFI config after resolving source info (avoid holding !Send across await)

        // Get source info for session handle
        let sources = self.enumerate_sources().await?;
        let source_info = sources
            .into_iter()
            .find(|s| s.id == config.source_id)
            .ok_or_else(|| anyhow::anyhow!("Source not found: {}", config.source_id))?;

        // Create frame callback that sends to broadcast channel
        let frame_sender_clone = frame_sender.clone();
        let frame_callback = move |frame: SCKFrameData| {
            use std::ffi::CStr;

            // Validate pointer and size before creating slice
            if frame.data.is_null() {
                eprintln!("Warning: Received null frame data pointer, skipping frame");
                return;
            }

            if frame.data_len == 0 {
                eprintln!("Warning: Received zero-length frame data, skipping frame");
                return;
            }

            // Check that size doesn't exceed isize::MAX
            if frame.data_len > isize::MAX as usize {
                eprintln!("Warning: Frame data length exceeds isize::MAX, skipping frame");
                return;
            }

            // Convert C frame data to Rust types
            let data_slice = unsafe {
                std::slice::from_raw_parts(frame.data, frame.data_len)
            };

            let pixel_format = unsafe {
                if !frame.pixel_format.is_null() {
                    CStr::from_ptr(frame.pixel_format)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    "BGRA".to_string()
                }
            };

            let captured_frame = CapturedFrame {
                data: Bytes::copy_from_slice(data_slice),
                width: frame.width,
                height: frame.height,
                pixel_format,
                timestamp_us: frame.timestamp_us,
                stride: frame.stride,
            };

            // Send frame to broadcast channel (ignore if no receivers)
            let _ = frame_sender_clone.send(captured_frame);
        };

        // Create audio callback that sends to broadcast channel
        let audio_sender_clone = audio_sender.clone();
        let audio_callback = move |audio: SCKAudioData| {
            // Validate pointer and size before creating slice
            if audio.data.is_null() {
                // Audio data can be null if audio capture is not configured or unavailable
                // This is not an error - just skip silently
                return;
            }

            if audio.data_len == 0 {
                // Empty audio data, skip silently
                return;
            }

            // Check that size doesn't exceed isize::MAX
            if audio.data_len > isize::MAX as usize {
                eprintln!("Warning: Audio data length exceeds isize::MAX, skipping audio sample");
                return;
            }

            // Convert C audio data to Rust types
            let data_slice = unsafe {
                std::slice::from_raw_parts(audio.data, audio.data_len)
            };

            let captured_audio = CapturedAudio {
                data: Bytes::copy_from_slice(data_slice),
                sample_rate: audio.sample_rate,
                channels: audio.channels,
                timestamp_us: audio.timestamp_us,
            };

            // Send audio to broadcast channel (ignore if no receivers)
            let _ = audio_sender_clone.send(captured_audio);
        };

        // Build FFI config now to keep !Send pointer out of await regions
        let sck_config = SCKCaptureConfig {
            source_id: config.source_id,
            is_display: matches!(config.source_type, CaptureSourceType::Display),
            fps: config.fps,
            include_cursor: config.include_cursor,
            include_audio: config.include_audio,
            crop_x: config.region.as_ref().map(|r| r.x).unwrap_or(0),
            crop_y: config.region.as_ref().map(|r| r.y).unwrap_or(0),
            crop_width: config.region.as_ref().map(|r| r.width).unwrap_or(0),
            crop_height: config.region.as_ref().map(|r| r.height).unwrap_or(0),
            output_path: std::ptr::null(),
        };

        // Start native capture
        let capture_ptr = sck_wrapper::start_capture(sck_config, frame_callback, audio_callback)?;

        *self.capture_instance.lock().unwrap() = Some(capture_ptr);
        *self.is_active.lock().unwrap() = true;

        let session_id = uuid::Uuid::new_v4().to_string();
        let handle = CaptureSessionHandle {
            session_id: session_id.clone(),
            source_info,
        };

        *self.session_handle.lock().unwrap() = Some(handle.clone());

        Ok(handle)
    }

    async fn stop_capture(&mut self) -> Result<()> {
        let mut instance = self.capture_instance.lock().unwrap();
        if let Some(ptr) = instance.take() {
            sck_wrapper::stop_capture(ptr)?;
        }

        *self.is_active.lock().unwrap() = false;
        *self.frame_sender.lock().unwrap() = None;
        *self.audio_sender.lock().unwrap() = None;
        *self.session_handle.lock().unwrap() = None;

        Ok(())
    }

    fn frame_receiver(&self) -> Option<broadcast::Receiver<CapturedFrame>> {
        self.frame_sender
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.subscribe())
    }

    fn audio_receiver(&self) -> Option<broadcast::Receiver<CapturedAudio>> {
        self.audio_sender
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.subscribe())
    }

    fn is_active(&self) -> bool {
        *self.is_active.lock().unwrap()
    }
}

impl ScreenCaptureKitBackend {
    /// Check if system audio capture is supported (macOS 13+)
    #[allow(dead_code)]
    fn system_audio_supported() -> bool {
        use std::process::Command;

        if let Ok(output) = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            if let Ok(version) = String::from_utf8(output.stdout) {
                if let Some(major) = version.trim().split('.').next() {
                    if let Ok(major_num) = major.parse::<u32>() {
                        return major_num >= 13;
                    }
                }
            }
        }

        false
    }
}

impl Drop for ScreenCaptureKitBackend {
    fn drop(&mut self) {
        // Ensure capture is stopped - use synchronous cleanup to avoid executor conflicts
        let mut instance = self.capture_instance.lock().unwrap();
        if let Some(ptr) = instance.take() {
            let _ = sck_wrapper::stop_capture(ptr);
        }
        *self.is_active.lock().unwrap() = false;
    }
}
