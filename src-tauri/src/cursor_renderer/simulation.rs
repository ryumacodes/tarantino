use super::*;
use crate::video_processing::CursorSettings;
use crate::video_processing::zoom_trajectory::{CAMERA_SIMULATION_FPS, ZoomFrameState};

mod visibility;
#[cfg(test)]
mod visibility_tests;

pub fn parse_cursor_events(
    events: &[serde_json::Value],
    _scale_factor: f64,
    effective_x: f64,
    effective_y: f64,
    effective_width: f64,
    effective_height: f64,
) -> Vec<CursorEvent> {
    if ![effective_x,effective_y,effective_width,effective_height].iter().all(|v|v.is_finite())
        || effective_width<=0.0 || effective_height<=0.0 { return Vec::new(); }
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
            // Preserve off-window travel for the spring; clipping belongs to
            // composition, otherwise the pointer sticks to the recording edge.
            let norm_x = adjusted_x / effective_width;
            let norm_y = adjusted_y / effective_height;
            if !norm_x.is_finite() || !norm_y.is_finite() { return None; }

            Some(CursorEvent { is_release: base.get("event_type").is_some_and(|kind|
                    kind.as_str()==Some("ButtonRelease") || kind.get("ButtonRelease").is_some()),
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
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TrailPoint {
    pub x: f32,
    pub y: f32,
    pub alpha: f32,
    pub size: f32,
}

/// Per-frame cursor state for GPU rendering (position + effect state, no image data).
#[derive(Clone, Debug, Default)]
pub struct CursorFrameState {
    pub x: f32,
    pub y: f32,
    pub press_amount: f32, // relative size reduction; zero at rest
    pub fade_blur_radius: f32, // stage pixels, independent of cursor size
    pub opacity: f32,         // unclamped spring value; clamp after temporal sampling
    pub rotation: f32,        // degrees
    pub is_clicking: f32,     // 0 or 1
    pub ripple_progress: f32, // 0 = no ripple, 0.0-1.0 = active
    pub ripple_x: f32,
    pub ripple_y: f32,
    pub circle_hl_progress: f32, // 0=inactive, 0-1=active
    pub circle_hl_x: f32,
    pub circle_hl_y: f32,
    pub trail_count: f32,
    trail_points: Option<std::sync::Arc<[TrailPoint]>>,
}

impl CursorFrameState {
    pub(crate) fn interpolate(&self, next: &Self, fraction: f32) -> Self {
        let lerp = |a: f32, b: f32| a + (b - a) * fraction;
        Self {
            x: lerp(self.x, next.x), y: lerp(self.y, next.y),
            opacity: lerp(self.opacity, next.opacity),
            rotation: lerp(self.rotation, next.rotation),
            fade_blur_radius: lerp(self.fade_blur_radius, next.fade_blur_radius),
            press_amount: lerp(self.press_amount, next.press_amount),
            ..self.clone()
        }
    }

    pub fn trail_points(&self) -> &[TrailPoint] {
        self.trail_points.as_deref().unwrap_or(&[])
    }
}

/// Simulate cursor positions with spring physics for GPU rendering.
///
/// Returns per-frame cursor state without rendering any images.
/// Cursor coordinates are in zoomed output space when zoom_trajectory is provided.
/// Now includes idle fade, rotation, circle highlight, click state, and trail.
#[cfg(test)]
fn simulate_cursor_positions(
    events: &[CursorEvent],
    spring_config: &SpringConfig,
    fps: f64,
    duration_ms: u64,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>,
    cursor_settings: &CursorSettings,
) -> Vec<CursorFrameState> {
    simulate_cursor_frames(
        events,
        spring_config,
        fps,
        duration_ms,
        zoom_trajectory,
        cursor_settings,
        Clone::clone,
    )
}

/// Project samples directly into the caller's output format to avoid retaining
/// unused trail/effect fields in compact preview responses.
pub fn simulate_cursor_frames<T>(
    events: &[CursorEvent],
    spring_config: &SpringConfig,
    fps: f64,
    duration_ms: u64,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>,
    cursor_settings: &CursorSettings,
    project: impl Fn(&CursorFrameState) -> T,
) -> Vec<T> {
    simulate_cursor_frames_with_rotation(events,spring_config,fps,duration_ms,zoom_trajectory,
        cursor_settings,None,project)
}

pub fn simulate_cursor_frames_with_rotation<T>(
    events: &[CursorEvent],
    spring_config: &SpringConfig,
    fps: f64,
    duration_ms: u64,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>,
    cursor_settings: &CursorSettings,
    rotation_keys: Option<&[super::rotation::RotationKey]>,
    project: impl Fn(&CursorFrameState) -> T,
) -> Vec<T> {
    simulate_cursor_frames_with_activity(events, spring_config, fps, duration_ms,
        zoom_trajectory, cursor_settings, rotation_keys, None, project)
}

pub(crate) fn simulate_cursor_frames_with_activity<T>(
    events: &[CursorEvent], spring_config: &SpringConfig, fps: f64, duration_ms: u64,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>, cursor_settings: &CursorSettings,
    rotation_keys: Option<&[super::rotation::RotationKey]>,
    typing: Option<&dyn Fn(f64) -> bool>, project: impl Fn(&CursorFrameState) -> T,
) -> Vec<T> {
    simulate_cursor_frames_from(
        events,
        spring_config,
        fps,
        duration_ms,
        zoom_trajectory,
        cursor_settings,
        0,
        fps,
        rotation_keys,
        typing,
        None,
        None,
        project,
    )
}

/// Integrate source history but retain only the requested trimmed output samples.
pub fn simulate_cursor_positions_for_range(
    events: &[CursorEvent],
    spring_config: &SpringConfig,
    fps: f64,
    range: crate::video_processing::export_range::ExportRange,
    source_zoom: &Option<Vec<ZoomFrameState>>,
    cursor_settings: &CursorSettings,
) -> Vec<CursorFrameState> {
    simulate_cursor_positions_for_range_with_rotation(events,spring_config,fps,range,source_zoom,
        cursor_settings,None)
}

pub fn simulate_cursor_positions_for_range_with_rotation(
    events: &[CursorEvent],
    spring_config: &SpringConfig,
    fps: f64,
    range: crate::video_processing::export_range::ExportRange,
    source_zoom: &Option<Vec<ZoomFrameState>>,
    cursor_settings: &CursorSettings,
    rotation_keys: Option<&[super::rotation::RotationKey]>,
) -> Vec<CursorFrameState> {
    simulate_cursor_range_with_activity(events, spring_config, fps, range, source_zoom,
        cursor_settings, rotation_keys, None)
}

pub(crate) fn simulate_cursor_range_with_activity(
    events: &[CursorEvent], spring_config: &SpringConfig, fps: f64,
    range: crate::video_processing::export_range::ExportRange,
    source_zoom: &Option<Vec<ZoomFrameState>>, cursor_settings: &CursorSettings,
    rotation_keys: Option<&[super::rotation::RotationKey]>, typing: Option<&dyn Fn(f64) -> bool>,
) -> Vec<CursorFrameState> {
    simulate_cursor_frames_from(
        events,
        spring_config,
        fps,
        range.end_ms,
        source_zoom,
        cursor_settings,
        range.start_ms,
        CAMERA_SIMULATION_FPS,
        rotation_keys,
        typing,
        None,
        None,
        Clone::clone,
    )
}

pub(crate) fn simulate_cursor_frames_from<T>(
    events: &[CursorEvent],
    spring_config: &SpringConfig,
    fps: f64,
    duration_ms: u64,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>,
    cursor_settings: &CursorSettings,
    start_ms: u64,
    zoom_fps: f64,
    rotation_keys: Option<&[super::rotation::RotationKey]>,
    typing: Option<&dyn Fn(f64) -> bool>,
    planned: Option<&dyn Fn(f64) -> [f64;2]>,
    pressed: Option<(&dyn Fn(f64) -> bool, f64)>,
    project: impl Fn(&CursorFrameState) -> T,
) -> Vec<T> {
    if !fps.is_finite() || fps <= 0.0 {
        return Vec::new();
    }
    let output_count =
        ((duration_ms.saturating_sub(start_ms) as f64 * fps) / 1000.0).ceil() as usize;
    let total_frames = ((duration_ms as f64 * CAMERA_SIMULATION_FPS) / 1000.0).ceil() as u64;
    let dt = 1.0 / CAMERA_SIMULATION_FPS;
    let mut positions = Vec::with_capacity(output_count);

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
    let trail_length = cursor_settings.trail_length.unwrap_or(15).min(30) as usize;
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
    let press_animation=pressed.and_then(|(sample,rate)|super::press_animation::PressAnimation::build(
        duration_ms as f64,rate,spring_config,|time|sample(time.min(freeze_time_ms as f64))));
    let looped_events=loop_to_start.then(||super::loop_path::cursor_events(events,duration_ms,loop_duration_ms));
    let motion_events=looped_events.as_deref().unwrap_or(events);
    let loop_start_ms=motion_events.last().map_or(duration_ms,|event|event.timestamp_ms);

    // Track click events
    let click_events: Vec<(u64, f64, f64)> = motion_events
        .iter()
        .filter(|e| e.is_click)
        .map(|e| (e.timestamp_ms, e.x, e.y))
        .collect();

    let mut last_click_time: Option<u64> = None;
    let mut active_ripple: Option<(f64, f64, u64)> = None;
    let mut active_circle_hl: Option<(f64, f64, u64)> = None;
    let ripple_duration_frames = (0.6 * CAMERA_SIMULATION_FPS) as u64;
    let circle_hl_duration_frames = (0.3 * CAMERA_SIMULATION_FPS) as u64;

    let mut visibility = visibility::VisibilityTimeline::new();

    let mut angular_state = super::angular_spring::AngularTimeline::new(
        rotation_keys.map_or(0.0, |keys| super::rotation::sample(keys, 0.0)));
    // Rotation state
    let mut horizontal_velocity: f64 = 0.0;
    let mut last_x_position: f64 = init_x;

    // Trail history (pre-zoom positions for trail rendering)
    let mut trail_history: Vec<(f64, f64)> = Vec::with_capacity(trail_length);
    let mut trail_cache: Option<(u64, f32, std::sync::Arc<[TrailPoint]>)> = None;

    let visibility_targets = |time: f64| {
        let count = events.partition_point(|event| event.timestamp_ms as f64 <= time.min(freeze_time_ms as f64));
        let latest = count.checked_sub(1).map(|index| &events[index]);
        (latest.is_some(), hide_when_idle && latest.is_some_and(|event| time-event.timestamp_ms as f64>idle_timeout_ms),
            typing.is_some_and(|active| active(time)))
    };
    let mut previous_sample: Option<CursorFrameState> = None;
    for frame_num in 0..=total_frames {
        let time_ms = ((frame_num as f64 * 1000.0) / CAMERA_SIMULATION_FPS) as u64;
        let time_ms_f = frame_num as f64 * 1000.0 / CAMERA_SIMULATION_FPS;

        // Find target cursor position
        let effective_time_ms = time_ms.min(freeze_time_ms);
        let (target_x, target_y) = find_target_at_time(motion_events, effective_time_ms);

        // Use actual input activity, not interpolated motion toward a future event.
        // A stationary click also wakes the pointer after an idle fade.
        let event_count = events.partition_point(|event| event.timestamp_ms <= effective_time_ms);
        let latest_event = event_count.checked_sub(1).map(|index| &events[index]);

        // Apply spring physics
        if planned.is_none() { spring_x = spring_step(
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
        ); }

        let position=planned.map_or([spring_x.value,spring_y.value],|sample|sample(time_ms_f));
        let mut cursor_x = position[0];
        let mut cursor_y = position[1];

        let [mut rendered_opacity, mut fade_blur_radius] = visibility.sample(time_ms_f, &visibility_targets);
        if latest_event.is_none() { rendered_opacity=0.0; fade_blur_radius=0.0; }

        // Rotation
        let dx = cursor_x - last_x_position;
        last_x_position = cursor_x;
        horizontal_velocity = horizontal_velocity * 0.85 + dx * 0.15;

        let mut rotation = static_rotation;
        if rotate_while_moving {
            let max_rot = 30.0 * (rotation_intensity / 100.0);
            if let Some(keys) = rotation_keys {
                rotation += angular_state.sample(time_ms_f, |time| {
                    let target = super::rotation::sample(keys, time.min(freeze_time_ms as f64));
                    let resting_ms = (time-freeze_time_ms as f64).max(0.0);
                    target * if resting_ms>=720.0 {0.0} else {(-resting_ms/120.0).exp()}
                });
            } else {
                rotation += (horizontal_velocity * 2.0).clamp(-max_rot, max_rot);
            }
        }

        // Transform through zoom if active
        if let Some(trajectory) = zoom_trajectory {
            if !trajectory.is_empty() {
                let tidx = ((time_ms_f * zoom_fps / 1000.0 + 1e-7).floor() as usize)
                    .min(trajectory.len() - 1);
                let zs = &trajectory[tidx];
                if zs.scale > 1.0 {
                    cursor_x = (cursor_x - zs.center_x) * zs.scale + 0.5;
                    cursor_y = (cursor_y - zs.center_y) * zs.scale + 0.5;
                }
            }
        }

        // Update trail (post-zoom positions)
        if trail_enabled && latest_event.is_some() && frame_num % 2 == 0 {
            // A fixed 60 Hz history keeps trail duration independent of output fps.
            trail_history.push((cursor_x, cursor_y));
            while trail_history.len() > trail_length {
                trail_history.remove(0);
            }
        }

        // Check for click
        let allow_click_effects = (!stop_at_end || time_ms <= freeze_time_ms)
            && (!loop_to_start || time_ms < loop_start_ms);
        let click_at_time = if allow_click_effects {
            // Events are ordered. Find the first non-expired click rather than
            // rescanning the recording's complete click history at every frame.
            let first =
                click_events.partition_point(|(t, _, _)| time_ms >= *t && time_ms - *t >= 100);
            click_events
                .get(first)
                .filter(|(t, _, _)| time_ms >= *t && *t <= freeze_time_ms)
        } else {
            None
        };
        let is_clicking = click_at_time.is_some();

        if let Some((click_time, exact_x, exact_y)) = click_at_time {
            if last_click_time.map_or(true, |t| *click_time > t + 100) {
                // Transform effect position through zoom
                let mut rx = *exact_x;
                let mut ry = *exact_y;
                if let Some(trajectory) = zoom_trajectory {
                    if !trajectory.is_empty() {
                        let tidx = ((time_ms_f * zoom_fps / 1000.0 + 1e-7).floor() as usize)
                            .min(trajectory.len() - 1);
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

        // Build only the neighboring samples needed for the next output frame.
        let next_sample = (start_ms as f64 * CAMERA_SIMULATION_FPS / 1000.0
            + positions.len() as f64 * CAMERA_SIMULATION_FPS / fps
            + 1e-7)
            .floor() as u64;
        if positions.len() >= output_count || frame_num < next_sample {
            continue;
        }
        let trail_count = trail_history.len();
        let trail_points = if trail_count == 0 {
            None
        } else {
            // History advances at 60 Hz. Consecutive 120 Hz samples can share
            // immutable trail payloads unless idle fading changes their alpha.
            let key = (frame_num / 2, rendered_opacity.clamp(0.0, 1.0) as f32);
            if let Some((tick, alpha, points)) = &trail_cache {
                if (*tick, *alpha) == key {
                    Some(points.clone())
                } else {
                    None
                }
            } else {
                None
            }
            .or_else(|| {
                let points = trail_history
                    .iter()
                    .enumerate()
                    .map(|(i, &(tx, ty))| {
                        let progress = i as f32 / trail_count as f32;
                        TrailPoint {
                            x: tx as f32,
                            y: ty as f32,
                            alpha: progress * trail_opacity as f32 * rendered_opacity.clamp(0.0, 1.0) as f32,
                            size: (2.0 + progress * 4.0) * cursor_scale as f32,
                        }
                    })
                    .collect::<std::sync::Arc<[TrailPoint]>>();
                trail_cache = Some((key.0, key.1, points.clone()));
                Some(points)
            })
        };

        let state = CursorFrameState {
            press_amount: 0.0,
            x: cursor_x as f32,
            y: cursor_y as f32,
            opacity: rendered_opacity as f32,
            fade_blur_radius: fade_blur_radius as f32,
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
        };
        while positions.len() < output_count {
            let sample_time = start_ms as f64 + positions.len() as f64 * 1000.0 / fps;
            let position = sample_time * CAMERA_SIMULATION_FPS / 1000.0;
            let position = if (position - position.round()).abs() < 1e-7 { position.round() } else { position };
            if position > frame_num as f64 { break; }
            let mut sampled = if position == frame_num as f64 {
                state.clone()
            } else {
                previous_sample.as_ref().unwrap().interpolate(&state, (position - position.floor()) as f32)
            };
            let [opacity, blur] = visibility.sample(sample_time, &visibility_targets);
            let has_pointer = visibility_targets(sample_time).0;
            sampled.opacity = if has_pointer { opacity as f32 } else { 0.0 };
            sampled.fade_blur_radius = if has_pointer { blur as f32 } else { 0.0 };
            sampled.press_amount = press_animation.as_ref().map_or(0.0, |animation| animation.at(sample_time) as f32);
            positions.push(project(&sampled));
        }
        previous_sample = Some(state);

    }

    positions
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
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

#[cfg(test)]
mod cadence_tests;
