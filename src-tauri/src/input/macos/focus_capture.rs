use crate::input::{focus_samples::FocusSample, MouseTracker};
use parking_lot::Mutex;
use std::{collections::VecDeque, sync::{mpsc, Arc}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

pub(super) struct Nudger {
    sender: mpsc::Sender<(SystemTime, Instant)>,
    last: Option<(SystemTime, Instant)>,
}

impl Nudger {
    pub(super) fn start(tracker: Arc<Mutex<MouseTracker>>) -> Self {
        let (sender, receiver) = mpsc::channel::<(SystemTime, Instant)>();
        std::thread::spawn(move || {
            let mut pending: VecDeque<(SystemTime, Instant)> = VecDeque::new();
            loop {
                if let Some(&(session, due)) = pending.front() {
                    if Instant::now() >= due {
                        pending.pop_front();
                        sample(&tracker, session, super::focused_caret_position);
                        continue;
                    }
                    match receiver.recv_timeout(due.saturating_duration_since(Instant::now())) {
                        Ok(request) => pending.push_back(request),
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match receiver.recv() {
                        Ok(request) => pending.push_back(request),
                        Err(_) => break,
                    }
                }
            }
        });
        Self { sender, last: None }
    }

    pub(super) fn request(&mut self, session: SystemTime, now: Instant) {
        if self.last.is_some_and(|(previous_session, previous)| previous_session == session
            && now.duration_since(previous) <= Duration::from_millis(50)) { return; }
        self.last = Some((session, now));
        let _ = self.sender.send((session, now + Duration::from_millis(100)));
    }
}

fn sample(tracker: &Mutex<MouseTracker>, session: SystemTime, query: impl FnOnce() -> Option<(f64, f64)>) {
    {
        let state = tracker.lock();
        if !state.is_tracking || state.recording_start_time != Some(session) { return; }
    }
    let caret = query().map(|(x, y)| [x, y]);
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut state = tracker.lock();
    if state.is_tracking && state.recording_start_time == Some(session) {
        state.focus_samples.push(FocusSample { timestamp, caret });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nudge_throttle_preserves_independent_delayed_samples_and_resets_per_session() {
        let (sender, receiver) = mpsc::channel();
        let mut nudger = Nudger { sender, last: None };
        let now = Instant::now();
        let session = UNIX_EPOCH;
        nudger.request(session, now);
        nudger.request(session, now + Duration::from_millis(50));
        nudger.request(session, now + Duration::from_millis(51));
        nudger.request(session + Duration::from_secs(1), now + Duration::from_millis(52));
        let requests: Vec<_> = receiver.try_iter().collect();
        assert_eq!(requests, vec![(session, now + Duration::from_millis(100)),
            (session, now + Duration::from_millis(151)),
            (session + Duration::from_secs(1), now + Duration::from_millis(152))]);
    }

    #[test]
    fn query_releases_tracker_and_does_not_rewrite_key_events() {
        let tracker = Mutex::new(MouseTracker::new());
        {
            let mut state = tracker.lock();
            state.is_tracking = true;
            state.recording_start_time = Some(UNIX_EPOCH);
        }
        sample(&tracker, UNIX_EPOCH, || {
            assert!(tracker.try_lock().is_some());
            Some((10.0, 20.0))
        });
        sample(&tracker, UNIX_EPOCH, || None);
        let state = tracker.lock();
        assert!(state.get_key_events().is_empty());
        assert_eq!(state.focus_samples.len(), 2);
        assert_eq!(state.focus_samples[0].caret, Some([10.0, 20.0]));
        assert_eq!(state.focus_samples[1].caret, None);
        assert!(state.focus_samples[0].timestamp > 0);
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
            sample(&tracker, UNIX_EPOCH, || {
                let mut state = tracker.lock();
                state.is_tracking = restart;
                state.recording_start_time = restart.then_some(UNIX_EPOCH + Duration::from_secs(1));
                Some((10.0, 20.0))
            });
            assert!(tracker.lock().focus_samples.is_empty());
            sample(&tracker, UNIX_EPOCH, || panic!("expired session must not query"));
        }
    }
}
