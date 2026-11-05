use super::*;
use crate::event_capture::SessionMetadata;

#[test]
fn releases_never_create_typing_zooms_even_if_input_mislabels_them() {
    let session = create_test_session(vec![], 10000);
    let key: KeyEvent = serde_json::from_value(serde_json::json!({
        "timestamp":1000,"is_release":true,"is_modifier":false,"is_typing":true,
        "caret_x":200,"caret_y":300})).unwrap();
    let result = ZoomProcessor::with_default_config().analyze_session(&session, &[key]).unwrap();
    assert!(result.zoom_blocks.is_empty());
}

#[test]
fn typing_uses_caret_bounds_before_normalizing_or_falling_back_to_pointer() {
    let mut session = create_test_session(vec![], 10000);
    session.metadata.capture_region = Some((95,33,1280,820));
    for (x,y,inside) in [(289.0,-1004.0,false), (95.0,33.0,true),
        (1374.0,852.0,true), (1375.0,100.0,false), (200.0,853.0,false),
        (f64::NAN,100.0,false), (200.0,f64::INFINITY,false)] {
        let key = KeyEvent { shortcut: None, is_release: false, timestamp:1000, is_modifier:false, is_typing:true,
            key_motion:crate::input::KeyMotion::Character,
            cursor_x:Some(300.0), cursor_y:Some(300.0), caret_x:Some(x), caret_y:Some(y) };
        let result = ZoomProcessor::with_default_config().analyze_session(&session,&[key]).unwrap();
        assert_eq!(!result.zoom_blocks.is_empty(),inside,"caret ({x},{y})");
    }
}

#[test]
fn typing_supports_negative_capture_origins_and_cursor_fallback_bounds() {
    let mut session = create_test_session(vec![],10000);
    session.metadata.capture_region = Some((-100,-500,200,100));
    for (caret, cursor, inside) in [
        (Some((-50.0,-450.0)), (300.0,300.0), true),
        (None, (-50.0,-450.0), true),
        (None, (339.0,-565.0), false),
    ] {
        let key = KeyEvent { shortcut: None, is_release: false, timestamp:1000, is_modifier:false, is_typing:true,
            key_motion:crate::input::KeyMotion::Character,
            cursor_x:Some(cursor.0),cursor_y:Some(cursor.1),
            caret_x:caret.map(|p|p.0),caret_y:caret.map(|p|p.1) };
        let result=ZoomProcessor::with_default_config().analyze_session(&session,&[key]).unwrap();
        assert_eq!(!result.zoom_blocks.is_empty(),inside);
    }
}

#[test]
fn window_recording_ignores_clicks_above_its_content() {
    let mut session = create_test_session(vec![
        create_test_mouse_event(108541, 865.23046875, 17.7734375),
        create_test_mouse_event(109958, 510.48046875, 18.9296875),
        create_test_mouse_event(111073, 532.0390625, 117.95703125),
    ], 113324);
    session.metadata.capture_region = Some((0, 33, 1470, 923));
    let analysis = ZoomProcessor::with_default_config().analyze_session(&session, &[]).unwrap();
    assert_eq!(analysis.total_clicks, 1);
    assert_eq!(analysis.zoom_blocks.len(), 1);
    let block = &analysis.zoom_blocks[0];
    assert_eq!(block.centers.len(), 1);
    assert_eq!(block.start_time, 110773);
    assert!((block.center_y - (117.95703125 - 33.0) / 923.0).abs() < 1e-9);
}

#[test]
fn click_bounds_support_negative_origins_and_exclude_far_edges() {
    let mut session = create_test_session(vec![
        create_test_mouse_event(1000, -100.0, -50.0),
        create_test_mouse_event(2000, 100.0, 0.0),
        create_test_mouse_event(3000, 0.0, 50.0),
        create_test_mouse_event(4000, f64::NAN, 0.0),
        create_test_mouse_event(5000, 99.0, 49.0),
    ], 10000);
    session.metadata.capture_region = Some((-100, -50, 200, 100));
    let analysis = ZoomProcessor::with_default_config().analyze_session(&session, &[]).unwrap();
    assert_eq!(analysis.total_clicks, 2);
    assert_eq!(analysis.zoom_blocks.iter().map(|b| b.centers.len()).sum::<usize>(), 2);
}

