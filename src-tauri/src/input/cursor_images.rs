//! Deduplicated cursor appearances and their recording-clock transitions.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemCursor {
    Arrow, Text, Hand, Crosshair, ClosedHand, OpenHand, ResizeLeftRight, ResizeUpDown,
    ResizeLeft, ResizeRight, ResizeUp, ResizeDown, VerticalText, DisappearingItem,
    NotAllowed, DragLink, DragCopy, ContextualMenu, ZoomIn, ZoomOut,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CursorImage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_cursor: Option<SystemCursor>,
    pub width: f64,
    pub height: f64,
    pub hotspot_x: f64,
    pub hotspot_y: f64,
    pub png_base64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CursorChange {
    pub timestamp: u64,
    pub image_id: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CursorImages {
    pub images: Vec<CursorImage>,
    pub changes: Vec<CursorChange>,
}

impl CursorImages {
    pub fn record(&mut self, timestamp: u64, image: CursorImage) {
        let image_id = self.images.iter().position(|old| {
            old.width == image.width && old.height == image.height
                && old.hotspot_x == image.hotspot_x && old.hotspot_y == image.hotspot_y
                && old.system_cursor == image.system_cursor
                && old.png_base64 == image.png_base64
        }).unwrap_or_else(|| { self.images.push(image); self.images.len() - 1 });
        if self.changes.last().is_none_or(|last| last.image_id != image_id) {
            self.changes.push(CursorChange { timestamp, image_id });
        }
    }

    pub fn normalized(&self, start: u64) -> Self {
        let mut changes = Vec::new();
        if let Some(seed) = self.changes.iter().rev().find(|c| c.timestamp <= start) {
            changes.push(CursorChange { timestamp: 0, image_id: seed.image_id });
        }
        changes.extend(self.changes.iter().filter(|c| c.timestamp > start)
            .map(|c| CursorChange { timestamp: c.timestamp - start, image_id: c.image_id }));
        Self { images: self.images.clone(), changes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(png: &str) -> CursorImage {
        CursorImage { system_cursor: None, width: 28.0, height: 40.0, hotspot_x: 5.0, hotspot_y: 5.0, png_base64: png.into() }
    }
    #[test]
    fn returning_shapes_reuse_assets_and_keep_hotspot_changes() {
        let mut track = CursorImages::default();
        for (time, name) in [(100,"arrow"),(110,"arrow"),(120,"text"),(130,"arrow")] {
            track.record(time, image(name));
        }
        let mut shifted = image("arrow"); shifted.hotspot_x = 6.0;
        track.record(140, shifted);
        assert_eq!(track.images.len(), 3);
        assert_eq!(track.changes.iter().map(|c| c.image_id).collect::<Vec<_>>(), [0,1,0,2]);
        let relative = track.normalized(125);
        assert_eq!(relative.changes, vec![CursorChange {timestamp:0,image_id:1},
            CursorChange {timestamp:5,image_id:0}, CursorChange {timestamp:15,image_id:2}]);
        assert_eq!(track.normalized(90).changes[0].timestamp, 10);
        assert_eq!(track.normalized(120).changes[0], CursorChange {timestamp:0,image_id:1});
    }

    #[test]
    fn system_identity_roundtrips_without_guessing_for_legacy_assets() {
        let legacy: CursorImage = serde_json::from_value(serde_json::json!({
            "width":28,"height":40,"hotspot_x":5,"hotspot_y":5,"png_base64":"png"
        })).unwrap();
        assert_eq!(legacy.system_cursor, None);
        let mut arrow = legacy.clone();
        arrow.system_cursor = Some(SystemCursor::Arrow);
        let mut track = CursorImages::default();
        track.record(0,legacy);
        track.record(100,arrow.clone());
        track.record(200,arrow);
        let restored: CursorImages = serde_json::from_slice(&serde_json::to_vec(&track).unwrap()).unwrap();
        assert_eq!(restored.images.len(),2);
        assert_eq!(restored.changes.len(),2);
        assert_eq!(restored.normalized(150).images[1].system_cursor,Some(SystemCursor::Arrow));
        assert_eq!(restored.normalized(150).changes[0].image_id,1);
    }

    #[test]
    fn additional_native_shapes_keep_distinct_identities_in_saved_tracks() {
        use SystemCursor::*;
        let kinds = [ResizeLeft,ResizeRight,ResizeUp,ResizeDown,VerticalText,
            DisappearingItem,NotAllowed,DragLink,DragCopy,ContextualMenu,ZoomIn,ZoomOut];
        let mut track = CursorImages::default();
        for (index,kind) in kinds.iter().enumerate() {
            let mut asset = image("same-pixels");
            asset.system_cursor = Some(*kind);
            track.record(index as u64 * 100, asset);
        }
        let restored: CursorImages = serde_json::from_slice(&serde_json::to_vec(&track).unwrap()).unwrap();
        assert_eq!(restored.images.len(), kinds.len());
        assert_eq!(restored.changes.len(), kinds.len());
        for (index,kind) in kinds.iter().enumerate() {
            assert_eq!(restored.images[index].system_cursor,Some(*kind));
            assert_eq!(restored.normalized(index as u64 * 100).changes[0].image_id,index);
        }
    }
}
