use super::*;
use crate::cursor_renderer::recorded_images::Track;
use crate::video_processing::{loupe::mip_pyramid::MipPyramid, CursorSettings};
use std::path::Path;

pub(super) struct RecordedCursor {
    track: Track,
    textures: Vec<Option<wgpu::Texture>>,
    active: Option<usize>,
    bound: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use crate::input::cursor_images::{CursorImage,CursorImages};

    fn asset(w:u32,h:u32,color:[u8;4],hotspot:[f64;2])->CursorImage {
        let mut pixels=image::RgbaImage::from_pixel(w,h,image::Rgba(color));
        if color[0]==255 { for y in 0..h/4 {for x in 0..w {pixels.put_pixel(x,y,image::Rgba([0,0,255,255]));}} }
        let mut png=std::io::Cursor::new(Vec::new());
        pixels.write_to(&mut png,image::ImageFormat::Png).unwrap();
        CursorImage {system_cursor:None,width:w as f64,height:h as f64,hotspot_x:hotspot[0],hotspot_y:hotspot[1],
            png_base64:base64::engine::general_purpose::STANDARD.encode(png.into_inner())}
    }

    #[test]
    #[ignore="native GPU recorded cursor image transitions and hotspot"]
    fn recorded_cursor_shapes_press_and_fallback_render() {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "cursor_settings":{"enabled":true,"size":1,"style":"pointer","color":"#ffffff","click_effect":"none","shadow_intensity":0},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.background_color=[0.0;4];config.motion_blur_enabled=false;
        let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
        let placeholder=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
        let mut compositor=GpuCompositor::new(config,Some(&placeholder),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
        let mut images=CursorImages::default();
        images.record(0,asset(8,12,[255,0,0,255],[0.0,0.0]));
        images.record(500,asset(12,8,[0,255,0,255],[0.0,0.0]));
        images.record(1000,asset(48,8,[0,255,0,255],[0.0,0.0]));
        let path=std::env::temp_dir().join(format!("recorded-shapes-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":images})).unwrap()).unwrap();
        compositor.configure_recorded_cursor(&path,settings.cursor_settings.as_ref().unwrap()).unwrap();
        std::fs::remove_file(path).unwrap();
        let camera=ZoomFrameState{scale:1.0,center_x:0.5,center_y:0.5};
        let mut state=crate::cursor_renderer::CursorFrameState::default();
        state.x=0.5;state.y=0.5;state.opacity=1.0;
        let count=|frame:&[u8],channel:usize|frame.chunks_exact(4)
            .filter(|p|p[channel]>100 && p[1-channel]<40).count();
        compositor.set_recorded_cursor_time(Some(499.9));
        let red=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        assert!(count(&red,0)>100 && count(&red,1)==0);
        compositor.set_recorded_cursor_time(Some(500.0));
        let green=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        assert!(count(&green,1)>200 && count(&green,0)==0);
        let bounds=|frame:&[u8]| {
            let mut bounds=[128usize,128,0,0];
            for (i,p) in frame.chunks_exact(4).enumerate().filter(|(_,p)|p[1]>100&&p[0]<40) {
                let (x,y)=(i%128,i/128);bounds[0]=bounds[0].min(x);bounds[1]=bounds[1].min(y);
                bounds[2]=bounds[2].max(x);bounds[3]=bounds[3].max(y);let _=p;
            } bounds
        };
        let normal=bounds(&green);
        state.press_amount=0.1;
        let pressed=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        let small=bounds(&pressed);
        assert_eq!(&normal[..2],&small[..2],"press must keep hotspot fixed");
        assert!(small[2]<normal[2] && small[3]<normal[3]);
        state.fade_blur_radius=2.0;
        let blurred=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        assert!(count(&blurred,1)>0);
        state.fade_blur_radius=0.0;state.press_amount=0.0;
        compositor.set_recorded_cursor_time(Some(0.0));
        assert_eq!(red,compositor.composite_frame(&source,&camera,Some(&state)).unwrap());
        assert_eq!(compositor.recorded_cursor.as_ref().unwrap().textures.iter().filter(|t|t.is_some()).count(),2);
        compositor.set_recorded_cursor_time(None);
        let fallback=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        assert_eq!(count(&fallback,0),0);assert_eq!(count(&fallback,1),0);
        assert!(fallback.chunks_exact(4).any(|p|p[0]>100&&p[1]>100&&p[2]>100));
        compositor.set_recorded_cursor_time(Some(1000.0));
        compositor.config.motion_blur_enabled=true;
        compositor.config.motion_blur_pan_intensity=1.0;
        compositor.config.motion_blur_zoom_intensity=1.0;
        compositor.reset_motion_history();
        state.x=16.0/128.0;state.y=0.5;
        compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        state.x=20.0/128.0;
        let wide=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        assert!(bounds(&wide)[2]>=118,"wide moving cursor clipped at {:?}",bounds(&wide));
        compositor.config.motion_blur_enabled=false;
        state.x=0.5;state.rotation=90.0;
        let rotated=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        assert!(bounds(&rotated)[3]>=120,"rotated wide cursor clipped");
        if let Ok(path)=std::env::var("TARANTINO_RECORDED_CURSOR_FIXTURE") {
            let track=&compositor.recorded_cursor.as_ref().unwrap().track;
            let images=track.images.iter().map(|image|image.as_ref().map(|image|serde_json::json!({
                "width":image.pixels.width(),"height":image.pixels.height(),
                "rgba_base64":base64::engine::general_purpose::STANDARD.encode(image.pixels.as_raw()),
                "rect":image.geometry.rect([0.0;2],17.0)
            }))).collect::<Vec<_>>();
            let mut frames=[(0.0,1.0,0.0,red),(500.0,1.0,0.0,green),
                (500.0,0.9,0.0,pressed),(500.0,0.9,2.0,blurred)].into_iter()
                .map(|(time,scale,blur,expected)|serde_json::json!({"time":time,"scale":scale,"blur":blur,"expected":expected})).collect::<Vec<_>>();
            frames.push(serde_json::json!({"time":1000,"scale":1,"blur":0,"x":20.0/128.0,
                "before":[16,64,1,0],"now":[20,64,1,0],"expected":wide}));
            frames.push(serde_json::json!({"time":1000,"scale":1,"blur":0,"rotation":90,"expected":rotated}));
            std::fs::write(path,serde_json::to_vec(&serde_json::json!({"data":{"images":images,"changes":track.changes},"frames":frames})).unwrap()).unwrap();
        }
    }
}

