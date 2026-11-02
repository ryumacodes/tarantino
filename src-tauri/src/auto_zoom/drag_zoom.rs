//! Keep automatic zoom active until a pointer selection or drag finishes.
use super::{ZoomBlock, point_in_capture};
use crate::{event_capture::CaptureSession, input::{MouseButton, MouseEventType}};

pub(super) fn extend_drag_zooms(blocks: &mut [ZoomBlock], session: &CaptureSession) {
    let duration = session.end_time.unwrap_or(session.start_time).saturating_sub(session.start_time);
    let mut press = None;
    let mut moved = false;
    for event in &session.mouse_events {
        let event = &event.base;
        match event.event_type {
            MouseEventType::ButtonPress { button: MouseButton::Left } => {
                press = point_in_capture(event.x,event.y,&session.metadata)
                    .then_some((event.timestamp,event.x,event.y));
                moved = false;
            }
            MouseEventType::Move | MouseEventType::ButtonRelease { button: MouseButton::Left } => {
                if let Some((start,x,y)) = press {
                    moved |= (event.x-x).hypot(event.y-y) >= 4.0;
                    if matches!(event.event_type,MouseEventType::ButtonRelease {..}) {
                        if moved { extend(blocks,start,event.timestamp,duration); }
                        press = None;
                    }
                }
            }
            _ => {}
        }
    }
    // Stopping while the button is held must not zoom out during the drag.
    if let Some((start,_,_)) = press { if moved { extend(blocks,start,duration,duration); } }
}
fn extend(blocks:&mut [ZoomBlock],start:u64,end:u64,duration:u64) {
    // Nearby presses may be suppressed by click spacing. Their drag still
    // belongs to the active click block (for example double-click word selection).
    if let Some(block)=blocks.iter_mut().rev().find(|b| {
        b.centers.iter().any(|c|c.time==start)
            || (b.start_time<=start && start<=b.end_time
                && b.centers.iter().any(|c|c.time<=start))
    }) {
        block.end_time=block.end_time.max(end.saturating_add(3000).min(duration));
    }
}
