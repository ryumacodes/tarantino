use crate::cursor_renderer::{SpringConfig, SpringState, TargetTolerance, step_frame_spring};

/// One opacity spring retains momentum when the visibility reason changes.
pub(super) struct CursorVisibility {
    state: SpringState,
    blur: SpringState,
    started: bool,
    opacity_tolerance: TargetTolerance,
    blur_tolerance: TargetTolerance,
}
impl CursorVisibility {
    pub(super) fn new() -> Self {
        Self { state: SpringState { value: 0.0, velocity: 0.0 }, blur: SpringState { value: 0.0, velocity: 0.0 }, started: false, opacity_tolerance: TargetTolerance::default(), blur_tolerance: TargetTolerance::default() }
    }
    pub(super) fn sample(&mut self, has_pointer: bool, idle: bool, typing: bool, dt: f64) -> f64 {
        if !has_pointer {
            self.state=SpringState {value:0.0,velocity:0.0};
            self.blur=SpringState {value:0.0,velocity:0.0};
            self.opacity_tolerance=TargetTolerance::default();
            self.blur_tolerance=TargetTolerance::default();
            return 0.0;
        }
        let target = if idle || typing { 0.0 } else { 1.0 };
        let blur_target = if idle || typing { 10.0 } else { 0.0 };
        if !self.started {
            self.started = true;
            self.state = SpringState { value: target, velocity: 0.0 };
            self.blur = SpringState { value: blur_target, velocity: 0.0 };
        }
        let ordinary = SpringConfig { tension: 340.0, friction: 60.0, mass: 3.0 };
        self.blur = step_frame_spring(self.blur, blur_target, &ordinary, dt * 1000.0,
            self.blur_tolerance.update(self.blur.value, blur_target));
        let config = if typing {
            SpringConfig { tension: 600.0, friction: 50.0, mass: 1.0 }
        } else {
            SpringConfig { tension: 340.0, friction: 60.0, mass: 3.0 }
        };
        self.state = step_frame_spring(self.state, target, &config, dt * 1000.0,
            self.opacity_tolerance.update(self.state.value, target));
        self.state.value.clamp(0.0, 1.0)
    }
    /// Blur radius in stage pixels; independent of the opacity spring.
    pub(super) fn blur_radius(&self) -> f32 { self.blur.value.max(0.0) as f32 }
}

/// Two neighboring style samples are enough for monotonically increasing playback time.
pub(in crate::cursor_renderer) struct VisibilityTimeline {
    spring: CursorVisibility,
    next_frame: usize,
    lower: [f64; 2],
    upper: [f64; 2],
}
impl VisibilityTimeline {
    pub(in crate::cursor_renderer) fn new() -> Self {
        Self { spring: CursorVisibility::new(), next_frame: 0, lower: [0.0; 2], upper: [0.0; 2] }
    }

    pub(in crate::cursor_renderer) fn sample(&mut self, time_ms: f64, targets: impl Fn(f64) -> (bool, bool, bool)) -> [f64; 2] {
        let position = time_ms * 60.0 / 1000.0;
        let position = if (position-position.round()).abs()<1e-7 {position.round()} else {position};
        let end = position.ceil() as usize;
        while self.next_frame <= end {
            let (pointer, idle, typing) = targets(self.next_frame as f64 * 1000.0 / 60.0);
            self.spring.sample(pointer, idle, typing, if self.next_frame==0 {0.0} else {1.0/60.0});
            self.lower = self.upper;
            self.upper = if pointer { [self.spring.state.value, self.spring.blur.value] } else { [0.0; 2] };
            self.next_frame += 1;
        }
        let fraction = position-position.floor();
        let value = if fraction==0.0 { self.upper } else {
            std::array::from_fn(|i| self.lower[i]+(self.upper[i]-self.lower[i])*fraction)
        };
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires external visibility spring fixtures"]
    fn visibility_matches_external_spring_samples() {
        #[derive(serde::Deserialize)]
        struct Sample { typing: bool, idle: bool, opacity: f64, blur: f64 }
        #[derive(serde::Deserialize)]
        struct Case { fps: f64, samples: Vec<Sample> }
        let path = std::env::var("TARANTINO_VISIBILITY_FIXTURES").unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let mut fade = CursorVisibility::new();
            for (index, sample) in case.samples.iter().enumerate() {
                let opacity = fade.sample(true, sample.idle, sample.typing, if index==0 {0.0} else {1.0/case.fps});
                assert!((opacity-sample.opacity.clamp(0.0,1.0)).abs()<1e-10,"{}Hz frame {index}: opacity {opacity} != {}",case.fps,sample.opacity);
                assert!((fade.blur.value-sample.blur).abs()<1e-10,"{}Hz frame {index}: blur {} != {}",case.fps,fade.blur.value,sample.blur);
            }
        }
    }

    #[test]
    fn blur_has_independent_timing_and_preserves_momentum_between_hide_reasons() {
        let mut typing=CursorVisibility::new();let mut idle=CursorVisibility::new();
        typing.sample(true,false,false,1.0/120.0);idle.sample(true,false,false,1.0/120.0);
        let mut opacity=1.0;
        for _ in 0..12 {
            opacity=typing.sample(true,false,true,1.0/120.0);
            idle.sample(true,true,false,1.0/120.0);
        }
        // Twelve frame steps at 120 Hz, with retained target precision.
        assert!((typing.blur_radius()-3.1624169).abs()<1e-6);
        assert!((typing.blur_radius()-idle.blur_radius()).abs()<1e-6);
        assert!((typing.blur_radius() as f64-10.0*(1.0-opacity)).abs()>2.0);
        let before=typing.blur_radius();
        typing.sample(true,true,false,1.0/120.0);idle.sample(true,true,false,1.0/120.0);
        assert!(typing.blur_radius()>before);
        assert_eq!(typing.blur_radius(),idle.blur_radius());
        for _ in 0..240 {typing.sample(true,false,false,1.0/120.0);}
        assert!(typing.blur_radius()<0.001);
        let mut absent=CursorVisibility::new();
        absent.sample(false,true,true,1.0/120.0);assert_eq!(absent.blur_radius(),0.0);
    }
    #[test]
    fn idle_fade_follows_damped_response_instead_of_a_linear_ramp() {
        let mut fade=CursorVisibility::new();
        fade.sample(true,false,false,1.0/120.0);
        let mut opacity=1.0;
        for _ in 0..12 {opacity=fade.sample(true,true,false,1.0/120.0);}
        assert!((opacity-0.6837583146230749).abs()<1e-12);
    }
    #[test]
    fn switching_from_typing_to_idle_does_not_flash_or_restart_the_fade() {
        let mut fade=CursorVisibility::new();
        fade.sample(true,false,false,1.0/120.0);
        for _ in 0..60 {fade.sample(true,false,true,1.0/120.0);}
        let before=fade.state.value;
        let after=fade.sample(true,true,false,1.0/120.0);
        assert!(after<=before && after<0.001);
        for _ in 0..120 {assert!(fade.sample(true,true,false,1.0/120.0)<0.001);}
        let waking=fade.sample(true,false,false,1.0/120.0);
        assert!(waking>0.0 && waking<0.02);
        for _ in 0..120 {fade.sample(true,false,false,1.0/120.0);}
        assert!(fade.state.value>0.99);
    }
}
