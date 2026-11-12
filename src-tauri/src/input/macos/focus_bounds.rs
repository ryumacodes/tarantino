use core_graphics::geometry::CGRect;

pub(super) fn enclosing_scroll_area<T>(
    mut element: T,
    mut parent: impl FnMut(&T) -> Option<T>,
    mut is_scroll_area: impl FnMut(&T) -> bool,
) -> Option<T> {
    for _ in 0..20 {
        element = parent(&element)?;
        if is_scroll_area(&element) { return Some(element); }
    }
    None
}

pub(super) fn finite(bounds: CGRect) -> bool {
    [bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height].iter().all(|value| value.is_finite())
}

pub(super) fn select(element: CGRect, viewport: Option<CGRect>) -> [f64; 4] {
    let bounds = match viewport {
        Some(viewport) if !unsafe { CGRectEqualToRect(CGRectUnion(element, viewport), viewport) } => viewport,
        _ => element,
    };
    [bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height]
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGRectUnion(first: CGRect, second: CGRect) -> CGRect;
    fn CGRectEqualToRect(first: CGRect, second: CGRect) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::geometry::{CGPoint, CGSize};

    #[test]
    fn nearest_parent_wins_and_the_element_itself_is_not_checked() {
        let found = enclosing_scroll_area(0, |value| Some(value + 1), |value| matches!(value, 0 | 2 | 4));
        assert_eq!(found, Some(2));
        assert_eq!(enclosing_scroll_area(0, |_| None, |_| true), None);
    }

    #[test]
    fn twentieth_parent_is_included_but_traversal_never_reaches_twenty_first() {
        assert_eq!(enclosing_scroll_area(0, |value| Some(value + 1), |value| *value == 20), Some(20));
        let mut reads = 0;
        assert_eq!(enclosing_scroll_area(0, |value| { reads += 1; Some(value + 1) }, |value| *value == 21), None);
        assert_eq!(reads, 20);
        reads = 0;
        assert_eq!(enclosing_scroll_area(0, |_| { reads += 1; Some(0) }, |_| false), None);
        assert_eq!(reads, 20);
    }

    #[test]
    fn element_metadata_keeps_finite_degenerate_rectangles() {
        for (width, height) in [(0.0, 0.0), (200.0, 0.0), (0.0, 100.0), (-20.0, 30.0)] {
            let bounds = CGRect::new(&CGPoint::new(-200.0, 100.0), &CGSize::new(width, height));
            assert!(finite(bounds));
            assert_eq!(select(bounds, None), [-200.0, 100.0, width, height]);
        }
        assert!(!finite(CGRect::new(&CGPoint::new(f64::NAN, 0.0), &CGSize::new(100.0, 100.0))));
        assert!(!finite(CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(100.0, f64::INFINITY))));
    }

    #[test]
    fn overflowing_elements_use_the_viewport_without_intersection() {
        let rect = |x, y, w, h| CGRect::new(&CGPoint::new(x, y), &CGSize::new(w, h));
        let viewport = rect(-200.0, 100.0, 400.0, 300.0);
        for element in [rect(-150.0, 120.0, 100.0, 30.0), viewport] {
            assert_eq!(select(element, Some(viewport)), select(element, None));
        }
        for element in [rect(-250.0, 120.0, 100.0, 30.0), rect(-150.0, 120.0, 100.0, 400.0), rect(500.0, 500.0, 50.0, 30.0)] {
            assert_eq!(select(element, Some(viewport)), [-200.0, 100.0, 400.0, 300.0]);
        }
    }
}
