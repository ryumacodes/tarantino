use rdev::Key;

/// Track physical sides independently. Shift remains ordinary typing.
#[derive(Debug, Default)]
pub(super) struct CommandModifiers(u8);

impl CommandModifiers {
    pub(super) fn update(&mut self, key: Key, pressed: bool) {
        let bit = match key {
            Key::MetaLeft => 1,
            Key::MetaRight => 2,
            Key::ControlLeft => 4,
            Key::ControlRight => 8,
            Key::Alt => 16,
            Key::AltGr => 32,
            _ => return,
        };
        if pressed { self.0 |= bit; } else { self.0 &= !bit; }
    }

    pub(super) fn held(&self) -> bool { self.0 != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releasing_one_side_keeps_the_other_side_held() {
        for (left, right) in [(Key::MetaLeft, Key::MetaRight),
            (Key::ControlLeft, Key::ControlRight), (Key::Alt, Key::AltGr)] {
            for (first, second) in [(left, right), (right, left)] {
                let mut state = CommandModifiers::default();
                state.update(first, true);
                state.update(second, true);
                state.update(first, false);
                assert!(state.held());
                state.update(second, false);
                assert!(!state.held());
            }
        }
    }

    #[test]
    fn repeated_presses_are_idempotent_and_modifiers_are_independent() {
        let mut state = CommandModifiers::default();
        state.update(Key::MetaLeft, true);
        state.update(Key::MetaLeft, true);
        state.update(Key::ControlRight, true);
        state.update(Key::MetaLeft, false);
        state.update(Key::Alt, false);
        assert!(state.held());
        state.update(Key::ControlRight, false);
        assert!(!state.held());
        state.update(Key::ShiftLeft, true);
        state.update(Key::KeyA, true);
        assert!(!state.held());
    }
}
