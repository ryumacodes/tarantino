//! Keep the native event tap free of accessibility work and tracker locks.
use rdev::{Event, EventType};
use std::sync::mpsc::{self, Sender};

fn worker<T: Send + 'static>(handle: impl Fn(T) + Send + 'static) -> Sender<T> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for event in receiver {
            handle(event);
        }
    });
    sender
}

pub(super) fn off_thread(handle: impl Fn(Event) + Send + 'static) -> impl Fn(Event) {
    let sender = worker(handle);
    move |mut event| {
        if matches!(
            event.event_type,
            EventType::KeyPress(_) | EventType::KeyRelease(_)
        ) {
            // Detection stores categories, never typed characters.
            event.unicode = None;
            let _ = sender.send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_input_is_ordered_and_does_not_wait_for_a_slow_lookup() {
        let caller = std::thread::current().id();
        let (started, ready) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = std::sync::Mutex::new(gate);
        let (done, results) = mpsc::channel();
        let sender = worker(move |number| {
            assert_ne!(std::thread::current().id(), caller);
            if number == 1 {
                started.send(()).unwrap();
                gate.lock().unwrap().recv().unwrap();
            }
            done.send(number).unwrap();
        });
        sender.send(1).unwrap();
        ready
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        // The worker is blocked in the lookup; native dispatch still returns.
        sender.send(2).unwrap();
        assert!(results.try_recv().is_err());
        release.send(()).unwrap();
        assert_eq!(
            results
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap(),
            1
        );
        assert_eq!(
            results
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap(),
            2
        );
    }
}
