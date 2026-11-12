use crate::input::{
    KeyEvent, MouseTracker, caret_lookup, classify_key, key_motion, keyboard_dispatch,
};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventType, EventField};
use parking_lot::Mutex;
use std::{
    sync::{Arc, mpsc::Sender},
    time::SystemTime,
};

pub(super) struct Input {
    time: SystemTime,
    session: SystemTime,
    event: KeyEvent,
    query_caret: bool,
}

pub(super) fn capture(
    kind: CGEventType,
    event: &CGEvent,
    time: SystemTime,
    timestamp: u64,
    session: SystemTime,
    pointer: (f64, f64),
) -> Option<Input> {
    use CGEventFlags as F;
    let code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
    let key = rdev::macos_key_from_code(code as u16);
    let (modifier, character) = classify_key(&key);
    let flags = event.get_flags();
    let command =
        flags.intersects(F::CGEventFlagCommand | F::CGEventFlagControl | F::CGEventFlagAlternate);
    let mut captured = KeyEvent::release(timestamp, modifier);
    match kind {
        CGEventType::KeyDown => {
            captured.is_release = false;
            captured.is_typing = character && !command;
            captured.key_motion = key_motion(&key);
            captured.cursor_x = Some(pointer.0);
            captured.cursor_y = Some(pointer.1);
        }
        CGEventType::KeyUp => {}
        CGEventType::FlagsChanged => {
            // Typing uses event-local flags; modifier observations never query AX.
            let flag = match key {
                rdev::Key::MetaLeft | rdev::Key::MetaRight => F::CGEventFlagCommand,
                rdev::Key::ControlLeft | rdev::Key::ControlRight => F::CGEventFlagControl,
                rdev::Key::Alt | rdev::Key::AltGr => F::CGEventFlagAlternate,
                rdev::Key::ShiftLeft | rdev::Key::ShiftRight => F::CGEventFlagShift,
                rdev::Key::CapsLock => F::CGEventFlagAlphaShift,
                _ => return None,
            };
            captured.is_release = !flags.contains(flag);
        }
        _ => return None,
    }
    Some(Input {
        time,
        session,
        event: captured,
        query_caret: !matches!(kind, CGEventType::FlagsChanged) && character && !command,
    })
}

pub(super) fn start(tracker: Arc<Mutex<MouseTracker>>) -> Sender<Input> {
    keyboard_dispatch::worker(move |input| process(&tracker, input, super::focused_caret_position))
}

