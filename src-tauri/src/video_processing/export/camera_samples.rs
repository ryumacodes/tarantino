//! Reduce tiny pointer movements for camera targeting in logical source pixels.
use serde_json::Value;
use std::borrow::Borrow;

/// Discard a sample isolated from both following samples, including the release
/// paired with a discarded press. Work before normalization so thresholds are pixels.
pub(crate) fn remove_shake<T: Borrow<Value>>(mut events: Vec<T>, threshold: f64) -> Vec<T> {
    if !threshold.is_finite() || threshold <= 0.0 { return events; }
    events.sort_by_key(|event| {
        let event = event.borrow();
        event.get("base").unwrap_or(event).get("timestamp").and_then(Value::as_u64)
    });
    let point = |event: &T| {
        let event = event.borrow();
        let base = event.get("base").unwrap_or(event);
        Some((base.get("x")?.as_f64()?, base.get("y")?.as_f64()?))
    };
    let mut skip_release = false;
    let mut keep = Vec::with_capacity(events.len());
    for (index, event) in events.iter().enumerate() {
        let value = event.borrow();
        let kind = &value.get("base").unwrap_or(value)["event_type"];
        let press = kind.as_str() == Some("ButtonPress") || kind.get("ButtonPress").is_some();
        let release = kind.as_str() == Some("ButtonRelease") || kind.get("ButtonRelease").is_some();
        if skip_release && release { skip_release = false; keep.push(false); continue; }
        let isolated = point(event).is_some_and(|(x, y)| {
            [index + 1, index + 2].iter().all(|&next| {
                events.get(next).and_then(point).is_some_and(|(nx, ny)| {
                    (x - nx).abs() + (y - ny).abs() > threshold
                })
            })
        });
        if isolated && press { skip_release = true; }
        keep.push(!isolated);
    }
    events.into_iter().zip(keep).filter_map(|(event, keep)| keep.then_some(event)).collect()
}

