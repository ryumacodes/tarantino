pub(super) fn resolve<T>(
    focused: T,
    active: impl FnOnce(&T) -> Option<T>,
    editable_ancestor: impl FnOnce(&T) -> Option<T>,
) -> T {
    let active = active(&focused).unwrap_or(focused);
    editable_ancestor(&active).unwrap_or(active)
}

pub(super) fn selected_child<T>(role: &str, mut last_selected: impl FnMut(&str) -> Option<T>) -> Option<T> {
    if !matches!(role, "AXTable" | "AXOutline" | "AXList") {
        return None;
    }
    ["AXSelectedRows", "AXSelectedCells", "AXSelectedColumns", "AXSelectedChildren"]
        .into_iter().find_map(|attribute| last_selected(attribute))
}

pub(super) fn caret<T>(
    role: &str,
    range: impl FnOnce() -> Option<T>,
    marker: impl FnOnce() -> Option<T>,
    geometry: impl FnOnce() -> Option<T>,
) -> Option<T> {
    if !super::focus_target::is_text_role(role) { return None; }
    range().or_else(marker).or_else(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_text_uses_only_the_first_available_caret_provider() {
        use std::cell::RefCell;
        for role in ["AXTextField", "AXTextArea", "AXSearchField", "AXComboBox"] {
            for available in 0..3 {
                let calls = RefCell::new(Vec::new());
                let provider = |index| { calls.borrow_mut().push(index); (index == available).then_some(index) };
                assert_eq!(caret(role, || provider(0), || provider(1), || provider(2)), Some(available));
                assert_eq!(*calls.borrow(), (0..=available).collect::<Vec<_>>());
            }
        }
        assert_eq!(caret::<()>("AXTextArea", || None, || None, || None), None);
        for role in ["AXGroup", "AXWindow", "AXStaticText", "AXWebArea", ""] {
            assert_eq!(caret::<()>(role, || panic!("range on non-text"), || panic!("marker on non-text"), || panic!("geometry on non-text")), None);
        }
    }

    #[test]
    fn editable_ancestor_is_resolved_from_the_active_element() {
        let result = resolve(1, |element| {
            assert_eq!(*element, 1);
            Some(2)
        }, |element| {
            assert_eq!(*element, 2);
            Some(3)
        });
        assert_eq!(result, 3);
    }

    #[test]
    fn missing_relationships_retain_the_last_resolved_element() {
        assert_eq!(resolve(1, |_| None, |element| {
            assert_eq!(*element, 1);
            Some(3)
        }), 3);
        assert_eq!(resolve(1, |_| Some(2), |_| None), 2);
        assert_eq!(resolve(1, |_| None, |_| None), 1);
    }

    #[test]
    fn selected_container_attributes_are_tried_in_priority_order() {
        let attributes = ["AXSelectedRows", "AXSelectedCells", "AXSelectedColumns", "AXSelectedChildren"];
        for role in ["AXTable", "AXOutline", "AXList"] {
            for available in 0..attributes.len() {
                let mut calls = Vec::new();
                let child = selected_child(role, |attribute| {
                    calls.push(attribute.to_owned());
                    (attribute == attributes[available]).then_some(7)
                });
                assert_eq!(child, Some(7));
                assert_eq!(calls, attributes[..=available]);
            }
        }
    }

    #[test]
    fn ordinary_elements_do_not_query_selection_containers() {
        for role in ["AXTextField", "AXGroup", "AXWindow", ""] {
            assert_eq!(selected_child::<()>(role, |_| panic!("not a selection container")), None);
        }
        assert_eq!(selected_child::<()>("AXTable", |_| None), None);
    }
}
