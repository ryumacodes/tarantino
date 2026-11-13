use super::*;
use crate::cursor_renderer::{simulate_cursor_frames,CursorEvent,SpringConfig};

#[test]
#[ignore="native GPU; isolated cursor bodies retain style, rotation and visibility"]
fn cursor_body_styles_rotation_and_opacity_remain_distinct() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":30},
        "visual_settings":{"padding":25,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.background_color=[0.0;4];config.motion_blur_enabled=false;config.clip_cursor_to_video=true;
    let left=config.content_offset_x+config.input_offset_x;
    let top=config.content_offset_y+config.input_offset_y;
    let bounds=(left..left+config.input_width,top..top+config.input_height);
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255,255,255,255]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    let states=simulate_cursor_frames(&[CursorEvent { is_release: false,timestamp_ms:0,x:0.5,y:0.5,is_click:false}],
        &SpringConfig {tension:170.0,friction:30.0,mass:0.0},30.0,100,&None,
        settings.cursor_settings.as_ref().unwrap(),Clone::clone);
    let mut cursor=states[0].clone();cursor.opacity=1.0;cursor.rotation=0.0;
    let camera=ZoomFrameState { rect: None,scale:1.0,center_x:0.5,center_y:0.5};
    let baseline=compositor.composite_frame(&source,&camera,None).unwrap();
    let mut shapes=Vec::new();
    let alpha_sum=|pixels:&[u8]|pixels.chunks_exact(4).map(|p|p[3] as u64).sum::<u64>();
    for style in 0..5 {
        compositor.cursor_style=style as f32;cursor.opacity=1.0;cursor.rotation=0.0;
        let normal=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        assert!(alpha_sum(&normal)>1000,"style{style} must draw a visible body");
        assert!(!shapes.contains(&normal),"style{style} collapsed into a previous style");
        cursor.opacity=1.2;cursor.fade_blur_radius=-0.1;
        assert_eq!(normal,compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap(),
            "style{style} must bound spring overshoot at rendering");
        cursor.opacity=1.0;cursor.fade_blur_radius=0.0;
        cursor.rotation=35.0;
        let rotated=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        if style!=1 {assert_ne!(normal,rotated,"style{style} lost rotation");}
        cursor.rotation=0.0;cursor.opacity=0.4;
        let faded=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        assert!(alpha_sum(&faded)>0 && alpha_sum(&faded)<alpha_sum(&normal));
        cursor.opacity=-0.1;
        assert_eq!(baseline,compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap(),"style{style} leaves a body/shadow when hidden");
        shapes.push(normal);
        // Enable temporal sampling only after the static style checks.
        compositor.config.motion_blur_enabled=true;
        compositor.config.motion_blur_pan_intensity=0.0;
        compositor.config.motion_blur_zoom_intensity=0.0;
        compositor.config.motion_blur_cursor_intensity=1.0;
        cursor.opacity=1.0;cursor.x=0.7;cursor.rotation=35.0;
        compositor.reset_motion_history();
        let target=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        let mut before=cursor.clone();before.x=0.3;before.rotation=-20.0;
        compositor.reset_motion_history();
        compositor.composite_frame(&source,&camera,Some(&before)).unwrap();
        let moving=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        assert_ne!(moving,target,"style{style} does not integrate cursor movement");
        let trailing=moving.chunks_exact(4).zip(target.chunks_exact(4))
            .filter(|(a,b)|a[3]>0 && b[3]==0).count();
        assert!(trailing>20,"style{style} has no swept body coverage");
        compositor.config.motion_blur_cursor_intensity=0.0;
        compositor.composite_frame(&source,&camera,Some(&before)).unwrap();
        assert_eq!(target,compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap(),
            "style{style} ignores the disabled cursor blur control");
        compositor.config.motion_blur_cursor_intensity=1.0;
        assert_eq!(target,compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap(),"stationary cursor accumulates blur");
        compositor.composite_frame(&source,&camera,Some(&before)).unwrap();
        compositor.reset_motion_history();
        assert_eq!(target,compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap(),"cut retains cursor history");
        compositor.composite_frame(&source,&camera,Some(&before)).unwrap();
        assert_eq!(baseline,compositor.composite_frame(&source,&camera,None).unwrap());
        assert_eq!(target,compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap(),"hidden cursor retains history");
        cursor.x=1.1;
        let clipped=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        for y in 0..128 {for x in 0..128 {
            if !bounds.0.contains(&(x as u32)) || !bounds.1.contains(&(y as u32)) {
                assert_eq!(clipped[(y*128+x)*4+3],0,"style{style} swept cursor leaks beyond crop at {x},{y}");
            }
        }}
        // Wrapping through +/-180 degrees must travel two degrees, not a full turn.
        cursor.x=0.5;cursor.rotation=-179.0;compositor.reset_motion_history();
        compositor.cursor_shutter(&camera,Some(&cursor));cursor.rotation=179.0;
        let (start,end)=compositor.cursor_shutter(&camera,Some(&cursor));
        assert!((start[3]-end[3]-2.0).abs()<0.0001);
        compositor.config.motion_blur_enabled=false;
        cursor.x=0.5;

    }
}

