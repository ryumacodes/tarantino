use crate::cursor_engine::{EnhancedCursorEvent, ZoomReason};

pub(super) struct FocusSmoother {
    last_focus: Option<(f32, f32)>,
    smoothing_factor: f32,
    prediction_weight: f32,
}

impl FocusSmoother {
    pub(super) fn new(smoothing_factor: f32, prediction_weight: f32) -> Self {
        Self {
            last_focus: None,
            smoothing_factor: smoothing_factor.clamp(0.0, 1.0),
            prediction_weight: prediction_weight.clamp(0.0, 1.0),
        }
    }

    pub(super) fn smooth_focus(
        &mut self,
        focus_x: f32,
        focus_y: f32,
        _event: &EnhancedCursorEvent,
    ) -> (f32, f32) {
        if let Some((last_x, last_y)) = self.last_focus {
            // Apply exponential smoothing
            let smoothed_x =
                self.smoothing_factor * focus_x + (1.0 - self.smoothing_factor) * last_x;
            let smoothed_y =
                self.smoothing_factor * focus_y + (1.0 - self.smoothing_factor) * last_y;

            self.last_focus = Some((smoothed_x, smoothed_y));
            (smoothed_x, smoothed_y)
        } else {
            self.last_focus = Some((focus_x, focus_y));
            (focus_x, focus_y)
        }
    }

    pub(super) fn reset(&mut self) {
        self.last_focus = None;
    }
}

/// Current zoom state for rendering
#[derive(Debug, Clone)]
pub struct ZoomState {
    pub zoom_factor: f32,
    pub focus_x: f32,
    pub focus_y: f32,
    pub progress: f32, // 0.0 - 1.0
    pub reason: ZoomReason,
}
