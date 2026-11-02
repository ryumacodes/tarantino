use super::*;

#[test]
fn automatic_zoom_enters_at_in_range_activity_not_the_previous_pointer() {
    let block: ZoomBlock=serde_json::from_value(serde_json::json!({
        "start_time_ms":100,"end_time_ms":500,"zoom_level":2,
        "center_x":0.5,"center_y":0.5,"instant_animation":true
    })).unwrap();
    let events=[(0,0.1),(200,0.6),(400,0.7)].map(|(timestamp_ms,x)|CursorEvent { is_release: false,
        timestamp_ms,x,y:x,is_click:false });
    let config=SpringConfig { tension:250.0,friction:40.0,mass:2.25 };
    let expected=(0.6*200.0+0.7*100.0)/300.0;
    for fps in [24.0,30.0,60.0,120.0] {
        let frames=simulate_zoom_trajectory(&[block.clone()],&events,&config,true,"focused",fps,600,true);
        for (index,frame) in frames.iter().enumerate() {
            let time=index as f64*1000.0/fps;
            if time>=125.0 && time<200.0 {
                assert!((frame.center_x-expected).abs()<1e-12,"{fps} fps at {time}: {}",frame.center_x);
                assert!((frame.center_y-expected).abs()<1e-12);
            }
        }
    }
}

#[test]
fn magnifier_blocks_screen_anticipation_until_its_end() {
    let lens: ZoomBlock = serde_json::from_value(serde_json::json!({
        "start_time_ms":0,"end_time_ms":1000,"zoom_level":3,"center_x":0.2,"center_y":0.3,
        "loupe":{"radius":0.2,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}
    })).unwrap();
    let screen: ZoomBlock = serde_json::from_value(serde_json::json!({
        "start_time_ms":1200,"end_time_ms":2200,"zoom_level":2,"center_x":0.5,"center_y":0.5,
        "animation_lead_in_ms":600,"is_manual":true
    })).unwrap();
    let config = SpringConfig { tension:250.0,friction:40.0,mass:2.25 };
    for fps in [24.0,30.0,60.0,120.0] {
        for reversed in [false,true] {
            let blocks = if reversed { vec![screen.clone(),lens.clone()] } else { vec![lens.clone(),screen.clone()] };
            let frames = simulate_zoom_trajectory(&blocks,&[],&config,true,"focused",fps,2400,true);
            for (index,frame) in frames.iter().enumerate() {
                let time = index as f64*1000.0/fps;
                if time <= 1000.0 { assert_eq!(frame.scale,1.0,"premature screen zoom at {time}ms"); }
                if time >= 1100.0 && time < 1200.0 { assert!(frame.scale>1.0,"lead-in lost after magnifier ends"); }
            }
            let mut disabled = lens.clone();disabled.is_disabled=true;
            let frames = simulate_zoom_trajectory(&[disabled,screen.clone()],&[],&config,true,"focused",fps,2400,true);
            assert!(frames[(fps*0.9) as usize].scale>1.0,"disabled magnifier blocks anticipation");
        }
    }
}

#[test]
fn manual_zoom_ignores_pointer_and_typing_centers() {
    let config = SpringConfig {
        tension: 170.0,
        friction: 30.0,
        mass: 1.0,
    };
    for kind in ["click", "typing"] {
        let mut block: ZoomBlock = serde_json::from_value(serde_json::json!({
            "start_time_ms": 0, "end_time_ms": 2000, "zoom_level": 2.0,
            "center_x": 0.3, "center_y": 0.4, "is_manual": true,
            "kind": kind, "centers": [{"x": 0.8, "y": 0.8, "time": 0}]
        }))
        .unwrap();
        let events = vec![CursorEvent { is_release: false,
            timestamp_ms: 0,
            x: 0.8,
            y: 0.8,
            is_click: false,
        }];
        for fps in [24.0, 30.0, 60.0, 120.0] {
            let frames = simulate_zoom_trajectory(
                &[block.clone()],
                &events,
                &config,
                false,
                "mellow",
                fps,
                2000,
                true,
            );
            let last = frames.last().unwrap();
            assert!((last.center_x - 0.3).abs() < 0.001);
            assert!((last.center_y - 0.4).abs() < 0.001);
        }
        block.is_manual = false;
        let frames = simulate_zoom_trajectory(
            &[block],
            &events,
            &config,
            false,
            "mellow",
            30.0,
            2000,
            true,
        );
        assert!(frames.last().unwrap().center_x > 0.7);
    }
}

