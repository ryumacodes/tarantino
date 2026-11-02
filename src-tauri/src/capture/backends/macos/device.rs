//! USB device screen capture through AVFoundation, using the shared encoder.
use super::*;
use std::ffi::{CStr, c_char, c_void};

#[repr(C)]
struct DeviceFrame {
    data: *const u8,
    data_len: usize,
    width: u32,
    height: u32,
    pixel_format: *const c_char,
    timestamp_us: u64,
    stride: u32,
}
unsafe extern "C" {
    fn avc_enumerate_screen_devices(
        context: *mut c_void,
        callback: extern "C" fn(*mut c_void, *const c_char, *const c_char, bool),
    ) -> usize;
    fn avc_start_screen_device(
        id: *const c_char,
        fps: i32,
        context: *mut c_void,
        callback: extern "C" fn(*mut c_void, DeviceFrame),
    ) -> *mut c_void;
    fn avc_stop_webcam(session: *mut c_void);
}

#[derive(serde::Serialize)]
pub struct ScreenDevice {
    pub id: String,
    pub name: String,
}
extern "C" fn discovered(ctx: *mut c_void, id: *const c_char, name: *const c_char, _: bool) {
    if ctx.is_null() || id.is_null() || name.is_null() {
        return;
    }
    unsafe {
        (&mut *(ctx as *mut Vec<ScreenDevice>)).push(ScreenDevice {
            id: CStr::from_ptr(id).to_string_lossy().into_owned(),
            name: CStr::from_ptr(name).to_string_lossy().into_owned(),
        });
    }
}
pub fn enumerate() -> Vec<ScreenDevice> {
    let mut devices = Vec::new();
    unsafe {
        avc_enumerate_screen_devices(&mut devices as *mut Vec<ScreenDevice> as _, discovered);
    }
    devices
}
extern "C" fn received(ctx: *mut c_void, frame: DeviceFrame) {
    if ctx.is_null()
        || frame.data.is_null()
        || frame.data_len == 0
        || frame.data_len > isize::MAX as usize
    {
        return;
    }
    unsafe {
        let sender = &*(ctx as *const broadcast::Sender<CapturedFrame>);
        let _ = sender.send(CapturedFrame {
            data: Bytes::copy_from_slice(std::slice::from_raw_parts(frame.data, frame.data_len)),
            width: frame.width,
            height: frame.height,
            stride: frame.stride,
            pixel_format: "BGRA".into(),
            timestamp_us: frame.timestamp_us,
            captured_at: std::time::SystemTime::now(),
        });
    }
}

pub struct DeviceBackend {
    id: String,
    session: *mut c_void,
    // Stable address until native stop drains the serial callback queue.
    sender: Box<broadcast::Sender<CapturedFrame>>,
}
unsafe impl Send for DeviceBackend {}
unsafe impl Sync for DeviceBackend {}
impl DeviceBackend {
    pub fn new(id: String) -> Self {
        let (sender, _) = broadcast::channel(8);
        Self {
            id,
            session: std::ptr::null_mut(),
            sender: Box::new(sender),
        }
    }
    fn stop(&mut self) {
        if !self.session.is_null() {
            unsafe {
                avc_stop_webcam(self.session);
            }
            self.session = std::ptr::null_mut();
        }
    }
}
#[async_trait::async_trait]
impl NativeCaptureBackend for DeviceBackend {
    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>> {
        Ok(Vec::new())
    }
    async fn check_permissions(&self) -> Result<PermissionStatus> {
        let p = sck_wrapper::check_permissions();
        Ok(PermissionStatus {
            screen_recording: p.screen_recording,
            microphone: p.microphone,
            camera: p.camera,
        })
    }
    async fn request_permissions(&self) -> Result<PermissionStatus> {
        self.check_permissions().await
    }
    async fn start_capture(&mut self, config: CaptureConfig) -> Result<()> {
        anyhow::ensure!(self.session.is_null(), "Device capture is already active");
        anyhow::ensure!(
            !config.include_audio,
            "USB device audio capture is not available yet. Turn off system audio for device recordings."
        );
        anyhow::ensure!(
            enumerate().iter().any(|d| d.id == self.id),
            "Device disconnected. Connect by USB, unlock it, tap Trust, then select it again."
        );
        let id = CString::new(self.id.as_str())?;
        self.session = unsafe {
            avc_start_screen_device(
                id.as_ptr(),
                config.fps as i32,
                &*self.sender as *const broadcast::Sender<CapturedFrame> as *mut c_void,
                received,
            )
        };
        anyhow::ensure!(
            !self.session.is_null(),
            "Could not start device screen capture. Unlock the device and close other capture apps."
        );
        Ok(())
    }
    async fn stop_capture(&mut self) -> Result<()> {
        self.stop();
        Ok(())
    }
    fn frame_receiver(&self) -> Option<broadcast::Receiver<CapturedFrame>> {
        Some(self.sender.subscribe())
    }
    fn audio_receiver(&self) -> Option<broadcast::Receiver<CapturedAudio>> {
        None
    }
}
impl Drop for DeviceBackend {
    fn drop(&mut self) {
        self.stop();
    }
}