#[test]
fn test_zoom_block_merging() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);

    // Clicks at 1s and 2s should merge into one block (2s is within first block's active range)
    // Rapid clicks retain their own target and extend the same range.
    let session = create_test_session(
        vec![
            create_test_mouse_event(1000, 100.0, 200.0), // First click
            create_test_mouse_event(2000, 300.0, 400.0), // Merges into first block
            create_test_mouse_event(2200, 310.0, 410.0), // Extends the range
        ],
        15000,
    );

    let analysis = processor.analyze_session(&session, &[]).unwrap();

    // One merged block retains all three targets.
    assert_eq!(analysis.zoom_blocks.len(), 1);
    assert_eq!(analysis.total_clicks, 3);

    let block = &analysis.zoom_blocks[0];
    assert_eq!(block.centers.len(), 3);
    assert!(!block.is_manual);
}

#[test]
fn test_zoom_block_separate_when_far_apart() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);

    // Clicks 10s apart should create separate blocks
    let session = create_test_session(
        vec![
            create_test_mouse_event(1000, 100.0, 200.0),
            create_test_mouse_event(11000, 300.0, 400.0),
        ],
        20000,
    );

    let analysis = processor.analyze_session(&session, &[]).unwrap();
    assert_eq!(analysis.zoom_blocks.len(), 2);
}

#[test]
fn early_first_click_survives_preview_sidecar_round_trip() {
    let session = create_test_session(vec![create_test_mouse_event(100, 750.0, 250.0)], 5000);
    let analysis = ZoomProcessor::with_default_config()
        .analyze_session(&session, &[])
        .unwrap();

    assert_eq!(analysis.total_clicks, 1);
    assert_eq!(analysis.zoom_blocks.len(), 1);
    assert_eq!(analysis.zoom_blocks[0].start_time, 1);
    assert!((analysis.zoom_blocks[0].center_x - 0.75).abs() < 0.001);
    assert!((analysis.zoom_blocks[0].center_y - 0.25).abs() < 0.001);

    let json = serde_json::to_string(&analysis).unwrap();
    let reloaded: ZoomAnalysis = serde_json::from_str(&json).unwrap();
    assert_eq!(reloaded.zoom_blocks.len(), 1);
    assert!(reloaded.zoom_blocks[0].end_time > reloaded.zoom_blocks[0].start_time + 500);
}

#[test]
fn test_typing_session_detection() {
    // Simulate typing: 10 keys over 2 seconds, then a 6s gap, then 5 more keys
    let key_events: Vec<KeyEvent> = vec![
        create_test_key_event(1000, true),
        create_test_key_event(1200, true),
        create_test_key_event(1400, true),
        create_test_key_event(1600, true),
        create_test_key_event(1800, true),
        // 6s gap — new session
        create_test_key_event(8000, true),
        create_test_key_event(8200, true),
        create_test_key_event(8400, true),
        create_test_key_event(8600, true),
    ];

    // Mouse at (500, 500) throughout
    let mouse_events = vec![create_test_enhanced_move_event(0, 500.0, 500.0)];

    let metadata = crate::event_capture::SessionMetadata {
        display_id: "test".to_string(),
        display_resolution: (1000, 1000),
        scale_factor: 1.0,
        capture_region: None,
        has_microphone: false,
        has_system_audio: false,
        recording_fps: 60,
        recording_quality: 1.0,
    };

    let config = TypingZoomConfig::default();
    let sessions = detect_typing_sessions(&key_events, &mouse_events, &metadata, &config);
    assert_eq!(sessions.len(), 2, "Should detect 2 typing sessions");
    assert_eq!(sessions[0].key_count, 5);
    assert_eq!(sessions[1].key_count, 4);
    assert_eq!(sessions[0].centers.len(), 5);
    assert_eq!(sessions[1].centers.len(), 4);
}

#[test]
fn test_single_typing_key_creates_session() {
    let key_events: Vec<KeyEvent> = vec![create_test_key_event(1000, true)];

    let metadata = crate::event_capture::SessionMetadata {
        display_id: "test".to_string(),
        display_resolution: (1000, 1000),
        scale_factor: 1.0,
        capture_region: None,
        has_microphone: false,
        has_system_audio: false,
        recording_fps: 60,
        recording_quality: 1.0,
    };

    let config = TypingZoomConfig::default();
    let sessions = detect_typing_sessions(&key_events, &[], &metadata, &config);
    assert_eq!(
        sessions.len(),
        1,
        "A single typing key should start a typing zoom session"
    );
}

