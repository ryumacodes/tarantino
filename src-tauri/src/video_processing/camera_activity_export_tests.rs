use super::{export::export_video,types::{ExportSettings,VideoInfo}};
use std::process::Command;
fn ffmpeg(args:&[&str])->Vec<u8> {
    let output=Command::new(crate::ffmpeg::binary("ffmpeg")).args(args).output().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));output.stdout
}

#[tokio::test]
#[ignore="requires FFmpeg and native GPU; edited camera activity through encoded export"]
async fn encoded_camera_targets_respect_trim_speed_and_loop_bounds() {
    let root=std::env::temp_dir().join(format!("camera-window-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("source.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i",
        "color=red:s=128x128:r=60:d=1,drawbox=x=42:y=0:w=43:h=128:color=lime:t=fill,drawbox=x=85:y=0:w=43:h=128:color=blue:t=fill",
        "-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),serde_json::json!({
        "display_width":100,"display_height":100,"mouse_events":[
            {"timestamp":0,"x":10,"y":50},{"timestamp":300,"x":55,"y":50},
            {"timestamp":800,"x":90,"y":50,"event_type":{"ButtonRelease":"Left"}}]}).to_string()).unwrap();
    let info=VideoInfo{duration_ms:1000,width:128,height:128,fps:60.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(60)};
    for (mode,center,time,channel) in [("full",0.9,"0.55",2),("trim",0.55,"0.35",1),
        ("speed",0.55,"0.175",1),("loop",0.1,"0.55",0)] {
        let mut frames=Vec::new();
        for manual in [false,true] {
            let output=root.join(format!("{mode}-{manual}.mp4"));
            let mut settings:ExportSettings=serde_json::from_value(serde_json::json!({
                "output_path":output,"format":"mp4","codec":"h264","frame_rate":60,
                "resolution":{"width":128,"height":128},"source_width":128,"source_height":128,"capture_mode":"window",
                "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
                "cursor_settings":{"enabled":false,"spring_tension":170,"spring_friction":30,"spring_mass":1,
                    "loop_to_start":mode=="loop","loop_duration_ms":500},
                "zoom_blocks":[{"start_time_ms":500,"end_time_ms":600,"zoom_level":2,
                    "center_x":if manual{center}else{0.5},"center_y":0.5,"is_manual":manual,"instant_animation":true}]
            })).unwrap();
            if mode=="trim" {settings.trim_start=Some(200);settings.trim_end=Some(700);}
            if mode=="speed" {settings.timeline_clips=Some(serde_json::from_value(serde_json::json!([
                {"id":"fast","startTime":0,"duration":250,"sourceIn":200,"sourceOut":700,
                    "playbackRate":2,"trackOrder":0,"muted":false}])).unwrap());}
            export_video(&root,&input,settings,&info,None).await.unwrap();
            let frame=ffmpeg(&["-v","error","-ss",time,"-i",output.to_str().unwrap(),"-frames:v","1",
                "-f","rawvideo","-pix_fmt","rgb24","-"]);
            assert_eq!(frame.len(),128*128*3);frames.push(frame);
        }
        for y in 60..68 {for x in 60..68 {
            let index=(y*128+x)*3;
            let actual=&frames[0][index..index+3];let expected=&frames[1][index..index+3];
            assert!(actual[channel]>200,"{mode}: target color {actual:?}; artifacts {}",root.display());
            for c in 0..3 {assert!((actual[c] as i16-expected[c] as i16).abs()<=3,"{mode}: auto {actual:?}, manual {expected:?}");}
        }}
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore="requires FFmpeg and native GPU; short zoom through encoded export"]
async fn encoded_brief_zooms_initialize_or_animate_then_return() {
    let root=std::env::temp_dir().join(format!("brief-camera-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("source.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i",
        "color=black:s=128x128:r=120:d=1,drawbox=x=48:y=48:w=32:h=32:color=white:t=fill",
        "-c:v","ffv1",input.to_str().unwrap()]);
    let info=VideoInfo{duration_ms:1000,width:128,height:128,fps:120.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(120)};
    for start in [0,1] { for fps in [24,30,60,120] {
        let output=root.join(format!("short-{start}-{fps}.mp4"));
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "output_path":output,"format":"mp4","codec":"h264","frame_rate":fps,
            "resolution":{"width":128,"height":128},"source_width":128,"source_height":128,
            "capture_mode":"window","screen_motion_style":"focused",
            "zoom_spring_tension":250,"zoom_spring_friction":40,"zoom_spring_mass":2.25,
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
            "cursor_settings":{"enabled":false},
            "zoom_blocks":[{"start_time_ms":start,"end_time_ms":201,"zoom_level":2,
                "center_x":0.5,"center_y":0.5}]
        })).unwrap();
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let pixels=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),
            "-f","rawvideo","-pix_fmt","rgb24","-"]);
        let frame_bytes=128*128*3;
        assert_eq!(pixels.len(),fps as usize*frame_bytes,"frame count at {fps}fps; {}",root.display());
        let widths:Vec<_>=pixels.chunks_exact(frame_bytes).map(|frame| {
            (0..128).filter(|&x| {
                let index=(64*128+x)*3;
                frame[index..index+3].iter().all(|&channel|channel>200)
            }).count()
        }).collect();
        let first=widths[0];let last=*widths.last().unwrap();
        let peak=*widths.iter().max().unwrap();
        if start == 0 {
            assert!((62..=66).contains(&first),"initial zoom missing at {fps}fps: {widths:?}; {}",root.display());
            assert!(peak.abs_diff(first)<=2,"initial zoom should hold its scale; {}",root.display());
        } else {
            assert!((30..=34).contains(&first),"initial width {first}; {}",root.display());
            assert!(peak>=first+8,"short zoom missing at {fps}fps: {widths:?}; {}",root.display());
        }
        assert!((30..=34).contains(&last),"zoom failed to exit at {fps}fps: {widths:?}; {}",root.display());
    }}
    std::fs::remove_dir_all(root).unwrap();
}
