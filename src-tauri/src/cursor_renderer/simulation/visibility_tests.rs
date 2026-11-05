use super::*;

fn frames(events: &[CursorEvent], idle: bool, fps: f64) -> Vec<CursorFrameState> {
    let settings = serde_json::from_value(serde_json::json!({
        "hide_when_idle": idle, "idle_timeout": 200, "trail_enabled": true
    })).unwrap();
    simulate_cursor_positions(events, &SpringConfig { tension: 170.0, friction: 30.0, mass: 1.0 },
        fps, 3000, &None, &settings)
}

#[test]
fn missing_or_future_events_do_not_draw_a_phantom_cursor_or_trail() {
    let events = [CursorEvent { is_release: false, timestamp_ms: 1000, x: 0.8, y: 0.7, is_click: false }];
    for fps in [24.0, 30.0, 60.0, 120.0] {
        for idle in [false, true] {
            let empty = frames(&[], idle, fps);
            assert!(empty.iter().all(|frame| frame.opacity == 0.0 && frame.trail_count == 0.0));
            let captured = frames(&events, idle, fps);
            assert!(captured[..fps as usize].iter().all(|frame| {
                frame.opacity == 0.0 && frame.trail_count == 0.0 && frame.is_clicking == 0.0
            }));
            assert_eq!(captured[fps as usize].opacity, 1.0);
        }
    }
}

#[test]
fn stationary_click_wakes_an_idle_cursor_with_interpolated_opacity() {
    let events = [
        CursorEvent { is_release: false, timestamp_ms: 0, x: 0.5, y: 0.5, is_click: false },
        CursorEvent { is_release: false, timestamp_ms: 2000, x: 0.5, y: 0.5, is_click: true },
    ];
    for fps in [24.0, 30.0, 60.0, 120.0] {
        let captured = frames(&events, true, fps);
        assert!(captured[fps as usize].opacity < 0.002);
        let prior = &captured[(fps * 2.0) as usize - 1];
        let wake = &captured[(fps * 2.0) as usize];
        if fps == 120.0 { assert!((prior.opacity-wake.opacity/2.0).abs()<1e-7); }
        else { assert_eq!(prior.opacity,0.0); }
        assert_eq!(prior.is_clicking,0.0);
        assert!(captured[(fps * 2.0) as usize].opacity > 0.0);
        assert_eq!(captured[(fps * 2.0) as usize].is_clicking, 1.0);
    }
}

#[test]
#[ignore = "requires external visibility spring fixtures"]
fn production_visibility_matches_sixty_hz_timeline_at_multiple_output_rates() {
    #[derive(serde::Deserialize)]
    struct Sample { opacity: f64, blur: f64 }
    #[derive(serde::Deserialize)]
    struct Case { fps: f64, samples: Vec<Sample> }
    let path = std::env::var("TARANTINO_VISIBILITY_FIXTURES").unwrap();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let expected = &cases.iter().find(|case|case.fps==60.0).unwrap().samples;
    let mut events = vec![CursorEvent { timestamp_ms:0, x:0.5, y:0.5, is_click:false, is_release:false }];
    events.extend((900..=3100).step_by(100).map(|timestamp_ms| CursorEvent {
        timestamp_ms, x:0.5, y:0.5, is_click:false, is_release:false,
    }));
    let settings = serde_json::from_value(serde_json::json!({"hide_when_idle":true,"idle_timeout":300})).unwrap();
    let typing = |time| (300.0..500.0).contains(&time);
    let transport = simulate_cursor_frames_with_activity(&events,
        &SpringConfig { tension:170.0, friction:30.0, mass:1.0 }, 120.0,3001,&None,&settings,
        None,Some(&typing),Clone::clone);
    for fps in [24.0,30.0,60.0,120.0,144.0] {
        let frames = simulate_cursor_frames_with_activity(&events,
            &SpringConfig { tension:170.0, friction:30.0, mass:1.0 }, fps,3000,&None,&settings,
            None,Some(&typing),Clone::clone);
        for (index, actual) in frames.iter().enumerate() {
            let position = index as f64*60.0/fps;
            let lower = position.floor() as usize;
            let fraction = position-lower as f64;
            let a = &expected[lower]; let b = &expected[(lower+1).min(expected.len()-1)];
            let opacity = a.opacity+(b.opacity-a.opacity)*fraction;
            let blur = a.blur+(b.blur-a.blur)*fraction;
            assert!((actual.opacity as f64-opacity).abs()<1e-6,"{fps}fps frame {index}: opacity {} != {opacity}",actual.opacity);
            assert!((actual.fade_blur_radius as f64-blur).abs()<1e-6,"{fps}fps frame {index}: blur {} != {blur}",actual.fade_blur_radius);
            let tick = index as f64 * 120.0 / fps;
            let start = tick.floor() as usize;
            let resampled = transport[start].interpolate(&transport[start+1], (tick-start as f64) as f32);
            assert!((resampled.opacity as f64-opacity).abs()<1e-6);
            assert!((resampled.fade_blur_radius as f64-blur).abs()<1e-6);
        }
    }
}
