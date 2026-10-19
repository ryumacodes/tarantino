//! Desktop-generated clicks (including Steam Input) alongside physical evdev input.
use super::{MouseButton, MouseEvent, MouseEventType, MouseTracker, STREAM_POINTER};
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static RUNNING: AtomicBool = AtomicBool::new(false);
static RECENT: Mutex<Recent> = Mutex::new(Recent {
    recording: None,
    events: VecDeque::new(),
});
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Source {
    Device,
    Desktop,
    Injected,
}
struct Recent {
    recording: Option<SystemTime>,
    events: VecDeque<(u8, bool, Source, SystemTime)>,
}
impl Recent {
    fn accept(
        &mut self,
        recording: SystemTime,
        button: u8,
        pressed: bool,
        source: Source,
        time: SystemTime,
    ) -> bool {
        if self.recording != Some(recording) {
            self.events.clear();
            self.recording = Some(recording);
        }
        // Suppress only the other backend's copy. Fast clicks from the same
        // backend remain distinct, including alternating press/release events.
        if self.events.iter().any(|&(b, p, s, t)| {
            b == button
                && p == pressed
                && s != source
                && time
                    .duration_since(t)
                    .or_else(|_| t.duration_since(time))
                    .unwrap_or_default()
                    <= Duration::from_millis(40)
        }) {
            return false;
        }
        self.events.push_back((button, pressed, source, time));
        while self.events.len() > 32 {
            self.events.pop_front();
        }
        true
    }
}

pub(super) fn record(
    tracker: &Arc<Mutex<MouseTracker>>,
    event_type: MouseEventType,
    source: Source,
    time: SystemTime,
) {
    let position = {
        let pointer = STREAM_POINTER.lock();
        if !pointer.enabled {
            return;
        }
        pointer.position
    };
    let Some((x, y)) = position else { return };
    let (button, pressed) = match &event_type {
        MouseEventType::ButtonPress { button } => (button, true),
        MouseEventType::ButtonRelease { button } => (button, false),
        _ => return,
    };
    let code = match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::Unknown => return,
    };
    let tracker = tracker.lock();
    let Some(start) = tracker.recording_start_time else {
        return;
    };
    if !tracker.is_tracking
        || time < start
        || !RECENT.lock().accept(start, code, pressed, source, time)
    {
        return;
    }
    tracker.add_event(MouseEvent {
        timestamp: time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        x,
        y,
        event_type,
        display_id: None,
    });
}

pub(super) fn x11_available() -> bool {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session == "wayland" || (session != "x11" && std::env::var_os("WAYLAND_DISPLAY").is_some()) {
        return false;
    }
    // Probe the actual extension, not merely DISPLAY's presence. Each probe
    // owns its connection and never intercepts or injects desktop input.
    unsafe {
        let display = x11::xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return false;
        }
        let mut major = 0;
        let mut minor = 0;
        let available = x11::xrecord::XRecordQueryVersion(display, &mut major, &mut minor) != 0;
        x11::xlib::XCloseDisplay(display);
        available
    }
}

pub(super) fn start(tracker: Arc<Mutex<MouseTracker>>) -> anyhow::Result<()> {
    super::xtest::start(tracker.clone());
    if std::env::var_os("DISPLAY").is_none()
        || RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return Ok(());
    }
    let tracker = Arc::downgrade(&tracker);
    if let Err(error) = std::thread::Builder::new()
        .name("linux-desktop-clicks".into())
        .spawn(move || {
            // rdev owns the XRecord connection for the app lifetime. Only button
            // events are consumed, and record() drops everything outside recording.
            let result = rdev::listen(move |event| {
                let kind = match event.event_type {
                    rdev::EventType::ButtonPress(button) => MouseEventType::ButtonPress {
                        button: button.into(),
                    },
                    rdev::EventType::ButtonRelease(button) => MouseEventType::ButtonRelease {
                        button: button.into(),
                    },
                    _ => return,
                };
                if let Some(tracker) = tracker.upgrade() {
                    record(&tracker, kind, Source::Desktop, event.time);
                }
            });
            if let Err(error) = result {
                eprintln!("Linux desktop click tracking unavailable: {error:?}");
            }
            RUNNING.store(false, Ordering::Release);
        })
    {
        RUNNING.store(false, Ordering::Release);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an isolated X11 server with RECORD and xdotool"]
    fn native_x11_click_without_raw_devices() {
        assert!(x11_available());
        let tracker = Arc::new(Mutex::new(MouseTracker::new()));
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(SystemTime::now());
            let mut pointer = STREAM_POINTER.lock();
            pointer.enabled = true;
            pointer.dimensions = Some((800, 600));
            pointer.position = Some((200.0, 150.0));
        }
        // The production X11 branch must start without opening evdev devices.
        super::super::create_pointer_listener(tracker.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            std::process::Command::new("xdotool")
                .args(["mousemove", "200", "150", "click", "1"])
                .status()
                .unwrap()
                .success()
        );
        std::thread::sleep(Duration::from_millis(200));
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
        assert!(
            events
                .iter()
                .all(|event| event.x == 200.0 && event.y == 150.0)
        );
        tracker.lock().is_tracking = false;
        super::super::configure_stream_pointer(false);
    }
    #[test]
    fn deduplicates_backend_copies_but_preserves_real_double_clicks() {
        let mut recent = Recent {
            recording: None,
            events: VecDeque::new(),
        };
        let t = UNIX_EPOCH + Duration::from_secs(100);
        assert!(recent.accept(t, 1, true, Source::Device, t));
        assert!(!recent.accept(t, 1, true, Source::Desktop, t + Duration::from_millis(8)));
        assert!(recent.accept(t, 1, false, Source::Device, t + Duration::from_millis(10)));
        assert!(recent.accept(t, 1, true, Source::Device, t + Duration::from_millis(20)));
        assert!(recent.accept(t, 3, true, Source::Desktop, t + Duration::from_millis(22)));
        let next = t + Duration::from_millis(25);
        assert!(recent.accept(next, 1, true, Source::Desktop, next));
    }
    #[test]
    #[ignore = "requires X11/XWayland, Python GTK3, xdotool and writable /dev/uinput"]
    fn native_desktop_click_creates_zoom() {
        use crate::{
            auto_zoom::ZoomProcessor,
            event_capture::{CaptureSession, EnhancedMouseEvent},
        };
        let tracker = Arc::new(Mutex::new(MouseTracker::new()));
        let now = SystemTime::now();
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(now);
            let mut pointer = STREAM_POINTER.lock();
            pointer.enabled = true;
            pointer.dimensions = Some((800, 600));
            pointer.position = Some((200.0, 150.0));
        }
        start(tracker.clone()).unwrap();
        // XTest produces a desktop click without a physical evdev event, the
        // missing path for controller/remote pointer emulation under XWayland.
        let code = r#"import gi,subprocess,os,fcntl,struct,time
