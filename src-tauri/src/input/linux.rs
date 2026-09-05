//! Linux pointer tracking through evdev.

use super::{MouseButton, MouseEvent, MouseEventType, MouseTracker};
use anyhow::{Context, Result, anyhow};
use evdev::{AbsoluteAxisCode, Device, EventSummary, KeyCode, RelativeAxisCode};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

const CANONICAL_WIDTH: f64 = 1920.0;
const CANONICAL_HEIGHT: f64 = 1080.0;

#[derive(Default)]
struct StreamPointer {
    enabled: bool,
    dimensions: Option<(u32, u32)>,
    position: Option<(f64, f64)>,
    tracker: Weak<Mutex<MouseTracker>>,
}

static STREAM_POINTER: LazyLock<Mutex<StreamPointer>> = LazyLock::new(Default::default);

pub fn pointer_coordinate_space() -> (u32, u32) {
    STREAM_POINTER
        .lock()
        .dimensions
        .unwrap_or((CANONICAL_WIDTH as u32, CANONICAL_HEIGHT as u32))
}

pub(crate) fn configure_stream_pointer(enabled: bool) {
    let mut pointer = STREAM_POINTER.lock();
    pointer.enabled = enabled;
    pointer.dimensions = None;
    pointer.position = None;
}

pub(crate) fn stream_pointer_enabled() -> bool {
    STREAM_POINTER.lock().enabled
}

pub(crate) fn observe_stream_pointer(width: u32, height: u32, position: Option<(u32, u32)>) {
    let mut pointer = STREAM_POINTER.lock();
    if !pointer.enabled || width == 0 || height == 0 {
        return;
    }
    pointer.dimensions = Some((width, height));
    let Some((x, y)) = position else {
        return;
    };
    let position = stream_position(width, height, x, y);
    if pointer.position == position {
        return;
    }
    pointer.position = position;
    let tracker = pointer.tracker.upgrade();
    drop(pointer);
    if let (Some(tracker), Some((x, y))) = (tracker, position) {
        push_pointer_event(&tracker, x, y, MouseEventType::Move);
    }
}

fn stream_position(width: u32, height: u32, x: u32, y: u32) -> Option<(f64, f64)> {
    (x < width && y < height).then_some((x as f64, y as f64))
}

#[derive(Clone, Copy)]
struct AxisRange {
    minimum: i32,
    maximum: i32,
}

#[derive(Clone, Copy, Default)]
struct HighResolutionWheelAxes {
    horizontal: bool,
    vertical: bool,
}

impl AxisRange {
    fn map(self, value: i32, output_max: f64) -> f64 {
        let span = (self.maximum - self.minimum).max(1) as f64;
        (((value - self.minimum) as f64 / span) * output_max).clamp(0.0, output_max)
    }
}

struct PointerDevices {
    position: Device,
    position_path: PathBuf,
    buttons: Option<(Device, PathBuf)>,
    x: AxisRange,
    y: AxisRange,
}

