use super::{Detector, Sample, reader::Reader};

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    video: std::path::PathBuf,
    width: u32,
    height: u32,
    samples: Vec<Sample>,
    clicks: Vec<f64>,
    expected: Vec<f64>,
    frame_count: usize,
    source_width: u32,
    source_height: u32,
}

#[test]
#[ignore = "requires local video decode fixtures and hardware access"]
fn native_reader_preserves_timestamps_and_hover_results() {
    let path = std::env::var("TARANTINO_HOVER_VIDEO_FIXTURE").unwrap();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(cases.len() >= 4);
    for case in cases {
        let mut reader = Reader::open(&case.video, case.width, case.height).unwrap();
        let mut detector = Detector::new(case.samples, case.clicks, 1.0).unwrap();
        let mut changes = Vec::new();
        let mut count = 0;
        let mut previous = f64::NEG_INFINITY;
        while let Some(frame) = reader.next().unwrap() {
            assert_eq!(
                (frame.width, frame.height),
                (case.width as usize, case.height as usize),
                "{}",
                case.name
            );
            assert!(frame.time_ms >= previous);
            previous = frame.time_ms;
            count += 1;
            if let Some(time) = detector.push(frame).unwrap() {
                changes.push(time);
            }
        }
        assert_eq!(count, case.frame_count, "{}", case.name);
        assert_eq!(changes, case.expected, "{}", case.name);
        println!(
            "{}: {count} frames, {} hover changes",
            case.name,
            changes.len()
        );
    }
}

#[test]
fn copying_strided_planes_excludes_row_padding() {
    let data = [1, 2, 3, 99, 4, 5, 6, 99];
    assert_eq!(
        super::reader::copy_plane(data.as_ptr(), 4, 3, 2).unwrap(),
        vec![1, 2, 3, 4, 5, 6]
    );
}

#[test]
#[ignore = "requires local video decode fixtures and hardware access"]
fn recording_analysis_reuses_results_and_invalidates_changed_input() {
    let path = std::env::var("TARANTINO_HOVER_VIDEO_FIXTURE").unwrap();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for case in cases {
        let sidecar =
            std::env::temp_dir().join(format!("hover-input-{}.json", uuid::Uuid::new_v4()));
        let events = case
            .samples
            .iter()
            .map(|s| {
                serde_json::json!({"timestamp":s.time_ms as u64,
            "x":s.x+123.0,"y":s.y-76.0,"event_type":"Move"})
            })
            .collect::<Vec<_>>();
        let value = serde_json::json!({"display_width":case.width,"display_height":case.height,
            "recording_area":{"x":123,"y":-76,"width":case.width,"height":case.height},"mouse_events":events});
        let bytes = serde_json::to_vec(&value).unwrap();
        std::fs::write(&sidecar, &bytes).unwrap();
        let size = (case.source_width, case.source_height);
        let first = super::analysis::analyze(&case.video, &sidecar, size).unwrap();
        assert_eq!(*first, case.expected, "{}", case.name);
        let again = super::analysis::analyze(&case.video, &sidecar, size).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &again));
        let mut inactive = value;
        for event in inactive["mouse_events"].as_array_mut().unwrap() {
            event["event_type"] = serde_json::json!("Unknown");
        }
        std::fs::write(&sidecar, serde_json::to_vec(&inactive).unwrap()).unwrap();
        let changed = super::analysis::analyze(&case.video, &sidecar, size).unwrap();
        assert!(changed.is_empty());
        assert!(!std::sync::Arc::ptr_eq(&first, &changed));
        std::fs::write(&sidecar, &bytes).unwrap();
        let restored = super::analysis::analyze(&case.video, &sidecar, size).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &restored));
        std::fs::remove_file(sidecar).unwrap();
    }
}

#[tokio::test]
#[ignore = "requires local video decode fixtures and hardware access"]
async fn preview_and_shared_planner_use_detected_hover_constraints() {
    let path = std::env::var("TARANTINO_HOVER_VIDEO_FIXTURE").unwrap();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let case = cases
        .into_iter()
        .find(|case| case.name.ends_with("motion"))
        .unwrap();
    let sidecar = std::env::temp_dir().join(format!("hover-planner-{}.json", uuid::Uuid::new_v4()));
    let events = case
        .samples
        .iter()
        .map(|sample| {
            serde_json::json!({
                "timestamp":sample.time_ms as u64,"x":sample.x,"y":sample.y,"event_type":"Move"
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        &sidecar,
        serde_json::to_vec(&serde_json::json!({
            "display_width":case.width,"display_height":case.height,"mouse_events":events
        }))
        .unwrap(),
    )
    .unwrap();
    let spring = crate::cursor_renderer::SpringConfig {
        tension: 470.0,
        friction: 70.0,
        mass: 3.0,
    };
    let settings: crate::video_processing::CursorSettings =
        serde_json::from_value(serde_json::json!({
            "follow_hover":true,"always_use_pointer":false,"optimize_cursor_types":false
        }))
        .unwrap();
    let planned = crate::cursor_renderer::planner::runtime::prepare(
        &sidecar,
        (case.source_width, case.source_height),
        None,
        None,
        &case.video,
        2000.0,
        &settings,
        spring,
    )
    .unwrap()
    .unwrap();
    let hover =
        serde_json::from_value(serde_json::json!({"recordingPath":case.video,"enabled":true}))
            .unwrap();
    let response = crate::commands::video::compute_cursor_trajectory(
        sidecar.to_string_lossy().into_owned(),
        2000,
        100,
        case.source_width,
        case.source_height,
        1.0,
        false,
        2000,
        false,
        0,
        false,
        None,
        None,
        Some(spring.tension),
        Some(spring.friction),
        Some(spring.mass),
        Some(0.0),
        Some(false),
        Some(100.0),
        None,
        Some(false),
        Some(false),
        Some(hover),
    )
    .await
    .unwrap();
    let frames: Vec<Vec<f64>> = serde_json::from_str(&response).unwrap();
    for time in case.expected {
        let target = super::input::position(&case.samples, time).unwrap();
        let actual = planned.at(time);
        let error = (actual[0] * case.width as f64 - target[0])
            .hypot(actual[1] * case.height as f64 - target[1]);
        assert!(error <= 4.0, "hover at {time} missed by {error} points");
        let index = (time / 10.0).round() as usize;
        let sampled_time = (index as f64 * 120.0 / 100.0 + 1e-7).floor() * 1000.0 / 120.0;
        let sampled = planned.at(sampled_time);
        let preview = &frames[index];
        assert!(
            (preview[0] - sampled[0]).abs() < 0.000001 && (preview[1] - sampled[1]).abs() < 0.000001,
            "preview differs from shared planner at {sampled_time}"
        );
    }
    std::fs::remove_file(sidecar).unwrap();
}
