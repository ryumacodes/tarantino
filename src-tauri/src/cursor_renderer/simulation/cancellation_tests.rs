use super::*;
use std::cell::Cell;

fn events() -> Vec<CursorEvent> {
    vec![
        CursorEvent { timestamp_ms: 0, x: 0.2, y: 0.3, is_click: false, is_release: false },
        CursorEvent { timestamp_ms: 600, x: 0.8, y: 0.7, is_click: true, is_release: false },
        CursorEvent { timestamp_ms: 900, x: 0.4, y: 0.3, is_click: false, is_release: true },
    ]
}

#[test]
fn cancellation_stops_simulation_before_producing_the_remaining_frames() {
    let settings = serde_json::from_value(serde_json::json!({})).unwrap();
    let spring = SpringConfig { tension: 470.0, friction: 70.0, mass: 3.0 };
    for limit in [1, 4] {
        let checks = Cell::new(0);
        let frames = Cell::new(0);
        let check = || {
            checks.set(checks.get() + 1);
            anyhow::ensure!(checks.get() < limit, "cancelled");
            Ok(())
        };
        let result = simulate_cursor_frames_checked(&events(), &spring, 120.0, 60000,
            &None, &settings, 0, 120.0, None, None, None, None, None, Some(&check), false, |_| {
                frames.set(frames.get() + 1);
            });
        assert!(result.is_err());
        assert_eq!(checks.get(), limit);
        if limit == 1 { assert_eq!(frames.get(), 0); }
        else { assert!(frames.get() > 0 && frames.get() < 1024); }
    }
}

#[test]
fn checkpoints_preserve_completed_frames() {
    let settings = serde_json::from_value(serde_json::json!({
        "rotate_while_moving":true,"hide_when_idle":true,"idle_timeout":500,
        "stop_at_end":true,"stop_duration_ms":200,"loop_to_start":true
    })).unwrap();
    let spring = SpringConfig { tension: 470.0, friction: 70.0, mass: 3.0 };
    let project = |p: &CursorFrameState| [p.x,p.y,p.opacity,p.is_clicking,p.ripple_progress,
        p.ripple_x,p.ripple_y,p.rotation,p.fade_blur_radius,p.press_amount];
    let check = || Ok(());
    for until in [None, Some(1501)] {
        let expected = simulate_cursor_frames_until(&events(), &spring, 120.0, 5000,
            &None, &settings, 0, 120.0, None, None, None, None, until, project);
        let actual = simulate_cursor_frames_checked(&events(), &spring, 120.0, 5000,
            &None, &settings, 0, 120.0, None, None, None, None, until, Some(&check), false, project).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn edited_preview_propagates_cancellation_from_local_simulation() {
    let clips: Vec<crate::video_processing::timeline::EditClip> = serde_json::from_value(serde_json::json!([
        {"id":"fast","startTime":0,"duration":30000,"sourceIn":0,"sourceOut":60000,
         "playbackRate":2,"trackOrder":0,"muted":false}
    ])).unwrap();
    let timeline = crate::video_processing::timeline::EditTimeline::build(&clips, 0.0, 30000.0).unwrap();
    let settings = serde_json::from_value(serde_json::json!({})).unwrap();
    let checks = Cell::new(0);
    let check = || {
        checks.set(checks.get() + 1);
        anyhow::ensure!(checks.get() < 5, "cancelled inside local simulation");
        Ok(())
    };
    let result = super::super::edited_simulation::simulate_with_planned_path_checked(
        &events(), &[], &timeline, "recording.mp4",
        &SpringConfig { tension: 470.0, friction: 70.0, mass: 3.0 },
        &settings, &None, 120.0, None, None, 60.0, None, Some(&check));
    assert!(result.unwrap_err().to_string().contains("cancelled inside local simulation"));
    assert_eq!(checks.get(), 5);
}

#[test]
fn deferred_visibility_preserves_motion_and_trails_after_global_fade() {
    let settings = serde_json::from_value(serde_json::json!({
        "trail_enabled":true,"trail_opacity":0.7,"hide_when_idle":true,"idle_timeout":250,
        "rotate_while_moving":true,"stop_at_end":true,"stop_duration_ms":200
    })).unwrap();
    let spring = SpringConfig { tension: 470.0, friction: 70.0, mass: 3.0 };
    let calls = Cell::new(0);
    let typing = |time| { calls.set(calls.get() + 1); (200.0..800.0).contains(&time) };
    let run = |defer| simulate_cursor_frames_checked(&events(), &spring, 120.0, 2000,
        &None, &settings, 0, 120.0, None, Some(&typing), None, None, None, None, defer, Clone::clone).unwrap();
    let expected = run(false);
    assert!(calls.get() > 0);
    calls.set(0);
    let actual = run(true);
    assert_eq!(calls.get(), 0);
    assert_eq!(actual.len(), expected.len());
    for (index, (mut actual, mut expected)) in actual.into_iter().zip(expected).enumerate() {
        let opacity = [0.0, 0.25, 1.0][index % 3];
        actual.set_visibility(opacity, 3.0, 0.7);
        expected.set_visibility(opacity, 3.0, 0.7);
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"), "frame {index}");
    }
}
