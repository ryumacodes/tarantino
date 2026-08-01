//! macOS Accessibility caret lookup.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use std::ffi::c_void;

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
    fn CFArrayGetCount(theArray: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(theArray: CFArrayRef, idx: CFIndex) -> *const c_void;
}

unsafe fn copy_rect_for_parameterized_attribute(
    element: AXUIElementRef,
    attribute: CFStringRef,
    parameter: CFTypeRef,
) -> Option<CGRect> {
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

    has_bounds.then_some(bounds)
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
    let mut value: CFTypeRef = std::ptr::null();
    let status = AXUIElementCopyAttributeValue(element, attribute, &mut value);
    if status != K_AX_ERROR_SUCCESS || value.is_null() {
        None
    } else {
        Some(value)
    }
}

unsafe fn copy_focused_elements(
    system: AXUIElementRef,
    focused_app_attr: CFStringRef,
    focused_element_attr: CFStringRef,
) -> Vec<CFTypeRef> {
    let mut elements = Vec::new();

    if let Some(focused) = copy_attribute(system, focused_element_attr) {
        elements.push(focused);
    }

    if let Some(app) = copy_attribute(system, focused_app_attr) {
        if let Some(focused) = copy_attribute(app as AXUIElementRef, focused_element_attr) {
            elements.push(focused);
        }
        CFRelease(app);
    }

    elements
}

unsafe fn copy_range_attribute(element: AXUIElementRef, attribute: CFStringRef) -> Option<CFRange> {
    let value = copy_attribute(element, attribute)?;
    let mut range = CFRange::default();
    let ok = AXValueGetValue(
        value,
        K_AX_VALUE_CF_RANGE_TYPE,
        &mut range as *mut _ as *mut c_void,
    );
    CFRelease(value);
    ok.then_some(range)
}

unsafe fn focused_element_frame(
    focused: AXUIElementRef,
    position_attr: CFStringRef,
    size_attr: CFStringRef,
) -> Option<CGRect> {
    let position_value = copy_attribute(focused, position_attr)?;
    let size_value = match copy_attribute(focused, size_attr) {
        Some(value) => value,
        None => {
            CFRelease(position_value);
            return None;
        }
    };

    let mut origin = CGPoint::default();
    let mut size = CGSize::default();
    let has_origin = AXValueGetValue(
        position_value,
        K_AX_VALUE_CG_POINT_TYPE,
        &mut origin as *mut _ as *mut c_void,
    );
    let has_size = AXValueGetValue(
        size_value,
        K_AX_VALUE_CG_SIZE_TYPE,
        &mut size as *mut _ as *mut c_void,
    );
    CFRelease(position_value);
    CFRelease(size_value);

    (has_origin && has_size && size.width > 0.0 && size.height > 0.0)
        .then_some(CGRect::new(&origin, &size))
}

fn estimated_caret_from_visible_range(
    element_frame: CGRect,
    selected_range: CFRange,
    visible_range: CFRange,
) -> Option<(f64, f64)> {
    if visible_range.length <= 1 || selected_range.location < visible_range.location {
        return None;
    }

    let offset =
        (selected_range.location - visible_range.location).clamp(0, visible_range.length) as f64;
    let progress = (offset / visible_range.length as f64).clamp(0.0, 1.0);
    Some((
        element_frame.origin.x + element_frame.size.width * progress,
        element_frame.origin.y + element_frame.size.height / 2.0,
    ))
}

unsafe fn caret_position_from_cf_range(
    element: AXUIElementRef,
    selected_range_attr: CFStringRef,
    bounds_for_range_attr: CFStringRef,
    position_attr: CFStringRef,
    size_attr: CFStringRef,
    visible_range_attr: CFStringRef,
) -> Option<(f64, f64)> {
    let mut range_value: CFTypeRef = std::ptr::null();
    let range_status =
        AXUIElementCopyAttributeValue(element, selected_range_attr, &mut range_value);
    if range_status != K_AX_ERROR_SUCCESS || range_value.is_null() {
        return None;
    }

    let mut range = CFRange::default();
    if !AXValueGetValue(
        range_value,
        K_AX_VALUE_CF_RANGE_TYPE,
        &mut range as *mut _ as *mut c_void,
    ) {
        CFRelease(range_value);
        return None;
    }

    let element_frame = focused_element_frame(element, position_attr, size_attr);
    let visible_range = copy_range_attribute(element, visible_range_attr);
    let selected_bounds = copy_bounds_for_range(element, bounds_for_range_attr, range_value);

    CFRelease(range_value);

    if range.length == 0 && range.location > 0 {
        selected_bounds
            .map(|bounds| {
                (
                    bounds.origin.x + bounds.size.width / 2.0,
                    bounds.origin.y + bounds.size.height / 2.0,
                )
            })
            .or_else(|| {
                let previous_char_range = CFRange {
                    location: range.location - 1,
                    length: 1,
                };

                create_range_value(previous_char_range).and_then(|previous_char_value| {
                    let bounds =
                        copy_bounds_for_range(element, bounds_for_range_attr, previous_char_value);
                    CFRelease(previous_char_value);
                    bounds.map(|bounds| {
                        (
                            bounds.origin.x + bounds.size.width,
                            bounds.origin.y + bounds.size.height / 2.0,
                        )
                    })
                })
            })
            .or_else(|| {
                element_frame
                    .zip(visible_range)
                    .and_then(|(frame, visible)| {
                        estimated_caret_from_visible_range(frame, range, visible)
                    })
            })
    } else {
        selected_bounds.map(|bounds| {
            (
                bounds.origin.x + bounds.size.width / 2.0,
                bounds.origin.y + bounds.size.height / 2.0,
            )
        })
    }
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

    bounds.map(|bounds| {
        (
            bounds.origin.x + bounds.size.width / 2.0,
            bounds.origin.y + bounds.size.height / 2.0,
        )
    })
}

struct CaretAttributes {
    selected_range: CFString,
    bounds_for_range: CFString,
    position: CFString,
    size: CFString,
    visible_range: CFString,
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
            position: CFString::new("AXPosition"),
            size: CFString::new("AXSize"),
            visible_range: CFString::new("AXVisibleCharacterRange"),
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
        attrs.position.as_concrete_TypeRef(),
        attrs.size.as_concrete_TypeRef(),
        attrs.visible_range.as_concrete_TypeRef(),
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
    let focused_app_attr = CFString::new("AXFocusedApplication");
    let focused_attr = CFString::new("AXFocusedUIElement");
    let attrs = CaretAttributes::new();

    unsafe {
        let system = AXUIElementCreateSystemWide();
        if system.is_null() {
            return None;
        }

        let focused_elements = copy_focused_elements(
            system,
            focused_app_attr.as_concrete_TypeRef(),
            focused_attr.as_concrete_TypeRef(),
        );
        CFRelease(system as CFTypeRef);

        for focused in focused_elements {
            let mut visited = 0;
            let focused_element = focused as AXUIElementRef;
            let caret_position = caret_position_for_element(focused_element, &attrs)
                .or_else(|| caret_position_from_related_elements(focused_element, &attrs))
                .or_else(|| caret_position_from_descendants(focused_element, &attrs, &mut visited));

            CFRelease(focused);

            if caret_position.is_some() {
                return caret_position;
            }
        }

        None
    }
}
