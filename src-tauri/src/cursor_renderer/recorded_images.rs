//! Decode each recorded cursor asset once; playback selects by source timestamp.
use super::image_geometry::Geometry;
use crate::input::cursor_images::{CursorChange, CursorImages};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::RgbaImage;
use std::{io::Cursor, path::Path};

pub struct Image {
    pub pixels: RgbaImage,
    pub geometry: Geometry,
}

#[derive(Default)]
pub struct Track {
    pub images: Vec<Option<Image>>,
    pub changes: Vec<CursorChange>,
    pub fallback: Option<usize>,
}

impl Track {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        Self::load_with_filter(path,true)
    }

    pub fn load_with_filter(path: &Path, optimize: bool) -> anyhow::Result<Self> {
        if !path.exists() { return Ok(Self::default()); }
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let Some(data) = value.get("cursor_images") else { return Ok(Self::default()); };
        let data: CursorImages = serde_json::from_value(data.clone())?;
        let shape = super::shape_track::prepare(&value,optimize);
        let mut track = Self::new(data);
        track.changes = shape.changes;
        track.fallback = shape.fallback;
        Ok(track)
    }

    pub fn new(mut data: CursorImages) -> Self {
        let images = data.images.iter().map(|asset| {
            let png = STANDARD.decode(&asset.png_base64).ok()?;
            let reader = image::ImageReader::with_format(Cursor::new(&png),image::ImageFormat::Png);
            let (w,h) = reader.into_dimensions().ok()?;
            // Bound allocations for imported sidecars before decoding their pixels.
            if w == 0 || h == 0 || w as u64 * h as u64 > 4_194_304 { return None; }
            let pixels = image::load_from_memory_with_format(&png,image::ImageFormat::Png).ok()?.to_rgba8();
            let geometry = Geometry::new(&pixels,[asset.width,asset.height],[asset.hotspot_x,asset.hotspot_y])?;
            Some(Image { pixels,geometry })
        }).collect();
        data.changes.sort_by_key(|change| change.timestamp);
        Self { images, changes: data.changes, fallback: None }
    }

    pub fn index_at(&self, source_ms: f64) -> Option<usize> {
        if !source_ms.is_finite() || source_ms < 0.0 { return None; }
        let (mut low,mut high) = (0,self.changes.len());
        while low < high {
            let middle = low+(high-low)/2;
            if (self.changes[middle].timestamp as f64) < source_ms { low=middle+1; }
            else { high=middle; }
        }
        let selected = if self.changes.get(low).is_some_and(|c|c.timestamp as f64 == source_ms) { Some(low) }
            else { low.checked_sub(1) };
        let id = selected.map(|i|self.changes[i].image_id).or(self.fallback)?;
        self.images.get(id).and_then(Option::as_ref).map(|_|id)
            .or_else(||self.fallback.filter(|&i|self.images.get(i).is_some_and(Option::is_some)))
    }
}
