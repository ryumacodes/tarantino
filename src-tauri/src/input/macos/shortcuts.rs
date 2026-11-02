//! Opt-in shortcut labels from event-local Quartz flags and Unicode.
use crate::input::{KeyEvent, ShortcutModifier, ShortcutSample};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventType, EventField};
use foreign_types::ForeignType;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(in crate::input) struct Recorder {
    enabled: bool,
    held: HashMap<i64, String>,
    caps_lock: Option<bool>,
}
impl Recorder {
    pub(in crate::input) fn configure(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.reset();
    }
    pub(in crate::input) fn reset(&mut self) { self.held.clear(); self.caps_lock=None; }

    pub(super) fn capture(&mut self, kind: CGEventType, event: &CGEvent, timestamp: u64) -> Option<KeyEvent> {
        if !self.enabled { return None; }
        let flags = event.get_flags();
        let modifiers = modifiers(flags);
        let code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
        let repeat = event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0;
        let key = match kind {
            CGEventType::FlagsChanged => {
                let caps=flags.contains(CGEventFlags::CGEventFlagAlphaShift);
                let changed=self.caps_lock.replace(caps)!=Some(caps);
                (code==57&&changed).then(||"CapsLock".to_string())
            },
            CGEventType::KeyUp => Some(self.held.remove(&code)?),
            CGEventType::KeyDown => {
                let command = flags.intersects(CGEventFlags::CGEventFlagCommand
                    | CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagAlternate);
                let named_modifier=command||flags.intersects(CGEventFlags::CGEventFlagShift|CGEventFlags::CGEventFlagAlphaShift);
                let named = named_key(code).filter(|name| named_modifier || !matches!(*name, "Enter" | "Tab" | " " | "Escape"));
                if !command && named.is_none() { return None; }
                if let Some(key) = self.held.get(&code) { Some(key.clone()) }
                else {
                    // Never read character content for ordinary typing.
                    let key = if let Some(named) = named { named.to_string() }
                        else if command { printable_character(event)? }
                        else { return None; };
                    self.held.insert(code, key.clone());
                    Some(key)
                }
            }
            _ => return None,
        };
        let mut captured = KeyEvent::release(timestamp, key.is_none());
        captured.is_release = matches!(kind, CGEventType::KeyUp) || key.is_none();
        captured.shortcut = Some(ShortcutSample { key, modifiers, repeat });
        Some(captured)
    }
}

fn modifiers(flags: CGEventFlags) -> Vec<ShortcutModifier> {
    use CGEventFlags as F;
    use ShortcutModifier as M;
    [(F::CGEventFlagControl,M::Control),(F::CGEventFlagAlternate,M::Option),
        (F::CGEventFlagShift,M::Shift),(F::CGEventFlagCommand,M::Command),
        (F::CGEventFlagAlphaShift,M::CapsLock),(F::CGEventFlagSecondaryFn,M::Function)]
        .into_iter().filter_map(|(flag,name)|flags.contains(flag).then_some(name)).collect()
}

fn named_key(code: i64) -> Option<&'static str> {
    Some(match code {
        36|76=>"Enter",48=>"Tab",49=>" ",51=>"Backspace",53=>"Escape",57=>"CapsLock",
        115=>"Home",116=>"PageUp",117=>"Delete",119=>"End",121=>"PageDown",
        123=>"ArrowLeft",124=>"ArrowRight",125=>"ArrowDown",126=>"ArrowUp",
        122=>"F1",120=>"F2",99=>"F3",118=>"F4",96=>"F5",97=>"F6",98=>"F7",100=>"F8",
        101=>"F9",109=>"F10",103=>"F11",111=>"F12",105=>"F13",107=>"F14",113=>"F15",
        106=>"F16",64=>"F17",79=>"F18",80=>"F19",90=>"F20",_=>return None,
    })
}

