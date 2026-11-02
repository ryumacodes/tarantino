//! Recording shortcuts use event-local Quartz flags, independent of caret queries.
use core_graphics::event::{CGEvent, CGEventFlags, CGEventType, EventField};
use std::sync::Arc;
use tauri::Manager;

static APP: once_cell::sync::OnceCell<tauri::AppHandle> = once_cell::sync::OnceCell::new();
pub fn install(app: &tauri::AppHandle) { let _ = APP.set(app.clone()); }

#[derive(Debug, PartialEq)]
enum Action { Stop, Restart }

#[derive(Default)]
pub(crate) struct Chord { r_down: bool }

impl Chord {
    fn update(&mut self, kind: CGEventType, key: i64, flags: CGEventFlags, repeat: bool) -> Option<Action> {
        // ANSI R virtual key. Modifiers come from this event, so a listener
        // started while modifiers are already held still recognizes the chord.
        if key != 15 { return None; }
        if matches!(kind, CGEventType::KeyUp) { self.r_down = false; return None; }
        if !matches!(kind, CGEventType::KeyDown) { return None; }
        let was_down = self.r_down;
        self.r_down = true;
        let required = CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagAlternate;
        if repeat || was_down || !flags.contains(required) { return None; }
        Some(if flags.contains(CGEventFlags::CGEventFlagShift) { Action::Restart } else { Action::Stop })
    }

    pub(crate) fn handle(&mut self, kind: CGEventType, event: &CGEvent) {
        let Some(action) = self.update(kind,
            event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
            event.get_flags(),
            event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0,
        ) else { return; };
        let Some(app) = APP.get().cloned() else { return; };
        // Never wait for recording state, accessibility, or finalization in the tap.
        tauri::async_runtime::spawn(async move {
            let state = app.state::<Arc<crate::state::UnifiedAppState>>();
            if !state.recording.is_recording() { return; }
            let result = match action {
                Action::Stop => crate::commands::record_stop_instant(app.clone(), state).await.map(|_| ()),
                Action::Restart => crate::recording_commands::record_restart_new(app.clone(), state).await.map(|_| ()),
            };
            if let Err(error) = result { eprintln!("Recording shortcut failed: {error}"); }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn modifiers() -> CGEventFlags {
        CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagAlternate
    }
    #[test]
    fn recognizes_held_modifiers_without_prior_modifier_events() {
        let mut chord = Chord::default();
        assert_eq!(chord.update(CGEventType::KeyDown, 15, modifiers(), false), Some(Action::Stop));
        assert_eq!(chord.update(CGEventType::KeyDown, 15, modifiers(), true), None);
        assert_eq!(chord.update(CGEventType::KeyDown, 15, modifiers(), false), None);
        chord.update(CGEventType::KeyUp, 15, CGEventFlags::empty(), false);
        assert_eq!(chord.update(CGEventType::KeyDown, 15, modifiers() | CGEventFlags::CGEventFlagShift, false), Some(Action::Restart));
    }
    #[test]
    fn current_flags_prevent_stale_modifiers_and_unrelated_keys() {
        let mut chord = Chord::default();
        assert_eq!(chord.update(CGEventType::KeyDown, 14, modifiers(), false), None);
        for missing in [CGEventFlags::CGEventFlagCommand, CGEventFlags::CGEventFlagControl, CGEventFlags::CGEventFlagAlternate] {
            assert_eq!(chord.update(CGEventType::KeyDown, 15, modifiers() - missing, false), None);
            chord.update(CGEventType::KeyUp, 15, modifiers(), false);
        }
        assert_eq!(chord.update(CGEventType::KeyDown, 15, modifiers(), true), None);
    }
}