#[test]
fn instant_zoom_snaps_on_entry_and_exit_at_each_output_rate() {
    let block: ZoomBlock = serde_json::from_value(serde_json::json!({
        "start_time_ms": 500, "end_time_ms": 1500, "zoom_level": 2.0,
        "center_x": 0.3, "center_y": 0.4, "is_manual": true,
        "instant_animation": true, "timing_adjusted": true
    }))
    .unwrap();
    let config = SpringConfig {
        tension: 170.0,
        friction: 30.0,
        mass: 1.0,
    };
    for fps in [24.0, 30.0, 60.0, 120.0] {
        let frames = simulate_zoom_trajectory(
            &[block.clone()],
            &[],
            &config,
            false,
            "mellow",
            fps,
            2000,
            true,
        );
        for (index, frame) in frames.iter().enumerate() {
            let time = index as f64 * 1000.0 / fps;
            let active = (500.0..=1500.0).contains(&time);
            assert_eq!(
                frame.scale,
                if active { 2.0 } else { 1.0 },
                "fps={fps} t={time}"
            );
            assert!((frame.center_x - if active { 0.3 } else { 0.5 }).abs() < 1e-9);
        }
    }
    let mut animated = block;
    animated.instant_animation = false;
    let frames =
        simulate_zoom_trajectory(&[animated], &[], &config, false, "mellow", 60.0, 2000, true);
    assert!(frames[30].scale > 1.0 && frames[30].scale < 2.0);
}

#[test]
fn unified_screen_style_changes_camera_pan() {
    let block: ZoomBlock = serde_json::from_value(serde_json::json!({
        "start_time_ms": 0, "end_time_ms": 2000, "zoom_level": 2.0,
        "center_x": 0.3, "center_y": 0.4
    }))
    .unwrap();
    let zoom = SpringConfig {
        tension: 170.0,
        friction: 50.0,
        mass: 3.0,
    };
    let events = [CursorEvent { is_release: false,
        timestamp_ms: 0,
        x: 0.8,
        y: 0.8,
        is_click: false,
    }];
    let legacy = simulate_zoom_trajectory(
        &[block.clone()],
        &events,
        &zoom,
        false,
        "mellow",
        60.0,
        2000,
        true,
    );
    let styled =
        simulate_zoom_trajectory(&[block], &events, &zoom, true, "mellow", 60.0, 2000, true);
    assert!(styled[15].center_x < legacy[15].center_x);
    assert_eq!(styled[15].scale, legacy[15].scale);
    assert!((styled.last().unwrap().center_x - 0.8).abs() < 0.01);
}

#[test]
fn test_no_zoom_trajectory() {
    let default_config = SpringConfig {
        tension: 170.0,
        friction: 30.0,
        mass: 1.0,
    };
    let trajectory = simulate_zoom_trajectory(
        &[],
        &[],
        &default_config,
        false,
        "mellow",
        60.0,
        1000,
        false,
    );
    assert_eq!(trajectory.len(), 60);
    for frame in &trajectory {
        assert!((frame.scale - 1.0).abs() < 0.01);
        assert!((frame.center_x - 0.5).abs() < 0.01);
        assert!((frame.center_y - 0.5).abs() < 0.01);
    }
}

#[test]
fn test_zoom_reaches_target() {
    let blocks = vec![ZoomBlock {
        is_manual: false,
        instant_animation: false,
        is_disabled: false,
        loupe: None,
        animation_lead_in_ms: 0,
        start_time_ms: 100,
        end_time_ms: 2000,
        zoom_level: 2.0,
        center_x: 0.7,
        center_y: 0.3,
        kind: None,
        zoom_in_speed: None,
        zoom_out_speed: None,
        timing_adjusted: false,
        centers: vec![],
    }];
    let zoom_config = SpringConfig {
        tension: 280.0,
        friction: 38.0,
        mass: 1.0,
    };
    let cursor_config = SpringConfig {
        tension: 170.0,
        friction: 30.0,
        mass: 1.0,
    };
    let trajectory = simulate_zoom_trajectory(
        &blocks,
        &[],
        &zoom_config,
        false,
        "mellow",
        60.0,
        3000,
        false,
    );
    // Mid-zoom frame should be near target
    let mid = &trajectory[60]; // 1000ms
    assert!(
        mid.scale > 1.5,
        "scale should approach 2.0, got {}",
        mid.scale
    );
}

