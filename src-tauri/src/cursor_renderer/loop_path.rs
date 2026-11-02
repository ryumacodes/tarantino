//! Prepare the return-to-start target before position or rotation smoothing.

/// Keep events through the loop cutoff, then target the opening position one
/// millisecond after the final retained event. The spring supplies the return.
pub(super) fn prepare<T: Clone>(
    points: &[T], duration_ms: u64, loop_duration_ms: u64,
    time: impl Fn(&T) -> f64, retime: impl Fn(&T, f64, bool) -> T,
) -> Vec<T> {
    let Some(first) = points.first() else { return Vec::new(); };
    let cutoff=duration_ms.saturating_sub(loop_duration_ms.min(duration_ms)) as f64;
    let mut retained=points.iter().take_while(|point|time(point)<=cutoff).cloned().collect::<Vec<_>>();
    if retained.is_empty() {retained.push(first.clone());}
    retained[0]=retime(&retained[0],0.0,false);
    let end=time(retained.last().unwrap())+1.0;
    retained.push(retime(&retained[0],end,true));
    retained
}

pub(super) fn cursor_events(points:&[super::CursorEvent],duration_ms:u64,loop_ms:u64)->Vec<super::CursorEvent> {
    prepare(points,duration_ms,loop_ms,|point|point.timestamp_ms as f64,|point,time,synthetic| {
        super::CursorEvent { is_release: false,timestamp_ms:time as u64,x:point.x,y:point.y,
            is_click:!synthetic && point.is_click}
    })
}

pub(crate) fn rotation_points(points:&[super::rotation::MotionPoint],duration_ms:u64,loop_ms:u64)->Vec<super::rotation::MotionPoint> {
    prepare(points,duration_ms,loop_ms,|point|point.time_ms,|point,time,_| {
        super::rotation::MotionPoint{time_ms:time,x:point.x,y:point.y}
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(time:u64,x:f64)->super::super::CursorEvent {
        super::super::CursorEvent{ is_release: false,timestamp_ms:time,x,y:0.0,is_click:true}
    }
    #[test]
    fn sparse_events_return_after_last_retained_event_without_fake_click() {
        let events=cursor_events(&[event(20,0.2),event(800,0.8),event(1600,0.9)],2000,500);
        assert_eq!(events.iter().map(|p|p.timestamp_ms).collect::<Vec<_>>(),vec![0,800,801]);
        assert_eq!(events[2].x,0.2);
        assert!(!events[2].is_click);
        assert!(events[0].is_click);
    }
    #[test]
    fn physical_and_normalized_paths_keep_identical_timing() {
        let events=[event(0,0.2),event(1000,0.8),event(1800,0.1)];
        let raw=events.iter().map(|p|super::super::rotation::MotionPoint{
            time_ms:p.timestamp_ms as f64,x:p.x*1000.0+50.0,y:0.0}).collect::<Vec<_>>();
        let normalized=cursor_events(&events,2000,500);
        let physical=rotation_points(&raw,2000,500);
        for (a,b) in normalized.iter().zip(&physical) {
            assert_eq!(a.timestamp_ms as f64,b.time_ms);
            assert!((a.x*1000.0+50.0-b.x).abs()<1e-10);
        }
    }
    #[test]
    fn empty_and_short_recordings_are_bounded() {
        assert!(cursor_events(&[],200,500).is_empty());
        let points=cursor_events(&[event(50,0.4)],200,500);
        assert_eq!(points.iter().map(|p|p.timestamp_ms).collect::<Vec<_>>(),vec![0,1]);
        assert!(points.iter().all(|p|p.x==0.4));
    }
}
