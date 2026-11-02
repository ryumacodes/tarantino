/// Keep the stopping threshold fixed during an approach. Recomputing it from
/// the shrinking remaining distance makes large movements settle too late.
#[derive(Clone, Copy, Default)]
pub(crate) struct TargetTolerance {
    target: Option<f64>,
    precision: f64,
}

impl TargetTolerance {
    pub(crate) fn update(&mut self, current: f64, target: f64) -> f64 {
        if self.target != Some(target) {
            self.target = Some(target);
            self.precision = if current == target { 1.0 } else {
                (target - current).abs().max(1.0) * 0.002
            };
        }
        self.precision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor_renderer::{spring_step_with_precision, SpringConfig, SpringState};

    #[test]
    fn threshold_tracks_target_changes_not_remaining_distance() {
        let mut tolerance = TargetTolerance::default();
        assert_eq!(tolerance.update(0.0, 1000.0), 2.0);
        assert_eq!(tolerance.update(999.0, 1000.0), 2.0);
        assert_eq!(tolerance.update(999.0, 0.0), 1.998);
        assert_eq!(tolerance.update(0.0, 0.25), 0.002);
    }

    #[test]
    fn scaled_movements_settle_at_the_same_time() {
        let config = SpringConfig { tension:470.0, friction:70.0, mass:3.0 };
        let simulate = |distance: f64| {
            let mut tolerance = TargetTolerance::default();
            let mut state = SpringState::default();
            for frame in 1..=240 {
                state = spring_step_with_precision(state.value, distance, state.velocity,
                    &config, 1.0 / 120.0, tolerance.update(state.value, distance));
                if state.value == distance && state.velocity == 0.0 { return frame; }
            }
            panic!("Camera failed to settle");
        };
        assert_eq!(simulate(1.0), simulate(1000.0));
        assert_eq!(simulate(1.0), 96);
    }
}
