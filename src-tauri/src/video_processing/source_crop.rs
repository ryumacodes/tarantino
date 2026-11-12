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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropSampling {
    pub bounds: [f64; 4],
    pub decode: PixelCrop,
    pub view: [f64; 4],
}

impl SourceCrop {
    pub fn sampling(self, width: u32, height: u32, padding: u32) -> Result<CropSampling> {
        self.validate(width, height)?;
        let bounds = [self.x * width as f64, self.y * height as f64,
            self.width * width as f64, self.height * height as f64];
        let left = bounds[0].floor() as u32;
        let top = bounds[1].floor() as u32;
        let right = (bounds[0] + bounds[2]).ceil().min(width as f64) as u32;
        let bottom = (bounds[1] + bounds[3]).ceil().min(height as f64) as u32;
        if right <= left || bottom <= top {
            bail!("Crop must intersect the source video");
        }
        let x = left.saturating_sub(padding);
        let y = top.saturating_sub(padding);
        let right = right.saturating_add(padding).min(width);
        let bottom = bottom.saturating_add(padding).min(height);
        let decode = PixelCrop { x, y, width: right - x, height: bottom - y,
            source_width: width, source_height: height };
        let view = [(bounds[0] - x as f64) / decode.width as f64,
            (bounds[1] - y as f64) / decode.height as f64,
            bounds[2] / decode.width as f64, bounds[3] / decode.height as f64];
        Ok(CropSampling { bounds, decode, view })
    }

    // Keep the original mip grid and enough neighboring pixels for filtering.
    // Odd source extents cannot be rebased without changing normalized mip sampling.
    pub fn sampling_for_mips(self, width: u32, height: u32, last_level: u32) -> Result<CropSampling> {
        let mut plan = self.sampling(width, height, 0)?;
        let block = 1u32.checked_shl(last_level).unwrap_or(u32::MAX);
        let axis = |start: u32, extent: u32, source: u32| {
            if source % block != 0 { return (0, source); }
            let padding = block.saturating_mul(2);
            let first = start.saturating_sub(padding) / block * block;
            let end = start.saturating_add(extent).saturating_add(padding).min(source);
            let last = end.div_ceil(block).saturating_mul(block).min(source);
            (first, last - first)
        };
        let (x, w) = axis(plan.decode.x, plan.decode.width, width);
        let (y, h) = axis(plan.decode.y, plan.decode.height, height);
        plan.decode = PixelCrop { x, y, width: w, height: h, source_width: width, source_height: height };
        plan.view = [(plan.bounds[0] - x as f64) / w as f64,
            (plan.bounds[1] - y as f64) / h as f64, plan.bounds[2] / w as f64, plan.bounds[3] / h as f64];
        Ok(plan)
    }

    fn validate(self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0
            || ![self.x, self.y, self.width, self.height].iter().all(|n| n.is_finite())
            || self.x < 0.0 || self.y < 0.0 || self.width <= 0.0 || self.height <= 0.0
            || self.x + self.width > 1.0 + 1e-9 || self.y + self.height > 1.0 + 1e-9
        {
            bail!("Crop must be a nonempty rectangle inside the source video");
        }
        Ok(())
    }

