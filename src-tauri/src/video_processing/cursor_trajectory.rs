//! Cursor trajectory building and FFmpeg filter generation

use std::path::Path;
use anyhow::{Result, anyhow};
use super::types::{CursorFrame, ZoomBlock};

/// Build cursor trajectory from mouse events sidecar file
pub fn build_cursor_trajectory(
    mouse_events_path: &Path,
    fps: f64,
    duration_ms: u64,
    video_width: u32,
    video_height: u32,
) -> Result<Vec<CursorFrame>> {
    let content = std::fs::read_to_string(mouse_events_path)?;
    let sidecar: serde_json::Value = serde_json::from_str(&content)?;

    // Handle both formats: new format with display dimensions, or legacy raw array
    // Also extract scale_factor and recording_area for proper coordinate normalization
    let (events, scale_factor, effective_x, effective_y, effective_width, effective_height):
        (Vec<serde_json::Value>, f64, f64, f64, f64, f64) =
        if let Some(mouse_events) = sidecar.get("mouse_events") {
            // New format: { display_width, display_height, scale_factor, recording_area, mouse_events }
            let dw = sidecar.get("display_width")
                .and_then(|v| v.as_f64())
                .unwrap_or(video_width as f64);
            let dh = sidecar.get("display_height")
                .and_then(|v| v.as_f64())
                .unwrap_or(video_height as f64);

            // Scale factor for Retina displays (physical pixels / logical pixels)
            // On Retina Macs this is typically 2.0
            let sf = sidecar.get("scale_factor")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);

            // Recording area for partial screen captures
            let recording_area = sidecar.get("recording_area");
            let (eff_x, eff_y, eff_w, eff_h) = if let Some(area) = recording_area {
                (
                    area.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    area.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    area.get("width").and_then(|v| v.as_f64()).unwrap_or(dw),
                    area.get("height").and_then(|v| v.as_f64()).unwrap_or(dh),
                )
            } else {
                (0.0, 0.0, dw, dh)
            };

            let events = mouse_events.as_array()
                .cloned()
                .unwrap_or_default();
            println!("Loaded sidecar: display={}x{}, scale_factor={}, recording_area=({},{} {}x{})",
                     dw, dh, sf, eff_x, eff_y, eff_w, eff_h);
            (events, sf, eff_x, eff_y, eff_w, eff_h)
        } else if let Some(arr) = sidecar.as_array() {
            // Legacy format: raw array of events (no display info)
            println!("Loaded legacy sidecar format (no display dimensions)");
            (arr.clone(), 1.0, 0.0, 0.0, video_width as f64, video_height as f64)
        } else {
            return Err(anyhow!("Invalid sidecar format"));
        };

    if events.is_empty() {
        return Ok(Vec::new());
    }

    let total_frames = ((duration_ms as f64 * fps) / 1000.0) as u64;
    let mut trajectory = Vec::with_capacity(total_frames as usize);

    for frame in 0..total_frames {
        let time_ms = ((frame as f64 * 1000.0) / fps) as u64;

        // Find closest event to this timestamp - apply proper coordinate normalization
        let (x, y, is_click) = interpolate_cursor_at_time(
            &events, time_ms,
            scale_factor, effective_x, effective_y, effective_width, effective_height
        );

        trajectory.push(CursorFrame {
            frame,
            x,
            y,
            is_click,
        });
    }

    println!("Built cursor trajectory: {} frames from {} events",
             trajectory.len(), events.len());
    Ok(trajectory)
}

