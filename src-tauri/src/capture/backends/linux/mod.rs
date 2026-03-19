//! Linux PipeWire backend (stub - to be implemented)

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use super::*;

/// PipeWire backend for Linux
pub struct PipeWireBackend {
    frame_sender: Arc<Mutex<Option<broadcast::Sender<CapturedFrame>>>>,
    is_active: Arc<Mutex<bool>>,
}

impl PipeWireBackend {
    pub fn new() -> Result<Self> {
        // TODO: Connect to PipeWire daemon
        Ok(Self {
            frame_sender: Arc::new(Mutex::new(None)),
            is_active: Arc::new(Mutex::new(false)),
        })
    }
}

#[async_trait::async_trait]
impl NativeCaptureBackend for PipeWireBackend {
    fn backend_name(&self) -> &'static str {
        "PipeWire"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            display_capture: true,
            window_capture: true,
            region_capture: true,
            cursor_capture: true,
            hdr_support: false,
            system_audio: true,
            app_audio: true,
            hardware_acceleration: false, // Depends on system
            pixel_formats: vec!["BGRA".to_string(), "RGB".to_string()],
        }
    }

    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>> {
        // TODO: Enumerate sources via PipeWire
        anyhow::bail!("Linux PipeWire backend not yet implemented")
    }

    async fn check_permissions(&self) -> Result<PermissionStatus> {
        // Linux permissions handled via xdg-desktop-portal
        Ok(PermissionStatus {
            screen_recording: true,
            microphone: true,
            camera: true,
        })
    }

    async fn request_permissions(&self) -> Result<PermissionStatus> {
        self.check_permissions().await
    }

    async fn start_capture(&mut self, _config: CaptureConfig) -> Result<CaptureSessionHandle> {
        // TODO: Implement PipeWire capture
        anyhow::bail!("Linux PipeWire backend not yet implemented")
    }

    async fn stop_capture(&mut self) -> Result<()> {
        *self.is_active.lock().unwrap() = false;
        Ok(())
    }

    fn frame_receiver(&self) -> Option<broadcast::Receiver<CapturedFrame>> {
        self.frame_sender
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.subscribe())
    }

    fn is_active(&self) -> bool {
        *self.is_active.lock().unwrap()
    }
}
