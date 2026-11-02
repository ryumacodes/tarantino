//! Logical-pixel motion planning primitives. Keep coordinates unnormalized so
//! synchronization and settling tolerances do not depend on recording size.
use super::SpringConfig;
use anyhow::{bail, Result};
pub mod activity;
pub mod approach;
pub mod optimize;
pub mod plan;
pub mod input;
pub mod playback;
pub mod press;
pub(crate) mod press_window;
pub(crate) mod source_window;
pub mod runtime;
pub mod timing;

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct Point {
    pub time_ms: f64,
    pub x: f64,
    pub y: f64,
}

pub struct SourcePath {
    points: Vec<Point>,
    activity: activity::MotionIntervals,
}

impl SourcePath {
    pub fn new(mut points: Vec<Point>) -> Result<Self> {
        if points
            .iter()
            .any(|p| ![p.time_ms, p.x, p.y].iter().all(|v| v.is_finite()))
        {
            bail!("Cursor path contains nonfinite coordinates or time");
        }
        points.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        let activity = activity::MotionIntervals::new(&points);
        Ok(Self { points, activity })
    }

    pub fn start(&self) -> f64 {
        self.points.first().map_or(0.0, |p| p.time_ms)
    }

    pub fn activity(&self) -> &activity::MotionIntervals {
        &self.activity
    }

    pub fn at(&self, time: f64) -> [f64; 2] {
        let end = self.points.partition_point(|p| p.time_ms <= time);
        let Some(previous) = self.points.get(end.saturating_sub(1)) else {
            return [0.0; 2];
        };
        if end == 0 {
            return [previous.x, previous.y];
        }
        let Some(next) = self.points.get(end) else {
            return [previous.x, previous.y];
        };
        let span = next.time_ms - previous.time_ms;
        // Missing samples represent a resting pointer, not slow travel over
        // the whole missing interval. Continuous sampled motion interpolates.
        if span > 40.0 || span <= 0.0 {
            return [previous.x, previous.y];
        }
        let fraction = (time - previous.time_ms) / span;
        [
            previous.x + (next.x - previous.x) * fraction,
            previous.y + (next.y - previous.y) * fraction,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub position: [f64; 2],
    pub velocity: [f64; 2],
}

impl State {
    pub fn resting(position: [f64; 2]) -> Self {
        Self {
            position,
            velocity: [0.0; 2],
        }
    }
}

pub struct Solver<'a> {
    path: &'a SourcePath,
    spring: SpringConfig,
    instant: &'a [(f64, f64)],
}

impl<'a> Solver<'a> {
    pub fn new(
        path: &'a SourcePath,
        spring: SpringConfig,
        instant: &'a [(f64, f64)],
    ) -> Result<Self> {
        if ![spring.tension, spring.friction, spring.mass]
            .iter()
            .all(|v| v.is_finite())
            || spring.tension <= 0.0
            || spring.friction < 0.0
            || spring.mass < 0.0
        {
            bail!("Invalid cursor planning spring");
        }
        if instant
            .iter()
            .any(|&(a, b)| a.is_nan() || b.is_nan() || a >= b)
        {
            bail!("Invalid instant cursor interval");
        }
        Ok(Self {
            path,
            spring,
            instant,
        })
    }

    /// Lead is measured in playback milliseconds; speed scales the spring,
    /// not the recording clock. Observe completed integer-millisecond states.
    pub fn advance(
        &self,
        from: f64,
        to: f64,
        mut state: State,
        lead: &impl Fn(f64) -> f64,
        speed: &impl Fn(f64) -> f64,
        mut observe: impl FnMut(f64, State),
    ) -> State {
        let mut time = from;
        let mut bucket = f64::NAN;
        let (mut stiffness, mut damping, mut frequency, mut braking) = (0.0, 0.0, 0.0, 0.0);
        while time < to {
            let next = (time.floor() + 1.0).min(to);
            let target = self.path.at(next + lead(next));
            if self.spring.mass == 0.0 || self.instant.iter().any(|&(a, b)| next >= a && next < b) {
                state = State::resting(target);
            } else if state.position != target || state.velocity != [0.0; 2] {
                let current_bucket = (next / 4.0).floor() * 4.0;
                if current_bucket != bucket {
                    bucket = current_bucket;
                    let rate = speed(bucket);
                    stiffness = self.spring.tension * rate * rate;
                    damping = self.spring.friction * rate;
                    frequency = (stiffness / self.spring.mass).sqrt();
                    braking = damping.max(2.0 * (stiffness * self.spring.mass).sqrt());
                }
                let dt = (next - time) / 1000.0;
                for axis in 0..2 {
                    let remaining = target[axis] - state.position[axis];
                    let velocity = state.velocity[axis];
                    let friction = if velocity * remaining > 0.0
                        && velocity.abs() > remaining.abs() * frequency
                    {
                        braking
                    } else {
                        damping
                    };
                    state.velocity[axis] +=
                        (remaining * stiffness - velocity * friction) / self.spring.mass * dt;
                    state.position[axis] += state.velocity[axis] * dt;
                    if state.velocity[axis].abs() < 0.01
                        && (state.position[axis] - target[axis]).abs() < 0.01
                    {
                        state.position[axis] = target[axis];
                        state.velocity[axis] = 0.0;
                    }
                }
            }
            time = next;
            if time == time.floor() {
                observe(time, state);
            }
        }
        state
    }

    pub fn trajectory(
        &self,
        end: f64,
        lead: &impl Fn(f64) -> f64,
        speed: &impl Fn(f64) -> f64,
    ) -> MotionTrack {
        let start = self.path.start().floor().min(0.0);
        let initial = self.path.at(start + lead(start));
        let mut track = MotionTrack {
            initial,
            runs: Vec::new(),
        };
        let mut previous = initial;
        let mut run: Option<Run> = None;
        self.advance(
            start,
            end.ceil().max(start),
            State::resting(initial),
            lead,
            speed,
            |time, state| {
                if state.position == previous {
                    if let Some(done) = run.take() {
                        track.runs.push(done);
                    }
                } else {
                    run.get_or_insert_with(|| Run {
                        start: time,
                        positions: Vec::new(),
                    })
                    .positions
                    .push(state.position.map(|v| v as f32));
                    previous = state.position;
                }
            },
        );
        if let Some(done) = run {
            track.runs.push(done);
        }
        track
    }
}

struct Run {
    start: f64,
    positions: Vec<[f32; 2]>,
}
pub struct MotionTrack {
    initial: [f64; 2],
    runs: Vec<Run>,
}
impl MotionTrack {
    pub fn at(&self, time: f64) -> [f64; 2] {
        let count = self.runs.partition_point(|r| r.start <= time);
        if count == 0 {
            return self.initial;
        }
        let run = &self.runs[count - 1];
        let local = time - run.start;
        let last = run.positions.len() - 1;
        if local >= last as f64 {
            return run.positions[last].map(f64::from);
        }
        let index = local.floor() as usize;
        let fraction = local - index as f64;
        std::array::from_fn(|axis| {
            let a = run.positions[index][axis] as f64;
            a + (run.positions[index + 1][axis] as f64 - a) * fraction
        })
    }
}

#[cfg(test)]
mod tests;
