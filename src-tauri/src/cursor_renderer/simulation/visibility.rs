use crate::cursor_renderer::{SpringConfig, SpringState, spring_step};

/// One opacity spring retains momentum when the visibility reason changes.
pub(super) struct CursorVisibility {
    state: SpringState,
    blur: SpringState,
    started: bool,
}
impl CursorVisibility {
    pub(super) fn new() -> Self {
        Self { state: SpringState { value: 0.0, velocity: 0.0 }, blur: SpringState { value: 0.0, velocity: 0.0 }, started: false }
    }
    pub(super) fn sample(&mut self, has_pointer: bool, idle: bool, typing: bool, dt: f64) -> f64 {
        if !has_pointer { self.blur = SpringState { value: 0.0, velocity: 0.0 }; return 0.0; }
        let target = if idle || typing { 0.0 } else { 1.0 };
        let blur_target = if idle || typing { 10.0 } else { 0.0 };
        if !self.started {
            self.started = true;
            self.state = SpringState { value: target, velocity: 0.0 };
            self.blur = SpringState { value: blur_target, velocity: 0.0 };
        }
        let ordinary = SpringConfig { tension: 340.0, friction: 60.0, mass: 3.0 };
        self.blur = spring_step(self.blur.value, blur_target, self.blur.velocity, &ordinary, dt);
        let config = if typing {
            SpringConfig { tension: 600.0, friction: 50.0, mass: 1.0 }
        } else {
            SpringConfig { tension: 340.0, friction: 60.0, mass: 3.0 }
        };
        self.state = spring_step(self.state.value, target, self.state.velocity, &config, dt);
        self.state.value.clamp(0.0, 1.0)
    }
    /// Blur radius in stage pixels; independent of the opacity spring.
    pub(super) fn blur_radius(&self) -> f32 { self.blur.value.max(0.0) as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blur_has_independent_timing_and_preserves_momentum_between_hide_reasons() {
        let mut typing=CursorVisibility::new();let mut idle=CursorVisibility::new();
        typing.sample(true,false,false,1.0/120.0);idle.sample(true,false,false,1.0/120.0);
        let mut opacity=1.0;
        for _ in 0..12 {
            opacity=typing.sample(true,false,true,1.0/120.0);
            idle.sample(true,true,false,1.0/120.0);
        }
        // Analytical ordinary-spring step at 100ms, independently computed.
        assert!((typing.blur_radius()-2.966159).abs()<0.04);
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
        // Analytical damped oscillator with x(0)=1, v(0)=0, target=0.
        let frequency=(340.0_f64/3.0-100.0).sqrt();
        let expected=(-1.0_f64).exp()*((frequency*0.1).cos()+10.0/frequency*(frequency*0.1).sin());
        assert!((opacity-expected).abs()<0.004,"{opacity} != {expected}");
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
