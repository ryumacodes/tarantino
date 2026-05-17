use crate::event_capture::EnhancedMouseEvent;
use crate::mouse_tracking::KeyEvent;

use super::{ZoomBlock, ZoomCenter};

pub(super) struct TypingSession {
    pub(super) start_time: u64, // First keystroke timestamp
    pub(super) end_time: u64,   // Last keystroke timestamp
    pub(super) key_count: usize,
    pub(super) cursor_x: f64, // Cursor position at session start (normalized 0-1)
    pub(super) cursor_y: f64,
}

/// Configuration for typing-based zoom detection
pub(super) struct TypingZoomConfig {
    min_typing_keys: usize,    // Minimum keystrokes to trigger zoom (default: 1)
    session_gap_ms: u64,       // Max gap between keys in one session (default: 5000)
    hold_after_ms: u64,        // Hold zoom after last key (default: 5000)
    zoom_in_before_ms: u64,    // Start zoom this much before first key (default: 500)
    zoom_out_duration_ms: u64, // Duration of zoom-out animation (default: 1000)
    zoom_factor: f32,          // Zoom level (default: 2.0)
}

impl Default for TypingZoomConfig {
    fn default() -> Self {
        Self {
            min_typing_keys: 1,
            session_gap_ms: 5000,
            hold_after_ms: 5000,
            zoom_in_before_ms: 500,
            zoom_out_duration_ms: 1000,
            zoom_factor: 2.0,
        }
    }
}

pub(super) fn detect_typing_sessions(
    key_events: &[KeyEvent],
    mouse_events: &[EnhancedMouseEvent],
    metadata: &crate::event_capture::SessionMetadata,
    config: &TypingZoomConfig,
) -> Vec<TypingSession> {
    // Filter to typing-only key events
    let typing_keys: Vec<&KeyEvent> = key_events.iter().filter(|k| k.is_typing).collect();
    if typing_keys.is_empty() {
        return vec![];
    }

    println!(
        "⌨️ [TYPING] Detecting sessions from {} typing keys",
        typing_keys.len()
    );

    let mut sessions: Vec<TypingSession> = Vec::new();
    let mut session_first_key = typing_keys[0];
    let mut session_start = typing_keys[0].timestamp;
    let mut session_end = typing_keys[0].timestamp;
    let mut key_count = 1usize;

    for key in &typing_keys[1..] {
        if key.timestamp.saturating_sub(session_end) <= config.session_gap_ms {
            // Continue current session
            session_end = key.timestamp;
            key_count += 1;
        } else {
            // End current session, start new one
            if key_count >= config.min_typing_keys {
                let (cx, cy) =
                    lookup_typing_cursor_position(session_first_key, mouse_events, metadata);
                sessions.push(TypingSession {
                    start_time: session_start,
                    end_time: session_end,
                    key_count,
                    cursor_x: cx,
                    cursor_y: cy,
                });
            }
            session_first_key = key;
            session_start = key.timestamp;
            session_end = key.timestamp;
            key_count = 1;
        }
    }

    // Finalize last session
    if key_count >= config.min_typing_keys {
        let (cx, cy) = lookup_typing_cursor_position(session_first_key, mouse_events, metadata);
        sessions.push(TypingSession {
            start_time: session_start,
            end_time: session_end,
            key_count,
            cursor_x: cx,
            cursor_y: cy,
        });
    }

    println!("⌨️ [TYPING] Found {} typing sessions", sessions.len());
    for (i, s) in sessions.iter().enumerate() {
        println!(
            "   Session {}: {}ms-{}ms, {} keys, cursor=({:.2}, {:.2})",
            i, s.start_time, s.end_time, s.key_count, s.cursor_x, s.cursor_y
        );
    }

    sessions
}

