//! Zoom trajectory simulation using spring physics.
//!
//! Replicates VideoViewer.tsx from the preview pipeline exactly,
//! producing per-frame zoom level and center coordinates using the
//! same spring physics, hard phase switching, and edge clamping.
//!
//! This is the single source of truth for zoom animation in export,
//! replacing the old FFmpeg zoompan smoothstep approximation.

use crate::cursor_renderer::{spring_step, CursorEvent, SpringConfig, SpringState};
use crate::video_processing::types::ZoomBlock;

/// Per-frame zoom state produced by the spring simulation
#[derive(Clone, Debug)]
pub struct ZoomFrameState {
    pub scale: f64,
    pub center_x: f64,
    pub center_y: f64,
}

/// Zoom pan spring config — matches VideoViewer.tsx zoomPanConfig.
/// Used for center x/y spring during active zoom blocks.
const ZOOM_PAN_CONFIG: SpringConfig = SpringConfig {
    tension: 80.0,
    friction: 40.0,
    mass: 2.0,
};

const FOLLOW_PAN_CONFIG: SpringConfig = SpringConfig {
    tension: 520.0,
    friction: 52.0,
    mass: 1.0,
};

/// Resolve a per-block spring preset name to a SpringConfig, falling back to the global config.
fn resolve_block_spring(preset: &Option<String>, fallback: &SpringConfig) -> SpringConfig {
    match preset.as_deref() {
        Some("slow") => SpringConfig {
            tension: 120.0,
            friction: 28.0,
            mass: 1.0,
        },
        Some("mellow") => SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        },
        Some("quick") => SpringConfig {
            tension: 280.0,
            friction: 38.0,
            mass: 1.0,
        },
        Some("rapid") => SpringConfig {
            tension: 400.0,
            friction: 44.0,
            mass: 1.0,
        },
        _ => *fallback,
    }
}

