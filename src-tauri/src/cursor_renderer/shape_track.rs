//! Associate captured appearances with pointer interactions before filtering.
use super::shape_events::{self, Event, Kind};
use crate::input::cursor_images::CursorChange;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub struct Track {
    pub changes: Vec<CursorChange>,
    pub fallback: Option<usize>,
}

pub fn prepare(value: &Value, optimize: bool) -> Track {
    let Some(images) = value.pointer("/cursor_images/images").and_then(Value::as_array) else {
        return Track { changes: Vec::new(), fallback: None };
    };
    let mut changes: Vec<CursorChange> = value.pointer("/cursor_images/changes")
        .and_then(Value::as_array).into_iter().flatten().filter_map(|change| {
            let timestamp = change.get("timestamp")?.as_u64()?;
            let image_id = usize::try_from(change.get("image_id")?.as_u64()?).ok()?;
            (image_id < images.len()).then_some(CursorChange { timestamp, image_id })
        }).collect();
    changes.sort_by_key(|change|change.timestamp);
    let mouse = value.get("mouse_events").and_then(Value::as_array);
    if !optimize || mouse.is_none() { return Track { changes, fallback: None }; }

    let mut identities = HashMap::new();
    let canonical: Vec<usize> = images.iter().enumerate().map(|(index,image)| {
        image.get("system_cursor").and_then(Value::as_str)
            .map(|kind| *identities.entry(kind).or_insert(index)).unwrap_or(index)
    }).collect();
    let fallback = identities.get("arrow").copied();
    let arrow = fallback.unwrap_or(images.len());
    // Appearance observations precede interactions at the same timestamp.
    let mut observations: Vec<(u64,usize,Option<usize>,&Value)> = changes.iter()
        .map(|change|(change.timestamp,0,Some(canonical[change.image_id]),&Value::Null)).collect();
    observations.extend(mouse.into_iter().flatten().filter_map(|event| {
        let base = event.get("base").unwrap_or(event);
        Some((base.get("timestamp")?.as_u64()?,1,None,&base["event_type"]))
    }));
    observations.sort_by_key(|(time,priority,_,_)|(*time,*priority));
    let mut pressed = HashSet::new();
    let mut shape = arrow;
    let events: Vec<Event> = observations.into_iter().map(|(time,_,appearance,kind)| {
        let kind = if let Some(id) = appearance { shape=id; Kind::Other }
        else if has_type(kind,"ButtonPress") {
            pressed.insert(button(kind,"ButtonPress")); Kind::Down
        } else if has_type(kind,"ButtonRelease") {
            pressed.remove(&button(kind,"ButtonRelease")); Kind::Up
        } else if has_type(kind,"Move") {
            if pressed.is_empty() { Kind::Move } else { Kind::Drag }
        } else { Kind::Other };
        Event { time, kind, shape }
    }).collect();
    let changes = shape_events::filter(&events,arrow).into_iter()
        .map(|event|CursorChange {timestamp:event.time,image_id:event.shape}).collect();
    Track { changes, fallback }
}

fn has_type(value:&Value,name:&str)->bool { value.as_str()==Some(name) || value.get(name).is_some() }
fn button(value:&Value,name:&str)->String { value[name]["button"].as_str().or_else(||value[name].as_str()).unwrap_or("Left").to_owned() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captured_shapes_promote_at_clicks_and_keep_arrow_resets_in_order() {
        let mut value=serde_json::json!({"cursor_images":{"images":[
            {"system_cursor":"arrow"},{"system_cursor":"text"},{"system_cursor":"hand"}],
            "changes":[{"timestamp":0,"image_id":0},{"timestamp":100,"image_id":1},
                {"timestamp":400,"image_id":2}]},"mouse_events":[
            {"timestamp":0,"event_type":"Move"},
            {"timestamp":200,"event_type":{"ButtonPress":"Left"}},
            {"timestamp":300,"event_type":{"ButtonRelease":"Left"}},
            {"timestamp":550,"event_type":"Move"},
            {"timestamp":551,"event_type":"Move"},
            {"timestamp":600,"event_type":{"ButtonPress":"Left"}}]});
        let filtered=prepare(&value,true);
        assert_eq!(filtered.fallback,Some(0));
        assert_eq!(filtered.changes.iter().map(|c|(c.timestamp,c.image_id)).collect::<Vec<_>>(),
            [(100,1),(300,1),(551,0),(400,2)]);
        let raw=prepare(&value,false);
        assert_eq!(raw.fallback,None);
        assert_eq!(raw.changes.iter().map(|c|(c.timestamp,c.image_id)).collect::<Vec<_>>(),
            [(0,0),(100,1),(400,2)]);
        for event in value["mouse_events"].as_array_mut().unwrap() {
            event["x"]=50.into(); event["y"]=50.into();
        }
        let path=std::env::temp_dir().join(format!("shape-planning-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&value).unwrap()).unwrap();
        for (enabled,optimize,expected) in [(true,true,vec![400.0,551.0]),
            (true,false,vec![100.0,400.0]),(false,true,vec![])] {
            let planned=crate::cursor_renderer::planner::input::RecordingInput::load(
                &path,(100,100),0.0,None,enabled,optimize).unwrap();
            assert_eq!(planned.sync.iter().filter(|p|p.tolerance==6.0)
                .map(|p|p.time_ms).collect::<Vec<_>>(),expected);
        }
        std::fs::remove_file(path).unwrap();
    }
}
