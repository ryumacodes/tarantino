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

/// Some Electron windows report shadow/frame bounds at their top level and
/// client coordinates on a document ancestor. Accept only a stream-sized
/// ancestor contained within bounded frame insets; never derive a scale from
/// an arbitrary focused field's aspect ratio.
pub(super) fn coordinate_frame(
    logical: Option<(i32, i32)>,
    pixels: (u32, u32),
    bounds: (i32, i32, i32, i32),
    ancestors: &[(i32, i32, i32, i32)],
) -> Option<((i32, i32), (i32, i32))> {
    if let Some(size) = coordinate_size(logical, pixels, bounds) {
        return Some(((0, 0), size));
    }
    for &(x, y, w, h) in ancestors {
        let inside = frame_insets(bounds, (x, y, w, h))
            || ancestors
                .iter()
                .any(|&outer| frame_insets(bounds, outer) && same_scaled_rect(outer, (x, y, w, h)));
        if !inside {
            continue;
        }
        if let Some(size) = coordinate_size(logical, pixels, (0, 0, w, h)) {
            return Some(((x, y), size));
        }
    }
    None
}

fn frame_insets(top: (i32, i32, i32, i32), client: (i32, i32, i32, i32)) -> bool {
    let (tx, ty, tw, th) = top;
    let (x, y, w, h) = client;
    let left = i64::from(x) - i64::from(tx);
    let above = i64::from(y) - i64::from(ty);
    let right = i64::from(tw) - left - i64::from(w);
    let below = i64::from(th) - above - i64::from(h);
    w > 0
        && h > 0
        && (0..=64).contains(&left)
        && (0..=256).contains(&above)
        && (0..=64).contains(&right)
        && (0..=64).contains(&below)
}

// Electron's native shell can use logical units while its web tree uses
// pixels. Require an explicit ancestor pair with matching scaled origin AND
// extent, rather than guessing a scale from any field's aspect ratio.
fn same_scaled_rect(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    if a.2 <= 0 || a.3 <= 0 || b.2 <= 0 || b.3 <= 0 {
        return false;
    }
    let scale = b.2 as f64 / a.2 as f64;
    (0.5..=4.0).contains(&scale)
        && (a.0 as f64 * scale - b.0 as f64).abs() <= 1.0
        && (a.1 as f64 * scale - b.1 as f64).abs() <= 1.0
        && (a.3 as f64 * scale - b.3 as f64).abs() <= 1.0
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

#[cfg(test)]
mod frame_tests {
    use super::*;
    #[test]
    fn electron_client_excludes_shadow_and_handles_fractional_scale() {
        assert_eq!(
            coordinate_frame(
                Some((1600, 869)),
                (2320, 1260),
                (0, 0, 1632, 911),
                &[(16, 10, 1600, 869)]
            ),
            Some(((16, 10), (1600, 869)))
        );
    }
    #[test]
    fn unrelated_field_or_unknown_scale_cannot_bind_a_window() {
        assert_eq!(
            coordinate_frame(
                None,
                (2320, 1260),
                (0, 0, 1632, 911),
                &[(16, 10, 1600, 869)]
            ),
            None
        );
        assert_eq!(
            coordinate_frame(
                Some((400, 200)),
                (800, 400),
                (0, 0, 1600, 900),
                &[(100, 100, 400, 200)]
            ),
            None
        );
    }
}

#[cfg(test)]
mod mixed_units_tests {
    use super::*;
    #[test]
    fn electron_web_pixels_and_native_logical_shell() {
        assert_eq!(
            coordinate_frame(
                None,
                (2320, 1260),
                (0, 0, 1632, 911),
                &[
                    (173, 100, 509, 54),
                    (23, 14, 2320, 1261),
                    (16, 10, 1600, 869)
                ]
            ),
            Some(((23, 14), (2320, 1261)))
        );
        assert_eq!(
            coordinate_frame(
                None,
                (2320, 1260),
                (0, 0, 1632, 911),
                &[(200, 200, 2320, 1261), (16, 10, 1600, 869)]
            ),
            None
        );
    }
}

/// Correct a Text/Component origin disagreement only when the independent
/// PARENT-coordinate query confirms exactly the selected client-frame offset.
pub(super) fn text_point(
    point: (f64, f64),
    component_point: Option<(f64, f64)>,
    origin: (i32, i32),
) -> (f64, f64) {
    component_point
        .filter(|other| {
            (other.0 - point.0 - origin.0 as f64).abs() <= 1.0
                && (other.1 - point.1 - origin.1 as f64).abs() <= 1.0
        })
        .unwrap_or(point)
}

#[cfg(test)]
mod text_origin_tests {
    use super::*;
    #[test]
    fn independent_parent_bounds_confirm_electron_origin_difference() {
        assert_eq!(
            text_point((283.5, 113.5), Some((306.5, 127.5)), (23, 14)),
            (306.5, 127.5)
        );
        assert_eq!(
            text_point((283.5, 113.5), Some((283.5, 113.5)), (23, 14)),
            (283.5, 113.5)
        );
        assert_eq!(text_point((283.5, 113.5), None, (23, 14)), (283.5, 113.5));
    }
}
