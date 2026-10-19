//! Privacy-preserving classification of Linux input key codes.
//! Only motion categories leave this module; typed characters are never stored.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypingMotion {
    Character,
    Newline,
    Backspace,
    Tab,
}

#[derive(Default)]
pub(crate) struct TypingState {
    // One bit per command modifier, independently for every keyboard.
    modifiers: BTreeMap<usize, u8>,
}

impl TypingState {
    pub(crate) fn remove_device(&mut self, device: usize) {
        self.modifiers.remove(&device);
    }

    pub(crate) fn key(&mut self, device: usize, code: u16, value: i32) -> Option<TypingMotion> {
        if !matches!(value, 0..=2) {
            return None;
        }
        // Stable Linux input-event-codes.h ABI: left/right Ctrl, Alt and Meta.
        // Shift changes text case and must not suppress typing.
        let modifier = match code {
            29 => Some(0),
            97 => Some(1),
            56 => Some(2),
            100 => Some(3),
            125 => Some(4),
            126 => Some(5),
            _ => None,
        };
        if let Some(bit) = modifier {
            let mask = self.modifiers.entry(device).or_default();
            if value == 0 {
                *mask &= !(1 << bit);
            } else {
                *mask |= 1 << bit;
            }
            return None;
        }
        if value == 0 || self.modifiers.values().any(|mask| *mask != 0) {
            return None;
        }
        // Printable key positions, editing keys and numeric keypad keys only.
        // Function keys, navigation, mouse buttons and media controls are excluded.
        match code {
            28 | 96 => Some(TypingMotion::Newline),
            14 | 111 => Some(TypingMotion::Backspace),
            15 => Some(TypingMotion::Tab),
            2..=13
            | 16..=27
            | 30..=41
            | 43..=53
            | 55
            | 57
            | 71..=83
            | 86
            | 89
            | 95
            | 98
            | 117
            | 118
            | 121
            | 124 => Some(TypingMotion::Character),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_repeat_emit_only_motion_categories() {
        let mut state = TypingState::default();
        assert_eq!(state.key(1, 30, 1), Some(TypingMotion::Character));
        assert_eq!(state.key(1, 30, 2), Some(TypingMotion::Character));
        assert_eq!(state.key(1, 30, 0), None);
        assert_eq!(state.key(1, 28, 1), Some(TypingMotion::Newline));
        assert_eq!(state.key(1, 96, 1), Some(TypingMotion::Newline));
        assert_eq!(state.key(1, 14, 1), Some(TypingMotion::Backspace));
        assert_eq!(state.key(1, 15, 1), Some(TypingMotion::Tab));
    }

    #[test]
    fn releasing_one_control_key_does_not_release_the_other() {
        let mut state = TypingState::default();
        state.key(1, 29, 1);
        state.key(1, 97, 1);
        state.key(1, 29, 0);
        assert_eq!(state.key(1, 30, 1), None);
        state.key(1, 97, 0);
        assert_eq!(state.key(1, 30, 1), Some(TypingMotion::Character));
    }

    #[test]
    fn modifiers_apply_across_keyboards_and_disconnect_clears_them() {
        let mut state = TypingState::default();
        state.key(1, 56, 1);
        assert_eq!(state.key(2, 30, 1), None);
        state.remove_device(1);
        assert_eq!(state.key(2, 30, 1), Some(TypingMotion::Character));
    }

    #[test]
    fn shift_allows_typing_but_navigation_and_media_do_not() {
        let mut state = TypingState::default();
        assert_eq!(state.key(1, 42, 1), None);
        assert_eq!(state.key(1, 30, 1), Some(TypingMotion::Character));
        for code in [1, 59, 87, 103, 105, 106, 108, 113, 114, 115, 272] {
            assert_eq!(state.key(1, code, 1), None);
        }
        assert_eq!(state.key(1, 30, -1), None);
        assert_eq!(state.key(1, 30, 3), None);
    }
}
