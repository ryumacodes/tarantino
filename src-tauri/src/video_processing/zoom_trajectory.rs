//! Zoom trajectory simulation using spring physics.
//!
//! Produces source-time zoom level and center coordinates for preview and
//! export from one fixed simulation, independent of display or output cadence.

use crate::cursor_renderer::{CursorEvent, SpringConfig, SpringState, TargetTolerance, spring_step, step_frame_spring};
use crate::video_processing::types::ZoomBlock;
mod schedule;
mod center_lookup;
mod viewport;
mod rectangle;
pub(crate) use viewport::CameraViewport;
pub(crate) mod targets;
use schedule::ZoomSchedule;

/// Per-frame zoom state produced by the spring simulation
#[derive(Clone, Debug)]
pub struct ZoomFrameState {
    /// Top-left and size relative to the unzoomed source plane.
    pub rect: Option<[f64; 4]>,
    pub scale: f64,
    pub center_x: f64,
    pub center_y: f64,
}

impl ZoomFrameState {
    fn interpolate(&self, next: &Self, fraction: f64) -> Self {
        let lerp = |a:f64,b:f64| a+(b-a)*fraction;
        let rect = match (self.rect,next.rect) {
            (Some(a),Some(b)) => Some(std::array::from_fn(|i|lerp(a[i],b[i]))),
            _ => None,
        };
        Self { rect, scale:lerp(self.scale,next.scale),
            center_x:lerp(self.center_x,next.center_x),center_y:lerp(self.center_y,next.center_y) }
    }

    pub(crate) fn from_sample(value: [f64; 7]) -> Self {
        Self { scale: value[0], center_x: value[1], center_y: value[2],
            rect: (value[5] > 0.0 && value[6] > 0.0).then_some([value[3],value[4],value[5],value[6]]) }
    }

    pub(crate) fn sample(&self) -> [f64; 7] {
        let [x,y,width,height] = self.rect.unwrap_or([0.0;4]);
        [self.scale,self.center_x,self.center_y,x,y,width,height]
    }
}

pub(crate) fn sample_camera_frame(frames: &[ZoomFrameState], time_ms: f64) -> Option<ZoomFrameState> {
    if frames.is_empty() { return None; }
    let position = (time_ms * CAMERA_SIMULATION_FPS / 1000.0).max(0.0);
    let position = if (position - position.round()).abs() < 1e-7 { position.round() } else { position };
    let index = (position.floor() as usize).min(frames.len() - 1);
    let current = &frames[index];
    match frames.get(index + 1) {
        Some(next) => Some(current.interpolate(next, position - index as f64)),
        None => Some(current.clone()),
    }
}

/// Legacy center-panning spring for active zoom blocks.
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
/// Follows the preview target-resolution rules:
/// - Blocks resolve to continuous camera targets
/// - Per-block zoom_in_speed / zoom_out_speed override global zoom_spring_config
/// - Separate spring for zoom scale and center (uses ZOOM_PAN_CONFIG)
/// - Edge clamping on both target and animated values
///
/// `cursor_events` are RAW mouse events (not spring-smoothed) used for cursor-following.
/// `zoom_spring_config` comes from SPRING_PRESETS[zoomSpeedPreset] via the frontend (global fallback).
/// Named screen styles use one spring for scale and panning. Legacy presets retain phase-specific panning.
// Preview/export transport stays at 120Hz; camera dynamics advance at 60Hz.
pub const CAMERA_SIMULATION_FPS: f64 = 120.0;
const CAMERA_STEP_FPS: f64 = 60.0;

/// Evaluate a fixed timeline before sampling output frames. Render cadence must
/// not change the camera path or its adaptive damping decisions.
pub fn simulate_zoom_trajectory(
    zoom_blocks: &[ZoomBlock],
    cursor_events: &[CursorEvent],
    zoom_spring_config: &SpringConfig,
    unified_screen_motion: bool,
    zoom_speed_preset: &str,
    fps: f64,
    duration_ms: u64,
    window_mode: bool,
) -> Vec<ZoomFrameState> {
    simulate_zoom_trajectory_in_viewport(zoom_blocks, cursor_events, zoom_spring_config,
        unified_screen_motion, zoom_speed_preset, fps, duration_ms, window_mode, CameraViewport::default(),None)
}

pub(crate) fn simulate_zoom_trajectory_in_viewport(
    zoom_blocks: &[ZoomBlock],
    cursor_events: &[CursorEvent],
    zoom_spring_config: &SpringConfig,
    unified_screen_motion: bool,
    zoom_speed_preset: &str,
    fps: f64,
    duration_ms: u64,
    window_mode: bool,
    viewport: CameraViewport,
    activity: Option<targets::CameraActivity<'_>>,
) -> Vec<ZoomFrameState> {
    if !fps.is_finite() || fps <= 0.0 || duration_ms == 0 {
        return Vec::new();
    }
    let frames = simulate_zoom_steps(
        zoom_blocks,
        cursor_events,
        zoom_spring_config,
        unified_screen_motion,
        zoom_speed_preset,
        CAMERA_STEP_FPS,
        duration_ms.saturating_add((1000.0 / CAMERA_STEP_FPS).ceil() as u64),
        window_mode,
        viewport,
        activity,
    );
    let count = (duration_ms as f64 * fps / 1000.0).ceil() as usize;
    (0..count)
        .map(|index| {
            let position = index as f64 * CAMERA_STEP_FPS / fps;
            let position = if (position - position.round()).abs() < 1e-7 {
                position.round()
            } else { position };
            let source = position.floor() as usize;
            let lower = &frames[source];
            let fraction = position - source as f64;
            if fraction == 0.0 { return lower.clone(); }
            let upper = &frames[source + 1];
            lower.interpolate(upper,fraction)
        })
        .collect()
}

