//! macOS Accessibility caret lookup.
mod pointer;
mod keyboard;
mod cursor_images;
pub use cursor_images::install as install_cursor_images;
pub(super) mod shortcuts;
mod query_budget;
mod caret_range;
mod focused_element;
mod focus_capture;
mod focus_application;
mod focus_activation;
pub(crate) use focus_activation::refresh as refresh_focus_attachment;
mod application_thread;
mod focus_observer;
mod focus_target;
mod focus_bounds;
pub use focus_observer::install as install_focus_observer;
pub(super) use focus_observer::detach as detach_focus_observer;
pub(super) use pointer::install as install_pointer_listener;

use core_foundation::base::{CFGetTypeID, CFRelease, CFType, CFTypeID, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use std::{ffi::c_void, time::SystemTime};
use super::focus_samples::FocusState;

/// Read Quartz desktop coordinates without waiting for a movement event.
pub fn current_pointer_position() -> Option<(f64, f64)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let point = CGEvent::new(source).ok()?.location();
    (point.x.is_finite() && point.y.is_finite()).then_some((point.x, point.y))
}

type AXUIElementRef = *const c_void;
type AXError = i32;
type CFArrayRef = *const c_void;
type CFIndex = isize;

const K_AX_ERROR_SUCCESS: AXError = 0;
const K_AX_ERROR_NO_VALUE: AXError = -25212;
const K_AX_VALUE_CG_POINT_TYPE: i32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: i32 = 2;
const K_AX_VALUE_CF_RANGE_TYPE: i32 = 4;
const K_AX_VALUE_CG_RECT_TYPE: i32 = 3;
const MAX_FOCUS_SELECTION_DEPTH: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementGetTypeID() -> CFTypeID;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout: f32) -> AXError;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        parameterizedAttribute: CFStringRef,
        parameter: CFTypeRef,
        result: *mut CFTypeRef,
    ) -> AXError;
    fn AXValueCreate(theType: i32, valuePtr: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, theType: i32, valuePtr: *mut c_void) -> bool;
    fn CFArrayGetTypeID() -> CFTypeID;
    fn CFArrayGetCount(theArray: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(theArray: CFArrayRef, idx: CFIndex) -> *const c_void;
}

unsafe fn copy_rect_for_parameterized_attribute(
    element: AXUIElementRef,
    attribute: CFStringRef,
    parameter: CFTypeRef,
) -> Option<CGRect> {
    if !query_budget::available() || AXUIElementSetMessagingTimeout(element, 0.02) != K_AX_ERROR_SUCCESS { return None; }
    let mut bounds_value: CFTypeRef = std::ptr::null();
    let bounds_status = AXUIElementCopyParameterizedAttributeValue(
        element,
        attribute,
        parameter,
        &mut bounds_value,
    );
    if bounds_status != K_AX_ERROR_SUCCESS || bounds_value.is_null() {
        return None;
    }

    let mut bounds = CGRect::default();
    let has_bounds = AXValueGetValue(
        bounds_value,
        K_AX_VALUE_CG_RECT_TYPE,
        &mut bounds as *mut _ as *mut c_void,
    );
    CFRelease(bounds_value);

    (has_bounds && usable_caret_bounds(bounds)).then_some(bounds)
}

unsafe fn copy_bounds_for_range(
    focused: AXUIElementRef,
    bounds_for_range_attr: CFStringRef,
    range_value: CFTypeRef,
) -> Option<CGRect> {
    copy_rect_for_parameterized_attribute(focused, bounds_for_range_attr, range_value)
}

unsafe fn create_range_value(range: CFRange) -> Option<CFTypeRef> {
    let value = AXValueCreate(
        K_AX_VALUE_CF_RANGE_TYPE,
        &range as *const _ as *const c_void,
    );

    if value.is_null() { None } else { Some(value) }
}

unsafe fn copy_attribute(element: AXUIElementRef, attribute: CFStringRef) -> Option<CFTypeRef> {
    if !query_budget::available() || AXUIElementSetMessagingTimeout(element, 0.02) != K_AX_ERROR_SUCCESS { return None; }
    let mut value: CFTypeRef = std::ptr::null();
    let status = AXUIElementCopyAttributeValue(element, attribute, &mut value);
    if status != K_AX_ERROR_SUCCESS || value.is_null() {
        None
    } else {
        Some(value)
    }
}

