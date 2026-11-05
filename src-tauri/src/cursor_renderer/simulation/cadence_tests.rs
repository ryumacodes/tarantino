use super::*;

    #[test]
    fn trails_allocate_only_when_enabled_and_never_exceed_shader_capacity() {
        let spring = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        let settings = |enabled, length| {
            serde_json::from_value::<CursorSettings>(
                serde_json::json!({"trail_enabled":enabled,"trail_length":length}),
            )
            .unwrap()
        };
        let disabled =
            simulate_cursor_positions(&[], &spring, 120.0, 1000, &None, &settings(false, 30));
        assert!(
            disabled
                .iter()
                .all(|s| s.trail_points.is_none() && s.trail_count == 0.0)
        );
        assert!(std::mem::size_of::<CursorFrameState>() <= 80);
        let events = [CursorEvent { is_release: false, timestamp_ms: 0, x: 0.5, y: 0.5, is_click: false }];
        let enabled =
            simulate_cursor_positions(&events, &spring, 120.0, 1000, &None, &settings(true, 1000));
        for state in &enabled {
            assert_eq!(state.trail_count as usize, state.trail_points().len());
            assert!(state.trail_points().len() <= 30);
        }
        assert_eq!(enabled.last().unwrap().trail_points().len(), 30);
        assert!(std::sync::Arc::ptr_eq(
            enabled[0].trail_points.as_ref().unwrap(),
            enabled[1].trail_points.as_ref().unwrap()
        ));
    }

    #[test]
    fn trimmed_cursor_samples_keep_source_history_without_storing_discarded_frames() {
        let events = [
            CursorEvent { is_release: false,
                timestamp_ms: 0,
                x: 0.2,
                y: 0.3,
                is_click: false,
            },
            CursorEvent { is_release: false,
                timestamp_ms: 1800,
                x: 0.8,
                y: 0.7,
                is_click: true,
            },
        ];
        let spring = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        let settings: CursorSettings = serde_json::from_value(serde_json::json!({})).unwrap();
        let source = simulate_cursor_positions(&events, &spring, 120.0, 3000, &None, &settings);
        let range = crate::video_processing::export_range::ExportRange {
            start_ms: 1253,
            end_ms: 2253,
        };
        for fps in [24, 30, 60, 120] {
            let trimmed = simulate_cursor_positions_for_range(
                &events, &spring, fps as f64, range, &None, &settings,
            );
            assert_eq!(trimmed.len(), fps as usize);
            for (i, frame) in trimmed.iter().enumerate() {
                let position = (range.start_ms as f64 + i as f64 * 1000.0 / fps as f64) * 120.0 / 1000.0;
                let lower = position.floor() as usize;
                let expected = source[lower].interpolate(&source[lower + 1], (position - lower as f64) as f32);
                assert_eq!(
                    (frame.x, frame.y, frame.ripple_progress),
                    (expected.x, expected.y, expected.ripple_progress)
                );
            }
        }
    }

    #[test]
    fn raw_cursor_and_effect_states_match_at_all_output_rates() {
        let events = [
            CursorEvent { is_release: false,
                timestamp_ms: 0,
                x: 0.2,
                y: 0.3,
                is_click: false,
            },
            CursorEvent { is_release: false,
                timestamp_ms: 500,
                x: 0.8,
                y: 0.7,
                is_click: true,
            },
            CursorEvent { is_release: false,
                timestamp_ms: 1000,
                x: 0.3,
                y: 0.4,
                is_click: false,
            },
        ];
        let spring = SpringConfig {
            tension: 530.0,
            friction: 40.0,
            mass: 1.0,
        };
        let settings: CursorSettings = serde_json::from_value(serde_json::json!({
            "trail_enabled": true, "rotate_while_moving": true,
            "hide_when_idle": true, "idle_timeout": 200
        }))
        .unwrap();
        let preview = simulate_cursor_positions(&events, &spring, 120.0, 2000, &None, &settings);
        for fps in [24.0, 30.0, 60.0, 120.0] {
            let output = simulate_cursor_positions(&events, &spring, fps, 2000, &None, &settings);
            assert_eq!(output.len(), (fps * 2.0) as usize);
            for (i, actual) in output.iter().enumerate() {
                let expected = &preview[(i as f64 * 120.0 / fps).round() as usize];
                assert_eq!(
                    (
                        actual.x,
                        actual.y,
                        actual.opacity,
                        actual.rotation,
                        actual.ripple_progress
                    ),
                    (
                        expected.x,
                        expected.y,
                        expected.opacity,
                        expected.rotation,
                        expected.ripple_progress
                    )
                );
                assert_eq!(actual.trail_count, expected.trail_count);
                for (a, b) in actual
                    .trail_points()
                    .iter()
                    .zip(expected.trail_points().iter())
                {
                    assert_eq!((a.x, a.y, a.alpha, a.size), (b.x, b.y, b.alpha, b.size));
                }
            }
        }
        let compact =
            simulate_cursor_frames(&events, &spring, 120.0, 2000, &None, &settings, |p| {
                [p.x, p.y]
            });
        assert_eq!(
            compact,
            preview.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>()
        );
    }

