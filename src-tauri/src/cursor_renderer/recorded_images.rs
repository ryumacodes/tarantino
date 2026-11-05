//! Decode each recorded cursor asset once; playback selects by source timestamp.
use super::image_geometry::Geometry;
use crate::input::cursor_images::{CursorChange, CursorImages, SystemCursor};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::RgbaImage;
use std::{io::Cursor, path::Path};

#[derive(Clone)]
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
        Self::load_for_playback(path, optimize, false)
    }

    pub fn load_for_playback(path: &Path, optimize: bool, fixed_pointer: bool) -> anyhow::Result<Self> {
        Self::load_for_style(path,optimize,fixed_pointer,"pointer")
    }

    pub fn load_for_style(path: &Path, optimize: bool, fixed_pointer: bool, style: &str) -> anyhow::Result<Self> {
        if !matches!(style,"pointer"|"filled"|"outline") || (fixed_pointer && style!="pointer") {
            return Ok(Self::default());
        }
        if !path.exists() { return Ok(Self::default()); }
        let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let shape = (!fixed_pointer).then(|| super::shape_track::prepare(&value,optimize));
        let Some(data) = value.get_mut("cursor_images") else { return Ok(Self::default()); };
        let data: CursorImages = serde_json::from_value(data.take())?;
        if fixed_pointer {
            let Some(pointer) = data.images.iter().position(|asset| asset.system_cursor == Some(SystemCursor::Arrow))
                else { return Ok(Self::default()); };
            let mut track = Self::decode(data, |index| index == pointer,style);
            track.changes.clear();
            track.fallback = Some(pointer);
            return Ok(track);
        }
        let shape = shape.expect("dynamic cursor track was prepared");
        let mut selected = vec![false;data.images.len()];
        for id in shape.changes.iter().map(|change|change.image_id).chain(shape.fallback) {
            if let Some(selected) = selected.get_mut(id) { *selected = true; }
        }
        let mut track = Self::decode(data, |index| selected[index],style);
        track.changes = shape.changes;
        track.fallback = shape.fallback;
        Ok(track)
    }

    pub fn new(data: CursorImages) -> Self {
        Self::decode(data, |_| true,"pointer")
    }

    fn decode(mut data: CursorImages, selected: impl Fn(usize) -> bool, style: &str) -> Self {
        let images = data.images.iter().enumerate().map(|(index,asset)| {
            if !selected(index) { return None; }
            if style!="pointer" {
                return (asset.system_cursor==Some(SystemCursor::Hand))
                    .then(||super::preset_hand::image(style=="outline"));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::cursor_images::CursorImage;

    #[test]
    fn alternate_presets_use_semantic_hands_and_fall_back_for_other_shapes() {
        let mut data=CursorImages::default();
        for (index,kind) in [SystemCursor::Arrow,SystemCursor::Hand,SystemCursor::Text].into_iter().enumerate() {
            data.record(index as u64*100,CursorImage {system_cursor:Some(kind),
                width:8.0,height:12.0,hotspot_x:0.0,hotspot_y:0.0,png_base64:String::new()});
        }
        let path=std::env::temp_dir().join(format!("preset-cursor-{}.json",uuid::Uuid::new_v4()));
        let save=|data:&CursorImages|std::fs::write(&path,
            serde_json::to_vec(&serde_json::json!({"cursor_images":data})).unwrap()).unwrap();
        save(&data);
        for style in ["filled","outline"] {
            let track=Track::load_for_style(&path,false,false,style).unwrap();
            assert_eq!([0.0,99.9,100.0,199.9,200.0,100.0,0.0].map(|t|track.index_at(t)),
                [None,None,Some(1),Some(1),None,Some(1),None]);
            assert_eq!(track.images.iter().flatten().count(),1);
            let fixed=Track::load_for_style(&path,false,true,style).unwrap();
            assert!(fixed.images.is_empty());
        }
        for style in ["circle","dotted"] {
            assert!(Track::load_for_style(&path,false,false,style).unwrap().images.is_empty());
        }
        for asset in &mut data.images {asset.system_cursor=None;}
        save(&data);
        for style in ["filled","outline"] {
            let legacy=Track::load_for_style(&path,false,false,style).unwrap();
            assert_eq!(legacy.index_at(100.0),None);
            assert_eq!(legacy.images.iter().flatten().count(),0);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn fixed_pointer_keeps_captured_bitmap_and_hotspot_across_shape_changes() {
        let mut png = Cursor::new(Vec::new());
        RgbaImage::from_pixel(8, 12, image::Rgba([80, 120, 200, 255]))
            .write_to(&mut png, image::ImageFormat::Png).unwrap();
        let asset = |kind| CursorImage {
            system_cursor: kind, width: 8.0, height: 12.0, hotspot_x: 2.0, hotspot_y: 3.0,
            png_base64: STANDARD.encode(png.get_ref()),
        };
        let mut data = CursorImages::default();
        data.record(0, asset(Some(SystemCursor::Text)));
        data.record(100, asset(Some(SystemCursor::Arrow)));
        data.record(200, asset(Some(SystemCursor::Hand)));
        let path = std::env::temp_dir().join(format!("fixed-cursor-{}.json", uuid::Uuid::new_v4()));
        let save = |data: &CursorImages| std::fs::write(&path,
            serde_json::to_vec(&serde_json::json!({"cursor_images":data})).unwrap()).unwrap();
        save(&data);
        let dynamic = Track::load_for_playback(&path, false, false).unwrap();
        assert_eq!([0.0, 100.0, 200.0].map(|t| dynamic.index_at(t)), [Some(0), Some(1), Some(2)]);
        for optimize in [false, true] {
            let fixed = Track::load_for_playback(&path, optimize, true).unwrap();
            for time in [0.0, 99.5, 100.0, 200.0, 900.0] { assert_eq!(fixed.index_at(time), Some(1)); }
            assert_eq!(fixed.images.iter().flatten().count(), 1);
            let arrow = fixed.images[1].as_ref().unwrap();
            assert_eq!(arrow.pixels, dynamic.images[1].as_ref().unwrap().pixels);
            assert_eq!(arrow.geometry.rect([10.0, 20.0], 17.0),
                dynamic.images[1].as_ref().unwrap().geometry.rect([10.0, 20.0], 17.0));
            assert_eq!(fixed.index_at(-1.0), None);
            assert_eq!(fixed.index_at(f64::NAN), None);
        }
        for asset in &mut data.images { asset.system_cursor = None; }
        save(&data);
        let legacy = Track::load_for_playback(&path, true, true).unwrap();
        assert!(legacy.images.is_empty());
        assert_eq!(legacy.index_at(200.0), None, "unknown shapes must use the procedural pointer");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn filtered_loading_retains_only_reachable_images_without_changing_seeks() {
        let mut png = Cursor::new(Vec::new());
        RgbaImage::from_pixel(16,24,image::Rgba([80,120,200,255]))
            .write_to(&mut png,image::ImageFormat::Png).unwrap();
        let mut data = CursorImages::default();
        for (index,kind) in [SystemCursor::Arrow,SystemCursor::Text,SystemCursor::Hand].into_iter().enumerate() {
            data.record(index as u64*100, CursorImage {system_cursor:Some(kind),
                width:16.0,height:24.0,hotspot_x:2.0,hotspot_y:3.0,
                png_base64:STANDARD.encode(png.get_ref())});
        }
        let value = serde_json::json!({"cursor_images":data,"mouse_events":[
            {"timestamp":0,"event_type":"Move"},
            {"timestamp":100,"event_type":{"ButtonPress":"Left"}},
            {"timestamp":150,"event_type":{"ButtonRelease":"Left"}},
            {"timestamp":200,"event_type":"Move"},
            {"timestamp":500,"event_type":"Move"}
        ]});
        let shape = super::super::shape_track::prepare(&value,true);
        let mut baseline = Track::new(data);
        baseline.changes=shape.changes;
        baseline.fallback=shape.fallback;
        let path=std::env::temp_dir().join(format!("filtered-cursor-load-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&value).unwrap()).unwrap();
        let filtered=Track::load_for_playback(&path,true,false).unwrap();
        let raw=Track::load_for_playback(&path,false,false).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(filtered.images.iter().flatten().count(),2);
        assert!(filtered.images[2].is_none());
        assert_eq!(raw.images.iter().flatten().count(),3);
        for time in [0.0,99.9,100.0,150.0,200.0,499.9,500.0,700.0,100.0] {
            assert_eq!(filtered.index_at(time),baseline.index_at(time));
            if let Some(id)=filtered.index_at(time) {
                assert_eq!(filtered.images[id].as_ref().unwrap().pixels,baseline.images[id].as_ref().unwrap().pixels);
            }
        }
    }

    #[test]
    #[ignore = "cursor image loading memory and timing workload"]
    fn fixed_pointer_image_loading_profile() {
        let mut png = Cursor::new(Vec::new());
        RgbaImage::from_fn(256,256,|x,y| image::Rgba([x as u8,y as u8,120,255]))
            .write_to(&mut png,image::ImageFormat::Png).unwrap();
        let asset = CursorImage { system_cursor:None, width:256.0,height:256.0,
            hotspot_x:20.0,hotspot_y:10.0,png_base64:STANDARD.encode(png.get_ref()) };
        let mut data = CursorImages { images:vec![asset;96],changes:(0..96).map(|image_id| CursorChange {timestamp:image_id as u64 * 100,image_id}).collect() };
        data.images[42].system_cursor=Some(SystemCursor::Arrow);
        let path=std::env::temp_dir().join(format!("cursor-load-profile-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":data})).unwrap()).unwrap();
        let started=std::time::Instant::now();
        let all=Track::load_for_playback(&path,false,false).unwrap();
        let all_time=started.elapsed();
        let started=std::time::Instant::now();
        let fixed=Track::load_for_playback(&path,false,true).unwrap();
        let fixed_time=started.elapsed();
        std::fs::remove_file(path).unwrap();
        let bytes=|track:&Track|track.images.iter().flatten().map(|i|i.pixels.as_raw().len()).sum::<usize>();
        assert_eq!(bytes(&all),96*256*256*4);
        assert_eq!(bytes(&fixed),256*256*4);
        assert_eq!(fixed.index_at(100.0),Some(42));
        assert_eq!(fixed.images[42].as_ref().unwrap().pixels,all.images[42].as_ref().unwrap().pixels);
        println!("cursor images: all={all_time:?} fixed={fixed_time:?}; decoded bytes all={} fixed={}",bytes(&all),bytes(&fixed));
    }

}