#[test]
fn test_typing_zoom_blocks_created() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);

    // No clicks, but typing at 2s
    let session = create_test_session(vec![], 30000);

    let key_events: Vec<KeyEvent> = (0..10)
        .map(|i| create_test_key_event(2000 + i * 200, true))
        .collect();

    let analysis = processor.analyze_session(&session, &key_events).unwrap();
    assert_eq!(
        analysis.zoom_blocks.len(),
        1,
        "Should create 1 typing zoom block"
    );
    assert!(analysis.zoom_blocks[0].id.starts_with("typing_zoom_"));
    assert_eq!(analysis.zoom_blocks[0].kind, "typing");
    assert_eq!(analysis.zoom_blocks[0].zoom_factor, 2.0);
    assert_eq!(analysis.zoom_blocks[0].centers.len(), 10);
}

#[test]
fn test_static_typing_centers_synthesize_pan() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);
    let session = create_test_session(vec![], 30000);

    let key_events: Vec<KeyEvent> = (0..50)
        .map(|i| KeyEvent { shortcut: None,
            is_release: false, timestamp: 2000 + i * 100,
            is_modifier: false,
            is_typing: true,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: Some(300.0),
            cursor_y: Some(800.0),
            caret_x: None,
            caret_y: None,
        })
        .collect();

    let analysis = processor.analyze_session(&session, &key_events).unwrap();
    let centers = &analysis.zoom_blocks[0].centers;
    assert_eq!(centers.len(), 50);
    assert!((centers[0].x - 0.3).abs() < 0.001);
    assert!(
        centers.last().unwrap().x > centers[0].x + 0.25,
        "Static typing centers should pan horizontally during long typing"
    );
    assert!((centers.last().unwrap().y - centers[0].y).abs() < 0.001);
}

#[test]
fn test_static_typing_centers_wrap_on_newline() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);
    let session = create_test_session(vec![], 30000);

    let key_events: Vec<KeyEvent> = (0..12)
        .map(|i| KeyEvent { shortcut: None,
            is_release: false, timestamp: 2000 + i * 100,
            is_modifier: false,
            is_typing: true,
            key_motion: if i == 6 {
                crate::input::KeyMotion::Newline
            } else {
                crate::input::KeyMotion::Character
            },
            cursor_x: Some(300.0),
            cursor_y: Some(500.0),
            caret_x: None,
            caret_y: None,
        })
        .collect();

    let analysis = processor.analyze_session(&session, &key_events).unwrap();
    let centers = &analysis.zoom_blocks[0].centers;
    assert_eq!(centers.len(), 12);
    assert!(
        centers[6].x < centers[5].x,
        "Newline should move the synthetic typing center back toward line start"
    );
    assert!(
        centers[6].y > centers[5].y,
        "Newline should move the synthetic typing center down"
    );
}

#[test]
fn test_typing_click_between_keys_starts_new_context() {
    let key_events = vec![
        create_test_key_event(1000, true),
        create_test_key_event(1200, true),
        create_test_key_event(2000, true),
        create_test_key_event(2200, true),
    ];
    let mouse_events = vec![create_test_mouse_event(1500, 800.0, 300.0)];
    let metadata = crate::event_capture::SessionMetadata {
        display_id: "test".to_string(),
        display_resolution: (1000, 1000),
        scale_factor: 1.0,
        capture_region: None,
        has_microphone: false,
        has_system_audio: false,
        recording_fps: 60,
        recording_quality: 1.0,
    };

    let sessions = detect_typing_sessions(
        &key_events,
        &mouse_events,
        &metadata,
        &TypingZoomConfig::default(),
    );

    assert_eq!(sessions.len(), 2);
    assert!((sessions[0].cursor_x - 0.5).abs() < 0.001);
    assert!((sessions[1].cursor_x - 0.8).abs() < 0.001);
}

