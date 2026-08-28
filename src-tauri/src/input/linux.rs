//! Native Linux mouse-button capture for Wayland sessions.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

use super::{LAST_MOUSE_POSITION, MouseButton, MouseEvent, MouseEventType, MouseTracker};

const EV_KEY: u16 = 0x01;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const BTN_SIDE: u16 = 0x113;
const BTN_EXTRA: u16 = 0x114;

#[repr(C)]
struct InputEvent {
    time: libc::timeval,
    event_type: u16,
    code: u16,
    value: i32,
}

type XDisplay = std::ffi::c_void;
type XWindow = libc::c_ulong;

#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const libc::c_char) -> *mut XDisplay;
    fn XDefaultRootWindow(display: *mut XDisplay) -> XWindow;
    fn XQueryPointer(
        display: *mut XDisplay,
        window: XWindow,
        root_return: *mut XWindow,
        child_return: *mut XWindow,
        root_x_return: *mut libc::c_int,
        root_y_return: *mut libc::c_int,
        win_x_return: *mut libc::c_int,
        win_y_return: *mut libc::c_int,
        mask_return: *mut libc::c_uint,
    ) -> libc::c_int;
    fn XCloseDisplay(display: *mut XDisplay) -> libc::c_int;
}

pub(super) fn create_button_listener(tracker: Arc<Mutex<MouseTracker>>) {
    std::thread::spawn(move || {
        let mut listening = HashSet::new();
        loop {
            for path in mouse_event_devices() {
                if listening.insert(path.clone()) {
                    let device_tracker = tracker.clone();
                    std::thread::spawn(move || listen_to_device(&path, device_tracker));
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

fn mouse_event_devices() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir("/dev/input") else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("event"))
        })
        .filter(|path| supports_left_button(path))
        .collect()
}

fn supports_left_button(path: &Path) -> bool {
    let Some(event_name) = path.file_name() else {
        return false;
    };
    let capabilities = Path::new("/sys/class/input")
        .join(event_name)
        .join("device/capabilities/key");
    let Ok(bitmap) = fs::read_to_string(capabilities) else {
        return false;
    };
    bitmap_has_code(&bitmap, BTN_LEFT as usize)
}

fn bitmap_has_code(bitmap: &str, code: usize) -> bool {
    let words = bitmap
        .split_whitespace()
        .rev()
        .filter_map(|word| u64::from_str_radix(word, 16).ok())
        .collect::<Vec<_>>();
    let word = code / u64::BITS as usize;
    let bit = code % u64::BITS as usize;
    words.get(word).is_some_and(|value| value & (1 << bit) != 0)
}

fn listen_to_device(path: &Path, tracker: Arc<Mutex<MouseTracker>>) {
    let Ok(file) = File::open(path) else {
        return;
    };
    println!("Linux native click tracking active on {}", path.display());

    loop {
        let mut event = std::mem::MaybeUninit::<InputEvent>::uninit();
        let result = unsafe {
            libc::read(
                file.as_raw_fd(),
                event.as_mut_ptr().cast(),
                std::mem::size_of::<InputEvent>(),
            )
        };
        if result < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
        if result as usize != std::mem::size_of::<InputEvent>() {
            return;
        }
        let event = unsafe { event.assume_init() };
        if event.event_type != EV_KEY || event.value != 1 {
            continue;
        }
        let button = match event.code {
            BTN_LEFT => MouseButton::Left,
            BTN_RIGHT => MouseButton::Right,
            BTN_MIDDLE => MouseButton::Middle,
            BTN_SIDE | BTN_EXTRA => MouseButton::Unknown,
            _ => continue,
        };

        let guard = tracker.lock();
        if !guard.is_tracking {
            continue;
        }
        let (x, y) = query_pointer_position().unwrap_or_else(|| *LAST_MOUSE_POSITION.lock());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        guard.add_event(MouseEvent {
            timestamp,
            x,
            y,
            event_type: MouseEventType::ButtonPress { button },
            display_id: None,
        });
    }
}

fn query_pointer_position() -> Option<(f64, f64)> {
    unsafe {
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let root = XDefaultRootWindow(display);
        let mut root_return = 0;
        let mut child_return = 0;
        let mut root_x = 0;
        let mut root_y = 0;
        let mut win_x = 0;
        let mut win_y = 0;
        let mut mask = 0;
        let result = XQueryPointer(
            display,
            root,
            &mut root_return,
            &mut child_return,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        );
        XCloseDisplay(display);
        (result != 0).then_some((root_x as f64, root_y as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::bitmap_has_code;

    #[test]
    fn recognizes_linux_mouse_button_bitmap() {
        assert!(bitmap_has_code("1f0000 0 0 0 0", 0x110));
        assert!(bitmap_has_code("1f0000 0 0 0 0", 0x114));
        assert!(!bitmap_has_code("1f0000 0 0 0 0", 0x115));
    }
}
