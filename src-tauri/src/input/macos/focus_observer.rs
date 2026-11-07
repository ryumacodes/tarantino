use super::{
    AXUIElementGetTypeID, AXUIElementRef, AXUIElementSetMessagingTimeout, CFArrayGetCount,
    CFArrayGetTypeID, CFArrayGetValueAtIndex, CFArrayRef, copy_attribute, focus_capture::Request,
};
use crate::input::{MouseTracker, focus_samples::FocusTrigger};
use core_foundation::{
    base::{CFGetTypeID, CFType, TCFType},
    runloop::{CFRunLoop, CFRunLoopSource, CFRunLoopSourceRef, kCFRunLoopCommonModes},
    string::{CFString, CFStringRef},
};
use parking_lot::Mutex;
use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::c_void,
    sync::{Arc, mpsc::Sender},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
    observer: CFType,
    source: CFRunLoopSource,
    context: Box<Context>,
    app: CFType,
    pending: VecDeque<Watch>,
    scan_scheduled: bool,
}

enum Watch {
    Visit(CFType, usize),
    Register(CFType, &'static str),
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
                let subscription = current.as_mut().unwrap();
                subscription
                    .pending
                    .push_back(Watch::Visit(subscription.app.clone(), 0));
                schedule_scan(subscription);
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
            if let Some(subscription) = current.as_mut() {
                schedule_scan(subscription);
            }
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
    let context = Box::new(context);
    let mut pending = VecDeque::new();
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
        pending.push_back(Watch::Register(app.clone(), name));
    }
    pending.push_back(Watch::Visit(app.clone(), 0));
    CFRunLoop::get_main().add_source(&source, kCFRunLoopCommonModes);
    Some(Subscription {
        observer,
        source,
        context,
        app,
        pending,
        scan_scheduled: false,
    })
}

fn schedule_scan(subscription: &mut Subscription) {
    if subscription.scan_scheduled || subscription.pending.is_empty() {
        return;
    }
    let Some(app) = APP.get().cloned() else {
        return;
    };
    subscription.scan_scheduled = true;
    let (session, pid) = (subscription.context.session, subscription.context.pid);
    // Dispatch from the runtime so a main-thread caller queues rather than recurses.
    tauri::async_runtime::spawn(async move {
        let _ = app.run_on_main_thread(move || scan_chunk(session, pid));
    });
}

fn scan_chunk(session: SystemTime, pid: i32) {
    CURRENT.with(|current| {
        let mut current = current.borrow_mut();
        let Some(subscription) = current
            .as_mut()
            .filter(|value| value.context.session == session && value.context.pid == pid)
        else {
            return;
        };
        subscription.scan_scheduled = false;
        if super::focus_application::frontmost_pid() != Some(pid) {
            subscription.pending.clear();
            return;
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(16) {
            let Some(action) = subscription.pending.pop_front() else {
                break;
            };
            unsafe {
                match action {
                    Watch::Register(element, name) => {
                        let element_ref = element.as_CFTypeRef() as AXUIElementRef;
                        AXUIElementSetMessagingTimeout(element_ref, 0.02);
                        AXObserverAddNotification(
                            subscription.observer.as_CFTypeRef(),
                            element_ref,
                            CFString::new(name).as_concrete_TypeRef(),
                            (&mut *subscription.context as *mut Context).cast(),
                        );
                    }
                    Watch::Visit(element, depth) => visit(subscription, element, depth),
                }
            }
        }
        schedule_scan(subscription);
    });
}

#[derive(Debug, PartialEq)]
enum Walk {
    Stop,
    Subscribe,
    Children,
}

fn walk(role: &str, depth: usize) -> Walk {
    if depth > 9 {
        return Walk::Stop;
    }
    match role {
        "AXTable" | "AXOutline" | "AXList" => Walk::Subscribe,
        "AXWebArea" | "AXMenuBar" => Walk::Stop,
        _ => Walk::Children,
    }
}

unsafe fn visit(subscription: &mut Subscription, element: CFType, depth: usize) {
    if depth > 9 {
        return;
    }
    let element_ref = element.as_CFTypeRef() as AXUIElementRef;
    if AXUIElementSetMessagingTimeout(element_ref, 0.02) != 0 {
        return;
    }
    let role = copy_attribute(element_ref, CFString::new("AXRole").as_concrete_TypeRef())
        .map(|value| {
            let value = CFType::wrap_under_create_rule(value);
            if CFGetTypeID(value.as_CFTypeRef()) == CFString::type_id() {
                CFString::wrap_under_get_rule(value.as_CFTypeRef() as CFStringRef).to_string()
            } else {
                String::new()
            }
        })
        .unwrap_or_default();
    match walk(&role, depth) {
        Walk::Stop => {}
        Walk::Subscribe => {
            for name in [
                "AXSelectedCellsChanged",
                "AXSelectedRowsChanged",
                "AXSelectedColumnsChanged",
                "AXSelectedChildrenChanged",
            ] {
                subscription
                    .pending
                    .push_back(Watch::Register(element.clone(), name));
            }
        }
        Walk::Children => {
            let Some(children) = copy_attribute(
                element_ref,
                CFString::new("AXChildren").as_concrete_TypeRef(),
            ) else {
                return;
            };
            let children = CFType::wrap_under_create_rule(children);
            if CFGetTypeID(children.as_CFTypeRef()) != CFArrayGetTypeID() {
                return;
            }
            let array = children.as_CFTypeRef() as CFArrayRef;
            for index in 0..CFArrayGetCount(array) {
                let child = CFArrayGetValueAtIndex(array, index);
                if !child.is_null() && CFGetTypeID(child) == AXUIElementGetTypeID() {
                    subscription
                        .pending
                        .push_back(Watch::Visit(CFType::wrap_under_get_rule(child), depth + 1));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn container_walk_stops_at_subscribed_and_excluded_subtrees() {
        for depth in [0, 9] {
            for role in ["AXTable", "AXOutline", "AXList"] {
                assert_eq!(walk(role, depth), Walk::Subscribe);
            }
            for role in ["AXWebArea", "AXMenuBar"] {
                assert_eq!(walk(role, depth), Walk::Stop);
            }
            for role in ["AXApplication", "AXWindow", "AXGroup", ""] {
                assert_eq!(walk(role, depth), Walk::Children);
            }
        }
        for role in ["AXTable", "AXGroup", ""] {
            assert_eq!(walk(role, 10), Walk::Stop);
        }
    }
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