fn input_event_paths() -> Result<Vec<PathBuf>> {
    let mut paths = std::fs::read_dir("/dev/input")
        .context("Linux input devices are unavailable")?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("event"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn open_pointer_devices() -> Result<PointerDevices> {
    let paths = input_event_paths()?;
    let mut position = None;

    for path in &paths {
        let Ok(device) = Device::open(path) else {
            continue;
        };
        let has_xy = device.supported_absolute_axes().is_some_and(|axes| {
            axes.contains(AbsoluteAxisCode::ABS_X) && axes.contains(AbsoluteAxisCode::ABS_Y)
        });
        if !has_xy {
            continue;
        }

        let mut x = None;
        let mut y = None;
        for (axis, info) in device.get_absinfo()? {
            let range = AxisRange {
                minimum: info.minimum(),
                maximum: info.maximum(),
            };
            match axis {
                AbsoluteAxisCode::ABS_X => x = Some(range),
                AbsoluteAxisCode::ABS_Y => y = Some(range),
                _ => {}
            }
        }
        if let (Some(x), Some(y)) = (x, y) {
            position = Some((device, path.clone(), x, y));
            break;
        }
    }

    let (position, position_path, x, y) = position.ok_or_else(|| {
        anyhow!(
            "No readable absolute mouse device was found; automatic zoom needs pointer permission"
        )
    })?;

    // VMware exposes position and button events through separate devices.
    let buttons = paths
        .iter()
        .filter(|path| **path != position_path)
        .find_map(|path| {
            let device = Device::open(path).ok()?;
            let has_left_button = device
                .supported_keys()
                .is_some_and(|keys| keys.contains(KeyCode::BTN_LEFT));
            let has_relative_xy = device.supported_relative_axes().is_some_and(|axes| {
                axes.contains(RelativeAxisCode::REL_X) && axes.contains(RelativeAxisCode::REL_Y)
            });
            (has_left_button && has_relative_xy).then(|| (device, path.clone()))
        });

    let position_has_buttons = position
        .supported_keys()
        .is_some_and(|keys| keys.contains(KeyCode::BTN_LEFT));
    if buttons.is_none() && !position_has_buttons {
        return Err(anyhow!("No readable mouse button device was found"));
    }

    Ok(PointerDevices {
        position,
        position_path,
        buttons,
        x,
        y,
    })
}

pub fn raw_pointer_tracking_available() -> bool {
    !open_button_devices().is_empty()
}

pub fn create_mouse_listener(tracker: Arc<Mutex<MouseTracker>>) -> Result<()> {
    STREAM_POINTER.lock().tracker = Arc::downgrade(&tracker);
    if stream_pointer_enabled() {
        let buttons = open_button_devices();
        anyhow::ensure!(
            !buttons.is_empty(),
            "No readable mouse button device was found"
        );
        for (device, path) in buttons {
            println!(
                "Linux compositor pointer listening to buttons: {}",
                path.display()
            );
            let tracker = tracker.clone();
            let range = AxisRange {
                minimum: 0,
                maximum: 1,
            };
            let coordinates = Arc::new(Mutex::new((0, 0, range, range)));
            std::thread::spawn(move || listen_buttons(device, tracker, coordinates));
        }
        return Ok(());
    }
    let mut devices = open_pointer_devices()?;
    println!(
        "Linux automatic zoom listening to pointer position: {}",
        devices.position_path.display()
    );

    let raw_x =
        current_axis_value(&devices.position, AbsoluteAxisCode::ABS_X).unwrap_or(devices.x.minimum);
    let raw_y =
        current_axis_value(&devices.position, AbsoluteAxisCode::ABS_Y).unwrap_or(devices.y.minimum);
    let coordinates = Arc::new(Mutex::new((raw_x, raw_y, devices.x, devices.y)));
    let buttons_are_separate = devices.buttons.is_some();

    if let Some((button_device, path)) = devices.buttons.take() {
        println!(
            "Linux automatic zoom listening to pointer buttons: {}",
            path.display()
        );
        let button_tracker = tracker.clone();
        let button_coordinates = coordinates.clone();
        std::thread::spawn(move || {
            listen_buttons(button_device, button_tracker, button_coordinates)
        });
    }

    std::thread::spawn(move || {
        listen_position(devices, tracker, coordinates, !buttons_are_separate)
    });
    Ok(())
}

fn open_button_devices() -> Vec<(Device, PathBuf)> {
    input_event_paths()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| {
            let device = Device::open(&path).ok()?;
            device
                .supported_keys()
                .is_some_and(|keys| keys.contains(KeyCode::BTN_LEFT))
                .then_some((device, path))
        })
        .collect()
}

fn current_axis_value(device: &Device, wanted: AbsoluteAxisCode) -> Option<i32> {
    device.get_absinfo().ok().and_then(|axes| {
        axes.filter(|(axis, _)| *axis == wanted)
            .map(|(_, info)| info.value())
            .next()
    })
}

fn listen_position(
    mut devices: PointerDevices,
    tracker: Arc<Mutex<MouseTracker>>,
    coordinates: Arc<Mutex<(i32, i32, AxisRange, AxisRange)>>,
    include_buttons: bool,
) {
    let wheel_axes = high_resolution_wheel_axes(&devices.position);
    loop {
        let events = match devices.position.fetch_events() {
            Ok(events) => events.collect::<Vec<_>>(),
            Err(error) => {
                tracker.lock().is_tracking = false;
                eprintln!("Linux pointer position listener stopped: {error}");
                return;
            }
        };

        for event in events {
            let is_tracking = tracker.lock().is_tracking;
            match event.destructure() {
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, value) => {
                    coordinates.lock().0 = value;
                    if is_tracking {
                        add_current_event(&tracker, &coordinates, MouseEventType::Move);
                    }
                }
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, value) => {
                    coordinates.lock().1 = value;
                    if is_tracking {
                        add_current_event(&tracker, &coordinates, MouseEventType::Move);
                    }
                }
                EventSummary::Key(_, code, value) if is_tracking && include_buttons => {
                    if let Some(event_type) = button_event(code, value) {
                        add_current_event(&tracker, &coordinates, event_type);
                    }
                }
                EventSummary::RelativeAxis(_, axis, value) if is_tracking && include_buttons => {
                    if let Some(event_type) = wheel_event(axis, value, wheel_axes) {
                        add_current_event(&tracker, &coordinates, event_type);
                    }
                }
                _ => {}
            }
        }
    }
}

fn listen_buttons(
    mut device: Device,
    tracker: Arc<Mutex<MouseTracker>>,
    coordinates: Arc<Mutex<(i32, i32, AxisRange, AxisRange)>>,
) {
    let wheel_axes = high_resolution_wheel_axes(&device);
    loop {
        let events = match device.fetch_events() {
            Ok(events) => events.collect::<Vec<_>>(),
            Err(error) => {
                eprintln!("Linux pointer button listener stopped: {error}");
                return;
            }
        };
        if !tracker.lock().is_tracking {
            continue;
        }
        for event in events {
            let event_type = match event.destructure() {
                EventSummary::Key(_, code, value) => button_event(code, value),
                EventSummary::RelativeAxis(_, axis, value) => wheel_event(axis, value, wheel_axes),
                _ => None,
            };
            if let Some(event_type) = event_type {
                add_current_event(&tracker, &coordinates, event_type);
            }
        }
    }
}