#[derive(Clone, Copy)]
enum FocusPosition {
    Measured(caret_range::Geometry),
    Estimated(caret_range::Geometry),
}

struct FocusObservation {
    observed_at: SystemTime,
    state: FocusState,
    role: Option<String>,
    bounds: Option<[f64; 4]>,
    caret: Option<FocusPosition>,
}

impl Default for FocusObservation {
    fn default() -> Self {
        Self { observed_at: SystemTime::now(), state: FocusState::Unavailable, role: None, bounds: None, caret: None }
    }
}

fn focused_attribute_state(status: AXError, valid_element: bool) -> FocusState {
    match status {
        K_AX_ERROR_SUCCESS if valid_element => FocusState::Present,
        K_AX_ERROR_NO_VALUE => FocusState::Absent,
        _ => FocusState::Unavailable,
    }
}

unsafe fn focus_from_focused_element(
    parent: AXUIElementRef,
    focused_element_attr: CFStringRef,
    attrs: &CaretAttributes,
    estimate: bool,
) -> FocusObservation {
    if !query_budget::available() || AXUIElementSetMessagingTimeout(parent, 0.02) != K_AX_ERROR_SUCCESS {
        return FocusObservation::default();
    }
    let mut value = std::ptr::null();
    let status = AXUIElementCopyAttributeValue(parent, focused_element_attr, &mut value);
    let focused = (!value.is_null()).then(|| CFType::wrap_under_create_rule(value));
    let valid_element = focused.as_ref().is_some_and(|value| CFGetTypeID(value.as_CFTypeRef()) == AXUIElementGetTypeID());
    let state = focused_attribute_state(status, valid_element);
    match (state, focused) {
        (FocusState::Present, Some(element)) => focus_from_raw_element(element, attrs, estimate),
        _ => FocusObservation { state, ..Default::default() },
    }
}

unsafe fn focus_from_raw_element(focused: CFType, attrs: &CaretAttributes, estimate: bool) -> FocusObservation {
    let related = |element: &CFType, attribute: &CFString| {
        let value = CFType::wrap_under_create_rule(copy_attribute(
            element.as_CFTypeRef() as AXUIElementRef, attribute.as_concrete_TypeRef(),
        )?);
        (CFGetTypeID(value.as_CFTypeRef()) == AXUIElementGetTypeID()).then_some(value)
    };
    let mut focused = focused_element::resolve(focused,
        |element| related(element, &attrs.active_element),
        |element| related(element, &attrs.editable_ancestor));
    for _ in 0..MAX_FOCUS_SELECTION_DEPTH {
        let Some(child) = copy_selected_child(focused.as_CFTypeRef() as AXUIElementRef) else { break; };
        focused = child;
    }
    let element = focused.as_CFTypeRef() as AXUIElementRef;
    focus_for_element(element, attrs, estimate)
}

unsafe fn copy_role(element: AXUIElementRef) -> Option<String> {
    let role = CFType::wrap_under_create_rule(copy_attribute(
        element, CFString::new("AXRole").as_concrete_TypeRef(),
    )?);
    if CFGetTypeID(role.as_CFTypeRef()) != CFString::type_id() { return None; }
    Some(CFString::wrap_under_get_rule(role.as_CFTypeRef() as CFStringRef).to_string())
}

unsafe fn copy_selected_child(element: AXUIElementRef) -> Option<CFType> {
    let role = copy_role(element)?;
    focused_element::selected_child(&role, |attribute| {
        let selected = CFType::wrap_under_create_rule(copy_attribute(
            element, CFString::new(attribute).as_concrete_TypeRef(),
        )?);
        if CFGetTypeID(selected.as_CFTypeRef()) != CFArrayGetTypeID() { return None; }
        let count = CFArrayGetCount(selected.as_CFTypeRef() as CFArrayRef);
        if count <= 0 { return None; }
        let child = CFArrayGetValueAtIndex(selected.as_CFTypeRef() as CFArrayRef, count - 1);
        if child.is_null() || CFGetTypeID(child) != AXUIElementGetTypeID() { return None; }
        Some(CFType::wrap_under_get_rule(child))
    })
}

