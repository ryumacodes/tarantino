//! Cursor Renderer - Generates cursor overlay frames for video export
//!
//! Ports the spring physics and cursor rendering logic from MouseCursorOverlay.tsx
//! to produce frame-by-frame cursor animations that match the preview exactly.

mod simulation;

pub use simulation::{CursorFrameState, parse_cursor_events, simulate_cursor_positions};

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