#[test]
#[ignore="native GPU; writes static cursor rotation pixel fixtures"]
fn write_cursor_rotation_preview_fixtures() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":30},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.background_color=[0.0;4];config.motion_blur_enabled=false;
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255,255,255,255]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    let states=simulate_cursor_frames(&[CursorEvent { is_release: false,timestamp_ms:0,x:0.5,y:0.5,is_click:false}],
        &SpringConfig {tension:170.0,friction:30.0,mass:0.0},30.0,100,&None,
        settings.cursor_settings.as_ref().unwrap(),Clone::clone);
    let mut cursor=states[0].clone();cursor.opacity=1.0;
    let camera=ZoomFrameState { rect: None,scale:1.0,center_x:0.5,center_y:0.5};
    let mut fixtures=Vec::new();
    for style in 0..5 {for rotation in [-45.0,0.0,45.0,90.0] {
        compositor.cursor_style=style as f32;cursor.rotation=rotation;
        let expected=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        fixtures.push(serde_json::json!({"style":style,"rotation":rotation,"expected":expected}));
    }}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/cursor-rotation-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}

#[test]
#[ignore="native GPU; writes temporal cursor preview fixtures"]
fn write_temporal_cursor_preview_fixtures() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},"frame_rate":60,
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":30},
        "visual_settings":{"padding":25,"shadow_enabled":false,"motion_blur_enabled":true,
            "motion_blur_pan_intensity":1,"motion_blur_zoom_intensity":1,"motion_blur_cursor_intensity":1}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));config.background_color=[0.0;4];
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255,255,255,255]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    let states=simulate_cursor_frames(&[CursorEvent { is_release: false,timestamp_ms:0,x:0.5,y:0.5,is_click:false}],
        &SpringConfig {tension:170.0,friction:30.0,mass:0.0},30.0,100,&None,
        settings.cursor_settings.as_ref().unwrap(),Clone::clone);
    let mut fixtures=Vec::new();
    for radius in [0.0,3.0,10.0] {for window in [false,true] {for clip in [false,true] {for style in 0..5 {
      for strength in [0.0,0.4,1.0] {for screen_strength in [0.0,1.0] {
        compositor.config.window_mode=window;compositor.config.clip_cursor_to_video=clip;compositor.cursor_style=style as f32;
        compositor.config.motion_blur_cursor_intensity=strength;
        compositor.config.motion_blur_pan_intensity=screen_strength;
        compositor.config.motion_blur_zoom_intensity=screen_strength;
        let old_camera=ZoomFrameState{rect:None,scale:if window {1.2}else{1.0},center_x:0.3,center_y:0.5};
        let camera=ZoomFrameState{rect:None,scale:if window {1.5}else{1.0},center_x:0.7,center_y:0.5};
        let mut old=states[0].clone();old.fade_blur_radius=radius;old.x=0.1;old.y=0.6;old.opacity=0.6;old.rotation=-30.0;
        let mut cursor=old.clone();cursor.x=0.9;cursor.y=0.4;cursor.rotation=45.0;
        compositor.reset_motion_history();compositor.cursor_shutter(&old_camera,Some(&old));
        let (start,now)=compositor.cursor_shutter(&camera,Some(&cursor));
        let width=compositor.config.input_width;
        let height=compositor.config.input_height;
        let fixed=[64.0,64.0,width as f32,height as f32];
        let (clip_start,clip_now)=if window {motion::shutter_rect(&camera,Some(&old_camera),&compositor.config)}else{(fixed,fixed)};
        compositor.reset_motion_history();compositor.composite_frame(&source,&old_camera,Some(&old)).unwrap();
        let expected=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        compositor.reset_motion_history();
        let static_expected=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        fixtures.push(serde_json::json!({"width":width,"height":height,"strength":strength,"screenStrength":screen_strength,
            "radius":radius,"staticExpected":static_expected,"style":style,"clip":clip,"window":window,"before":start,"now":now,"clipBefore":clip_start,"clipNow":clip_now,"expected":expected}));
    }}}}}}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/temporal-cursor-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}

