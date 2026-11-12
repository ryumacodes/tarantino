use crate::input::{focus_samples::{FocusSample, FocusTrigger}, MouseTracker};
use parking_lot::Mutex;
use std::{collections::VecDeque, sync::{mpsc, Arc}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Request {
    pub(super) session: SystemTime,
    pub(super) due: Instant,
    pub(super) trigger_timestamp: u64,
    pub(super) trigger: FocusTrigger,
    pub(super) expected_pid: Option<i32>,
}

pub(super) struct Nudger {
    sender: Arc<mpsc::Sender<Request>>,
    last: Option<(SystemTime, Instant)>,
}

impl Nudger {
    pub(super) fn start(tracker: Arc<Mutex<MouseTracker>>) -> Self {
        let (sender, receiver) = mpsc::channel::<Request>();
        let sender = Arc::new(sender);
        let notifications = Arc::downgrade(&sender);
        super::focus_activation::install(&tracker, &sender);
        std::thread::spawn(move || {
            let mut pending: VecDeque<Request> = VecDeque::new();
            loop {
                if let Some(&request) = pending.front() {
                    let due = request.due;
                    if Instant::now() >= due {
                        pending.pop_front();
                        let Some(pid) = super::focus_application::frontmost_pid() else { continue; };
                        if request.expected_pid.is_some_and(|expected| expected != pid) { continue; }
                        sample(&tracker, request, pid, || {
                            let attachment = if matches!(request.trigger, FocusTrigger::Application) {
                                request.expected_pid.inspect(|&pid| super::focus_application::refresh(pid))
                            } else {
                                super::focus_application::prepare(request.session, pid).inspect(|&pid| {
                                    for retry in attachment_retries(request, pid, Instant::now()) {
                                        enqueue(&mut pending, retry);
                                    }
                                })
                            };
                            if let Some(pid) = attachment {
                                if let Some(sender) = notifications.upgrade() {
                                    super::focus_observer::attach(request.session, pid, sender.as_ref().clone(), tracker.clone());
                                }
                            }
                            let caret = super::focus_observer::selected_focus(Some(request.session), pid)
                                .unwrap_or_else(|| super::focus_in_application(pid, true));
                            if super::focus_application::frontmost_pid() == Some(pid) { caret } else { super::FocusObservation::default() }
                        });
                        continue;
                    }
                    match receiver.recv_timeout(due.saturating_duration_since(Instant::now())) {
                        Ok(request) => enqueue(&mut pending, request),
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match receiver.recv() {
                        Ok(request) => enqueue(&mut pending, request),
                        Err(_) => break,
                    }
                }
            }
        });
        Self { sender, last: None }
    }

    pub(super) fn request(&mut self, session: SystemTime, now: Instant, trigger_timestamp: u64, trigger: FocusTrigger) {
        if self.last.is_some_and(|(previous_session, previous)| previous_session == session
            && now.duration_since(previous) <= Duration::from_millis(50)) { return; }
        self.last = Some((session, now));
        let _ = self.sender.send(Request { session, due: now + Duration::from_millis(100), trigger_timestamp, trigger, expected_pid: None });
    }
}

fn attachment_retries(initial: Request, pid: i32, now: Instant) -> [Request; 2] {
    [300, 1000].map(|delay| Request {
        session: initial.session,
        due: now + Duration::from_millis(delay),
        trigger_timestamp: initial.trigger_timestamp,
        trigger: FocusTrigger::Application,
        expected_pid: Some(pid),
    })
}

fn enqueue(pending: &mut VecDeque<Request>, request: Request) {
    let index = pending.partition_point(|queued| queued.due <= request.due);
    pending.insert(index, request);
}

fn sample(tracker: &Mutex<MouseTracker>, request: Request, pid: i32, query: impl FnOnce() -> super::FocusObservation) {
    let session = request.session;
    {
        let state = tracker.lock();
        if !state.is_tracking || state.recording_start_time != Some(session) { return; }
    }
    let observation = query();
    let (caret, estimated_caret, caret_bounds) = match observation.caret {
        Some(super::FocusPosition::Measured(value)) => (Some([value.point.0, value.point.1]), None, Some(value.bounds)),
        Some(super::FocusPosition::Estimated(value)) => (None, Some([value.point.0, value.point.1]), Some(value.bounds)),
        None => (None, None, None),
    };
    let timestamp = observation.observed_at.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut state = tracker.lock();
    if state.is_tracking && state.recording_start_time == Some(session) {
        state.focus_samples.push(FocusSample {
            timestamp,
            focus_state: Some(observation.state),
            app_pid: (observation.state == super::FocusState::Present).then_some(pid),
            focus_role: observation.role,
            focus_bounds: observation.bounds,
            caret,
            caret_bounds,
            estimated_caret,
            trigger_timestamp: request.trigger_timestamp,
            trigger: request.trigger,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{FocusObservation, FocusState};

    fn request() -> Request {
        Request { session: UNIX_EPOCH, due: Instant::now(), trigger_timestamp: 123, trigger: FocusTrigger::Key, expected_pid: None }
    }

    #[test]
    fn nudge_throttle_preserves_independent_delayed_samples_and_resets_per_session() {
        let (sender, receiver) = mpsc::channel();
        let mut nudger = Nudger { sender: Arc::new(sender), last: None };
        let now = Instant::now();
        let session = UNIX_EPOCH;
        nudger.request(session, now, 1000, FocusTrigger::Key);
        nudger.request(session, now + Duration::from_millis(50), 1050, FocusTrigger::Pointer);
        nudger.request(session, now + Duration::from_millis(51), 1051, FocusTrigger::Pointer);
        nudger.request(session + Duration::from_secs(1), now + Duration::from_millis(52), 1052, FocusTrigger::Modifiers);
        let requests: Vec<_> = receiver.try_iter().collect();
        assert_eq!(requests, vec![
            Request { session, due: now + Duration::from_millis(100), trigger_timestamp: 1000, trigger: FocusTrigger::Key, expected_pid: None },
            Request { session, due: now + Duration::from_millis(151), trigger_timestamp: 1051, trigger: FocusTrigger::Pointer, expected_pid: None },
            Request { session: session + Duration::from_secs(1), due: now + Duration::from_millis(152), trigger_timestamp: 1052, trigger: FocusTrigger::Modifiers, expected_pid: None },
        ]);
    }

    #[test]
    fn attachment_retries_preserve_session_and_target_app() {
        let initial = request();
        let now = Instant::now();
        let retries = attachment_retries(initial, 42, now);
        assert_eq!(retries.map(|retry| retry.due.duration_since(now)), [Duration::from_millis(300), Duration::from_secs(1)]);
        assert!(retries.iter().all(|retry| retry.session == initial.session
            && retry.expected_pid == Some(42) && retry.trigger == FocusTrigger::Application));
    }

    #[test]
    fn immediate_notifications_precede_pending_delayed_keys() {
        let now = Instant::now();
        let mut pending = VecDeque::new();
        let mut key = request();
        key.due = now + Duration::from_millis(100);
        let mut notification = request();
        notification.due = now;
        notification.trigger = FocusTrigger::Accessibility;
        notification.expected_pid = Some(10);
        enqueue(&mut pending, key);
        enqueue(&mut pending, notification);
        assert_eq!(pending.pop_front(), Some(notification));
        assert_eq!(pending.pop_front(), Some(key));
    }

    #[test]
    fn query_releases_tracker_and_does_not_rewrite_key_events() {
        let tracker = Mutex::new(MouseTracker::new());
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(UNIX_EPOCH);
        }
        sample(&tracker, request(), 42, || {
            assert!(tracker.try_lock().is_some());
            FocusObservation { bounds: Some([0.0, 0.0, 300.0, 100.0]), state: FocusState::Present, role: Some("AXTextField".to_owned()), caret: Some(super::super::FocusPosition::Measured(super::super::caret_range::Geometry { point: (10.0, 20.0), bounds: [10.0, 12.0, 2.0, 16.0] })), ..Default::default() }
        });
        sample(&tracker, request(), 42, FocusObservation::default);
        sample(&tracker, request(), 42, || FocusObservation { bounds: Some([0.0, 0.0, 300.0, 100.0]), state: FocusState::Present, role: Some("AXTextField".to_owned()), caret: Some(super::super::FocusPosition::Estimated(super::super::caret_range::Geometry {
            point: (15.0, 25.0), bounds: [14.0, 21.0, 2.0, 8.0],
        })), ..Default::default() });
        let state = tracker.lock();
        assert!(state.get_key_events().is_empty());
        assert_eq!(state.focus_samples.len(), 3);
        assert_eq!(state.focus_samples[0].caret, Some([10.0, 20.0]));
        assert_eq!(state.focus_samples[0].focus_bounds, Some([0.0, 0.0, 300.0, 100.0]));
        assert_eq!(state.focus_samples[0].caret_bounds, Some([10.0, 12.0, 2.0, 16.0]));
        assert_eq!(state.focus_samples[0].trigger_timestamp, 123);
        assert_eq!(state.focus_samples[0].trigger, FocusTrigger::Key);
        assert_eq!(state.focus_samples[1].caret, None);
        assert_eq!(state.focus_samples[1].caret_bounds, None);
        assert_eq!(state.focus_samples[2].caret, None);
        assert_eq!(state.focus_samples[2].estimated_caret, Some([15.0, 25.0]));
        assert_eq!(state.focus_samples[2].caret_bounds, Some([14.0, 21.0, 2.0, 8.0]));
        let saved = serde_json::to_vec(&state.focus_samples).unwrap();
        assert_eq!(serde_json::from_slice::<Vec<FocusSample>>(&saved).unwrap(), state.focus_samples);
        assert!(state.focus_samples[0].timestamp > 0);
    }

    #[test]
    fn focus_without_a_caret_is_distinct_from_absence_and_lookup_failure() {
        let tracker = Mutex::new(MouseTracker::new());
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(UNIX_EPOCH);
        }
        let states = [FocusState::Present, FocusState::Absent, FocusState::Unavailable];
        for state in states {
            sample(&tracker, request(), 42, || FocusObservation { bounds: None, state, role: (state == FocusState::Present).then(|| "AXButton".to_owned()), caret: None, ..Default::default() });
        }
        let state = tracker.lock();
        assert_eq!(state.focus_samples.len(), states.len());
        for (sample, expected) in state.focus_samples.iter().zip(states) {
            assert_eq!(sample.focus_state, Some(expected));
            assert_eq!(sample.app_pid, (expected == FocusState::Present).then_some(42));
            assert_eq!(sample.focus_role.as_deref(), (expected == FocusState::Present).then_some("AXButton"));
            assert_eq!(sample.caret, None);
            assert_eq!(sample.estimated_caret, None);
        }
    }

    #[test]
    fn metadata_completion_does_not_shift_the_focus_observation_timestamp() {
        let tracker = Mutex::new(MouseTracker::new());
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(UNIX_EPOCH);
        }
        sample(&tracker, request(), 42, || {
            let mut observation = FocusObservation {
                observed_at: UNIX_EPOCH + Duration::from_millis(150),
                state: FocusState::Present,
                ..Default::default()
            };
            observation.bounds = Some([0.0, 0.0, 300.0, 100.0]);
            observation
        });
        let state = tracker.lock();
        assert_eq!(state.focus_samples[0].timestamp, 150);
        assert_eq!(state.focus_samples[0].trigger_timestamp, 123);
        assert_eq!(state.focus_samples[0].focus_bounds, Some([0.0, 0.0, 300.0, 100.0]));
    }

    #[test]
    fn late_queries_cannot_enter_stopped_or_restarted_recordings() {
        for restart in [false, true] {
            let tracker = Mutex::new(MouseTracker::new());
            {
                let mut state = tracker.lock();
                state.is_tracking = true;
                state.recording_start_time = Some(UNIX_EPOCH);
            }
            sample(&tracker, request(), 42, || {
                let mut state = tracker.lock();
                state.is_tracking = restart;
                state.recording_start_time = restart.then_some(UNIX_EPOCH + Duration::from_secs(1));
                FocusObservation { bounds: Some([0.0, 0.0, 300.0, 100.0]), state: FocusState::Present, role: Some("AXTextField".to_owned()), caret: Some(super::super::FocusPosition::Measured(super::super::caret_range::Geometry { point: (10.0, 20.0), bounds: [10.0, 12.0, 2.0, 16.0] })), ..Default::default() }
            });
            assert!(tracker.lock().focus_samples.is_empty());
            sample(&tracker, request(), 42, || panic!("expired session must not query"));
        }
    }
}
