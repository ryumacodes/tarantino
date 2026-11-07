//! Native input tracking and shared cursor/keyboard event types.

use anyhow::Result;
use parking_lot::Mutex;
mod caret_lookup;
pub mod cursor_images;
pub mod focus_samples;
#[cfg(target_os = "macos")]
pub use macos::install_cursor_images;
mod modifier_state;
use modifier_state::CommandModifiers;
#[cfg(target_os = "macos")]
mod keyboard_dispatch;
use rdev::{Button, Key};
#[cfg(target_os = "windows")]
use rdev::{Event, EventType, listen};

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

/// Global state for tracking the last known mouse position.
static LAST_MOUSE_POSITION: Mutex<(f64, f64)> = Mutex::new((0.0, 0.0));

/// Mouse event data structure for recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    /// Timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// X coordinate on screen
    pub x: f64,
    /// Y coordinate on screen
    pub y: f64,
    /// Type of mouse event
    pub event_type: MouseEventType,
    /// Display/monitor ID where event occurred
    pub display_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseEventType {
    /// Mouse moved to new position
    Move,
    /// Mouse button pressed
    ButtonPress { button: MouseButton },
    /// Mouse button released  
    ButtonRelease { button: MouseButton },
    /// Mouse wheel scrolled
    Wheel { delta_x: i64, delta_y: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Unknown,
}

pub fn normalize_mouse_events_for_timeline(
    events: &[MouseEvent],
    timeline_start_ms: u64,
) -> Vec<MouseEvent> {
    let initial_position = if events
        .iter()
        .any(|event| event.timestamp == timeline_start_ms)
    {
        None
    } else {
        events
            .iter()
            .filter(|event| event.timestamp < timeline_start_ms)
            .max_by_key(|event| event.timestamp)
            .map(|event| MouseEvent {
                timestamp: 0,
                x: event.x,
                y: event.y,
                event_type: MouseEventType::Move,
                display_id: event.display_id.clone(),
            })
    };

    let mut normalized = Vec::with_capacity(events.len() + usize::from(initial_position.is_some()));
    if let Some(initial) = initial_position {
        normalized.push(initial);
    }
    normalized.extend(
        events
            .iter()
            .filter(|event| event.timestamp >= timeline_start_ms)
            .cloned()
            .map(|mut event| {
                event.timestamp -= timeline_start_ms;
                event
            }),
    );
    normalized
}

impl From<Button> for MouseButton {
    fn from(button: Button) -> Self {
        match button {
            Button::Left => MouseButton::Left,
            Button::Right => MouseButton::Right,
            Button::Middle => MouseButton::Middle,
            Button::Unknown(_) => MouseButton::Unknown,
        }
    }
}

mod keyboard_event;
pub use keyboard_event::{KeyEvent, KeyMotion};
#[cfg(target_os = "macos")]
use keyboard_event::{ShortcutSample, ShortcutModifier};

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
mod macos;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::pointer_coordinate_space;
#[cfg(target_os = "linux")]
pub(crate) use linux::{
    configure_stream_geometry, configure_stream_pointer, observe_stream_pointer,
    stream_pointer_enabled,
};

#[cfg(not(target_os = "macos"))]
mod macos {
    pub fn focused_caret_position() -> Option<(f64, f64)> {
        None
    }
}

static POINTER_CAPTURE_CONSENT: AtomicBool = AtomicBool::new(true);

pub fn set_pointer_capture_consent(granted: bool) {
    POINTER_CAPTURE_CONSENT.store(granted, Ordering::Release);
}

pub fn pointer_capture_consented() -> bool {
    POINTER_CAPTURE_CONSENT.load(Ordering::Acquire)
}

/// Classify an rdev key: returns (is_command_modifier, is_character_key)
fn classify_key(key: &Key) -> (bool, bool) {
    match key {
        // Command modifiers — these suppress "typing" when held
        Key::MetaLeft | Key::MetaRight => (true, false),
        Key::ControlLeft | Key::ControlRight => (true, false),
        Key::Alt | Key::AltGr => (true, false),
        // Shift is NOT a command modifier (Shift+letter = typing)
        Key::ShiftLeft | Key::ShiftRight => (false, false),
        // Navigation / function keys — not typing
        Key::CapsLock
        | Key::Escape
        | Key::F1
        | Key::F2
        | Key::F3
        | Key::F4
        | Key::F5
        | Key::F6
        | Key::F7
        | Key::F8
        | Key::F9
        | Key::F10
        | Key::F11
        | Key::F12
        | Key::PrintScreen
        | Key::ScrollLock
        | Key::Pause
        | Key::Insert
        | Key::Home
        | Key::End
        | Key::PageUp
        | Key::PageDown
        | Key::UpArrow
        | Key::DownArrow
        | Key::LeftArrow
        | Key::RightArrow
        | Key::NumLock => (false, false),
        // Everything else (letters, numbers, space, enter, backspace, tab,
        // punctuation, etc.) counts as a character/typing key
        _ => (false, true),
    }
}

fn key_motion(key: &Key) -> KeyMotion {
    match key {
        Key::Return | Key::KpReturn => KeyMotion::Newline,
        Key::Backspace | Key::Delete => KeyMotion::Backspace,
        Key::Tab => KeyMotion::Tab,
        _ => KeyMotion::Character,
    }
}

/// Mouse tracking state and configuration
#[derive(Debug)]
pub struct MouseTracker {
    command_modifiers: CommandModifiers,
    #[cfg(target_os = "macos")]
    pub(super) shortcuts: macos::shortcuts::Recorder,
    /// Whether mouse tracking is currently active
    pub is_tracking: bool,
    pub cursor_images: cursor_images::CursorImages,
    pub focus_samples: Vec<focus_samples::FocusSample>,
    /// Recorded mouse events buffer
    pub events: Arc<Mutex<VecDeque<MouseEvent>>>,
    /// Recorded key events buffer (for typing detection)
    pub key_events: Arc<Mutex<VecDeque<KeyEvent>>>,
    /// Event broadcaster for real-time updates
    pub event_sender: broadcast::Sender<MouseEvent>,
    /// Recording start time for relative timestamps
    pub recording_start_time: Option<SystemTime>,
}

impl MouseTracker {
    pub fn configure_shortcut_capture(&mut self, enabled: bool) {
        #[cfg(target_os = "macos")]
        self.shortcuts.configure(enabled);
        #[cfg(not(target_os = "macos"))]
        let _ = enabled;
    }

    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(1000);

        Self {
            command_modifiers: CommandModifiers::default(),
            #[cfg(target_os = "macos")]
            shortcuts: macos::shortcuts::Recorder::default(),
            is_tracking: false,
            cursor_images: cursor_images::CursorImages::default(),
            focus_samples: Vec::new(),
            events: Arc::new(Mutex::new(VecDeque::new())),
            key_events: Arc::new(Mutex::new(VecDeque::new())),
            event_sender,
            recording_start_time: None,
        }
    }

    /// Start mouse tracking with permission validation
    pub fn start_tracking(&mut self) -> Result<()> {
        println!(
            "🖱️ [TRACKER] start_tracking called, current is_tracking: {}",
            self.is_tracking
        );

        if self.is_tracking {
            println!("🖱️ [TRACKER] Already tracking, returning early");
            return Ok(()); // Already tracking
        }

        // Validate accessibility permissions before starting
        if let Err(permission_error) = crate::permissions::validate_mouse_tracking_permissions() {
            println!(
                "❌ [TRACKER] Permission validation failed: {}",
                permission_error
            );
            return Err(anyhow::anyhow!(
                "Cannot start mouse tracking: {}",
                permission_error
            ));
        }

        self.cursor_images = cursor_images::CursorImages::default();
        self.focus_samples.clear();
        self.is_tracking = true;
        self.recording_start_time = Some(SystemTime::now());

        // Clear existing events
        {
            let mut events = self.events.lock();
            let old_count = events.len();
            events.clear();
            println!("🖱️ [TRACKER] Cleared {} old mouse events", old_count);
        }
        {
            let mut key_events = self.key_events.lock();
            let old_count = key_events.len();
            key_events.clear();
            println!("⌨️ [TRACKER] Cleared {} old key events", old_count);
        }
        #[cfg(target_os = "macos")]
        self.shortcuts.reset();
        // Reset modifier state
        self.command_modifiers = CommandModifiers::default();

        #[cfg(target_os = "macos")]
        if let Some(position) = macos::current_pointer_position() {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap_or_default().as_millis() as u64;
            self.seed_pointer_position(position, timestamp);
        }

        println!("✅ [TRACKER] Mouse tracking STARTED");
        println!("   - is_tracking: {}", self.is_tracking);
        println!("   - recording_start_time: {:?}", self.recording_start_time);
        Ok(())
    }

    /// Stop mouse tracking
    pub fn stop_tracking(&mut self) {
        let event_count = self.events.lock().len();
        println!("🛑 [TRACKER] stop_tracking called");
        println!("   - was_tracking: {}", self.is_tracking);
        println!("   - events captured: {}", event_count);

        self.is_tracking = false;
        self.recording_start_time = None;
        println!("✅ [TRACKER] Mouse tracking STOPPED");
    }

    #[cfg(any(target_os = "macos", test))]
    fn seed_pointer_position(&self, (x, y): (f64, f64), timestamp: u64) {
        if !self.is_tracking || !x.is_finite() || !y.is_finite() { return; }
        *LAST_MOUSE_POSITION.lock() = (x, y);
        self.add_event(MouseEvent {
            timestamp, x, y, event_type: MouseEventType::Move, display_id: None,
        });
    }

    /// Add a new mouse event to the buffer
    pub fn add_event(&self, event: MouseEvent) {
        if !self.is_tracking {
            return;
        }

        // These are recording data, not a rolling diagnostic buffer. Evicting
        // the beginning corrupts cursor playback and automatic zoom on long takes.
        let current_count = {
            let mut events = self.events.lock();
            events.push_back(event.clone());

            events.len()
        };

        // Log first event and every 500th event, plus all clicks
        let is_click = matches!(event.event_type, MouseEventType::ButtonPress { .. });
        if current_count == 1 {
            println!(
                "🖱️ [TRACKER] First event captured! Total: {}",
                current_count
            );
        } else if current_count % 500 == 0 {
            println!(
                "🖱️ [TRACKER] Event milestone: {} events captured",
                current_count
            );
        }

        if is_click {
            println!(
                "🖱️ [TRACKER] CLICK captured at ({:.1}, {:.1}) - Total events: {}",
                event.x, event.y, current_count
            );
        }

        // Broadcast event for real-time updates
        let _ = self.event_sender.send(event);
    }

    /// Get all recorded events
    pub fn get_events(&self) -> Vec<MouseEvent> {
        let events = self.events.lock();
        events.iter().cloned().collect()
    }

    /// Add a key event to the buffer
    pub fn add_key_event(&self, event: KeyEvent) {
        if !self.is_tracking {
            return;
        }

        let mut key_events = self.key_events.lock();
        key_events.push_back(event);

        let count = key_events.len();
        if count == 1 {
            println!("⌨️ [TRACKER] First key event captured!");
        } else if count % 200 == 0 {
            println!("⌨️ [TRACKER] Key event milestone: {} events", count);
        }
    }

    /// Update the latest typing key with a refined caret position, usually sampled
    /// on key release after the focused app has advanced its insertion point.
    pub fn update_latest_typing_caret(&self, caret_x: f64, caret_y: f64) {
        if !self.is_tracking {
            return;
        }

        let mut key_events = self.key_events.lock();
        // Stop at the last non-modifier press: a shortcut or navigation key
        // may have moved focus since the preceding typing event. In particular,
        // releasing Command before its character key must not retarget old text.
        if let Some(event) = key_events.iter_mut().rev()
            .find(|event| !event.is_release && !event.is_modifier)
            .filter(|event| event.is_typing) {
            event.caret_x = Some(caret_x);
            event.caret_y = Some(caret_y);
        }
    }

    /// Get all recorded key events
    pub fn get_key_events(&self) -> Vec<KeyEvent> {
        let key_events = self.key_events.lock();
        key_events.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests;

#[cfg(target_os = "macos")]
pub fn create_mouse_listener(tracker: Arc<Mutex<MouseTracker>>) -> Result<()> {
    crate::permissions::validate_mouse_tracking_permissions()?;
    macos::install_pointer_listener(tracker)
}

/// Global mouse listener function with permission validation
#[cfg(target_os = "windows")]
pub fn create_mouse_listener(tracker: Arc<Mutex<MouseTracker>>) -> Result<()> {
    // Check permissions before starting the listener
    if let Err(permission_error) = crate::permissions::validate_mouse_tracking_permissions() {
        return Err(anyhow::anyhow!(
            "Cannot create mouse listener: {}",
            permission_error
        ));
    }

    std::thread::spawn(move || {
        let listener_state = tracker.clone();
        let callback = move |event: Event| {
            // Check if tracking is enabled FIRST to avoid unnecessary work
            let mut tracker_guard = tracker.lock();

            if !tracker_guard.is_tracking {
                return;
            }

            // Queued input retains the capture time, not its processing time.
            if tracker_guard.recording_start_time.is_some_and(|start| event.time < start) {
                return;
            }
            let timestamp = event.time
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let mouse_event = match event.event_type {
                EventType::MouseMove { x, y } => {
                    // Update the last known mouse position for click tracking
                    {
                        let mut pos = LAST_MOUSE_POSITION.lock();
                        *pos = (x, y);
                    }

                    Some(MouseEvent {
                        timestamp,
                        x,
                        y,
                        event_type: MouseEventType::Move,
                        display_id: None, // TODO: Determine display from coordinates
                    })
                }
                EventType::ButtonPress(button) => {
                    // Click callbacks can arrive after movement data, so use the last tracked position.
                    let (click_x, click_y) = {
                        let pos = LAST_MOUSE_POSITION.lock();
                        *pos
                    };

                    println!(
                        "🖱️ Click detected at position: ({:.1}, {:.1}) - Button: {:?}",
                        click_x, click_y, button
                    );

                    Some(MouseEvent {
                        timestamp,
                        x: click_x,
                        y: click_y,
                        event_type: MouseEventType::ButtonPress {
                            button: MouseButton::from(button),
                        },
                        display_id: None,
                    })
                }
                EventType::ButtonRelease(button) => {
                    // Use last known mouse position for releases
                    let (click_x, click_y) = {
                        let pos = LAST_MOUSE_POSITION.lock();
                        *pos
                    };

                    Some(MouseEvent {
                        timestamp,
                        x: click_x,
                        y: click_y,
                        event_type: MouseEventType::ButtonRelease {
                            button: MouseButton::from(button),
                        },
                        display_id: None,
                    })
                }
                EventType::Wheel { delta_x, delta_y } => {
                    // Use last known mouse position for wheel events
                    let (wheel_x, wheel_y) = {
                        let pos = LAST_MOUSE_POSITION.lock();
                        *pos
                    };

                    Some(MouseEvent {
                        timestamp,
                        x: wheel_x,
                        y: wheel_y,
                        event_type: MouseEventType::Wheel { delta_x, delta_y },
                        display_id: None,
                    })
                }
                EventType::KeyPress(key) => {
                    let (is_cmd_modifier, is_char_key) = classify_key(&key);

                    tracker_guard.command_modifiers.update(key, true);

                    // Record a key event for typing detection
                    let is_typing = is_char_key && !tracker_guard.command_modifiers.held();
                    let (cursor_x, cursor_y) = {
                        let pos = LAST_MOUSE_POSITION.lock();
                        *pos
                    };
                    let caret_position = if is_typing {
                        match caret_lookup::without_tracker_lock(&mut tracker_guard, || caret_lookup::for_event(event.time, macos::focused_caret_position)) {
                            Ok(position) => position,
                            Err(()) => return,
                        }
                    } else {
                        None
                    };
                    tracker_guard.add_key_event(KeyEvent { shortcut: None,
                        is_release: false, timestamp,
                        is_modifier: is_cmd_modifier,
                        is_typing,
                        key_motion: key_motion(&key),
                        cursor_x: Some(cursor_x),
                        cursor_y: Some(cursor_y),
                        caret_x: caret_position.map(|(x, _)| x),
                        caret_y: caret_position.map(|(_, y)| y),
                    });

                    None // No mouse event
                }
                EventType::KeyRelease(key) => {
                    let (is_modifier, is_char_key) = classify_key(&key);
                    tracker_guard.add_key_event(KeyEvent::release(timestamp, is_modifier));
                    if is_char_key && !tracker_guard.command_modifiers.held() {
                        let position = match caret_lookup::without_tracker_lock(&mut tracker_guard, || caret_lookup::for_event(event.time, macos::focused_caret_position)) {
                            Ok(position) => position,
                            Err(()) => return,
                        };
                        if let Some((caret_x, caret_y)) = position {
                            tracker_guard.update_latest_typing_caret(caret_x, caret_y);
                        }
                    }

                    tracker_guard.command_modifiers.update(key, false);
                    None // No mouse event
                }
            };

            if let Some(mouse_event) = mouse_event {
                tracker_guard.add_event(mouse_event);
            }
        };

        if let Err(error) = listen(callback) {
            listener_state.lock().is_tracking = false;
            eprintln!("🚨 Mouse tracking error: {:?}", error);
            #[cfg(target_os = "macos")]
            {
                eprintln!("This is likely due to missing Accessibility permissions.");
                eprintln!(
                    "Please enable Accessibility permissions in System Preferences and restart Tarantino."
                );
            }
        }
    });

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn create_mouse_listener(tracker: Arc<Mutex<MouseTracker>>) -> Result<()> {
    linux::create_mouse_listener(tracker)
}

#[cfg(target_os = "linux")]
pub fn pointer_tracking_available() -> bool {
    linux::pointer_tracking_available()
}

#[cfg(not(target_os = "linux"))]
pub fn pointer_tracking_available() -> bool {
    true
}

/// Statistics about recorded mouse events
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MouseTrackingStats {
    pub total_events: usize,
    pub move_events: usize,
    pub click_events: usize,
    pub scroll_events: usize,
    pub duration_ms: u64,
    pub first_event_timestamp: Option<u64>,
    pub last_event_timestamp: Option<u64>,
}

impl MouseTracker {
    /// Generate statistics about recorded events
    pub fn get_stats(&self) -> MouseTrackingStats {
        {
            let events = self.events.lock();
            let mut stats = MouseTrackingStats {
                total_events: events.len(),
                move_events: 0,
                click_events: 0,
                scroll_events: 0,
                duration_ms: 0,
                first_event_timestamp: None,
                last_event_timestamp: None,
            };

            if !events.is_empty() {
                stats.first_event_timestamp = Some(events.front().unwrap().timestamp);
                stats.last_event_timestamp = Some(events.back().unwrap().timestamp);
                stats.duration_ms = stats.last_event_timestamp.unwrap_or(0)
                    - stats.first_event_timestamp.unwrap_or(0);

                for event in events.iter() {
                    match event.event_type {
                        MouseEventType::Move => stats.move_events += 1,
                        MouseEventType::ButtonPress { .. }
                        | MouseEventType::ButtonRelease { .. } => {
                            stats.click_events += 1;
                        }
                        MouseEventType::Wheel { .. } => stats.scroll_events += 1,
                    }
                }
            }

            stats
        }
    }
}
