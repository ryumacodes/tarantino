//! Reproducible CPU-only camera workload; no capture permissions or GPU needed.
use super::*;
use std::time::Instant;

#[test]
#[ignore = "opt-in camera transition trace; set TARANTINO_CAMERA_TRACE_OUTPUT"]
fn camera_transition_trace() {
    let output = std::env::var("TARANTINO_CAMERA_TRACE_OUTPUT").unwrap();
    let mut results = Vec::new();
    for (style, config) in [
        ("focused", SpringConfig { tension:250.0,friction:40.0,mass:2.25 }),
        ("smooth", SpringConfig { tension:170.0,friction:50.0,mass:3.0 }),
    ] {
        for (history, ranges) in [
            ("hold", vec![(300,1799,2.0)]),
            ("interrupted", vec![(300,449,2.0),(450,699,1.3),(700,999,2.5)]),
        ] {
            let blocks: Vec<ZoomBlock> = ranges.into_iter().map(|(start,end,scale)|
                serde_json::from_value(serde_json::json!({
                    "start_time_ms":start,"end_time_ms":end,"zoom_level":scale,
                    "center_x":0.5,"center_y":0.5,"is_manual":true,
                    "animation_lead_in_ms":0
                })).unwrap()).collect();
            let production = simulate_zoom_trajectory(&blocks,&[],&config,true,style,60.0,4001,true);
            let direct = simulate_zoom_steps(&blocks,&[],&config,true,style,60.0,4001,true,
                CameraViewport::default(),None);
            assert_eq!(production.len(),241);
            assert_eq!(direct.len(),production.len());
            results.push(serde_json::json!({"style":style,"history":history,
                "samples":production.iter().zip(&direct).enumerate().map(|(i,(actual,sixty))|
                    serde_json::json!({"time":i as f64*1000.0/60.0,
                        "production":actual.scale,"direct60":sixty.scale})).collect::<Vec<_>>()
            }));
        }
    }
    std::fs::write(output,serde_json::to_vec_pretty(&results).unwrap()).unwrap();
}

#[test]
#[ignore = "opt-in camera timeline profiling"]
fn camera_timeline_profile() {
    let duration_ms = 600_000;
    let events: Vec<CursorEvent> = (0..36_000)
        .map(|i| CursorEvent { is_release: false,
            timestamp_ms: i * 1000 / 60,
            x: 0.5 + (i as f64 / 180.0).sin() * 0.35,
            y: 0.5 + (i as f64 / 270.0).cos() * 0.35,
            is_click: i % 180 == 0,
        })
        .collect();
    let blocks: Vec<ZoomBlock> = (0..200)
        .map(|i| {
            serde_json::from_value(serde_json::json!({
                "start_time_ms": i * 3000, "end_time_ms": i * 3000 + 2500,
                "zoom_level": 2, "center_x": 0.5, "center_y": 0.5,
                "centers": [{"time": i * 3000, "x": 0.4, "y": 0.5},
                            {"time": i * 3000 + 1000, "x": 0.6, "y": 0.5}]
            }))
            .unwrap()
        })
        .collect();
    let config = SpringConfig {
        tension: 250.0,
        friction: 40.0,
        mass: 2.25,
    };
    let mut runs = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        let legacy = simulate_zoom_steps(
            &blocks,
            &events,
            &config,
            true,
            "mellow",
            60.0,
            duration_ms,
            true,
            CameraViewport::default(),None,
        );
        let legacy_ms = start.elapsed().as_secs_f64() * 1000.0;
        std::hint::black_box(&legacy);
        let start = Instant::now();
        let timeline = simulate_zoom_trajectory(
            &blocks,
            &events,
            &config,
            true,
            "mellow",
            120.0,
            duration_ms,
            true,
        );
        let simulation_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let compact: Vec<[f64; 3]> = timeline
            .iter()
            .map(|f| [f.scale, f.center_x, f.center_y])
            .collect();
        let serialized = serde_json::to_vec(&compact).unwrap();
        let serialization_ms = start.elapsed().as_secs_f64() * 1000.0;
        runs.push(serde_json::json!({"legacy_60hz_ms": legacy_ms, "fixed_120hz_ms": simulation_ms,
            "serialization_ms": serialization_ms, "json_bytes": serialized.len(), "frames": timeline.len()}));
    }
    println!(
        "CAMERA_PROFILE {}",
        serde_json::json!({"build": if cfg!(debug_assertions) {"debug"} else {"release"},
        "duration_ms": duration_ms, "events": events.len(), "blocks": blocks.len(), "runs": runs})
    );
}

