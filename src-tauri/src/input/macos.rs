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
pub(super) use pointer::install as install_pointer_listener;

use core_foundation::base::{CFGetTypeID, CFRelease, CFType, CFTypeID, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use std::ffi::c_void;

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
const K_AX_VALUE_CG_POINT_TYPE: i32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: i32 = 2;
const K_AX_VALUE_CF_RANGE_TYPE: i32 = 4;
const K_AX_VALUE_CG_RECT_TYPE: i32 = 3;
const MAX_CARET_DESCENDANTS: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
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
    if !query_budget::available() { return None; }
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
    if !query_budget::available() { return None; }
    let mut value: CFTypeRef = std::ptr::null();
    let status = AXUIElementCopyAttributeValue(element, attribute, &mut value);
    if status != K_AX_ERROR_SUCCESS || value.is_null() {
        None
    } else {
        Some(value)
    }
}

unsafe fn caret_from_focused_element(
    parent: AXUIElementRef,
    focused_element_attr: CFStringRef,
    attrs: &CaretAttributes,
) -> Option<(f64, f64)> {
    let focused = CFType::wrap_under_create_rule(copy_attribute(parent, focused_element_attr)?);
    let related = |element: &CFType, attribute: &CFString| {
        let value = CFType::wrap_under_create_rule(copy_attribute(
            element.as_CFTypeRef() as AXUIElementRef, attribute.as_concrete_TypeRef(),
        )?);
        (CFGetTypeID(value.as_CFTypeRef()) == AXUIElementGetTypeID()).then_some(value)
    };
    let mut focused = focused_element::resolve(focused,
        |element| related(element, &attrs.active_element),
        |element| related(element, &attrs.editable_ancestor));
    for _ in 0..MAX_CARET_DESCENDANTS {
        let Some(child) = copy_selected_child(focused.as_CFTypeRef() as AXUIElementRef) else { break; };
        focused = child;
    }
    let element = focused.as_CFTypeRef() as AXUIElementRef;
    let mut visited = 0;
    caret_position_for_element(element, attrs)
        .or_else(|| caret_position_from_related_elements(element, attrs))
        .or_else(|| caret_position_from_descendants(element, attrs, &mut visited))
}

unsafe fn copy_selected_child(element: AXUIElementRef) -> Option<CFType> {
    let role = CFType::wrap_under_create_rule(copy_attribute(
        element, CFString::new("AXRole").as_concrete_TypeRef(),
    )?);
    if CFGetTypeID(role.as_CFTypeRef()) != CFString::type_id() { return None; }
    let role = CFString::wrap_under_get_rule(role.as_CFTypeRef() as CFStringRef).to_string();
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
) -> Option<(f64, f64)> {
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
    caret_range::position(range, |range| {
        let parameter = create_range_value(range)?;
        let bounds = copy_bounds_for_range(element, bounds_for_range_attr, parameter);
        CFRelease(parameter);
        bounds
    }, || copy_element_bounds(element))
}

unsafe fn copy_element_bounds(element: AXUIElementRef) -> Option<CGRect> {
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
    usable_caret_bounds(bounds).then_some(bounds)
}

unsafe fn caret_position_from_text_marker(
    element: AXUIElementRef,
    selected_text_marker_range_attr: CFStringRef,
    bounds_for_text_marker_range_attr: CFStringRef,
) -> Option<(f64, f64)> {
    let range_value = copy_attribute(element, selected_text_marker_range_attr)?;
    let bounds = copy_rect_for_parameterized_attribute(
        element,
        bounds_for_text_marker_range_attr,
        range_value,
    );
    CFRelease(range_value);

    bounds.map(caret_range::text_marker_position)
}

struct CaretAttributes {
    selected_range: CFString,
    bounds_for_range: CFString,
    selected_text_marker_range: CFString,
    bounds_for_text_marker_range: CFString,
    active_element: CFString,
    editable_ancestor: CFString,
    highest_editable_ancestor: CFString,
    focusable_ancestor: CFString,
    children: CFString,
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
            highest_editable_ancestor: CFString::new("AXHighestEditableAncestor"),
            focusable_ancestor: CFString::new("AXFocusableAncestor"),
            children: CFString::new("AXChildren"),
        }
    }
}

