//! Cursor Renderer - Generates cursor overlay frames for video export
//!
//! Ports the spring physics and cursor rendering logic from MouseCursorOverlay.tsx
//! to produce frame-by-frame cursor animations that match the preview exactly.

use anyhow::Result;
use image::{Rgba, RgbaImage};
use imageproc::drawing::{
    draw_antialiased_line_segment_mut, draw_filled_circle_mut, draw_hollow_circle_mut,
    draw_polygon_mut,
};
mod simulation;

use crate::video_processing::CursorSettings;
use imageproc::point::Point;
pub use simulation::{
    parse_cursor_events, render_cursor_shape, simulate_cursor_positions, CursorFrameState,
    TrailPoint,
};

/// Spring physics state for smooth cursor animation
#[derive(Clone, Copy, Debug)]
pub struct SpringState {
    pub value: f64,
    pub velocity: f64,
}

impl Default for SpringState {
    fn default() -> Self {
        Self {
            value: 0.0,
            velocity: 0.0,
        }
    }
}

/// Spring configuration (tension, friction, mass)
#[derive(Clone, Copy, Debug)]
pub struct SpringConfig {
    pub tension: f64,
    pub friction: f64,
    pub mass: f64,
}

/// Build spring config from CursorSettings.
/// Uses the actual values passed from frontend (SPRING_PRESETS is the single source of truth).
/// Falls back to preset name lookup only if explicit values aren't provided.
pub fn get_spring_config(settings: &CursorSettings) -> SpringConfig {
    // Prefer explicit values from frontend (source of truth: stores/editor/constants.ts)
    if let (Some(t), Some(f), Some(m)) = (
        settings.spring_tension,
        settings.spring_friction,
        settings.spring_mass,
    ) {
        return SpringConfig {
            tension: t,
            friction: f,
            mass: m,
        };
    }
    // Fallback: resolve preset name (should not normally be needed)
    let name = settings.speed_preset.as_deref().unwrap_or("mellow");
    match name {
        "slow" => SpringConfig {
            tension: 120.0,
            friction: 28.0,
            mass: 1.0,
        },
        "mellow" => SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        },
        "quick" => SpringConfig {
            tension: 280.0,
            friction: 38.0,
            mass: 1.0,
        },
        "rapid" => SpringConfig {
            tension: 400.0,
            friction: 44.0,
            mass: 1.0,
        },
        _ => SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        },
    }
}

/// Spring physics step function matching MouseCursorOverlay.tsx springStep()
pub fn spring_step(
    current: f64,
    target: f64,
    velocity: f64,
    config: &SpringConfig,
    dt: f64,
) -> SpringState {
    let safe_dt = dt.min(0.064); // Cap at ~15fps minimum

    let displacement = current - target;
    let spring_force = -config.tension * displacement;
    let damping_force = -config.friction * velocity;
    let acceleration = (spring_force + damping_force) / config.mass;

    let new_velocity = velocity + acceleration * safe_dt;
    let new_value = current + new_velocity * safe_dt;

    // Snap to target if close enough and velocity is low
    // Must match preview threshold (0.0001) for sub-pixel precision
    if displacement.abs() < 0.0001 && new_velocity.abs() < 0.0001 {
        return SpringState {
            value: target,
            velocity: 0.0,
        };
    }

    SpringState {
        value: new_value,
        velocity: new_velocity,
    }
}

/// Parse hex color to RGB components
pub fn parse_hex_rgb(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    (r, g, b)
}

/// Mouse event from sidecar for processing
#[derive(Clone, Debug)]
pub struct CursorEvent {
    pub timestamp_ms: u64,
    pub x: f64, // Normalized 0-1
    pub y: f64, // Normalized 0-1
    pub is_click: bool,
}

/// Cursor renderer with state for animation
pub struct CursorRenderer {
    config: CursorSettings,
    spring_config: SpringConfig,
    width: u32,
    height: u32,
    fps: f64,

    // Animation state
    spring_x: SpringState,
    spring_y: SpringState,
    trail_history: Vec<(f64, f64)>, // Position history for trail
    active_ripples: Vec<RippleState>,
    last_click_time: Option<u64>,
}

#[derive(Clone, Debug)]
struct RippleState {
    x: f64,
    y: f64,
    start_frame: u64,
    duration_frames: u64,
}

