//! Linux keyboard activity for automatic zoom. No characters are persisted.

use super::super::{KeyEvent, KeyMotion, MouseTracker};
use evdev::{Device, EventSummary, KeyCode};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "linux_typing.rs"]
mod typing;
use typing::{TypingMotion, TypingState};

static RUNNING: AtomicBool = AtomicBool::new(false);

pub(super) fn start(tracker: Arc<Mutex<MouseTracker>>) {
    if RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let tracker = Arc::downgrade(&tracker);
    if let Err(error) = std::thread::Builder::new()
        .name("linux-typing".into())
        .spawn(move || {
            struct RunningGuard;
            impl Drop for RunningGuard {
                fn drop(&mut self) {
                    RUNNING.store(false, Ordering::Release);
                }
            }
            let _guard = RunningGuard;
            super::caret::start(tracker.clone());
            listen(tracker);
        })
    {
        RUNNING.store(false, Ordering::Release);
        eprintln!("Could not start Linux typing detection: {error}");
    }
}

fn listen(tracker: Weak<Mutex<MouseTracker>>) {
    let mut devices: BTreeMap<PathBuf, (usize, Device)> = BTreeMap::new();
    let mut typing = TypingState::default();
    let mut next_id = 0;
    let mut last_scan: Option<Instant> = None;
    loop {
        let Some(tracker) = tracker.upgrade() else {
            return;
        };
        if !tracker.lock().is_tracking {
            // Release device access between recordings and discard modifier state.
            devices.clear();
            typing = TypingState::default();
            last_scan = None;
            std::thread::sleep(Duration::from_millis(25));
            continue;
        }
        if last_scan.is_none_or(|time| time.elapsed() >= Duration::from_secs(1)) {
            scan_devices(&mut devices, &mut typing, &mut next_id);
            last_scan = Some(Instant::now());
        }
        devices.retain(|_, (id, device)| {
            let events = match device.fetch_events() {
                Ok(events) => events.collect::<Vec<_>>(),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return true,
                Err(_) => {
                    typing.remove_device(*id);
                    return false;
                }
            };
            for event in events {
                if let EventSummary::Key(_, code, value) = event.destructure() {
                    if let Some(motion) = typing.key(*id, code.0, value) {
                        record_typing(&tracker, motion, event.timestamp());
                    }
                }
            }
            true
        });
        std::thread::sleep(Duration::from_millis(8));
    }
}

fn scan_devices(
    devices: &mut BTreeMap<PathBuf, (usize, Device)>,
    typing: &mut TypingState,
    next_id: &mut usize,
) {
    for path in super::input_event_paths().unwrap_or_default() {
        if devices.contains_key(&path) {
            continue;
        }
        let Ok(device) = Device::open(&path) else {
            continue;
        };
        // Exclude pointer buttons, controllers and media-only interfaces.
        if !device.supported_keys().is_some_and(|keys| {
            keys.contains(KeyCode::KEY_A)
                && keys.contains(KeyCode::KEY_Z)
                && keys.contains(KeyCode::KEY_ENTER)
        }) {
            continue;
        }
        if device.set_nonblocking(true).is_err() {
            continue;
        }
        let Ok(pressed) = device.get_key_state() else {
            continue;
        };
        let id = *next_id;
        *next_id += 1;
        for code in pressed.iter() {
            // Seed held modifiers without emitting pre-recording keystrokes.
            let _ = typing.key(id, code.0, 1);
        }
        devices.insert(path, (id, device));
    }
}

fn record_typing(tracker: &Mutex<MouseTracker>, motion: TypingMotion, timestamp: SystemTime) {
    let tracker = tracker.lock();
    if !tracker.is_tracking
        || tracker
            .recording_start_time
            .is_none_or(|start| timestamp < start)
    {
        return;
    }
    let position = tracker.events.lock().back().map(|event| (event.x, event.y));
    tracker.add_key_event(KeyEvent {
        timestamp: timestamp
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        is_modifier: false,
        is_typing: true,
        key_motion: match motion {
            TypingMotion::Character => KeyMotion::Character,
            TypingMotion::Newline => KeyMotion::Newline,
            TypingMotion::Backspace => KeyMotion::Backspace,
            TypingMotion::Tab => KeyMotion::Tab,
        },
        cursor_x: position.map(|(x, _)| x),
        cursor_y: position.map(|(_, y)| y),
        caret_x: None,
        caret_y: None,
    });
}