#[test]
#[ignore="native GPU; trail preview comparison fixtures"]
fn write_cursor_trail_preview_fixtures() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":30,
            "trail_enabled":true,"trail_length":15,"trail_opacity":0.5},
        "visual_settings":{"padding":25,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.1,y:0.3,is_click:false},CursorEvent{ is_release: false,timestamp_ms:1000,x:0.9,y:0.7,is_click:false}];
    let frames=simulate_cursor_frames(&events,&SpringConfig{tension:170.0,friction:30.0,mass:0.0},120.0,1000,&None,
        settings.cursor_settings.as_ref().unwrap(),Clone::clone);
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255,255,255,255]));
    let mut fixtures=Vec::new();
    for window in [false,true] {for scale in [1.0,1.5,2.0] {
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.background_color=[0.0;4];config.motion_blur_enabled=false;config.window_mode=window;
        let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
        let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
        let camera=ZoomFrameState{rect:None,scale,center_x:0.5,center_y:0.5};
        let cursor=&frames[90];
        let expected=compositor.composite_frame(&source,&camera,Some(cursor)).unwrap();
        let trail=cursor.trail_points().iter().map(|point|[point.x,point.y,point.alpha,point.size]).collect::<Vec<_>>();
        fixtures.push(serde_json::json!({"window":window,"scale":scale,"cursor":[cursor.x,cursor.y,cursor.opacity],"trail":trail,
            "frames":frames.iter().map(|frame|[frame.x,frame.y,frame.opacity]).collect::<Vec<_>>(),"expected":expected}));
    }}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/cursor-trail-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}

#[test]
#[ignore="native GPU; writes cursor fade pixel fixtures"]
fn write_cursor_fade_preview_fixtures() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":30},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.background_color=[0.0;4];config.motion_blur_enabled=false;
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255,255,255,255]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    let states=simulate_cursor_frames(&[CursorEvent { is_release: false,timestamp_ms:0,x:0.5,y:0.5,is_click:false}],
        &SpringConfig {tension:170.0,friction:30.0,mass:0.0},30.0,100,&None,
        settings.cursor_settings.as_ref().unwrap(),Clone::clone);
    let mut cursor=states[0].clone();cursor.opacity=1.0;
    let camera=ZoomFrameState { rect: None,scale:1.0,center_x:0.5,center_y:0.5};
    let mut fixtures=Vec::new();
    for scale in [0.25,0.5,1.0] {for radius in [0.0,1.0,3.0,10.0] {for style in 0..5 {for rotation in [-45.0,0.0,45.0] {
        compositor.cursor_size=scale;cursor.fade_blur_radius=radius;cursor.opacity=0.65;
        compositor.cursor_style=style as f32;cursor.rotation=rotation;
        let expected=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        fixtures.push(serde_json::json!({"scale":scale,"radius":radius,"style":style,"rotation":rotation,"expected":expected}));
    }}}}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/cursor-fade-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}