impl CursorRenderer {
    pub fn new(config: CursorSettings, width: u32, height: u32, fps: f64) -> Self {
        let spring_config = get_spring_config(&config);

        Self {
            config,
            spring_config,
            width,
            height,
            fps,
            spring_x: SpringState::default(),
            spring_y: SpringState::default(),
            trail_history: Vec::new(),
            active_ripples: Vec::new(),
            last_click_time: None,
        }
    }

    /// Generate all cursor frames for the video
    pub fn generate_frames(
        &mut self,
        events: &[CursorEvent],
        duration_ms: u64,
    ) -> Result<Vec<RgbaImage>> {
        let total_frames = ((duration_ms as f64 * self.fps) / 1000.0).ceil() as u64;
        let mut frames = Vec::with_capacity(total_frames as usize);
        let dt = 1.0 / self.fps;

        // Collect click events with their EXACT (pre-spring) coordinates
        // This ensures ripples appear at the actual click position, not the spring-animated position
        let click_events: Vec<(u64, f64, f64)> = events
            .iter()
            .filter(|e| e.is_click)
            .map(|e| (e.timestamp_ms, e.x, e.y))
            .collect();

        // Initialize spring to first cursor position
        if let Some(first_event) = events.first() {
            self.spring_x = SpringState {
                value: first_event.x,
                velocity: 0.0,
            };
            self.spring_y = SpringState {
                value: first_event.y,
                velocity: 0.0,
            };
        }

        for frame_num in 0..total_frames {
            let time_ms = ((frame_num as f64 * 1000.0) / self.fps) as u64;

            // Find target cursor position at this time
            let (target_x, target_y, _is_click) = self.find_cursor_at_time(events, time_ms);

            // Apply spring physics
            self.spring_x = spring_step(
                self.spring_x.value,
                target_x,
                self.spring_x.velocity,
                &self.spring_config,
                dt,
            );
            self.spring_y = spring_step(
                self.spring_y.value,
                target_y,
                self.spring_y.velocity,
                &self.spring_config,
                dt,
            );

            let cursor_x = self.spring_x.value;
            let cursor_y = self.spring_y.value;

            // Update trail history
            if self.config.trail_enabled.unwrap_or(false) {
                let max_trail = self.config.trail_length.unwrap_or(10) as usize;
                self.trail_history.push((cursor_x, cursor_y));
                if self.trail_history.len() > max_trail {
                    self.trail_history.remove(0);
                }
            }

            // Handle click effects - use EXACT click coordinates, not spring-animated position
            // Find click event at this time and use its exact coordinates
            let click_at_time = click_events
                .iter()
                .find(|(t, _, _)| time_ms >= *t && time_ms < *t + 100);

            if let Some((click_time, exact_x, exact_y)) = click_at_time {
                if self.last_click_time.map_or(true, |t| *click_time > t + 100) {
                    // Spawn ripple at EXACT click position
                    let ripple_duration = (0.6 * self.fps) as u64; // 600ms
                    self.active_ripples.push(RippleState {
                        x: *exact_x, // Use EXACT click position
                        y: *exact_y, // Not spring-animated
                        start_frame: frame_num,
                        duration_frames: ripple_duration,
                    });
                    self.last_click_time = Some(*click_time);
                }
            }

            // Clean up expired ripples
            self.active_ripples
                .retain(|r| frame_num < r.start_frame + r.duration_frames);

            // Render the frame (is_click derived from click_at_time check)
            let is_click = click_at_time.is_some();
            let frame = self.render_frame(frame_num, cursor_x, cursor_y, is_click);
            frames.push(frame);
        }

        Ok(frames)
    }

    fn find_cursor_at_time(&self, events: &[CursorEvent], time_ms: u64) -> (f64, f64, bool) {
        if events.is_empty() {
            return (0.5, 0.5, false);
        }

        let mut closest_idx = 0;
        let mut closest_diff = u64::MAX;
        let mut is_click = false;

        for (i, event) in events.iter().enumerate() {
            let diff = if event.timestamp_ms > time_ms {
                event.timestamp_ms - time_ms
            } else {
                time_ms - event.timestamp_ms
            };

            if diff < closest_diff {
                closest_diff = diff;
                closest_idx = i;
            }

            // Check for click within 100ms window
            if diff < 100 && event.is_click {
                is_click = true;
            }
        }

        let event = &events[closest_idx];
        (event.x, event.y, is_click)
    }

    fn render_frame(
        &self,
        frame_num: u64,
        cursor_x: f64,
        cursor_y: f64,
        _is_click: bool,
    ) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(self.width, self.height, Rgba([0, 0, 0, 0]));