#[test]
fn edited_block_holds_zoom_through_its_right_edge() {
    let blocks = vec![ZoomBlock {
        is_manual: false,
        instant_animation: false,
        is_disabled: false,
        loupe: None,
        animation_lead_in_ms: 0,
        start_time_ms: 100,
        end_time_ms: 2000,
        zoom_level: 2.0,
        center_x: 0.5,
        center_y: 0.5,
        kind: None,
        zoom_in_speed: None,
        zoom_out_speed: Some("mellow".to_string()),
        timing_adjusted: true,
        centers: vec![],
    }];
    let config = SpringConfig {
        tension: 170.0,
        friction: 30.0,
        mass: 1.0,
    };

    let trajectory =
        simulate_zoom_trajectory(&blocks, &[], &config, false, "mellow", 60.0, 2400, false);

    assert!(trajectory[119].scale > 1.99);
    assert!(trajectory[120].scale > 1.99);
    assert!(trajectory[135].scale < 1.5);
    let mut unedited = blocks.clone();
    unedited[0].timing_adjusted = false;
    let expected = simulate_zoom_trajectory(&unedited, &[], &config, false, "mellow", 60.0, 2400, false);
    for (actual, expected) in trajectory.iter().zip(expected) {
        assert_eq!(actual.scale, expected.scale);
        assert_eq!(actual.center_x, expected.center_x);
        assert_eq!(actual.center_y, expected.center_y);
    }
}

#[test]
fn lead_in_anticipates_only_in_gaps_and_never_skips_an_instant_range() {
    let block: ZoomBlock = serde_json::from_value(serde_json::json!({
        "start_time_ms": 1000, "end_time_ms": 2000, "zoom_level": 2.0,
        "center_x": 0.3, "center_y": 0.4, "is_manual": true,
        "animation_lead_in_ms": 500
    }))
    .unwrap();
    let config = SpringConfig {
        tension: 250.0,
        friction: 40.0,
        mass: 2.25,
    };
    for fps in [24.0, 30.0, 60.0, 120.0] {
        let run = |blocks: &[ZoomBlock]| {
            simulate_zoom_trajectory(blocks, &[], &config, true, "mellow", fps, 2200, true)
        };
        let anticipated = run(&[block.clone()]);
        let mut legacy = block.clone();
        legacy.animation_lead_in_ms = 0;
        let original = run(&[legacy]);
        for (i, frame) in anticipated.iter().enumerate() {
            let time = i as f64 * 1000.0 / fps;
            if time < 500.0 {
                assert_eq!(frame.scale, 1.0);
            }
            if (750.0..950.0).contains(&time) {
                assert!(frame.scale > 1.5);
                assert_eq!(original[i].scale, 1.0);
            }
        }
        let mut instant = block.clone();
        instant.instant_animation = true;
        let instant_frames = run(&[instant.clone()]);
        assert!(
            instant_frames
                .iter()
                .take((fps * 1.0) as usize)
                .all(|f| f.scale == 1.0)
        );
        // A later long lead-in must not jump over the nearer instant range.
        let mut later = block.clone();
        later.start_time_ms = 2100;
        later.end_time_ms = 3000;
        later.animation_lead_in_ms = 2000;
        let blocked = run(&[later, instant]); // Deliberately unsorted.
        assert!(
            blocked
                .iter()
                .take((fps * 1.0) as usize)
                .all(|f| f.scale == 1.0)
        );
        // An existing range wins even when the next range's lead-in overlaps it.
        let mut earlier = block.clone();
        earlier.start_time_ms = 0;
        earlier.end_time_ms = 900;
        earlier.zoom_level = 1.25;
        earlier.animation_lead_in_ms = 0;
        let baseline = run(&[earlier.clone()]);
        let overlap = run(&[earlier, block.clone()]);
        for i in 0..(fps * 0.9) as usize {
            assert_eq!(overlap[i].scale, baseline[i].scale);
        }
    }
}