#[test]
#[ignore="native GPU; cursor fade coverage at small and large scales"]
fn cursor_fade_coverage_stays_local_across_scales() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":256,"height":256},
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":0},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(256),Some(256));
    config.background_color=[0.0;4];config.motion_blur_enabled=false;
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    compositor.cursor_style=1.0;
    let states=simulate_cursor_frames(&[CursorEvent { is_release: false,timestamp_ms:0,x:0.5,y:0.5,is_click:false}],
        &SpringConfig {tension:170.0,friction:30.0,mass:0.0},30.0,100,&None,
        settings.cursor_settings.as_ref().unwrap(),Clone::clone);
    let mut cursor=states[0].clone();cursor.opacity=1.0;
    let camera=ZoomFrameState { rect: None,scale:1.0,center_x:0.5,center_y:0.5};
    let mut rows=Vec::new();
    for scale in [0.25,0.5,1.0,3.0,5.0] {
        compositor.cursor_size=scale;
        let mut baseline=0.0;
        for radius in [0.0,3.0,10.0,10.5] {
            cursor.fade_blur_radius=radius;
            let pixels=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
            let mut mass=0.0;let mut distant=0u64;
            for (index,pixel) in pixels.chunks_exact(4).enumerate() {
                mass+=f64::from(pixel[3])/255.0;
                let x=(index%256) as f32-128.0;let y=(index/256) as f32-128.0;
                if x< -24.0*scale-1.5 || y< -24.0*scale-1.5 || x>40.0*scale+1.5 || y>40.0*scale+1.5 {
                    distant+=u64::from(pixel[3]);
                }
            }
            if radius==0.0 {baseline=mass;}
            rows.push(serde_json::json!({"scale":scale,"radius":radius,"mass":mass,"ratio":mass/baseline,"distant_alpha":distant}));
        }
    }
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/cursor-fade-coverage.json");
    std::fs::write(path,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    for row in rows {
        assert_eq!(row["distant_alpha"].as_u64().unwrap(),0,"nonlocal cursor coverage: {row}");
        let ratio=row["ratio"].as_f64().unwrap();
        assert!(ratio>0.0 && ratio<1.2,"cursor vanished or gained excessive energy: {row}");
        // Blur reaching the image boundary is clipped by its coverage mask.
        if 3.0*row["radius"].as_f64().unwrap()+1.5<=14.0*row["scale"].as_f64().unwrap() {
            assert!(ratio>0.8,"interior cursor blur loses excessive energy: {row}");
        }
    }
}