fn lookup_typing_cursor_position(
    key: &KeyEvent,
    mouse_events: &[EnhancedMouseEvent],
    metadata: &crate::event_capture::SessionMetadata,
) -> (f64, f64) {
    if let (Some(x), Some(y)) = (key.cursor_x, key.cursor_y) {
        normalize_coordinates(x, y, metadata)
    } else {
        lookup_cursor_position(key.timestamp, mouse_events, metadata)
    }
}

/// Look up the cursor position at a given time via binary search on mouse events.
/// Returns normalized (0-1) coordinates. Falls back to (0.5, 0.5) if no events found.
fn lookup_cursor_position(
    time_ms: u64,
    mouse_events: &[EnhancedMouseEvent],
    metadata: &crate::event_capture::SessionMetadata,
) -> (f64, f64) {
    if mouse_events.is_empty() {
        return (0.5, 0.5);
    }

    // Binary search for the closest event at or before `time_ms`
    let idx = mouse_events
        .partition_point(|e| e.base.timestamp <= time_ms)
        .saturating_sub(1);

    let event = &mouse_events[idx];

    normalize_coordinates(event.base.x, event.base.y, metadata)
}

fn normalize_coordinates(
    x: f64,
    y: f64,
    metadata: &crate::event_capture::SessionMetadata,
) -> (f64, f64) {
    // Normalize coordinates using capture_region or full display
    let (eff_x, eff_y, eff_w, eff_h) = if let Some((rx, ry, rw, rh)) = metadata.capture_region {
        (rx as f64, ry as f64, rw as f64, rh as f64)
    } else {
        (
            0.0,
            0.0,
            metadata.display_resolution.0 as f64,
            metadata.display_resolution.1 as f64,
        )
    };

    let norm_x = ((x - eff_x) / eff_w).clamp(0.0, 1.0);
    let norm_y = ((y - eff_y) / eff_h).clamp(0.0, 1.0);

    (norm_x, norm_y)
}

/// Create zoom blocks from detected typing sessions
pub(super) fn create_typing_zoom_blocks(
    sessions: &[TypingSession],
    config: &TypingZoomConfig,
    duration_ms: u64,
    id_offset: usize,
) -> Vec<ZoomBlock> {
    sessions
        .iter()
        .enumerate()
        .filter_map(|(i, session)| {
            let start_time = session.start_time.saturating_sub(config.zoom_in_before_ms);
            let end_time = (session.end_time + config.hold_after_ms + config.zoom_out_duration_ms)
                .min(duration_ms);

            // Skip blocks that are too short
            if end_time <= start_time + 500 {
                return None;
            }

            Some(ZoomBlock {
                id: format!("typing_zoom_{}", id_offset + i),
                click_x: session.cursor_x,
                click_y: session.cursor_y,
                center_x: session.cursor_x,
                center_y: session.cursor_y,
                start_time,
                end_time,
                zoom_factor: config.zoom_factor,
                is_manual: false,
                centers: vec![ZoomCenter {
                    x: session.cursor_x,
                    y: session.cursor_y,
                    time: session.start_time,
                }],
                kind: "typing".to_string(),
                zoom_in_speed: None,
                zoom_out_speed: None,
            })
        })
        .collect()
}

/// Validate zoom blocks: sort by time, clamp to duration, zero-gap truncation for overlaps
pub fn validate_zoom_blocks(blocks: &mut Vec<ZoomBlock>, duration_ms: u64) {
    if blocks.is_empty() {
        return;
    }

    blocks.sort_by_key(|b| b.start_time);

    for block in blocks.iter_mut() {
        block.end_time = block.end_time.min(duration_ms);
    }

    // Zero-gap: truncate A.end to B.start (no merge, no gap)
    for i in 1..blocks.len() {
        if blocks[i - 1].end_time > blocks[i].start_time {
            blocks[i - 1].end_time = blocks[i].start_time;
        }
    }

    // Remove blocks shorter than 500ms
    blocks.retain(|b| b.end_time > b.start_time + 500);
}