/// Interpolate cursor position at a specific timestamp from mouse events
/// Applies proper coordinate normalization matching MouseCursorOverlay.tsx:
/// 1. Divide by scale_factor (Retina: physical → logical pixels)
/// 2. Subtract recording area offset (for partial screen captures)
/// 3. Divide by recording area dimensions to get 0-1 range
fn interpolate_cursor_at_time(
    events: &[serde_json::Value],
    time_ms: u64,
    _scale_factor: f64,
    effective_x: f64,
    effective_y: f64,
    effective_width: f64,
    effective_height: f64,
) -> (f64, f64, bool) {
    if events.is_empty() {
        return (0.5, 0.5, false);
    }

    // Find the event closest to or just before this timestamp
    let mut closest_event: Option<&serde_json::Value> = None;
    let mut closest_diff = u64::MAX;
    let mut is_click = false;

    for event in events {
        // Handle both formats: direct or wrapped in "base"
        let base = event.get("base").unwrap_or(event);
        let event_time = base.get("timestamp")
            .and_then(|t| t.as_u64())
            .unwrap_or(0);

        let diff = if event_time > time_ms {
            event_time - time_ms
        } else {
            time_ms - event_time
        };

        if diff < closest_diff {
            closest_diff = diff;
            closest_event = Some(event);
        }

        // Check for click events within 100ms of target time
        if diff < 100 {
            if let Some(event_type) = base.get("event_type") {
                // Handle both formats: string "ButtonPress" or object { "ButtonPress": {...} }
                let is_button_press = event_type.as_str() == Some("ButtonPress")
                    || event_type.get("ButtonPress").is_some();
                if is_button_press {
                    is_click = true;
                }
            }
        }
    }

    if let Some(event) = closest_event {
        let base = event.get("base").unwrap_or(event);
        let raw_x = base.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let raw_y = base.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

        // Apply coordinate normalization matching MouseCursorOverlay.tsx:
        // Note: Coordinates from rdev (CGEvent) are already in logical pixels (points),
        // not physical pixels. No scale_factor division needed.
        let logical_x = raw_x;
        let logical_y = raw_y;

        // Subtract recording area offset (for partial screen captures)
        let adjusted_x = logical_x - effective_x;
        let adjusted_y = logical_y - effective_y;

        // 3. Normalize by recording area dimensions to get 0-1 range
        let norm_x = (adjusted_x / effective_width).clamp(0.0, 1.0);
        let norm_y = (adjusted_y / effective_height).clamp(0.0, 1.0);

        (norm_x, norm_y, is_click)
    } else {
        (0.5, 0.5, false)
    }
}

/// Build FFmpeg filter for cursor rendering using drawbox
/// Groups frames into segments for efficiency
pub fn build_cursor_filter(
    trajectory: &[CursorFrame],
    video_width: u32,
    video_height: u32,
    cursor_size: u32,
    click_highlight_size: u32,
) -> String {
    if trajectory.is_empty() {
        return String::new();
    }

    let mut filters = Vec::new();

    // Group consecutive frames into segments (every 15 frames = 0.25s at 60fps)
    let segment_size = 15;
    for chunk in trajectory.chunks(segment_size) {
        if chunk.is_empty() {
            continue;
        }

        let start_frame = chunk.first().unwrap().frame;
        let end_frame = chunk.last().unwrap().frame;

        // Average position for this segment
        let avg_x: f64 = chunk.iter().map(|c| c.x).sum::<f64>() / chunk.len() as f64;
        let avg_y: f64 = chunk.iter().map(|c| c.y).sum::<f64>() / chunk.len() as f64;

        let px_x = (avg_x * video_width as f64) as i32;
        let px_y = (avg_y * video_height as f64) as i32;

        // Main cursor (white circle with black outline for visibility)
        let half_size = cursor_size as i32 / 2;
        filters.push(format!(
            "drawbox=x={}:y={}:w={}:h={}:c=white@0.9:t=fill:enable='between(n,{},{})'",
            (px_x - half_size).max(0),
            (px_y - half_size).max(0),
            cursor_size,
            cursor_size,
            start_frame,
            end_frame
        ));

        // Check if any frame in this segment has a click
        let has_click = chunk.iter().any(|c| c.is_click);
        if has_click {
            // Click highlight (larger yellow/orange circle)
            let hl_half = click_highlight_size as i32 / 2;
            filters.push(format!(
                "drawbox=x={}:y={}:w={}:h={}:c=yellow@0.5:t=3:enable='between(n,{},{})'",
                (px_x - hl_half).max(0),
                (px_y - hl_half).max(0),
                click_highlight_size,
                click_highlight_size,
                start_frame,
                end_frame
            ));
        }
    }

    if filters.is_empty() {
        return String::new();
    }

    filters.join(",")
}

