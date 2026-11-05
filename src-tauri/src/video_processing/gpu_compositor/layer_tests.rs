use super::*;
use crate::video_processing::loupe::LoupeTarget;

#[test]
#[ignore="native GPU; actual padded window compositor layer integration"]
fn glass_layers_preserve_window_mask_backdrop_and_cached_frame_updates() {
    for background in ["#204080ff","#20408080"] {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},"source_type":"window",
            "visual_settings":{"padding":20,"corner_radius":12,"shadow_enabled":true,
                "shadow_blur":8,"shadow_intensity":35,"background_color":background}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.motion_blur_enabled=false;
        // Color UI currently resolves hex to opaque; exercise the compositor's
        // alpha contract directly rather than mistaking that for alpha coverage.
        config.background_color[3]=if background=="#20408080" {128.0/255.0} else {1.0};
        let (iw,ih)=(config.input_width,config.input_height);
        let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        assert_eq!(compositor.loupe_layers.foreground.width(),1,"ordinary exports must not allocate full layers");
        for scale in [1.0,1.6] {for red in [100u8,220] {
            let source=[red,30,10,128].repeat((iw*ih) as usize);
            let camera=ZoomFrameState { rect: None,scale,center_x:0.5,center_y:0.5};
            compositor.set_loupe_target(None,(128,128),None,&camera).unwrap();
            let baseline=compositor.composite_frame(&source,&camera,None).unwrap();
            let target=LoupeTarget {center_x:0.1,center_y:0.5,radius:0.3,zoom:2.0,presence:1.0,
                bevel:0.2,chromatic_aberration:0.5,shadow_alpha:0.35};
            compositor.set_loupe_target(Some(target),(128,128),None,&camera).unwrap();
            let glass=compositor.composite_frame(&source,&camera,None).unwrap();
            assert!(compositor.loupe_layered_pass.is_some());
            let front=read_texture(&compositor,&compositor.loupe_layers.foreground);
            let back=read_texture(&compositor,&compositor.loupe_layers.background);
            assert_eq!(back[3],if background=="#20408080" {128} else {255});
            let mut transparent=0;let mut covered=0;
            for (index,(f,b)) in front.chunks_exact(4).zip(back.chunks_exact(4)).enumerate() {
                assert_eq!(b,&back[..4],"screen or shadow leaked into solid backdrop");
                assert!(f[..3].iter().all(|c|*c<=f[3]),"foreground must be premultiplied");
                if f[3]==0 {transparent+=1;} else {covered+=1;}
                let fa=f[3] as f64/255.0;let ba=b[3] as f64/255.0;
                let alpha=fa+ba*(1.0-fa);
                for c in 0..4 {
                    let expected=if c==3 {alpha*255.0} else if alpha>0.0 {
                        (f[c] as f64+b[c] as f64*(1.0-fa))/alpha
                    } else {0.0};
                    assert!((baseline[index*4+c] as f64-expected).abs()<=2.5,
                        "layer reconstruction index{index} channel{c}: {} vs {expected}",baseline[index*4+c]);
                }
            }
            assert!(transparent>100 && covered>100,"fixture needs a real transparent rounded/padded boundary");
            assert_ne!(glass,baseline,"glass must affect the composed window");
            let repeated=compositor.composite_frame(&source,&camera,None).unwrap();
            assert_eq!(glass,repeated,"cached layers must not accumulate the previous image");
            compositor.set_loupe_target(None,(128,128),None,&camera).unwrap();
            assert_eq!(baseline,compositor.composite_frame(&source,&camera,None).unwrap(),"clearing lens must restore straight RGBA output");
        }}
    }
}

pub(super) fn read_texture(compositor:&GpuCompositor,texture:&wgpu::Texture)->Vec<u8> {
    let (width,height)=(texture.width(),texture.height());
    let stride=(width*4).div_ceil(256)*256;
    let buffer=compositor.device.create_buffer(&wgpu::BufferDescriptor {
        label:None,size:(stride*height) as u64,usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::MAP_READ,mapped_at_creation:false,
    });
    let mut encoder=compositor.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo {texture,mip_level:0,
        origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},wgpu::TexelCopyBufferInfo {
            buffer:&buffer,layout:wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(stride),rows_per_image:Some(height)}},texture.size());
    compositor.queue.submit([encoder.finish()]);
    let slice=buffer.slice(..);let (tx,rx)=std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read,move |r|tx.send(r).unwrap());
    compositor.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
    let bytes=slice.get_mapped_range().unwrap();
    let result=bytes.chunks_exact(stride as usize).flat_map(|row|row[..(width*4) as usize].iter().copied()).collect();
    drop(bytes);buffer.unmap();result
}