    pub fn resolve(self, width: u32, height: u32) -> Result<PixelCrop> {
        self.validate(width, height)?;
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

    /// Keep outside positions outside so motion continues across the crop edge.
    /// Camera target bounds are applied separately after grouping.
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
        for center in &mut block.centers {
            (center.x,center.y) = crop.source_point(center.x,center.y);
        }
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
    fn cropped_typing_follows_the_same_path_as_crop_local_caret_points() {
        use crate::{cursor_renderer::SpringConfig,video_processing::zoom_trajectory::simulate_zoom_trajectory};
        let crop=SourceCrop{x:0.5,y:0.25,width:0.5,height:0.5}.resolve(1920,1080).unwrap();
        let original:super::super::types::ZoomBlock=serde_json::from_value(serde_json::json!({
            "start_time_ms":0,"end_time_ms":1000,"zoom_level":2,
            "center_x":0.7,"center_y":0.5,"kind":"typing",
            "centers":[{"time":100,"x":0.65,"y":0.45},{"time":800,"x":0.85,"y":0.55}]
        })).unwrap();
        let mut actual=[original.clone()];
        remap_motion(Some(crop),&mut [],&mut actual);
        let mut expected=original.clone();
        expected.center_x=0.4;expected.center_y=0.5;
        expected.centers[0].x=0.3;expected.centers[0].y=0.4;
        expected.centers[1].x=0.7;expected.centers[1].y=0.6;
        let spring=SpringConfig{tension:250.0,friction:40.0,mass:2.25};
        for window in [false,true] {for fps in [30.0,60.0,120.0] {
            let actual=simulate_zoom_trajectory(&actual,&[],&spring,true,"focused",fps,1200,window);
            let expected=simulate_zoom_trajectory(&[expected.clone()],&[],&spring,true,"focused",fps,1200,window);
            for (a,b) in actual.iter().zip(&expected) {
                for (a,b) in a.sample().into_iter().zip(b.sample()) {
                    assert!((a-b).abs()<1e-10,"window={window}, fps={fps}: {a} != {b}");
                }
            }
        }}
        let mut manual=[original];manual[0].is_manual=true;
        remap_motion(Some(crop),&mut [],&mut manual);
        assert_eq!((manual[0].center_x,manual[0].centers[0].x),(0.7,0.65));
    }

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

#[cfg(test)]
mod sampling_tests {
    use super::*;

    #[test]
    fn fractional_geometry_survives_decode_envelopes_and_filter_padding() {
        let crop=SourceCrop{x:0.25,y:0.1,width:0.5,height:0.8};
        for padding in [0,1,16,u32::MAX] {
            let plan=crop.sampling(1919,1079,padding).unwrap();
            assert_eq!(plan.bounds,[479.75,107.9,959.5,863.2]);
            let raster=plan.decode;
            assert!(raster.x as f64<=plan.bounds[0] && raster.y as f64<=plan.bounds[1]);
            assert!((raster.x+raster.width) as f64>=plan.bounds[0]+plan.bounds[2]);
            assert!((raster.y+raster.height) as f64>=plan.bounds[1]+plan.bounds[3]);
            for [u,v] in [[0.0,0.0],[0.7,0.625],[1.0,1.0]] {
                let texture=[plan.view[0]+u*plan.view[2],plan.view[1]+v*plan.view[3]];
                let source=[raster.x as f64+texture[0]*raster.width as f64,
                    raster.y as f64+texture[1]*raster.height as f64];
                assert!((source[0]-(plan.bounds[0]+u*plan.bounds[2])).abs()<1e-10);
                assert!((source[1]-(plan.bounds[1]+v*plan.bounds[3])).abs()<1e-10);
            }
        }
    }

    #[test]
    fn subpixel_crops_keep_geometry_and_padding_stays_inside_source() {
        let crop=SourceCrop{x:0.001,y:0.998,width:0.002,height:0.002};
        let plan=crop.sampling(100,100,1).unwrap();
        assert_eq!((plan.decode.x,plan.decode.y,plan.decode.width,plan.decode.height),(0,98,2,2));
        assert_eq!(plan.bounds[2],0.2);
        let full=SourceCrop{x:0.0,y:0.0,width:1.0,height:1.0}.sampling(100,100,16).unwrap();
        assert_eq!(full.view,[0.0,0.0,1.0,1.0]);
        assert!(SourceCrop{x:1.0,y:0.0,width:1e-10,height:1.0}.sampling(100,100,1).is_err());
        assert!(SourceCrop{x:f64::NAN,y:0.0,width:0.5,height:0.5}.sampling(100,100,1).is_err());
    }
}
