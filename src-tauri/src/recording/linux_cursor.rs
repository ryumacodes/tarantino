//! Owns a PipeWire metadata consumer; pixels remain in the recording pipeline.
use anyhow::{Result, anyhow};
use std::{ffi::c_void, os::fd::AsRawFd, ptr::NonNull};

unsafe extern "C" {
    fn tarantino_cursor_start(
        fd: i32,
        node: u32,
        callback: extern "C" fn(u32, u32, u32, u32),
    ) -> *mut c_void;
    fn tarantino_cursor_stop(reader: *mut c_void);
}

pub(super) struct CursorReader(NonNull<c_void>);
// The C reader owns its thread loop; Rust only transfers ownership or joins
// that loop on drop. Its callback communicates through the input mutex.
unsafe impl Send for CursorReader {}

impl CursorReader {
    pub(super) fn start(fd: &impl AsRawFd, node: u32) -> Result<Self> {
        extern "C" fn position(width: u32, height: u32, x: u32, y: u32) {
            crate::input::observe_stream_pointer(width, height, Some((x, y)));
        }
        // The C constructor duplicates this dedicated portal connection.
        NonNull::new(unsafe { tarantino_cursor_start(fd.as_raw_fd(), node, position) })
            .map(Self)
            .ok_or_else(|| anyhow!("Failed to start Linux cursor metadata capture"))
    }
}

impl Drop for CursorReader {
    fn drop(&mut self) {
        // Joins the worker before releasing any callback state or portal access.
        unsafe { tarantino_cursor_stop(self.0.as_ptr()) };
    }
}