/// Simulate the full zoom trajectory using spring physics.
///
/// Replicates VideoViewer.tsx frame-by-frame:
/// - Blocks resolve to continuous camera targets
/// - Per-block zoom_in_speed / zoom_out_speed override global zoom_spring_config
/// - Separate spring for zoom scale and center (uses ZOOM_PAN_CONFIG)
/// - Edge clamping on both target and animated values
///
/// `cursor_events` are RAW mouse events (not spring-smoothed) used for cursor-following.
/// `zoom_spring_config` comes from SPRING_PRESETS[zoomSpeedPreset] via the frontend (global fallback).
/// `cursor_spring_config` comes from SPRING_PRESETS[cursorSpeedPreset] — used for pan when NOT zooming.
pub fn simulate_zoom_trajectory(
    zoom_blocks: &[ZoomBlock],
    cursor_events: &[CursorEvent],
    zoom_spring_config: &SpringConfig,
    cursor_spring_config: &SpringConfig,
    fps: f64,
    duration_ms: u64,
    window_mode: bool,
) -> Vec<ZoomFrameState> {
    let total_frames = ((duration_ms as f64 * fps) / 1000.0).ceil() as u64;
    let dt = 1.0 / fps;

    // Spring state — starts at no-zoom, centered
    let mut zoom_spring = SpringState {
        value: 1.0,
        velocity: 0.0,
    };
    let mut center_spring_x = SpringState {
        value: 0.5,
        velocity: 0.0,
    };
    let mut center_spring_y = SpringState {
        value: 0.5,
        velocity: 0.0,
    };

    let mut trajectory = Vec::with_capacity(total_frames as usize);
    let mut prev_block_idx: Option<usize> = None;
    let mut last_block_out_config: Option<SpringConfig> = None;

    for frame_num in 0..total_frames {
        let time_ms = (frame_num as f64 * 1000.0) / fps;

        let mut target_scale = 1.0;
        let mut target_center_x = 0.5;
        let mut target_center_y = 0.5;
        let mut is_zooming = false;
        let mut is_follow_phase = false;
        // Default: use last block's out config for zoom-out after block ends, else global
        let mut active_zoom_config = last_block_out_config.unwrap_or(*zoom_spring_config);

        // Find active zoom block (matching VideoViewer.tsx)
        if let Some((block_idx, block)) = zoom_blocks
            .iter()
            .enumerate()
            .find(|(_, b)| time_ms >= b.start_time_ms as f64 && time_ms <= b.end_time_ms as f64)
        {
            is_zooming = true;
            target_scale = block.zoom_level;

            // Resolve per-block spring configs
            let block_in_config = resolve_block_spring(&block.zoom_in_speed, zoom_spring_config);
            let block_out_config = resolve_block_spring(&block.zoom_out_speed, zoom_spring_config);

            // Snap center springs only when entering zoom from unzoomed state
            if prev_block_idx != Some(block_idx) {
                prev_block_idx = Some(block_idx);
                let already_zoomed = zoom_spring.value > 1.1;
                if !already_zoomed {
                    center_spring_x = SpringState {
                        value: block.center_x,
                        velocity: 0.0,
                    };
                    center_spring_y = SpringState {
                        value: block.center_y,
                        velocity: 0.0,
                    };
                }
            }

            active_zoom_config = block_in_config;
            let cursor_position = find_cursor_at_time(cursor_events, time_ms as u64);
            let first_center_time = block
                .centers
                .iter()
                .map(|center| center.time)
                .min()
                .unwrap_or(block.start_time_ms);
            let mut anchor_center_x = block.center_x;
            let mut anchor_center_y = block.center_y;
            for center in block.centers.iter() {
                if time_ms >= center.time as f64 {
                    anchor_center_x = center.x;
                    anchor_center_y = center.y;
                }
            }

            if time_ms >= first_center_time as f64 {
                if let Some((cursor_x, cursor_y)) = cursor_position {
                    is_follow_phase = true;
                    target_center_x = cursor_x;
                    target_center_y = cursor_y;
                } else {
                    target_center_x = anchor_center_x;
                    target_center_y = anchor_center_y;
                }
            } else {
                target_center_x = anchor_center_x;
                target_center_y = anchor_center_y;
            }

            last_block_out_config = Some(block_out_config);
        }

        if !is_zooming {
            target_center_x = 0.5;
            target_center_y = 0.5;
            prev_block_idx = None;
        }

        // Edge clamping on target — uses current (pre-step) spring scale
        let current_scale = zoom_spring.value;
        if !window_mode && current_scale > 1.0 {
            let half_visible = 0.5 / current_scale;
            target_center_x = target_center_x.clamp(half_visible, 1.0 - half_visible);
            target_center_y = target_center_y.clamp(half_visible, 1.0 - half_visible);
        }
        target_center_x = target_center_x.clamp(0.0, 1.0);
        target_center_y = target_center_y.clamp(0.0, 1.0);

        let pan_config = if is_follow_phase {
            &FOLLOW_PAN_CONFIG
        } else if is_zooming {
            &ZOOM_PAN_CONFIG
        } else {
            cursor_spring_config
        };

        center_spring_x = spring_step(
            center_spring_x.value,
            target_center_x,
            center_spring_x.velocity,
            pan_config,
            dt,
        );
        center_spring_y = spring_step(
            center_spring_y.value,
            target_center_y,
            center_spring_y.velocity,
            pan_config,
            dt,
        );
        zoom_spring = spring_step(
            zoom_spring.value,
            target_scale,
            zoom_spring.velocity,
            &active_zoom_config,
            dt,
        );

        let mut animated_cx = center_spring_x.value;
        let mut animated_cy = center_spring_y.value;
        let animated_scale = zoom_spring.value;

        // Edge clamp animated values (spring may overshoot)
        if !window_mode && animated_scale > 1.0 {
            let half_visible = 0.5 / animated_scale;
            animated_cx = animated_cx.clamp(half_visible, 1.0 - half_visible);
            animated_cy = animated_cy.clamp(half_visible, 1.0 - half_visible);
        } else if window_mode {
            animated_cx = animated_cx.clamp(0.0, 1.0);
            animated_cy = animated_cy.clamp(0.0, 1.0);
        }

        trajectory.push(ZoomFrameState {
            scale: animated_scale,
            center_x: animated_cx,
            center_y: animated_cy,
        });
    }

    trajectory
}

/// Find the interpolated cursor position at a given time (for cursor-following during zoom).
fn find_cursor_at_time(events: &[CursorEvent], time_ms: u64) -> Option<(f64, f64)> {
    if events.is_empty() {
        return None;
    }

    let next_idx = events.partition_point(|event| event.timestamp_ms <= time_ms);
    if next_idx == 0 {
        let event = &events[0];
        return Some((event.x, event.y));
    }
    if next_idx >= events.len() {
        let event = &events[events.len() - 1];
        return Some((event.x, event.y));
    }

    let previous = &events[next_idx - 1];
    let next = &events[next_idx];
    let span = next.timestamp_ms.saturating_sub(previous.timestamp_ms);
    if span == 0 {
        return Some((previous.x, previous.y));
    }

    let t = (time_ms.saturating_sub(previous.timestamp_ms) as f64 / span as f64).clamp(0.0, 1.0);
    Some((
        previous.x + (next.x - previous.x) * t,
        previous.y + (next.y - previous.y) * t,
    ))
}

