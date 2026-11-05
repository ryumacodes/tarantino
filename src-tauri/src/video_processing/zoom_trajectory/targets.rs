//! Stable automatic camera targets from spatially grouped pointer samples.
use super::{ZoomBlock, CameraViewport};
use crate::cursor_renderer::CursorEvent;

#[derive(Clone, Copy)]
pub(crate) struct CameraActivity<'a> {
    pub events: &'a [CursorEvent],
    pub smooth: bool,
    pub prepared: Option<&'a crate::video_processing::camera_activity::Tracks>,
}

#[derive(Debug)]
pub(crate) struct Target {
    time: f64,
    x: f64,
    y: f64,
}

pub(crate) fn prepare(block: &ZoomBlock, events: &[CursorEvent]) -> Vec<Target> {
    prepare_in_viewport(block, events, CameraViewport::default(),None)
}

pub(crate) fn prepare_in_viewport(block: &ZoomBlock, events: &[CursorEvent], viewport: CameraViewport,
    activity: Option<CameraActivity<'_>>) -> Vec<Target> {
    if block.is_manual || block.kind.as_deref() == Some("typing") {
        return Vec::new();
    }
    if let Some(input)=activity.and_then(|input|input.prepared.map(|track|(track,input.smooth))) {
        return prepare_range(&input.0.grouped,block.start_time_ms as f64,block.end_time_ms as f64,
            |e|(e.time,e.x,e.y),||input.0.fallback(block.start_time_ms as f64,input.1).map(|e|(e.time,e.x,e.y)),
            viewport.activity_extent(block.zoom_level));
    }
    let activity = activity.unwrap_or(CameraActivity { events, smooth: true, prepared:None });
    prepare_range(events, block.start_time_ms as f64, block.end_time_ms as f64,
        |event| (event.timestamp_ms as f64, event.x, event.y),
        || fallback(activity, block.start_time_ms).map(|event| (event.timestamp_ms as f64,event.x,event.y)),
        viewport.activity_extent(block.zoom_level))
}

/// Bounds and seeded target times stay continuous after playback-to-source mapping.
fn prepare_range<T>(events: &[T], start: f64, end: f64,
    point: impl Fn(&T) -> (f64,f64,f64),
    fallback: impl FnOnce() -> Option<(f64,f64,f64)>, extent: (f64,f64),
) -> Vec<Target> {
    let first = events.partition_point(|event| point(event).0 < start);
    let last = events.partition_point(|event| point(event).0 <= end);
    if first >= last {
        return fallback().map(|p| group_indexed(1, |_| p, extent)).unwrap_or_default();
    }
    let events = &events[first..last];
    let seed = (point(&events[0]).0 == start).then(|| {
        let (_,x,y) = point(&events[0]);
        (start-1.0,x,y)
    });
    let offset = usize::from(seed.is_some());
    group_indexed(events.len()+offset, |index| {
        if index == 0 { if let Some(seed) = seed { return seed; } }
        point(&events[index-offset])
    }, extent)
}

fn fallback(activity: CameraActivity<'_>, time: u64) -> Option<&CursorEvent> {
    let events=activity.events;
    let first=events.partition_point(|event|event.timestamp_ms<time);
    if activity.smooth {
        if let Some(event)=events[first..].iter().take_while(|event|event.timestamp_ms-time<=500)
            .find(|event|event.is_click || event.is_release) { return Some(event); }
    }
    if events.get(first).is_some_and(|event|event.timestamp_ms==time) { events.get(first) }
    else { events.get(first.saturating_sub(1)) }
}

#[cfg(test)]
fn group(points: &[(f64, f64, f64)], extent: (f64, f64)) -> Vec<Target> {
    group_indexed(points.len(), |index| points[index], extent)
}

