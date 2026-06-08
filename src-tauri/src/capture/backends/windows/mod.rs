//! Windows DXGI Desktop Duplication backend (stub - to be implemented)

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use super::*;

/// DXGI Desktop Duplication backend for Windows
pub struct DXGIBackend {
    frame_sender: Arc<Mutex<Option<broadcast::Sender<CapturedFrame>>>>,
    is_active: Arc<Mutex<bool>>,
}

impl DXGIBackend {
    pub fn new() -> Result<Self> {
        // TODO: Initialize Direct3D 11 device
        Ok(Self {
            frame_sender: Arc::new(Mutex::new(None)),
            is_active: Arc::new(Mutex::new(false)),
        })
    }
}

#[async_trait::async_trait]
impl NativeCaptureBackend for DXGIBackend {
    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>> {
        // TODO: Enumerate displays via DXGI
        anyhow::bail!("Windows DXGI backend not yet implemented")
    }

    async fn check_permissions(&self) -> Result<PermissionStatus> {
        // Windows doesn't require special permissions for screen capture
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
        // TODO: Implement DXGI capture
        anyhow::bail!("Windows DXGI backend not yet implemented")
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
}