fn process(
    tracker: &Mutex<MouseTracker>,
    mut input: Input,
    query: impl FnOnce() -> Option<(f64, f64)>,
) {
    let mut state = tracker.lock();
    if !state.is_tracking
        || state.recording_start_time != Some(input.session)
        || input.time < input.session
    {
        return;
    }
    let caret = if input.query_caret {
        match caret_lookup::without_tracker_lock(&mut state, || {
            caret_lookup::for_event(input.time, query)
        }) {
            Ok(caret) => caret.filter(|_| !state.focus_changed_since(input.event.timestamp)),
            Err(()) => return,
        }
    } else {
        None
    };
    if input.event.is_typing {
        input.event.caret_x = caret.map(|point| point.0);
        input.event.caret_y = caret.map(|point| point.1);
    }
    let release = input.event.is_release;
    state.add_key_event(input.event);
    if release {
        if let Some((x, y)) = caret {
            state.update_latest_typing_caret(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use std::time::{Duration, UNIX_EPOCH};

    fn input(code: u16, kind: CGEventType, flags: CGEventFlags) -> Input {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).unwrap();
        let event = CGEvent::new_keyboard_event(source, code, matches!(kind, CGEventType::KeyDown))
            .unwrap();
        event.set_flags(flags);
        capture(
            kind,
            &event,
            SystemTime::now(),
            123,
            UNIX_EPOCH,
            (10.0, 20.0),
        )
        .unwrap()
    }

    #[test]
    fn typing_uses_current_flags_without_prior_modifier_events() {
        use CGEventFlags as F;
        for flags in [F::empty(), F::CGEventFlagShift, F::CGEventFlagAlphaShift] {
            let sample = input(0, CGEventType::KeyDown, flags);
            assert!(sample.event.is_typing);
            assert!(sample.query_caret);
            assert_eq!(sample.event.timestamp, 123);
            assert_eq!(
                (sample.event.cursor_x, sample.event.cursor_y),
                (Some(10.0), Some(20.0))
            );
            assert!(sample.event.shortcut.is_none());
        }
        for flags in [
            F::CGEventFlagCommand,
            F::CGEventFlagControl,
            F::CGEventFlagAlternate,
        ] {
            let sample = input(0, CGEventType::KeyDown, flags);
            assert!(!sample.event.is_typing);
            assert!(!sample.query_caret);
        }
    }

    #[test]
    fn navigation_releases_and_motion_categories_are_preserved() {
        for (code, motion) in [
            (36, crate::input::KeyMotion::Newline),
            (51, crate::input::KeyMotion::Backspace),
            (48, crate::input::KeyMotion::Tab),
        ] {
            assert_eq!(
                input(code, CGEventType::KeyDown, CGEventFlags::empty())
                    .event
                    .key_motion,
                motion
            );
        }
        let arrow = input(123, CGEventType::KeyDown, CGEventFlags::empty());
        assert!(!arrow.event.is_typing);
        assert!(!arrow.query_caret);
        let release = input(0, CGEventType::KeyUp, CGEventFlags::empty());
        assert!(release.event.is_release);
        assert!(!release.event.is_typing);
        assert!(release.query_caret);
    }

    #[test]
    fn queued_keys_keep_order_and_release_can_refine_caret() {
        let tracker = Mutex::new(MouseTracker::new());
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(UNIX_EPOCH);
        }
        process(
            &tracker,
            input(0, CGEventType::KeyDown, CGEventFlags::empty()),
            || {
                assert!(tracker.try_lock().is_some());
                Some((100.0, 200.0))
            },
        );
        process(
            &tracker,
            input(0, CGEventType::KeyUp, CGEventFlags::empty()),
            || Some((110.0, 200.0)),
        );
        let events = tracker.lock().get_key_events();
        assert_eq!(events.len(), 2);
        assert!(events[0].is_typing);
        assert_eq!(events[0].caret_x, Some(110.0));
        assert!(events[1].is_release);
    }

    #[test]
    fn repeats_are_retained_even_when_caret_queries_are_stale() {
        let tracker = Mutex::new(MouseTracker::new());
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(UNIX_EPOCH);
        }
        for timestamp in [100, 130, 160] {
            let mut sample = input(0, CGEventType::KeyDown, CGEventFlags::empty());
            sample.time = UNIX_EPOCH + Duration::from_millis(timestamp);
            sample.event.timestamp = timestamp;
            process(&tracker, sample, || {
                panic!("stale input must not query today's caret")
            });
        }
        let events = tracker.lock().get_key_events();
        assert_eq!(
            events
                .iter()
                .map(|event| event.timestamp)
                .collect::<Vec<_>>(),
            [100, 130, 160]
        );
        assert!(
            events
                .iter()
                .all(|event| event.is_typing && event.caret_x.is_none())
        );
    }

    #[test]
    fn click_during_caret_query_keeps_the_key_without_the_new_fields_caret() {
        for pointer_kind in [crate::input::MouseEventType::Move,
            crate::input::MouseEventType::ButtonPress { button: crate::input::MouseButton::Left }] {
            let tracker = Mutex::new(MouseTracker::new());
            {
                let mut state = tracker.lock();
                state.is_tracking = true;
                state.recording_start_time = Some(UNIX_EPOCH);
            }
            let clicked = matches!(pointer_kind, crate::input::MouseEventType::ButtonPress { .. });
            process(&tracker, input(0, CGEventType::KeyDown, CGEventFlags::empty()), || {
                tracker.lock().add_event(crate::input::MouseEvent {
                    timestamp: 124, x: 1200.0, y: 900.0,
                    event_type: pointer_kind, display_id: None,
                });
                Some((1200.0, 900.0))
            });
            let events = tracker.lock().get_key_events();
            assert_eq!(events.len(), 1);
            assert!(events[0].is_typing);
            assert_eq!(events[0].timestamp, 123);
            assert_eq!(events[0].caret_x, (!clicked).then_some(1200.0));
            assert_eq!(events[0].caret_y, (!clicked).then_some(900.0));
        }
    }

    #[test]
    fn activation_during_lookup_rejects_caret_but_keeps_the_key() {
        for activation in [122,124] {
            let tracker = Mutex::new(MouseTracker::new());
            {
                let mut state = tracker.lock();
                state.is_tracking = true;
                state.recording_start_time = Some(UNIX_EPOCH);
            }
            process(&tracker, input(0,CGEventType::KeyDown,CGEventFlags::empty()), || {
                tracker.lock().note_application_activation(activation);
                Some((700.0,300.0))
            });
            let state=tracker.lock();
            let events=state.get_key_events();
            assert_eq!(events.len(),1);
            assert!(events[0].is_typing);
            assert_eq!(events[0].caret_x,(activation<123).then_some(700.0));
            state.update_latest_typing_caret(800.0,400.0);
            assert_eq!(state.get_key_events()[0].caret_x,(activation<123).then_some(800.0));
        }
    }

    #[test]
    fn queued_keys_cannot_enter_a_restarted_session() {
        let tracker = Mutex::new(MouseTracker::new());
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(UNIX_EPOCH + Duration::from_secs(1));
        }
        process(
            &tracker,
            input(0, CGEventType::KeyDown, CGEventFlags::empty()),
            || panic!("old session must not query"),
        );
        assert!(tracker.lock().get_key_events().is_empty());
    }
}
