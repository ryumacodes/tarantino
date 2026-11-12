use std::ffi::{CStr, CString, c_char};

unsafe extern "C" {
    fn tarantino_video_frame(path: *const c_char, time_ms: f64, width: u32, lossless: bool,
        length: *mut usize, error: *mut c_char, capacity: usize) -> *mut u8;
    fn tarantino_video_frame_free(bytes: *mut u8);
}

pub fn extract(path: &str, time_ms: f64, width: u32, lossless: bool) -> Result<Vec<u8>, String> {
    let path = CString::new(path).map_err(|error| error.to_string())?;
    let mut length = 0;
    let mut error = [0 as c_char; 2048];
    let bytes = unsafe {
        tarantino_video_frame(path.as_ptr(), time_ms, width, lossless, &mut length, error.as_mut_ptr(), error.len())
    };
    if bytes.is_null() {
        return Err(unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned());
    }
    let image = unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec();
    unsafe { tarantino_video_frame_free(bytes) };
    Ok(image)
}
