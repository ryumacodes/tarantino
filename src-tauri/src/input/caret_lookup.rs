use super::MouseTracker;
use parking_lot::MutexGuard;
use std::time::{Duration, SystemTime};

/// A live caret query cannot recover a historical caret. Keep delayed key
/// events for typing detection, but do not label them with today's position.
pub(super) fn for_event<T>(event_time: SystemTime, query: impl FnOnce() -> Option<T>) -> Option<T> {
    if is_recent(event_time, SystemTime::now()) {
        query()
    } else {
        None
    }
}

fn is_recent(event_time: SystemTime, now: SystemTime) -> bool {
    now.duration_since(event_time)
        .is_ok_and(|age| age <= Duration::from_millis(100))
}

/// Accessibility queries may wait on another application. Pointer capture and
/// stop must remain available, and late replies must not enter a new recording.
pub(super) fn without_tracker_lock<T>(
    tracker: &mut MutexGuard<'_, MouseTracker>,
    query: impl FnOnce() -> T,
) -> Result<T, ()> {
    let session = tracker.recording_start_time;
    let result = MutexGuard::unlocked(tracker, query);
    if tracker.is_tracking && tracker.recording_start_time == session {
        Ok(result)
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    #[test]
    fn delayed_events_do_not_query_current_caret() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        assert!(is_recent(now, now));
        assert!(is_recent(now - Duration::from_millis(100), now));
        assert!(!is_recent(now - Duration::from_millis(101), now));
        assert!(!is_recent(now + Duration::from_millis(1), now));
        assert_eq!(
            for_event(SystemTime::UNIX_EPOCH, || -> Option<()> {
                panic!("stale input must not perform accessibility IPC")
            }),
            None
        );
    }

    #[test]
    fn pointer_capture_can_acquire_tracker_during_caret_query() {
        let tracker = Mutex::new(MouseTracker::new());
        let mut guard = tracker.lock();
        guard.is_tracking = true;
        guard.recording_start_time = Some(SystemTime::UNIX_EPOCH);
        let result = without_tracker_lock(&mut guard, || {
            let input = tracker
                .try_lock()
                .expect("caret query must release tracker");
            input.add_event(super::super::MouseEvent {
                timestamp: 10,
                x: 12.0,
                y: 34.0,
                event_type: super::super::MouseEventType::Move,
                display_id: None,
            });
            (20.0, 30.0)
        });
        assert_eq!(result, Ok((20.0, 30.0)));
        assert_eq!(guard.get_events().len(), 1);
    }

    #[test]
    fn stopped_or_replaced_session_discards_late_caret_reply() {
        for restart in [false, true] {
            let tracker = Mutex::new(MouseTracker::new());
            let mut guard = tracker.lock();
            guard.is_tracking = true;
            guard.recording_start_time = Some(SystemTime::UNIX_EPOCH);
            let result = without_tracker_lock(&mut guard, || {
                let mut state = tracker.lock();
                state.stop_tracking();
                if restart {
                    state.is_tracking = true;
                    state.recording_start_time =
                        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
                }
                (20.0, 30.0)
            });
            assert_eq!(result, Err(()));
        }
    }
}
