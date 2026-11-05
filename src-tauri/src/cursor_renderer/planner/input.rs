//! Recording input for logical-pixel cursor planning. Down/up and held-button
//! movement are constraints; ordinary movement only defines the source path.
use super::{
    activity::SyncPoint,
    press::{Event, Kind},
    Point,
};
use crate::video_processing::{export::filter_pointer_shakes, source_crop::SourceCrop};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{collections::HashSet, path::Path};

pub struct RecordingInput {
    pub points: Vec<Point>,
    pub sync: Vec<SyncPoint>,
    pub press_events: Vec<Event>,
    pub width: f64,
    pub height: f64,
}

impl RecordingInput {
    pub fn load(
        path: &Path,
        source_size: (u32, u32),
        shake_threshold: f64,
        crop: Option<SourceCrop>,
        include_shape_constraints: bool,
        optimize_shapes: bool,
    ) -> Result<Self> {
        let bytes = std::fs::read(path).context("Cannot read cursor input")?;
        let value = serde_json::from_slice(&bytes).context("Invalid cursor input JSON")?;
        let mut input = Self::parse(&value, source_size, shake_threshold, crop)?;
        if include_shape_constraints { input.add_image_sync(&value,optimize_shapes); }
        Ok(input)
    }

    pub fn parse(
        value: &Value,
        source_size: (u32, u32),
        shake_threshold: f64,
        crop: Option<SourceCrop>,
    ) -> Result<Self> {
        let events = value
            .get("mouse_events")
            .unwrap_or(value)
            .as_array()
            .context("Cursor input must contain an event array")?;
        let display_width = number(value, "display_width", source_size.0 as f64)?;
        let display_height = number(value, "display_height", source_size.1 as f64)?;
        let area = value
            .get("recording_area")
            .filter(|a| !a.is_null())
            .unwrap_or(&Value::Null);
        let mut width = number(area, "width", display_width)?;
        let mut height = number(area, "height", display_height)?;
        let mut origin = [number(area, "x", 0.0)?, number(area, "y", 0.0)?];
        if width <= 0.0 || height <= 0.0 {
            bail!("Cursor capture dimensions must be positive");
        }
        if let Some(crop) = crop {
            // Resolve against recorded pixels for validation, but retain the
            // logical-pixel metric used for interaction accuracy.
            let crop = crop.resolve(source_size.0, source_size.1)?.normalized();
            origin[0] += crop.x * width;
            origin[1] += crop.y * height;
            width *= crop.width;
            height *= crop.height;
        }
        let mut events = filter_pointer_shakes(events.clone(), shake_threshold);
        events.sort_by_key(|event| {
            event
                .get("base")
                .unwrap_or(event)
                .get("timestamp")
                .and_then(Value::as_u64)
        });
        let mut points = Vec::with_capacity(events.len());
        let mut sync = Vec::new();
        let mut press_events = Vec::with_capacity(events.len());
        let mut pressed = HashSet::new();
        for event in &events {
            let base = event.get("base").unwrap_or(event);
            let Some(time) = base.get("timestamp").and_then(Value::as_u64) else {
                continue;
            };
            let (Some(x), Some(y)) = (
                base.get("x").and_then(Value::as_f64),
                base.get("y").and_then(Value::as_f64),
            ) else {
                continue;
            };
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            let time_ms = time as f64;
            let (x, y) = (x - origin[0], y - origin[1]);
            if !x.is_finite() || !y.is_finite() {
                bail!("Cursor coordinate exceeds capture geometry");
            }
            points.push(Point { time_ms, x, y });
            let kind = &base["event_type"];
            let press = has_type(kind, "ButtonPress");
            let release = has_type(kind, "ButtonRelease");
            let moving = has_type(kind, "Move");
            if press {
                pressed.insert(button(kind, "ButtonPress"));
            }
            if press || release || moving {
                press_events.push(Event {
                    time: time_ms,
                    kind: if press {
                        Kind::Down
                    } else if release {
                        Kind::Up
                    } else if !pressed.is_empty() {
                        Kind::Drag
                    } else {
                        Kind::Move
                    },
                });
            }
            if press || release || (moving && !pressed.is_empty()) {
                sync.push(SyncPoint {
                    time_ms,
                    tolerance: 2.0,
                });
            }
            if release {
                pressed.remove(&button(kind, "ButtonRelease"));
            }
        }
        Ok(Self {
            points,
            sync,
            press_events,
            width,
            height,
        })
    }

