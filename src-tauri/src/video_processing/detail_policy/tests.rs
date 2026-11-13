use super::*;

fn policy() -> DetailPolicy {
    DetailPolicy { quality: Quality::Export, motion_pixels: 0.0, blur_strength: 1.0,
        projected_scale: 2.0, input_resolution: 1.0, magnifier_presence: 0.0, magnifier: false }
}

#[test]
fn minimum_motion_retains_stationary_points_during_zoom() {
    let before = [0.0, 0.0, 100.0, 80.0];
    assert_eq!(slowest_motion([3.0, 4.0, 100.0, 80.0], before), 5.0);
    assert_eq!(slowest_motion([-50.0, -40.0, 200.0, 160.0], before), 0.0);
    assert_eq!(slowest_motion([0.0, 0.0, 200.0, 160.0], before), 0.0);
    assert_eq!(slowest_motion([3.0, -40.0, 200.0, 160.0], before), 3.0);
    assert_eq!(slowest_motion([-103.0, -84.0, 200.0, 160.0], before), 5.0);
}

#[test]
#[ignore = "requires external rectangle motion fixtures"]
fn matches_external_motion_fixtures() {
    #[derive(serde::Deserialize)]
    struct Case { now: [f64; 4], before: [f64; 4], expected: f64 }
    let path = std::env::var("TARANTINO_DETAIL_MOTION_FIXTURES").unwrap();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(!cases.is_empty());
    for (i, case) in cases.into_iter().enumerate() {
        assert!((slowest_motion(case.now, case.before) - case.expected).abs() < 1e-9, "case {i}");
    }
}

#[test]
fn quality_and_strength_gate_region_allocation() {
    let mut state = policy();
    let planned = |state: &DetailPolicy| state.plan([100, 80], [100.0, 80.0],
        [0.0, 0.0, 1.0, 1.0], 2.0, [0.0; 2], None);
    for quality in [Quality::Export, Quality::PausedHigh] {
        state.quality = quality;
        assert_eq!(planned(&state).unwrap().0, 1.0);
    }
    for quality in [Quality::Playback, Quality::PausedLow] {
        state.quality = quality;
        assert!(planned(&state).is_none());
    }
    state.quality = Quality::Export;
    state.magnifier_presence = 0.8125;
    assert_eq!(state.strength(), 0.1875);
    assert!(planned(&state).is_none());
    state.magnifier_presence = 0.75;
    assert_eq!(planned(&state).unwrap().0, 0.25);
    state.motion_pixels = 8.0;
    assert!(planned(&state).is_none());
}

#[test]
#[ignore = "requires external enhancement strength fixtures"]
fn matches_external_strength_fixtures() {
    #[derive(serde::Deserialize)]
    struct Case { motion: f64, blur: f64, scale: f64, resolution: f64,
        presence: f64, magnifier: bool, strength: f64 }
    let path = std::env::var("TARANTINO_DETAIL_STRENGTH_FIXTURES").unwrap();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(!cases.is_empty());
    for (i, case) in cases.into_iter().enumerate() {
        let state = DetailPolicy { quality: Quality::Export, motion_pixels: case.motion,
            blur_strength: case.blur, projected_scale: case.scale, input_resolution: case.resolution,
            magnifier_presence: case.presence, magnifier: case.magnifier };
        assert_eq!(state.strength(), case.strength, "case {i}");
    }
}
