#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::event_capture::{CaptureSession, EnhancedMouseEvent};
use crate::mouse_tracking::KeyEvent;

/// Simple zoom configuration following Screen Studio's approach
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomConfig {
    pub enabled: bool,
    pub zoom_factor: f32,          // Default: 2.0x (Screen Studio typical)
    pub zoom_duration: u64,        // Default: 5000ms (5s: 1s in, 3s hold, 1s out)
    pub min_click_spacing: u64,    // Minimum time between click zooms (500ms)
}

impl Default for ZoomConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            zoom_factor: 2.0,
            zoom_duration: 5000,      // 5 seconds total (1s zoom in, 3s hold, 1s zoom out)
            min_click_spacing: 500,
        }
    }
}

/// A re-center point within a zoom block (when user clicks a new location while zoomed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomCenter {
    pub x: f64,               // Normalized center X (0-1)
    pub y: f64,               // Normalized center Y (0-1)
    pub time: u64,            // When to start panning to this center (ms)
}

/// Simple click-based zoom block (like Screen Studio's purple blocks)
/// Consecutive clicks merge into one block with multiple centers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomBlock {
    pub id: String,
    pub click_x: f64,         // First click position X
    pub click_y: f64,         // First click position Y
    pub center_x: f64,        // Current/initial zoom center X
    pub center_y: f64,        // Current/initial zoom center Y
    pub start_time: u64,      // When zoom starts
    pub end_time: u64,        // When zoom ends
    pub zoom_factor: f32,     // Zoom level (default from config)
    pub is_manual: bool,      // True if user manually adjusted the zoom area
    #[serde(default)]
    pub centers: Vec<ZoomCenter>, // Re-center points from merged clicks
    #[serde(default = "default_kind")]
    pub kind: String,              // "click" or "typing"
    #[serde(default)]
    pub zoom_in_speed: Option<String>,
    #[serde(default)]
    pub zoom_out_speed: Option<String>,
}

fn default_kind() -> String { "click".into() }

/// Zoom analysis result following Screen Studio pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomAnalysis {
    pub zoom_blocks: Vec<ZoomBlock>,
    pub total_clicks: usize,
    pub session_duration: u64,
    pub config: ZoomConfig,
}

/// A detected typing session (consecutive keystrokes with small gaps)
#[derive(Debug, Clone)]
struct TypingSession {
    start_time: u64,   // First keystroke timestamp
    end_time: u64,     // Last keystroke timestamp
    key_count: usize,
    cursor_x: f64,     // Cursor position at session start (normalized 0-1)
    cursor_y: f64,
}

/// Configuration for typing-based zoom detection
struct TypingZoomConfig {
    min_typing_keys: usize, // Minimum keystrokes to trigger zoom (default: 3)
    session_gap_ms: u64,    // Max gap between keys in one session (default: 5000)
    hold_after_ms: u64,     // Hold zoom after last key (default: 5000)
    zoom_in_before_ms: u64, // Start zoom this much before first key (default: 500)
    zoom_out_duration_ms: u64, // Duration of zoom-out animation (default: 1000)
    zoom_factor: f32,       // Zoom level (default: 2.0)
}

impl Default for TypingZoomConfig {
    fn default() -> Self {
        Self {
            min_typing_keys: 3,
            session_gap_ms: 5000,
            hold_after_ms: 5000,
            zoom_in_before_ms: 500,
            zoom_out_duration_ms: 1000,
            zoom_factor: 2.0,
        }
    }
}

/// Simple zoom processor following Screen Studio's approach
pub struct ZoomProcessor {
    config: ZoomConfig,
}

impl ZoomProcessor {
    pub fn new(config: ZoomConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(ZoomConfig::default())
    }

