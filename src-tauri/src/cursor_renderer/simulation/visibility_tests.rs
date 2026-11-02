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
fn stationary_click_wakes_an_idle_cursor_without_future_activity_anticipation() {
    let events = [
        CursorEvent { is_release: false, timestamp_ms: 0, x: 0.5, y: 0.5, is_click: false },
        CursorEvent { is_release: false, timestamp_ms: 2000, x: 0.5, y: 0.5, is_click: true },
    ];
    for fps in [24.0, 30.0, 60.0, 120.0] {
        let captured = frames(&events, true, fps);
        assert!(captured[fps as usize].opacity < 0.002);
        assert_eq!(captured[(fps * 2.0) as usize - 1].opacity, 0.0);
        assert!(captured[(fps * 2.0) as usize].opacity > 0.0);
        assert_eq!(captured[(fps * 2.0) as usize].is_clicking, 1.0);
    }
}