unsafe fn caret_position_for_element(
    element: AXUIElementRef,
    attrs: &CaretAttributes,
) -> Option<(f64, f64)> {
    caret_position_from_cf_range(
        element,
        attrs.selected_range.as_concrete_TypeRef(),
        attrs.bounds_for_range.as_concrete_TypeRef(),
    )
    .or_else(|| {
        caret_position_from_text_marker(
            element,
            attrs.selected_text_marker_range.as_concrete_TypeRef(),
            attrs.bounds_for_text_marker_range.as_concrete_TypeRef(),
        )
    })
}

unsafe fn caret_position_from_related_elements(
    element: AXUIElementRef,
    attrs: &CaretAttributes,
) -> Option<(f64, f64)> {
    let related_attrs = [
        attrs.active_element.as_concrete_TypeRef(),
        attrs.editable_ancestor.as_concrete_TypeRef(),
        attrs.highest_editable_ancestor.as_concrete_TypeRef(),
        attrs.focusable_ancestor.as_concrete_TypeRef(),
    ];

    for attr in related_attrs {
        if let Some(related) = copy_attribute(element, attr) {
            let caret = caret_position_for_element(related as AXUIElementRef, attrs);
            CFRelease(related);
            if caret.is_some() {
                return caret;
            }
        }
    }

    None
}

unsafe fn caret_position_from_descendants(
    element: AXUIElementRef,
    attrs: &CaretAttributes,
    visited: &mut usize,
) -> Option<(f64, f64)> {
    if *visited >= MAX_CARET_DESCENDANTS {
        return None;
    }

    let children = copy_attribute(element, attrs.children.as_concrete_TypeRef())?;
    let count = CFArrayGetCount(children as CFArrayRef).max(0) as usize;

    for idx in 0..count {
        if *visited >= MAX_CARET_DESCENDANTS {
            break;
        }

        let child = CFArrayGetValueAtIndex(children as CFArrayRef, idx as CFIndex);
        if child.is_null() {
            continue;
        }

        *visited += 1;
        let child_element = child as AXUIElementRef;
        if let Some(caret) = caret_position_for_element(child_element, attrs)
            .or_else(|| caret_position_from_related_elements(child_element, attrs))
            .or_else(|| caret_position_from_descendants(child_element, attrs, visited))
        {
            CFRelease(children);
            return Some(caret);
        }
    }

    CFRelease(children);
    None
}

pub fn focused_caret_position() -> Option<(f64, f64)> {
    let _budget = query_budget::LookupBudget::begin();
    let focused_app_attr = CFString::new("AXFocusedApplication");
    let focused_attr = CFString::new("AXFocusedUIElement");
    let attrs = CaretAttributes::new();

    unsafe {
        let system = AXUIElementCreateSystemWide();
        if system.is_null() {
            return None;
        }
        // System-wide AX objects configure this process's messaging timeout.
        // A slow application must not hold up subsequent keyboard events.
        if AXUIElementSetMessagingTimeout(system, 0.02) != K_AX_ERROR_SUCCESS {
            CFRelease(system as CFTypeRef);
            return None;
        }

        let caret = caret_from_focused_element(system, focused_attr.as_concrete_TypeRef(), &attrs)
            .or_else(|| {
                let app = CFType::wrap_under_create_rule(copy_attribute(
                    system, focused_app_attr.as_concrete_TypeRef(),
                )?);
                caret_from_focused_element(
                    app.as_CFTypeRef() as AXUIElementRef,
                    focused_attr.as_concrete_TypeRef(),
                    &attrs,
                )
            });
        CFRelease(system as CFTypeRef);
        caret
    }
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
    fn rejects_empty_provider_bounds_but_keeps_insertion_carets() {
        let rect=|x,y,w,h|CGRect::new(&CGPoint::new(x,y),&CGSize::new(w,h));
        assert!(!usable_caret_bounds(rect(0.0,956.0,0.0,0.0)));
        assert!(!usable_caret_bounds(rect(f64::NAN,10.0,1.0,16.0)));
        assert!(usable_caret_bounds(rect(0.0,0.0,0.0,16.0)));
        assert!(usable_caret_bounds(rect(-800.0,-300.0,0.0,16.0)));
    }
}
