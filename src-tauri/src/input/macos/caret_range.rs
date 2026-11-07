use super::CFRange;
use core_graphics::geometry::CGRect;

pub(super) fn position(
    selection: CFRange,
    mut bounds: impl FnMut(CFRange) -> Option<CGRect>,
    element_bounds: impl FnOnce() -> Option<CGRect>,
) -> Option<(f64, f64)> {
    if selection.location < 0 || selection.length < 0 {
        return None;
    }
    let end = selection.location.checked_add(selection.length)?;
    let next = bounds(CFRange { location: end, length: 1 });
    let previous = (end > 0)
        .then(|| bounds(CFRange { location: end - 1, length: 1 }))
        .flatten();
    match (next, previous) {
        (Some(next), Some(previous)) if (next.origin.y - previous.origin.y).abs() < 1.0 => {
            Some((previous.origin.x + previous.size.width,
                previous.origin.y + previous.size.height / 2.0))
        }
        (Some(next), _) => Some((next.origin.x, next.origin.y + next.size.height / 2.0)),
        (None, Some(previous)) => Some((previous.origin.x + previous.size.width,
            previous.origin.y + previous.size.height / 2.0)),
        (None, None) => bounds(CFRange { location: end, length: 0 })
            .map(|rect| {
                let offset = if element_bounds().is_some_and(|element| rect.origin.y < element.origin.y) {
                    rect.size.height
                } else { 0.0 };
                (rect.origin.x, rect.origin.y + offset + rect.size.height / 2.0)
            }),
    }
}

pub(super) fn text_marker_position(bounds: CGRect) -> (f64, f64) {
    let width = if bounds.size.width > 6.0 { 2.0 } else { bounds.size.width.max(2.0) };
    (bounds.origin.x + width / 2.0, bounds.origin.y + bounds.size.height / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::geometry::{CGPoint, CGSize};

    fn position(selection: CFRange, bounds: impl FnMut(CFRange) -> Option<CGRect>) -> Option<(f64, f64)> {
        super::position(selection, bounds, || None)
    }

    fn rect(x: f64, y: f64, width: f64) -> CGRect {
        CGRect::new(&CGPoint::new(x, y), &CGSize::new(width, 16.0))
    }

    #[test]
    fn selection_uses_its_end_and_prefers_the_previous_character_on_the_same_line() {
        let mut requests = Vec::new();
        let result = position(CFRange { location: 3, length: 7 }, |range| {
            requests.push((range.location, range.length));
            match (range.location, range.length) {
                (10, 1) => Some(rect(110.0, 40.5, 12.0)),
                (9, 1) => Some(rect(98.0, 40.0, 10.0)),
                _ => panic!("character bounds should resolve the selection end"),
            }
        });
        assert_eq!(result, Some((108.0, 48.0)));
        assert_eq!(requests, [(10, 1), (9, 1)]);
    }

    #[test]
    fn line_wrap_uses_the_next_character_instead_of_the_previous_line_end() {
        for y in [41.0, 56.0] {
            let result = position(CFRange { location: 10, length: 0 }, |range| {
                Some(if range.location == 10 { rect(20.0, y, 10.0) }
                    else { rect(198.0, 40.0, 10.0) })
            });
            assert_eq!(result, Some((20.0, y + 8.0)));
        }
    }

    #[test]
    fn final_character_and_start_of_text_need_only_one_neighbor() {
        assert_eq!(position(CFRange { location: 10, length: 0 }, |range|
            (range.location == 9).then(|| rect(98.0, 40.0, 10.0))), Some((108.0, 48.0)));
        assert_eq!(position(CFRange { location: 0, length: 0 }, |range| {
            assert_eq!((range.location, range.length), (0, 1));
            Some(rect(20.0, 40.0, 10.0))
        }), Some((20.0, 48.0)));
    }

    #[test]
    fn collapsed_only_provider_is_queried_after_both_neighbors() {
        let mut requests = Vec::new();
        let result = position(CFRange { location: 5, length: 2 }, |range| {
            requests.push((range.location, range.length));
            (range.length == 0).then(|| rect(120.0, 40.0, 0.0))
        });
        assert_eq!(result, Some((120.0, 48.0)));
        assert_eq!(requests, [(7, 1), (6, 1), (7, 0)]);
    }

    #[test]
    fn collapsed_caret_above_its_element_moves_down_one_line_height() {
        for (element_y, expected_y) in [(39.0, 48.0), (40.0, 48.0), (40.1, 64.0), (100.0, 64.0)] {
            let result = super::position(CFRange { location: 5, length: 0 }, |range|
                (range.length == 0).then(|| rect(120.0, 40.0, 0.0)),
                || Some(rect(100.0, element_y, 400.0)));
            assert_eq!(result, Some((120.0, expected_y)));
        }
    }

    #[test]
    fn character_bounds_do_not_request_element_geometry() {
        let result = super::position(CFRange { location: 0, length: 0 },
            |_| Some(rect(20.0, 40.0, 10.0)), || panic!("unneeded element query"));
        assert_eq!(result, Some((20.0, 48.0)));
    }

    #[test]
    fn text_marker_selections_use_a_caret_width_without_shifting_to_the_selection_center() {
        for (width, x) in [(0.0, 101.0), (1.0, 101.0), (2.0, 101.0),
            (4.0, 102.0), (6.0, 103.0), (6.01, 101.0), (400.0, 101.0)] {
            assert_eq!(text_marker_position(rect(100.0, 40.0, width)), (x, 48.0));
        }
    }

    #[test]
    fn unavailable_and_invalid_ranges_do_not_invent_a_position() {
        assert_eq!(position(CFRange { location: 0, length: 0 }, |_| None), None);
        for (location, length) in [(-1, 0), (0, -1), (isize::MAX, 1)] {
            assert_eq!(position(CFRange { location, length }, |_| panic!("invalid range")), None);
        }
    }
}