fn button_event(code: KeyCode, value: i32) -> Option<MouseEventType> {
    let button = match code {
        KeyCode::BTN_LEFT => MouseButton::Left,
        KeyCode::BTN_RIGHT => MouseButton::Right,
        KeyCode::BTN_MIDDLE => MouseButton::Middle,
        _ => return None,
    };
    match value {
        1 => Some(MouseEventType::ButtonPress { button }),
        0 => Some(MouseEventType::ButtonRelease { button }),
        _ => None,
    }
}

fn high_resolution_wheel_axes(device: &Device) -> HighResolutionWheelAxes {
    let Some(axes) = device.supported_relative_axes() else {
        return HighResolutionWheelAxes::default();
    };
    HighResolutionWheelAxes {
        horizontal: axes.contains(RelativeAxisCode::REL_HWHEEL_HI_RES),
        vertical: axes.contains(RelativeAxisCode::REL_WHEEL_HI_RES),
    }
}

fn wheel_event(
    axis: RelativeAxisCode,
    value: i32,
    high_resolution: HighResolutionWheelAxes,
) -> Option<MouseEventType> {
    let (delta_x, delta_y) = match axis {
        RelativeAxisCode::REL_HWHEEL_HI_RES => (value as i64, 0),
        RelativeAxisCode::REL_WHEEL_HI_RES => (0, value as i64),
        RelativeAxisCode::REL_HWHEEL if !high_resolution.horizontal => (value as i64, 0),
        RelativeAxisCode::REL_WHEEL if !high_resolution.vertical => (0, value as i64),
        _ => return None,
    };
    Some(MouseEventType::Wheel { delta_x, delta_y })
}

fn add_current_event(
    tracker: &Arc<Mutex<MouseTracker>>,
    coordinates: &Arc<Mutex<(i32, i32, AxisRange, AxisRange)>>,
    event_type: MouseEventType,
) {
    let pointer = STREAM_POINTER.lock();
    if pointer.enabled {
        let position = pointer.position;
        drop(pointer);
        if let Some((x, y)) = position {
            push_pointer_event(tracker, x, y, event_type);
        }
        return;
    }
    drop(pointer);
    let (raw_x, raw_y, x_range, y_range) = *coordinates.lock();
    let x = x_range.map(raw_x, CANONICAL_WIDTH);
    let y = y_range.map(raw_y, CANONICAL_HEIGHT);
    push_pointer_event(tracker, x, y, event_type);
}

fn push_pointer_event(
    tracker: &Arc<Mutex<MouseTracker>>,
    x: f64,
    y: f64,
    event_type: MouseEventType,
) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    tracker.lock().add_event(MouseEvent {
        timestamp,
        x,
        y,
        event_type,
        display_id: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositor_coordinates_stay_relative_to_the_captured_window() {
        assert_eq!(stream_position(1378, 1137, 200, 800), Some((200.0, 800.0)));
        assert_eq!(stream_position(1378, 1137, u32::MAX, u32::MAX), None);
        assert_eq!(stream_position(1378, 1137, 1378, 200), None);
        assert_eq!(stream_position(0, 0, 0, 0), None);
    }

    #[test]
    fn maps_absolute_device_range_to_canonical_canvas() {
        assert_eq!(pointer_coordinate_space(), (1920, 1080));
        let range = AxisRange {
            minimum: 0,
            maximum: 32767,
        };
        assert_eq!(range.map(0, 1920.0), 0.0);
        assert_eq!(range.map(32767, 1920.0), 1920.0);
        assert!((range.map(16384, 1920.0) - 960.0).abs() < 0.1);
    }

    #[test]
    fn maps_only_supported_mouse_buttons() {
        assert!(matches!(
            button_event(KeyCode::BTN_LEFT, 1),
            Some(MouseEventType::ButtonPress {
                button: MouseButton::Left
            })
        ));
        assert!(button_event(KeyCode::KEY_A, 1).is_none());
    }

    #[test]
    fn prefers_high_resolution_wheel_events() {
        let high_resolution = HighResolutionWheelAxes {
            horizontal: true,
            vertical: true,
        };
        assert!(wheel_event(RelativeAxisCode::REL_WHEEL, 1, high_resolution).is_none());
        assert!(wheel_event(RelativeAxisCode::REL_HWHEEL, -1, high_resolution).is_none());
        assert!(matches!(
            wheel_event(RelativeAxisCode::REL_WHEEL_HI_RES, 120, high_resolution),
            Some(MouseEventType::Wheel {
                delta_x: 0,
                delta_y: 120
            })
        ));
    }
}
