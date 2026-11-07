//! Preserve Quartz coordinates on every pointer event, including button drags.
use crate::input::{MouseButton,MouseEvent,MouseEventType,MouseTracker,LAST_MOUSE_POSITION};
use core_foundation::{base::TCFType,runloop::{CFRunLoop,kCFRunLoopCommonModes}};
use core_graphics::event::{CGEvent,CGEventTap,CGEventTapLocation,CGEventTapOptions,CGEventTapPlacement,CGEventType,CallbackResult,EventField};
use parking_lot::Mutex;
use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
use std::time::{SystemTime,UNIX_EPOCH};

fn pointer_event(kind:CGEventType,event:&CGEvent,timestamp:u64)->Option<MouseEvent>{
    use CGEventType::*;
    let button=match kind {
        LeftMouseDown|LeftMouseUp=>MouseButton::Left,
        RightMouseDown|RightMouseUp=>MouseButton::Right,
        _=>match event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER) {
            2=>MouseButton::Middle,_=>MouseButton::Unknown,
        },
    };
    let event_type=match kind {
        MouseMoved|LeftMouseDragged|RightMouseDragged|OtherMouseDragged=>MouseEventType::Move,
        LeftMouseDown|RightMouseDown|OtherMouseDown=>MouseEventType::ButtonPress{button},
        LeftMouseUp|RightMouseUp|OtherMouseUp=>MouseEventType::ButtonRelease{button},
        ScrollWheel=>MouseEventType::Wheel{
            delta_x:event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2),
            delta_y:event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1),
        },
        _=>return None,
    };
    let point=event.location();
    if !point.x.is_finite() || !point.y.is_finite(){return None;}
    Some(MouseEvent{timestamp,x:point.x,y:point.y,event_type,display_id:None})
}

pub(in crate::input) fn install(tracker:Arc<Mutex<MouseTracker>>)->anyhow::Result<()> {
    let (ready,receiver)=std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        use CGEventType::*;
        let port=Arc::new(AtomicUsize::new(0));
        let callback_port=port.clone();
        let shortcuts=Mutex::new(crate::capture_hotkeys::Chord::default());
        let keyboard = super::keyboard::start(tracker.clone());
        let focus = Mutex::new(super::focus_capture::Nudger::start(tracker.clone()));
        let tap=CGEventTap::new(CGEventTapLocation::HID,CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,vec![MouseMoved,LeftMouseDragged,RightMouseDragged,OtherMouseDragged,
                LeftMouseDown,LeftMouseUp,RightMouseDown,RightMouseUp,OtherMouseDown,OtherMouseUp,ScrollWheel,KeyDown,KeyUp,FlagsChanged],
            move |_,kind,event| {
                if matches!(kind,TapDisabledByTimeout|TapDisabledByUserInput){
                    *shortcuts.lock()=crate::capture_hotkeys::Chord::default();
                    tracker.lock().shortcuts.reset();
                    let port=callback_port.load(Ordering::Acquire);
                    if port!=0 { unsafe { CGEventTapEnable(port as *const std::ffi::c_void,true); } }
                    return CallbackResult::Keep;
                }
                let time = SystemTime::now();
                let timestamp = time.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                if matches!(kind,LeftMouseDown|RightMouseDown|OtherMouseDown|KeyDown|FlagsChanged) {
                    let state = tracker.lock();
                    if state.is_tracking {
                        if let Some(session) = state.recording_start_time {
                            use crate::input::focus_samples::FocusTrigger;
                            let trigger = match kind {
                                KeyDown => FocusTrigger::Key,
                                FlagsChanged => FocusTrigger::Modifiers,
                                _ => FocusTrigger::Pointer,
                            };
                            focus.lock().request(session, std::time::Instant::now(), timestamp, trigger);
                        }
                    }
                }
                if matches!(kind,KeyDown|KeyUp|FlagsChanged) {
                    shortcuts.lock().handle(kind,event);
                    let mut state=tracker.lock();
                    if state.is_tracking {
                        if let Some(session) = state.recording_start_time {
                            let pointer = *LAST_MOUSE_POSITION.lock();
                            if let Some(input) = super::keyboard::capture(kind, event, time, timestamp, session, pointer) {
                                let _ = keyboard.send(input);
                            }
                        }
                        if let Some(sample)=state.shortcuts.capture(kind,event,timestamp) {
                            state.add_key_event(sample);
                        }
                    }
                    return CallbackResult::Keep;
                }
                if let Some(event)=pointer_event(kind,event,timestamp){
                    let state=tracker.lock();
                    if state.is_tracking {
                        *LAST_MOUSE_POSITION.lock()=(event.x,event.y);
                        state.add_event(event);
                    }
                }
                CallbackResult::Keep
            });
        let Ok(tap)=tap else {let _=ready.send(Err("Could not create pointer event tap"));return;};
        let Ok(source)=tap.mach_port().create_runloop_source(0) else {let _=ready.send(Err("Could not create pointer run loop"));return;};
        port.store(tap.mach_port().as_concrete_TypeRef() as usize,Ordering::Release);
        CFRunLoop::get_current().add_source(&source,unsafe{kCFRunLoopCommonModes});
        tap.enable();
        let _=ready.send(Ok(()));
        CFRunLoop::run_current();
    });
    receiver.recv().map_err(|_|anyhow::anyhow!("Pointer listener stopped during startup"))?
        .map_err(|message|anyhow::anyhow!(message))
}

#[link(name="ApplicationServices",kind="framework")]
unsafe extern "C" {fn CGEventTapEnable(tap:*const std::ffi::c_void,enable:bool);}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::{event::CGMouseButton,event_source::{CGEventSource,CGEventSourceStateID},geometry::CGPoint};
    #[test]
    fn every_drag_variant_preserves_quartz_desktop_coordinates() {
        for kind in [CGEventType::MouseMoved,CGEventType::LeftMouseDragged,CGEventType::RightMouseDragged,CGEventType::OtherMouseDragged] {
            let source=CGEventSource::new(CGEventSourceStateID::CombinedSessionState).unwrap();
            // Construct only; never post an event to the user's desktop.
            let event=CGEvent::new_mouse_event(source,kind,CGPoint::new(-210.5,340.25),CGMouseButton::Left).unwrap();
            let captured=pointer_event(kind,&event,123).unwrap();
            assert_eq!((captured.x,captured.y,captured.timestamp),(-210.5,340.25,123));
            assert!(matches!(captured.event_type,MouseEventType::Move));
        }
    }
    #[test]
    fn button_events_use_their_own_location_and_do_not_require_a_prior_move() {
        for (kind,button) in [(CGEventType::LeftMouseDown,CGMouseButton::Left),(CGEventType::RightMouseUp,CGMouseButton::Right),(CGEventType::OtherMouseDown,CGMouseButton::Center)] {
            let source=CGEventSource::new(CGEventSourceStateID::CombinedSessionState).unwrap();
            let event=CGEvent::new_mouse_event(source,kind,CGPoint::new(1170.25,-44.5),button).unwrap();
            let captured=pointer_event(kind,&event,456).unwrap();
            assert_eq!((captured.x,captured.y),(1170.25,-44.5));
            match kind {
                CGEventType::LeftMouseDown=>assert!(matches!(captured.event_type,MouseEventType::ButtonPress{button:MouseButton::Left})),
                CGEventType::RightMouseUp=>assert!(matches!(captured.event_type,MouseEventType::ButtonRelease{button:MouseButton::Right})),
                _=>assert!(matches!(captured.event_type,MouseEventType::ButtonPress{button:MouseButton::Middle})),
            }
        }
    }
}
