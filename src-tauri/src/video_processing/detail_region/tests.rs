use super::*;

fn full(size: [u32; 2], scale: f64) -> Option<Plan> {
    plan(size, size.map(f64::from), [0.0, 0.0, 1.0, 1.0], scale, [0.0; 2], None)
}

#[test]
fn magnification_and_memory_limits_select_scale_or_bypass() {
    assert_eq!(full([100, 80], 2.4999).unwrap().scale, 2);
    assert_eq!(full([100, 80], 2.5).unwrap().scale, 4);
    assert_eq!(full([2048, 100], 4.0).unwrap().scale, 4);
    assert_eq!(full([2049, 100], 4.0).unwrap().scale, 2);
    assert_eq!(full([4096, 100], 4.0).unwrap().scale, 2);
    assert!(full([4097, 100], 4.0).is_none());
    assert_eq!(full([1500, 1000], 4.0).unwrap().scale, 4);
    assert_eq!(full([1501, 1000], 4.0).unwrap().scale, 2);
    assert!(full([3001, 2000], 4.0).is_none());
    assert!(full([u32::MAX; 2], 4.0).is_none());
}

#[test]
fn visible_crop_preserves_source_coordinates_and_pixel_boundary_tolerance() {
    let cropped = plan([100, 80], [200.0, 160.0], [0.25, 0.25, 0.5, 0.5],
        2.0, [0.0; 2], Some([120.00001, 100.0, 40.0, 40.0])).unwrap();
    assert_eq!(cropped.region, [10, 10, 20, 20]);
    assert_eq!(cropped.output_window, [0.3, 0.3125, 0.1, 0.125]);
    assert_eq!(cropped.output_size, [40, 40]);
    assert!(plan([100, 80], [100.0, 80.0], [0.0, 0.0, 1.0, 1.0],
        2.0, [0.0; 2], Some([200.0, 0.0, 40.0, 40.0])).is_none());
}

#[test]
#[ignore = "requires external enhancement region fixtures"]
fn matches_external_region_fixtures() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        decoded: [u32; 2], logical: [f64; 2], window: [f64; 4],
        scale: f64, origin: [f64; 2], visible: Option<[f64; 4]>,
        region: Option<[u32; 4]>, output_size: Option<[u32; 2]>,
        output_window: Option<[f64; 4]>, output_scale: Option<u32>,
    }
    let path = std::env::var("TARANTINO_DETAIL_REGION_FIXTURES").unwrap();
    let cases: Vec<Fixture> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(!cases.is_empty());
    for (index, case) in cases.into_iter().enumerate() {
        let result = plan(case.decoded, case.logical, case.window, case.scale, case.origin, case.visible);
        assert_eq!(result.map(|v| v.region), case.region, "case {index}");
        assert_eq!(result.map(|v| v.scale), case.output_scale, "case {index}");
        assert_eq!(result.map(|v| v.output_size), case.output_size, "case {index}");
        if let Some(result) = result {
            for (a, b) in result.output_window.into_iter().zip(case.output_window.unwrap()) {
                assert!((a - b).abs() < 1e-12, "case {index}: {a} != {b}");
            }
        }
    }
}
