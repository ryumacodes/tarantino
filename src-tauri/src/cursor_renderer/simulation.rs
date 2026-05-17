use super::*;
use crate::video_processing::zoom_trajectory::ZoomFrameState;

pub fn parse_cursor_events(
    events: &[serde_json::Value],
    _scale_factor: f64,
    effective_x: f64,
    effective_y: f64,
    effective_width: f64,
    effective_height: f64,
) -> Vec<CursorEvent> {
    let mut parsed = events
        .iter()
        .filter_map(|event| {
            let base = event.get("base").unwrap_or(event);

            let timestamp = base.get("timestamp").and_then(|t| t.as_u64())?;
            let raw_x = base.get("x").and_then(|v| v.as_f64())?;
            let raw_y = base.get("y").and_then(|v| v.as_f64())?;

            // Check if this is a click event
            let is_click = if let Some(event_type) = base.get("event_type") {
                event_type.as_str() == Some("ButtonPress")
                    || event_type.get("ButtonPress").is_some()
            } else {
                false
            };

            // Apply coordinate normalization (same as build_cursor_trajectory)
            // Note: Coordinates from rdev (CGEvent) are already in logical pixels (points),
            // not physical pixels. No scale_factor division needed.
            let logical_x = raw_x;
            let logical_y = raw_y;
            let adjusted_x = logical_x - effective_x;
            let adjusted_y = logical_y - effective_y;
            let norm_x = (adjusted_x / effective_width).clamp(0.0, 1.0);
            let norm_y = (adjusted_y / effective_height).clamp(0.0, 1.0);

            Some(CursorEvent {
                timestamp_ms: timestamp,
                x: norm_x,
                y: norm_y,
                is_click,
            })
        })
        .collect::<Vec<_>>();

    parsed.sort_by_key(|event| event.timestamp_ms);
    parsed
}

/// Trail point data for export (matches preview trail rendering)
#[derive(Clone, Copy, Debug, Default)]
pub struct TrailPoint {
    pub x: f32,
    pub y: f32,
    pub alpha: f32,
    pub size: f32,
}

/// Per-frame cursor state for GPU rendering (position + effect state, no image data).
#[derive(Clone, Debug)]
pub struct CursorFrameState {
    pub x: f32,
    pub y: f32,
    pub opacity: f32,         // idle fade (0-1)
    pub rotation: f32,        // degrees
    pub is_clicking: f32,     // 0 or 1
    pub ripple_progress: f32, // 0 = no ripple, 0.0-1.0 = active
    pub ripple_x: f32,
    pub ripple_y: f32,
    pub circle_hl_progress: f32, // 0=inactive, 0-1=active
    pub circle_hl_x: f32,
    pub circle_hl_y: f32,
    pub trail_count: f32,
    pub trail_points: [TrailPoint; 30],
}

