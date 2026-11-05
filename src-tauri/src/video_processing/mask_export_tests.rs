use super::{export::export_video,types::{ExportSettings,VideoInfo}};
use std::process::Command;
fn ffmpeg(args:&[&str])->Vec<u8>{
    let output=Command::new(crate::ffmpeg::binary("ffmpeg")).args(args).output().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));output.stdout
}
#[tokio::test]
#[ignore="requires FFmpeg and native GPU"]
async fn masks_reach_encoded_video_only_during_their_intervals(){
    let root=std::env::temp_dir().join(format!("mask-export-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
    let input=root.join("input.mkv");let output=root.join("output.mp4");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=c=0x8090a0:s=128x128:r=30:d=2","-c:v","ffv1",input.to_str().unwrap()]);
    let settings:ExportSettings=serde_json::from_value(serde_json::json!({
        "output_path":output,"resolution":{"width":128,"height":128},"source_width":128,"source_height":128,
        "frame_rate":30,"format":"mp4","codec":"h264","zoom_blocks":[],"cursor_settings":{"enabled":false},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"motion_blur_enabled":false},
        "mask_overlay":{"frames":60,"spans":[{"start":15,"end":30,"rect":[0.25,0.25,0.5,0.5],"radius":0.05,"sensitive":false,"highlightOpacity":0.5}]}
    })).unwrap();
    let info=VideoInfo{duration_ms:2000,width:128,height:128,fps:30.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(60)};
    export_video(&root,&input,settings,&info,None).await.unwrap();
    for (time,active) in [("0.2",false),("0.7",true),("1.2",false)]{
        let pixels=ffmpeg(&["-v","error","-ss",time,"-i",output.to_str().unwrap(),"-frames:v","1","-f","rawvideo","-pix_fmt","rgb24","-"]);
        assert_eq!(pixels.len(),128*128*3);
        let brightness=|x:usize,y:usize|pixels[(y*128+x)*3..(y*128+x)*3+3].iter().map(|v|u32::from(*v)).sum::<u32>();
        let center=brightness(64,64);let outside=brightness(10,10);
        if active {assert!(center>outside+80,"highlight must dim exterior");}
        else {assert!(center.abs_diff(outside)<10,"mask must not remain outside interval");}
    }
    println!("Encoded mask interval verified: {}",output.display());
}
