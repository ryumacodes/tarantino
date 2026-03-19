#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use rdev::{listen, Event, EventType, Button, Key};
use tokio::sync::broadcast;
use anyhow::Result;

/// Global state for tracking the last known mouse position (Screen Studio approach)
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

/// Lightweight key event for typing detection (no key content for privacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    /// Timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Whether this is a modifier key (Cmd, Ctrl, Alt — NOT Shift)
    pub is_modifier: bool,
    /// Whether this counts as a typing keystroke (unmodified character key)
    pub is_typing: bool,
}

/// Track whether a command modifier (Cmd/Ctrl/Alt) is currently held.
/// Shift is excluded so Shift+letter still counts as typing.
static CMD_HELD: AtomicBool = AtomicBool::new(false);
static CTRL_HELD: AtomicBool = AtomicBool::new(false);
static ALT_HELD: AtomicBool = AtomicBool::new(false);

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
        Key::CapsLock | Key::Escape | Key::F1 | Key::F2 | Key::F3 | Key::F4
        | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10
        | Key::F11 | Key::F12 | Key::PrintScreen | Key::ScrollLock | Key::Pause
        | Key::Insert | Key::Home | Key::End | Key::PageUp | Key::PageDown
        | Key::UpArrow | Key::DownArrow | Key::LeftArrow | Key::RightArrow
        | Key::NumLock => (false, false),
        // Everything else (letters, numbers, space, enter, backspace, tab,
        // punctuation, etc.) counts as a character/typing key
        _ => (false, true),
    }
}

/// Check if any command modifier is currently held
fn any_modifier_held() -> bool {
    CMD_HELD.load(Ordering::Relaxed)
        || CTRL_HELD.load(Ordering::Relaxed)
        || ALT_HELD.load(Ordering::Relaxed)
}

/// Mouse tracking state and configuration
#[derive(Debug)]
pub struct MouseTracker {
    /// Whether mouse tracking is currently active
    pub is_tracking: bool,
    /// Recorded mouse events buffer
    pub events: Arc<Mutex<VecDeque<MouseEvent>>>,
    /// Recorded key events buffer (for typing detection)
    pub key_events: Arc<Mutex<VecDeque<KeyEvent>>>,
    /// Maximum number of events to keep in memory
    pub max_buffer_size: usize,
    /// Event broadcaster for real-time updates
    pub event_sender: broadcast::Sender<MouseEvent>,
    /// Recording start time for relative timestamps
    pub recording_start_time: Option<SystemTime>,
}

impl MouseTracker {
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        Self {
            is_tracking: false,
            events: Arc::new(Mutex::new(VecDeque::new())),
            key_events: Arc::new(Mutex::new(VecDeque::new())),
            max_buffer_size: 100_000, // Limit to prevent memory issues
            event_sender,
            recording_start_time: None,
        }
    }

    /// Start mouse tracking with permission validation
    pub fn start_tracking(&mut self) -> Result<()> {
        println!("🖱️ [TRACKER] start_tracking called, current is_tracking: {}", self.is_tracking);

        if self.is_tracking {
            println!("🖱️ [TRACKER] Already tracking, returning early");
            return Ok(()); // Already tracking
        }

        // Validate accessibility permissions before starting
        if let Err(permission_error) = crate::permissions::validate_mouse_tracking_permissions() {
            println!("❌ [TRACKER] Permission validation failed: {}", permission_error);
            return Err(anyhow::anyhow!("Cannot start mouse tracking: {}", permission_error));
        }

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
        // Reset modifier state
        CMD_HELD.store(false, Ordering::Relaxed);
        CTRL_HELD.store(false, Ordering::Relaxed);
        ALT_HELD.store(false, Ordering::Relaxed);

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

    /// Add a new mouse event to the buffer
    pub fn add_event(&self, event: MouseEvent) {
        if !self.is_tracking {
            return;
        }

        // Add to buffer with size limit
        let current_count = {
            let mut events = self.events.lock();
            events.push_back(event.clone());

            // Maintain buffer size limit
            while events.len() > self.max_buffer_size {
                events.pop_front();
            }

            events.len()
        };

        // Log first event and every 500th event, plus all clicks
        let is_click = matches!(event.event_type, MouseEventType::ButtonPress { .. });
        if current_count == 1 {
            println!("🖱️ [TRACKER] First event captured! Total: {}", current_count);
        } else if current_count % 500 == 0 {
            println!("🖱️ [TRACKER] Event milestone: {} events captured", current_count);
        }

        if is_click {
            println!("🖱️ [TRACKER] CLICK captured at ({:.1}, {:.1}) - Total events: {}", event.x, event.y, current_count);
        }

        // Broadcast event for real-time updates
        let _ = self.event_sender.send(event);
    }

    /// Get all recorded events
    pub fn get_events(&self) -> Vec<MouseEvent> {
        let events = self.events.lock();
        events.iter().cloned().collect()
    }

    /// Get events within a time range
    pub fn get_events_in_range(&self, start_ms: u64, end_ms: u64) -> Vec<MouseEvent> {
        let events = self.events.lock();
        events
            .iter()
            .filter(|event| event.timestamp >= start_ms && event.timestamp <= end_ms)
            .cloned()
            .collect()
    }

    /// Clear all recorded events
    pub fn clear_events(&self) {
        let mut events = self.events.lock();
        events.clear();
    }

    /// Add a key event to the buffer
    pub fn add_key_event(&self, event: KeyEvent) {
        if !self.is_tracking {
            return;
        }

        let mut key_events = self.key_events.lock();
        key_events.push_back(event);

        // Maintain buffer size limit
        while key_events.len() > self.max_buffer_size {
            key_events.pop_front();
        }

        let count = key_events.len();
        if count == 1 {
            println!("⌨️ [TRACKER] First key event captured!");
        } else if count % 200 == 0 {
            println!("⌨️ [TRACKER] Key event milestone: {} events", count);
        }
    }

    /// Get all recorded key events
    pub fn get_key_events(&self) -> Vec<KeyEvent> {
        let key_events = self.key_events.lock();
        key_events.iter().cloned().collect()
    }
}