#[test]
fn physical_rotation_keeps_history_across_output_rates_and_trim() {
    use crate::cursor_renderer::rotation::{MotionPoint,horizontal_tilt};
    let points=[MotionPoint{time_ms:0.0,x:0.0,y:0.0},MotionPoint{time_ms:1000.0,x:1000.0,y:0.0}];
    let keys=horizontal_tilt(&points,1.0);
    let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.0,y:0.0,is_click:false},
        CursorEvent{ is_release: false,timestamp_ms:1000,x:1.0,y:0.0,is_click:false}];
    let settings:CursorSettings=serde_json::from_value(serde_json::json!({"rotation":15,"rotate_while_moving":true})).unwrap();
    let spring=SpringConfig{tension:170.0,friction:30.0,mass:1.0};
    let full=simulate_cursor_frames_with_rotation(&events,&spring,120.0,2000,&None,&settings,Some(&keys),|p|p.rotation);
    for fps in [24,30,60,120] {
        let trimmed=simulate_cursor_positions_for_range_with_rotation(&events,&spring,fps as f64,
            crate::video_processing::export_range::ExportRange{start_ms:500,end_ms:2000},&None,&settings,Some(&keys));
        for (index,frame) in trimmed.iter().enumerate() {
            assert_eq!(frame.rotation,full[60+index*120/fps]);
        }
    }
    assert!((full[60]-27.0).abs()<0.001);
}

#[test]
fn loop_return_position_and_tilt_follow_the_same_retained_path() {
    use crate::cursor_renderer::{rotation::{MotionPoint,horizontal_tilt},loop_path};
    let points=[MotionPoint{time_ms:0.0,x:200.0,y:0.0},MotionPoint{time_ms:1000.0,x:800.0,y:0.0},
        MotionPoint{time_ms:1800.0,x:900.0,y:0.0}];
    let events=points.iter().map(|p|CursorEvent{ is_release: false,timestamp_ms:p.time_ms as u64,x:p.x/1000.0,y:0.0,is_click:false}).collect::<Vec<_>>();
    let keys=horizontal_tilt(&loop_path::rotation_points(&points,2000,500),1.0);
    let settings:CursorSettings=serde_json::from_value(serde_json::json!({
        "rotate_while_moving":true,"loop_to_start":true,"loop_duration_ms":500,"click_effect":"ripple"
    })).unwrap();
    let frames=simulate_cursor_frames_with_rotation(&events,&SpringConfig{tension:170.0,friction:30.0,mass:0.0},120.0,
        2000,&None,&settings,Some(&keys),Clone::clone);
    assert!((frames[120].x-0.8).abs()<0.00001);
    assert!((frames[121].x-0.2).abs()<0.00001);
    assert!(frames[160].rotation<0.0,"return to the left must tilt left");
    assert!(frames.iter().all(|frame|frame.is_clicking==0.0 && frame.ripple_progress==0.0));
    assert!((frames.last().unwrap().x-0.2).abs()<0.00001);
}

#[test]
fn fractional_output_samples_blend_motion_but_keep_discrete_click_state() {
    let events = [
        CursorEvent { timestamp_ms: 0, x: 0.1, y: 0.2, is_click: false, is_release: false },
        CursorEvent { timestamp_ms: 250, x: 0.9, y: 0.8, is_click: true, is_release: false },
    ];
    let spring = SpringConfig { tension: 530.0, friction: 40.0, mass: 1.0 };
    let settings: CursorSettings = serde_json::from_value(serde_json::json!({})).unwrap();
    let source = simulate_cursor_positions(&events, &spring, 120.0, 1000, &None, &settings);
    let output = simulate_cursor_positions(&events, &spring, 144.0, 900, &None, &settings);
    for (index, actual) in output.iter().enumerate() {
        let position = index as f64 * 120.0 / 144.0;
        let lower = position.floor() as usize;
        let expected = source[lower].interpolate(&source[lower+1], (position-lower as f64) as f32);
        assert!((actual.x-expected.x).abs()<1e-6);
        assert!((actual.y-expected.y).abs()<1e-6);
        assert_eq!(actual.is_clicking, source[lower].is_clicking);
        assert_eq!(actual.ripple_progress, source[lower].ripple_progress);
    }
}

#[test]
#[ignore = "requires external angular timeline fixtures"]
fn production_rotation_matches_sixty_hz_timeline() {
    #[derive(serde::Deserialize)]
    struct Case { keys: Vec<[f64;2]>, samples: Vec<f64> }
    let path = std::env::var("TARANTINO_ROTATION_TIMELINE_FIXTURES").unwrap();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(!cases.is_empty());
    let events = [CursorEvent { timestamp_ms:0, x:0.5, y:0.5, is_click:false, is_release:false }];
    let settings = serde_json::from_value(serde_json::json!({"rotate_while_moving":true,"rotation":7})).unwrap();
    let spring = SpringConfig { tension:170.0, friction:30.0, mass:1.0 };
    for case in cases {
        let keys = case.keys.iter().map(|&[time_ms,degrees]| super::super::rotation::RotationKey {time_ms,degrees}).collect::<Vec<_>>();
        for fps in [24.0,30.0,60.0,120.0,144.0] {
            let frames = simulate_cursor_frames_with_rotation(&events,&spring,fps,3000,&None,&settings,Some(&keys),Clone::clone);
            for (index, frame) in frames.iter().enumerate() {
                let position = index as f64*60.0/fps;
                let lower = position.floor() as usize;
                let expected = 7.0+case.samples[lower]+(case.samples[lower+1]-case.samples[lower])*(position-lower as f64);
                assert!((frame.rotation as f64-expected).abs()<1e-5,"{fps}fps frame {index}: {} != {expected}",frame.rotation);
            }
        }
    }
}
