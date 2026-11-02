use super::*;
use crate::event_capture::CaptureSession;

fn events() -> Vec<EnhancedMouseEvent> {
    (0..36_000).map(|i| {
        serde_json::from_value(serde_json::json!({
            "base": {"timestamp": i * 10, "x": if i % 7 == 0 { 1200 } else { 300 },
                "y": 200, "event_type": if i % 31 == 0 {
                    serde_json::json!({"ButtonPress":{"button":"Left"}})
                } else { serde_json::json!("Move") }},
            "window_id": null, "app_name": null, "is_double_click": false, "cluster_id": null
        })).unwrap()
    }).collect()
}

fn linear(events: &[EnhancedMouseEvent], after: u64, until: u64,
    metadata: &crate::event_capture::SessionMetadata) -> bool {
    events.iter().any(|event| event.base.timestamp > after
        && event.base.timestamp <= until
        && point_in_capture(event.base.x, event.base.y, metadata)
        && matches!(event.base.event_type, MouseEventType::ButtonPress { .. }))
}

#[test]
fn typing_anchor_intervals_preserve_boundaries_and_capture_containment() {
    let events = events();
    let mut metadata = CaptureSession::new().metadata;
    metadata.display_resolution = (1000, 500);
    // Includes exact press boundaries, empty/reversed intervals, and beyond EOF.
    for after in (0..361_000).step_by(310) {
        for offset in [-1i64, 0, 1, 10, 310, 900] {
            let until = (after as i64 + offset).max(0) as u64;
            assert_eq!(has_click_between(&events, after, until, &metadata),
                linear(&events, after, until, &metadata), "{after}..{until}");
        }
    }
    assert!(!has_click_between(&[], 0, 1000, &metadata));
}

#[test]
#[ignore = "opt-in typing interval performance comparison"]
fn typing_interval_profile() {
    let events = events();
    let mut metadata = CaptureSession::new().metadata;
    metadata.display_resolution = (1000, 500);
    let intervals: Vec<_> = (0..360_000).step_by(100).map(|t| (t, t + 100)).collect();
    let mut results = Vec::new();
    for bounded in [false, true] {
        let start = std::time::Instant::now();
        let found: Vec<_> = intervals.iter().map(|&(a,b)| {
            if bounded { has_click_between(&events,a,b,&metadata) }
            else { linear(&events,a,b,&metadata) }
        }).collect();
        println!("TYPING_INTERVAL_PROFILE bounded={bounded} events={} intervals={} ms={}",
            events.len(), intervals.len(), start.elapsed().as_secs_f64()*1000.0);
        results.push(found);
    }
    assert_eq!(results[0], results[1]);
}