/// Global mouse listener function with permission validation
pub fn create_mouse_listener(tracker: Arc<Mutex<MouseTracker>>) -> Result<()> {
    // Check permissions before starting the listener
    if let Err(permission_error) = crate::permissions::validate_mouse_tracking_permissions() {
        return Err(anyhow::anyhow!("Cannot create mouse listener: {}", permission_error));
    }

    std::thread::spawn(move || {
        let callback = move |event: Event| {
            // Check if tracking is enabled FIRST to avoid unnecessary work
            let tracker_guard = tracker.lock();

            if !tracker_guard.is_tracking {
                return;
            }

            // Only log in debug builds and sparingly
            #[cfg(debug_assertions)]
            {
                static LOG_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let count = LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if count % 100 == 0 {
                    println!("DEBUG: Mouse event #{} (tracking active)", count);
                }
            }

            let timestamp = SystemTime::now()
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
                    // Use last known mouse position for clicks (critical for Screen Studio zoom)
                    let (click_x, click_y) = {
                        let pos = LAST_MOUSE_POSITION.lock();
                        *pos
                    };
                    
                    println!("🖱️ Click detected at position: ({:.1}, {:.1}) - Button: {:?}", click_x, click_y, button);
                    
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

                    if is_cmd_modifier {
                        // Track modifier press
                        match key {
                            Key::MetaLeft | Key::MetaRight => CMD_HELD.store(true, Ordering::Relaxed),
                            Key::ControlLeft | Key::ControlRight => CTRL_HELD.store(true, Ordering::Relaxed),
                            Key::Alt | Key::AltGr => ALT_HELD.store(true, Ordering::Relaxed),
                            _ => {}
                        }
                    }

                    // Record a key event for typing detection
                    let is_typing = is_char_key && !any_modifier_held();
                    tracker_guard.add_key_event(KeyEvent {
                        timestamp,
                        is_modifier: is_cmd_modifier,
                        is_typing,
                    });

                    None // No mouse event
                }
                EventType::KeyRelease(key) => {
                    // Update modifier held state on release
                    match key {
                        Key::MetaLeft | Key::MetaRight => CMD_HELD.store(false, Ordering::Relaxed),
                        Key::ControlLeft | Key::ControlRight => CTRL_HELD.store(false, Ordering::Relaxed),
                        Key::Alt | Key::AltGr => ALT_HELD.store(false, Ordering::Relaxed),
                        _ => {}
                    }
                    None // No mouse event
                }
            };

            if let Some(mouse_event) = mouse_event {
                tracker_guard.add_event(mouse_event);
            }
        };

        // Start the rdev listener - this blocks the thread
        if let Err(error) = listen(callback) {
            eprintln!("🚨 Mouse tracking error: {:?}", error);
            eprintln!("This is likely due to missing Accessibility permissions.");
            eprintln!("Please enable Accessibility permissions in System Preferences and restart Tarantino.");
        }
    });

    Ok(())
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
                        MouseEventType::ButtonPress { .. } | MouseEventType::ButtonRelease { .. } => {
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