//! Scalar press animation. Its millisecond stepping and settling tolerance are
//! independent from logical-pixel path planning.
use super::{SpringConfig, SpringState};

pub fn advance(
    mut state: SpringState,
    target: f64,
    config: &SpringConfig,
    elapsed_ms: f64,
) -> SpringState {
    if !elapsed_ms.is_finite() || elapsed_ms <= 0.0 {
        return state;
    }
    if config.mass == 0.0 {
        return SpringState {
            value: target,
            velocity: 0.0,
        };
    }
    if ![
        state.value,
        state.velocity,
        target,
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
        return state;
    }
    let distance = target - state.value;
    let damping = if state.velocity * distance > 0.0
        && state.velocity.abs() > distance.abs() * (config.tension / config.mass).sqrt()
    {
        config
            .friction
            .max(2.0 * (config.mass * config.tension).sqrt())
    } else {
        config.friction
    };
    // The fractional boundary uses the distance to the next integer millisecond.
    // Keep this convention local: other motion solvers use elapsed-time remainder.
    for tick in 1..=elapsed_ms.ceil() as u64 {
        let dt = if tick as f64 > elapsed_ms {
            (tick as f64 - elapsed_ms) / 1000.0
        } else {
            0.001
        };
        state.velocity += (-config.tension * (state.value - target) - damping * state.velocity)
            / config.mass
            * dt;
        state.value += state.velocity * dt;
        if (state.value - target).abs() < 0.002 && state.velocity.abs() < 0.002 {
            return SpringState {
                value: target,
                velocity: 0.0,
            };
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_and_zero_elapsed_preserve_frame_boundaries() {
        let initial = SpringState {
            value: 0.0,
            velocity: 0.0,
        };
        let raw = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 0.0,
        };
        assert_eq!(advance(initial, 0.1, &raw, 0.0).value, 0.0);
        let pressed = advance(initial, 0.1, &raw, 1.0);
        assert_eq!(pressed.value, 0.1);
        assert_eq!(pressed.velocity, 0.0);
    }

    #[test]
    #[ignore = "requires external scalar animation fixtures"]
    fn scalar_press_matches_external_samples() {
        #[derive(serde::Deserialize)]
        struct Config {
            stiffness: f64,
            damping: f64,
            mass: f64,
        }
        #[derive(serde::Deserialize)]
        struct Sample {
            target: f64,
            value: f64,
            velocity: f64,
        }
        #[derive(serde::Deserialize)]
        struct Case {
            config: Config,
            dt: f64,
            initial: f64,
            samples: Vec<Sample>,
        }
        let path = std::env::var("TARANTINO_PRESS_SPRING_FIXTURES").unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut prior_error = 0.0_f64;
        for case in cases {
            let config = SpringConfig {
                tension: case.config.stiffness,
                friction: case.config.damping,
                mass: case.config.mass,
            };
            let mut state = SpringState {
                value: 1.0 - case.initial,
                velocity: 0.0,
            };
            let mut prior = state;
            for sample in case.samples {
                prior = super::super::spring_step(
                    prior.value,
                    1.0 - sample.target,
                    prior.velocity,
                    &config,
                    case.dt / 1000.0,
                );
                prior_error = prior_error.max((1.0 - prior.value - sample.value).abs());
                state = advance(state, 1.0 - sample.target, &config, case.dt);
                assert!((1.0 - state.value - sample.value).abs() < 1e-9);
                assert!((-state.velocity - sample.velocity).abs() < 1e-9);
            }
        }
        eprintln!("Prior scalar scale maximum difference: {prior_error}");
    }
}
