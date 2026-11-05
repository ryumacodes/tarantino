//! Angular settling uses radians so precision is independent of display units.

pub(super) struct AngularSpring {
    value: f64,
    velocity: f64,
    target: f64,
    precision: f64,
}

impl AngularSpring {
    pub(super) fn new(degrees: f64) -> Self {
        let value=degrees.to_radians();
        Self {value,velocity:0.0,target:value,precision:0.002}
    }

    pub(super) fn advance(&mut self, degrees: f64, dt: f64) -> f64 {
        if !degrees.is_finite() || !dt.is_finite() || dt <= 0.0 {
            return self.value.to_degrees();
        }
        let target=degrees.to_radians();
        if target != self.target {
            self.precision=(target-self.value).abs().max(1.0)*0.002;
            self.target=target;
        }
        if self.value==target && self.velocity==0.0 { return degrees; }
        let distance=target-self.value;
        let damping=if self.velocity*distance>0.0 && self.velocity.abs()>distance.abs()*120.0_f64.sqrt() {
            2.0*120.0_f64.sqrt()
        } else {8.0};
        let milliseconds=dt*1000.0;
        let ticks=milliseconds.ceil() as usize;
        for tick in 1..=ticks {
            // Preserve the fractional tick convention of the animation clock:
            // its final tick uses the distance to the next whole millisecond.
            let step=if tick as f64>milliseconds {(tick as f64-milliseconds)/1000.0} else {0.001};
            self.velocity+=(120.0*(target-self.value)-damping*self.velocity)*step;
            self.value+=self.velocity*step;
            if self.velocity.abs()<self.precision && (self.value-target).abs()<self.precision {
                self.value=target;
                self.velocity=0.0;
                break;
            }
        }
        self.value.to_degrees()
    }
}

pub(super) struct AngularTimeline {
    spring: AngularSpring,
    next_frame: usize,
    lower: f64,
    upper: f64,
}
impl AngularTimeline {
    pub(super) fn new(initial: f64) -> Self {
        Self { spring: AngularSpring::new(initial), next_frame: 0, lower: initial, upper: initial }
    }

    pub(super) fn sample(&mut self, time_ms: f64, target: impl Fn(f64) -> f64) -> f64 {
        let position = time_ms * 60.0 / 1000.0;
        let position = if (position-position.round()).abs()<1e-7 {position.round()} else {position};
        while self.next_frame <= position.ceil() as usize {
            self.lower = self.upper;
            self.upper = self.spring.advance(target(self.next_frame as f64*1000.0/60.0),
                if self.next_frame==0 {0.0} else {1.0/60.0});
            self.next_frame += 1;
        }
        let fraction = position-position.floor();
        if fraction==0.0 {self.upper} else {self.lower+(self.upper-self.lower)*fraction}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_step_follows_radian_acceleration_and_velocity() {
        let mut spring=AngularSpring::new(0.0);
        let first=spring.advance(20.0,0.001);
        assert!((first-0.0024).abs()<1e-12);
        let second=spring.advance(20.0,0.001);
        assert!((second-0.007180512).abs()<1e-12);
    }
    #[test]
    fn small_angular_residual_settles_exactly_and_new_target_wakes_it() {
        let mut spring=AngularSpring::new(0.0);
        assert_eq!(spring.advance(0.05,0.001),0.05);
        assert_eq!(spring.velocity,0.0);
        let moving=spring.advance(20.0,1.0/120.0);
        assert!(moving>0.05 && moving<20.0);
        for _ in 0..600 { spring.advance(20.0,1.0/120.0); }
        assert_eq!(spring.value,20.0_f64.to_radians());
        assert_eq!(spring.velocity,0.0);
        assert!(spring.advance(-10.0,1.0/120.0)<20.0);
    }
    #[test]
    fn fractional_frame_interval_preserves_clock_tick_convention() {
        let mut spring=AngularSpring::new(0.0);
        let actual=spring.advance(20.0,1.0/120.0);
        let mut position=0.0;
        let mut velocity=0.0;
        for step in [0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,2.0/3000.0] {
            velocity+=(120.0*(20.0-position)-8.0*velocity)*step;
            position+=velocity*step;
        }
        assert!((actual-position).abs()<1e-12);
    }
    #[test]
    fn base_angle_does_not_change_motion_or_precision() {
        let mut zero=AngularSpring::new(0.0);
        let mut offset=AngularSpring::new(45.0);
        for index in 0..240 {
            let target=if index<120 {12.0} else {-8.0};
            assert!((zero.advance(target,1.0/120.0)+45.0-offset.advance(target+45.0,1.0/120.0)).abs()<1e-10);
        }
    }
}