unsafe fn caret_position_from_cf_range(
    element: AXUIElementRef,
    selected_range_attr: CFStringRef,
    bounds_for_range_attr: CFStringRef,
) -> Option<caret_range::Geometry> {
    let range_value = copy_attribute(element, selected_range_attr)?;

    let mut range = CFRange::default();
    if !AXValueGetValue(
        range_value,
        K_AX_VALUE_CF_RANGE_TYPE,
        &mut range as *mut _ as *mut c_void,
    ) {
        CFRelease(range_value);
        return None;
    }

    CFRelease(range_value);
    caret_range::geometry(range, |range| {
        let parameter = create_range_value(range)?;
        let bounds = copy_bounds_for_range(element, bounds_for_range_attr, parameter);
        CFRelease(parameter);
        bounds
    }, || copy_element_bounds(element))
}

unsafe fn copy_element_bounds(element: AXUIElementRef) -> Option<CGRect> {
    copy_element_rect(element).filter(|bounds| usable_caret_bounds(*bounds))
}

unsafe fn copy_element_rect(element: AXUIElementRef) -> Option<CGRect> {
    let position = CFType::wrap_under_create_rule(copy_attribute(
        element, CFString::new("AXPosition").as_concrete_TypeRef(),
    )?);
    let size = CFType::wrap_under_create_rule(copy_attribute(
        element, CFString::new("AXSize").as_concrete_TypeRef(),
    )?);
    let mut origin = CGPoint::default();
    let mut dimensions = CGSize::default();
    if !AXValueGetValue(position.as_CFTypeRef(), K_AX_VALUE_CG_POINT_TYPE, &mut origin as *mut _ as *mut c_void)
        || !AXValueGetValue(size.as_CFTypeRef(), K_AX_VALUE_CG_SIZE_TYPE, &mut dimensions as *mut _ as *mut c_void) {
        return None;
    }
    let bounds = CGRect::new(&origin, &dimensions);
    focus_bounds::finite(bounds).then_some(bounds)
}

unsafe fn caret_position_from_text_marker(
    element: AXUIElementRef,
    selected_text_marker_range_attr: CFStringRef,
    bounds_for_text_marker_range_attr: CFStringRef,
) -> Option<caret_range::Geometry> {
    let range_value = copy_attribute(element, selected_text_marker_range_attr)?;
    let bounds = copy_rect_for_parameterized_attribute(
        element,
        bounds_for_text_marker_range_attr,
        range_value,
    );
    CFRelease(range_value);

    bounds.map(caret_range::text_marker)
}

struct CaretAttributes {
    selected_range: CFString,
    bounds_for_range: CFString,
    selected_text_marker_range: CFString,
    bounds_for_text_marker_range: CFString,
    active_element: CFString,
    editable_ancestor: CFString,
}

impl CaretAttributes {
    fn new() -> Self {
        Self {
            selected_range: CFString::new("AXSelectedTextRange"),
            bounds_for_range: CFString::new("AXBoundsForRange"),
            selected_text_marker_range: CFString::new("AXSelectedTextMarkerRange"),
            bounds_for_text_marker_range: CFString::new("AXBoundsForTextMarkerRange"),
            active_element: CFString::new("AXActiveElement"),
            editable_ancestor: CFString::new("AXEditableAncestor"),
        }
    }
}

unsafe fn focus_for_element(
    element: AXUIElementRef,
    attrs: &CaretAttributes,
    estimate: bool,
) -> FocusObservation {
    observe_element(copy_role(element), |role| focused_element::caret(role, || {
        caret_position_from_cf_range(
            element,
            attrs.selected_range.as_concrete_TypeRef(),
            attrs.bounds_for_range.as_concrete_TypeRef(),
        ).map(FocusPosition::Measured)
    }, || {
        caret_position_from_text_marker(
            element,
            attrs.selected_text_marker_range.as_concrete_TypeRef(),
            attrs.bounds_for_text_marker_range.as_concrete_TypeRef(),
        ).map(FocusPosition::Measured)
    }, || {
        if !estimate { return None; }
        caret_range::geometric(copy_element_bounds(element)?).map(FocusPosition::Estimated)
    }), || if estimate { copy_focus_bounds(element) } else { None })
}

fn observe_element(
    role: Option<String>,
    caret: impl FnOnce(&str) -> Option<FocusPosition>,
    bounds: impl FnOnce() -> Option<[f64; 4]>,
) -> FocusObservation {
    let caret = role.as_deref().and_then(caret);
    let mut observation = FocusObservation { role, caret, ..Default::default() };
    observation.bounds = bounds();
    if observation.role.is_some() || observation.bounds.is_some() {
        observation.state = FocusState::Present;
    }
    observation
}

