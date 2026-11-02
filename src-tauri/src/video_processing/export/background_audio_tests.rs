use super::*;
use crate::video_processing::{background_audio::PreparedBackgroundAudio, export_control::ExportControl};
use std::{io::Write,process::{Command,Stdio}};
fn run(command:&mut Command)->Vec<u8>{let out=command.output().unwrap();assert!(out.status.success(),"{}",String::from_utf8_lossy(&out.stderr));out.stdout}
fn amplitude(samples:&[f32],hz:f64)->f64 {
    let slice=&samples[12000..36000];
    let (sin,cos)=slice.iter().enumerate().fold((0.0,0.0),|(s,c),(i,v)|{
        let angle=std::f64::consts::TAU*hz*i as f64/48000.0;
        (s+*v as f64*angle.sin(),c+*v as f64*angle.cos())
    });
    2.0*(sin*sin+cos*cos).sqrt()/slice.len() as f64
}
#[test]
fn soundtrack_is_encoded_alone_mixed_or_separate_without_double_trimming(){
    if std::env::var_os("TARANTINO_TEST_FFMPEG").is_none(){return;}
    let root=std::env::temp_dir().join(format!("soundtrack-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let music=root.join("music.wav");let mic=root.join("mic.wav");
    for (path,hz,duration) in [(&music,880,1),(&mic,440,2)] {
        run(Command::new("ffmpeg").args(["-v","error","-f","lavfi","-i",&format!("sine=frequency={hz}:sample_rate=48000:duration={duration}"),"-c:a","pcm_f32le"]).arg(path));
    }
    for with_mic in [false,true] {for dual in [false,true] {for volume in [0.5,0.0] {
        let prepared=PreparedBackgroundAudio::new(&root,&music,750.0,1000.0,volume,&ExportControl::default()).unwrap();
        let tracks=AudioTrackFiles{mic:with_mic.then(||mic.clone()),system:None,background:Some(prepared.path.clone())};
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({"format":"mp4","quality":"low","codec":"h264",
            "trim_start":750,"trim_end":1750,"audio_settings":{"dual_track":dual}})).unwrap();
        let path=root.join(format!("{with_mic}-{dual}-{volume}.mp4"));
        let args=encoder_args(&settings,&ExportAcceleration::Software,&tracks,&settings,(16,16),10,&path);
        let mut child=Command::new("ffmpeg").args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(&vec![128;16*16*4*10]).unwrap();
        let output=child.wait_with_output().unwrap();assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
        let probe:serde_json::Value=serde_json::from_slice(&run(Command::new("ffprobe").args(["-v","error","-select_streams","a","-show_streams","-of","json"]).arg(&path))).unwrap();
        let count=if with_mic && dual {2}else{1};assert_eq!(probe["streams"].as_array().unwrap().len(),count);
        for index in 0..count {
            let bytes=run(Command::new("ffmpeg").args(["-v","error","-i"]).arg(&path).args(["-map",&format!("0:a:{index}"),"-ac","1","-ar","48000","-f","f32le","pipe:1"]));
            let samples:Vec<f32>=bytes.chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())).collect();
            assert!((48000..=49024).contains(&samples.len()));
            let has_music=!with_mic || !dual || index==1;
            let has_mic=with_mic && (!dual || index==0);
            assert!((amplitude(&samples,880.0)-if has_music {0.125*volume}else{0.0}).abs()<0.012);
            assert!((amplitude(&samples,440.0)-if has_mic {0.125}else{0.0}).abs()<0.012);
        }
    }}}
    std::fs::remove_dir_all(root).unwrap();
}
