use super::{run,strings,verify,ExportSettings};
use crate::video_processing::{export::export_video,types::VideoInfo};

#[tokio::test]
#[ignore="requires local FFmpeg and GPU; decoded camera silence visibility"]
async fn camera_silence_export_follows_cuts_rates_and_fullscreen_override(){
    let root=std::env::temp_dir().join(format!("camera-silence-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    verify(&root);
    let input=root.join("colors.mkv");
    let mic=input.with_extension("mic.wav");
    run(&strings(&["-v","error","-f","lavfi","-i",
        "aevalsrc=if(lt(n\\,2400)\\,0\\,if(lt(n\\,8000)\\,0.5\\,0)):s=8000:d=2",
        "-c:a","pcm_f32le",mic.to_str().unwrap()]));
    run(&strings(&["-v","error","-i",input.to_str().unwrap(),"-vf","format=rgb24,negate,format=yuv420p","-c:v","libx264",
        input.with_extension("webcam.mp4").to_str().unwrap()]));
    let silence=crate::commands::video::silence::detect_audio_silence(mic.to_string_lossy().into_owned(),100.0,0.1).await.unwrap();
    assert_eq!(silence,vec![
        crate::video_processing::webcam_timeline::SilenceRange{start_ms:0.0,end_ms:300.0},
        crate::video_processing::webcam_timeline::SilenceRange{start_ms:1000.0,end_ms:2000.0},
    ]);
    let output=root.join("camera-silence.mp4");
    let mut settings:ExportSettings=serde_json::from_value(serde_json::json!({
        "output_path":output,"resolution":{"width":64,"height":64},"source_width":16,"source_height":16,
        "frame_rate":60,"format":"mp4","codec":"h264","cursor_settings":{"enabled":false},"zoom_blocks":[],
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
        "webcam_x":0.75,"webcam_y":0.75,"webcam_size":0.25,"webcam_shape":"circle",
        "timeline_clips":[
            {"id":"fast","startTime":0,"duration":500,"sourceIn":1000,"sourceOut":2000,"playbackRate":2,"trackOrder":0,"muted":false},
            {"id":"slow","startTime":750,"duration":1000,"sourceIn":0,"sourceOut":500,"playbackRate":0.5,"trackOrder":0,"muted":false}
        ],
        "webcam_keyframes":[
            {"id":"full","time":1500,"x":0.75,"y":0.75,"size":0.25,"shape":"circle","visible":true,"layout":"fullscreen"},
            {"id":"overlay","time":1750,"x":0.75,"y":0.75,"size":0.25,"shape":"circle","visible":true,"layout":"overlay"}
        ]
    })).unwrap();
    settings.webcam_silence_ranges=Some(silence);
    let info=VideoInfo{duration_ms:2000,width:16,height:16,fps:30.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(60)};
    let reference=super::camera_reference::render(&root,&input,&settings,&info).await;
    export_video(&root,&input,settings.clone(),&info,None).await.unwrap();
    let frames=run(&strings(&["-v","error","-i",output.to_str().unwrap(),"-map","0:v","-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgba","-"]));
    assert_eq!(frames.len(),105*64*64*4);
    for (i,frame) in frames.chunks_exact(64*64*4).enumerate(){
        super::camera_reference::compare(&reference,frame,i);
    }

    // A global hide must also suppress timed fullscreen states, including on cuts.
    settings.webcam_enabled=Some(false);
    let disabled=root.join("camera-disabled.mp4");
    settings.output_path=Some(disabled.to_string_lossy().into_owned());
    export_video(&root,&input,settings,&info,None).await.unwrap();
    let frames=run(&strings(&["-v","error","-i",disabled.to_str().unwrap(),"-map","0:v","-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgba","-"]));
    assert_eq!(frames.len(),105*64*64*4);
    for (i,frame) in frames.chunks_exact(64*64*4).enumerate(){
        for (x,y) in [(48,48),(5,5)] {
            let pixel=&frame[(y*64+x)*4..][..3];
            match i {
                0..=29=>assert!(pixel[2]>180 && pixel[0]<40,"disabled fast frame {i}: {pixel:?}"),
                30..=44=>assert!(pixel.iter().all(|v|*v<15),"disabled gap frame {i}"),
                _=>assert!(pixel[0]>180 && pixel[2]<40,"disabled slow frame {i}: {pixel:?}"),
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
