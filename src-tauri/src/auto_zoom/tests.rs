use super::*;
use crate::event_capture::SessionMetadata;

#[test]
fn test_zoom_block_merging() {
    let config = ZoomConfig::default();
    let processor = ZoomProcessor::new(config);

    // Clicks at 1s and 2s should merge into one block (2s is within first block's active range)
    // Click at 2.2s is filtered by min_click_spacing (< 500ms from 2s)
    let session = create_test_session(
        vec![
            create_test_mouse_event(1000, 100.0, 200.0), // First click
            create_test_mouse_event(2000, 300.0, 400.0), // Merges into first block
            create_test_mouse_event(2200, 310.0, 410.0), // Filtered (too close to 2000)
        ],
        15000,
    );

    let analysis = processor.analyze_session(&session, &[]).unwrap();

    // Should create 1 merged block with 2 centers
    assert_eq!(analysis.zoom_blocks.len(), 1);
    assert_eq!(analysis.total_clicks, 3);

    let block = &analysis.zoom_blocks[0];
    assert_eq!(block.centers.len(), 2); // Two click centers merged
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
        .map(|i| KeyEvent {
            timestamp: 2000 + i * 100,
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
        .map(|i| KeyEvent {
            timestamp: 2000 + i * 100,
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
        .map(|i| KeyEvent {
            timestamp: 2000 + i * 100,
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
        .map(|i| KeyEvent {
            timestamp: 2000 + i * 100,
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

    let key_events = vec![KeyEvent {
        timestamp: 1000,
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
fn test_modifier_keys_not_typing() {
    // Modifier keys should not count as typing
    let key_events: Vec<KeyEvent> = vec![
        KeyEvent {
            timestamp: 1000,
            is_modifier: true,
            is_typing: false,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: None,
            cursor_y: None,
            caret_x: None,
            caret_y: None,
        }, // Cmd
        KeyEvent {
            timestamp: 1100,
            is_modifier: false,
            is_typing: false,
            key_motion: crate::input::KeyMotion::Character,
            cursor_x: None,
            cursor_y: None,
            caret_x: None,
            caret_y: None,
        }, // Cmd+C (modified)
        KeyEvent {
            timestamp: 1200,
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
    KeyEvent {
        timestamp,
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
