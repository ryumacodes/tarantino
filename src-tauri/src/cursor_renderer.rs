//! Cursor Renderer - Generates cursor overlay frames for video export
//!
//! Deterministic source-time motion shared with preview sampling. Final shader
//! appearance still depends on each renderer and requires separate validation.

mod simulation;
pub(crate) mod image_geometry;
pub(crate) mod recorded_images;
mod preset_hand;
pub(crate) mod shape_events;
pub(crate) mod shape_track;
pub(crate) use simulation::simulate_cursor_frames_from;
pub(crate) mod typing_activity;
pub(crate) use simulation::{simulate_cursor_frames_with_activity, simulate_cursor_range_with_activity};
mod spring;
mod press_spring;
mod press_animation;
mod edited_press;
mod angular_spring;
mod edited_path;
pub(crate) mod edited_simulation;
pub(crate) mod loop_path;
pub(crate) mod rotation;
pub use spring::spring_step;
pub(crate) use spring::spring_step_with_precision;
mod settling;
pub(crate) mod planner;
pub(crate) use settling::TargetTolerance;

pub use simulation::{
    CursorFrameState, TrailPoint, parse_cursor_events, simulate_cursor_frames,
    simulate_cursor_positions_for_range, simulate_cursor_frames_with_rotation,
    simulate_cursor_positions_for_range_with_rotation,
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
    pub is_release: bool,
    pub timestamp_ms: u64,
    pub x: f64, // Normalized 0-1
    pub y: f64, // Normalized 0-1
    pub is_click: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spring_physics() {
        let config = SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        };
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

#[cfg(test)]
mod benchmark;