/// Apply zoom/pan transform to a raw RGBA frame buffer using bilinear interpolation.
///
/// Crops the visible region (determined by scale + center) and scales back to
/// full frame dimensions. Uses a reusable temp buffer to avoid per-frame allocation.
///
/// No-op if scale <= 1.001 (no visible zoom).
pub fn apply_zoom_to_frame(
    frame_buffer: &mut [u8],
    temp_buffer: &mut Vec<u8>,
    width: u32,
    height: u32,
    zoom_state: &ZoomFrameState,
) {
    if zoom_state.scale <= 1.001 {
        return; // No zoom, skip
    }

    let w = width as f64;
    let h = height as f64;
    let scale = zoom_state.scale;
    let cx = zoom_state.center_x;
    let cy = zoom_state.center_y;

    // Visible region in source pixels
    let vis_w = w / scale;
    let vis_h = h / scale;

    // Top-left of visible region, clamped to frame bounds
    let src_x = (cx * w - vis_w / 2.0).clamp(0.0, w - vis_w);
    let src_y = (cy * h - vis_h / 2.0).clamp(0.0, h - vis_h);

    let frame_size = (width * height * 4) as usize;
    temp_buffer.resize(frame_size, 0);

    let w_i32 = width as i32;
    let h_i32 = height as i32;

    // Bilinear interpolation: map each output pixel to source position
    for out_y in 0..height {
        for out_x in 0..width {
            // Map output pixel → source coordinate
            let fx = src_x + (out_x as f64 / w) * vis_w;
            let fy = src_y + (out_y as f64 / h) * vis_h;

            let x0 = fx.floor() as i32;
            let y0 = fy.floor() as i32;
            let x1 = x0 + 1;
            let y1 = y0 + 1;
            let dx = (fx - x0 as f64) as f32;
            let dy = (fy - y0 as f64) as f32;
            let inv_dx = 1.0 - dx;
            let inv_dy = 1.0 - dy;

            // Clamp to frame bounds
            let x0c = x0.clamp(0, w_i32 - 1) as usize;
            let y0c = y0.clamp(0, h_i32 - 1) as usize;
            let x1c = x1.clamp(0, w_i32 - 1) as usize;
            let y1c = y1.clamp(0, h_i32 - 1) as usize;

            let stride = width as usize * 4;
            let i00 = y0c * stride + x0c * 4;
            let i10 = y0c * stride + x1c * 4;
            let i01 = y1c * stride + x0c * 4;
            let i11 = y1c * stride + x1c * 4;

            let out_idx = (out_y as usize * stride) + (out_x as usize * 4);

            // Bilinear blend for each RGBA channel
            for c in 0..4usize {
                let p00 = frame_buffer[i00 + c] as f32;
                let p10 = frame_buffer[i10 + c] as f32;
                let p01 = frame_buffer[i01 + c] as f32;
                let p11 = frame_buffer[i11 + c] as f32;

                let val =
                    p00 * inv_dx * inv_dy + p10 * dx * inv_dy + p01 * inv_dx * dy + p11 * dx * dy;

                temp_buffer[out_idx + c] = val.clamp(0.0, 255.0) as u8;
            }
        }
    }

    frame_buffer.copy_from_slice(temp_buffer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_zoom_trajectory() {
        let default_config = SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        };
        let trajectory = simulate_zoom_trajectory(
            &[],
            &[],
            &default_config,
            &default_config,
            60.0,
            1000,
            false,
        );
        assert_eq!(trajectory.len(), 60);
        for frame in &trajectory {
            assert!((frame.scale - 1.0).abs() < 0.01);
            assert!((frame.center_x - 0.5).abs() < 0.01);
            assert!((frame.center_y - 0.5).abs() < 0.01);
        }
    }

    #[test]
    fn test_zoom_reaches_target() {
        let blocks = vec![ZoomBlock {
            start_time_ms: 100,
            end_time_ms: 2000,
            zoom_level: 2.0,
            center_x: 0.7,
            center_y: 0.3,
            kind: None,
            zoom_in_speed: None,
            zoom_out_speed: None,
            centers: vec![],
        }];
        let zoom_config = SpringConfig {
            tension: 280.0,
            friction: 38.0,
            mass: 1.0,
        };
        let cursor_config = SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        };
        let trajectory = simulate_zoom_trajectory(
            &blocks,
            &[],
            &zoom_config,
            &cursor_config,
            60.0,
            3000,
            false,
        );
        // Mid-zoom frame should be near target
        let mid = &trajectory[60]; // 1000ms
        assert!(
            mid.scale > 1.5,
            "scale should approach 2.0, got {}",
            mid.scale
        );
    }

    #[test]
    fn test_apply_zoom_noop_at_scale_1() {
        let mut frame = vec![128u8; 4 * 4 * 4]; // 4x4 RGBA
        let original = frame.clone();
        let mut temp = Vec::new();
        apply_zoom_to_frame(
            &mut frame,
            &mut temp,
            4,
            4,
            &ZoomFrameState {
                scale: 1.0,
                center_x: 0.5,
                center_y: 0.5,
            },
        );
        assert_eq!(frame, original);
    }
}