unsafe fn copy_focus_bounds(element: AXUIElementRef) -> Option<[f64; 4]> {
    let bounds = copy_element_rect(element)?;
    let parent_attribute = CFString::new("AXParent");
    let viewport = focus_bounds::enclosing_scroll_area(
        CFType::wrap_under_get_rule(element),
        |element| {
            let parent = CFType::wrap_under_create_rule(copy_attribute(
                element.as_CFTypeRef() as AXUIElementRef, parent_attribute.as_concrete_TypeRef(),
            )?);
            (CFGetTypeID(parent.as_CFTypeRef()) == AXUIElementGetTypeID()).then_some(parent)
        },
        |element| copy_role(element.as_CFTypeRef() as AXUIElementRef).as_deref() == Some("AXScrollArea"),
    ).and_then(|element| copy_element_rect(element.as_CFTypeRef() as AXUIElementRef));
    Some(focus_bounds::select(bounds, viewport))
}

fn focus_in_application(pid: i32, estimate: bool) -> FocusObservation {
    application_thread::query(pid, move || query_application_focus(pid, estimate)).unwrap_or_default()
}

fn query_application_focus(pid: i32, estimate: bool) -> FocusObservation {
    let _budget = query_budget::LookupBudget::begin();
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() { return FocusObservation::default(); }
        let app = CFType::wrap_under_create_rule(app.cast());
        let element = app.as_CFTypeRef() as AXUIElementRef;
        if AXUIElementSetMessagingTimeout(element, 0.02) != K_AX_ERROR_SUCCESS {
            return FocusObservation::default();
        }
        focus_from_focused_element(
            element,
            CFString::new("AXFocusedUIElement").as_concrete_TypeRef(),
            &CaretAttributes::new(),
            estimate,
        )
    }
}

pub fn focused_caret_position() -> Option<(f64, f64)> {
    let pid = focus_application::frontmost_pid()?;
    let caret = focus_observer::selected_focus(None, pid).unwrap_or_else(|| focus_in_application(pid, false));
    if focus_application::frontmost_pid() != Some(pid) { return None; }
    match caret.caret { Some(FocusPosition::Measured(value)) => Some(value.point), _ => None }
}

// Some accessibility providers return an empty rectangle when no caret bounds
// are available. Preserve zero-width insertion carets, but not zero-height boxes.
fn usable_caret_bounds(bounds:CGRect)->bool {
    [bounds.origin.x,bounds.origin.y,bounds.size.width,bounds.size.height].iter().all(|n|n.is_finite())
        && bounds.size.width>=0.0 && bounds.size.height>0.0
}
#[cfg(test)]
mod caret_bounds_tests {
    use super::*;
    use core_graphics::geometry::{CGPoint, CGSize};
    #[test]
    fn missing_role_keeps_element_geometry_without_inventing_a_caret() {
        let bounds = [-200.0, 100.0, 400.0, 300.0];
        let observation = observe_element(None, |_| panic!("unknown role must not query caret providers"), || Some(bounds));
        assert_eq!(observation.state, FocusState::Present);
        assert_eq!(observation.role, None);
        assert_eq!(observation.bounds, Some(bounds));
        assert!(observation.caret.is_none());

        let unavailable = observe_element(None, |_| panic!("unknown role"), || None);
        assert_eq!(unavailable.state, FocusState::Unavailable);
        assert!(unavailable.caret.is_none());
    }

    #[test]
    fn failed_accessibility_queries_are_not_focus_loss() {
        assert_eq!(focused_attribute_state(K_AX_ERROR_SUCCESS, true), FocusState::Present);
        assert_eq!(focused_attribute_state(K_AX_ERROR_NO_VALUE, false), FocusState::Absent);
        for status in [K_AX_ERROR_SUCCESS, -25201, -25202, -25204, -25205, -25211] {
            assert_eq!(focused_attribute_state(status, false), FocusState::Unavailable);
        }
    }

    #[test]
    fn rejects_empty_provider_bounds_but_keeps_insertion_carets() {
        let rect=|x,y,w,h|CGRect::new(&CGPoint::new(x,y),&CGSize::new(w,h));
        assert!(!usable_caret_bounds(rect(0.0,956.0,0.0,0.0)));
        assert!(!usable_caret_bounds(rect(f64::NAN,10.0,1.0,16.0)));
        assert!(usable_caret_bounds(rect(0.0,0.0,0.0,16.0)));
        assert!(usable_caret_bounds(rect(-800.0,-300.0,0.0,16.0)));
    }
}
