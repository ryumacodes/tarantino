pub(super) fn resolve<T>(
    focused: T,
    active: impl FnOnce(&T) -> Option<T>,
    editable_ancestor: impl FnOnce(&T) -> Option<T>,
) -> T {
    let active = active(&focused).unwrap_or(focused);
    editable_ancestor(&active).unwrap_or(active)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