#[test]
fn adjusted_zoom_continuity_uses_timeline_order_not_storage_order() {
    let block = |start, end| {
        serde_json::from_value::<ZoomBlock>(serde_json::json!({
            "start_time_ms": start, "end_time_ms": end, "zoom_level": 2.0,
            "center_x": 0.5, "center_y": 0.5, "is_manual": true,
            "timing_adjusted": true
        }))
        .unwrap()
    };
    let ordered = vec![block(0, 1999), block(2000, 3999), block(5000, 5999)];
    let reordered = vec![ordered[2].clone(), ordered[1].clone(), ordered[0].clone()];
    let config = SpringConfig {
        tension: 170.0,
        friction: 50.0,
        mass: 3.0,
    };
    for fps in [24.0, 30.0, 60.0, 120.0] {
        let simulate = |blocks: &[ZoomBlock]| {
            simulate_zoom_trajectory(blocks, &[], &config, true, "mellow", fps, 6500, false)
        };
        let expected = simulate(&ordered);
        let actual = simulate(&reordered);
        for (a, b) in actual.iter().zip(&expected) {
            assert_eq!(a.scale, b.scale);
            assert_eq!(a.center_x, b.center_x);
            assert_eq!(a.center_y, b.center_y);
        }
        // Both ranges hold through their right edges; only the genuine gap exits.
        assert!(actual[(1.9 * fps) as usize].scale > 1.99);
        assert!(actual[(3.9 * fps) as usize].scale > 1.99);
        assert!(actual[(4.5 * fps) as usize].scale < 1.5);
    }
}

#[test]
fn automatic_zoom_holds_small_pointer_motion_and_pans_to_a_new_region() {
    let block: ZoomBlock = serde_json::from_value(serde_json::json!({
        "start_time_ms": 0, "end_time_ms": 6000, "zoom_level": 2.0,
        "center_x": 0.5, "center_y": 0.5
    }))
    .unwrap();
    let events: Vec<_> = (0..60)
        .map(|i| CursorEvent { is_release: false,
            timestamp_ms: i * 100,
            x: if i < 45 {
                if i % 2 == 0 { 0.45 } else { 0.55 }
            } else {
                0.9
            },
            y: 0.5,
            is_click: i == 0,
        })
        .collect();
    let spring = SpringConfig {
        tension: 170.0,
        friction: 50.0,
        mass: 3.0,
    };
    for fps in [24.0, 30.0, 60.0, 120.0] {
        let frames = simulate_zoom_trajectory(
            &[block.clone()],
            &events,
            &spring,
            true,
            "mellow",
            fps,
            6000,
            true,
        );
        let stable = &frames[(2.0 * fps) as usize..(4.4 * fps) as usize];
        let min = stable
            .iter()
            .map(|f| f.center_x)
            .fold(f64::INFINITY, f64::min);
        let max = stable
            .iter()
            .map(|f| f.center_x)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max - min < 0.0001,
            "small pointer motion moved camera at {fps} fps"
        );
        assert!(frames.last().unwrap().center_x > 0.89);
    }
}

#[test]
fn disabled_zoom_is_absent_from_lead_in_and_neighbor_transitions() {
    let config = SpringConfig { tension: 170.0, friction: 30.0, mass: 1.0 };
    let first: ZoomBlock = serde_json::from_value(serde_json::json!({
        "start_time_ms": 500, "end_time_ms": 1200, "zoom_level": 2.0,
        "center_x": 0.3, "center_y": 0.4, "is_manual": true
    })).unwrap();
    let mut disabled = first.clone();
    disabled.start_time_ms = 1200; disabled.end_time_ms = 2500;
    disabled.zoom_level = 4.0; disabled.animation_lead_in_ms = 1000;
    disabled.is_disabled = true;
    for window_mode in [false, true] {
        for instant in [false, true] {
            disabled.instant_animation = instant;
            let run = |blocks: &[ZoomBlock]| simulate_zoom_trajectory(
                blocks, &[], &config, false, "mellow", 120.0, 3000, window_mode);
            let baseline = run(&[first.clone()]);
            let actual = run(&[first.clone(), disabled.clone()]);
            for (a, b) in actual.iter().zip(&baseline) {
                assert_eq!((a.scale, a.center_x, a.center_y), (b.scale, b.center_x, b.center_y));
            }
            assert!(run(&[disabled.clone()]).iter().all(|frame| frame.scale == 1.0));
            let mut enabled = disabled.clone(); enabled.is_disabled = false;
            assert!(run(&[enabled]).iter().any(|frame| frame.scale > 3.0));
        }
    }
}
