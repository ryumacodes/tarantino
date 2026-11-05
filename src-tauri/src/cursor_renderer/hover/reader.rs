use super::Frame;
use anyhow::{Context, Result, anyhow, ensure};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::Path,
    ptr::NonNull,
};

#[repr(C)]
struct FrameData {
    y: *const u8,
    uv: *const u8,
    width: usize,
    height: usize,
    y_stride: usize,
    uv_stride: usize,
    time_ms: f64,
}

unsafe extern "C" {
    fn tarantino_hover_reader_open(
        path: *const c_char,
        width: u32,
        height: u32,
        start_ms: f64,
        end_ms: f64,
        error: *mut c_char,
        capacity: usize,
    ) -> *mut c_void;
    fn tarantino_hover_reader_next(
        handle: *mut c_void,
        frame: *mut FrameData,
        error: *mut c_char,
        capacity: usize,
    ) -> i32;
    fn tarantino_hover_reader_close(handle: *mut c_void);
}

pub struct Reader(NonNull<c_void>);

impl Reader {
    pub fn open(path: &Path, width: u32, height: u32) -> Result<Self> {
        Self::open_range(path, width, height, 0.0, f64::INFINITY)
    }

    // A frame spanning the seek boundary is clipped to start_ms. Keep its pixels
    // as warmup history when a variable-rate recording holds a frame across an idle gap.
    pub fn open_range(path: &Path, width: u32, height: u32, start_ms: f64, end_ms: f64) -> Result<Self> {
        ensure!(width > 0 && height > 0, "Invalid video decode size");
        ensure!(start_ms.is_finite() && start_ms >= 0.0 && end_ms > start_ms,
            "Invalid video decode range");
        let path = CString::new(path.to_str().context("Video path is not UTF-8")?)?;
        let mut error = [0 as c_char; 2048];
        let handle = unsafe {
            tarantino_hover_reader_open(
                path.as_ptr(),
                width,
                height,
                start_ms,
                end_ms,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        NonNull::new(handle)
            .map(Self)
            .ok_or_else(|| native_error(&error))
    }

    pub fn next(&mut self) -> Result<Option<Frame>> {
        self.next_reusing(None)
    }

    pub fn next_reusing(&mut self, recycled: Option<Frame>) -> Result<Option<Frame>> {
        let mut data = std::mem::MaybeUninit::<FrameData>::uninit();
        let mut error = [0 as c_char; 2048];
        let status = unsafe {
            tarantino_hover_reader_next(
                self.0.as_ptr(),
                data.as_mut_ptr(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        match status {
            0 => Ok(None),
            1 => {
                // The reader owns locked planes until the next read or drop.
                let data = unsafe { data.assume_init() };
                let (mut y, mut uv) = recycled.map(Frame::into_planes).unwrap_or_default();
                copy_plane(data.y, data.y_stride, data.width, data.height, &mut y)?;
                copy_plane(
                    data.uv,
                    data.uv_stride,
                    data.width.div_ceil(2) * 2,
                    data.height.div_ceil(2),
                    &mut uv,
                )?;
                Frame::new(data.time_ms, data.width, data.height, y, uv).map(Some)
            }
            _ => Err(native_error(&error)),
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        unsafe { tarantino_hover_reader_close(self.0.as_ptr()) };
    }
}

fn native_error(error: &[c_char]) -> anyhow::Error {
    anyhow!(
        unsafe { CStr::from_ptr(error.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    )
}

pub(super) fn copy_plane(
    source: *const u8,
    stride: usize,
    width: usize,
    height: usize,
    output: &mut Vec<u8>,
) -> Result<()> {
    ensure!(
        !source.is_null() && width > 0 && height > 0 && stride >= width,
        "Invalid decoded video plane"
    );
    let length = stride
        .checked_mul(height)
        .context("Decoded video plane is too large")?;
    ensure!(
        length <= isize::MAX as usize,
        "Decoded video plane is too large"
    );
    let bytes = unsafe { std::slice::from_raw_parts(source, length) };
    output.clear();
    output.reserve(width * height);
    for row in bytes.chunks_exact(stride) {
        output.extend_from_slice(&row[..width]);
    }
    Ok(())
}