    /// Analyze capture session and create zoom blocks for clicks and typing
    pub fn analyze_session(&self, session: &CaptureSession, key_events: &[KeyEvent]) -> Result<ZoomAnalysis> {
        println!("🔍 [ZOOM] analyze_session called");
        println!("🔍 [ZOOM] Session ID: {}", session.session_id);
        println!("🔍 [ZOOM] Total mouse events in session: {}", session.mouse_events.len());
        println!("🔍 [ZOOM] Total key events: {}", key_events.len());
        println!("🔍 [ZOOM] Session start_time: {}", session.start_time);
        println!("🔍 [ZOOM] Session end_time: {:?}", session.end_time);
        println!("🔍 [ZOOM] Display resolution: {:?}", session.metadata.display_resolution);

        if !self.config.enabled {
            println!("⚠️ [ZOOM] Zoom creation disabled in config");
            return Ok(ZoomAnalysis {
                zoom_blocks: vec![],
                total_clicks: 0,
                session_duration: session.end_time.unwrap_or(session.start_time) - session.start_time,
                config: self.config.clone(),
            });
        }

        // Extract mouse click events (button presses only)
        let click_events = self.extract_click_events(session);
        println!("🖱️ [ZOOM] Extracted {} click events (ButtonPress only)", click_events.len());

        // Log first few clicks for debugging
        for (i, event) in click_events.iter().take(5).enumerate() {
            println!("   Click {}: time={}ms, pos=({:.1}, {:.1})",
                i, event.base.timestamp, event.base.x, event.base.y);
        }
        if click_events.len() > 5 {
            println!("   ... and {} more clicks", click_events.len() - 5);
        }

        // Create zoom blocks for clicks (Screen Studio style)
        let mut zoom_blocks = self.create_zoom_blocks(&click_events, session)?;
        println!("✅ [ZOOM] Created {} zoom blocks from {} clicks", zoom_blocks.len(), click_events.len());

        // Detect typing sessions and create additional zoom blocks
        let typing_config = TypingZoomConfig::default();
        let typing_sessions = detect_typing_sessions(key_events, &session.mouse_events, &session.metadata, &typing_config);
        if !typing_sessions.is_empty() {
            let session_duration = session.end_time.unwrap_or(session.start_time) - session.start_time;
            let typing_blocks = create_typing_zoom_blocks(&typing_sessions, &typing_config, session_duration, zoom_blocks.len());
            println!("⌨️ [ZOOM] Created {} typing zoom blocks from {} typing sessions",
                typing_blocks.len(), typing_sessions.len());
            zoom_blocks.extend(typing_blocks);
        }

        let session_duration = session.end_time.unwrap_or(session.start_time) - session.start_time;
        // Re-validate after merging click + typing blocks (handles overlaps)
        validate_zoom_blocks(&mut zoom_blocks, session_duration);
        println!("📊 [ZOOM] Final: {} zoom blocks, session duration: {}ms", zoom_blocks.len(), session_duration);

        Ok(ZoomAnalysis {
            zoom_blocks,
            total_clicks: click_events.len(),
            session_duration,
            config: self.config.clone(),
        })
    }

    /// Extract click events from enhanced mouse events (button presses only)
    fn extract_click_events(&self, session: &CaptureSession) -> Vec<EnhancedMouseEvent> {
        session.mouse_events.iter()
            .filter(|event| {
                matches!(event.base.event_type, 
                    crate::mouse_tracking::MouseEventType::ButtonPress { .. }
                )
            })
            .cloned()
            .collect()
    }

