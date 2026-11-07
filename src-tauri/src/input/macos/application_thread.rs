use std::{sync::mpsc, time::{Duration, Instant}};

static APP: once_cell::sync::OnceCell<tauri::AppHandle> = once_cell::sync::OnceCell::new();

pub(super) fn install(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
}

pub(super) fn schedule(operation: impl FnOnce() + Send + 'static) -> Option<()> {
    APP.get()?.run_on_main_thread(operation).ok()
}

pub(super) fn query<T: Send + 'static>(pid: i32, operation: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    if pid != std::process::id() as i32 || unsafe { pthread_main_np() != 0 } {
        return Some(operation());
    }
    // Self-app AX reads enter AppKit directly rather than messaging another process.
    main_query(operation)
}

pub(super) fn main_query<T: Send + 'static>(operation: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    if unsafe { pthread_main_np() != 0 } { return Some(operation()); }
    let app = APP.get()?;
    dispatch(Duration::from_millis(100), operation, |work| {
        app.run_on_main_thread(work).map_err(|_| ())
    })
}

fn dispatch<T: Send + 'static>(
    timeout: Duration,
    operation: impl FnOnce() -> T + Send + 'static,
    schedule: impl FnOnce(Box<dyn FnOnce() + Send>) -> Result<(), ()>,
) -> Option<T> {
    let deadline = Instant::now() + timeout;
    let (send, receive) = mpsc::sync_channel(1);
    schedule(Box::new(move || {
        if Instant::now() < deadline {
            let _ = send.send(operation());
        }
    })).ok()?;
    receive.recv_timeout(deadline.saturating_duration_since(Instant::now())).ok()
}

unsafe extern "C" {
    fn pthread_main_np() -> std::ffi::c_int;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_query_returns_its_result_without_sharing_native_objects() {
        let result = dispatch(Duration::from_secs(1), || (10.0, 20.0), |work| {
            std::thread::spawn(work);
            Ok(())
        });
        assert_eq!(result, Some((10.0, 20.0)));
    }

    #[test]
    fn expired_queued_query_does_not_touch_the_application() {
        let mut queued = None;
        let result = dispatch(Duration::ZERO, || panic!("expired query executed"), |work| {
            queued = Some(work);
            Ok(())
        });
        assert_eq!(result, None::<()>);
        queued.unwrap()();
    }
}
