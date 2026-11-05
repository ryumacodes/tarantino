//! Source-time silence intervals for camera visibility analysis.
use super::waveform;
use crate::video_processing::webcam_timeline::SilenceRange;
fn ranges(levels:&[f32],duration_ms:f64,threshold:f64,minimum_ms:f64)->Vec<SilenceRange>{
    let mut result=Vec::new();let mut start=None;
    for (index,&level) in levels.iter().enumerate(){
        let time=index as f64*50.0;
        if (level as f64)<threshold {if start.is_none(){start=Some(time);}}
        else if let Some(begin)=start.take(){if time-begin>=minimum_ms{result.push(SilenceRange{start_ms:begin,end_ms:time});}}
    }
    if let Some(begin)=start {if duration_ms-begin>=minimum_ms{result.push(SilenceRange{start_ms:begin,end_ms:duration_ms});}}
    result
}
#[tauri::command]
pub async fn detect_audio_silence(audio_path:String,minimum_ms:f64,threshold:f64)->Result<Vec<SilenceRange>,String>{
    if !minimum_ms.is_finite() || !(100.0..=60000.0).contains(&minimum_ms)
        || !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) || threshold==0.0 {
        return Err("Invalid silence duration or threshold".into());
    }
    let levels=waveform::extract_rms(&audio_path,20).await?;
    Ok(ranges(&levels.peaks,levels.duration_ms,threshold,minimum_ms))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn preserves_speech_boundaries_and_partial_end_windows(){
        assert_eq!(ranges(&[0.0,0.0,0.5,0.0,0.0,0.0],275.0,0.1,100.0),vec![SilenceRange{start_ms:0.0,end_ms:100.0},SilenceRange{start_ms:150.0,end_ms:275.0}]);
        assert!(ranges(&[0.0,0.5,0.0],125.0,0.1,100.0).is_empty());
        assert!(ranges(&[0.5,0.5],100.0,0.1,100.0).is_empty());
        assert_eq!(ranges(&[0.0,0.0],100.0,0.1,100.0),vec![SilenceRange{start_ms:0.0,end_ms:100.0}]);
    }
    #[tokio::test]
    async fn stereo_phase_and_single_channel_audio_do_not_become_false_silence(){
        if std::env::var_os("TARANTINO_TEST_FFMPEG").is_none(){return;}
        for right in ["-0.5","0","0.5"] {
            let path=std::env::temp_dir().join(format!("silence-stereo-{}.wav",uuid::Uuid::new_v4()));
            let expression=format!("aevalsrc=if(lt(n\\,1600)\\,0.5\\,0)|if(lt(n\\,1600)\\,{right}\\,0):s=8000:d=0.425");
            let status=std::process::Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",&expression,"-c:a","pcm_f32le"])
                .arg(&path).status().unwrap();assert!(status.success());
            let source=path.to_string_lossy().into_owned();
            let rms=waveform::extract_rms(&source,20).await.unwrap();
            assert_eq!(rms.duration_ms,425.0);
            assert!(rms.peaks[..4].iter().all(|value|(*value-0.5).abs()<0.00001),"RMS for {right}: {:?}",rms.peaks);
            let peaks=waveform::extract(&source,100).await.unwrap();
            assert!(peaks.peaks[..20].iter().all(|value|(*value-0.5).abs()<0.00001));
            let detected=detect_audio_silence(source,100.0,0.1).await.unwrap();
            std::fs::remove_file(path).unwrap();
            assert_eq!(detected,vec![SilenceRange{start_ms:200.0,end_ms:425.0}]);
        }
    }
    #[tokio::test]
    async fn detects_real_pcm_silence_and_validates_parameters(){
        assert!(detect_audio_silence("missing".into(),0.0,0.1).await.is_err());
        assert!(detect_audio_silence("missing".into(),100.0,f64::NAN).await.is_err());
        if std::env::var_os("TARANTINO_TEST_FFMPEG").is_none(){return;}
        let path=std::env::temp_dir().join(format!("silence-{}.wav",uuid::Uuid::new_v4()));
        let status=std::process::Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i","aevalsrc=if(lt(n\\,8000)\\,0.5\\,if(lt(n\\,56000)\\,0\\,if(lt(n\\,60000)\\,0.5\\,0))):s=8000:d=12.525","-c:a","pcm_f32le"])
            .arg(&path).status().unwrap();assert!(status.success());
        let detected=detect_audio_silence(path.to_string_lossy().into_owned(),5000.0,0.1).await.unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(detected,vec![SilenceRange{start_ms:1000.0,end_ms:7000.0},SilenceRange{start_ms:7500.0,end_ms:12525.0}]);
    }
}
