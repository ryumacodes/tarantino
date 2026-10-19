use super::*;
#[test]
fn maps_scaled_monitor_with_negative_origin() {
    let source = Source {
        origin: Some((-1920, 0)),
        size: Some((1920, 1080)),
        ..Source::default()
    };
    assert_eq!(
        map_point(source, (3840, 2160), (-1800.0, 100.0)),
        Some((240.0, 200.0))
    );
    assert_eq!(map_point(source, (3840, 2160), (100.0, 100.0)), None);
}
#[test]
fn window_uses_local_coordinates_and_rejects_invalid_geometry() {
    let source = Source {
        window: true,
        size: Some((400, 200)),
        ..Source::default()
    };
    assert_eq!(
        map_point(source, (580, 290), (100.0, 50.0)),
        Some((145.0, 72.5))
    );
    assert_eq!(map_point(source, (580, 290), (f64::NAN, 50.0)), None);
    assert_eq!(map_point(source, (580, 290), (400.0, 50.0)), None);
}
#[test]
fn never_refines_stale_or_future_keys() {
    assert!(recent(1000, 1050));
    assert!(!recent(1000, 1201));
    assert!(!recent(1100, 1000));
}
#[tokio::test]
#[ignore = "requires a graphical Linux desktop, AT-SPI and kdialog"]
async fn native_text_field_geometry() {
    native_field_geometry(false).await;
}
#[tokio::test]
#[ignore = "requires a graphical Linux desktop, AT-SPI and Python GTK3"]
async fn native_gtk_text_field_geometry() {
    native_field_geometry(true).await;
}
async fn native_field_geometry(gtk: bool) {
    struct Fixture(std::process::Child);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut command = if gtk {
        let mut command = std::process::Command::new("/usr/bin/python3");
        command.env("GDK_BACKEND", "x11").args([
            "-c",
            r#"import gi
gi.require_version('Gtk','3.0')
from gi.repository import Gtk
w=Gtk.Window(title='Tarantino Caret Smoke Test');w.set_default_size(400,160)
e=Gtk.Entry();e.set_text('test');w.add(e);w.show_all();e.grab_focus();e.set_position(4)
Gtk.main()
"#,
        ]);
        command
    } else {
        let mut command = std::process::Command::new("kdialog");
        command.env("QT_QPA_PLATFORM", "xcb").args([
            "--title",
            "Tarantino Caret Smoke Test",
            "--inputbox",
            "Test-only accessibility field",
            "test",
        ]);
        command
    };
    let _fixture = Fixture(command.stdout(std::process::Stdio::null()).spawn().unwrap());
    let connection = connect().await.unwrap();
    let root = proxy(
        &connection,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/accessible/root",
        "org.a11y.atspi.Accessible",
    )
    .await
    .unwrap();
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let apps: Vec<(String, OwnedObjectPath)> = root.call("GetChildren", &()).await.unwrap();
        for (bus, path) in apps {
            let app = proxy(
                &connection,
                bus.as_str(),
                path.as_str(),
                "org.a11y.atspi.Accessible",
            )
            .await
            .unwrap();
            let Ok(windows) = app
                .call::<_, _, Vec<(String, OwnedObjectPath)>>("GetChildren", &())
                .await
            else {
                continue;
            };
            for (bus, path) in windows {
                let window = proxy(
                    &connection,
                    bus.as_str(),
                    path.as_str(),
                    "org.a11y.atspi.Accessible",
                )
                .await
                .unwrap();
                if window
                    .get_property::<String>("Name")
                    .await
                    .unwrap_or_default()
                    != "Tarantino Caret Smoke Test"
                {
                    continue;
                }
                let mut pending = vec![(bus, path)];
                while let Some((bus, path)) = pending.pop() {
                    let sampled = sample(&connection, &bus, path.as_str(), true).await;
                    if let Ok(sample) = sampled
                        && sample.caret_valid
                    {
                        assert!(sample.point.0 > 0.0 && sample.point.1 > 0.0);
                        assert!(sample.field.contains(sample.point.0 - 1.0, sample.point.1));
                        let size = geometry::client_size(
                            sample.window_bounds.expect("top-level geometry"),
                        )
                        .unwrap();
                        assert!(size.0 > 0 && size.1 > 0);
                        println!("Native caret {:?}, top-level {:?}", sample.point, size);
                        check_native_event(&connection, &bus, path.as_str(), &sample, gtk).await;
                        return;
                    }
                    let object = proxy(
                        &connection,
                        bus.as_str(),
                        path.as_str(),
                        "org.a11y.atspi.Accessible",
                    )
                    .await
                    .unwrap();
                    pending.extend(
                        object
                            .call::<_, _, Vec<(String, OwnedObjectPath)>>("GetChildren", &())
                            .await
                            .unwrap_or_default(),
                    );
                }
            }
        }
    }
    panic!("The test text field did not expose focused caret geometry");
}
async fn check_native_event(
    connection: &Connection,
    bus: &str,
    path: &str,
    initial: &Sample,
    pixel_coordinates: bool,
) {
    use crate::input::{KeyEvent, KeyMotion, MouseButton, MouseEvent, MouseEventType};
    use std::sync::Arc;
    let size = geometry::client_size(initial.window_bounds.unwrap()).unwrap();
    let factor = if pixel_coordinates { 1 } else { 2 };
    let dimensions = (size.0 as u32 * factor, size.1 as u32 * factor);
    let logical_size = if pixel_coordinates {
        (
            (size.0 as f64 / 1.45).round() as i32,
            (size.1 as f64 / 1.45).round() as i32,
        )
    } else {
        size
    };
    let click = (
        (initial.field.x as f64 + initial.field.width as f64 / 2.0) * factor as f64,
        (initial.field.y as f64 + initial.field.height as f64 / 2.0) * factor as f64,
    );
    configure_stream_geometry(None, (!pixel_coordinates).then_some(logical_size), true);
    {
        let mut pointer = STREAM_POINTER.lock();
        pointer.enabled = true;
        pointer.dimensions = Some(dimensions);
        pointer.position = Some(click);
    }
    let tracker = Arc::new(Mutex::new(MouseTracker::new()));
    {
        let mut tracker = tracker.lock();
        tracker.is_tracking = true;
        tracker.recording_start_time = Some(SystemTime::now());
        tracker.events.lock().push_back(MouseEvent {
            timestamp: now_ms(),
            x: click.0,
            y: click.1,
            event_type: MouseEventType::ButtonPress {
                button: MouseButton::Left,
            },
            display_id: None,
        });
    }
    let weak = Arc::downgrade(&tracker);
    let worker = tokio::spawn(async move { listen(&weak).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let text = proxy(connection, bus, path, "org.a11y.atspi.Text")
        .await
        .unwrap();
    // No focus/caret signal: this field was already focused before the worker
    // started. Discover it from the captured click, then type after the old
    // deadline with the pointer elsewhere.
    tokio::time::sleep(Duration::from_millis(2100)).await;
    STREAM_POINTER.lock().position = Some((dimensions.0 as f64 - 1.0, dimensions.1 as f64 - 1.0));
    let mut result = None;
    for attempt in 0..8 {
        {
            let tracker = tracker.lock();
            tracker.key_events.lock().clear();
            tracker.add_key_event(KeyEvent {
                timestamp: now_ms(),
                is_modifier: false,
                is_typing: true,
                key_motion: KeyMotion::Character,
                cursor_x: Some(click.0),
                cursor_y: Some(click.1),
                caret_x: None,
                caret_y: None,
            });
        }
        if pixel_coordinates {
            // Real KDE window streams can omit logical size, and GTK hides
            // the pointer on typing. Exercise that production combination.
            STREAM_POINTER.lock().position = None;
        }
        let moved: bool = text
            .call(
                "SetCaretOffset",
                &(if attempt % 2 == 0 { 3i32 } else { 4i32 },),
            )
            .await
            .unwrap();
        assert!(moved);
        tokio::time::sleep(Duration::from_millis(100)).await;
        result = tracker
            .lock()
            .key_events
            .lock()
            .back()
            .and_then(|event| event.caret_x.zip(event.caret_y));
        if result.is_some() {
            break;
        }
    }
    tracker.lock().is_tracking = false;
    let worker_result = tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
    super::super::configure_stream_pointer(false);
    worker_result.expect("native caret subscription");
    let result = result.expect("AT-SPI caret signal must refine the recorded typing event");
    assert!(
        result.0 < click.0 - 10.0,
        "caret must move zoom away from the pointer"
    );
    assert!((result.1 - initial.point.1 * factor as f64).abs() < 1.0);
    println!("Native caret event mapped to recording pixels: {result:?}");
    // Feed the accessibility-refined event through the actual zoom engine.
    // A valid coordinate alone would not prove the editor receives a zoom.
    let mut key = tracker.lock().key_events.lock().back().unwrap().clone();
    key.timestamp = 1_000;
    let mut session = crate::event_capture::CaptureSession::new();
    session.start_time = 0;
    session.end_time = Some(5_000);
    session.metadata.display_resolution = dimensions;
    let analysis = crate::auto_zoom::ZoomProcessor::with_default_config()
        .analyze_session(&session, &[key])
        .unwrap();
    let typing = analysis
        .zoom_blocks
        .iter()
        .find(|block| block.kind == "typing")
        .expect("native accessibility event must create typing zoom by default");
    assert!((typing.center_x - result.0 / dimensions.0 as f64).abs() < 0.0001);
    assert!((typing.center_y - result.1 / dimensions.1 as f64).abs() < 0.0001);
    assert!(typing.zoom_factor > 1.0);
}