    fn add_image_sync(&mut self, value: &Value, optimize: bool) {
        let track = crate::cursor_renderer::shape_track::prepare(value,optimize);
        #[derive(PartialEq)]
        enum Identity<'a> { System(&'a str), Image(usize) }
        let images = value.pointer("/cursor_images/images").and_then(Value::as_array);
        let mut previous = None;
        for change in track.changes {
            let identity = images.and_then(|images| images.get(change.image_id))
                .and_then(|image| image.get("system_cursor")).and_then(Value::as_str)
                .map_or(Identity::Image(change.image_id), Identity::System);
            if previous.as_ref().is_some_and(|old| old != &identity) {
                self.sync.push(SyncPoint { time_ms: change.timestamp as f64, tolerance: 6.0 });
            }
            previous = Some(identity);
        }
        self.sync.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    }

    pub fn normalize(&self, point: [f64; 2]) -> [f64; 2] {
        [point[0] / self.width, point[1] / self.height]
    }
}

fn number(value: &Value, key: &str, fallback: f64) -> Result<f64> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(fallback),
        Some(value) => value
            .as_f64()
            .filter(|n| n.is_finite())
            .with_context(|| format!("Invalid cursor capture {key}")),
    }
}
fn has_type(kind: &Value, name: &str) -> bool {
    kind.as_str() == Some(name) || kind.get(name).is_some()
}
fn button(kind: &Value, name: &str) -> String {
    kind.get(name)
        .and_then(|v| v.get("button").or(Some(v)))
        .and_then(Value::as_str)
        .unwrap_or("Unknown")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn image_changes_constrain_motion_without_synthesizing_clicks() {
        let value = json!({"mouse_events":[
            {"timestamp":0,"x":0,"y":0,"event_type":"Move"},
            {"timestamp":100,"x":50,"y":50,"event_type":{"ButtonPress":"Left"}}
        ],"cursor_images":{"images":[{},{}],"changes":[
            {"timestamp":0,"image_id":0}, {"timestamp":20,"image_id":0},
            {"timestamp":50,"image_id":1}, {"timestamp":70,"image_id":99},
            {"timestamp":100,"image_id":0}
        ]}});
        let mut input = RecordingInput::parse(&value,(100,100),0.0,None).unwrap();
        input.add_image_sync(&value,false);
        assert_eq!(input.sync, vec![SyncPoint{time_ms:50.0,tolerance:6.0},
            SyncPoint{time_ms:100.0,tolerance:2.0},SyncPoint{time_ms:100.0,tolerance:6.0}]);
        assert_eq!(input.press_events.len(),2);
        let path = std::env::temp_dir().join(format!("cursor-shapes-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&value).unwrap()).unwrap();
        let fixed = RecordingInput::load(&path,(100,100),0.0,None,false,false).unwrap();
        let shapes = RecordingInput::load(&path,(100,100),0.0,None,true,false).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(fixed.sync,vec![SyncPoint{time_ms:100.0,tolerance:2.0}]);
        assert_eq!(shapes.sync,input.sync);
    }

    #[test]
    fn repeated_system_bitmaps_do_not_add_shape_constraints() {
        let value = json!({"mouse_events":[
            {"timestamp":0,"x":0,"y":0,"event_type":"Move"},
            {"timestamp":600,"x":90,"y":40,"event_type":"Move"}
        ],"cursor_images":{"images":[
            {"system_cursor":"arrow"},{"system_cursor":"arrow"},
            {"system_cursor":"text"},{"system_cursor":"text"},{},{}
        ],"changes":[
            {"timestamp":0,"image_id":0},{"timestamp":100,"image_id":1},
            {"timestamp":200,"image_id":2},{"timestamp":300,"image_id":3},
            {"timestamp":400,"image_id":4},{"timestamp":500,"image_id":5}
        ]}});
        let mut input = RecordingInput::parse(&value,(100,100),0.0,None).unwrap();
        input.add_image_sync(&value,false);
        assert_eq!(input.sync, [200.0,400.0,500.0].map(|time_ms|SyncPoint {time_ms,tolerance:6.0}));
        assert!(input.press_events.iter().all(|event| !matches!(event.kind, Kind::Down | Kind::Up)));
        let raw = crate::cursor_renderer::shape_track::prepare(&value,false);
        assert_eq!(raw.changes.len(),6,"raw bitmap playback must retain all observations");
    }

    #[test]
    fn button_transitions_and_drag_keep_logical_pixels_and_stable_order() {
        let value = json!({"display_width":1470,"display_height":956,"scale_factor":2,
            "recording_area":{"x":-300,"y":50,"width":200,"height":100},"mouse_events":[
            {"timestamp":0,"x":-250,"y":75,"event_type":"Move"},
            {"timestamp":10,"x":-250,"y":75,"event_type":{"ButtonPress":{"button":"Left"}}},
            {"base":{"timestamp":20,"x":-240,"y":80,"event_type":"Move"}},
            {"timestamp":30,"x":-240,"y":80,"event_type":{"ButtonPress":"Right"}},
            {"timestamp":40,"x":-240,"y":80,"event_type":{"ButtonRelease":{"button":"Left"}}},
            {"timestamp":50,"x":-230,"y":80,"event_type":"Move"},
            {"timestamp":60,"x":-230,"y":80,"event_type":{"ButtonRelease":"Right"}},
            {"timestamp":70,"x":-400,"y":80,"event_type":"Move"}
        ]});
        let input = RecordingInput::parse(&value, (400, 200), 0.0, None).unwrap();
        assert_eq!(
            input
                .press_events
                .iter()
                .map(|e| e.kind)
                .collect::<Vec<_>>(),
            vec![
                Kind::Move,
                Kind::Down,
                Kind::Drag,
                Kind::Down,
                Kind::Up,
                Kind::Drag,
                Kind::Up,
                Kind::Move
            ]
        );
        assert_eq!(
            input.sync.iter().map(|p| p.time_ms).collect::<Vec<_>>(),
            vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0]
        );
        assert_eq!(
            input.normalize([input.points[0].x, input.points[0].y]),
            [0.25, 0.25]
        );
        assert_eq!(input.points[2].x - input.points[1].x, 10.0);
        assert_eq!(
            input.normalize([input.points[7].x, input.points[7].y]),
            [-0.5, 0.3]
        );
        let cropped = RecordingInput::parse(
            &value,
            (400, 200),
            0.0,
            Some(SourceCrop {
                x: 0.25,
                y: 0.0,
                width: 0.5,
                height: 1.0,
            }),
        )
        .unwrap();
        assert_eq!(
            cropped.normalize([cropped.points[0].x, cropped.points[0].y]),
            [0.0, 0.25]
        );
        assert_eq!(cropped.points[2].x - cropped.points[1].x, 10.0);
        assert_eq!(cropped.sync, input.sync);
    }

    #[test]
    fn compatibility_shapes_and_shake_removal_do_not_fabricate_drags() {
        let value = json!([
            {"timestamp":0,"x":200,"y":200,"event_type":"ButtonPress"},
            {"timestamp":1,"x":0,"y":0,"event_type":"Move"},
            {"timestamp":2,"x":1,"y":1,"event_type":"Move"},
            {"timestamp":3,"x":2,"y":2,"event_type":"ButtonRelease"},
            {"timestamp":4,"x":3,"y":3,"event_type":"Move"}
        ]);
        let input = RecordingInput::parse(&value, (100, 100), 100.0, None).unwrap();
        assert!(input.sync.is_empty());
        assert!(input.press_events.iter().all(|e| e.kind == Kind::Move));
        assert_eq!(input.points.len(), 3);
        assert!(RecordingInput::parse(&value, (0, 100), 0.0, None).is_err());
    }
}
