//! End-to-end crop checks through decode, pointer mapping and GPU composition.
use super::{export::export_video, types::{ExportSettings, VideoInfo}};
use std::process::Command;

fn ffmpeg(args: &[&str]) -> Vec<u8> {
    let output = Command::new(crate::ffmpeg::binary("ffmpeg")).args(args).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    output.stdout
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; cropped source and cursor integration"]
async fn cropped_exports_keep_source_and_cursor_aligned() {
    let root = std::env::temp_dir().join(format!("crop-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("halves.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i",
        "color=red:s=128x64:r=30:d=1,format=rgba,drawbox=x=64:y=0:w=64:h=64:color=blue:t=fill",
        "-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),r#"[{"timestamp":0,"x":96,"y":32},{"timestamp":999,"x":96,"y":32}]"#).unwrap();
    let info = VideoInfo { duration_ms:1000,width:128,height:64,fps:30.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(30) };
    for edited in [false,true] {
        let output = root.join(format!("crop-{edited}.mp4"));
        let mut settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "output_path":output,"resolution":{"width":128,"height":128},
            "source_crop":{"x":0.5,"y":0,"width":0.5,"height":1},
            "source_width":128,"source_height":64,"frame_rate":30,"format":"mp4","codec":"h264",
            "zoom_blocks":[],"cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none"},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        if edited {
            settings.timeline_clips=Some(serde_json::from_value(serde_json::json!([
                {"id":"fast","startTime":0,"duration":500,"sourceIn":0,"sourceOut":1000,"playbackRate":2,"trackOrder":0,"muted":false}
            ])).unwrap());
        }
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let frames=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]);
        assert_eq!(frames.len(),(if edited {15} else {30})*128*128*3);
        for frame in frames.chunks_exact(128*128*3) {
            // Corners prove crop aspect filled the square and removed the red half.
            for (x,y) in [(8,8),(120,8),(8,120),(120,120)] {
                let rgb=&frame[(y*128+x)*3..][..3];
                assert!(rgb[2]>220 && rgb[0]<30 && rgb[1]<30,"wrong source layout: {rgb:?}");
            }
            let whites:Vec<_>=frame.chunks_exact(3).enumerate().filter(|(_,rgb)|rgb[0]>160 && rgb[1]>160 && rgb[2]>160).collect();
            assert!(whites.len()>20,"missing pointer; artifacts at {}",root.display());
            let min_x=whites.iter().map(|(i,_)|i%128).min().unwrap();
            let min_y=whites.iter().map(|(i,_)|i/128).min().unwrap();
            assert!((60..=68).contains(&min_x) && (60..=68).contains(&min_y),"cursor tip {min_x},{min_y}; expected crop center");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; crop edge effect clipping"]
async fn crop_edges_clip_cursor_effects_to_video_during_zoom() {
    let root=std::env::temp_dir().join(format!("crop-edge-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("blue.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=blue:s=128x64:r=30:d=0.5","-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),r#"[{"timestamp":0,"x":60,"y":32,"event_type":{"ButtonPress":"Left"}},{"timestamp":499,"x":60,"y":32}]"#).unwrap();
    let info=VideoInfo{duration_ms:500,width:128,height:64,fps:30.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(15)};
    for mode in ["display","window"] {
        let mut baseline=Vec::new();
        for effect in ["baseline","none","ripple","circle"] {
            let output=root.join(format!("{mode}-{effect}.mp4"));
            let settings:ExportSettings=serde_json::from_value(serde_json::json!({
                "output_path":output,"resolution":{"width":128,"height":128},
                "source_crop":{"x":0.5,"y":0,"width":0.5,"height":1},"capture_mode":mode,
                "source_width":128,"source_height":64,"frame_rate":30,"format":"mp4","codec":"h264",
                "zoom_blocks":[{"start_time_ms":0,"end_time_ms":500,"zoom_level":1.25,"center_x":0.5,"center_y":0.5,"is_manual":true,"instant_animation":true}],
                "cursor_settings":{"enabled":effect!="baseline","size":1,"color":"#ffffff","click_effect":effect,"trail_enabled":true},
                "visual_settings":{"padding":25,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"}
            })).unwrap();
            export_video(&root,&input,settings,&info,None).await.unwrap();
            let frames=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]);
            if effect=="baseline" { baseline=frames; continue; }
            assert_eq!(frames.len(),baseline.len());
            let mut visible=0;
            for (pixel,base) in frames.chunks_exact(3).zip(baseline.chunks_exact(3)) {
                if base.iter().all(|value|*value<8) {
                    assert!(pixel.iter().all(|value|*value<32),"{mode}/{effect} leaked into background: {pixel:?}; artifacts {}",root.display());
                } else if pixel[0]>80 && pixel[1]>80 { visible+=1; }
            }
            if mode=="window" { assert!(visible>20,"fixture did not exercise a partially clipped cursor"); }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; source detail during zoom"]
async fn zoom_preserves_detail_above_unzoomed_resolution() {
    let root = std::env::temp_dir().join(format!("zoom-detail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("stripes.mkv");
    ffmpeg(&["-v", "error", "-f", "lavfi", "-i",
        "nullsrc=s=512x512:r=30:d=0.2,geq=lum='mod(floor(X/2),2)*255':cb=128:cr=128",
        "-c:v", "ffv1", input.to_str().unwrap()]);
    let info = VideoInfo { duration_ms: 200, width: 512, height: 512, fps: 30.0,
        bitrate: 0, format: "mkv".into(), size_bytes: 0, frame_count: Some(6) };
    for mode in ["display", "window"] {
        let output = root.join(format!("{mode}.mp4"));
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "output_path": output, "resolution": {"width":128,"height":128},
            "source_width":512,"source_height":512,"frame_rate":30,"format":"mp4","codec":"h264",
            "capture_mode": mode,
            "zoom_blocks":[{"start_time_ms":0,"end_time_ms":200,"zoom_level":4,"center_x":0.5,"center_y":0.5,"is_manual":true,"instant_animation":true}],
            "cursor_settings":{"enabled":false},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        export_video(&root, &input, settings, &info, None).await.unwrap();
        let frames = ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-f","rawvideo","-pix_fmt","rgb24","-"]);
        assert_eq!(frames.len(), 6 * 128 * 128 * 3);
        for frame in frames.chunks_exact(128 * 128 * 3) {
            let row = &frame[64 * 128 * 3..65 * 128 * 3];
            let dark = row.chunks_exact(3).filter(|rgb| rgb[0] < 40).count();
            let light = row.chunks_exact(3).filter(|rgb| rgb[0] > 215).count();
            assert!(dark >= 32 && light >= 32,
                "{mode}: source stripes lost during zoom ({dark} dark, {light} light); {}", root.display());
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