#[tokio::test]
#[ignore = "opt-in existing-recording camera profile; set TARANTINO_PROFILE_VIDEO"]
async fn recorded_camera_command_profile() {
    use crate::{commands::motion::compute_zoom_trajectory, recording::artifacts::RecordingArtifacts};
    let input = std::env::var("TARANTINO_PROFILE_VIDEO").expect("set TARANTINO_PROFILE_VIDEO");
    let info = crate::video_processing::VideoProcessor::new().unwrap()
        .get_video_info(&input).await.unwrap();
    let artifacts = RecordingArtifacts::new(&input);
    let raw = std::fs::read(artifacts.mouse()).unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let analysis: serde_json::Value = serde_json::from_slice(&std::fs::read(artifacts.auto_zoom()).unwrap()).unwrap();
    let blocks: Vec<_> = analysis["zoom_blocks"].as_array().unwrap().iter().map(|block| {
        let mut mapped = block.clone();
        let fields = mapped.as_object_mut().unwrap();
        for (from, to) in [("start_time", "start_time_ms"), ("end_time", "end_time_ms"), ("zoom_factor", "zoom_level")] {
            let value = fields.remove(from).unwrap();
            fields.insert(to.into(), value);
        }
        mapped
    }).collect();
    assert!(!blocks.is_empty(), "profile requires recorded zoom blocks");
    let settings: crate::video_processing::ExportSettings = serde_json::from_value(serde_json::json!({
        "source_width":info.width, "source_height":info.height,
        "resolution":{"width":info.width,"height":info.height},
        "capture_mode":metadata["capture_mode"].as_str().unwrap_or("display"),
        "zoom_blocks":blocks, "screen_motion_style":"focused",
        "zoom_spring_tension":250, "zoom_spring_friction":40, "zoom_spring_mass":2.25
    })).unwrap();
    let reference = compute_zoom_trajectory(input.clone(), info.duration_ms, settings.clone()).await.unwrap();
    assert_eq!(reference.len(), (info.duration_ms as f64 * 0.12).ceil() as usize);
    assert!(reference.iter().flatten().all(|value| value.is_finite()));
    assert!(reference.iter().any(|frame| frame[0] > 1.1));
    let mut runs = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let frames = compute_zoom_trajectory(input.clone(), info.duration_ms, settings.clone()).await.unwrap();
        let command_ms = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(frames, reference, "repeated computation must preserve the camera path");
        let start = Instant::now();
        let encoded = serde_json::to_vec(&frames).unwrap();
        runs.push(serde_json::json!({"command_ms":command_ms,
            "serialization_ms":start.elapsed().as_secs_f64()*1000.0,"json_bytes":encoded.len()}));
    }
    assert_eq!(std::fs::read(artifacts.mouse()).unwrap(), raw);
    println!("RECORDED_CAMERA_PROFILE {}", serde_json::json!({
        "build":if cfg!(debug_assertions){"debug"}else{"release"},
        "source":std::path::Path::new(&input).file_name().unwrap().to_string_lossy(),
        "duration_ms":info.duration_ms,"frames":reference.len(),"blocks":blocks.len(),
        "raw_events":metadata["mouse_events"].as_array().map(Vec::len),"warmups":1,"runs":runs
    }));
}
