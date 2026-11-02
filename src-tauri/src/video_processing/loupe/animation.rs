//! Fixed source-time simulation; output cadence only selects completed samples.
use super::{
    movement_spring,
    timeline::{LoupeRange, LoupeTimeline},
    LoupeTarget, SourceCrop,
};
use crate::cursor_renderer::{spring_step_with_precision, SpringConfig, SpringState, TargetTolerance};
use anyhow::{bail, Result};

pub const SIMULATION_FPS: f64 = 120.0;

fn empty() -> LoupeTarget {
    LoupeTarget {
        center_x: 0.5,
        center_y: 0.5,
        radius: 0.0,
        zoom: 1.0,
        bevel: 0.0,
        chromatic_aberration: 0.0,
        shadow_alpha: 0.0,
        presence: 0.0,
    }
}

fn channels(target: LoupeTarget) -> [f64; 8] {
    [
        target.center_x,
        target.center_y,
        target.radius,
        target.zoom,
        target.bevel,
        target.chromatic_aberration,
        target.shadow_alpha,
        target.presence,
    ]
}

fn frame(state: &[SpringState; 8]) -> LoupeTarget {
    LoupeTarget {
        center_x: state[0].value,
        center_y: state[1].value,
        radius: state[2].value.max(0.0),
        zoom: state[3].value.max(0.05),
        bevel: state[4].value.max(0.0),
        chromatic_aberration: state[5].value.max(0.0),
        shadow_alpha: state[6].value.clamp(0.0, 1.0),
        presence: state[7].value.clamp(0.0, 1.0),
    }
}

pub fn simulate(
    ranges: &[LoupeRange],
    source: (u32, u32),
    crop: SourceCrop,
    duration_ms: u64,
    output_fps: f64,
    lead_in_ms: u64,
    screen_spring: SpringConfig,
    mut focus_at: impl FnMut(usize, f64) -> (f64, f64),
) -> Result<Vec<LoupeTarget>> {
    if !output_fps.is_finite()
        || output_fps <= 0.0
        || ![
            screen_spring.mass,
            screen_spring.tension,
            screen_spring.friction,
        ]
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0)
    {
        bail!("Invalid magnifier animation cadence or spring");
    }
    crop.resolve(source.0, source.1)?;
    let timeline = LoupeTimeline::new(ranges)?;
    if duration_ms == 0 {
        return Ok(Vec::new());
    }
    let initial = timeline
        .target(source, crop, 0.0, lead_in_ms, &mut focus_at)?
        .unwrap_or_else(empty);
    let mut state = channels(initial.inactive()).map(|value| SpringState {
        value,
        velocity: 0.0,
    });
    let spring = movement_spring(screen_spring);
    let mut tolerances = [TargetTolerance::default(); 8];
    let count = (duration_ms as f64 * SIMULATION_FPS / 1000.0).ceil() as usize;
    let mut frames = Vec::with_capacity(count);
    for index in 0..count {
        let time = index as f64 * 1000.0 / SIMULATION_FPS;
        let target = timeline
            .target(source, crop, time, lead_in_ms, &mut focus_at)?
            .unwrap_or_else(empty);
        let snap = timeline.snaps_at(time);
        for ((state, tolerance), target) in state.iter_mut().zip(&mut tolerances).zip(channels(target)) {
            let precision = tolerance.update(state.value, target);
            *state = if snap {
                SpringState {
                    value: target,
                    velocity: 0.0,
                }
            } else {
                spring_step_with_precision(
                    state.value,
                    target,
                    state.velocity,
                    &spring,
                    1.0 / SIMULATION_FPS,
                    precision,
                )
            };
        }
        frames.push(frame(&state));
    }
    if output_fps == SIMULATION_FPS {
        return Ok(frames);
    }
    let output_count = (duration_ms as f64 * output_fps / 1000.0).ceil() as usize;
    Ok((0..output_count)
        .map(|index| {
            let source_index = (index as f64 * SIMULATION_FPS / output_fps + 1e-7).floor() as usize;
            frames[source_index.min(frames.len() - 1)]
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::super::LoupeOptions;
    use super::*;
    fn range() -> LoupeRange {
        LoupeRange {
            start_ms: 500,
            end_ms: 1500,
            disabled: false,
            instant: false,
            options: Some(LoupeOptions {
                radius: 0.25,
                zoom: 2.0,
                glass: true,
                bevel: 0.1,
                chromatic_aberration: 0.01,
                shadow_alpha: 0.35,
            }),
        }
    }
    fn crop() -> SourceCrop {
        SourceCrop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
    fn spring() -> SpringConfig {
        SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        }
    }
    fn run(ranges: &[LoupeRange], fps: f64) -> Vec<LoupeTarget> {
        simulate(
            ranges,
            (1920, 1080),
            crop(),
            3000,
            fps,
            0,
            spring(),
            |_, time| (0.3 + time / 10000.0, 0.4),
        )
        .unwrap()
    }

    #[test]
    fn export_rates_sample_the_same_optics_and_position_path() {
        let ranges = [range()];
        let baseline = run(&ranges, 120.0);
        for fps in [24.0, 30.0, 60.0, 144.0] {
            let frames = run(&ranges, fps);
            for (index, actual) in frames.iter().enumerate() {
                let source = (index as f64 * 120.0 / fps + 1e-7).floor() as usize;
                assert_eq!(*actual, baseline[source]);
            }
        }
        assert_eq!(run(&ranges, 120.0), baseline);
        assert!(baseline[120].presence > 0.99);
        assert!(baseline[359].presence < 0.001);
    }

    #[test]
    fn instant_entry_and_exit_have_no_spring_tail() {
        let frames = run(
            &[LoupeRange {
                instant: true,
                ..range()
            }],
            120.0,
        );
        assert_eq!(frames[59].presence, 0.0);
        assert_eq!((frames[60].presence, frames[60].zoom), (1.0, 2.0));
        assert_eq!((frames[181].presence, frames[181].zoom), (0.0, 1.0));
        assert_eq!(frames[187].presence, 0.0);
    }

    #[test]
    fn disabled_ranges_never_magnify_and_zero_mass_follows_targets() {
        assert!(run(
            &[LoupeRange {
                disabled: true,
                ..range()
            }],
            30.0
        )
        .iter()
        .all(|frame| frame.presence == 0.0 && frame.zoom == 1.0));
        let frames = simulate(
            &[range()],
            (1920, 1080),
            crop(),
            2000,
            120.0,
            0,
            SpringConfig {
                mass: 0.0,
                ..spring()
            },
            |_, _| (0.3, 0.4),
        )
        .unwrap();
        assert_eq!(frames[60].zoom, 2.0);
        assert_eq!(frames[181].zoom, 1.0);
    }

    #[test]
    fn invalid_configuration_is_rejected_before_allocating_frames() {
        for fps in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                simulate(&[], (100, 100), crop(), 1000, fps, 0, spring(), |_, _| (
                    0.5, 0.5
                ))
                .is_err()
            );
        }
        assert!(simulate(
            &[],
            (100, 100),
            crop(),
            1000,
            30.0,
            0,
            SpringConfig {
                friction: -1.0,
                ..spring()
            },
            |_, _| (0.5, 0.5)
        )
        .is_err());
    }
}
