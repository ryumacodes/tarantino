//! Real mux/encode/decode coverage for mixed and independent audio exports.
use super::*;
use crate::video_processing::{export::export_video, types::VideoInfo};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn successful(command: &mut Command) -> Vec<u8> {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

#[test]
#[ignore = "requires local FFmpeg with libvpx-vp9/libopus; actual export audio integration"]
fn exported_audio_uses_supported_codecs_and_preserves_channel_gains() {
    let root = std::env::temp_dir().join(format!("export-audio-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let result = std::panic::catch_unwind(|| verify_audio(&root));
    std::fs::remove_dir_all(root).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn verify_audio(root: &Path) {
    let mic = root.join("mic.wav");
    let system = root.join("system.wav");
    for (path, hz) in [(&mic, 440), (&system, 880)] {
        successful(
            Command::new(crate::ffmpeg::binary("ffmpeg"))
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("sine=frequency={hz}:sample_rate=48000:duration=1"),
                    "-c:a",
                    "pcm_s16le",
                ])
                .arg(path),
        );
    }
    let tracks = AudioTrackFiles {
        background: None,
        mic: Some(mic),
        system: Some(system),
    };
    for format in ["mp4", "webm"] {
        for dual in [false, true] {
            for (mic_muted, system_muted) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let settings: ExportSettings = serde_json::from_value(serde_json::json!({
                "format": format, "quality": "low", "codec": "h264",
                "audio_settings": { "mic_gain": -12, "system_gain": -6, "dual_track": dual, "mic_muted": mic_muted, "system_muted": system_muted }
            }))
            .unwrap();
                let path = root.join(format!("{format}-{dual}.{format}"));
                let args = encoder_args(
                    &settings,
                    &ExportAcceleration::Software,
                    &tracks,
                    &settings,
                    (16, 16),
                    10,
                    &path,
                );
                let mut child = Command::new(crate::ffmpeg::binary("ffmpeg"))
                    .args(args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(&vec![128; 16 * 16 * 4 * 10])
                    .unwrap();
                let output = child.wait_with_output().unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let probe: serde_json::Value = serde_json::from_slice(&successful(
                    Command::new(crate::ffmpeg::binary("ffprobe"))
                        .args(["-v", "error", "-show_streams", "-of", "json"])
                        .arg(&path),
                ))
                .unwrap();
                let audio: Vec<_> = probe["streams"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|stream| stream["codec_type"] == "audio")
                    .collect();
                assert_eq!(audio.len(), if dual { 2 } else { 1 });
                for (index, stream) in audio.iter().enumerate() {
                    assert_eq!(
                        stream["codec_name"],
                        if format == "webm" { "opus" } else { "aac" }
                    );
                    let pcm = successful(
                        Command::new(crate::ffmpeg::binary("ffmpeg"))
                            .args(["-v", "error", "-i"])
                            .arg(&path)
                            .args([
                                "-map",
                                &format!("0:a:{index}"),
                                "-ac",
                                "1",
                                "-ar",
                                "48000",
                                "-f",
                                "f32le",
                                "-",
                            ]),
                    );
                    let samples: Vec<f64> = pcm
                        .chunks_exact(4)
                        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()) as f64)
                        .collect();
                    assert!(samples.len() >= 47000 && samples.len() <= 50000);
                    // Ignore codec padding and startup transients; the source sine amplitude is 1/8.
                    let stable = &samples[4800..43200];
                    let rms =
                        (stable.iter().map(|s| s * s).sum::<f64>() / stable.len() as f64).sqrt();
                    let mic_rms = if mic_muted {
                        0.0
                    } else {
                        0.125 / 2_f64.sqrt() * 10_f64.powf(-12.0 / 20.0)
                    };
                    let system_rms = if system_muted {
                        0.0
                    } else {
                        0.125 / 2_f64.sqrt() * 10_f64.powf(-6.0 / 20.0)
                    };
                    let expected = if dual {
                        if index == 0 { mic_rms } else { system_rms }
                    } else {
                        (mic_rms * mic_rms + system_rms * system_rms).sqrt()
                    };
                    assert!(
                        (rms - expected).abs() < (expected * 0.1).max(0.00001),
                        "{format} dual={dual} track={index}: rms={rms}, expected={expected}"
                    );
                }
            }
        }
    }
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; full export with audio-only clip edits"]
async fn audio_only_clip_edits_preserve_trimmed_video_and_channel_levels() {
    let root=std::env::temp_dir().join(format!("clip-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("source.mkv");
    successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
        "color=red:s=64x64:r=30:d=1","-f","lavfi","-i","color=blue:s=64x64:r=30:d=1",
        "-filter_complex","[0:v][1:v]concat=n=2:v=1:a=0[out]","-map","[out]","-c:v","ffv1"]).arg(&input));
    for extension in ["mic.wav","system.wav"] {
        successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
            "sine=frequency=440:sample_rate=48000:duration=2","-c:a","pcm_f32le"]).arg(input.with_extension(extension)));
    }
    let output=root.join("result.mp4");
    let settings:ExportSettings=serde_json::from_value(serde_json::json!({
        "output_path":output,"resolution":{"width":64,"height":64},
        "source_width":64,"source_height":64,"frame_rate":30,"format":"mp4","codec":"h264",
        "trim_start":250,"trim_end":1750,"zoom_blocks":[],"cursor_settings":{"enabled":false},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
        "audio_settings":{"dual_track":true,"mic_gain":-6,"system_gain":0},
        "timeline_clips":[
            {"id":"red","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,"playbackRate":1,"trackOrder":0,"muted":false,"micVolume":0.25,"systemVolume":0.7},
            {"id":"blue","startTime":1000,"duration":1000,"sourceIn":1000,"sourceOut":2000,"playbackRate":1,"trackOrder":0,"muted":false,"micVolume":0,"systemVolume":0.5}
        ]
    })).unwrap();
    let info=VideoInfo{duration_ms:2000,width:64,height:64,fps:30.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(60)};
    export_video(&root,&input,settings,&info,None).await.unwrap();
    let frames=successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-i"]).arg(&output)
        .args(["-map","0:v","-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]));
    assert_eq!(frames.len(),45*64*64*3);
    for (index,red) in [(0,true),(15,true),(30,false),(44,false)] {
        let pixel=&frames[index*64*64*3+(32*64+32)*3..][..3];
        assert!(if red {pixel[0]>200 && pixel[2]<30} else {pixel[2]>200 && pixel[0]<30}, "frame {index}: {pixel:?}");
    }
    for (track,gains) in [(0,[0.25*10_f64.powf(-6.0/20.0),0.0]),(1,[0.7,0.5])] {
        let bytes=successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-i"]).arg(&output)
            .args(["-map",&format!("0:a:{track}"),"-ac","1","-ar","48000","-f","f32le","-"]));
        let samples:Vec<f64>=bytes.chunks_exact(4).map(|s|f32::from_le_bytes(s.try_into().unwrap()) as f64).collect();
        assert!((samples.len() as i64-72000).abs()<2048);
        for (start,gain) in [(200,gains[0]),(1000,gains[1])] {
            let region=&samples[start*48..(start+300)*48];
            let rms=(region.iter().map(|s|s*s).sum::<f64>()/region.len() as f64).sqrt();
            let expected=0.125/2_f64.sqrt()*gain;
            assert!((rms-expected).abs()<(expected*0.1).max(0.0001),"track {track} time {start}: {rms} != {expected}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; composed high-speed export with two audio tracks"]
async fn high_speed_composed_exports_keep_video_and_audio_in_sync() {
    let root=std::env::temp_dir().join(format!("fast-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("source.mkv");
    successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
        "color=red:s=64x64:r=30:d=12","-f","lavfi","-i","color=blue:s=64x64:r=30:d=12",
        "-filter_complex","[0:v][1:v]concat=n=2:v=1:a=0[out]","-map","[out]","-c:v","ffv1"]).arg(&input));
    for (extension,hz) in [("mic.wav",440),("system.wav",880)] {
        successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
            &format!("sine=frequency={hz}:sample_rate=48000:duration=24"),"-c:a","pcm_f32le"]).arg(input.with_extension(extension)));
    }
    let info=VideoInfo{duration_ms:24000,width:64,height:64,fps:30.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(720)};
    for rate in [8.0_f64,16.0,24.0] {
        let half=12000.0/rate;
        let duration=half*2.0-200.0;
        let output=root.join(format!("speed-{rate}.mp4"));
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "output_path":output,"resolution":{"width":64,"height":64},
            "source_width":64,"source_height":64,"frame_rate":30,"format":"mp4","codec":"h264",
            "trim_start":100,"trim_end":(half*2.0-100.0) as u64,"zoom_blocks":[],"cursor_settings":{"enabled":false},
            "visual_settings":{"padding":8,"corner_radius":0,"shadow_enabled":false},
            "audio_settings":{"dual_track":true,"mic_gain":0,"system_gain":0},
            "timeline_clips":[
                {"id":"red","startTime":0,"duration":half,"sourceIn":0,"sourceOut":12000,"playbackRate":rate,"trackOrder":0,"muted":false,"micVolume":0.5,"systemVolume":0.8},
                {"id":"blue","startTime":half,"duration":half,"sourceIn":12000,"sourceOut":24000,"playbackRate":rate,"trackOrder":0,"muted":false,"micVolume":0.25,"systemVolume":0.4}
            ]
        })).unwrap();
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let frames=successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-i"]).arg(&output)
            .args(["-map","0:v","-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]));
        let count=(duration*30.0/1000.0).round() as usize;
        assert_eq!(frames.len(),count*64*64*3,"rate {rate}");
        for (index,red) in [(count/4,true),(count*3/4,false)] {
            let frame=&frames[index*64*64*3..][..64*64*3];
            let pixel=&frame[(32*64+32)*3..][..3];
            assert!(if red {pixel[0]>200 && pixel[2]<30} else {pixel[2]>200 && pixel[0]<30},"rate {rate}, frame {index}: {pixel:?}");
            assert_ne!(&frame[..3],pixel,"padding must survive composition");
        }
        for (track,hz,gains) in [(0,440,[0.5,0.25]),(1,880,[0.8,0.4])] {
            let bytes=successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-i"]).arg(&output)
                .args(["-map",&format!("0:a:{track}"),"-ac","1","-ar","48000","-f","f32le","-"]));
            let samples:Vec<f64>=bytes.chunks_exact(4).map(|s|f32::from_le_bytes(s.try_into().unwrap()) as f64).collect();
            assert!((samples.len() as i64-(duration*48.0).round() as i64).abs()<2048,"rate {rate}, audio length");
            for (fraction,gain) in [(0.25,gains[0]),(0.75,gains[1])] {
                let center=(duration*fraction*48.0).round() as usize;
                let region=&samples[center-2400..center+2400];
                let rms=(region.iter().map(|s|s*s).sum::<f64>()/region.len() as f64).sqrt();
                let expected=0.125/2_f64.sqrt()*gain;
                assert!((rms-expected).abs()<expected*0.15,"rate {rate}, track {track}, rms {rms}, expected {expected}");
                let crossings=region.windows(2).filter(|w|w[0]<=0.0 && w[1]>0.0).count() as i32;
                assert!((crossings-hz/10).abs()<=2,"rate {rate}, track {track}, pitch crossings {crossings}");
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; full soundtrack composition and export"]
async fn composed_soundtrack_follows_edited_clock_across_cuts_and_speed_changes() {
    let root=std::env::temp_dir().join(format!("soundtrack-composed-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("source.mkv");
    successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i","color=red:s=64x64:r=30:d=1",
        "-f","lavfi","-i","color=blue:s=64x64:r=30:d=1","-filter_complex","[0:v][1:v]concat=n=2:v=1:a=0",
        "-c:v","ffv1"]).arg(&input));
    let silent=root.join("silent.mkv");std::fs::copy(&input,&silent).unwrap();
    for (extension,hz) in [("mic.wav",330),("system.wav",660)] {
        successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
            &format!("sine=frequency={hz}:sample_rate=48000:duration=2"),"-c:a","pcm_f32le"]).arg(input.with_extension(extension)));
    }
    let music=root.join("music.wav");
    successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i","sine=frequency=220:sample_rate=48000:duration=0.5",
        "-f","lavfi","-i","sine=frequency=880:sample_rate=48000:duration=0.5","-filter_complex","[0:a][1:a]concat=n=2:v=0:a=1",
        "-c:a","pcm_f32le"]).arg(&music));
    let info=VideoInfo{duration_ms:2000,width:64,height:64,fps:30.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(60)};
    for (recorded,dual) in [(false,false),(true,true),(true,false)] {
        let output=root.join(format!("result-{recorded}-{dual}.mp4"));
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "output_path":output,"resolution":{"width":64,"height":64},"source_width":64,"source_height":64,
            "frame_rate":30,"format":"mp4","codec":"h264","trim_start":250,"trim_end":2250,
            "zoom_blocks":[],"cursor_settings":{"enabled":false},
            "visual_settings":{"padding":8,"corner_radius":0,"shadow_enabled":false},
            "background_audio":{"path":music,"volume":0.4,"muted":false},"audio_settings":{"dual_track":dual},
            "timeline_clips":[
                {"id":"blue","startTime":0,"duration":500,"sourceIn":1000,"sourceOut":2000,"playbackRate":2,"trackOrder":0,"muted":false,"micVolume":0.3,"systemVolume":0.2},
                {"id":"red","startTime":500,"duration":2000,"sourceIn":0,"sourceOut":1000,"playbackRate":0.5,"trackOrder":0,"muted":false,"micVolume":0.3,"systemVolume":0.2}
            ]
        })).unwrap();
        export_video(&root,if recorded {&input}else{&silent},settings,&info,None).await.unwrap();
        let frames=successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-i"]).arg(&output)
            .args(["-map","0:v","-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]));
        assert_eq!(frames.len(),60*64*64*3);
        for (index,red) in [(0,false),(20,true),(59,true)] {
            let frame=&frames[index*64*64*3..][..64*64*3];let pixel=&frame[(32*64+32)*3..][..3];
            assert!(if red {pixel[0]>200 && pixel[2]<30}else{pixel[2]>200 && pixel[0]<30},"frame {index}: {pixel:?}");
            assert_ne!(&frame[..3],pixel);
        }
        let probe:serde_json::Value=serde_json::from_slice(&successful(Command::new(crate::ffmpeg::binary("ffprobe")).args(["-v","error","-select_streams","a","-show_streams","-of","json"]).arg(&output))).unwrap();
        let count=if dual {3}else{1};assert_eq!(probe["streams"].as_array().unwrap().len(),count);
        for track in 0..count {
            let bytes=successful(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-i"]).arg(&output)
                .args(["-map",&format!("0:a:{track}"),"-ac","1","-ar","48000","-f","f32le","-"]));
            let samples:Vec<f64>=bytes.chunks_exact(4).map(|s|f32::from_le_bytes(s.try_into().unwrap()) as f64).collect();
            assert!((samples.len() as i64-96000).abs()<2048);
            for (time,music_hz) in [(100,220),(400,880),(900,220),(1400,880),(1900,220)] {
                let region=&samples[(time-25)*48..(time+25)*48];
                let amplitude=|hz:f64| {
                    let (sin,cos)=region.iter().enumerate().fold((0.0,0.0),|(s,c),(i,v)|{
                        let phase=std::f64::consts::TAU*hz*i as f64/48000.0;(s+v*phase.sin(),c+v*phase.cos())
                    });2.0*(sin*sin+cos*cos).sqrt()/region.len() as f64
                };
                let has_music=!dual || track==2;
                let expected=if has_music {0.05}else{0.0};
                assert!((amplitude(music_hz as f64)-expected).abs()<0.008,"recorded={recorded} dual={dual} track={track} at={time}");
                // The opposite music tone would reveal restarting or rate-scaling at a video cut.
                assert!(amplitude(if music_hz==220 {880.0}else{220.0})<0.008);
                for (hz,gain,channel) in [(330.0,0.3,0),(660.0,0.2,1)] {
                    let expected=if recorded && (!dual || track==channel) {0.125*gain}else{0.0};
                    assert!((amplitude(hz)-expected).abs()<0.01);
                }
            }
        }
        assert!(!std::fs::read_dir(&root).unwrap().any(|entry|entry.unwrap().file_name().to_string_lossy().starts_with("background-audio-")));
    }
    std::fs::remove_dir_all(root).unwrap();
}