    /// Create zoom blocks for clicks (Screen Studio approach).
    /// Consecutive clicks that fall within an active zoom are MERGED into one
    /// continuous block with multiple re-center points — no zoom-out between them.
    fn create_zoom_blocks(&self, click_events: &[EnhancedMouseEvent], session: &CaptureSession) -> Result<Vec<ZoomBlock>> {
        let mut zoom_blocks: Vec<ZoomBlock> = Vec::new();
        let mut last_zoom_time = 0u64;

        // Timing constants
        let zoom_in_duration = 1000u64;   // 1 second to zoom in before click
        let hold_duration = 3000u64;      // 3 seconds hold at peak after click
        let zoom_out_duration = 1000u64;  // 1 second to zoom out
        let session_duration = session.end_time.unwrap_or(session.start_time) - session.start_time;

        for (i, event) in click_events.iter().enumerate() {
            // Skip clicks that are too close in time to the previous zoom
            if event.base.timestamp.saturating_sub(last_zoom_time) < self.config.min_click_spacing {
                continue;
            }

            let (norm_x, norm_y) = self.normalize_coordinates(
                event.base.x,
                event.base.y,
                &session.metadata
            );

            // Check if this click falls within (or near) the previous block's active range.
            // If so, merge it into that block instead of creating a new one.
            let merged = if let Some(prev_block) = zoom_blocks.last_mut() {
                // The previous block is "active" up to its end_time minus the zoom-out tail.
                // If the click lands before the block would have started zooming out,
                // merge it in and extend the block.
                let prev_active_end = prev_block.end_time.saturating_sub(zoom_out_duration);
                if event.base.timestamp <= prev_active_end + 500 {
                    // Merge: add a re-center point and extend the block
                    prev_block.centers.push(ZoomCenter {
                        x: norm_x,
                        y: norm_y,
                        time: event.base.timestamp,
                    });
                    // Extend end_time from this click
                    let new_end = (event.base.timestamp + hold_duration + zoom_out_duration).min(session_duration);
                    prev_block.end_time = new_end;
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if !merged {
                let start_time = event.base.timestamp.saturating_sub(zoom_in_duration);
                let end_time = (event.base.timestamp + hold_duration + zoom_out_duration).min(session_duration);

                if end_time <= start_time + zoom_in_duration + 500 {
                    continue;
                }

                zoom_blocks.push(ZoomBlock {
                    id: format!("zoom_{}", i),
                    click_x: norm_x,
                    click_y: norm_y,
                    center_x: norm_x,
                    center_y: norm_y,
                    start_time,
                    end_time,
                    zoom_factor: self.config.zoom_factor,
                    is_manual: false,
                    centers: vec![ZoomCenter {
                        x: norm_x,
                        y: norm_y,
                        time: event.base.timestamp,
                    }],
                    kind: "click".to_string(),
                    zoom_in_speed: None,
                    zoom_out_speed: None,
                });
            }

            last_zoom_time = event.base.timestamp;
        }

        validate_zoom_blocks(&mut zoom_blocks, session_duration);
        Ok(zoom_blocks)
    }
    
    /// Normalize screen coordinates to 0-1 range
    /// Uses capture_region for partial recordings to match cursor coordinate system
    fn normalize_coordinates(&self, x: f64, y: f64, metadata: &crate::event_capture::SessionMetadata) -> (f64, f64) {
        // Use capture_region for partial recordings, fall back to full display
        let (eff_x, eff_y, eff_w, eff_h) = if let Some((rx, ry, rw, rh)) = metadata.capture_region {
            (rx as f64, ry as f64, rw as f64, rh as f64)
        } else {
            (0.0, 0.0, metadata.display_resolution.0 as f64, metadata.display_resolution.1 as f64)
        };

        // Adjust coordinates relative to recording area (matches cursor normalization in cursor_renderer.rs)
        let adjusted_x = x - eff_x;
        let adjusted_y = y - eff_y;
        let norm_x = (adjusted_x / eff_w).clamp(0.0, 1.0);
        let norm_y = (adjusted_y / eff_h).clamp(0.0, 1.0);

        (norm_x, norm_y)
    }

}

/// Detect typing sessions from key events by grouping consecutive typing keys
/// with gaps smaller than `session_gap_ms`. Discards sessions with fewer than
/// `min_typing_keys` keystrokes. Cursor position is looked up from mouse events
/// via binary search at session start time.
fn detect_typing_sessions(
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

    println!("⌨️ [TYPING] Detecting sessions from {} typing keys", typing_keys.len());

    let mut sessions: Vec<TypingSession> = Vec::new();
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
                let (cx, cy) = lookup_cursor_position(session_start, mouse_events, metadata);
                sessions.push(TypingSession {
                    start_time: session_start,
                    end_time: session_end,
                    key_count,
                    cursor_x: cx,
                    cursor_y: cy,
                });
            }
            session_start = key.timestamp;
            session_end = key.timestamp;
            key_count = 1;
        }
    }

    // Finalize last session
    if key_count >= config.min_typing_keys {
        let (cx, cy) = lookup_cursor_position(session_start, mouse_events, metadata);
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
        println!("   Session {}: {}ms-{}ms, {} keys, cursor=({:.2}, {:.2})",
            i, s.start_time, s.end_time, s.key_count, s.cursor_x, s.cursor_y);
    }

    sessions
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

    // Normalize coordinates using capture_region or full display
    let (eff_x, eff_y, eff_w, eff_h) = if let Some((rx, ry, rw, rh)) = metadata.capture_region {
        (rx as f64, ry as f64, rw as f64, rh as f64)
    } else {
        (0.0, 0.0, metadata.display_resolution.0 as f64, metadata.display_resolution.1 as f64)
    };

    let norm_x = ((event.base.x - eff_x) / eff_w).clamp(0.0, 1.0);
    let norm_y = ((event.base.y - eff_y) / eff_h).clamp(0.0, 1.0);

    (norm_x, norm_y)
}

