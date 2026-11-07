use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(super) enum Notification { Selection, Focus, Refresh }

impl Notification {
    pub(super) fn from_name(name: &str) -> Self {
        match name {
            "AXSelectedCellsChanged" | "AXSelectedRowsChanged" | "AXSelectedColumnsChanged" | "AXSelectedChildrenChanged" => Self::Selection,
            "AXFocusedUIElementChanged" | "AXFocusedWindowChanged" => Self::Focus,
            _ => Self::Refresh,
        }
    }
}

pub(super) struct FocusTarget<T> {
    pub(super) selected: Option<T>,
    last_text_focus: Option<Instant>,
}

impl<T> Default for FocusTarget<T> {
    fn default() -> Self { Self { selected: None, last_text_focus: None } }
}

impl<T> FocusTarget<T> {
    pub(super) fn notify(
        &mut self,
        notification: Notification,
        source: T,
        now: Instant,
        text_role: bool,
        selected_child: bool,
    ) {
        match notification {
            Notification::Selection => {
                if selected_child && self.last_text_focus.is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_millis(500)) {
                    self.selected = Some(source);
                }
            }
            Notification::Focus => {
                self.selected = None;
                if text_role { self.last_text_focus = Some(now); }
            }
            Notification::Refresh => {}
        }
    }
}

pub(super) fn is_text_role(role: &str) -> bool {
    matches!(role, "AXTextField" | "AXTextArea" | "AXSearchField" | "AXComboBox")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notify(target: &mut FocusTarget<u8>, name: &str, source: u8, now: Instant) {
        target.notify(Notification::from_name(name), source, now, source == 1, source >= 2);
    }

    #[test]
    fn text_focus_wins_until_the_half_second_boundary() {
        let mut target = FocusTarget::default();
        let now = Instant::now();
        notify(&mut target, "AXFocusedUIElementChanged", 1, now);
        notify(&mut target, "AXSelectedRowsChanged", 2, now + Duration::from_millis(499));
        assert_eq!(target.selected, None);
        notify(&mut target, "AXSelectedRowsChanged", 2, now + Duration::from_millis(500));
        assert_eq!(target.selected, Some(2));
    }

    #[test]
    fn selection_persists_through_refreshes_and_empty_selection_notifications() {
        let mut target = FocusTarget::default();
        let now = Instant::now();
        notify(&mut target, "AXSelectedCellsChanged", 2, now);
        for name in ["AXValueChanged", "AXWindowMoved", "AXSelectedTextChanged", "AXSelectedChildrenChanged"] {
            notify(&mut target, name, 0, now);
            assert_eq!(target.selected, Some(2));
        }
        notify(&mut target, "AXSelectedColumnsChanged", 3, now);
        assert_eq!(target.selected, Some(3));
        notify(&mut target, "AXFocusedWindowChanged", 0, now);
        assert_eq!(target.selected, None);
    }

    #[test]
    fn non_text_focus_clears_selection_without_extending_text_priority() {
        let mut target = FocusTarget::default();
        let now = Instant::now();
        notify(&mut target, "AXFocusedUIElementChanged", 1, now);
        notify(&mut target, "AXFocusedUIElementChanged", 0, now + Duration::from_millis(499));
        notify(&mut target, "AXSelectedChildrenChanged", 2, now + Duration::from_millis(500));
        assert_eq!(target.selected, Some(2));
        for role in ["AXTextField", "AXTextArea", "AXSearchField", "AXComboBox"] { assert!(is_text_role(role)); }
        assert!(!is_text_role("AXStaticText"));
    }
}
