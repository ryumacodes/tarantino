use super::*;

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
