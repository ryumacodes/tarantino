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
    #[ignore="native GPU preset hand transitions and fixed-pointer fallback"]
    fn preset_hand_transitions_preserve_selected_appearance() {
        use crate::input::cursor_images::SystemCursor;
        let mut images=CursorImages::default();
        for (index,kind) in [SystemCursor::Arrow,SystemCursor::Hand,SystemCursor::Text].into_iter().enumerate() {
            let mut image=asset(8,12,[255,0,0,255],[0.0,0.0]);
            image.system_cursor=Some(kind);
            images.record(index as u64*100,image);
        }
        let path=std::env::temp_dir().join(format!("preset-shapes-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":images})).unwrap()).unwrap();
        let mut hands=Vec::new();
        for style in ["filled","outline","circle","dotted"] {
            let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
                "resolution":{"width":128,"height":128},
                "cursor_settings":{"enabled":true,"size":2,"style":style,"color":"#ffffff",
                    "click_effect":"none","shadow_intensity":0,"optimize_cursor_types":false},
                "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
            })).unwrap();
            let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
            config.background_color=[0.0;4];config.motion_blur_enabled=false;
            let source=vec![0u8;128*128*4];
            let placeholder=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
            let mut compositor=GpuCompositor::new(config,Some(&placeholder),2.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
            let mut cursor_settings=settings.cursor_settings.unwrap();
            compositor.configure_recorded_cursor(&path,&cursor_settings).unwrap();
            let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
            let mut state=crate::cursor_renderer::CursorFrameState::default();
            state.x=0.5;state.y=0.5;state.opacity=1.0;
            let mut frames=Vec::new();
            for time in [0.0,99.9,100.0,199.9,200.0,100.0,0.0] {
                compositor.set_recorded_cursor_time(Some(time));
                frames.push(compositor.composite_frame(&source,&camera,Some(&state)).unwrap());
            }
            assert_eq!(frames[0],frames[1]);
            assert_eq!(frames[0],frames[4]);
            assert_eq!(frames[0],frames[6]);
            assert_eq!(frames[2],frames[3]);
            assert_eq!(frames[2],frames[5]);
            assert!(frames[2].chunks_exact(4).any(|p|p[3]>100));
            if matches!(style,"filled"|"outline") {
                assert!(frames[0]!=frames[2],"{style} must change to a hand");
                hands.push(frames[2].clone());
            } else { assert_eq!(frames[0],frames[2]); }
            cursor_settings.always_use_pointer=Some(true);
            compositor.configure_recorded_cursor(&path,&cursor_settings).unwrap();
            compositor.set_recorded_cursor_time(Some(100.0));
            assert_eq!(frames[0],compositor.composite_frame(&source,&camera,Some(&state)).unwrap());
        }
        assert!(hands[0]!=hands[1],"filled and outline hands must remain distinct");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore="native GPU subpixel cursor shutter"]
    fn subpixel_recorded_cursor_motion_reconstructs_at_midpoint() {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "cursor_settings":{"enabled":true,"size":1,"style":"pointer","click_effect":"none","shadow_intensity":0},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.background_color=[0.0;4];config.frame_rate=60.0;
        config.motion_blur_cursor_intensity=1.0;
        let source=vec![0u8;128*128*4];
        let placeholder=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
        let mut compositor=GpuCompositor::new(config,Some(&placeholder),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
        let mut images=CursorImages::default();
        images.record(0,asset(8,12,[255,0,0,255],[0.0;2]));
        let path=std::env::temp_dir().join(format!("cursor-midpoint-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":images})).unwrap()).unwrap();
        compositor.configure_recorded_cursor(&path,settings.cursor_settings.as_ref().unwrap()).unwrap();
        std::fs::remove_file(path).unwrap();
        compositor.set_recorded_cursor_time(Some(0.0));
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let mut state=crate::cursor_renderer::CursorFrameState::default();
        state.y=0.5;state.opacity=1.0;
        let mut frames=Vec::new();
        for rotation in [0.0,37.0,90.0] {for delta in [0.25,0.5,0.99] {
            state.rotation=rotation;
            compositor.config.motion_blur_enabled=false;
            state.x=(64.0+delta*0.5)/128.0;
            let expected=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
            compositor.config.motion_blur_enabled=true;
            compositor.reset_motion_history();
            state.x=0.5;
            compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
            state.x=(64.0+delta)/128.0;
            let actual=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
            let error=actual.iter().zip(&expected).map(|(a,b)|a.abs_diff(*b)).max().unwrap();
            assert!(error<=1,"rotation {rotation}, delta {delta}: midpoint error {error}");
            frames.push(serde_json::json!({"time":0,"scale":1,"blur":0,"rotation":rotation,"x":state.x,
                "before":[64,64,1,rotation],"now":[64.0+delta,64,1,rotation],"expected":actual}));
        }}
        if let Ok(path)=std::env::var("TARANTINO_CURSOR_MIDPOINT_FIXTURE") {
            let track=&compositor.recorded_cursor.as_ref().unwrap().track;
            let images=track.images.iter().map(|image|image.as_ref().map(|image|serde_json::json!({
                "width":image.pixels.width(),"height":image.pixels.height(),
                "rgba_base64":base64::engine::general_purpose::STANDARD.encode(image.pixels.as_raw()),
                "rect":image.geometry.rect([0.0;2],17.0)
            }))).collect::<Vec<_>>();
            std::fs::write(path,serde_json::to_vec(&serde_json::json!({"data":{"images":images,"changes":track.changes},"frames":frames})).unwrap()).unwrap();
        }
    }

    #[test]
    #[ignore="native GPU segmented cursor coverage"]
    fn cursor_segments_match_integrated_shutter_coverage() {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "cursor_settings":{"enabled":true,"size":1,"style":"pointer","click_effect":"none","shadow_intensity":0},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.background_color=[0.0;4];config.frame_rate=60.0;
        config.motion_blur_enabled=true;config.motion_blur_cursor_intensity=1.0;
        let source=vec![0u8;128*128*4];
        let placeholder=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
        let mut compositor=GpuCompositor::new(config,Some(&placeholder),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
        let mut images=CursorImages::default();images.record(0,asset(17,17,[255;4],[0.0;2]));
        let pixels=RgbaImage::from_fn(64,32,|x,y| {
            let inside=(8..56).contains(&x) && (4..28).contains(&y) && (x<20 || y<12 || x.abs_diff(2*y)<3);
            let alpha=if inside {if (x+y)%2==0 {255} else {96}} else {0};
            image::Rgba([255,if alpha==0 {0} else {255},255,alpha])
        });
        let mut png=std::io::Cursor::new(Vec::new());pixels.write_to(&mut png,image::ImageFormat::Png).unwrap();
        let mut transparent=asset(64,32,[255;4],[8.0,4.0]);
        transparent.png_base64=base64::engine::general_purpose::STANDARD.encode(png.into_inner());
        images.record(500,transparent);
        let path=std::env::temp_dir().join(format!("cursor-segments-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":images})).unwrap()).unwrap();
        compositor.configure_recorded_cursor(&path,settings.cursor_settings.as_ref().unwrap()).unwrap();
        std::fs::remove_file(path).unwrap();compositor.set_recorded_cursor_time(Some(0.0));
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let poses=[([24.0,48.0,1.0,0.0],[88.0,48.0,1.0,0.0]),
            ([24.0,24.0,1.0,0.0],[88.0,88.0,1.0,0.0]),
            ([64.0,64.0,1.0,-60.0],[64.0,64.0,1.0,60.0]),
            ([32.0,48.0,0.6,-30.0],[80.0,64.0,1.2,45.0])];
        let local=|x:f64,y:f64,pose:[f32;4]| {
            let angle=-(pose[3] as f64).to_radians();let (sin,cos)=angle.sin_cos();
            let x=x-pose[0] as f64;let y=y-pose[1] as f64;
            [(x*cos-y*sin)/pose[2] as f64,(x*sin+y*cos)/pose[2] as f64]
        };
        let mut frames=Vec::new();
        for time in [0.0,500.0] {for (before,now) in poses {
            compositor.set_recorded_cursor_time(Some(time));
            compositor.prev_cursor_pose=Some(before);compositor.cursor_size=now[2];
            let mut state=crate::cursor_renderer::CursorFrameState::default();
            state.x=now[0]/128.0;state.y=now[1]/128.0;state.rotation=now[3];state.opacity=if time==0.0 {1.0} else {0.45};
            let frame=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
            assert!(frame.chunks_exact(4).all(|p|p[3] as f32<=state.opacity*255.0+1.0));
            if time==0.0 {for y in (8..120).step_by(4) {for x in (8..120).step_by(4) {
                let a=local(x as f64,y as f64,before);let b=local(x as f64,y as f64,now);
                if ((b[0]-a[0]).hypot(b[1]-a[1]))*(before[2] as f64)<1.0 {continue;}
                let distance=|t:f32| {
                    let pose=std::array::from_fn(|i|before[i]+(now[i]-before[i])*t);
                    let point=local(x as f64,y as f64,pose);
                    let dx=((point[0]-8.5).abs()-8.5)*pose[2] as f64;
                    let dy=((point[1]-8.5).abs()-8.5)*pose[2] as f64;
                    dx.max(0.0).hypot(dy.max(0.0))+dx.max(dy).min(0.0)
                };
                let (a,m,b)=(distance(0.0),distance(0.5),distance(1.0));
                let mut coverage=0.0;
                for i in 0..2000 {
                    let t=(i as f64+0.5)/2000.0;
                    let d=2.0*(t-0.5)*(t-1.0)*a+4.0*t*(1.0-t)*m+2.0*t*(t-0.5)*b;
                    coverage+=(0.5-d/3.0).clamp(0.0,1.0)/2000.0;
                }
                let actual=frame[(y*128+x)*4+3] as f64;
                assert!((actual-coverage*255.0).abs()<=2.0,"{before:?} -> {now:?} pixel {x},{y}: {actual} vs {}",coverage*255.0);
            }}}
            frames.push(serde_json::json!({"time":time,"opacity":state.opacity,"scale":now[2],"blur":0,"rotation":now[3],
                "x":state.x,"y":state.y,"before":before,"now":now,"expected":frame}));
            for blur in [0.5,2.0] {
                compositor.prev_cursor_pose=Some(before);state.fade_blur_radius=blur;
                let frame=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
                assert!(frame.chunks_exact(4).any(|p|p[3]>0),"fading cursor disappeared");
                frames.push(serde_json::json!({"time":time,"opacity":state.opacity,"scale":now[2],"blur":blur,"rotation":now[3],
                    "x":state.x,"y":state.y,"before":before,"now":now,"expected":frame}));
            }
        }}
        if let Ok(path)=std::env::var("TARANTINO_CURSOR_SEGMENTS_FIXTURE") {
            let track=&compositor.recorded_cursor.as_ref().unwrap().track;
            let images=track.images.iter().map(|image|image.as_ref().map(|image|serde_json::json!({
                "width":image.pixels.width(),"height":image.pixels.height(),
                "rgba_base64":base64::engine::general_purpose::STANDARD.encode(image.pixels.as_raw()),
                "rect":image.geometry.rect([0.0;2],17.0)
            }))).collect::<Vec<_>>();
            std::fs::write(path,serde_json::to_vec(&serde_json::json!({"data":{"images":images,"changes":track.changes},"frames":frames})).unwrap()).unwrap();
        }
    }

    #[test]
    #[ignore="native GPU recorded cursor minification"]
    fn small_recorded_cursor_filters_subpixel_checkerboard() {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "cursor_settings":{"enabled":true,"size":1,"style":"pointer","shadow_intensity":0},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.background_color=[0.0;4];config.motion_blur_enabled=false;
        let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
        let placeholder=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
        let mut compositor=GpuCompositor::new(config,Some(&placeholder),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
        let pixels=RgbaImage::from_fn(64,64,|x,y| {
            let value=if (x+y)%2==0 {0} else {255};
            image::Rgba([value,value,value,255])
        });
        let mut png=std::io::Cursor::new(Vec::new());
        pixels.write_to(&mut png,image::ImageFormat::Png).unwrap();
        let mut image=asset(64,64,[255;4],[0.0;2]);
        image.png_base64=base64::engine::general_purpose::STANDARD.encode(png.into_inner());
        let mut images=CursorImages::default();images.record(0,image);
        let path=std::env::temp_dir().join(format!("cursor-minification-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":images})).unwrap()).unwrap();
        compositor.configure_recorded_cursor(&path,settings.cursor_settings.as_ref().unwrap()).unwrap();
        std::fs::remove_file(path).unwrap();
        compositor.set_recorded_cursor_time(Some(0.0));
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let mut state=crate::cursor_renderer::CursorFrameState::default();
        state.x=0.5;state.y=0.5;state.opacity=1.0;
        let mut frames=Vec::new();
        for rotation in [0.0,37.0,90.0] {for blur in [0.0,0.5,2.0] {for offset in [0.0,0.2,0.5,0.8] {
            state.x=(64.0+offset)/128.0;
            state.rotation=rotation;state.fade_blur_radius=blur;
            let frame=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
            assert!(frame.chunks_exact(4).any(|pixel|pixel[3]>200),"filtered cursor disappeared");
            if rotation==0.0 && blur==0.0 {for y in 68..75 {for x in 68..75 {
                let pixel=&frame[(y*128+x)*4..(y*128+x+1)*4];
                assert!(pixel[..3].iter().all(|v|(126..=129).contains(v)),"offset {offset}: {pixel:?}");
                assert_eq!(pixel[3],255);
            }}}
            frames.push(serde_json::json!({"time":0,"scale":1,"blur":blur,"rotation":rotation,"x":state.x,"expected":frame}));
        }}}
        if let Ok(path)=std::env::var("TARANTINO_CURSOR_MINIFICATION_FIXTURE") {
            let track=&compositor.recorded_cursor.as_ref().unwrap().track;
            let images=track.images.iter().map(|image|image.as_ref().map(|image|serde_json::json!({
                "width":image.pixels.width(),"height":image.pixels.height(),
                "rgba_base64":base64::engine::general_purpose::STANDARD.encode(image.pixels.as_raw()),
                "rect":image.geometry.rect([0.0;2],17.0)
            }))).collect::<Vec<_>>();
            std::fs::write(path,serde_json::to_vec(&serde_json::json!({"data":{"images":images,"changes":track.changes},"frames":frames})).unwrap()).unwrap();
        }
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
        images.images[0].system_cursor = Some(crate::input::cursor_images::SystemCursor::Arrow);
        let path=std::env::temp_dir().join(format!("recorded-shapes-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":images})).unwrap()).unwrap();
        compositor.configure_recorded_cursor(&path,settings.cursor_settings.as_ref().unwrap()).unwrap();
        std::fs::remove_file(path).unwrap();
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
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
        compositor.config.motion_blur_cursor_intensity=1.0;
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
        let path=std::env::temp_dir().join(format!("fixed-recorded-shapes-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"cursor_images":images})).unwrap()).unwrap();
        let mut fixed_settings=settings.cursor_settings.unwrap();
        fixed_settings.always_use_pointer=Some(true);
        compositor.configure_recorded_cursor(&path,&fixed_settings).unwrap();
        std::fs::remove_file(path).unwrap();
        compositor.set_recorded_cursor_time(Some(500.0));
        state.rotation=0.0;
        let fixed=compositor.composite_frame(&source,&camera,Some(&state)).unwrap();
        assert!(count(&fixed,0)>100 && count(&fixed,1)==0,"fixed mode must retain the captured arrow");
    }
}

impl GpuCompositor {
    pub fn configure_recorded_cursor(&mut self, path: &Path, settings: &CursorSettings) -> Result<()> {
        let track = Track::load_for_style(path,settings.optimize_cursor_types.unwrap_or(true),
            settings.always_use_pointer.unwrap_or(false),settings.style.as_deref().unwrap_or("pointer"))?;
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
        if uniforms.cursor_enabled>0.5 && uniforms.cursor_opacity>0.01 {
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
