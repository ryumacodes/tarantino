//! Event-driven AT-SPI caret geometry. Never requests or retains text content.
use super::{MouseTracker, STREAM_POINTER};
#[path = "linux_caret_geometry.rs"]
mod geometry;
use ashpd::zbus::{self, Connection, MatchRule, MessageStream, Proxy, zvariant::OwnedObjectPath};
use futures::StreamExt;
use parking_lot::Mutex;
use std::{
    sync::Weak,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Default)]
struct Source {
    origin: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
    window: bool,
    generation: u64,
}
static SOURCE: Mutex<Source> = Mutex::new(Source {
    origin: None,
    size: None,
    window: false,
    generation: 0,
});

pub(crate) fn configure_stream_geometry(
    origin: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
    window: bool,
) {
    let mut source = SOURCE.lock();
    *source = Source {
        origin,
        size,
        window,
        generation: source.generation.wrapping_add(1),
    };
}

pub(super) fn start(tracker: Weak<Mutex<MouseTracker>>) {
    tauri::async_runtime::spawn(async move {
        let mut last_error = None;
        loop {
            let Some(current) = tracker.upgrade() else {
                return;
            };
            let active = current.lock().is_tracking;
            drop(current);
            if active {
                // A broken/missing accessibility service must not block recording.
                match listen(&tracker).await {
                    Ok(()) => last_error = None,
                    Err(error) => {
                        let message = error.to_string();
                        if last_error.as_ref() != Some(&message) {
                            eprintln!("Linux caret tracking unavailable: {message}");
                            last_error = Some(message);
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

fn active(tracker: &Weak<Mutex<MouseTracker>>, generation: u64) -> bool {
    let current_generation = SOURCE.lock().generation;
    current_generation == generation
        && tracker
            .upgrade()
            .is_some_and(|tracker| tracker.lock().is_tracking)
}

// Qt accessibility bridges can fail GetAll even though individual Get calls
// work. Avoid proxy property caches, including their eager GetAll request.
async fn proxy<'a>(
    connection: &Connection,
    sender: &'a str,
    path: &'a str,
    interface: &'a str,
) -> zbus::Result<Proxy<'a>> {
    zbus::proxy::Builder::new(connection)
        .destination(sender)?
        .path(path)?
        .interface(interface)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

async fn connect() -> zbus::Result<Connection> {
    let session = Connection::session().await?;
    // Enable accessibility bridges without enabling a screen reader. This is
    // shared desktop state: never disable it when our recording ends.
    let status = proxy(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Status").await?;
    status.set_property("IsEnabled", true).await?;
    let bus = proxy(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus").await?;
    let address: String = bus.call("GetAddress", &()).await?;
    zbus::connection::Builder::address(address.as_str())?
        .build()
        .await
}

async fn listen(tracker: &Weak<Mutex<MouseTracker>>) -> anyhow::Result<()> {
    let source = *SOURCE.lock();
    let debug = std::env::var_os("TARANTINO_DEBUG_CARET").is_some();
    if debug {
        eprintln!(
            "Caret listener: window={} size={:?}",
            source.window, source.size
        );
    }
    let connection = tokio::time::timeout(Duration::from_secs(1), connect()).await??;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.a11y.atspi.Event.Object")?
        .member("TextCaretMoved")?
        .build();
    let mut events = MessageStream::for_match_rule(rule, &connection, Some(32)).await?;
    let registry = proxy(
        &connection,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/registry",
        "org.a11y.atspi.Registry",
    )
    .await?;
    tokio::time::timeout(
        Duration::from_secs(1),
        registry.call::<_, _, ()>(
            "RegisterEvent",
            &("object:text-caret-moved", Vec::<String>::new(), ""),
        ),
    )
    .await??;
    let mut window_binding: Option<(String, String)> = None;
    while active(tracker, source.generation) {
        let message = match tokio::time::timeout(Duration::from_millis(100), events.next()).await {
            Ok(Some(Ok(message))) => message,
            Ok(_) => break,
            Err(_) => continue,
        };
        let received = now_ms();
        // Let the evdev worker publish the corresponding key before refinement.
        tokio::time::sleep(Duration::from_millis(20)).await;
        let Some(tracker) = tracker.upgrade() else {
            break;
        };
        let key_time = {
            let tracker = tracker.lock();
            let events = tracker.key_events.lock();
            events
                .iter()
                .rev()
                .find(|event| event.is_typing && recent(event.timestamp, received))
                .map(|event| event.timestamp)
        };
        if debug {
            eprintln!("Caret signal: recent key={key_time:?}");
        }
        let Some(key_time) = key_time else { continue };
        let header = message.header();
        let (Some(sender), Some(path)) = (header.sender(), header.path()) else {
            continue;
        };
        let sample = tokio::time::timeout(
            Duration::from_millis(150),
            sample(&connection, sender.as_str(), path.as_str(), source.window),
        )
        .await;
        if debug {
            eprintln!("Caret sample: {sample:?}");
        }
        let Ok(Ok(sample)) = sample else { continue };
        let (enabled, dimensions, position) = {
            let pointer = STREAM_POINTER.lock();
            (pointer.enabled, pointer.dimensions, pointer.position)
        };
        let mapped = dimensions.and_then(|dimensions| {
            if !enabled {
                return None;
            }
            let mut mapped_source = source;
            if source.window {
                let coordinates =
                    geometry::coordinate_size(source.size, dimensions, sample.window_bounds?)?;
                mapped_source.size = Some(coordinates);
                let identity = (sender.to_string(), sample.window_path.clone()?);
                if window_binding
                    .as_ref()
                    .is_some_and(|bound| *bound != identity)
                {
                    return None;
                }
                // Acquire a window binding only from a click in this focused field.
                // The captured click and matching top-level dimensions guard
                // against another application's focus event.
                if window_binding.is_none() {
                    let logical = coordinates;
                    let click = tracker
                        .lock()
                        .events
                        .lock()
                        .iter()
                        .rev()
                        .find(|event| {
                            matches!(event.event_type, super::MouseEventType::ButtonPress { .. })
                        })
                        .cloned()?;
                    // GTK may hide the stream cursor as soon as typing starts.
                    // The recent click still belongs to this capture; if the
                    // cursor remains visible it must not have moved elsewhere.
                    let point = position.unwrap_or((click.x, click.y));
                    if received.saturating_sub(click.timestamp) > 1500
                        || (point.0 - click.x).abs() > 2.0
                        || (point.1 - click.y).abs() > 2.0
                        || !sample.field.contains(
                            point.0 * logical.0 as f64 / dimensions.0 as f64,
                            point.1 * logical.1 as f64 / dimensions.1 as f64,
                        )
                    {
                        return None;
                    }
                    window_binding = Some(identity);
                }
            }
            map_point(mapped_source, dimensions, sample.point)
        });
        if debug {
            eprintln!(
                "Caret mapping: dimensions={dimensions:?} pointer={position:?} result={mapped:?}"
            );
        }
        let Some((x, y)) = mapped else { continue };
        let tracker = tracker.lock();
        if !tracker.is_tracking || SOURCE.lock().generation != source.generation {
            continue;
        }
        if let Some(event) = tracker
            .key_events
            .lock()
            .iter_mut()
            .rev()
            .find(|event| event.timestamp == key_time)
        {
            event.caret_x = Some(x);
            event.caret_y = Some(y);
        }
    }
    // Closing this private bus connection also removes registrations if a daemon
    // is too old to accept the app_bus_name argument on DeregisterEvent.
    let _ = tokio::time::timeout(
        Duration::from_millis(150),
        registry.call::<_, _, ()>("DeregisterEvent", &("object:text-caret-moved", "")),
    )
    .await;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}
impl From<(i32, i32, i32, i32)> for Rect {
    fn from((x, y, width, height): (i32, i32, i32, i32)) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}
impl Rect {
    fn valid(self) -> bool {
        self.width >= 0 && self.height > 0
    }
    fn contains(self, x: f64, y: f64) -> bool {
        self.valid()
            && x >= self.x as f64
            && y >= self.y as f64
            && x < self.x as f64 + self.width as f64
            && y < self.y as f64 + self.height as f64
    }
}
#[derive(Debug)]
struct Sample {
    point: (f64, f64),
    field: Rect,
    window_path: Option<String>,
    window_bounds: Option<(i32, i32, i32, i32)>,
}

async fn sample(
    connection: &Connection,
    sender: &str,
    path: &str,
    window: bool,
) -> zbus::Result<Sample> {
    let accessible = proxy(connection, sender, path, "org.a11y.atspi.Accessible").await?;
    let states: Vec<u32> = accessible.call("GetState", &()).await?;
    let flags = states.first().copied().unwrap_or(0);
    if flags & (1 << 12) == 0 || flags & (1 << 25) == 0 || flags & (1 << 30) == 0 {
        return Err(zbus::Error::Failure(
            "Caret is not focused and showing".into(),
        ));
    }
    let text = proxy(connection, sender, path, "org.a11y.atspi.Text").await?;
    let offset: i32 = text.get_property("CaretOffset").await?;
    let coord = u32::from(window);
    let rect: (i32, i32, i32, i32) = text.call("GetCharacterExtents", &(offset, coord)).await?;
    let mut rect = Rect::from(rect);
    let mut trailing = false;
    // Qt returns an empty rectangle at end-of-text. Use the previous glyph's
    // center (also safe for right-to-left text) without fetching the character or the field's contents.
    if !rect.valid() && offset > 0 {
        let previous: (i32, i32, i32, i32) = text
            .call("GetCharacterExtents", &(offset - 1, coord))
            .await?;
        rect = previous.into();
        trailing = true;
    }
    if !rect.valid() {
        return Err(zbus::Error::Failure("Caret bounds unavailable".into()));
    }
    let component = proxy(connection, sender, path, "org.a11y.atspi.Component").await?;
    let (field,): ((i32, i32, i32, i32),) = component.call("GetExtents", &(coord,)).await?;
    let mut result = Sample {
        point: (
            rect.x as f64
                + if trailing {
                    rect.width as f64 / 2.0
                } else {
                    0.0
                },
            rect.y as f64 + rect.height as f64 / 2.0,
        ),
        field: field.into(),
        window_path: None,
        window_bounds: None,
    };
    if window {
        let mut current = path.to_owned();
        for _ in 0..32 {
            let object = proxy(
                connection,
                sender,
                current.as_str(),
                "org.a11y.atspi.Accessible",
            )
            .await?;
            let (bus, parent): (String, OwnedObjectPath) = object.get_property("Parent").await?;
            if bus != sender || parent.as_str() == "/org/a11y/atspi/null" {
                break;
            }
            if parent.as_str() == "/org/a11y/atspi/accessible/root" {
                let component = proxy(
                    connection,
                    sender,
                    current.as_str(),
                    "org.a11y.atspi.Component",
                )
                .await?;
                let (bounds,): ((i32, i32, i32, i32),) =
                    component.call("GetExtents", &(1u32,)).await?;
                result.window_path = Some(current);
                result.window_bounds = Some(bounds);
                break;
            }
            current = parent.to_string();
        }
    }
    Ok(result)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn recent(key: u64, signal: u64) -> bool {
    key <= signal.saturating_add(20) && signal.saturating_sub(key) <= 200
}
fn map_point(source: Source, pixels: (u32, u32), point: (f64, f64)) -> Option<(f64, f64)> {
    let (width, height) = source.size?;
    if width <= 0 || height <= 0 || pixels.0 == 0 || pixels.1 == 0 {
        return None;
    }
    let origin = if source.window {
        (0, 0)
    } else {
        source.origin?
    };
    let x = (point.0 - origin.0 as f64) * pixels.0 as f64 / width as f64;
    let y = (point.1 - origin.1 as f64) * pixels.1 as f64 / height as f64;
    (x.is_finite()
        && y.is_finite()
        && x >= 0.0
        && y >= 0.0
        && x < pixels.0 as f64
        && y < pixels.1 as f64)
        .then_some((x, y))
}

#[cfg(test)]
#[path = "linux_caret_tests.rs"]
mod tests;