fn simulate_zoom_steps(
    zoom_blocks: &[ZoomBlock],
    cursor_events: &[CursorEvent],
    zoom_spring_config: &SpringConfig,
    unified_screen_motion: bool,
    _zoom_speed_preset: &str,
    fps: f64,
    duration_ms: u64,
    window_mode: bool,
    viewport: CameraViewport,
    activity: Option<targets::CameraActivity<'_>>,
) -> Vec<ZoomFrameState> {
    // Sort centers once per trajectory, not once per simulated frame.
    let mut prepared_blocks: Vec<_> = zoom_blocks.iter()
        .filter(|block| !block.is_disabled).cloned().collect();
    for block in &mut prepared_blocks {
        block.centers.sort_by_key(|center| center.time);
    }
    let zoom_blocks = prepared_blocks.as_slice();
    let automatic_targets: Vec<_> = zoom_blocks.iter()
        .map(|block| if block.loupe.is_some() { Vec::new() }
            else { targets::prepare_in_viewport(block, cursor_events, viewport,activity) }).collect();
    let total_frames = ((duration_ms as f64 * fps) / 1000.0).ceil() as u64;
    let dt = 1.0 / fps;

    // The first timeline sample initializes these states at its resolved target.
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

    let mut settling = TargetTolerance::default();
    let mut rectangle_motion: Option<rectangle::RectMotion> = None;
    let mut trajectory = Vec::with_capacity(total_frames as usize);
    let mut prev_block_idx: Option<usize> = None;
    let mut last_block_out_config: Option<SpringConfig> = None;
    let mut schedule = ZoomSchedule::new(zoom_blocks);

    for frame_num in 0..total_frames {
        let time_ms = (frame_num as f64 * 1000.0) / fps;

        let mut target_scale = 1.0;
        let mut target_center_x = 0.5;
        let mut target_center_y = 0.5;
        let mut is_zooming = false;
        let mut is_follow_phase = false;
        // Default: use last block's out config for zoom-out after block ends, else global
        let mut active_zoom_config = last_block_out_config.unwrap_or(*zoom_spring_config);

        let (active_block, instant_motion) = schedule.advance(time_ms);
        if let Some(block_idx) = active_block {
            let block = &zoom_blocks[block_idx];
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
                .first()
                .map(|center| center.time)
                .unwrap_or(block.start_time_ms);
            let (anchor_center_x, anchor_center_y) = resolve_center_at_time(
                &block.centers,
                time_ms,
                block.center_x,
                block.center_y,
                block.kind.as_deref() == Some("typing"),
            );

            if block.is_manual {
                target_center_x = block.center_x;
                target_center_y = block.center_y;
            } else if time_ms >= first_center_time as f64 && block.kind.as_deref() == Some("typing")
            {
                is_follow_phase = true;
                target_center_x = anchor_center_x;
                target_center_y = anchor_center_y;
            } else if block.kind.as_deref() != Some("typing")
                && !automatic_targets[block_idx].is_empty() {
                is_follow_phase = true;
                let (x, y) = targets::at(&automatic_targets[block_idx], time_ms).unwrap();
                target_center_x = x;
                target_center_y = y;
            } else if time_ms >= first_center_time as f64 {
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

            // Range endpoints start the exit; editing a range must not shorten its hold.
            last_block_out_config = Some(block_out_config);
        }

        if !is_zooming {
            target_center_x = 0.5;
            target_center_y = 0.5;
            prev_block_idx = None;
        }

        if unified_screen_motion && window_mode {
            let source = viewport.source;
            let target = source.zoom_to(viewport.zoom_area,(target_center_x,target_center_y),target_scale,None);
            let motion = rectangle_motion.get_or_insert_with(|| rectangle::RectMotion::new(target));
            let config = if instant_motion { SpringConfig { mass:0.0,..active_zoom_config } }
                else { active_zoom_config };
            let rect = motion.advance(target,&config,if frame_num==0 {0.0}else{dt*1000.0});
            trajectory.push(ZoomFrameState {
                rect:Some([(rect.x-source.x)/source.width,(rect.y-source.y)/source.height,
                    rect.width/source.width,rect.height/source.height]),
                scale:rect.width/source.width,
                center_x:(viewport.zoom_area.x+viewport.zoom_area.width/2.0-rect.x)/rect.width,
                center_y:(viewport.zoom_area.y+viewport.zoom_area.height/2.0-rect.y)/rect.height,
            });
            continue;
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

        if frame_num == 0 {
            zoom_spring = SpringState { value: target_scale, velocity: 0.0 };
            center_spring_x = SpringState { value: target_center_x, velocity: 0.0 };
            center_spring_y = SpringState { value: target_center_y, velocity: 0.0 };
        }

        let instant_config = SpringConfig {
            tension: 1.0,
            friction: 0.0,
            mass: 0.0,
        };
        if instant_motion {
            active_zoom_config = instant_config;
        }
        let pan_config = if instant_motion {
            &instant_config
        } else if unified_screen_motion {
            &active_zoom_config
        } else if is_follow_phase {
            &FOLLOW_PAN_CONFIG
        } else if is_zooming {
            &ZOOM_PAN_CONFIG
        } else {
            &active_zoom_config
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
        zoom_spring = step_frame_spring(
            zoom_spring,
            target_scale,
            &active_zoom_config,
            dt * 1000.0,
            settling.update(zoom_spring.value, target_scale),
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

        trajectory.push(ZoomFrameState { rect: None,
            scale: animated_scale,
            center_x: animated_cx,
            center_y: animated_cy,
        });
    }

    trajectory
}



pub(crate) use center_lookup::resolve_center_at_time;

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
#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmark;
