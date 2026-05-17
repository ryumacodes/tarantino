use crate::cursor_engine::{ZoomEasing, ZoomReason};

/// Rule for determining zoom behavior
#[derive(Debug, Clone)]
pub(super) struct ZoomRule {
    pub(super) name: String,
    pub(super) priority: u32,
    pub(super) conditions: ZoomConditions,
    pub(super) zoom_action: ZoomAction,
}

/// Conditions that trigger zoom
#[derive(Debug, Clone)]
pub(super) struct ZoomConditions {
    // Pattern requirements
    pub(super) requires_hovering: bool,
    pub(super) requires_clicking: bool,
    pub(super) requires_precise_work: bool,
    pub(super) requires_reading: bool,
    pub(super) requires_navigating: bool,

    // Motion requirements
    pub(super) max_velocity: Option<f64>,
    pub(super) min_velocity: Option<f64>,
    pub(super) max_acceleration: Option<f64>,
    pub(super) min_stability_score: Option<f32>,
    pub(super) min_dwell_duration_ms: Option<u64>,

    // Context requirements
    pub(super) min_distance_from_edge: Option<f64>, // pixels from screen edge
}

/// Action to take when rule triggers
#[derive(Debug, Clone)]
pub(super) struct ZoomAction {
    pub(super) zoom_factor: f32,
    pub(super) duration_ms: u64,
    pub(super) easing: ZoomEasing,
    pub(super) reason: ZoomReason,
    pub(super) confidence: f32,

    // Focus point calculation
    pub(super) focus_strategy: FocusStrategy,

    // Priority handling
    pub(super) can_interrupt_existing: bool,
}

/// Strategy for calculating zoom focus point
#[derive(Debug, Clone)]
pub(super) enum FocusStrategy {
    /// Focus on current cursor position
    Cursor,
    /// Focus on predicted cursor position
    PredictedCursor,
    /// Focus on center of recent activity
    ActivityCenter,
    /// Focus on optimal point for detected pattern
    PatternOptimal,
}

/// Active zoom session
#[derive(Debug, Clone)]
pub(super) struct ActiveZoom {
    pub(super) id: String,
    pub(super) start_time_ms: u64,
    pub(super) end_time_ms: u64,
    pub(super) focus_x: f32,
    pub(super) focus_y: f32,
    pub(super) zoom_factor: f32,
    pub(super) reason: ZoomReason,
    pub(super) priority: u32,
}

impl Default for ZoomConditions {
    fn default() -> Self {
        Self {
            requires_hovering: false,
            requires_clicking: false,
            requires_precise_work: false,
            requires_reading: false,
            requires_navigating: false,
            max_velocity: None,
            min_velocity: None,
            max_acceleration: None,
            min_stability_score: None,
            min_dwell_duration_ms: None,
            min_distance_from_edge: None,
        }
    }
}