fn group_indexed(len: usize, point: impl Fn(usize) -> (f64,f64,f64), extent: (f64, f64)) -> Vec<Target> {
    let mut targets = Vec::new();
    let mut start = 0;
    while start < len {
        let (_, x, y) = point(start);
        let (mut left, mut right, mut top, mut bottom) = (x, x, y, y);
        let mut end = start + 1;
        while end < len {
            let (_, x, y) = point(end);
            let bounds = (left.min(x), right.max(x), top.min(y), bottom.max(y));
            if bounds.1 - bounds.0 > extent.0 || bounds.3 - bounds.2 > extent.1 {
                break;
            }
            (left, right, top, bottom) = bounds;
            end += 1;
        }
        let mut weight = 0.0;
        let (mut x, mut y) = (0.0, 0.0);
        for index in start..end {
            let current = point(index);
            let until = if index+1 < len { point(index+1).0 } else { current.0+100.0 };
            let duration = until - current.0;
            weight += duration;
            x += current.1 * duration;
            y += current.2 * duration;
        }
        let last = point(end - 1);
        targets.push(Target {
            time: point(start).0,
            x: if weight > 0.0 { x / weight } else { last.1 },
            y: if weight > 0.0 { y / weight } else { last.2 },
        });
        start = end;
    }
    targets
}

