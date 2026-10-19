//! Match accessibility coordinates to a portal stream without confusing GTK
//! XWayland pixels with Qt/compositor logical units.

pub(super) fn client_size(bounds: (i32, i32, i32, i32)) -> Option<(i32, i32)> {
    let (x, y, width, height) = bounds;
    // GTK's WINDOW coordinates have a client origin, but its top-level bounds
    // may include the frame: e.g. (-1,-41,602,362) for a 600x320 client. Strip
    // the symmetric side/bottom border and top decoration, with bounded insets.
    if !(-64..=0).contains(&x) || !(-256..=0).contains(&y) {
        return None;
    }
    let width = width.checked_add(x.checked_mul(2)?)?;
    let height = height.checked_add(y)?.checked_add(x)?;
    (width > 0 && height > 0).then_some((width, height))
}

pub(super) fn coordinate_size(
    logical: Option<(i32, i32)>,
    pixels: (u32, u32),
    bounds: (i32, i32, i32, i32),
) -> Option<(i32, i32)> {
    let actual = client_size(bounds)?;
    let pixels = (i32::try_from(pixels.0).ok()?, i32::try_from(pixels.1).ok()?);
    [logical.unwrap_or_default(), pixels]
        .into_iter()
        .any(|expected| {
            expected.0 > 0
                && expected.1 > 0
                && (expected.0 as i64 - actual.0 as i64).abs() <= 1
                && (expected.1 as i64 - actual.1 as i64).abs() <= 1
        })
        .then_some(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gtk_decorated_xwayland_window_uses_client_pixels() {
        assert_eq!(
            coordinate_size(Some((414, 221)), (600, 320), (-1, -41, 602, 362)),
            Some((600, 320))
        );
    }
    #[test]
    fn omitted_portal_size_requires_matching_stream_pixels() {
        assert_eq!(
            coordinate_size(None, (600, 320), (-1, -41, 602, 362)),
            Some((600, 320))
        );
        assert_eq!(coordinate_size(None, (600, 320), (0, 0, 414, 221)), None);
    }
    #[test]
    fn qt_logical_coordinates_keep_fractional_scaling() {
        assert_eq!(
            coordinate_size(Some((205, 125)), (297, 181), (0, 0, 205, 125)),
            Some((205, 125))
        );
    }
    #[test]
    fn client_side_decorations_remain_inside_the_capture() {
        assert_eq!(
            coordinate_size(Some((400, 200)), (800, 400), (0, 0, 800, 400)),
            Some((800, 400))
        );
    }
    #[test]
    fn mismatched_windows_and_invalid_extents_are_rejected() {
        assert_eq!(
            coordinate_size(Some((414, 221)), (600, 320), (-1, -41, 802, 462)),
            None
        );
        assert_eq!(
            coordinate_size(Some((600, 320)), (600, 320), (1, 0, 600, 320)),
            None
        );
        assert_eq!(client_size((-1, -41, 1, 1)), None);
        assert_eq!(client_size((i32::MIN, 0, 600, 320)), None);
        assert_eq!(
            coordinate_size(Some((0, 0)), (0, 0), (0, 0, 600, 320)),
            None
        );
    }
}
