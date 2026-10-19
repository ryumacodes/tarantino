//! Observe XTEST button requests before XWayland forwards them to native Wayland.
//! No keyboard requests or character data are consumed.
use super::{MouseButton, MouseEventType, MouseTracker, desktop_buttons};
use parking_lot::Mutex;
use std::{
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};
use x11::{xlib, xrecord};

static RUNNING: AtomicBool = AtomicBool::new(false);

pub(super) fn start(tracker: Arc<Mutex<MouseTracker>>) {
    if std::env::var_os("DISPLAY").is_none() || RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let tracker = Arc::downgrade(&tracker);
    if let Err(error) = std::thread::Builder::new()
        .name("linux-xtest-clicks".into())
        .spawn(move || {
            if let Err(error) = listen(tracker) {
                eprintln!("Linux emulated click tracking unavailable: {error}");
            }
            RUNNING.store(false, Ordering::Release);
        })
    {
        RUNNING.store(false, Ordering::Release);
        eprintln!("Could not start Linux emulated click tracking: {error}");
    }
}

// The byte-sized event type/detail have the same position for either byte order.
fn button_request(bytes: &[u8]) -> Option<MouseEventType> {
    if bytes.len() < 36 || bytes[1] != 2 {
        return None;
    }
    let button = match bytes[5] {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        _ => return None,
    };
    match bytes[4] {
        4 => Some(MouseEventType::ButtonPress { button }),
        5 => Some(MouseEventType::ButtonRelease { button }),
        _ => None,
    }
}

unsafe extern "C" fn callback(
    closure: *mut std::os::raw::c_char,
    raw: *mut xrecord::XRecordInterceptData,
) {
    // XRecord owns each buffer, including category markers with no data.
    unsafe {
        if raw.is_null() {
            return;
        }
        let data = &*raw;
        if data.category == xrecord::XRecordFromClient && !data.data.is_null() {
            let tracker = &*(closure as *const Weak<Mutex<MouseTracker>>);
            if let Some(tracker) = tracker.upgrade() {
                // Requests selected by this context are fixed-size XTestFakeInput.
                let bytes = std::slice::from_raw_parts(data.data, data.data_len as usize * 4);
                for request in bytes.chunks_exact(36) {
                    if let Some(kind) = button_request(request) {
                        desktop_buttons::record(
                            &tracker,
                            kind,
                            desktop_buttons::Source::Injected,
                            SystemTime::now(),
                        );
                    }
                }
            }
        }
        xrecord::XRecordFreeData(raw);
    }
}

fn listen(mut tracker: Weak<Mutex<MouseTracker>>) -> anyhow::Result<()> {
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        anyhow::ensure!(!display.is_null(), "X display unavailable");
        // Each worker owns its connection and context for the listener lifetime.
        let result = (|| {
            let (mut opcode, mut event, mut error) = (0, 0, 0);
            anyhow::ensure!(
                xlib::XQueryExtension(
                    display,
                    c"XTEST".as_ptr(),
                    &mut opcode,
                    &mut event,
                    &mut error
                ) != 0,
                "XTEST unavailable"
            );
            let mut range: xrecord::XRecordRange = std::mem::zeroed();
            range.ext_requests.ext_major.first = opcode as u8;
            range.ext_requests.ext_major.last = opcode as u8;
            range.ext_requests.ext_minor.first = 2;
            range.ext_requests.ext_minor.last = 2;
            let mut range_ptr = &mut range as *mut _;
            let mut clients = xrecord::XRecordAllClients;
            let context =
                xrecord::XRecordCreateContext(display, 0, &mut clients, 1, &mut range_ptr, 1);
            anyhow::ensure!(context != 0, "XRecord request context unavailable");
            xlib::XSync(display, 0);
            let status = xrecord::XRecordEnableContext(
                display,
                context,
                Some(callback),
                (&mut tracker as *mut Weak<Mutex<MouseTracker>>).cast(),
            );
            xrecord::XRecordFreeContext(display, context);
            anyhow::ensure!(status != 0, "XRecord request listener failed");
            Ok(())
        })();
        xlib::XCloseDisplay(display);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires isolated X11 with RECORD, XTEST and xdotool"]
    fn native_xtest_requests_without_server_events() {
        let tracker = Arc::new(Mutex::new(MouseTracker::new()));
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(SystemTime::now());
            let mut pointer = super::super::STREAM_POINTER.lock();
            pointer.enabled = true;
            pointer.position = Some((200.0, 150.0));
        }
        // Deliberately start neither evdev nor the server-event listener.
        start(tracker.clone());
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert!(
            std::process::Command::new("xdotool")
                .args(["click", "1"])
                .status()
                .unwrap()
                .success()
        );
        std::thread::sleep(std::time::Duration::from_millis(250));
        let events = tracker.lock().get_events();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.event_type,
                    MouseEventType::ButtonPress {
                        button: MouseButton::Left
                    }
                ))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.event_type,
                    MouseEventType::ButtonRelease {
                        button: MouseButton::Left
                    }
                ))
                .count(),
            1
        );
        assert!(
            events
                .iter()
                .all(|event| event.x == 200.0 && event.y == 150.0)
        );
        tracker.lock().is_tracking = false;
        super::super::configure_stream_pointer(false);
    }

    #[test]
    fn accepts_only_mouse_button_requests() {
        let mut request = [0u8; 36];
        request[1] = 2;
        request[4] = 4;
        request[5] = 1;
        assert!(matches!(
            button_request(&request),
            Some(MouseEventType::ButtonPress {
                button: MouseButton::Left
            })
        ));
        request[4] = 5;
        assert!(matches!(
            button_request(&request),
            Some(MouseEventType::ButtonRelease { .. })
        ));
        // Keyboard requests must never become clicks even with matching detail.
        request[4] = 2;
        assert!(button_request(&request).is_none());
        request[4] = 4;
        request[5] = 4;
        assert!(button_request(&request).is_none());
        assert!(button_request(&request[..6]).is_none());
    }
}
