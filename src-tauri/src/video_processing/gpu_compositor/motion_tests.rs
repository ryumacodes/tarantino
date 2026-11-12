use super::*;

#[test]
#[ignore="native GPU; explicit camera rectangle must preserve independent dimensions"]
fn explicit_camera_rectangle_reaches_export_pixels() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},"capture_mode":"window",
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.motion_blur_enabled=false;
    let source=[255u8;4].repeat((config.input_width*config.input_height) as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    let camera=ZoomFrameState {scale:3.0,center_x:0.9,center_y:0.9,
        rect:Some([0.125,0.25,0.5,0.25])};
    let pixels=compositor.composite_frame(&source,&camera,None).unwrap();
    let white=|x:usize,y:usize| pixels[(y*128+x)*4..(y*128+x)*4+3].iter().all(|&v|v>240);
    let row:Vec<_>=(0..128).filter(|&x|white(x,48)).collect();
    let column:Vec<_>=(0..128).filter(|&y|white(48,y)).collect();
    assert!((63..=65).contains(&row.len()),"horizontal extent {row:?}");
    assert!((31..=33).contains(&column.len()),"vertical extent {column:?}");
    assert!((15..=17).contains(row.first().unwrap()));
    assert!((31..=33).contains(column.first().unwrap()));
}

#[test]
#[ignore="native GPU; motion blur must affect exported pixels and reset at cuts"]
fn window_zoom_blur_reaches_export_and_resets_at_cuts() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},"capture_mode":"window","frame_rate":60,
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.window_mode=true;
    config.motion_blur_enabled=true;
    config.motion_blur_pan_intensity=0.0;
    config.motion_blur_zoom_intensity=1.0;
    let (w,h)=(config.input_width,config.input_height);
    let source:Vec<u8>=(0..h).flat_map(|y|(0..w).flat_map(move |x| {
        let c=if (x+y)%2==0 {0} else {255};[c,255-c,80,255]
    })).collect();
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    let before=ZoomFrameState { rect: None,scale:1.9,center_x:0.5,center_y:0.5};
    let now=ZoomFrameState { rect: None,scale:2.0,center_x:0.5,center_y:0.5};
    let baseline=compositor.composite_frame(&source,&now,None).unwrap();
    compositor.reset_motion_history();
    compositor.composite_frame(&source,&before,None).unwrap();
    let moving=compositor.composite_frame(&source,&now,None).unwrap();
    assert_ne!(baseline,moving,"zoom-only window motion must reach exported pixels");
    assert_eq!(baseline,compositor.composite_frame(&source,&now,None).unwrap(),"stationary frame must not accumulate blur");
    compositor.composite_frame(&source,&before,None).unwrap();
    compositor.reset_motion_history();
    assert_eq!(baseline,compositor.composite_frame(&source,&now,None).unwrap(),"edit boundary must discard previous camera velocity");
}

