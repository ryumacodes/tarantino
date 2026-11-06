use super::{sample, Keys, MotionPoint, RotationKey};

pub(crate) fn targets(points: &[MotionPoint], seed: Option<f64>) -> Vec<RotationKey> {
    let Some(first) = points.first() else { return Vec::new(); };
    let neutral = (-112.5_f64).to_radians();
    let limit = 135.0_f64.to_radians();
    let entry = points.get(1).is_some_and(|next| {
        let dt = next.time_ms - first.time_ms;
        dt > 0.0 && (next.x-first.x).hypot(next.y-first.y)/dt > 15.0
    });
    let initial_index = if entry { 2 } else { 1 };
    let anchor = points.get(initial_index-1).unwrap_or(first);
    let mut angle = seed.unwrap_or_else(|| {
        points.iter().skip(initial_index).find(|p| (p.x-anchor.x).hypot(p.y-anchor.y) >= 4.0)
            .map_or(0.0, |p| {
                let heading = (p.y-anchor.y).atan2(p.x-anchor.x)-neutral;
                ((heading+std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)-std::f64::consts::PI).clamp(-limit,limit)
            })
    });
    let mut velocity = 0.0;
    let mut keys = Keys::default();
    keys.emit(RotationKey { time_ms: first.time_ms, degrees: angle.to_degrees() }, true);
    let mut advance = |start: f64, duration: f64, heading: Option<f64>, speed: f64, keys: &mut Keys| {
        let count = (duration/16.0).ceil().max(1.0) as usize;
        let dt = duration/count as f64;
        for index in 1..=count {
            if let Some(heading) = heading {
                velocity += 0.00005*speed.min(3.0)*(heading-(neutral+angle)).sin()*dt;
            }
            velocity *= (-dt/100.0).exp();
            velocity = velocity.clamp(-0.4_f64.to_radians(),0.4_f64.to_radians());
            angle += velocity*dt;
            if angle.abs() > limit { angle = angle.clamp(-limit,limit); velocity = 0.0; }
            keys.emit(RotationKey { time_ms: start+index as f64*dt, degrees: angle.to_degrees() }, false);
        }
    };
    let (mut dx, mut dy, mut elapsed) = (0.0_f64,0.0_f64,0.0_f64);
    for (index,pair) in points.windows(2).enumerate() {
        let dt = pair[1].time_ms-pair[0].time_ms;
        if dt <= 0.0 { continue; }
        if index+1 < initial_index { advance(pair[0].time_ms,dt,None,0.0,&mut keys); continue; }
        dx += pair[1].x-pair[0].x;
        dy += pair[1].y-pair[0].y;
        elapsed += dt;
        let distance = dx.hypot(dy);
        if distance >= 4.0 {
            advance(pair[0].time_ms,dt,Some(dy.atan2(dx)),distance/elapsed,&mut keys);
            (dx,dy,elapsed) = (0.0,0.0,0.0);
        } else { advance(pair[0].time_ms,dt,None,0.0,&mut keys); }
    }
    let end = points.last().unwrap().time_ms+800.0;
    advance(end-800.0,800.0,None,0.0,&mut keys);
    keys.emit(RotationKey { time_ms:end,degrees:angle.to_degrees() },true);
    let calm_end = first.time_ms+500.0;
    let opening = sample(&keys.values,calm_end);
    let mut result = vec![RotationKey {time_ms:first.time_ms,degrees:opening},RotationKey {time_ms:calm_end,degrees:opening}];
    result.extend(keys.values.into_iter().filter(|key| key.time_ms>calm_end));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heading_is_retained_after_motion_stops() {
        for (x,y,expected) in [(1000.0,0.0,112.5),(0.0,-1000.0,22.5),(-1000.0,0.0,-67.5),(0.0,1000.0,-135.0)] {
            let keys = targets(&[MotionPoint{time_ms:0.0,x:0.0,y:0.0},MotionPoint{time_ms:1000.0,x,y}],None);
            for time in [0.0,500.0,1000.0,1800.0,3000.0] {
                assert!((sample(&keys,time)-expected).abs()<1e-8);
            }
        }
    }
    #[test]
    #[ignore = "requires external motion fixtures"]
    fn recorded_directional_targets() {
        let path = std::env::var("DIRECTIONAL_ROTATION_FIXTURE").unwrap();
        let data: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in data["cases"].as_array().unwrap() {
            let points = case["points"].as_array().unwrap().iter().map(|p| MotionPoint {
                time_ms:p["recordingTimeMs"].as_f64().unwrap(),x:p["x"].as_f64().unwrap(),y:p["y"].as_f64().unwrap()
            }).collect::<Vec<_>>();
            if let Some(input) = case["inputPoints"].as_array() {
                let input = input.iter().map(|p| MotionPoint {
                    time_ms:p["recordingTimeMs"].as_f64().unwrap(),x:p["x"].as_f64().unwrap(),y:p["y"].as_f64().unwrap()
                }).collect::<Vec<_>>();
                let actual = crate::cursor_renderer::loop_path::rotation_points(&input,
                    case["duration"].as_u64().unwrap(),case["loopDuration"].as_u64().unwrap());
                assert_eq!(actual.len(),points.len(),"{}",case["name"]);
                for (a,b) in actual.iter().zip(&points) {
                    assert_eq!((a.time_ms,a.x,a.y),(b.time_ms,b.x,b.y),"{}",case["name"]);
                }
            }
            let keys = targets(&points,case["seed"].as_f64());
            assert_eq!(keys.len(),case["keys"].as_array().unwrap().len(),"{}",case["name"]);
            for (actual,expected) in keys.iter().zip(case["keys"].as_array().unwrap()) {
                assert!((actual.time_ms-expected["time_ms"].as_f64().unwrap()).abs()<1e-8,"{}",case["name"]);
                assert!((actual.degrees-expected["degrees"].as_f64().unwrap()).abs()<1e-8,"{}",case["name"]);
            }
            let mut animation = crate::cursor_renderer::angular_spring::AngularTimeline::for_mode(sample(&keys,0.0),Some("directional"));
            for expected in case["animation"].as_array().unwrap() {
                let actual = animation.sample(expected["time_ms"].as_f64().unwrap(),|time| sample(&keys,time));
                assert!((actual-expected["degrees"].as_f64().unwrap()).abs()<1e-8,"{}: {} vs {}",case["name"],actual,expected);
            }
            for expected in case["samples"].as_array().unwrap() {
                assert!((sample(&keys,expected["time_ms"].as_f64().unwrap())-expected["degrees"].as_f64().unwrap()).abs()<1e-8,"{}",case["name"]);
            }
        }
    }
}
