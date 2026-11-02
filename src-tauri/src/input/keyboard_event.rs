use serde::{Deserialize, Serialize};

/// Keyboard activity, with optional explicitly enabled shortcut labels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    /// Absent for ordinary typing and recordings without shortcut capture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<ShortcutSample>,
    /// Timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Older sidecars contain presses only.
    #[serde(default)]
    pub is_release: bool,
    /// Whether this is a modifier key (Cmd, Ctrl, Alt — NOT Shift)
    pub is_modifier: bool,
    /// Whether this counts as a typing keystroke (unmodified character key)
    pub is_typing: bool,
    /// Privacy-safe typing movement category. Does not store the typed character.
    #[serde(default)]
    pub key_motion: KeyMotion,
    /// Last known cursor X coordinate when the key was pressed.
    #[serde(default)]
    pub cursor_x: Option<f64>,
    /// Last known cursor Y coordinate when the key was pressed.
    #[serde(default)]
    pub cursor_y: Option<f64>,
    /// Focused text caret X coordinate when available.
    #[serde(default)]
    pub caret_x: Option<f64>,
    /// Focused text caret Y coordinate when available.
    #[serde(default)]
    pub caret_y: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutSample {
    /// None denotes a modifier observation, not a displayable keystroke.
    pub key: Option<String>,
    pub modifiers: Vec<ShortcutModifier>,
    pub repeat: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutModifier { Control, Option, Shift, Command, CapsLock, Function }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KeyMotion {
    #[default]
    Character,
    Newline,
    Backspace,
    Tab,
}

impl KeyEvent {
    pub(super) fn release(timestamp: u64, is_modifier: bool) -> Self {
        Self { shortcut: None, timestamp, is_release: true, is_modifier, is_typing: false,
            key_motion: KeyMotion::default(), cursor_x: None, cursor_y: None,
            caret_x: None, caret_y: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_presses_and_new_releases_round_trip_without_key_contents() {
        let press: KeyEvent = serde_json::from_str(
            r#"{"timestamp":100,"is_modifier":false,"is_typing":true}"#).unwrap();
        assert!(!press.is_release);
        let release = KeyEvent::release(150, false);
        let encoded = serde_json::to_value(&release).unwrap();
        let restored: KeyEvent = serde_json::from_value(encoded.clone()).unwrap();
        assert!(restored.is_release);
        assert!(!restored.is_typing);
        assert_eq!(restored.timestamp, 150);
        assert!(encoded.get("character").is_none());
        assert!(encoded.get("key").is_none());
    }
}
