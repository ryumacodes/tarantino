//! Main-thread AppKit bitmap observations; no Objective-C objects escape the pool.
#![allow(deprecated, unexpected_cfgs)]
use cocoa::{base::{id, nil}, foundation::{NSPoint, NSRect, NSSize}};
use objc::{class, msg_send, sel, sel_impl};
use std::hash::{Hash, Hasher};
use crate::input::cursor_images::SystemCursor;

pub(super) struct Bitmap {
    pub rep: id,
    pub size: NSSize,
    pub hotspot: NSPoint,
    pub fingerprint: u64,
}

/// Caller owns an autorelease pool and runs on the main thread.
pub(super) unsafe fn read(cursor: id) -> Option<Bitmap> {
    unsafe {
        if cursor == nil { return None; }
        let image: id = msg_send![cursor, image];
        if image == nil { return None; }
        let size: NSSize = msg_send![image, size];
        let hotspot: NSPoint = msg_send![cursor, hotSpot];
        if ![size.width,size.height,hotspot.x,hotspot.y].iter().all(|v|v.is_finite())
            || size.width <= 0.0 || size.height <= 0.0 { return None; }
        let cg: *const std::ffi::c_void = msg_send![image,
            CGImageForProposedRect: std::ptr::null_mut::<NSRect>() context: nil hints: nil];
        if cg.is_null() { return None; }
        let rep: id = msg_send![class!(NSBitmapImageRep), alloc];
        let rep: id = msg_send![rep, initWithCGImage: cg];
        if rep == nil { return None; }
        let rep: id = msg_send![rep, autorelease];
        let stride: usize = msg_send![rep, bytesPerRow];
        let height: usize = msg_send![rep, pixelsHigh];
        let width: usize = msg_send![rep, pixelsWide];
        let bits: usize = msg_send![rep, bitsPerPixel];
        let planar: bool = msg_send![rep, isPlanar];
        let format: usize = msg_send![rep, bitmapFormat];
        let row = width.checked_mul(bits)?.checked_add(7)? / 8;
        let length = stride.checked_mul(height)?;
        if planar || row == 0 || row > stride || length == 0 || length > 16 * 1024 * 1024 { return None; }
        let data: *const u8 = msg_send![rep, bitmapData];
        if data.is_null() { return None; }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (width,height,bits,format).hash(&mut hash);
        // Padding is not part of the appearance and may differ across allocations.
        for y in 0..height { std::slice::from_raw_parts(data.add(y*stride),row).hash(&mut hash); }
        for value in [size.width,size.height,hotspot.x,hotspot.y] { value.to_bits().hash(&mut hash); }
        Some(Bitmap { rep, size, hotspot, fingerprint: hash.finish() })
    }
}

pub(super) unsafe fn identify(fingerprint: u64) -> Option<SystemCursor> {
    use SystemCursor::*;
    unsafe {
        let cursors = [
            (Arrow,sel!(arrowCursor)),
            (Text,sel!(IBeamCursor)),
            (Hand,sel!(pointingHandCursor)),
            (Crosshair,sel!(crosshairCursor)),
            (ClosedHand,sel!(closedHandCursor)),
            (OpenHand,sel!(openHandCursor)),
            (ResizeLeftRight,sel!(resizeLeftRightCursor)),
            (ResizeUpDown,sel!(resizeUpDownCursor)),
            (ResizeLeft,sel!(resizeLeftCursor)),
            (ResizeRight,sel!(resizeRightCursor)),
            (ResizeUp,sel!(resizeUpCursor)),
            (ResizeDown,sel!(resizeDownCursor)),
            (VerticalText,sel!(IBeamCursorForVerticalLayout)),
            (DisappearingItem,sel!(disappearingItemCursor)),
            (NotAllowed,sel!(operationNotAllowedCursor)),
            (DragLink,sel!(dragLinkCursor)),
            (DragCopy,sel!(dragCopyCursor)),
            (ContextualMenu,sel!(contextualMenuCursor)),
            (ZoomIn,sel!(zoomInCursor)),
            (ZoomOut,sel!(zoomOutCursor)),
        ];
        cursors.into_iter().find_map(|(kind,selector)| {
            // Some public shapes were added after our minimum supported macOS.
            let available: bool = msg_send![class!(NSCursor), respondsToSelector: selector];
            if !available { return None; }
            let cursor: id = msg_send![class!(NSCursor), performSelector: selector];
            read(cursor).filter(|bitmap| bitmap.fingerprint == fingerprint).map(|_|kind)
        })
    }
}