/// Create zoom blocks from detected typing sessions
fn create_typing_zoom_blocks(
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

/// Save zoom analysis to file
pub fn save_analysis(analysis: &ZoomAnalysis, path: &str) -> Result<()> {
    let json = serde_json::to_string_pretty(analysis)?;
    std::fs::write(path, json)?;
    println!("Zoom analysis saved to: {}", path);
    Ok(())
}

/// Load zoom analysis from file
pub fn load_analysis(path: &str) -> Result<ZoomAnalysis> {
    let json = std::fs::read_to_string(path)?;
    let analysis: ZoomAnalysis = serde_json::from_str(&json)?;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_capture::SessionMetadata;
    
    #[test]
    fn test_zoom_block_merging() {
        let config = ZoomConfig::default();
        let processor = ZoomProcessor::new(config);

        // Clicks at 1s and 2s should merge into one block (2s is within first block's active range)
        // Click at 2.2s is filtered by min_click_spacing (< 500ms from 2s)
        let session = create_test_session(vec![
            create_test_mouse_event(1000, 100.0, 200.0),  // First click
            create_test_mouse_event(2000, 300.0, 400.0),  // Merges into first block
            create_test_mouse_event(2200, 310.0, 410.0),  // Filtered (too close to 2000)
        ], 15000);

        let analysis = processor.analyze_session(&session, &[]).unwrap();

        // Should create 1 merged block with 2 centers
        assert_eq!(analysis.zoom_blocks.len(), 1);
        assert_eq!(analysis.total_clicks, 3);

        let block = &analysis.zoom_blocks[0];
        assert_eq!(block.centers.len(), 2); // Two click centers merged
        assert!(!block.is_manual);
    }

    #[test]
    fn test_zoom_block_separate_when_far_apart() {
        let config = ZoomConfig::default();
        let processor = ZoomProcessor::new(config);

        // Clicks 10s apart should create separate blocks
        let session = create_test_session(vec![
            create_test_mouse_event(1000, 100.0, 200.0),
            create_test_mouse_event(11000, 300.0, 400.0),
        ], 20000);

        let analysis = processor.analyze_session(&session, &[]).unwrap();
        assert_eq!(analysis.zoom_blocks.len(), 2);
    }

    #[test]
    fn test_typing_session_detection() {
        // Simulate typing: 10 keys over 2 seconds, then a 6s gap, then 5 more keys
        let key_events: Vec<KeyEvent> = vec![
            KeyEvent { timestamp: 1000, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 1200, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 1400, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 1600, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 1800, is_modifier: false, is_typing: true },
            // 6s gap — new session
            KeyEvent { timestamp: 8000, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 8200, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 8400, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 8600, is_modifier: false, is_typing: true },
        ];

        // Mouse at (500, 500) throughout
        let mouse_events = vec![
            create_test_enhanced_move_event(0, 500.0, 500.0),
        ];

        let metadata = crate::event_capture::SessionMetadata {
            display_id: "test".to_string(),
            display_resolution: (1000, 1000),
            scale_factor: 1.0,
            capture_region: None,
            has_microphone: false,
            has_system_audio: false,
            recording_fps: 60,
            recording_quality: 1.0,
        };

        let config = TypingZoomConfig::default();
        let sessions = detect_typing_sessions(&key_events, &mouse_events, &metadata, &config);
        assert_eq!(sessions.len(), 2, "Should detect 2 typing sessions");
        assert_eq!(sessions[0].key_count, 5);
        assert_eq!(sessions[1].key_count, 4);
    }

    #[test]
    fn test_typing_session_too_few_keys() {
        // Only 2 keys — below min_typing_keys threshold of 3
        let key_events: Vec<KeyEvent> = vec![
            KeyEvent { timestamp: 1000, is_modifier: false, is_typing: true },
            KeyEvent { timestamp: 1200, is_modifier: false, is_typing: true },
        ];

        let metadata = crate::event_capture::SessionMetadata {
            display_id: "test".to_string(),
            display_resolution: (1000, 1000),
            scale_factor: 1.0,
            capture_region: None,
            has_microphone: false,
            has_system_audio: false,
            recording_fps: 60,
            recording_quality: 1.0,
        };

        let config = TypingZoomConfig::default();
        let sessions = detect_typing_sessions(&key_events, &[], &metadata, &config);
        assert_eq!(sessions.len(), 0, "Should not detect session with fewer than 3 keys");
    }

    #[test]
    fn test_typing_zoom_blocks_created() {
        let config = ZoomConfig::default();
        let processor = ZoomProcessor::new(config);

        // No clicks, but typing at 2s
        let session = create_test_session(vec![], 30000);

        let key_events: Vec<KeyEvent> = (0..10)
            .map(|i| KeyEvent {
                timestamp: 2000 + i * 200,
                is_modifier: false,
                is_typing: true,
            })
            .collect();

        let analysis = processor.analyze_session(&session, &key_events).unwrap();
        assert_eq!(analysis.zoom_blocks.len(), 1, "Should create 1 typing zoom block");
        assert!(analysis.zoom_blocks[0].id.starts_with("typing_zoom_"));
    }

    #[test]
    fn test_modifier_keys_not_typing() {
        // Modifier keys should not count as typing
        let key_events: Vec<KeyEvent> = vec![
            KeyEvent { timestamp: 1000, is_modifier: true, is_typing: false },  // Cmd
            KeyEvent { timestamp: 1100, is_modifier: false, is_typing: false }, // Cmd+C (modified)
            KeyEvent { timestamp: 1200, is_modifier: true, is_typing: false },  // Cmd release
        ];

        let metadata = crate::event_capture::SessionMetadata {
            display_id: "test".to_string(),
            display_resolution: (1000, 1000),
            scale_factor: 1.0,
            capture_region: None,
            has_microphone: false,
            has_system_audio: false,
            recording_fps: 60,
            recording_quality: 1.0,
        };

        let config = TypingZoomConfig::default();
        let sessions = detect_typing_sessions(&key_events, &[], &metadata, &config);
        assert_eq!(sessions.len(), 0, "Modifier keys should not create typing sessions");
    }

    fn create_test_enhanced_move_event(timestamp: u64, x: f64, y: f64) -> EnhancedMouseEvent {
        EnhancedMouseEvent {
            base: crate::mouse_tracking::MouseEvent {
                timestamp,
                x,
                y,
                event_type: crate::mouse_tracking::MouseEventType::Move,
                display_id: None,
            },
            window_id: None,
            app_name: None,
            is_double_click: false,
            cluster_id: None,
        }
    }
    
    fn create_test_mouse_event(timestamp: u64, x: f64, y: f64) -> EnhancedMouseEvent {
        EnhancedMouseEvent {
            base: crate::mouse_tracking::MouseEvent {
                timestamp,
                x,
                y,
                event_type: crate::mouse_tracking::MouseEventType::ButtonPress { 
                    button: crate::mouse_tracking::MouseButton::Left 
                },
                display_id: None,
            },
            window_id: None,
            app_name: None,
            is_double_click: false,
            cluster_id: None,
        }
    }
    
    fn create_test_session(mouse_events: Vec<EnhancedMouseEvent>, duration_ms: u64) -> CaptureSession {
        CaptureSession {
            session_id: "test_session".to_string(),
            start_time: 0,
            end_time: Some(duration_ms),
            mouse_events,
            keyboard_events: vec![],
            window_events: vec![],
            audio_events: vec![],
            metadata: SessionMetadata {
                display_id: "test_display".to_string(),
                display_resolution: (1000, 1000), // Test resolution
                scale_factor: 1.0,
                capture_region: None,
                has_microphone: false,
                has_system_audio: false,
                recording_fps: 60,
                recording_quality: 1.0,
            },
        }
    }
}