        let px_x = (cursor_x * self.width as f64) as i32;
        let px_y = (cursor_y * self.height as f64) as i32;
        let cursor_scale = self.config.size.unwrap_or(1.0) as f32;

        // Get colors
        let cursor_color = self.config.color.as_deref().unwrap_or("#ffffff");
        let highlight_color = self.config.highlight_color.as_deref().unwrap_or("#ff6b6b");
        let ripple_color = self.config.ripple_color.as_deref().unwrap_or("#64b4ff");
        let (cursor_r, cursor_g, cursor_b) = parse_hex_rgb(cursor_color);
        let (hl_r, hl_g, hl_b) = parse_hex_rgb(highlight_color);
        let (ripple_r, ripple_g, ripple_b) = parse_hex_rgb(ripple_color);

        // 1. Render active ripples
        let click_effect = self.config.click_effect.as_deref().unwrap_or("ripple");
        if click_effect == "ripple" {
            for ripple in &self.active_ripples {
                let elapsed = frame_num - ripple.start_frame;
                let progress = elapsed as f64 / ripple.duration_frames as f64;
                if progress <= 1.0 {
                    let alpha = ((1.0 - progress) * 180.0) as u8;
                    let radius = (20.0 + progress * 40.0) * cursor_scale as f64;
                    let ripple_px_x = (ripple.x * self.width as f64) as i32;
                    let ripple_px_y = (ripple.y * self.height as f64) as i32;

                    draw_hollow_circle_mut(
                        &mut img,
                        (ripple_px_x, ripple_px_y),
                        radius as i32,
                        Rgba([ripple_r, ripple_g, ripple_b, alpha]),
                    );
                }
            }
        } else if click_effect == "circle" {
            for ripple in &self.active_ripples {
                let elapsed = frame_num - ripple.start_frame;
                let progress = elapsed as f64 / ripple.duration_frames as f64;
                if progress <= 1.0 {
                    let alpha = ((1.0 - progress) * 100.0) as u8;
                    let radius = (25.0 * cursor_scale as f64) as i32;
                    let ripple_px_x = (ripple.x * self.width as f64) as i32;
                    let ripple_px_y = (ripple.y * self.height as f64) as i32;

                    draw_filled_circle_mut(
                        &mut img,
                        (ripple_px_x, ripple_px_y),
                        radius,
                        Rgba([hl_r, hl_g, hl_b, alpha]),
                    );
                }
            }
        }

        // 2. Render trail
        if self.config.trail_enabled.unwrap_or(false) && self.trail_history.len() > 1 {
            let trail_opacity = self.config.trail_opacity.unwrap_or(0.5);
            for (i, &(tx, ty)) in self.trail_history.iter().enumerate() {
                let alpha =
                    ((i as f64 / self.trail_history.len() as f64) * trail_opacity * 255.0) as u8;
                let trail_size =
                    (5.0 * cursor_scale as f64 * (i as f64 / self.trail_history.len() as f64))
                        as i32;
                let trail_px_x = (tx * self.width as f64) as i32;
                let trail_px_y = (ty * self.height as f64) as i32;

                if trail_size > 0 {
                    draw_filled_circle_mut(
                        &mut img,
                        (trail_px_x, trail_px_y),
                        trail_size,
                        Rgba([cursor_r, cursor_g, cursor_b, alpha]),
                    );
                }
            }
        }

        // 3. Render cursor based on style
        // If always_use_pointer is enabled, force pointer style regardless of configured style
        let cursor_style = if self.config.always_use_pointer.unwrap_or(false) {
            "pointer"
        } else {
            self.config.style.as_deref().unwrap_or("pointer")
        };
        match cursor_style {
            "pointer" | "filled" | "outline" | "dotted" => {
                self.render_pointer_cursor(
                    &mut img,
                    px_x,
                    px_y,
                    cursor_scale,
                    cursor_r,
                    cursor_g,
                    cursor_b,
                    cursor_style,
                );
            }
            "circle" => {
                self.render_circle_cursor(
                    &mut img,
                    px_x,
                    px_y,
                    cursor_scale,
                    cursor_r,
                    cursor_g,
                    cursor_b,
                );
            }
            _ => {
                self.render_pointer_cursor(
                    &mut img,
                    px_x,
                    px_y,
                    cursor_scale,
                    cursor_r,
                    cursor_g,
                    cursor_b,
                    "pointer",
                );
            }
        }