#[test]
#[ignore="native GPU; measures warmed compositor latency including output readback"]
fn measure_cursor_fade_compositor_latency() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":1920,"height":1080},
        "cursor_settings":{"enabled":true,"size":3,"color":"#ffffff","click_effect":"none","shadow_intensity":30},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(1920),Some(1080));
    config.background_color=[0.0;4];config.motion_blur_enabled=false;
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),3.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    let states=simulate_cursor_frames(&[CursorEvent { is_release: false,timestamp_ms:0,x:0.5,y:0.5,is_click:false}],
        &SpringConfig {tension:170.0,friction:30.0,mass:0.0},30.0,100,&None,
        settings.cursor_settings.as_ref().unwrap(),Clone::clone);
    let mut cursor=states[0].clone();cursor.opacity=0.65;
    let camera=ZoomFrameState { rect: None,scale:1.0,center_x:0.5,center_y:0.5};
    let guard="if(coverage<=0.0) {return;}";
    assert!(COMPOSITE_SHADER.contains(guard));
    let baseline_shader=compositor.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label:Some("Unbounded cursor benchmark baseline"),
        source:wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.replace(guard,"").into()),
    });
    let layout=compositor.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label:None,bind_group_layouts:&[Some(&compositor.bind_group_layout)],immediate_size:0,
    });
    let baseline=compositor.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label:Some("Unbounded cursor benchmark"),layout:Some(&layout),module:&baseline_shader,
        entry_point:Some("main"),compilation_options:Default::default(),cache:None,
    });
    let pipelines=[baseline,compositor.composite_pipeline.clone()];
    let radii=[0.0,3.0,10.0];let mut measurements:Vec<Vec<f64>>=vec![Vec::new();6];
    let mut expected:Vec<Vec<u8>>=vec![Vec::new();3];
    // Same device, textures and readback buffer; reverse pipeline order every
    // round so thermal drift or queue warmup cannot always favor one variant.
    for round in 0..27 {for (index,radius) in radii.into_iter().enumerate() {for offset in 0..2 {
        let variant=(round+offset)%2;
        compositor.composite_pipeline=pipelines[variant].clone();
        cursor.fade_blur_radius=radius;
        let start=std::time::Instant::now();
        let output=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        if round>=3 {measurements[index*2+variant].push(start.elapsed().as_secs_f64()*1000.0);}
        if expected[index].is_empty() {expected[index]=output;}
        else {assert_eq!(output,expected[index],"bounded cursor changes output at radius {radius}");}
    }}}
    let rows:Vec<_>=measurements.iter_mut().enumerate().map(|(index,times)| {
        times.sort_by(f64::total_cmp);
        serde_json::json!({"radius":radii[index/2],"bounded":index%2==1,
            "median_ms":(times[11]+times[12])/2.0,"p95_ms":times[22],"samples_ms":times})
    }).collect();
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/cursor-fade-paired-latency.json");
    std::fs::write(path,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

#[test]
#[ignore="native GPU; fixed pointer must preserve the selected appearance"]
fn fixed_pointer_preserves_each_cursor_appearance() {
    let mut styles=Vec::new();
    for style in ["pointer","circle","filled","outline","dotted"] {
        let mut pair=Vec::new();
        for fixed in [false,true] {
            let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
                "resolution":{"width":128,"height":128},
                "cursor_settings":{"enabled":true,"size":2,"style":style,"always_use_pointer":fixed,
                    "color":"#ffffff","click_effect":"none","shadow_intensity":30},
                "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
            })).unwrap();
            let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
            config.background_color=[0.0;4];config.motion_blur_enabled=false;
            let source=vec![0u8;128*128*4];
            let shape=RgbaImage::from_pixel(1,1,image::Rgba([255,255,255,255]));
            let mut compositor=GpuCompositor::new(config,Some(&shape),2.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
            let states=simulate_cursor_frames(&[CursorEvent{timestamp_ms:0,x:0.5,y:0.5,is_click:false,is_release:false}],
                &SpringConfig{tension:170.0,friction:30.0,mass:0.0},30.0,100,&None,
                settings.cursor_settings.as_ref().unwrap(),Clone::clone);
            let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
            pair.push(compositor.composite_frame(&source,&camera,Some(&states[0])).unwrap());
        }
        assert!(pair[0]==pair[1],"fixed-pointer flag changed {style} appearance");
        assert!(pair[0].chunks_exact(4).any(|p|p[3]>0),"{style} disappeared");
        assert!(!styles.contains(&pair[0]),"{style} collapsed into another appearance");
        styles.push(pair.remove(0));
    }
}

