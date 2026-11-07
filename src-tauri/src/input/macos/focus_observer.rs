use super::{AXUIElementRef, AXUIElementSetMessagingTimeout, focus_capture::Request};
use crate::input::{MouseTracker, focus_samples::FocusTrigger};
use core_foundation::{
    base::{CFType, TCFType},
    runloop::{CFRunLoop, CFRunLoopSource, CFRunLoopSourceRef, kCFRunLoopCommonModes},
    string::{CFString, CFStringRef},
};
use parking_lot::Mutex;
use std::{
    cell::RefCell,
    ffi::c_void,
    sync::{Arc, mpsc::Sender},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

static APP: once_cell::sync::OnceCell<tauri::AppHandle> = once_cell::sync::OnceCell::new();
thread_local! { static CURRENT: RefCell<Option<Subscription>> = const { RefCell::new(None) }; }

pub fn install(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
}

type AXObserverRef = *const c_void;
struct Context {
    session: SystemTime,
    pid: i32,
    sender: Sender<Request>,
}
struct Subscription {
    _observer: CFType,
    source: CFRunLoopSource,
    context: Box<Context>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // Creation, callbacks and removal all run on the main run loop.
        CFRunLoop::get_main().remove_source(&self.source, unsafe { kCFRunLoopCommonModes });
    }
}

pub(super) fn attach(
    session: SystemTime,
    pid: i32,
    sender: Sender<Request>,
    tracker: Arc<Mutex<MouseTracker>>,
) {
    let Some(app) = APP.get() else {
        return;
    };
    let _ = app.run_on_main_thread(move || {
        {
            let state = tracker.lock();
            if !state.is_tracking || state.recording_start_time != Some(session) {
                return;
            }
        }
        if super::focus_application::frontmost_pid() != Some(pid) {
            return;
        }
        CURRENT.with(|current| {
            let mut current = current.borrow_mut();
            if current
                .as_ref()
                .is_some_and(|value| value.context.session == session && value.context.pid == pid)
            {
                return;
            }
            *current = None;
            *current = unsafe {
                subscribe(Context {
                    session,
                    pid,
                    sender,
                })
            };
        });
    });
}

pub(in crate::input) fn detach(session: Option<SystemTime>) {
    let Some(app) = APP.get() else {
        return;
    };
    let _ = app.run_on_main_thread(move || {
        CURRENT.with(|current| {
            let mut current = current.borrow_mut();
            if current
                .as_ref()
                .is_some_and(|value| Some(value.context.session) == session)
            {
                *current = None;
            }
        })
    });
}

unsafe fn subscribe(context: Context) -> Option<Subscription> {
    let mut observer = std::ptr::null();
    if AXObserverCreate(context.pid, callback, &mut observer) != 0 {
        return None;
    }
    let observer = CFType::wrap_under_create_rule(observer);
    let source = AXObserverGetRunLoopSource(observer.as_CFTypeRef());
    if source.is_null() {
        return None;
    }
    let source = CFRunLoopSource::wrap_under_get_rule(source);
    let app = AXUIElementCreateApplication(context.pid);
    if app.is_null() {
        return None;
    }
    let app = CFType::wrap_under_create_rule(app.cast());
    let app_ref = app.as_CFTypeRef() as AXUIElementRef;
    if AXUIElementSetMessagingTimeout(app_ref, 0.02) != 0 {
        return None;
    }
    let mut context = Box::new(context);
    for name in [
        "AXFocusedUIElementChanged",
        "AXFocusedWindowChanged",
        "AXSelectedCellsChanged",
        "AXSelectedRowsChanged",
        "AXSelectedColumnsChanged",
        "AXSelectedChildrenChanged",
        "AXValueChanged",
        "AXSelectedTextChanged",
        "AXWindowMoved",
        "AXWindowResized",
        "AXActiveElementChanged",
    ] {
        AXObserverAddNotification(
            observer.as_CFTypeRef(),
            app_ref,
            CFString::new(name).as_concrete_TypeRef(),
            (&mut *context as *mut Context).cast(),
        );
    }
    CFRunLoop::get_main().add_source(&source, kCFRunLoopCommonModes);
    Some(Subscription {
        _observer: observer,
        source,
        context,
    })
}

unsafe extern "C" fn callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    _notification: CFStringRef,
    context: *mut c_void,
) {
    let context = &*(context as *const Context);
    let mut pid = 0;
    if AXUIElementGetPid(_element, &mut pid) != 0 || pid != context.pid {
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let _ = context.sender.send(Request {
        session: context.session,
        due: Instant::now(),
        trigger_timestamp: timestamp,
        trigger: FocusTrigger::Accessibility,
        expected_pid: Some(context.pid),
    });
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXObserverCreate(
        pid: i32,
        callback: unsafe extern "C" fn(AXObserverRef, AXUIElementRef, CFStringRef, *mut c_void),
        observer: *mut AXObserverRef,
    ) -> i32;
    fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;
    fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
        context: *mut c_void,
    ) -> i32;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> i32;
}
