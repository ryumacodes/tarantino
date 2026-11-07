use super::{AXUIElementRef, AXUIElementSetMessagingTimeout, K_AX_ERROR_SUCCESS};
use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    string::{CFString, CFStringRef},
};
use objc::{class, msg_send, sel, sel_impl};
use std::{cell::RefCell, time::SystemTime};

#[derive(Default)]
struct Attachment {
    current: Option<(SystemTime, i32)>,
}

impl Attachment {
    fn prepare(&mut self, session: SystemTime, pid: i32, enable: impl FnOnce()) -> bool {
        if self.current == Some((session, pid)) {
            return false;
        }
        enable();
        self.current = Some((session, pid));
        true
    }
}

thread_local! { static ATTACHMENT: RefCell<Attachment> = RefCell::new(Attachment::default()); }

pub(super) fn prepare(session: SystemTime) -> Option<i32> {
    let pid = frontmost_pid()?;
    let changed = ATTACHMENT.with(|attachment| {
        attachment.borrow_mut().prepare(session, pid, || unsafe {
            let app = AXUIElementCreateApplication(pid);
            if app.is_null() {
                return;
            }
            let app = CFType::wrap_under_create_rule(app.cast());
            let app_ref = app.as_CFTypeRef() as AXUIElementRef;
            if AXUIElementSetMessagingTimeout(app_ref, 0.02) != K_AX_ERROR_SUCCESS {
                return;
            }
            // Providers without this attribute may reject it; caret lookup still proceeds.
            AXUIElementSetAttributeValue(
                app_ref,
                CFString::new("AXManualAccessibility").as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            );
        })
    });
    changed.then_some(pid)
}

#[allow(unexpected_cfgs)]
pub(super) fn frontmost_pid() -> Option<i32> {
    objc::rc::autoreleasepool(|| unsafe {
        let workspace: *mut objc::runtime::Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: *mut objc::runtime::Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        Some(msg_send![app, processIdentifier])
    })
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        time::{Duration, UNIX_EPOCH},
    };

    #[test]
    fn prepares_once_per_app_attachment_and_recording() {
        let mut attachment = Attachment::default();
        let attempts = Cell::new(0);
        let enable = || attempts.set(attempts.get() + 1);
        attachment.prepare(UNIX_EPOCH, 10, enable);
        attachment.prepare(UNIX_EPOCH, 10, enable);
        assert_eq!(attempts.get(), 1);
        attachment.prepare(UNIX_EPOCH, 20, enable);
        attachment.prepare(UNIX_EPOCH, 10, enable);
        assert_eq!(attempts.get(), 3);
        attachment.prepare(UNIX_EPOCH + Duration::from_secs(1), 10, enable);
        assert_eq!(attempts.get(), 4);
    }
}
