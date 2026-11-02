use super::*;

#[test]
fn release_events_preserve_the_preceding_typing_caret_update() {
    let mut tracker = MouseTracker::new();
    tracker.is_tracking = true;
    tracker.add_key_event(serde_json::from_value(serde_json::json!({
        "timestamp":100,"is_modifier":false,"is_typing":true})).unwrap());
    tracker.add_key_event(KeyEvent::release(150, false));
    tracker.update_latest_typing_caret(20.0, 30.0);
    tracker.stop_tracking();
    tracker.add_key_event(KeyEvent::release(200, false));
    let keys = tracker.get_key_events();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].caret_x, Some(20.0));
    assert_eq!(keys[0].caret_y, Some(30.0));
    assert!(keys[1].is_release);
    assert_eq!(keys[1].caret_x, None);
}

#[test]
fn release_after_a_shortcut_does_not_retarget_earlier_typing() {
    let mut tracker = MouseTracker::new();
    tracker.is_tracking = true;
    tracker.add_key_event(serde_json::from_value(serde_json::json!({
        "timestamp":100,"is_modifier":false,"is_typing":true,
        "caret_x":20.0,"caret_y":30.0})).unwrap());
    // A command chord can release its modifier before its character key.
    // That character release must not rewrite the earlier sentence's target.
    tracker.add_key_event(serde_json::from_value(serde_json::json!({
        "timestamp":200,"is_modifier":false,"is_typing":false})).unwrap());
    tracker.add_key_event(KeyEvent::release(220, true));
    tracker.add_key_event(KeyEvent::release(230, false));
    tracker.update_latest_typing_caret(1200.0, 900.0);
    let keys = tracker.get_key_events();
    assert_eq!((keys[0].caret_x, keys[0].caret_y), (Some(20.0), Some(30.0)));
    assert_eq!(keys[1].caret_x, None);
}

fn movement(timestamp: u64, x: f64, y: f64) -> MouseEvent {
    MouseEvent {
        timestamp,
        x,
        y,
        event_type: MouseEventType::Move,
        display_id: None,
    }
}

#[test]
fn preserves_last_warmup_position_at_video_start() {
    let events = vec![
        movement(100, 10.0, 20.0),
        movement(700, 30.0, 40.0),
        movement(1500, 50.0, 60.0),
    ];

    let normalized = normalize_mouse_events_for_timeline(&events, 1000);

    assert_eq!(normalized.len(), 2);
    assert_eq!(normalized[0].timestamp, 0);
    assert_eq!((normalized[0].x, normalized[0].y), (30.0, 40.0));
    assert!(matches!(normalized[0].event_type, MouseEventType::Move));
    assert_eq!(normalized[1].timestamp, 500);
}

#[test]
fn exact_start_event_takes_precedence_over_warmup_position() {
    let events = vec![movement(700, 30.0, 40.0), movement(1000, 50.0, 60.0)];

    let normalized = normalize_mouse_events_for_timeline(&events, 1000);

    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0].timestamp, 0);
    assert_eq!((normalized[0].x, normalized[0].y), (50.0, 60.0));
}

#[test]
fn long_recordings_preserve_early_pointer_and_typing_events() {
    let mut tracker = MouseTracker::new();
    tracker.is_tracking = true;
    let mut click = movement(1000, 12.0, 34.0);
    click.event_type = MouseEventType::ButtonPress { button: MouseButton::Left };
    tracker.add_event(click);
    for i in 1..=100_100 {
        tracker.add_event(movement(1000 + i, i as f64, 20.0));
        tracker.add_key_event(KeyEvent { shortcut: None,
            is_release: false, timestamp: 1000 + i,
            is_modifier: false,
            is_typing: true,
            key_motion: KeyMotion::Character,
            cursor_x: Some(i as f64), cursor_y: Some(20.0),
            caret_x: None, caret_y: None,
        });
    }
    tracker.stop_tracking();
    let mouse = tracker.get_events();
    assert_eq!(mouse.len(), 100_101);
    assert!(matches!(mouse[0].event_type, MouseEventType::ButtonPress { .. }));
    let normalized = normalize_mouse_events_for_timeline(&mouse, 1000);
    assert_eq!(normalized[0].timestamp, 0);
    assert_eq!((normalized[0].x, normalized[0].y), (12.0, 34.0));
    assert_eq!(normalized.last().unwrap().timestamp, 100_100);
    let keys = tracker.get_key_events();
    assert_eq!(keys.len(), 100_100);
    assert_eq!(keys[0].timestamp, 1001);
    assert_eq!(keys.last().unwrap().timestamp, 101_100);
    // Sidecar serialization must retain the complete timeline, too.
    let restored: Vec<MouseEvent> = serde_json::from_str(&serde_json::to_string(&normalized).unwrap()).unwrap();
    assert_eq!(restored.len(), normalized.len());
    assert_eq!(restored[0].timestamp, 0);
}

#[test]
fn stationary_recording_keeps_initial_position_without_a_motion_callback() {
    let mut tracker = MouseTracker::new();
    tracker.is_tracking = true;
    tracker.seed_pointer_position((-210.5, 340.25), 990);
    tracker.stop_tracking();
    let events = normalize_mouse_events_for_timeline(&tracker.get_events(), 1000);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].timestamp, 0);
    assert_eq!((events[0].x, events[0].y), (-210.5, 340.25));
    assert!(matches!(events[0].event_type, MouseEventType::Move));
}

#[test]
fn initial_pointer_sample_does_not_create_clicks_or_shift_later_motion() {
    let mut tracker = MouseTracker::new();
    tracker.is_tracking = true;
    tracker.seed_pointer_position((120.0, 240.0), 1000);
    tracker.add_event(movement(9700, 800.0, 600.0));
    let events = normalize_mouse_events_for_timeline(&tracker.get_events(), 1000);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].timestamp, 0);
    assert_eq!(events[1].timestamp, 8700);
    assert_eq!(events[0].x, 120.0);
    tracker.seed_pointer_position((f64::NAN, 0.0), 9800);
    tracker.stop_tracking();
    tracker.seed_pointer_position((0.0, 0.0), 9900);
    assert_eq!(tracker.get_events().len(), 2);
}