fn printable_character(event: &CGEvent) -> Option<String> {
    let mut units = [0u16; 8];
    let mut length = 0;
    unsafe { CGEventKeyboardGetUnicodeString(event.as_ptr().cast(), units.len(), &mut length, units.as_mut_ptr()); }
    if length == 0 || length > units.len() { return None; }
    let text = String::from_utf16(&units[..length]).ok()?;
    // Do not guess a US-layout letter from a physical keycode. Unsupported
    // control/dead-key sequences remain absent rather than displaying a lie.
    (text.chars().count() == 1 && !text.chars().any(char::is_control)).then_some(text)
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventKeyboardGetUnicodeString(event: *const std::ffi::c_void, max: usize,
        actual: *mut usize, buffer: *mut u16);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::event_source::{CGEventSource,CGEventSourceStateID};
    fn event(code:u16,text:&str,flags:CGEventFlags)->CGEvent {
        let source=CGEventSource::new(CGEventSourceStateID::CombinedSessionState).unwrap();
        let event=CGEvent::new_keyboard_event(source,code,true).unwrap();
        event.set_string(text);event.set_flags(flags);event
    }
    #[test]
    fn disabled_and_plain_typing_do_not_persist_characters() {
        let mut recorder=Recorder::default();
        let e=event(0,"a",CGEventFlags::CGEventFlagCommand);
        assert!(recorder.capture(CGEventType::KeyDown,&e,100).is_none());
        recorder.configure(true);
        for flags in [CGEventFlags::empty(),CGEventFlags::CGEventFlagShift,CGEventFlags::CGEventFlagAlphaShift] {
            e.set_flags(flags);
            assert!(recorder.capture(CGEventType::KeyDown,&e,100).is_none());
        }
    }
    #[test]
    fn current_flags_and_layout_label_survive_repeat_and_modifier_release() {
        let mut recorder=Recorder::default();recorder.configure(true);
        let e=event(0,"é",CGEventFlags::CGEventFlagCommand|CGEventFlags::CGEventFlagShift);
        let down=recorder.capture(CGEventType::KeyDown,&e,100).unwrap();
        assert!(!down.is_typing);
        let shortcut=down.shortcut.unwrap();
        assert_eq!(shortcut.key.as_deref(),Some("é"));
        assert_eq!(shortcut.modifiers,vec![ShortcutModifier::Shift,ShortcutModifier::Command]);
        let encoded=serde_json::to_value(&shortcut).unwrap();
        assert_eq!(encoded["modifiers"],serde_json::json!(["shift","command"]));
        let restored:ShortcutSample=serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.key,shortcut.key);
        e.set_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT,1);
        assert!(recorder.capture(CGEventType::KeyDown,&e,120).unwrap().shortcut.unwrap().repeat);
        e.set_flags(CGEventFlags::empty());e.set_string("different");
        assert!(recorder.capture(CGEventType::KeyDown,&e,140).is_none());
        let up=recorder.capture(CGEventType::KeyUp,&e,150).unwrap();
        assert!(up.is_release);
        assert_eq!(up.shortcut.unwrap().key.as_deref(),Some("é"));
        assert!(recorder.capture(CGEventType::KeyUp,&e,160).is_none());
    }
    #[test]
    fn named_keys_modifier_observations_and_reset_are_distinct() {
        let mut recorder=Recorder::default();recorder.configure(true);
        let e=event(123,"",CGEventFlags::empty());
        assert_eq!(recorder.capture(CGEventType::KeyDown,&e,100).unwrap().shortcut.unwrap().key.as_deref(),Some("ArrowLeft"));
        let observation=recorder.capture(CGEventType::FlagsChanged,&e,110).unwrap();
        assert!(observation.shortcut.unwrap().key.is_none());
        recorder.reset();
        assert!(recorder.capture(CGEventType::KeyUp,&e,120).is_none());
        let control=event(0,"\u{1}",CGEventFlags::CGEventFlagControl);
        assert!(recorder.capture(CGEventType::KeyDown,&control,130).is_none());
    }
    #[test]
    fn caps_lock_transitions_are_named_presses_and_duplicate_flags_are_observations() {
        let mut recorder=Recorder::default();recorder.configure(true);
        let e=event(57,"ignored",CGEventFlags::CGEventFlagAlphaShift);
        let on=recorder.capture(CGEventType::FlagsChanged,&e,100).unwrap();
        assert!(!on.is_release);assert!(!on.is_typing);
        assert_eq!(on.shortcut.as_ref().unwrap().key.as_deref(),Some("CapsLock"));
        assert_eq!(on.shortcut.unwrap().modifiers,vec![ShortcutModifier::CapsLock]);
        let duplicate=recorder.capture(CGEventType::FlagsChanged,&e,101).unwrap();
        assert!(duplicate.is_release);assert!(duplicate.shortcut.unwrap().key.is_none());
        e.set_flags(CGEventFlags::empty());
        let off=recorder.capture(CGEventType::FlagsChanged,&e,200).unwrap();
        assert!(!off.is_release);assert_eq!(off.shortcut.unwrap().key.as_deref(),Some("CapsLock"));
        recorder.reset();
        assert_eq!(recorder.capture(CGEventType::FlagsChanged,&e,300).unwrap().shortcut.unwrap().key.as_deref(),Some("CapsLock"));
    }

    #[test]
    fn shifted_named_keys_are_shortcuts_without_capturing_shifted_text() {
        let mut recorder=Recorder::default();recorder.configure(true);
        for (code,key) in [(48,"Tab"),(36,"Enter"),(49," "),(53,"Escape")] {
            let e=event(code,"ignored",CGEventFlags::empty());
            assert!(recorder.capture(CGEventType::KeyDown,&e,100).is_none());
            e.set_flags(CGEventFlags::CGEventFlagShift);
            let press=recorder.capture(CGEventType::KeyDown,&e,200).unwrap();
            assert!(!press.is_release);assert!(!press.is_typing);
            let sample=press.shortcut.unwrap();
            assert_eq!(sample.key.as_deref(),Some(key));assert_eq!(sample.modifiers,vec![ShortcutModifier::Shift]);
            assert!(recorder.capture(CGEventType::KeyUp,&e,250).unwrap().is_release);
        }
        let letter=event(0,"A",CGEventFlags::CGEventFlagShift);
        assert!(recorder.capture(CGEventType::KeyDown,&letter,300).is_none());
    }

}