#[test]
#[ignore="native GPU generated cursor motion segments"]
fn generated_cursor_segments_preserve_styles_and_press_colors() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","highlight_color":"#ff6b6b","click_effect":"none","shadow_intensity":30},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.background_color=[0.0;4];config.frame_rate=60.0;
    config.motion_blur_enabled=true;config.motion_blur_cursor_intensity=1.0;
    let source=vec![0u8;128*128*4];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
    let poses=[([64.0,64.0,1.0,0.0],[64.0,64.0,1.0,0.0]),
        ([64.0,64.0,1.0,0.0],[64.5,64.0,1.0,0.0]),
        ([24.0,48.0,1.0,0.0],[88.0,48.0,1.0,0.0]),
        ([24.0,24.0,1.0,0.0],[88.0,88.0,1.0,0.0]),
        ([64.0,64.0,1.0,-60.0],[64.0,64.0,1.0,60.0]),
        ([32.0,48.0,0.6,-30.0],[80.0,64.0,1.2,45.0])];
    let mut fixtures=Vec::new();let mut styles=std::collections::HashSet::new();
    for style in 0..5 {for pressed in [false,true] {for (pose_index,(before,now)) in poses.into_iter().enumerate() {for blur in [0.0,2.0] {
        compositor.cursor_style=style as f32;compositor.prev_cursor_pose=Some(before);
        let mut cursor=crate::cursor_renderer::CursorFrameState::default();
        cursor.x=now[0]/128.0;cursor.y=now[1]/128.0;cursor.rotation=now[3];cursor.opacity=0.65;
        cursor.is_clicking=if pressed {1.0} else {0.0};cursor.press_amount=if pressed {0.1} else {0.0};
        cursor.fade_blur_radius=blur;compositor.cursor_size=now[2]/(1.0-cursor.press_amount);
        let frame=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
        assert!(frame.chunks_exact(4).any(|p|p[3]>1),"style {style} disappeared");
        assert!(frame.chunks_exact(4).all(|p|p[3]<=167),"style {style} exceeded opacity");
        if pose_index==0 && !pressed && blur==0.0 {styles.insert(frame.clone());}
        fixtures.push(serde_json::json!({"style":style,"pressed":pressed,"scale":now[2],"blur":blur,
            "rotation":now[3],"x":cursor.x,"y":cursor.y,"opacity":cursor.opacity,
            "before":before,"now":now,"expected":frame}));
    }}}}
    assert_eq!(styles.len(),5,"motion must preserve the five cursor appearances");
    if let Ok(path)=std::env::var("TARANTINO_GENERATED_CURSOR_SEGMENTS_FIXTURE") {
        std::fs::write(path,serde_json::to_vec(&serde_json::json!({"data":null,"frames":fixtures})).unwrap()).unwrap();
    }
}

#[test]
#[ignore="native GPU cursor integration timing"]
fn measure_cursor_integration_frame_cost() {
    let mut results=Vec::new();
    for (width,height) in [(1280,720),(1920,1080)] {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":width,"height":height},
            "cursor_settings":{"enabled":true,"size":3,"color":"#ffffff","click_effect":"none","shadow_intensity":30},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(width),Some(height));
        config.background_color=[0.0;4];config.frame_rate=60.0;
        config.motion_blur_enabled=true;config.motion_blur_cursor_intensity=1.0;
        let source=vec![0u8;(width*height*4) as usize];
        let shape=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
        let mut compositor=GpuCompositor::new(config,Some(&shape),3.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let mut cursor=crate::cursor_renderer::CursorFrameState::default();
        cursor.x=0.5;cursor.y=0.5;cursor.opacity=1.0;
        for style in [0.0,3.0] {for travel in [24.0,160.0] {
            compositor.cursor_style=style;
            let mut samples=[Vec::new(),Vec::new()];
            // Alternate order to distribute warm-cache and scheduling effects.
            // The entire cursor sweep lies inside the video, so clipping selects
            // the point-integrated control without cutting away cursor pixels.
            for iteration in 0..36 {for offset in 0..2 {
                let mode=(iteration+offset)%2;
                compositor.config.clip_cursor_to_video=mode==1;
                compositor.prev_cursor_pose=Some([width as f32*0.5-travel,height as f32*0.5,3.0,0.0]);
                let start=std::time::Instant::now();
                let frame=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
                let elapsed=start.elapsed().as_secs_f64()*1000.0;
                assert_eq!(frame.len(),(width*height*4) as usize);
                if iteration>=6 {samples[mode].push(elapsed);}
            }}
            for (mode,mut timings) in samples.into_iter().enumerate() {
                timings.sort_by(f64::total_cmp);
                results.push(serde_json::json!({"width":width,"height":height,"style":style,"travel":travel,
                    "path":if mode==0 {"segments"} else {"points"},"samples_ms":timings,
                    "median_ms":(timings[14]+timings[15])*0.5,"p95_ms":timings[28]}));
            }
        }}
    }
    if let Ok(path)=std::env::var("TARANTINO_CURSOR_TIMING_FIXTURE") {
        std::fs::write(path,serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    }
    eprintln!("{}",serde_json::to_string(&results).unwrap());
}

#[test]
#[ignore="native GPU cursor motion across source crop boundaries"]
fn source_crop_preserves_cursor_motion_into_padding() {
    for mode in ["display","window"] {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},"capture_mode":mode,
            "source_crop":{"x":0.25,"y":0.25,"width":0.5,"height":0.5},
            "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":0},
            "visual_settings":{"padding":25,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(64),Some(64));
        config.background_color=[0.0;4];
        config.motion_blur_enabled=true;config.motion_blur_cursor_intensity=1.0;
        config.motion_blur_pan_intensity=0.0;config.motion_blur_zoom_intensity=0.0;
        let left=config.content_offset_x+config.input_offset_x;
        let right=left+config.input_width;
        let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
        let shape=RgbaImage::from_pixel(1,1,image::Rgba([255;4]));
        let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let mut cursor=crate::cursor_renderer::CursorFrameState::default();
        cursor.opacity=1.0;cursor.y=0.5;
        for x in [-0.1,1.05] {
            cursor.x=x;compositor.reset_motion_history();
            let stationary=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
            let outside_alpha=|frame:&[u8]|frame.chunks_exact(4).enumerate()
                .filter(|(i,_)|{let x=(*i%128) as u32;x<left||x>=right})
                .map(|(_,p)|p[3] as u64).sum::<u64>();
            assert!(outside_alpha(&stationary)>100,"{mode} hides stationary cursor beyond crop at {x}");
            cursor.x=0.5;compositor.reset_motion_history();
            compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
            cursor.x=x;
            let moving=compositor.composite_frame(&source,&camera,Some(&cursor)).unwrap();
            assert!(outside_alpha(&moving)>100,"{mode} clips moving cursor at {x}");
        }
    }
}