#[test]
fn test_typing_inference_ignores_mouse_movement_after_start() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);
    let session = create_test_session(vec![], 30000);

    let key_events: Vec<KeyEvent> = (0..50)
        .map(|i| KeyEvent { shortcut: None,
            is_release: false, timestamp: 2000 + i * 100,
            is_modifier: false,
            is_typing: true,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: Some(if i == 0 { 300.0 } else { 950.0 }),
            cursor_y: Some(if i == 0 { 800.0 } else { 50.0 }),
            caret_x: None,
            caret_y: None,
        })
        .collect();

    let analysis = processor.analyze_session(&session, &key_events).unwrap();
    let centers = &analysis.zoom_blocks[0].centers;
    assert_eq!(centers.len(), 50);
    assert!((centers[0].x - 0.3).abs() < 0.001);
    assert!(
        centers.last().unwrap().x < 0.9,
        "Inferred typing end should not jump to mouse position during typing"
    );
    assert!((centers.last().unwrap().y - 0.8).abs() < 0.001);
}

#[test]
fn test_moving_caret_centers_are_preserved() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);
    let session = create_test_session(vec![], 30000);

    let key_events: Vec<KeyEvent> = (0..10)
        .map(|i| KeyEvent { shortcut: None,
            is_release: false, timestamp: 2000 + i * 100,
            is_modifier: false,
            is_typing: true,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: Some(300.0),
            cursor_y: Some(800.0),
            caret_x: Some(100.0 + (i as f64 * 20.0)),
            caret_y: Some(500.0),
        })
        .collect();

    let analysis = processor.analyze_session(&session, &key_events).unwrap();
    let centers = &analysis.zoom_blocks[0].centers;
    assert!((centers[0].x - 0.1).abs() < 0.001);
    assert!((centers.last().unwrap().x - 0.28).abs() < 0.001);
    assert!((centers.last().unwrap().y - 0.5).abs() < 0.001);
}

#[test]
fn test_typing_centers_prefer_caret_over_cursor() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);
    let session = create_test_session(vec![], 10000);

    let key_events = vec![KeyEvent { shortcut: None,
        is_release: false, timestamp: 1000,
        is_modifier: false,
        is_typing: true,
        key_motion: crate::input::KeyMotion::Character,
        cursor_x: Some(100.0),
        cursor_y: Some(100.0),
        caret_x: Some(800.0),
        caret_y: Some(600.0),
    }];

    let analysis = processor.analyze_session(&session, &key_events).unwrap();
    let block = &analysis.zoom_blocks[0];
    assert_eq!(block.kind, "typing");
    assert!((block.center_x - 0.8).abs() < 0.001);
    assert!((block.center_y - 0.6).abs() < 0.001);
    assert!((block.centers[0].x - 0.8).abs() < 0.001);
    assert!((block.centers[0].y - 0.6).abs() < 0.001);
}

#[test]
fn stationary_and_small_caret_motion_do_not_invent_text_progression() {
    let session = create_test_session(vec![], 10000);
    for advance in [0.0, 0.1] {
        let keys: Vec<_> = (0..20).map(|i| KeyEvent { shortcut: None,
            is_release: false, timestamp: 1000 + i * 100,
            is_modifier: false, is_typing: true,
            key_motion: crate::input::KeyMotion::Newline,
            cursor_x: Some(900.0), cursor_y: Some(900.0),
            caret_x: Some(200.0 + i as f64 * advance), caret_y: Some(300.0),
        }).collect();
        let analysis = ZoomProcessor::with_default_config().analyze_session(&session, &keys).unwrap();
        let centers = &analysis.zoom_blocks[0].centers;
        assert_eq!(centers.len(), keys.len());
        for (center, key) in centers.iter().zip(&keys) {
            assert_eq!(center.time, key.timestamp);
            assert_eq!(center.x, key.caret_x.unwrap() / 1000.0);
            assert_eq!(center.y, 0.3);
        }
    }
}