gi.require_version('Gtk','3.0');gi.require_version('GdkX11','3.0')
from gi.repository import Gtk,GLib,GdkX11,Gdk
w=Gtk.Window(title='Tarantino Desktop Click Test');w.set_default_size(300,150)
w.add(Gtk.Label(label='Synthetic desktop click test'));w.show_all()
w.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
w.connect('button-press-event',lambda *args:print('fixture_received_click',flush=True))
fd=os.open('/dev/uinput',os.O_WRONLY|os.O_NONBLOCK)
for t in [1,3]:fcntl.ioctl(fd,0x40045564,t)
fcntl.ioctl(fd,0x40045565,272)
for a in [0,1]:fcntl.ioctl(fd,0x40045567,a)
os.write(fd,struct.pack('80sHHHHI',b'Tarantino test pointer',3,0x1234,0x5679,1,0)+struct.pack('64i',65535,65535,*([0]*62))+bytes(768));fcntl.ioctl(fd,0x5501)
def ev(t,c,v):os.write(fd,struct.pack('llHHi',0,0,t,c,v))
def click():
 wid=str(w.get_window().get_xid())
 subprocess.run(['xdotool','windowactivate','--sync',wid],check=True)
 geo=dict(line.split('=') for line in subprocess.check_output(['xdotool','getwindowgeometry','--shell',wid],text=True).splitlines())
 screen=Gdk.Screen.get_default()
 ev(3,0,round((int(geo['X'])+100)*65535/screen.get_width()));ev(3,1,round((int(geo['Y'])+75)*65535/screen.get_height()));ev(0,0,0)
 GLib.timeout_add(300,lambda:(subprocess.run(['xdotool','click','1'],check=True),False)[1])
 return False
GLib.timeout_add(800,click);GLib.timeout_add(1800,lambda:(Gtk.main_quit(),False)[1])
Gtk.main();fcntl.ioctl(fd,0x5502);os.close(fd);w.destroy()
"#;
        let status = std::process::Command::new("/usr/bin/python3")
            .env("GDK_BACKEND", "x11")
            .args(["-c", code])
            .status()
            .unwrap();
        assert!(status.success());
        std::thread::sleep(Duration::from_millis(100));
        let events = {
            let mut state = tracker.lock();
            state.is_tracking = false;
            state.get_events()
        };
        super::super::configure_stream_pointer(false);
        let base = now.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let mut session = CaptureSession::new();
        session.start_time = 0;
        session.end_time = Some(5000);
        session.metadata.display_resolution = (800, 600);
        session.mouse_events = events
            .into_iter()
            .map(|mut event| {
                event.timestamp = event.timestamp.saturating_sub(base);
                EnhancedMouseEvent {
                    base: event,
                    window_id: None,
                    app_name: None,
                    is_double_click: false,
                    cluster_id: None,
                }
            })
            .collect();
        let analysis = ZoomProcessor::with_default_config()
            .analyze_session(&session, &[])
            .unwrap();
        assert_eq!(
            analysis.total_clicks, 1,
            "desktop click must reach automatic zoom exactly once"
        );
        assert_eq!(analysis.zoom_blocks.len(), 1);
        assert_eq!(analysis.zoom_blocks[0].center_x, 0.25);
        assert_eq!(analysis.zoom_blocks[0].center_y, 0.25);
    }
}
