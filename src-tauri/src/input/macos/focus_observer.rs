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
    sync::{Arc, mpsc::Sender, atomic::{AtomicBool, Ordering}},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static HAS_SELECTION: AtomicBool = AtomicBool::new(false);
static APP: once_cell::sync::OnceCell<tauri::AppHandle> = once_cell::sync::OnceCell::new();
thread_local! { static CURRENT: RefCell<Option<Subscription>> = const { RefCell::new(None) }; }

pub fn install(app: &tauri::AppHandle) {
    super::application_thread::install(app);
    let _ = APP.set(app.clone());
}

type AXObserverRef = *const c_void;
struct Context {
    session: SystemTime,
    pid: i32,
    sender: Sender<Request>,
    target: RefCell<super::focus_target::FocusTarget<CFType>>,
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
    Children(CFType, usize, isize),
}

impl Drop for Subscription {
    fn drop(&mut self) {
        HAS_SELECTION.store(false, Ordering::Release);
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
                    target: RefCell::default(),
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
                    Watch::Children(children, depth, index) => queue_child(&mut subscription.pending, children, depth, index),
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
    let role = super::copy_role(element_ref).unwrap_or_default();
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
            subscription.pending.push_front(Watch::Children(children, depth + 1, 0));
        }
    }
}

unsafe fn queue_child(pending: &mut VecDeque<Watch>, children: CFType, depth: usize, index: isize) {
    let array = children.as_CFTypeRef() as CFArrayRef;
    let count = CFArrayGetCount(array);
    if index >= count { return; }
    let child = CFArrayGetValueAtIndex(array, index);
    if !child.is_null() && CFGetTypeID(child) == AXUIElementGetTypeID() {
        pending.push_back(Watch::Visit(CFType::wrap_under_get_rule(child), depth));
    }
    if index + 1 < count {
        pending.push_front(Watch::Children(children, depth, index + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_expansion_resumes_without_losing_order_or_native_ownership() {
        use core_foundation::array::CFArray;
        unsafe {
            let element = CFType::wrap_under_create_rule(AXUIElementCreateApplication(std::process::id() as i32).cast());
            let elements = [element.clone(), CFString::new("not an element").as_CFType(), element.clone()];
            let children = CFArray::from_CFTypes(&elements).as_CFType();
            let mut pending = VecDeque::from([Watch::Register(element.clone(), "AXValueChanged")]);
            queue_child(&mut pending, children, 3, 0);
            assert_eq!(pending.len(), 3);
            let Watch::Children(children, depth, index) = pending.pop_front().unwrap() else { panic!("missing continuation"); };
            assert_eq!((depth, index), (3, 1));
            queue_child(&mut pending, children, depth, index);
            assert_eq!(pending.len(), 3);
            let Watch::Children(children, depth, index) = pending.pop_front().unwrap() else { panic!("missing continuation"); };
            queue_child(&mut pending, children, depth, index);
            assert!(matches!(pending.pop_front(), Some(Watch::Register(_, "AXValueChanged"))));
            let expected = element.as_CFTypeRef();
            drop(elements);
            drop(element);
            for _ in 0..2 {
                let Watch::Visit(child, depth) = pending.pop_front().unwrap() else { panic!("missing child"); };
                assert_eq!(depth, 3);
                assert_eq!(child.as_CFTypeRef(), expected);
                assert_eq!(CFGetTypeID(child.as_CFTypeRef()), AXUIElementGetTypeID());
            }
            assert!(pending.is_empty());
        }
    }

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
    element: AXUIElementRef,
    notification: CFStringRef,
    context: *mut c_void,
) {
    let context = &*(context as *const Context);
    let mut pid = 0;
    if AXUIElementGetPid(element, &mut pid) != 0 || pid != context.pid {
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let now = Instant::now();
    let _budget = super::query_budget::LookupBudget::begin();
    let name = CFString::wrap_under_get_rule(notification).to_string();
    let notification = super::focus_target::Notification::from_name(&name);
    let (text_role, selected_child) = match notification {
        super::focus_target::Notification::Focus => {
            let text_role = super::copy_role(element)
                .is_some_and(|role| super::focus_target::is_text_role(&role));
            (text_role, false)
        }
        super::focus_target::Notification::Selection => (false, super::copy_selected_child(element).is_some()),
        super::focus_target::Notification::Refresh => (false, false),
    };
    let mut target = context.target.borrow_mut();
    target.notify(notification, CFType::wrap_under_get_rule(element.cast()), now, text_role, selected_child);
    HAS_SELECTION.store(target.selected.is_some(), Ordering::Release);
    drop(target);
    let _ = context.sender.send(Request {
        session: context.session,
        due: Instant::now(),
        trigger_timestamp: timestamp,
        trigger: FocusTrigger::Accessibility,
        expected_pid: Some(context.pid),
    });
}

pub(super) fn selected_focus(session: Option<SystemTime>, pid: i32) -> Option<super::FocusObservation> {
    if !HAS_SELECTION.load(Ordering::Acquire) { return None; }
    super::application_thread::main_query(move || {
        let selected = CURRENT.with(|current| {
            let current = current.borrow();
            let subscription = current.as_ref().filter(|value| value.context.pid == pid
                && session.is_none_or(|session| value.context.session == session))?;
            let selected = subscription.context.target.borrow().selected.clone();
            selected
        });
        selected.map(|source| {
            let _budget = super::query_budget::LookupBudget::begin();
            unsafe { super::focus_from_raw_element(source, &super::CaretAttributes::new(), session.is_some()) }
        })
    }).unwrap_or(Some(super::FocusObservation::default()))
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
