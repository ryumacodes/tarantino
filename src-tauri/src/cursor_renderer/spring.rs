use super::{SpringConfig, SpringState};

/// Seconds-based damped motion. Keep aligned with utils/motion/spring.ts.
pub fn spring_step(
    current: f64,
    target: f64,
    velocity: f64,
    config: &SpringConfig,
    dt: f64,
) -> SpringState {
    spring_step_with_precision(current, target, velocity, config, dt, 0.0001)
}

/// Integration with a tolerance retained by the caller for the current target.
pub(crate) fn spring_step_with_precision(
    mut current: f64, target: f64, mut velocity: f64,
    config: &SpringConfig, dt: f64, precision: f64,
) -> SpringState {
    if !dt.is_finite() || dt <= 0.0 {
        return SpringState {
            value: current,
            velocity,
        };
    }
    if config.mass == 0.0 {
        return SpringState {
            value: target,
            velocity: 0.0,
        };
    }
    if ![
        current,
        target,
        velocity,
        config.tension,
        config.friction,
        config.mass,
    ]
    .iter()
    .all(|v| v.is_finite())
        || config.tension <= 0.0
        || config.friction < 0.0
        || config.mass < 0.0
    {
        return SpringState {
            value: current,
            velocity,
        };
    }
    // Increase braking only for fast motion toward the target; preserve reversals.
    let distance = target - current;
    let friction = if velocity * distance > 0.0
        && velocity.abs() > distance.abs() * (config.tension / config.mass).sqrt()
    {
        config
            .friction
            .max(2.0 * (config.tension * config.mass).sqrt())
    } else {
        config.friction
    };
    let max_step = 0.001_f64.min(
        0.25 / (config.tension / config.mass)
            .sqrt()
            .max(friction / config.mass),
    );
    let mut remaining = dt;
    while remaining > 1e-12 {
        let step = max_step.min(remaining);
        velocity +=
            (-config.tension * (current - target) - friction * velocity) / config.mass * step;
        current += velocity * step;
        if (current - target).abs() < precision && velocity.abs() < precision {
            return SpringState {
                value: target,
                velocity: 0.0,
            };
        }
        remaining -= step;
    }
    SpringState {
        value: current,
        velocity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FOLLOW: SpringConfig = SpringConfig {
        tension: 520.0,
        friction: 52.0,
        mass: 1.0,
    };
    #[test]
    fn adaptive_braking_preserves_direction_and_existing_damping() {
        let soft = SpringConfig {
            tension: 100.0,
            friction: 2.0,
            mass: 1.0,
        };
        let critical = SpringConfig {
            friction: 20.0,
            ..soft
        };
        for fps in [24.0, 30.0, 60.0, 120.0] {
            let adaptive = spring_step(0.9, 1.0, 2.0, &soft, 1.0 / fps);
            let expected = spring_step(0.9, 1.0, 2.0, &critical, 1.0 / fps);
            assert_eq!(
                (adaptive.value, adaptive.velocity),
                (expected.value, expected.velocity)
            );
        }
        let reversed = spring_step(0.9, 0.0, 2.0, &soft, 0.001);
        assert!((reversed.velocity - 1.906).abs() < 1e-12);
        assert!((reversed.value - 0.901906).abs() < 1e-12);
        let slow = spring_step(0.0, 1.0, 1.0, &soft, 0.001);
        assert!((slow.velocity - 1.098).abs() < 1e-12);
        let overdamped = spring_step(
            0.9,
            1.0,
            2.0,
            &SpringConfig {
                friction: 30.0,
                ..soft
            },
            0.001,
        );
        assert!((overdamped.velocity - 1.95).abs() < 1e-12);
    }

    #[test]
    fn follow_stays_bounded_at_all_export_rates() {
        for fps in [24, 30, 60, 120] {
            let mut state = SpringState::default();
            for _ in 0..fps * 2 {
                state = spring_step(state.value, 1.0, state.velocity, &FOLLOW, 1.0 / fps as f64);
                assert!(
                    (0.0..=1.0).contains(&state.value),
                    "fps={fps} state={state:?}"
                );
            }
            assert!((state.value - 1.0).abs() < 0.0001);
            assert!(state.velocity.abs() < 0.0001);
        }
    }
    #[test]
    fn matches_critical_damping_solution() {
        let config = SpringConfig {
            tension: 100.0,
            friction: 20.0,
            mass: 1.0,
        };
        let state = spring_step(0.0, 1.0, 0.0, &config, 0.5);
        assert!((state.value - (1.0 - 6.0 * (-5.0_f64).exp())).abs() < 0.001);
        assert!((state.velocity - 50.0 * (-5.0_f64).exp()).abs() < 0.003);
    }
    #[test]
    fn long_frames_preserve_elapsed_time() {
        let mut regular = SpringState::default();
        for _ in 0..30 {
            regular = spring_step(regular.value, 1.0, regular.velocity, &FOLLOW, 1.0 / 60.0);
        }
        let mut irregular = SpringState::default();
        for dt in [0.016, 0.1, 0.024, 0.2, 0.16] {
            irregular = spring_step(irregular.value, 1.0, irregular.velocity, &FOLLOW, dt);
        }
        assert!((regular.value - irregular.value).abs() < 0.0001);
        assert!((regular.velocity - irregular.velocity).abs() < 0.001);
    }
}