pub(super) fn filter(mut events: Vec<Value>) -> Vec<Value> {
    events.sort_by_key(|event| {
        event.get("base").unwrap_or(event).get("timestamp").and_then(Value::as_u64)
    });
    let mut previous: Option<(f64, f64)> = None;
    events.retain(|event| {
        let base = event.get("base").unwrap_or(event);
        let Some(x) = base.get("x").and_then(Value::as_f64) else { return false };
        let Some(y) = base.get("y").and_then(Value::as_f64) else { return false };
        if !x.is_finite() || !y.is_finite() || base.get("timestamp").and_then(Value::as_u64).is_none() {
            return false;
        }
        let button = base.get("event_type").is_some_and(|kind| {
            matches!(kind.as_str(), Some("ButtonPress" | "ButtonRelease"))
                || kind.get("ButtonPress").is_some() || kind.get("ButtonRelease").is_some()
        });
        let keep = button || previous.is_none_or(|(px, py)| (x - px).hypot(y - py) >= 10.0);
        if keep { previous = Some((x, y)); }
        keep
    });
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shake_removal_preserves_sustained_moves_and_drops_paired_release() {
        let events = vec![
            json!({"timestamp":0,"x":200,"y":200,"event_type":"ButtonPress"}),
            json!({"timestamp":1,"x":0,"y":0}),
            json!({"timestamp":2,"x":1,"y":1}),
            json!({"timestamp":3,"x":2,"y":2,"event_type":{"ButtonRelease":{}}}),
            json!({"timestamp":4,"x":100,"y":100}),
            json!({"timestamp":5,"x":101,"y":101}),
        ];
        assert_eq!(remove_shake(events.clone(), 0.0), events);
        let kept = remove_shake(events, 100.0);
        assert_eq!(kept.iter().map(|v| v["timestamp"].as_u64().unwrap()).collect::<Vec<_>>(), [1,2,4,5]);
    }

    #[test]
    fn borrowed_samples_match_owned_filtering_without_copying_events() {
        let events = vec![
            json!({"base":{"timestamp":3,"x":2,"y":2,"event_type":"ButtonRelease"}}),
            json!({"base":{"timestamp":0,"x":200,"y":200,"event_type":"ButtonPress"}}),
            json!({"base":{"timestamp":1,"x":0,"y":0}}),
            json!({"base":{"timestamp":2,"x":1,"y":1}}),
            json!({"base":{"timestamp":4,"x":100,"y":100}}),
        ];
        for threshold in [0.0, 10.0, 100.0, f64::NAN] {
            let owned = remove_shake(events.clone(), threshold);
            let borrowed = remove_shake(events.iter().collect(), threshold);
            assert_eq!(borrowed, owned.iter().collect::<Vec<_>>());
            assert!(borrowed.iter().all(|event| events.iter().any(|original| std::ptr::eq(*event, original))));
        }
    }

    #[test]
    fn shake_threshold_is_strict_manhattan_distance() {
        let events = vec![
            json!({"timestamp":0,"x":0,"y":0}),
            json!({"timestamp":1,"x":6,"y":4}),
            json!({"timestamp":2,"x":6,"y":4}),
        ];
        assert_eq!(remove_shake(events.clone(), 10.0).len(), 3);
        assert_eq!(remove_shake(events, 9.0).len(), 2);
    }

    #[test]
    fn distance_is_from_last_retained_sample_and_includes_exact_threshold() {
        let result = filter(vec![
            json!({"timestamp": 0, "x": 0, "y": 0}),
            json!({"timestamp": 1, "x": 3, "y": 4}),
            json!({"timestamp": 2, "x": 6, "y": 8}),
            json!({"timestamp": 3, "x": 7, "y": 9}),
        ]);
        assert_eq!(result.iter().map(|v| v["timestamp"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 2]);
    }

    #[test]
    fn press_and_release_survive_and_reset_distance_anchor() {
        let result = filter(vec![
            json!({"base": {"timestamp": 0, "x": 0, "y": 0}}),
            json!({"base": {"timestamp": 1, "x": 2, "y": 0, "event_type": {"ButtonPress": {"button": "Left"}}}}),
            json!({"base": {"timestamp": 2, "x": 4, "y": 0, "event_type": "ButtonRelease"}}),
            json!({"base": {"timestamp": 3, "x": 12, "y": 0}}),
            json!({"base": {"timestamp": 4, "x": 14, "y": 0}}),
        ]);
        assert_eq!(result.iter().map(|v| v["base"]["timestamp"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 1, 2, 4]);
    }

    #[test]
    fn sidecar_filter_uses_logical_pixels_without_reducing_cursor_samples() {
        use crate::video_processing::export::{load_camera_events, load_raw_cursor_events};
        let path = std::env::temp_dir().join(format!("camera-filter-{}.json", uuid::Uuid::new_v4()));
        let sidecar = json!({
            "display_width": 1000, "display_height": 500, "scale_factor": 2,
            "recording_area": {"x": -100, "y": 50, "width": 1000, "height": 500},
            "mouse_events": [
                {"timestamp": 20, "x": -80, "y": 50},
                {"timestamp": 0, "x": -100, "y": 50},
                {"timestamp": 5, "x": -95, "y": 50},
                {"timestamp": 10, "x": -90, "y": 50}
            ]
        });
        std::fs::write(&path, sidecar.to_string()).unwrap();
        let camera = load_camera_events(&path, 2000, 1000, 0.0);
        let cursor = load_raw_cursor_events(&path, 2000, 1000);
        std::fs::remove_file(path).unwrap();
        assert_eq!(camera.iter().map(|v| v.timestamp_ms).collect::<Vec<_>>(), [0, 10, 20]);
        assert_eq!(cursor.len(), 4);
        assert_eq!(camera[1].x, 0.01);
        assert_eq!(camera[1].y, 0.0);
    }

    #[test]
    fn camera_loader_retains_drag_boundaries_without_extra_click_effects() {
        use crate::video_processing::export::load_camera_events;
        let path = std::env::temp_dir().join(format!("camera-buttons-{}.json", uuid::Uuid::new_v4()));
        let sidecar = json!({
            "display_width":1000,"display_height":500,"scale_factor":2,
            "mouse_events":[
                {"base":{"timestamp":0,"x":100,"y":100,"event_type":"Move"}},
                {"base":{"timestamp":1,"x":102,"y":100,"event_type":{"ButtonPress":{"button":"Left"}}}},
                {"base":{"timestamp":2,"x":104,"y":100,"event_type":{"ButtonRelease":{"button":"Left"}}}},
                {"base":{"timestamp":3,"x":112,"y":100,"event_type":"Move"}},
                {"base":{"timestamp":4,"x":114,"y":100,"event_type":"Move"}}
            ]
        });
        std::fs::write(&path,sidecar.to_string()).unwrap();
        let camera = load_camera_events(&path,2000,1000,0.0);
        std::fs::remove_file(path).unwrap();
        assert_eq!(camera.iter().map(|e|e.timestamp_ms).collect::<Vec<_>>(),[0,1,2,4]);
        assert_eq!(camera.iter().filter(|e|e.is_click).map(|e|e.timestamp_ms).collect::<Vec<_>>(),[1]);
        assert_eq!((camera[2].x,camera[2].y),(0.104,0.2));
    }

    #[test]
    #[ignore = "Set TARANTINO_MOUSE_SIDECAR to audit an actual recording"]
    fn recorded_sidecar_preserves_every_button_boundary() {
        use crate::video_processing::export::{load_camera_events,load_raw_cursor_events};
        let path = std::path::PathBuf::from(std::env::var("TARANTINO_MOUSE_SIDECAR").unwrap());
        let sidecar: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let source = sidecar.get("mouse_events").unwrap_or(&sidecar).as_array().unwrap();
        let buttons: Vec<_> = source.iter().filter_map(|event| {
            let base = event.get("base").unwrap_or(event);
            let kind = &base["event_type"];
            let press = kind.as_str()==Some("ButtonPress") || kind.get("ButtonPress").is_some();
            let release = kind.as_str()==Some("ButtonRelease") || kind.get("ButtonRelease").is_some();
            (press || release).then(||(base["timestamp"].as_u64().unwrap(),press))
        }).collect();
        assert!(!buttons.is_empty(),"Choose a recording with physical button activity");
        let camera = load_camera_events(&path,1920,1080,0.0);
        let raw = load_raw_cursor_events(&path,1920,1080);
        for (time,press) in &buttons {
            assert!(camera.iter().any(|e|e.timestamp_ms==*time && e.is_click==*press),
                "Button boundary lost at {time}");
        }
        assert!(camera.len()<=raw.len());
        assert!(camera.iter().all(|e|e.x.is_finite() && e.y.is_finite()));
        println!("Recorded input: {} raw, {} camera samples, {} button boundaries retained",
            raw.len(),camera.len(),buttons.len());
    }
}
