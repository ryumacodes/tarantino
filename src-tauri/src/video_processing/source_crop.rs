//! Source-pixel crop geometry, distinct from capture bounds and output padding.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Normalized edges in the original recording, with a top-left origin.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct SourceCrop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    source_width: u32,
    source_height: u32,
}

impl SourceCrop {
    pub fn resolve(self, width: u32, height: u32) -> Result<PixelCrop> {
        if width == 0 || height == 0
            || ![self.x, self.y, self.width, self.height].iter().all(|n| n.is_finite())
            || self.x < 0.0 || self.y < 0.0 || self.width <= 0.0 || self.height <= 0.0
            || self.x + self.width > 1.0 + 1e-9 || self.y + self.height > 1.0 + 1e-9
        {
            bail!("Crop must be a nonempty rectangle inside the source video");
        }
        // Quantize edges, not width independently, so adjacent rectangles share
        // the same boundary. Cropping runs in RGBA before output chroma conversion.
        let x = (self.x * width as f64).round() as u32;
        let y = (self.y * height as f64).round() as u32;
        let right = ((self.x + self.width) * width as f64).round().min(width as f64) as u32;
        let bottom = ((self.y + self.height) * height as f64).round().min(height as f64) as u32;
        if right <= x || bottom <= y {
            bail!("Crop must contain at least one source pixel");
        }
        Ok(PixelCrop { x, y, width: right - x, height: bottom - y,
            source_width: width, source_height: height })
    }
}

impl PixelCrop {
    pub fn normalized(self) -> SourceCrop {
        SourceCrop { x: self.x as f64 / self.source_width as f64,
            y: self.y as f64 / self.source_height as f64,
            width: self.width as f64 / self.source_width as f64,
            height: self.height as f64 / self.source_height as f64 }
    }

    /// Keep outside positions outside: clamping would pin an invisible cursor to
    /// the crop edge. Camera target bounds are applied separately after grouping.
    pub fn source_point(self, x: f64, y: f64) -> (f64, f64) {
        ((x * self.source_width as f64 - self.x as f64) / self.width as f64,
         (y * self.source_height as f64 - self.y as f64) / self.height as f64)
    }
}

/// Transform only source-relative targets. Manual targets already describe the
/// visible crop, so changing the source crop must not reinterpret their anchors.
pub fn remap_motion(crop: Option<PixelCrop>, events: &mut [crate::cursor_renderer::CursorEvent],
    blocks: &mut [super::types::ZoomBlock]) {
    let Some(crop) = crop else { return; };
    for event in events {
        (event.x,event.y) = crop.source_point(event.x,event.y);
    }
    for block in blocks.iter_mut().filter(|block| !block.is_manual) {
        (block.center_x,block.center_y) = crop.source_point(block.center_x,block.center_y);
    }
}

impl PixelCrop {
    pub fn decode_filter(self) -> String {
        format!("format=rgba,crop={}:{}:{}:{},",self.width,self.height,self.x,self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_targets_move_into_crop_space_while_manual_anchors_stay_relative() {
        let crop = SourceCrop{x:0.5,y:0.0,width:0.5,height:1.0}.resolve(128,64).unwrap();
        let mut events = vec![crate::cursor_renderer::CursorEvent{ is_release: false,timestamp_ms:0,x:0.75,y:0.5,is_click:true}];
        let automatic: super::super::types::ZoomBlock = serde_json::from_value(serde_json::json!({
            "start_time_ms":0,"end_time_ms":1000,"zoom_level":2,"center_x":0.75,"center_y":0.5
        })).unwrap();
        let mut manual = automatic.clone();
        manual.is_manual = true;
        let mut blocks = [automatic,manual];
        remap_motion(Some(crop),&mut events,&mut blocks);
        assert_eq!((events[0].x,events[0].y),(0.5,0.5));
        assert_eq!(blocks[0].center_x,0.5);
        assert_eq!(blocks[1].center_x,0.75);
    }

    #[test]
    fn quantizes_edges_and_preserves_off_crop_coordinates() {
        let crop = SourceCrop { x:0.25,y:0.1,width:0.5,height:0.8 }.resolve(1919,1079).unwrap();
        assert_eq!((crop.x,crop.y,crop.width,crop.height), (480,108,959,863));
        for (x,y,expected) in [(480.0,108.0,0.0),(1439.0,971.0,1.0)] {
            let point = crop.source_point(x/1919.0,y/1079.0);
            assert!((point.0-expected).abs()<1e-12 && (point.1-expected).abs()<1e-12);
        }
        assert!(crop.source_point(0.0,0.0).0 < 0.0);
        assert!(crop.source_point(1.0,1.0).1 > 1.0);
        let normalized = crop.normalized();
        assert_eq!(normalized.resolve(1919,1079).unwrap(),crop);
    }
    #[test]
    fn full_source_is_identity_and_invalid_crops_are_rejected() {
        let full = SourceCrop { x:0.0,y:0.0,width:1.0,height:1.0 };
        let crop = full.resolve(1920,1080).unwrap();
        assert_eq!(crop.source_point(0.25,0.75),(0.25,0.75));
        for invalid in [SourceCrop { x:-0.1,..full },SourceCrop { x:f64::NAN,..full },
            SourceCrop { width:0.0,..full },SourceCrop { width:1.1,..full },
            SourceCrop { width:0.00001,..full }] {
            assert!(invalid.resolve(100,100).is_err());
        }
        assert!(full.resolve(0,1080).is_err());
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;
    use crate::video_processing::{timeline::EditTimeline, timeline_decode::video_decode_args_with_crop};
    use std::process::Command;

    #[test]
    #[ignore = "requires local FFmpeg; odd-pixel crop through edited timeline"]
    fn crop_is_applied_before_scaling_and_never_to_generated_gaps() {
        let root = std::env::temp_dir().join(format!("crop-decode-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let input = root.join("source.mkv");
        // RGB generation keeps the odd blue rectangle exact, without source chroma subsampling.
        let result = Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
            "color=red:s=100x60:r=20:d=1,format=rgba,drawbox=x=33:y=11:w=31:h=25:color=blue:t=fill",
            "-c:v","ffv1"]).arg(&input).output().unwrap();
        assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
        let clips: Vec<crate::video_processing::timeline::EditClip> = serde_json::from_value(serde_json::json!([
            {"id":"fast","startTime":0,"duration":250,"sourceIn":0,"sourceOut":500,"playbackRate":2,"trackOrder":0,"muted":false},
            {"id":"later","startTime":500,"duration":250,"sourceIn":500,"sourceOut":750,"playbackRate":1,"trackOrder":0,"muted":false}
        ])).unwrap();
        let plan = EditTimeline::build(&clips,0.0,750.0).unwrap();
        let crop = SourceCrop{x:0.33,y:11.0/60.0,width:0.31,height:25.0/60.0}.resolve(100,60).unwrap();
        let result = Command::new(crate::ffmpeg::binary("ffmpeg")).args(video_decode_args_with_crop(&plan,&input,62,50,20,Some(crop))).output().unwrap();
        assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
        assert_eq!(result.stdout.len(),15*62*50*4);
        for (index,frame) in result.stdout.chunks_exact(62*50*4).enumerate() {
            for pixel in frame.chunks_exact(4) {
                if (5..10).contains(&index) {
                    assert!(pixel[..3].iter().all(|v|*v<8),"nonblack gap frame {index}");
                } else {
                    assert!(pixel[2]>240 && pixel[0]<8 && pixel[1]<8,"crop frame {index}: {pixel:?}");
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