/// Build cursor-following pan X expression for dynamic panning during zoom hold
pub fn build_cursor_following_pan_x(
    zoom_blocks: &[ZoomBlock],
    trajectory: &[CursorFrame],
    fps: f64,
) -> String {
    if zoom_blocks.is_empty() {
        return "iw/2-(iw/zoom/2)".to_string();
    }

    // Use flat additive structure to avoid deep nesting
    let transition_frames = (fps * 0.8).max(1.0) as u64;
    let mut expr = "iw/2-iw/zoom/2".to_string();

    for block in zoom_blocks.iter() {
        let start_sec = block.start_time_ms as f64 / 1000.0;
        let end_sec = block.end_time_ms as f64 / 1000.0;
        let start_frame = (start_sec * fps) as u64;
        let end_frame = (end_sec * fps) as u64;
        let hold_start = start_frame + transition_frames;
        let hold_end = end_frame.saturating_sub(transition_frames);
        let trans = transition_frames.max(1);

        // During hold phase, use average cursor position from trajectory
        let hold_cursor_positions: Vec<f64> = trajectory.iter()
            .filter(|c| c.frame >= hold_start && c.frame <= hold_end)
            .map(|c| c.x)
            .collect();

        let avg_cursor_x = if hold_cursor_positions.is_empty() {
            block.center_x
        } else {
            hold_cursor_positions.iter().sum::<f64>() / hold_cursor_positions.len() as f64
        };

        let delta_x = avg_cursor_x - 0.5;
        // Add contribution: delta * curve * mask * iw
        expr = format!(
            "{}+{}*min(min((on-{})/{}\\,1)\\,min(({}-on)/{}\\,1))*iw*between(on\\,{}\\,{})",
            expr, delta_x, start_frame, trans, end_frame, trans, start_frame, end_frame
        );
    }

    expr
}

/// Build cursor-following pan Y expression for dynamic panning during zoom hold
pub fn build_cursor_following_pan_y(
    zoom_blocks: &[ZoomBlock],
    trajectory: &[CursorFrame],
    fps: f64,
) -> String {
    if zoom_blocks.is_empty() {
        return "ih/2-(ih/zoom/2)".to_string();
    }

    // Use flat additive structure to avoid deep nesting
    let transition_frames = (fps * 0.8).max(1.0) as u64;
    let mut expr = "ih/2-ih/zoom/2".to_string();

    for block in zoom_blocks.iter() {
        let start_sec = block.start_time_ms as f64 / 1000.0;
        let end_sec = block.end_time_ms as f64 / 1000.0;
        let start_frame = (start_sec * fps) as u64;
        let end_frame = (end_sec * fps) as u64;
        let hold_start = start_frame + transition_frames;
        let hold_end = end_frame.saturating_sub(transition_frames);
        let trans = transition_frames.max(1);

        // During hold phase, use average cursor position from trajectory
        let hold_cursor_positions: Vec<f64> = trajectory.iter()
            .filter(|c| c.frame >= hold_start && c.frame <= hold_end)
            .map(|c| c.y)
            .collect();

        let avg_cursor_y = if hold_cursor_positions.is_empty() {
            block.center_y
        } else {
            hold_cursor_positions.iter().sum::<f64>() / hold_cursor_positions.len() as f64
        };

        let delta_y = avg_cursor_y - 0.5;
        // Add contribution: delta * curve * mask * ih
        expr = format!(
            "{}+{}*min(min((on-{})/{}\\,1)\\,min(({}-on)/{}\\,1))*ih*between(on\\,{}\\,{})",
            expr, delta_y, start_frame, trans, end_frame, trans, start_frame, end_frame
        );
    }

    expr
}