        img
    }

    fn render_pointer_cursor(
        &self,
        img: &mut RgbaImage,
        x: i32,
        y: i32,
        scale: f32,
        r: u8,
        g: u8,
        b: u8,
        style: &str,
    ) {
        // Pointer cursor shape matching SVG: M5,3 L5,19 L9,15 L12,21 L14.5,20 L11.5,14 L17,14 Z
        // Translated so tip is at origin (0,0)
        let points = [
            Point::new(x, y),
            Point::new(x, y + (16.0 * scale) as i32),
            Point::new(x + (4.0 * scale) as i32, y + (12.0 * scale) as i32),
            Point::new(x + (7.0 * scale) as i32, y + (18.0 * scale) as i32),
            Point::new(x + (9.5 * scale) as i32, y + (17.0 * scale) as i32),
            Point::new(x + (6.5 * scale) as i32, y + (11.0 * scale) as i32),
            Point::new(x + (12.0 * scale) as i32, y + (11.0 * scale) as i32),
        ];

        match style {
            "filled" | "pointer" => {
                // Filled arrow
                draw_polygon_mut(img, &points, Rgba([r, g, b, 255]));
                // Black outline
                for i in 0..points.len() {
                    let p1 = points[i];
                    let p2 = points[(i + 1) % points.len()];
                    draw_antialiased_line_segment_mut(
                        img,
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
            }
            "outline" => {
                // Just outline, no fill
                for i in 0..points.len() {
                    let p1 = points[i];
                    let p2 = points[(i + 1) % points.len()];
                    draw_antialiased_line_segment_mut(
                        img,
                        (p1.x, p1.y),
                        (p2.x, p2.y),
                        Rgba([r, g, b, 255]),
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
            }
            "dotted" => {
                // Dotted outline
                for i in 0..points.len() {
                    let p1 = points[i];
                    let p2 = points[(i + 1) % points.len()];
                    // Draw dots along the line
                    let len = ((p2.x - p1.x).pow(2) as f32 + (p2.y - p1.y).pow(2) as f32).sqrt();
                    let num_dots = (len / 4.0) as i32;
                    for d in 0..num_dots {
                        let t = d as f32 / num_dots as f32;
                        let dot_x = (p1.x as f32 + t * (p2.x - p1.x) as f32) as i32;
                        let dot_y = (p1.y as f32 + t * (p2.y - p1.y) as f32) as i32;
                        if dot_x >= 0
                            && dot_x < img.width() as i32
                            && dot_y >= 0
                            && dot_y < img.height() as i32
                        {
                            img.put_pixel(dot_x as u32, dot_y as u32, Rgba([r, g, b, 255]));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn render_circle_cursor(
        &self,
        img: &mut RgbaImage,
        x: i32,
        y: i32,
        scale: f32,
        r: u8,
        g: u8,
        b: u8,
    ) {
        let radius = (10.0 * scale) as i32;

        // Semi-transparent fill
        draw_filled_circle_mut(img, (x, y), radius, Rgba([128, 128, 128, 200]));

        // Colored outline
        draw_hollow_circle_mut(img, (x, y), radius, Rgba([r, g, b, 255]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spring_physics() {
        let settings = CursorSettings {
            spring_tension: Some(170.0),
            spring_friction: Some(30.0),
            spring_mass: Some(1.0),
            enabled: None,
            size: None,
            highlight_clicks: None,
            smoothing: None,
            style: None,
            always_use_pointer: None,
            color: None,
            highlight_color: None,
            ripple_color: None,
            shadow_intensity: None,
            trail_enabled: None,
            trail_length: None,
            trail_opacity: None,
            click_effect: None,
            speed_preset: None,
            rotation: None,
            rotate_while_moving: None,
            rotation_intensity: None,
            hide_when_idle: None,
            idle_timeout: None,
            stop_at_end: None,
            stop_duration_ms: None,
            loop_to_start: None,
            loop_duration_ms: None,
        };
        let config = get_spring_config(&settings);
        assert_eq!(config.tension, 170.0);
        assert_eq!(config.friction, 30.0);
        let state = spring_step(0.0, 100.0, 0.0, &config, 0.016);
        assert!(state.value > 0.0);
        assert!(state.velocity > 0.0);
    }

    #[test]
    fn test_hex_color_parsing() {
        let (r, g, b) = parse_hex_rgb("#ff6b6b");
        assert_eq!(r, 255);
        assert_eq!(g, 107);
        assert_eq!(b, 107);
    }
}
