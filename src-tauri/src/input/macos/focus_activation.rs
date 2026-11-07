use super::focus_capture::Request;
use crate::input::{MouseTracker, focus_samples::FocusTrigger};
use block::ConcreteBlock;
use objc::{class, msg_send, sel, sel_impl, rc::StrongPtr, runtime::Object};
use parking_lot::Mutex;
use std::{cell::RefCell, sync::{Arc, Weak, mpsc::Sender}, time::{Instant, SystemTime, UNIX_EPOCH}};

struct Connection {
    tracker: Weak<Mutex<MouseTracker>>,
    sender: Weak<Sender<Request>>,
}
static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);
thread_local! { static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) }; }

struct Observer {
    center: StrongPtr,
    token: StrongPtr,
}
impl Drop for Observer {
    fn drop(&mut self) {
        unsafe { let _: () = msg_send![*self.center, removeObserver: *self.token]; }
    }
}

pub(super) fn install(tracker: &Arc<Mutex<MouseTracker>>, sender: &Arc<Sender<Request>>) {
    *CONNECTION.lock() = Some(Connection { tracker: Arc::downgrade(tracker), sender: Arc::downgrade(sender) });
    let _ = super::application_thread::schedule(|| {
        OBSERVER.with(|observer| {
            let mut observer = observer.borrow_mut();
            if observer.is_none() { *observer = unsafe { subscribe() }; }
        });
    });
}

#[allow(unexpected_cfgs)]
unsafe fn subscribe() -> Option<Observer> {
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let center: *mut Object = msg_send![workspace, notificationCenter];
    let queue: *mut Object = msg_send![class!(NSOperationQueue), mainQueue];
    let callback = ConcreteBlock::new(|_notification: *mut Object| refresh()).copy();
    let token: *mut Object = msg_send![center,
        addObserverForName: NSWorkspaceDidActivateApplicationNotification
        object: std::ptr::null_mut::<Object>()
        queue: queue
        usingBlock: &*callback];
    if token.is_null() { return None; }
    Some(Observer { center: StrongPtr::retain(center), token: StrongPtr::retain(token) })
}

pub(crate) fn refresh() {
    let (tracker, sender) = {
        let connection = CONNECTION.lock();
        let Some(connection) = connection.as_ref() else { return; };
        let (Some(tracker), Some(sender)) = (connection.tracker.upgrade(), connection.sender.upgrade()) else { return; };
        (tracker, sender)
    };
    let session = {
        let state = tracker.lock();
        state.is_tracking.then_some(state.recording_start_time).flatten()
    };
    let Some(session) = session else { return; };
    let Some(pid) = super::focus_application::frontmost_pid() else { return; };
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let _ = sender.send(Request {
        session,
        due: Instant::now(),
        trigger_timestamp: timestamp,
        trigger: FocusTrigger::Activation,
        expected_pid: Some(pid),
    });
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSWorkspaceDidActivateApplicationNotification: *mut Object;
}