/// Simulate cursor positions with spring physics for GPU rendering.
///
/// Returns per-frame cursor state without rendering any images.
/// Cursor coordinates are in zoomed output space when zoom_trajectory is provided.
/// Now includes idle fade, rotation, circle highlight, click state, and trail.
pub fn simulate_cursor_positions(
    events: &[CursorEvent],
    spring_config: &SpringConfig,
    fps: f64,
    duration_ms: u64,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>,
    cursor_settings: &CursorSettings,
) -> Vec<CursorFrameState> {
    let total_frames = ((duration_ms as f64 * fps) / 1000.0).ceil() as u64;
    let dt = 1.0 / fps;
    let mut positions = Vec::with_capacity(total_frames as usize);

    let init_x = events.first().map_or(0.5, |e| e.x);
    let init_y = events.first().map_or(0.5, |e| e.y);
    let mut spring_x = SpringState {
        value: init_x,
        velocity: 0.0,
    };
    let mut spring_y = SpringState {
        value: init_y,
        velocity: 0.0,
    };

    // Settings
    let hide_when_idle = cursor_settings.hide_when_idle.unwrap_or(false);
    let idle_timeout_ms = cursor_settings.idle_timeout.unwrap_or(2000) as f64;
    let static_rotation = cursor_settings.rotation.unwrap_or(0.0);
    let rotate_while_moving = cursor_settings.rotate_while_moving.unwrap_or(false);
    let rotation_intensity = cursor_settings.rotation_intensity.unwrap_or(30.0);
    let trail_enabled = cursor_settings.trail_enabled.unwrap_or(false);
    let trail_length = cursor_settings.trail_length.unwrap_or(15) as usize;
    let trail_opacity = cursor_settings.trail_opacity.unwrap_or(0.5);
    let click_effect = cursor_settings.click_effect.as_deref().unwrap_or("ripple");
    let cursor_scale = cursor_settings.size.unwrap_or(1.0);
    let stop_at_end = cursor_settings.stop_at_end.unwrap_or(false);
    let stop_duration_ms = cursor_settings
        .stop_duration_ms
        .unwrap_or(0)
        .min(duration_ms);
    let loop_to_start = cursor_settings.loop_to_start.unwrap_or(false);
    let loop_duration_ms = cursor_settings
        .loop_duration_ms
        .unwrap_or(500)
        .min(duration_ms);
    let freeze_time_ms = if stop_at_end && stop_duration_ms > 0 {
        duration_ms.saturating_sub(stop_duration_ms)
    } else {
        duration_ms
    };
    let loop_start_ms = if loop_to_start && loop_duration_ms > 0 {
        duration_ms.saturating_sub(loop_duration_ms)
    } else {
        duration_ms
    };
    let loop_from = find_target_at_time(events, loop_start_ms.min(freeze_time_ms));

    // Track click events
    let click_events: Vec<(u64, f64, f64)> = events
        .iter()
        .filter(|e| e.is_click)
        .map(|e| (e.timestamp_ms, e.x, e.y))
        .collect();

    let mut last_click_time: Option<u64> = None;
    let mut active_ripple: Option<(f64, f64, u64)> = None;
    let mut active_circle_hl: Option<(f64, f64, u64)> = None;
    let ripple_duration_frames = (0.6 * fps) as u64;
    let circle_hl_duration_frames = (0.3 * fps) as u64;

    // Idle fade state
    let mut opacity: f64 = 1.0;
    let mut last_move_time_ms: f64 = 0.0;

    // Rotation state
    let mut horizontal_velocity: f64 = 0.0;
    let mut last_x_position: f64 = init_x;

    // Trail history (pre-zoom positions for trail rendering)
    let mut trail_history: Vec<(f64, f64)> = Vec::new();

    // Track last target for move detection
    let mut last_target_x: f64 = init_x;
    let mut last_target_y: f64 = init_y;

    for frame_num in 0..total_frames {
        let time_ms = ((frame_num as f64 * 1000.0) / fps) as u64;
        let time_ms_f = frame_num as f64 * 1000.0 / fps;

        // Find target cursor position
        let effective_time_ms = time_ms.min(freeze_time_ms);
        let (mut target_x, mut target_y) = find_target_at_time(events, effective_time_ms);
        if loop_to_start && time_ms >= loop_start_ms && duration_ms > loop_start_ms {
            let denom = (duration_ms - loop_start_ms).max(1) as f64;
            let progress = smoothstep(((time_ms - loop_start_ms) as f64 / denom).clamp(0.0, 1.0));
            target_x = lerp(loop_from.0, init_x, progress);
            target_y = lerp(loop_from.1, init_y, progress);
        }

        // Detect if cursor moved (for idle tracking)
        let target_moved =
            (target_x - last_target_x).abs() > 0.0001 || (target_y - last_target_y).abs() > 0.0001;
        if target_moved {
            last_move_time_ms = time_ms_f;
        }
        last_target_x = target_x;
        last_target_y = target_y;

        // Apply spring physics
        spring_x = spring_step(
            spring_x.value,
            target_x,
            spring_x.velocity,
            spring_config,
            dt,
        );
        spring_y = spring_step(
            spring_y.value,
            target_y,
            spring_y.velocity,
            spring_config,
            dt,
        );

        let mut cursor_x = spring_x.value;
        let mut cursor_y = spring_y.value;

        // Idle fade
        if hide_when_idle {
            let time_since_move = time_ms_f - last_move_time_ms;
            if time_since_move > idle_timeout_ms {
                opacity = (opacity - dt * 2.0).max(0.0);
            } else if time_since_move < 100.0 {
                opacity = (opacity + dt * 5.0).min(1.0);
            }
        } else {
            opacity = 1.0;
        }

        // Rotation
        let dx = cursor_x - last_x_position;
        last_x_position = cursor_x;
        horizontal_velocity = horizontal_velocity * 0.85 + dx * 0.15;

        let mut rotation = static_rotation;
        if rotate_while_moving {
            let max_rot = 30.0 * (rotation_intensity / 100.0);
            rotation += (horizontal_velocity * 2.0).clamp(-max_rot, max_rot);
        }

        // Transform through zoom if active
        if let Some(ref trajectory) = zoom_trajectory {
            if !trajectory.is_empty() {
                let tidx = (frame_num as usize).min(trajectory.len() - 1);
                let zs = &trajectory[tidx];
                if zs.scale > 1.0 {
                    cursor_x = (cursor_x - zs.center_x) * zs.scale + 0.5;
                    cursor_y = (cursor_y - zs.center_y) * zs.scale + 0.5;
                }
            }
        }

        // Update trail (post-zoom positions)
        if trail_enabled {
            trail_history.push((cursor_x, cursor_y));
            while trail_history.len() > trail_length {
                trail_history.remove(0);
            }
        }

        // Check for click
        let allow_click_effects = (!stop_at_end || time_ms <= freeze_time_ms)
            && (!loop_to_start || time_ms < loop_start_ms);
        let click_at_time = if allow_click_effects {
            click_events
                .iter()
                .find(|(t, _, _)| time_ms >= *t && time_ms < *t + 100 && *t <= freeze_time_ms)
        } else {
            None
        };
        let is_clicking = click_at_time.is_some();

        if let Some((click_time, exact_x, exact_y)) = click_at_time {
            if last_click_time.map_or(true, |t| *click_time > t + 100) {
                // Transform effect position through zoom
                let mut rx = *exact_x;
                let mut ry = *exact_y;
                if let Some(ref trajectory) = zoom_trajectory {
                    if !trajectory.is_empty() {
                        let tidx = (frame_num as usize).min(trajectory.len() - 1);
                        let zs = &trajectory[tidx];
                        if zs.scale > 1.0 {
                            rx = (rx - zs.center_x) * zs.scale + 0.5;
                            ry = (ry - zs.center_y) * zs.scale + 0.5;
                        }
                    }
                }
                if click_effect == "ripple" {
                    active_ripple = Some((rx, ry, frame_num));
                }
                if click_effect == "circle" {
                    active_circle_hl = Some((rx, ry, frame_num));
                }
                last_click_time = Some(*click_time);
            }
        }

        // Compute ripple progress
        let (ripple_progress, ripple_x, ripple_y) = if let Some((rx, ry, start)) = active_ripple {
            let elapsed = frame_num - start;
            if elapsed < ripple_duration_frames {
                (
                    elapsed as f32 / ripple_duration_frames as f32,
                    rx as f32,
                    ry as f32,
                )
            } else {
                active_ripple = None;
                (0.0, 0.0, 0.0)
            }
        } else {
            (0.0, 0.0, 0.0)
        };

        // Compute circle highlight progress
        let (circle_hl_progress, circle_hl_x, circle_hl_y) =
            if let Some((cx, cy, start)) = active_circle_hl {
                let elapsed = frame_num - start;
                if elapsed < circle_hl_duration_frames {
                    (
                        elapsed as f32 / circle_hl_duration_frames as f32,
                        cx as f32,
                        cy as f32,
                    )
                } else {
                    active_circle_hl = None;
                    (0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0)
            };

        // Build trail points
        let mut trail_points = [TrailPoint::default(); 30];
        let trail_count = if trail_enabled {
            trail_history.len()
        } else {
            0
        };
        for (i, &(tx, ty)) in trail_history.iter().enumerate() {
            if i >= 30 {
                break;
            }
            let progress = if trail_count > 0 {
                i as f32 / trail_count as f32
            } else {
                0.0
            };
            trail_points[i] = TrailPoint {
                x: tx as f32,
                y: ty as f32,
                alpha: progress * trail_opacity as f32 * opacity as f32,
                size: (2.0 + progress * 4.0) * cursor_scale as f32,
            };
        }

        positions.push(CursorFrameState {
            x: cursor_x as f32,
            y: cursor_y as f32,
            opacity: opacity as f32,
            rotation: rotation as f32,
            is_clicking: if is_clicking { 1.0 } else { 0.0 },
            ripple_progress,
            ripple_x,
            ripple_y,
            circle_hl_progress,
            circle_hl_x,
            circle_hl_y,
            trail_count: trail_count as f32,
            trail_points,
        });
    }

    positions
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn smoothstep(value: f64) -> f64 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn find_target_at_time(events: &[CursorEvent], time_ms: u64) -> (f64, f64) {
    if events.is_empty() {
        return (0.5, 0.5);
    }

    let next_idx = events.partition_point(|event| event.timestamp_ms <= time_ms);
    if next_idx == 0 {
        return (events[0].x, events[0].y);
    }
    if next_idx >= events.len() {
        let event = &events[events.len() - 1];
        return (event.x, event.y);
    }

    let previous = &events[next_idx - 1];
    let next = &events[next_idx];
    let span = next.timestamp_ms.saturating_sub(previous.timestamp_ms);
    if span == 0 {
        return (previous.x, previous.y);
    }

    let t = (time_ms.saturating_sub(previous.timestamp_ms) as f64 / span as f64).clamp(0.0, 1.0);
    (lerp(previous.x, next.x, t), lerp(previous.y, next.y, t))
}

/// Pre-render the cursor pointer shape as a small image for GPU texture upload.
/// Note: SDF rendering in gpu_compositor.rs replaces this for export. Kept for potential other uses.
#[allow(dead_code)]
pub fn render_cursor_shape(config: &CursorSettings) -> RgbaImage {
    let scale = config.size.unwrap_or(1.0) as f32;
    let w = (14.0 * scale).ceil() as u32 + 2;
    let h = (20.0 * scale).ceil() as u32 + 2;
    let mut img = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 0]));

    let (r, g, b) = parse_hex_rgb(config.color.as_deref().unwrap_or("#ffffff"));

    // Pointer cursor polygon (tip at 1,1 to avoid edge clipping)
    let x = 1i32;
    let y = 1i32;
    let points = [
        Point::new(x, y),
        Point::new(x, y + (16.0 * scale) as i32),
        Point::new(x + (4.0 * scale) as i32, y + (12.0 * scale) as i32),
        Point::new(x + (7.0 * scale) as i32, y + (18.0 * scale) as i32),
        Point::new(x + (9.5 * scale) as i32, y + (17.0 * scale) as i32),
        Point::new(x + (6.5 * scale) as i32, y + (11.0 * scale) as i32),
        Point::new(x + (12.0 * scale) as i32, y + (11.0 * scale) as i32),
    ];

    // Filled pointer
    draw_polygon_mut(&mut img, &points, Rgba([r, g, b, 255]));
    // Black outline
    for i in 0..points.len() {
        let p1 = points[i];
        let p2 = points[(i + 1) % points.len()];
        draw_antialiased_line_segment_mut(
            &mut img,
            (p1.x, p1.y),
            (p2.x, p2.y),
            Rgba([0, 0, 0, 200]),
            |bg, fg, t| {
                let blend = |b: u8, f: u8| ((1.0 - t) * b as f32 + t * f as f32) as u8;
                Rgba([
                    blend(bg[0], fg[0]),
                    blend(bg[1], fg[1]),
                    blend(bg[2], fg[2]),
                    blend(bg[3], fg[3]),
                ])
            },
        );
    }

    img
}