impl GpuCompositor {
    pub fn configure_recorded_cursor(&mut self, path: &Path, settings: &CursorSettings) -> Result<()> {
        // Other appearance presets retain their procedural renderer until their
        // per-shape treatment is available. Fixed-pointer recordings also use it.
        if settings.always_use_pointer.unwrap_or(false)
            || settings.style.as_deref().unwrap_or("pointer") != "pointer" { return Ok(()); }
        let track = Track::load_with_filter(path,settings.optimize_cursor_types.unwrap_or(true))?;
        let textures = vec![None;track.images.len()];
        self.recorded_cursor = Some(RecordedCursor {track,textures,active:None,bound:None});
        Ok(())
    }

    pub fn set_recorded_cursor_time(&mut self, source_ms: Option<f64>) {
        if let Some(cursor) = &mut self.recorded_cursor {
            cursor.active = source_ms.and_then(|time|cursor.track.index_at(time));
        }
    }

    pub(super) fn recorded_cursor_rect(&self) -> [f32;4] {
        self.recorded_cursor.as_ref().and_then(|cursor|cursor.active
            .and_then(|id|cursor.track.images[id].as_ref()))
            .map_or([0.0;4],|image|image.geometry.rect([0.0;2],17.0).map(|v|v as f32))
    }

    pub(super) fn prepare_cursor_texture(&mut self, uniforms: &CompositeUniforms) -> Result<()> {
        if let Some(cursor) = &mut self.recorded_cursor {
            if let Some(id) = cursor.active {
                if cursor.bound != Some(id) {
                    if cursor.textures[id].is_none() {
                        let image = &cursor.track.images[id].as_ref().unwrap().pixels;
                        let source = Self::create_rgba_texture(&self.device,&self.queue,"Recorded cursor",
                            image.width(),image.height(),image.as_raw());
                        let pyramid = MipPyramid::with_premultiply(&self.device,&source,true)?;
                        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label:Some("Recorded cursor mip chain"),
                        });
                        pyramid.encode(&mut encoder);
                        self.queue.submit(Some(encoder.finish()));
                        cursor.textures[id] = Some(pyramid.texture().clone());
                    }
                    self.cursor_texture = cursor.textures[id].as_ref().unwrap().clone();
                    self.bind_group_dirty = true;
                    cursor.bound = Some(id);
                }
                return Ok(());
            }
            if cursor.bound.take().is_some() { self.cursor_tile = None; }
        }
        if uniforms.cursor_enabled>0.5 && uniforms.cursor_opacity>0.01 && uniforms.cursor_fade_blur>0.04 {
            if self.cursor_tile.is_none() {
                let tile=cursor_tile::CursorTile::new(&self.device)?;
                self.cursor_texture=tile.pyramid.texture().clone();
                self.cursor_tile=Some(tile);self.bind_group_dirty=true;
            }
            self.cursor_tile.as_mut().unwrap().update(&self.device,&self.queue,uniforms);
        }
        Ok(())
    }
}