#[test]
#[ignore="native GPU; temporal window silhouette must extend through the swept edge"]
fn moving_window_edges_have_temporal_coverage_and_keep_straight_color() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},"frame_rate":60,
        "visual_settings":{"padding":25,"corner_radius":8,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.window_mode=true;config.background_color=[0.0;4];
    config.motion_blur_enabled=true;config.motion_blur_pan_intensity=1.0;config.motion_blur_zoom_intensity=1.0;
    let source=[255u8,255,255,255].repeat((config.input_width*config.input_height) as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    for (from,to) in [(0.3,0.7),(0.7,0.3)] {
        let before=ZoomFrameState { rect: None,scale:1.4,center_x:from,center_y:0.5};
        let now=ZoomFrameState { rect: None,scale:1.4,center_x:to,center_y:0.5};
        compositor.reset_motion_history();
        let stationary=compositor.composite_frame(&source,&now,None).unwrap();
        compositor.reset_motion_history();
        compositor.composite_frame(&source,&before,None).unwrap();
        let moving=compositor.composite_frame(&source,&now,None).unwrap();
        let mut outside=0;let mut softened=0;
        // Mid-row avoids corners and tests the actual moving vertical boundaries.
        for x in 0..128 {let i=(64*128+x)*4;
            if stationary[i+3]==0 && moving[i+3]>0 {outside+=1;}
            if stationary[i+3]==255 && moving[i+3]>0 && moving[i+3]<255 {softened+=1;}
            if moving[i+3]>0 {assert_eq!(&moving[i..i+3],&[255,255,255],"coverage must not darken straight RGB");}
        }
        assert!(outside>=5 && softened>=5,"expected a swept silhouette, got {outside} trailing and {softened} leading pixels");
        assert_eq!(&moving[..4],&[0,0,0,0],"far background remains transparent");
        assert_eq!(&moving[(64*128+64)*4..(64*128+64)*4+4],&[255,255,255,255],"interior remains fully covered");
        assert_eq!(stationary,compositor.composite_frame(&source,&now,None).unwrap(),"stationary path remains unchanged");
    }
}

#[test]
#[ignore="native GPU; shadow is sampled in temporal geometry, not the latest pose"]
fn temporal_shadow_follows_segment_geometry_without_double_composition() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},"frame_rate":60,
        "visual_settings":{"padding":50,"corner_radius":8,"shadow_enabled":true}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.window_mode=true;config.background_color=[0.0;4];
    config.shadow_enabled=true;config.shadow_blur=8.0;config.shadow_intensity=0.65;config.shadow_offset_y=20.0;
    config.motion_blur_enabled=true;config.motion_blur_pan_intensity=1.0;config.motion_blur_zoom_intensity=1.0;
    assert_eq!((config.input_width,config.input_height),(64,64));
    assert!(64.0+config.input_height as f64*1.4/2.0<118.0);
    let source=vec![0u8;(config.input_width*config.input_height*4) as usize];
    let opaque=[255u8,255,255,255].repeat((config.input_width*config.input_height) as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    let before=ZoomFrameState { rect: None,scale:1.4,center_x:0.3,center_y:0.5};
    let middle=ZoomFrameState { rect: None,scale:1.4,center_x:0.5,center_y:0.5};
    let now=ZoomFrameState { rect: None,scale:1.4,center_x:0.7,center_y:0.5};
    let reference=compositor.composite_frame(&source,&middle,None).unwrap();
    compositor.reset_motion_history();
    let latest=compositor.composite_frame(&source,&now,None).unwrap();
    compositor.reset_motion_history();
    compositor.composite_frame(&source,&before,None).unwrap();
    let temporal=compositor.composite_frame(&source,&now,None).unwrap();
    let mut visible=0;let mut different=0;
    // Below both silhouettes the distance has no crossings, so one temporal
    // segment must sample its midpoint shadow exactly once.
    for x in 0..128 {let i=(118*128+x)*4;
        assert!(temporal[i+3].abs_diff(reference[i+3])<=1,"midpoint shadow mismatch at {x}: {} vs {}",temporal[i+3],reference[i+3]);
        assert_eq!(&temporal[i..i+3],&[0,0,0]);
        if reference[i+3]>0 {visible+=1;}
        if temporal[i+3].abs_diff(latest[i+3])>2 {different+=1;}
    }
    assert!(visible>20 && different>10,"fixture must distinguish the temporal shadow from latest-frame shadow");
    compositor.reset_motion_history();
    compositor.composite_frame(&opaque,&before,None).unwrap();
    let covered=compositor.composite_frame(&opaque,&now,None).unwrap();
    assert_eq!(&covered[(64*128+64)*4..(64*128+64)*4+4],&[255,255,255,255],"shadow must stay behind opaque content");
    assert_eq!(latest,compositor.composite_frame(&source,&now,None).unwrap(),"stationary shadow returns to exact baseline");
}