#[test]
#[ignore="native GPU; cursor and click alignment at display zoom boundaries"]
fn display_cursor_projection_renders_like_viewport_coordinates() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":256,"height":256},"capture_mode":"display",
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"ripple","trail_enabled":true},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(256),Some(256));
    config.motion_blur_enabled=false;
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let shape=RgbaImage::from_pixel(1,1,image::Rgba([255,255,255,255]));
    let mut compositor=GpuCompositor::new(config,Some(&shape),1.0,[1.0;3],settings.cursor_settings.as_ref()).unwrap();
    let spring=SpringConfig{tension:170.0,friction:30.0,mass:0.0};
    let identity=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
    for (scale,cx,cy) in [(1.0005,0.4998,0.5002),(1.001,0.5,0.5),(1.2,0.1,0.9),(2.0,0.1,0.9)] {
        let camera=ZoomFrameState{rect:None,scale,center_x:cx,center_y:cy};
        let events=[CursorEvent{is_release:false,timestamp_ms:0,x:0.2,y:0.8,is_click:true}];
        let actual=simulate_cursor_frames(&events,&spring,120.0,200,&Some(vec![camera.clone();24]),
            settings.cursor_settings.as_ref().unwrap(),Clone::clone);
        let [x,y]=if scale<=1.001 {[0.2,0.8]} else {
            let half=0.5/scale;
            [(0.2-cx.clamp(half,1.0-half))*scale+0.5,(0.8-cy.clamp(half,1.0-half))*scale+0.5]
        };
        let expected=simulate_cursor_frames(&[CursorEvent{is_release:false,timestamp_ms:0,x,y,is_click:true}],
            &spring,120.0,200,&None,settings.cursor_settings.as_ref().unwrap(),Clone::clone);
        for index in [0,6,18] {
            compositor.reset_motion_history();
            let a=compositor.composite_frame(&source,&camera,Some(&actual[index])).unwrap();
            compositor.reset_motion_history();
            let b=compositor.composite_frame(&source,&identity,Some(&expected[index])).unwrap();
            assert_eq!(a,b,"scale {scale}, frame {index}");
            let blank=compositor.composite_frame(&source,&identity,None).unwrap();
            assert_ne!(a,blank,"fixture must draw the cursor");
        }
    }
}
