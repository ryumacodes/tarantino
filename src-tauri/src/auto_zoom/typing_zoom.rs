use crate::event_capture::EnhancedMouseEvent;
use crate::input::{KeyEvent, KeyMotion, MouseEventType};

use super::{ZoomBlock, ZoomCenter};

const SYNTHETIC_PAN_EDGE_PADDING: f64 = 0.08;
const SYNTHETIC_CHAR_ADVANCE: f64 = 0.008;
const SYNTHETIC_TAB_ADVANCE: f64 = SYNTHETIC_CHAR_ADVANCE * 4.0;
const SYNTHETIC_LINE_HEIGHT: f64 = 0.055;
const SYNTHETIC_TEXT_START_BACKSET: f64 = 0.02;
const CENTER_MOVEMENT_EPSILON: f64 = 0.005;

#[derive(Debug, Clone, Copy)]
struct SyntheticTextArea {
    left: f64,
    right: f64,
    bottom: f64,
    line_height: f64,
}

pub(super) struct TypingSession {
    pub(super) start_time: u64, // First keystroke timestamp
    pub(super) end_time: u64,   // Last keystroke timestamp
    pub(super) key_count: usize,
    pub(super) cursor_x: f64, // Cursor position at session start (normalized 0-1)
    pub(super) cursor_y: f64,
    pub(super) centers: Vec<ZoomCenter>,
}

/// Configuration for typing-based zoom detection
pub(super) struct TypingZoomConfig {
    min_typing_keys: usize, // Minimum keystrokes to trigger zoom (default: 1)
    session_gap_ms: u64,    // Max gap between keys in one session (default: 5000)
    hold_after_ms: u64,     // Hold zoom after last key (default: 5000)
    zoom_in_before_ms: u64, // Start zoom this much before first key (default: 500)
    zoom_factor: f32,       // Zoom level (default: 2.0)
}

