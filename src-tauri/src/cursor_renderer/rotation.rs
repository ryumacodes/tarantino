//! Rotation targets in recording pixel coordinates, independent of output FPS.
//! The target curve is separate from the angular spring used to render it.

#[derive(Clone, Copy, Debug)]
pub(crate) struct MotionPoint {
    pub time_ms: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RotationKey {
    pub time_ms: f64,
    pub degrees: f64,
}

pub(crate) fn retimed_points(points: &[MotionPoint], target_time: impl Fn(f64) -> f64) -> Vec<MotionPoint> {
    let mut drawn = Vec::with_capacity(points.len() + 1);
    if let Some(first) = points.first() { drawn.push(*first); }
    drawn.extend(points.iter().map(|point| MotionPoint {
        time_ms: target_time(point.time_ms), ..*point
    }));
    drawn.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    drawn
}

pub(crate) fn sample(keys: &[RotationKey], time_ms: f64) -> f64 {
    let right = keys.partition_point(|key| key.time_ms <= time_ms);
    if right == 0 { return keys.first().map_or(0.0, |key| key.degrees); }
    let left = keys[right - 1];
    let Some(next) = keys.get(right) else { return left.degrees; };
    let progress = ((time_ms - left.time_ms) / (next.time_ms - left.time_ms)).clamp(0.0, 1.0);
    left.degrees + (next.degrees - left.degrees) * progress
}

#[derive(Default)]
struct Keys {
    values: Vec<RotationKey>,
    hold: Option<RotationKey>,
}
impl Keys {
    fn emit(&mut self, key: RotationKey, force: bool) {
        // Suppress visually insignificant changes while retaining the end of a hold.
        if !force && self.values.last().is_some_and(|last|
            (last.degrees - key.degrees).abs().to_radians() < 0.0015) {
            self.hold = Some(key);
            return;
        }
        if let Some(hold) = self.hold.take().filter(|hold| hold.time_ms < key.time_ms) {
            self.values.push(hold);
        }
        if let Some(last) = self.values.last_mut().filter(|last| last.time_ms == key.time_ms) {
            *last = key;
        } else {
            self.values.push(key);
        }
    }
}

/// Build horizontal tilt targets from sorted, finite recording-pixel events.
/// `ratio` is a multiplier, not a percentage. Cropping must not rescale velocity.
pub(crate) fn horizontal_tilt(points: &[MotionPoint], ratio: f64) -> Vec<RotationKey> {
    if points.is_empty() || !ratio.is_finite() || ratio == 0.0 { return Vec::new(); }
    let first = points[0];
    let skip_entry = points.get(1).is_some_and(|next| {
        let dt = next.time_ms - first.time_ms;
        dt > 0.0 && (next.x-first.x).hypot(next.y-first.y) / dt > 15.0
    });
    let initial_index = if skip_entry { 2 } else { 1 };
    let anchor = points.get(initial_index - 1).unwrap_or(&first);
    let seed = points.iter().enumerate().skip(initial_index).find(|(index, point)|
        point.time_ms-anchor.time_ms >= 120.0 || *index == points.len()-1);
    let mut speed = seed.map_or(0.0, |(_, point)| {
        let dt = point.time_ms-anchor.time_ms;
        if dt > 0.0 { (point.x-anchor.x)/dt } else { 0.0 }
    });
    let angle = |velocity: f64| (velocity * 12.0 * ratio).clamp(-20.0, 20.0);
    let mut keys = Keys::default();
    keys.emit(RotationKey {time_ms:first.time_ms, degrees:angle(speed)}, true);
    let mut advance = |start: f64, duration: f64, target: f64, keys: &mut Keys| {
        let count = (duration / 16.0).ceil().max(1.0) as usize;
        let dt = duration / count as f64;
        let weight = 1.0 - (-dt / 120.0).exp();
        for index in 1..=count {
            speed += (target-speed)*weight;
            keys.emit(RotationKey {time_ms:start+index as f64*dt, degrees:angle(speed)}, false);
        }
    };
    for (index, pair) in points.windows(2).enumerate() {
        let dt = pair[1].time_ms-pair[0].time_ms;
        if dt > 0.0 {
            let velocity = if index+1 < initial_index { 0.0 } else { (pair[1].x-pair[0].x)/dt };
            advance(pair[0].time_ms, dt, velocity, &mut keys);
        }
    }
    let end = points.last().unwrap().time_ms;
    advance(end, 720.0, 0.0, &mut keys);
    keys.emit(RotationKey {time_ms:end+720.0, degrees:0.0}, true);
    let calm_end = first.time_ms+500.0;
    let opening = sample(&keys.values, calm_end);
    let mut result = vec![RotationKey {time_ms:first.time_ms,degrees:opening},
        RotationKey {time_ms:calm_end,degrees:opening}];
    result.extend(keys.values.into_iter().filter(|key| key.time_ms > calm_end));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(time_ms:f64,x:f64,y:f64)->MotionPoint {MotionPoint{time_ms,x,y}}
    #[test]
    fn anticipated_events_shift_rotation_targets_with_the_drawn_path() {
        let points = (0..200).map(|i| point(i as f64 * 10.0,
            if i < 60 { i as f64 * 10.0 } else { 600.0 - (i - 60) as f64 * 4.0 }, 100.0))
            .collect::<Vec<_>>();
        let keys = horizontal_tilt(&retimed_points(&points, |time| time - 120.0), 1.0);
        for (time, expected) in [(0.0, 11.328726219244055), (500.0, 9.173945154824779),
            (750.0, -3.060041605057827), (1000.0, -4.5805704885703555), (1500.0, -4.772155165492189)] {
            assert!((sample(&keys, time) - expected).abs() < 1e-9);
        }
    }

    #[test]
    fn constant_pixel_speed_has_defined_gain_and_saturation() {
        for (speed,ratio,expected) in [(1.0,1.0,12.0),(-1.0,0.5,-6.0),(3.0,1.0,20.0)] {
            let keys=horizontal_tilt(&[point(0.0,0.0,0.0),point(1000.0,speed*1000.0,0.0)],ratio);
            for time in [0.0,250.0,500.0,800.0,1000.0] {
                assert!((sample(&keys,time)-expected).abs()<1e-10);
            }
            assert_eq!(sample(&keys,1720.0),0.0);
        }
    }
    #[test]
    fn rest_decay_matches_exponential_and_opening_is_held() {
        let keys=horizontal_tilt(&[point(100.0,0.0,0.0),point(1100.0,1000.0,0.0)],1.0);
        assert_eq!(sample(&keys,100.0),sample(&keys,600.0));
        // 240ms is exactly fifteen 16ms integration intervals after movement stops.
        assert!((sample(&keys,1340.0)-12.0*(-2.0_f64).exp()).abs()<1e-10);
        assert_eq!(sample(&keys,1820.0),0.0);
    }
    #[test]
    fn entry_teleport_is_not_treated_as_deliberate_motion() {
        let keys=horizontal_tilt(&[point(0.0,0.0,0.0),point(1.0,2000.0,0.0),point(1000.0,2000.0,0.0)],1.0);
        assert!(keys.iter().all(|key|key.degrees==0.0));
        let vertical=horizontal_tilt(&[point(0.0,0.0,0.0),point(1000.0,0.0,1000.0)],1.0);
        assert!(vertical.iter().all(|key|key.degrees==0.0));
    }
    #[test]
    fn reversal_smooths_velocity_before_clamping_the_angle() {
        let keys=horizontal_tilt(&[point(0.0,0.0,0.0),point(600.0,600.0,0.0),
            point(840.0,360.0,0.0)],1.0);
        assert!((sample(&keys,840.0)-(-12.0+24.0*(-2.0_f64).exp())).abs()<1e-10);
        assert!(sample(&keys,616.0)>0.0);
        assert!(sample(&keys,840.0)<0.0);
    }
    #[test]
    fn sampling_density_and_origin_do_not_change_constant_motion() {
        let sparse=horizontal_tilt(&[point(0.0,50.0,0.0),point(1000.0,1050.0,0.0)],1.0);
        let dense=horizontal_tilt(&(0..=100).map(|i|point(i as f64*10.0,i as f64*10.0,0.0)).collect::<Vec<_>>(),1.0);
        for time in [0.0,500.0,1000.0,1240.0,1720.0] {
            assert!((sample(&sparse,time)-sample(&dense,time)).abs()<1e-10);
        }
        assert!(horizontal_tilt(&[],1.0).is_empty());
        assert!(horizontal_tilt(&[point(0.0,0.0,0.0)],0.0).is_empty());
    }
}