#[test]
#[ignore="native GPU; camera alpha compositing"]
fn camera_opacity_blends_without_replacing_cached_texture() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":64,"height":64},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(64),Some(64));
    config.motion_blur_enabled=false;
    config.webcam=Some(WebcamConfig{pos_x:0.5,pos_y:0.5,size:0.5,shape:"roundrect".into()});
    let source=[0,0,0,255].repeat((config.input_width*config.input_height) as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    compositor.set_webcam_texture(8,8);
    compositor.upload_webcam_frame(&[200,80,40,255].repeat(64),8,8);
    let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
    for alpha in [1.0,0.5,0.1,0.0,0.75,1.0] {
        compositor.set_webcam_opacity(alpha);
        let pixels=compositor.composite_frame(&source,&camera,None).unwrap();
        let center=&pixels[(32*64+32)*4..(32*64+32)*4+4];
        for (actual,full) in center[..3].iter().zip([200.0,80.0,40.0]) {
            assert!((*actual as f32-full*alpha).abs()<=1.0,"alpha {alpha}: {center:?}");
        }
        assert_eq!(center[3],255);
        assert_eq!(&pixels[..4],&[0,0,0,255]);
    }
}

#[test]
#[ignore="native GPU; separate camera contribution inside lens"]
fn magnifier_excludes_camera_from_content_without_hiding_stage_camera() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":64,"height":64},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(64),Some(64));
    config.motion_blur_enabled=false;
    config.webcam=Some(WebcamConfig{pos_x:0.5,pos_y:0.5,size:0.8,shape:"roundrect".into()});
    let source=[0,40,100,255].repeat((config.input_width*config.input_height) as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    compositor.set_webcam_texture(8,8);
    compositor.upload_webcam_frame(&[200,0,0,255].repeat(64),8,8);
    let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
    let target=LoupeTarget{center_x:0.5,center_y:0.5,radius:0.15,zoom:2.0,presence:1.0,
        bevel:0.0,chromatic_aberration:0.0,shadow_alpha:0.0};
    compositor.set_loupe_target(Some(target),(64,64),None,&camera).unwrap();
    let output=compositor.composite_frame(&source,&camera,None).unwrap();
    let center=(32*64+32)*4;
    assert_eq!(&output[center..center+4],&[0,40,100,255],"lens should show screen behind camera");
    let outside=(32*64+12)*4;
    assert_eq!(&output[outside..outside+4],&[200,0,0,255],"stage camera remains outside lens");
    let front=read_texture(&compositor,&compositor.loupe_layers.foreground);
    assert_eq!(&front[center..center+4],&[0,40,100,255]);
    compositor.set_loupe_target(Some(LoupeTarget{presence:0.5,..target}),(64,64),None,&camera).unwrap();
    compositor.composite_frame(&source,&camera,None).unwrap();
    let front=read_texture(&compositor,&compositor.loupe_layers.foreground);
    for (actual,expected) in front[center..center+4].iter().zip([100,20,50,255]) {
        assert!((*actual as i32-expected).abs()<=1,"half-presence camera factor: {:?}",&front[center..center+4]);
    }
    compositor.set_loupe_target(None,(64,64),None,&camera).unwrap();
    let restored=compositor.composite_frame(&source,&camera,None).unwrap();
    assert_eq!(&restored[center..center+4],&[200,0,0,255]);
}

