//! Linux source discovery for portal-based PipeWire capture.
//!
//! Wayland intentionally does not expose a global display/window list. The
//! desktop portal owns source selection, so these entries represent the two
//! choices the application can request. The compositor presents the concrete
//! screen or window picker when recording starts.

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
    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>> {
        Ok(vec![
            CaptureSourceInfo {
                id: 1,
                name: "Choose a screen when recording starts".to_string(),
                source_type: CaptureSourceType::Display,
                width: 0,
                height: 0,
                x: 0,
                y: 0,
                scale_factor: 1.0,
                is_primary: true,
                owner_name: String::new(),
            },
            CaptureSourceInfo {
                id: 2,
                name: "Choose a window when recording starts".to_string(),
                source_type: CaptureSourceType::Window,
                width: 0,
                height: 0,
                x: 0,
                y: 0,
                scale_factor: 1.0,
                is_primary: false,
                owner_name: String::new(),
            },
        ])
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

    async fn start_capture(&mut self, _config: CaptureConfig) -> Result<()> {
        anyhow::bail!("Linux capture is started through the desktop portal recorder")
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

    fn audio_receiver(&self) -> Option<broadcast::Receiver<CapturedAudio>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exposes_portal_source_choices_without_compositor_enumeration() {
        let backend = PipeWireBackend::new().unwrap();
        let sources = backend.enumerate_sources().await.unwrap();
        assert_eq!(sources.len(), 2);
        assert!(matches!(sources[0].source_type, CaptureSourceType::Display));
        assert!(matches!(sources[1].source_type, CaptureSourceType::Window));
    }
}
