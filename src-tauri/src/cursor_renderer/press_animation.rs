//! Frame-based scalar animation with fractional-frame sampling.
use super::{press_spring, SpringConfig, SpringState};

pub struct PressAnimation {
    fps: f64,
    values: Vec<f64>,
}

fn snap_integer(value: f64) -> f64 {
    if (value.round() - value).abs() < 1e-7 {
        value.round()
    } else {
        value
    }
}

impl PressAnimation {
    pub fn build(
        duration: f64,
        fps: f64,
        spring: &SpringConfig,
        pressed: impl Fn(f64) -> bool,
    ) -> Option<Self> {
        Self::build_with_config(duration, fps, |time| (pressed(time), *spring))
    }

    pub fn build_with_config(
        duration: f64,
        fps: f64,
        target: impl Fn(f64) -> (bool, SpringConfig),
    ) -> Option<Self> {
        if !duration.is_finite() || duration < 0.0 || !fps.is_finite() || fps <= 0.0 {
            return None;
        }
        let mut state = SpringState {
            value: if target(0.0).0 { 0.1 } else { 0.0 },
            velocity: 0.0,
        };
        let mut values = vec![state.value];
        let mut previous = 0.0;
        // Include the next frame so any timestamp through duration can interpolate.
        for i in 1..=(duration * fps / 1000.0).ceil() as usize + 1 {
            let time = snap_integer(i as f64 * (1000.0 / fps));
            let (pressed, spring) = target(time);
            state = press_spring::advance(
                state,
                if pressed { 0.1 } else { 0.0 },
                &spring,
                time - previous,
            );
            values.push(state.value);
            previous = time;
        }
        Some(Self { fps, values })
    }

    pub fn at(&self, time: f64) -> f64 {
        let frame = snap_integer(time.max(0.0) / (1000.0 / self.fps));
        let index = (frame.floor() as usize).min(self.values.len() - 1);
        let a = self.values[index];
        let b = self.values.get(index + 1).copied().unwrap_or(a);
        a + (b - a) * (frame - frame.floor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_clock_is_rejected_and_opening_press_is_settled() {
        let config = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        assert!(PressAnimation::build(100.0, 0.0, &config, |_| false).is_none());
        let animation = PressAnimation::build(100.0, 60.0, &config, |_| true).unwrap();
        for time in [0.0, 0.1, 16.66, 50.0, 100.0] {
            assert_eq!(animation.at(time), 0.1);
        }
    }

    #[test]
    #[ignore = "requires external frame-animation fixtures"]
    fn configured_frame_rate_and_fractional_samples_match_external_animation() {
        #[derive(serde::Deserialize)]
        struct Config {
            stiffness: f64,
            damping: f64,
            mass: f64,
        }
        #[derive(serde::Deserialize)]
        struct Sample {
            time: f64,
            value: f64,
        }
        #[derive(serde::Deserialize)]
        struct Case {
            config: Config,
            fps: f64,
            samples: Vec<Sample>,
        }
        let path = std::env::var("TARANTINO_PRESS_SAMPLING_FIXTURES").unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases {
            let spring = SpringConfig {
                tension: case.config.stiffness,
                friction: case.config.damping,
                mass: case.config.mass,
            };
            let animation =
                PressAnimation::build(2000.0, case.fps, &spring, |t| (103.0..607.0).contains(&t))
                    .unwrap();
            let settings = serde_json::from_value(serde_json::json!({"enabled":true})).unwrap();
            let events = [crate::cursor_renderer::CursorEvent { is_release: false,
                timestamp_ms: 0,
                x: 0.5,
                y: 0.5,
                is_click: false,
            }];
            let pressed = |t| (103.0..607.0).contains(&t);
            let transported = crate::cursor_renderer::simulate_cursor_frames_from(
                &events,
                &spring,
                400.0,
                2000,
                &None,
                &settings,
                0,
                120.0,
                None,
                None,
                None,
                Some((&pressed, case.fps)),
                |frame| frame.press_amount,
            );
            for (frame, sample) in transported.iter().zip(&case.samples) {
                assert!((1.0 - *frame as f64 - sample.value).abs() < 1e-7);
            }
            for sample in case.samples {
                assert!((1.0 - animation.at(sample.time) - sample.value).abs() < 1e-9);
            }
        }
    }
}