#[test]
#[ignore="native GPU; display blur must sample earlier camera poses inside a fixed clip"]
fn display_pan_samples_the_trailing_camera_without_blurring_the_frame() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":160,"height":160},"capture_mode":"display","frame_rate":60,
        "visual_settings":{"padding":12.5,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    assert_eq!((config.input_width,config.input_height),(128,128));
    config.background_color=[0.0;4];config.motion_blur_enabled=true;
    config.motion_blur_pan_intensity=0.2;config.motion_blur_zoom_intensity=0.0;
    let source:Vec<u8>=(0..128).flat_map(|_|(0..128).flat_map(|x|[x*2,0,0,255])).collect();
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    let before=ZoomFrameState{rect:Some([-0.1,-0.5,2.0,2.0]),scale:2.0,center_x:0.3,center_y:0.5};
    let now=ZoomFrameState{rect:Some([-0.9,-0.5,2.0,2.0]),scale:2.0,center_x:0.7,center_y:0.5};
    let still=compositor.composite_frame(&source,&now,None).unwrap();
    compositor.reset_motion_history();
    compositor.composite_frame(&source,&before,None).unwrap();
    let moving=compositor.composite_frame(&source,&now,None).unwrap();
    let center=(80*160+80)*4;
    // Six stratified samples over source x=.62..=.70, using the output-pixel phase.
    let expected = [
        [166, 169, 169, 171, 170, 173, 173, 176],
        [169, 168, 171, 171, 173, 172, 175, 175],
        [167, 170, 169, 171, 171, 174, 173, 175],
        [170, 169, 171, 170, 174, 173, 175, 175],
        [167, 169, 169, 172, 170, 173, 173, 176],
        [169, 168, 171, 171, 173, 172, 175, 175],
        [167, 170, 169, 171, 171, 174, 173, 175],
        [170, 169, 171, 171, 174, 173, 175, 174],
    ];
    for (y,row) in expected.iter().enumerate() {for (x,&red) in row.iter().enumerate() {
        let actual=moving[((80+y)*160+80+x)*4];
        assert!((actual as i32-red).abs()<=1,"dithered sample {x},{y}: {actual} != {red}");
    }}
    assert!(still[center]>moving[center]+7);
    assert_eq!(moving[(80*160+15)*4+3],0);
    assert_eq!(moving[(80*160+16)*4+3],255);
    compositor.reset_motion_history();
    assert_eq!(still,compositor.composite_frame(&source,&now,None).unwrap());

    let stripes:Vec<u8>=(0..128).flat_map(|_|(0..128).flat_map(|x|[if x%2==0 {0} else {255},0,0,255])).collect();
    let slow=ZoomFrameState{rect:Some([-0.12,-0.5,2.0,2.0]),scale:2.0,center_x:0.31,center_y:0.5};
    compositor.reset_motion_history();
    compositor.composite_frame(&stripes,&before,None).unwrap();
    let subtle=compositor.composite_frame(&stripes,&slow,None).unwrap();
    // Half a display pixel of travel: sample source u=.309, between texels 39 and 40.
    for y in 80..88 {
        assert!((subtle[(y*160+80)*4] as i32-242).abs()<=1,"slow pan must not carry a dither grid");
    }
}

#[test]
#[ignore="native GPU; shutter footprints filter detail between temporal taps"]
fn screen_motion_filters_between_taps_and_restores_static_detail() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},"frame_rate":60,
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let source:Vec<u8>=(0..512*512).flat_map(|i| {
        let value=if (i%512+i/512)%2==0 {0} else {255};[value,value,value,255]
    }).collect();
    for window in [false,true] {
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.source_size=[512,512];config.window_mode=window;
        config.motion_blur_enabled=true;config.motion_blur_pan_intensity=1.0;
        config.motion_blur_zoom_intensity=0.0;
        let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        let before=ZoomFrameState{rect:Some([-1.00390625,-1.00390625,4.0,4.0]),scale:4.0,center_x:0.3759765625,center_y:0.3759765625};
        let now=ZoomFrameState{rect:Some([-2.00390625,-1.00390625,4.0,4.0]),center_x:0.6259765625,..before.clone()};
        let still=compositor.composite_frame(&source,&now,None).unwrap();
        compositor.reset_motion_history();
        compositor.composite_frame(&source,&before,None).unwrap();
        let moving=compositor.composite_frame(&source,&now,None).unwrap();
        for y in 32..96 {for x in 32..96 {
            let pixel=&moving[(y*128+x)*4..][..4];
            assert!(pixel[..3].iter().all(|v|v.abs_diff(128)<=2),"window={window}, ({x},{y}): {pixel:?}");
            assert_eq!(pixel[3],255);
        }}
        assert!(still!=moving,"fixture must distinguish static detail from motion filtering");
        assert_eq!(still,compositor.composite_frame(&source,&now,None).unwrap());
    }
}