#[test]
fn intermittent_caret_observations_are_kept_without_inventing_missing_positions() {
    let session = create_test_session(vec![], 10000);
    let keys: Vec<_> = (0..10).map(|i| KeyEvent { shortcut: None,
        is_release: false, timestamp: 1000 + i * 100, is_modifier: false, is_typing: true,
        key_motion: crate::input::KeyMotion::Character,
        cursor_x: Some(900.0), cursor_y: Some(900.0),
        caret_x: (i == 2 || i == 7).then_some(200.0),
        caret_y: (i == 2 || i == 7).then_some(300.0),
    }).collect();
    let analysis = ZoomProcessor::with_default_config().analyze_session(&session, &keys).unwrap();
    let centers = &analysis.zoom_blocks[0].centers;
    assert_eq!(centers.iter().map(|c| c.time).collect::<Vec<_>>(), [1200,1700]);
    assert!(centers.iter().all(|c| c.x == 0.2 && c.y == 0.3));
}

#[test]
fn test_modifier_keys_not_typing() {
    // Modifier keys should not count as typing
    let key_events: Vec<KeyEvent> = vec![
        KeyEvent { shortcut: None,
            is_release: false, timestamp: 1000,
            is_modifier: true,
            is_typing: false,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: None,
            cursor_y: None,
            caret_x: None,
            caret_y: None,
        }, // Cmd
        KeyEvent { shortcut: None,
            is_release: false, timestamp: 1100,
            is_modifier: false,
            is_typing: false,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: None,
            cursor_y: None,
            caret_x: None,
            caret_y: None,
        }, // Cmd+C (modified)
        KeyEvent { shortcut: None,
            is_release: false, timestamp: 1200,
            is_modifier: true,
            is_typing: false,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: None,
            cursor_y: None,
            caret_x: None,
            caret_y: None,
        }, // Cmd release
    ];

    let metadata = crate::event_capture::SessionMetadata {
        display_id: "test".to_string(),
        display_resolution: (1000, 1000),
        scale_factor: 1.0,
        capture_region: None,
        has_microphone: false,
        has_system_audio: false,
        recording_fps: 60,
        recording_quality: 1.0,
    };

    let config = TypingZoomConfig::default();
    let sessions = detect_typing_sessions(&key_events, &[], &metadata, &config);
    assert_eq!(
        sessions.len(),
        0,
        "Modifier keys should not create typing sessions"
    );
}

fn create_test_enhanced_move_event(timestamp: u64, x: f64, y: f64) -> EnhancedMouseEvent {
    EnhancedMouseEvent {
        base: crate::input::MouseEvent {
            timestamp,
            x,
            y,
            event_type: crate::input::MouseEventType::Move,
            display_id: None,
        },
        window_id: None,
        app_name: None,
        is_double_click: false,
        cluster_id: None,
    }
}

fn create_test_key_event(timestamp: u64, is_typing: bool) -> KeyEvent {
    KeyEvent { shortcut: None,
        is_release: false, timestamp,
        is_modifier: false,
        is_typing,
        key_motion: crate::input::KeyMotion::Character,
        cursor_x: None,
        cursor_y: None,
        caret_x: None,
        caret_y: None,
    }
}

fn create_test_mouse_event(timestamp: u64, x: f64, y: f64) -> EnhancedMouseEvent {
    EnhancedMouseEvent {
        base: crate::input::MouseEvent {
            timestamp,
            x,
            y,
            event_type: crate::input::MouseEventType::ButtonPress {
                button: crate::input::MouseButton::Left,
            },
            display_id: None,
        },
        window_id: None,
        app_name: None,
        is_double_click: false,
        cluster_id: None,
    }
}

fn create_test_session(mouse_events: Vec<EnhancedMouseEvent>, duration_ms: u64) -> CaptureSession {
    CaptureSession {
        session_id: "test_session".to_string(),
        start_time: 0,
        end_time: Some(duration_ms),
        mouse_events,
        keyboard_events: vec![],
        window_events: vec![],
        audio_events: vec![],
        metadata: SessionMetadata {
            display_id: "test_display".to_string(),
            display_resolution: (1000, 1000), // Test resolution
            scale_factor: 1.0,
            capture_region: None,
            has_microphone: false,
            has_system_audio: false,
            recording_fps: 60,
            recording_quality: 1.0,
        },
    }
}