impl Default for TypingZoomConfig {
    fn default() -> Self {
        Self {
            min_typing_keys: 1,
            session_gap_ms: 5000,
            hold_after_ms: 5000,
            zoom_in_before_ms: 500,
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
    let mut session_keys = vec![typing_keys[0]];

    for key in &typing_keys[1..] {
        let has_new_anchor = has_click_between(mouse_events, session_end, key.timestamp);
        if !has_new_anchor && key.timestamp.saturating_sub(session_end) <= config.session_gap_ms {
            // Continue current session
            session_end = key.timestamp;
            key_count += 1;
            session_keys.push(key);
        } else {
            // End current session, start new one
            if key_count >= config.min_typing_keys {
                let centers = build_typing_block_centers(&session_keys, mouse_events, metadata);
                let (cx, cy) = centers
                    .first()
                    .map(|center| (center.x, center.y))
                    .unwrap_or_else(|| {
                        lookup_typing_cursor_position(session_first_key, mouse_events, metadata)
                    });
                sessions.push(TypingSession {
                    start_time: session_start,
                    end_time: session_end,
                    key_count,
                    cursor_x: cx,
                    cursor_y: cy,
                    centers,
                });
            }
            session_first_key = key;
            session_start = key.timestamp;
            session_end = key.timestamp;
            key_count = 1;
            session_keys = vec![key];
        }
    }

    // Finalize last session
    if key_count >= config.min_typing_keys {
        let centers = build_typing_block_centers(&session_keys, mouse_events, metadata);
        let (cx, cy) = centers
            .first()
            .map(|center| (center.x, center.y))
            .unwrap_or_else(|| {
                lookup_typing_cursor_position(session_first_key, mouse_events, metadata)
            });
        sessions.push(TypingSession {
            start_time: session_start,
            end_time: session_end,
            key_count,
            cursor_x: cx,
            cursor_y: cy,
            centers,
        });
    }

    println!("⌨️ [TYPING] Found {} typing sessions", sessions.len());
    for (i, s) in sessions.iter().enumerate() {
        let (min_x, max_x, min_y, max_y) = center_ranges(&s.centers);
        println!(
            "   Session {}: {}ms-{}ms, {} keys, cursor=({:.2}, {:.2}), centers x={:.3}-{:.3} y={:.3}-{:.3}",
            i,
            s.start_time,
            s.end_time,
            s.key_count,
            s.cursor_x,
            s.cursor_y,
            min_x,
            max_x,
            min_y,
            max_y
        );
    }

    sessions
}

fn has_click_between(
    mouse_events: &[EnhancedMouseEvent],
    after_ms: u64,
    before_or_at_ms: u64,
) -> bool {
    mouse_events.iter().any(|event| {
        event.base.timestamp > after_ms
            && event.base.timestamp <= before_or_at_ms
            && matches!(event.base.event_type, MouseEventType::ButtonPress { .. })
    })
}

fn build_typing_block_centers(
    session_keys: &[&KeyEvent],
    mouse_events: &[EnhancedMouseEvent],
    metadata: &crate::event_capture::SessionMetadata,
) -> Vec<ZoomCenter> {
    if session_keys.is_empty() {
        return vec![];
    }

    let caret_centers: Vec<ZoomCenter> = session_keys
        .iter()
        .filter_map(|key| typing_caret_center(key, metadata))
        .collect();

    if centers_have_movement(&caret_centers) {
        return caret_centers;
    }

    let first_key = session_keys[0];
    let start = caret_centers.first().cloned().unwrap_or_else(|| {
        let (x, y) = lookup_typing_anchor_position(first_key, mouse_events, metadata);
        ZoomCenter {
            x,
            y,
            time: first_key.timestamp,
        }
    });

    if session_keys.len() == 1 {
        return vec![start];
    }

    build_synthetic_typing_centers(session_keys, start)
}

fn typing_caret_center(
    key: &KeyEvent,
    metadata: &crate::event_capture::SessionMetadata,
) -> Option<ZoomCenter> {
    key.caret_x.zip(key.caret_y).map(|(x, y)| {
        let (x, y) = normalize_coordinates(x, y, metadata);
        ZoomCenter {
            x,
            y,
            time: key.timestamp,
        }
    })
}

fn lookup_typing_anchor_position(
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

fn build_synthetic_typing_centers(
    session_keys: &[&KeyEvent],
    start: ZoomCenter,
) -> Vec<ZoomCenter> {
    if session_keys.is_empty() {
        return vec![];
    }

    let area = synthetic_text_area(start.x, start.y);
    let mut centers = Vec::with_capacity(session_keys.len());
    let mut x = start.x.clamp(area.left, area.right);
    let mut y = start.y.clamp(SYNTHETIC_PAN_EDGE_PADDING, area.bottom);
    let mut line_start_x = x;

    centers.push(ZoomCenter {
        x,
        y,
        time: session_keys[0].timestamp,
    });

    for key in session_keys.iter().skip(1) {
        match key.key_motion {
            KeyMotion::Newline => {
                x = line_start_x;
                y = (y + area.line_height).min(area.bottom);
            }
            KeyMotion::Backspace => {
                x = (x - SYNTHETIC_CHAR_ADVANCE).max(line_start_x);
            }
            KeyMotion::Tab => {
                x += SYNTHETIC_TAB_ADVANCE;
            }
            KeyMotion::Character => {
                x += SYNTHETIC_CHAR_ADVANCE;
            }
        }

        if x > area.right {
            x = line_start_x;
            y = (y + area.line_height).min(area.bottom);
        }

        x = x.clamp(area.left, area.right);
        centers.push(ZoomCenter {
            x,
            y,
            time: key.timestamp,
        });

        if matches!(key.key_motion, KeyMotion::Newline) {
            line_start_x = x;
        }
    }

    collapse_static_centers(centers)
}

fn synthetic_text_area(start_x: f64, _start_y: f64) -> SyntheticTextArea {
    let left = (start_x - SYNTHETIC_TEXT_START_BACKSET)
        .clamp(SYNTHETIC_PAN_EDGE_PADDING, 1.0 - SYNTHETIC_PAN_EDGE_PADDING);
    let right = (1.0 - SYNTHETIC_PAN_EDGE_PADDING).max(left + SYNTHETIC_CHAR_ADVANCE);
    let bottom = 1.0 - SYNTHETIC_PAN_EDGE_PADDING;

    SyntheticTextArea {
        left,
        right,
        bottom,
        line_height: SYNTHETIC_LINE_HEIGHT,
    }
}

fn collapse_static_centers(centers: Vec<ZoomCenter>) -> Vec<ZoomCenter> {
    if centers.is_empty() || centers_have_movement(&centers) {
        centers
    } else {
        vec![centers[0].clone()]
    }
}

fn centers_have_movement(centers: &[ZoomCenter]) -> bool {
    if centers.len() < 2 {
        return false;
    }

    let (min_x, max_x, min_y, max_y) = center_ranges(centers);
    (max_x - min_x) > CENTER_MOVEMENT_EPSILON || (max_y - min_y) > CENTER_MOVEMENT_EPSILON
}

fn center_ranges(centers: &[ZoomCenter]) -> (f64, f64, f64, f64) {
    if centers.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }

    centers.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), center| {
            (
                min_x.min(center.x),
                max_x.max(center.x),
                min_y.min(center.y),
                max_y.max(center.y),
            )
        },
    )
}

fn lookup_typing_cursor_position(
    key: &KeyEvent,
    mouse_events: &[EnhancedMouseEvent],
    metadata: &crate::event_capture::SessionMetadata,
) -> (f64, f64) {
    if let (Some(x), Some(y)) = (key.caret_x, key.caret_y) {
        return normalize_coordinates(x, y, metadata);
    }

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
    let idx = mouse_events.partition_point(|e| e.base.timestamp <= time_ms);
    if idx == 0 {
        return (0.5, 0.5);
    }

    let event = &mouse_events[idx - 1];

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
            let end_time = (session.end_time + config.hold_after_ms).min(duration_ms);

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
                centers: session.centers.clone(),
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