#[test]
#[ignore="native GPU; writes camera preview comparison fixtures"]
fn write_camera_layer_preview_fixtures() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":64,"height":48},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"}
    })).unwrap();
    let mut fixtures=Vec::new();
    for (sw,sh) in [(16,8),(8,16)] {for shape in ["circle","roundrect","fullscreen"] {for opacity in [0.25,1.0] {
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(64),Some(48));
        config.motion_blur_enabled=false;
        config.webcam=Some(WebcamConfig{pos_x:0.5,pos_y:0.5,size:0.5,shape:shape.into()});
        let source=[0,0,0,255].repeat((config.input_width*config.input_height) as usize);
        let texture:Vec<u8>=(0..sh).flat_map(|y|(0..sw).flat_map(move|x|
            [(x*255/(sw-1)) as u8,(y*255/(sh-1)) as u8,if x<sw/2 {30}else{220},if y<sh/2 {128}else{255}])).collect();
        let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        compositor.set_webcam_texture(sw,sh);compositor.upload_webcam_frame(&texture,sw,sh);
        compositor.set_webcam_opacity(opacity);
        let expected=compositor.composite_frame(&source,&ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5},None).unwrap();
        fixtures.push(serde_json::json!({"width":64,"height":48,"sourceWidth":sw,"sourceHeight":sh,
            "texture":texture,"shape":shape,"opacity":opacity,
            "rect":if shape=="fullscreen"{[0,0,64,48]}else{[16,8,32,32]},"expected":expected}));
    }}}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oss-research/camera-layer-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}

#[test]
#[ignore="native GPU; camera horizontal mirroring without moving the overlay"]
fn camera_mirror_swaps_content_without_moving_mask() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":64,"height":64},"webcam_mirrored":true,
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(64),Some(64));
    assert!(config.webcam_mirrored);
    config.motion_blur_enabled=false;
    config.webcam=Some(WebcamConfig{pos_x:0.5,pos_y:0.5,size:0.5,shape:"roundrect".into()});
    let source=[0,0,0,255].repeat((config.input_width*config.input_height) as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    let texture:Vec<u8>=(0..64).flat_map(|i|if i%8<4 {[255,0,0,255]} else {[0,0,255,255]}).collect();
    compositor.set_webcam_texture(8,8);
    compositor.upload_webcam_frame(&texture,8,8);
    let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
    for mirrored in [false,true,false] {
        compositor.config.webcam_mirrored=mirrored;
        let pixels=compositor.composite_frame(&source,&camera,None).unwrap();
        for (x,blue) in [(24,mirrored),(40,!mirrored)] {
            let p=&pixels[(32*64+x)*4..(32*64+x)*4+3];
            assert_eq!(p,if blue {&[0,0,255]} else {&[255,0,0]});
        }
        assert_eq!(&pixels[(10*64+10)*4..(10*64+10)*4+3],&[0,0,0]);
    }
}

#[test]
#[ignore="native GPU; animated camera rectangle geometry and texture reuse"]
fn camera_rectangles_resize_translate_and_restore_without_stale_pixels() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":64,"height":48},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(64),Some(48));
    config.motion_blur_enabled=false;
    config.webcam=Some(WebcamConfig{pos_x:0.5,pos_y:0.5,size:1.0,shape:"fullscreen".into()});
    let source=[0,0,0,255].repeat((config.input_width*config.input_height) as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    compositor.set_webcam_texture(8,8);
    compositor.upload_webcam_frame(&[200,80,40,255].repeat(64),8,8);
    let zoom=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
    for rect in [[10.0,12.0,32.0,16.0],[4.0,3.0,16.0,40.0],[-8.0,4.0,24.0,20.0],
        [0.0,0.0,64.0,48.0],[12.0,12.0,0.0,0.0],[10.0,12.0,32.0,16.0]] {
        compositor.set_webcam_rect(Some(rect)).unwrap();
        let output=compositor.composite_frame(&source,&zoom,None).unwrap();
        for y in 0..48 {for x in 0..64 {
            let inside=x as f32>=rect[0] && (x as f32)<rect[0]+rect[2]
                && y as f32>=rect[1] && (y as f32)<rect[1]+rect[3];
            assert_eq!(&output[(y*64+x)*4..(y*64+x)*4+4],
                if inside {&[200,80,40,255]} else {&[0,0,0,255]},"rect {rect:?}, pixel {x},{y}");
        }}
    }
    assert!(compositor.set_webcam_rect(Some([0.0,0.0,f32::NAN,10.0])).is_err());
    compositor.set_webcam_rect(None).unwrap();
    let restored=compositor.composite_frame(&source,&zoom,None).unwrap();
    assert!(restored.chunks_exact(4).all(|p|p==[200,80,40,255]));
}