#[test]
fn long_selection_bridges_release_range_but_stationary_hold_does_not() {
    for (moved,released,end) in [(true,true,11000),(true,false,12000),(false,true,3500)] {
        let press=create_test_mouse_event(1000,100.0,100.0);
        let mut movement=create_test_mouse_event(6000,if moved {600.0}else{100.0},100.0);
        movement.base.event_type=crate::input::MouseEventType::Move;
        let mut release=movement.clone(); release.base.timestamp=8000;
        release.base.event_type=crate::input::MouseEventType::ButtonRelease{button:crate::input::MouseButton::Left};
        let mut events=vec![press,movement]; if released {events.push(release);}
        let session=create_test_session(events,12000);
        let analysis=ZoomProcessor::with_default_config().analyze_session(&session,&[]).unwrap();
        assert_eq!(analysis.total_clicks,1);
        assert_eq!(analysis.zoom_blocks.len(),if moved {1}else{2});
        assert_eq!(analysis.zoom_blocks[0].end_time,end);
        if !moved { assert_eq!((analysis.zoom_blocks[1].start_time,analysis.zoom_blocks[1].end_time),(7700,10500)); }
        assert!(analysis.zoom_blocks[0].loupe.is_none());
    }
}

#[test]
fn closely_spaced_selection_press_extends_the_existing_zoom() {
    use crate::input::{MouseButton, MouseEventType};
    for inside in [true, false] {
        let first = create_test_mouse_event(1000,100.0,100.0);
        let mut first_release = first.clone();
        first_release.base.timestamp = 1050;
        first_release.base.event_type = MouseEventType::ButtonRelease { button: MouseButton::Left };
        let second = create_test_mouse_event(1200,if inside {100.0}else{1100.0},100.0);
        let mut movement = create_test_mouse_event(6000,600.0,100.0);
        movement.base.event_type = MouseEventType::Move;
        let mut release = movement.clone();
        release.base.timestamp = 8000;
        release.base.event_type = MouseEventType::ButtonRelease { button: MouseButton::Left };
        let session = create_test_session(vec![first,first_release,second,movement,release],12000);
        let analysis = ZoomProcessor::with_default_config().analyze_session(&session,&[]).unwrap();
        assert_eq!(analysis.zoom_blocks.len(),if inside {1}else{2});
        assert_eq!(analysis.zoom_blocks[0].end_time,if inside {11000}else{3550});
        if !inside { assert_eq!(analysis.zoom_blocks[1].start_time,7700); }
    }
}

#[test]
fn click_range_timing_and_gap_boundaries() {
    let cases: &[(&[u64], u64, &[(u64,u64)])] = &[
        (&[],10000,&[]),
        (&[0],1001,&[(1,201)]),
        (&[0],1000,&[]),
        (&[2000],10000,&[(1700,4500)]),
        (&[0],10000,&[(1,2500)]),
        (&[8999],10000,&[(8699,9200)]),
        (&[9000],10000,&[]),
        (&[1000,6300],15000,&[(700,8800)]),
        (&[1000,6301],15000,&[(700,3500),(6001,8801)]),
        (&[2000,2100,2200],10000,&[(1700,4700)]),
    ];
    for &(clicks,duration,expected) in cases {
        let session = create_test_session(clicks.iter().map(|&time|
            create_test_mouse_event(time,100.0,200.0)).collect(),duration);
        let result = ZoomProcessor::with_default_config().analyze_session(&session,&[]).unwrap();
        let actual: Vec<_> = result.zoom_blocks.iter().map(|b|(b.start_time,b.end_time)).collect();
        assert_eq!(actual,expected,"clicks {clicks:?}, duration {duration}");
    }
}

#[test]
fn releases_extend_ranges_without_counting_as_additional_clicks() {
    use crate::input::{MouseButton, MouseEventType};
    for button in [MouseButton::Left,MouseButton::Right] {
        let mut press = create_test_mouse_event(2000,100.0,200.0);
        press.base.event_type = MouseEventType::ButtonPress { button: button.clone() };
        let mut release = press.clone();
        release.base.timestamp = 2100;
        release.base.event_type = MouseEventType::ButtonRelease { button };
        let session = create_test_session(vec![press,release],10000);
        let result = ZoomProcessor::with_default_config().analyze_session(&session,&[]).unwrap();
        assert_eq!(result.total_clicks,1);
        assert_eq!(result.zoom_blocks.len(),1);
        let block = &result.zoom_blocks[0];
        assert_eq!((block.start_time,block.end_time),(1700,4600));
        assert_eq!(block.centers.iter().map(|c|c.time).collect::<Vec<_>>(),[2000,2100]);
    }
}
