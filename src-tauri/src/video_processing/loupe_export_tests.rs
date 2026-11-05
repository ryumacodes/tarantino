use super::{
    export::export_video,
    types::{ExportSettings, VideoInfo},
};
use std::process::Command;

fn ffmpeg(args: &[&str]) -> Vec<u8> {
    let output = Command::new(crate::ffmpeg::binary("ffmpeg")).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[tokio::test]
#[ignore = "requires FFmpeg and native GPU; verifies encoded magnifier shadow"]
async fn flat_magnifier_shadow_survives_export() {
    let root = std::env::temp_dir().join(format!("loupe-shadow-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("gray.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=0xc8c8c8:s=256x128:r=30:d=1",
        "-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"), "[]").unwrap();
    let info = VideoInfo {duration_ms:1000,width:256,height:128,fps:30.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(30)};
    let mut images = Vec::new();
    for alpha in [0.0,0.5] {
        let output = root.join(format!("shadow-{alpha}.mp4"));
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "output_path":output,"format":"mp4","codec":"h264","frame_rate":30,
            "resolution":{"width":128,"height":128},"source_width":256,"source_height":128,
            "source_crop":{"x":0.5,"y":0,"width":0.5,"height":1},
            "cursor_settings":{"enabled":false},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
            "zoom_blocks":[{"start_time_ms":0,"end_time_ms":1000,"zoom_level":2,
                "center_x":0.5,"center_y":0.5,"is_manual":true,"instant_animation":true,
                "loupe":{"radius":0.2,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":alpha}}]
        })).unwrap();
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let image = ffmpeg(&["-v","error","-ss","0.5","-i",output.to_str().unwrap(),
            "-frames:v","1","-f","rawvideo","-pix_fmt","rgb24","-"]);
        assert_eq!(image.len(),128*128*3);
        images.push(image);
    }
    let pixel = |image: &Vec<u8>, y: usize| image[(y*128+64)*3] as i32;
    let above_loss = pixel(&images[0],15)-pixel(&images[1],15);
    let below_loss = pixel(&images[0],115)-pixel(&images[1],115);
    assert!(above_loss>5,"missing outer shadow");
    assert!(below_loss>above_loss+10,"shadow does not fall away from light: above={above_loss}, below={below_loss}");
    assert!((pixel(&images[0],64)-pixel(&images[1],64)).abs()<4,"shadow darkened lens interior");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires FFmpeg and native GPU; encoded loupe through edits and crop"]
async fn loupe_exports_follow_source_time_through_trim_speed_and_gaps() {
    let root = std::env::temp_dir().join(format!("loupe-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("source.mkv");
    ffmpeg(&[
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=black:s=256x128:r=30:d=1",
        "-c:v",
        "ffv1",
        input.to_str().unwrap(),
    ]);
    std::fs::write(
        input.with_extension("mouse.json"),
        r#"[{"timestamp":0,"x":192,"y":64},{"timestamp":999,"x":192,"y":64}]"#,
    )
    .unwrap();
    let info = VideoInfo {
        duration_ms: 1000,
        width: 256,
        height: 128,
        fps: 30.0,
        bitrate: 0,
        format: "mkv".into(),
        size_bytes: 0,
        frame_count: Some(30),
    };
    let mut baseline_count = 0;
    for case in ["baseline", "trimmed", "edited"] {
        let output = root.join(format!("{case}.mp4"));
        let mut settings:ExportSettings = serde_json::from_value(serde_json::json!({
            "output_path":output,"format":"mp4","codec":"h264","frame_rate":30,
            "resolution":{"width":128,"height":128},"source_width":256,"source_height":128,
            "source_crop":{"x":0.5,"y":0,"width":0.5,"height":1},
            "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none"},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"},
            "zoom_blocks":[{"start_time_ms":200,"end_time_ms":800,"zoom_level":2,
                "center_x":0.5,"center_y":0.5,"is_manual":true,"instant_animation":true,
                "is_disabled":case=="baseline",
                "loupe":{"radius":0.3125,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}}]
        })).unwrap();
        if case == "trimmed" {
            settings.trim_start = Some(200);
            settings.trim_end = Some(800);
        }
        if case == "edited" {
            settings.timeline_clips=Some(serde_json::from_value(serde_json::json!([
                {"id":"fast","startTime":0,"duration":250,"sourceIn":250,"sourceOut":750,"playbackRate":2,"trackOrder":0,"muted":false},
                {"id":"replay","startTime":500,"duration":500,"sourceIn":0,"sourceOut":500,"playbackRate":1,"trackOrder":0,"muted":false}
            ])).unwrap());
        }
        export_video(&root, &input, settings, &info, None)
            .await
            .unwrap();
        let decoded = ffmpeg(&[
            "-v",
            "error",
            "-i",
            output.to_str().unwrap(),
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]);
        assert_eq!(
            decoded.len(),
            (if case == "trimmed" { 18 } else { 30 }) * 128 * 128 * 3
        );
        for (index, frame) in decoded.chunks_exact(128 * 128 * 3).enumerate() {
            let whites = frame
                .chunks_exact(3)
                .filter(|pixel| pixel.iter().all(|&v| v > 180))
                .count();
            if case == "baseline" {
                baseline_count = whites;
                assert!(whites > 20);
                continue;
            }
            let output_ms = index as f64 * 1000.0 / 30.0;
            if case == "edited" && (250.0..500.0).contains(&output_ms) {
                assert!(
                    frame.iter().all(|&v| v < 24),
                    "gap retained lens/cursor pixels at {index}"
                );
                continue;
            }
            let active = case == "trimmed" || output_ms < 250.0 || output_ms >= 700.0;
            if active {
                assert!(
                    whites > baseline_count * 2,
                    "{case}/{index}: cursor not magnified: {whites} vs {baseline_count}"
                );
            } else {
                assert!(
                    whites.abs_diff(baseline_count) < 15,
                    "inactive source interval retained magnification"
                );
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires FFmpeg and native GPU; animated magnifier settling and cadence"]
async fn animated_magnifier_settles_without_residual_pixels_at_both_cadences() {
    let root = std::env::temp_dir().join(format!("loupe-settling-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("source.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=black:s=128x128:r=60:d=3",
        "-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),
        r#"[{"timestamp":0,"x":64,"y":64},{"timestamp":2999,"x":64,"y":64}]"#).unwrap();
    let info = VideoInfo {duration_ms:3000,width:128,height:128,fps:60.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(180)};
    let mut outputs = Vec::new();
    for (fps, glass) in [(30,false),(60,false),(30,true)] {
        let output = root.join(format!("animated-{fps}-{glass}.mp4"));
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "output_path":output,"format":"mp4","codec":"h264","frame_rate":fps,
            "resolution":{"width":128,"height":128},"source_width":128,"source_height":128,
            "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none"},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"},
            "zoom_blocks":[{"start_time_ms":200,"end_time_ms":1400,"zoom_level":2,
                "center_x":0.5,"center_y":0.5,"is_manual":true,"instant_animation":false,
                "loupe":{"radius":0.4,"glass":glass,"bevel":0.2,"chromatic_aberration":0.5,"shadow_alpha":0}}]
        })).unwrap();
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let decoded=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough",
            "-f","rawvideo","-pix_fmt","rgb24","-"]);
        let frames:Vec<_>=decoded.chunks_exact(128*128*3).map(|f|f.to_vec()).collect();
        assert_eq!(frames.len(),fps as usize*3);
        let bright=|frame:&Vec<u8>|frame.chunks_exact(3).filter(|p|p.iter().all(|&v|v>180)).count();
        let initial=bright(&frames[0]);
        let active=bright(&frames[fps as usize]);
        let settled=bright(frames.last().unwrap());
        assert!(initial>20 && active>initial*2,"lens never visibly magnified: {initial}/{active}");
        assert!(settled.abs_diff(initial)<15,"lens retained magnification after exit: {initial}/{settled}");
        outputs.push(frames);
    }
    // Same source times must produce the same visible motion despite encoder cadence.
    for (index,frame) in outputs[0].iter().enumerate() {
        let other=&outputs[1][index*2];
        let mean=frame.iter().zip(other).map(|(&a,&b)|a.abs_diff(b) as f64).sum::<f64>()/frame.len() as f64;
        assert!(mean<3.0,"cadence-dependent lens pixels at frame {index}: mean difference {mean}");
    }
    let changed=outputs[0][30].iter().zip(&outputs[2][30]).filter(|(a,b)|a.abs_diff(**b)>8).count();
    assert!(changed>100,"glass optics were silently dropped: {changed} changed channels");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore="requires FFmpeg and native GPU; camera overlap through encoded export"]
async fn camera_overlap_fades_and_recovers_in_encoded_export() {
    let root=std::env::temp_dir().join(format!("lens-camera-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("source.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=blue:s=256x128:r=30:d=3","-c:v","ffv1",input.to_str().unwrap()]);
    ffmpeg(&["-v","error","-f","lavfi","-i","color=red:s=64x64:r=30:d=3","-c:v","libx264","-pix_fmt","yuv420p",input.with_extension("webcam.mp4").to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),"[]").unwrap();
    let info=VideoInfo{duration_ms:3000,width:256,height:128,fps:30.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(90)};
    let output=root.join("output.mp4");
    let settings:ExportSettings=serde_json::from_value(serde_json::json!({
        "output_path":output,"format":"mp4","codec":"h264","frame_rate":30,
        "resolution":{"width":128,"height":128},"source_width":256,"source_height":128,
        "source_crop":{"x":0.5,"y":0,"width":0.5,"height":1},
        "cursor_settings":{"enabled":false},"webcam_x":0.5,"webcam_y":0.5,"webcam_size":0.5,"webcam_shape":"roundrect",
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
        "zoom_blocks":[{"start_time_ms":500,"end_time_ms":1500,"zoom_level":2,"is_manual":true,
            "center_x":0.5,"center_y":0.5,
            "loupe":{"radius":0.25,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}}]
    })).unwrap();
    export_video(&root,&input,settings.clone(),&info,None).await.unwrap();
    let frames=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-f","rawvideo","-pix_fmt","rgb24","-"]);
    assert_eq!(frames.len(),90*128*128*3);
    let pixel=|frame:usize|{let i=(frame*128*128+64*128+64)*3;&frames[i..i+3]};
    for frame in [0,84] {let p=pixel(frame);assert!(p[0]>200 && p[2]<30,"camera missing at {frame}: {p:?}");}
    let p=pixel(42);assert!(p[0]<30 && p[2]>200,"camera remained inside lens: {p:?}");
    // This corner is inside the rounded camera but outside the circular lens.
    // Lens-content exclusion alone cannot make it reveal the screen.
    for (frame,hidden) in [(0,false),(42,true),(84,false)] {
        let index=(frame*128*128+40*128+40)*3;
        let p=&frames[index..index+3];
        assert!(if hidden {p[0]<30 && p[2]>200} else {p[0]>200 && p[2]<30},
            "stage overlap fade at {frame}: {p:?}");
    }
    for edited in [false,true] {
        let output=root.join(if edited {"edited.mp4"} else {"trimmed.mp4"});
        let mut variant=settings.clone();
        variant.output_path=Some(output.to_string_lossy().into_owned());
        if edited {
            variant.timeline_clips=Some(serde_json::from_value(serde_json::json!([
                {"id":"covered-fast","startTime":0,"duration":200,"sourceIn":1200,"sourceOut":1600,"playbackRate":2,"trackOrder":0,"muted":false},
                {"id":"restored","startTime":400,"duration":400,"sourceIn":2600,"sourceOut":3000,"playbackRate":1,"trackOrder":0,"muted":false},
                {"id":"revisit","startTime":800,"duration":400,"sourceIn":0,"sourceOut":400,"playbackRate":1,"trackOrder":0,"muted":false}
            ])).unwrap());
        } else {
            variant.trim_start=Some(1200);
            variant.trim_end=Some(1600);
        }
        export_video(&root,&input,variant,&info,None).await.unwrap();
        let decoded=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough",
            "-f","rawvideo","-pix_fmt","rgb24","-"]);
        assert_eq!(decoded.len(),(if edited {36} else {12})*128*128*3);
        for (frame,image) in decoded.chunks_exact(128*128*3).enumerate() {
            for (x,y) in [(64,64),(40,40)] {
                let index=(y*128+x)*3;
                let p=&image[index..index+3];
                if edited && (6..12).contains(&frame) {
                    assert!(p.iter().all(|&v|v<24),"gap retained camera at {frame}: {p:?}");
                    continue;
                }
                let source_frame=if !edited {36+frame} else if frame<6 {36+frame*2}
                    else if frame<24 {78+frame-12} else {frame-24};
                let original=(source_frame*128*128+y*128+x)*3;
                let expected=&frames[original..original+3];
                assert!(p.iter().zip(expected).all(|(&a,&b)|a.abs_diff(b)<16),
                    "edited={edited}, frame={frame}, source={source_frame}, pixel=({x},{y}): {p:?} vs {expected:?}");
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore="requires FFmpeg and native GPU; changing camera frames across timeline edits"]
async fn camera_frames_follow_source_time_across_speed_gaps_and_backward_cuts() {
    let root=std::env::temp_dir().join(format!("camera-sync-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("source.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=blue:s=128x128:r=30:d=3",
        "-c:v","ffv1",input.to_str().unwrap()]);
    // Every camera frame has a distinct color, so a frozen or output-time decoder fails.
    let raw=root.join("camera.rgb");
    let pixels:Vec<u8>=(0..90u8).flat_map(|frame| {
        [40+(frame%6)*32,40+((frame/6)%6)*32,40+(frame/36)*64].repeat(64*64)
    }).collect();
    std::fs::write(&raw,pixels).unwrap();
    let camera=input.with_extension("webcam.mp4");
    ffmpeg(&["-v","error","-f","rawvideo","-pix_fmt","rgb24","-s","64x64","-r","30",
        "-i",raw.to_str().unwrap(),"-c:v","libx264","-crf","10","-pix_fmt","yuv420p",camera.to_str().unwrap()]);
    let reference=ffmpeg(&["-v","error","-i",camera.to_str().unwrap(),"-f","rawvideo","-pix_fmt","rgb24","-"]);
    std::fs::write(input.with_extension("mouse.json"),"[]").unwrap();
    let info=VideoInfo{duration_ms:3000,width:128,height:128,fps:30.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(90)};
    for edited in [false,true] {
        let output=root.join(if edited {"edited.mp4"} else {"trimmed.mp4"});
        let mut settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "output_path":output,"format":"mp4","codec":"h264","frame_rate":30,
            "resolution":{"width":128,"height":128},"source_width":128,"source_height":128,
            "cursor_settings":{"enabled":false},"webcam_x":0.2,"webcam_y":0.2,"webcam_size":0.25,"webcam_shape":"roundrect",
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
            "zoom_blocks":[{"start_time_ms":0,"end_time_ms":3000,"zoom_level":2,"is_manual":true,
                "center_x":0.8,"center_y":0.8,"instant_animation":true,
                "loupe":{"radius":0.15,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}}]
        })).unwrap();
        if edited {
            settings.timeline_clips=Some(serde_json::from_value(serde_json::json!([
                {"id":"fast","startTime":0,"duration":500,"sourceIn":1500,"sourceOut":2500,"playbackRate":2,"trackOrder":0,"muted":false},
                {"id":"slow-revisit","startTime":800,"duration":1000,"sourceIn":0,"sourceOut":500,"playbackRate":0.5,"trackOrder":0,"muted":false}
            ])).unwrap());
        } else {settings.trim_start=Some(1000);settings.trim_end=Some(2000);}
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let decoded=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough",
            "-f","rawvideo","-pix_fmt","rgb24","-"]);
        assert_eq!(decoded.len(),(if edited {54} else {30})*128*128*3);
        for (frame,image) in decoded.chunks_exact(128*128*3).enumerate() {
            let p=&image[(25*128+25)*3..(25*128+25)*3+3];
            if edited && (15..24).contains(&frame) {
                assert!(p.iter().all(|&v|v<24),"camera leaked into gap at {frame}: {p:?}");
                continue;
            }
            let source_frame=if !edited {30+frame} else if frame<15 {45+frame*2} else {(frame-24)/2};
            let index=(source_frame*64*64+32*64+32)*3;
            let expected=&reference[index..index+3];
            assert!(p.iter().zip(expected).all(|(&a,&b)|a.abs_diff(b)<12),
                "edited={edited}, output={frame}, source={source_frame}: {p:?} vs {expected:?}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