pub(crate) fn at(targets: &[Target], time: f64) -> Option<(f64, f64)> {
    let first = targets.partition_point(|target| target.time < time);
    let index = if targets.get(first).is_some_and(|target|target.time == time) {
        first
    } else { first.saturating_sub(1) };
    targets.get(index).map(|target| (target.x, target.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "long recording target preparation workload"]
    fn target_preparation_profile() {
        let events=(0..120_000).map(|i|(i as f64*8.0,
            (i as f64/500.0).sin()*0.4+0.5,(i as f64/700.0).cos()*0.4+0.5)).collect::<Vec<_>>();
        let ranges=(0..40).map(|i|(i as f64*1000.0,800_000.0+i as f64*1000.0)).collect::<Vec<_>>();
        let started=std::time::Instant::now();
        let baseline=ranges.iter().map(|&(start,end)| {
            let first=events.partition_point(|p|p.0<start);
            let last=events.partition_point(|p|p.0<=end);
            let mut points=Vec::new();
            if events[first].0==start { points.push((start-1.0,events[first].1,events[first].2)); }
            points.extend_from_slice(&events[first..last]);
            group(&points,(0.25,0.25))
        }).collect::<Vec<_>>();
        let copied_time=started.elapsed();
        let started=std::time::Instant::now();
        for (&(start,end),expected) in ranges.iter().zip(&baseline) {
            let actual=prepare_range(&events,start,end,|p|*p,||None,(0.25,0.25));
            assert_eq!(actual.len(),expected.len());
            for (actual,expected) in actual.iter().zip(expected) {
                assert_eq!((actual.time,actual.x,actual.y),(expected.time,expected.x,expected.y));
            }
        }
        println!("target preparation: copied={copied_time:?}, borrowed={:?}; removed temporary sample storage: {} bytes per range",
            started.elapsed(),100_002*std::mem::size_of::<(f64,f64,f64)>());
    }

    #[test]
    #[ignore = "requires external camera fallback fixtures"]
    fn empty_camera_ranges_match_external_fallback_fixtures() {
        #[derive(serde::Deserialize)]
        struct Point { time: u64, x: f64, y: f64, #[serde(default)] kind: String }
        #[derive(serde::Deserialize)]
        struct Fixture { smooth: bool, time: u64, events: Vec<Point>, expected: Option<Point> }
        let path=std::env::var("TARANTINO_CAMERA_FALLBACK_FIXTURES").unwrap();
        let fixtures:Vec<Fixture>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!fixtures.is_empty());
        for fixture in fixtures {
            let events=fixture.events.iter().map(|p|CursorEvent {timestamp_ms:p.time,x:p.x,y:p.y,
                is_click:p.kind=="mouseDown",is_release:p.kind=="mouseUp"}).collect::<Vec<_>>();
            let actual=fallback(CameraActivity{events:&events,smooth:fixture.smooth,prepared:None},fixture.time);
            assert_eq!(actual.map(|e|(e.timestamp_ms,e.x,e.y)),fixture.expected.map(|p|(p.time,p.x,p.y)));
        }
    }

    #[test]
    #[ignore = "requires external automatic target range fixtures"]
    fn automatic_targets_match_external_range_fixtures() {
        #[derive(serde::Deserialize)]
        struct Expected { time: f64, x: f64, y: f64 }
        #[derive(serde::Deserialize)]
        struct Fixture {
            name: String, start: f64, end: f64, zoom: f64,
            points: Vec<(f64,f64,f64)>, targets: Vec<Expected>, samples: Vec<Expected>,
        }
        let path=std::env::var("TARANTINO_CAMERA_TARGET_FIXTURES").unwrap();
        let fixtures: Vec<Fixture>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!fixtures.is_empty());
        for fixture in fixtures {
            let targets=prepare_range(&fixture.points,fixture.start,fixture.end,
                |point|*point,||None,(0.5/fixture.zoom,0.5/fixture.zoom));
            assert_eq!(targets.len(),fixture.targets.len(),"{} target count",fixture.name);
            for (actual,expected) in targets.iter().zip(&fixture.targets) {
                assert_eq!(actual.time,expected.time,"{} target time",fixture.name);
                assert!((actual.x-expected.x).abs()<1e-12 && (actual.y-expected.y).abs()<1e-12,
                    "{} center: {:?} expected {},{}",fixture.name,actual,expected.x,expected.y);
            }
            for sample in fixture.samples {
                let actual=at(&targets,sample.time).unwrap();
                assert!((actual.0-sample.x).abs()<1e-12 && (actual.1-sample.y).abs()<1e-12,
                    "{} at {}",fixture.name,sample.time);
            }
        }
    }

    #[test]
    fn fractional_source_boundaries_keep_weight_and_exact_transition() {
        let targets=prepare_range(&[(100.25,0.2,0.3),(200.125,0.25,0.35),(650.25,0.8,0.7)],
            100.25,900.75,|p|*p,||None,(0.25,0.25));
        let expected=(0.2*100.875+0.25*450.125)/551.0;
        assert!((at(&targets,650.249).unwrap().0-expected).abs()<1e-12);
        assert_eq!(at(&targets,650.25),Some((0.8,0.7)));
        assert_eq!(targets[0].time,99.25);
    }

    #[test]
    fn holds_weighted_region_until_pointer_exceeds_its_bounds() {
        let targets = group(&[(0.0, 0.2, 0.4), (100.0, 0.3, 0.5), (900.0, 0.9, 0.5)], (0.25, 0.25));
        assert_eq!(targets.len(), 2);
        let expected = (0.2 * 100.0 + 0.3 * 800.0) / 900.0;
        for time in [0, 50, 100, 899] {
            assert!((at(&targets, time as f64).unwrap().0 - expected).abs() < 1e-12);
        }
        assert_eq!(at(&targets, 900.0), Some((0.9, 0.5)));
    }
    #[test]
    fn inclusive_bounds_and_duplicate_times_remain_finite() {
        let targets = group(&[(0.0, 0.0, 0.0), (0.0, 0.25, 0.25), (0.0, 0.8, 0.8)], (0.25, 0.25));
        assert_eq!(targets.len(), 2);
        assert_eq!((targets[0].x, targets[0].y), (0.25, 0.25));
        assert_eq!(at(&targets, 0.0), Some((0.25, 0.25)));
        assert!(at(&[], 0.0).is_none());
    }

    #[test]
    fn tall_visible_region_holds_vertical_activity_but_splits_horizontal_activity() {
        let vertical = group(&[(0.0, 0.3,0.2), (100.0, 0.3,0.6)], (0.25,0.5));
        assert_eq!(vertical.len(), 1);
        let horizontal = group(&[(0.0, 0.2,0.3), (100.0, 0.6,0.3)], (0.25,0.5));
        assert_eq!(horizontal.len(), 2);
        assert_eq!(group(&[(0.0, 0.3,0.2), (100.0, 0.3,0.6)], (0.25,0.25)).len(), 2);
    }
}
