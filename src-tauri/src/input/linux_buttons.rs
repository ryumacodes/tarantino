//! Reconnectable button input paired with compositor-provided coordinates.
use super::{
    MouseTracker, add_current_event_at, button_event, high_resolution_wheel_axes,
    input_event_paths, stream_pointer_enabled, wheel_event,
};
use anyhow::Result;
use evdev::{Device, EventSummary, KeyCode};
use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

static RUNNING: AtomicBool = AtomicBool::new(false);

pub(super) fn start(tracker: Arc<Mutex<MouseTracker>>) -> Result<()> {
    if RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }
    let mut devices = BTreeMap::new();
    scan(&mut devices);
    if devices.is_empty() {
        RUNNING.store(false, Ordering::Release);
        anyhow::bail!("No readable mouse button device was found");
    }
    let tracker = Arc::downgrade(&tracker);
    if let Err(error) = std::thread::Builder::new()
        .name("linux-buttons".into())
        .spawn(move || {
            struct Guard;
            impl Drop for Guard {
                fn drop(&mut self) {
                    RUNNING.store(false, Ordering::Release);
                }
            }
            let _guard = Guard;
            listen(tracker, devices);
        })
    {
        RUNNING.store(false, Ordering::Release);
        return Err(error.into());
    }
    Ok(())
}

fn scan(devices: &mut BTreeMap<PathBuf, Device>) {
    for path in input_event_paths().unwrap_or_default() {
        if devices.contains_key(&path) {
            continue;
        }
        let Ok(device) = Device::open(&path) else {
            continue;
        };
        if device
            .supported_keys()
            .is_some_and(|keys| keys.contains(KeyCode::BTN_LEFT))
            && device.set_nonblocking(true).is_ok()
        {
            devices.insert(path, device);
        }
    }
}

fn listen(tracker: Weak<Mutex<MouseTracker>>, mut devices: BTreeMap<PathBuf, Device>) {
    let mut last_scan = None::<Instant>;
    loop {
        let Some(tracker) = tracker.upgrade() else {
            return;
        };
        if !tracker.lock().is_tracking || !stream_pointer_enabled() {
            // Do not retain device handles or queued input between recordings.
            devices.clear();
            last_scan = None;
            std::thread::sleep(Duration::from_millis(25));
            continue;
        }
        if last_scan.is_none_or(|time| time.elapsed() >= Duration::from_secs(1)) {
            scan(&mut devices);
            last_scan = Some(Instant::now());
        }
        devices.retain(|_, device| {
            let axes = high_resolution_wheel_axes(device);
            let events = match device.fetch_events() {
                Ok(events) => events.collect::<Vec<_>>(),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return true,
                Err(_) => return false,
            };
            for event in events {
                let kind = match event.destructure() {
                    EventSummary::Key(_, code, value) => button_event(code, value),
                    EventSummary::RelativeAxis(_, axis, value) => wheel_event(axis, value, axes),
                    _ => None,
                };
                if let Some(kind) = kind {
                    if matches!(
                        kind,
                        super::MouseEventType::ButtonPress { .. }
                            | super::MouseEventType::ButtonRelease { .. }
                    ) {
                        super::desktop_buttons::record(
                            &tracker,
                            kind,
                            super::desktop_buttons::Source::Device,
                            event.timestamp(),
                        );
                    } else {
                        add_current_event_at(&tracker, None, kind, event.timestamp());
                    }
                }
            }
            true
        });
        std::thread::sleep(Duration::from_millis(8));
    }
}
