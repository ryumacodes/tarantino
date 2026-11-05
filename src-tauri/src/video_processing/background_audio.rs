//! Prepare a looping soundtrack on the edited playback clock.
use anyhow::{Result, bail};
use std::{path::{Path, PathBuf}, process::Command};
use super::export_control::ExportControl;

/// Owns a finite PCM soundtrack; dropping it removes partial and completed output.
#[derive(Debug)]
pub struct PreparedBackgroundAudio {
    pub path: PathBuf,
    directory: PathBuf,
}
impl PreparedBackgroundAudio {
    pub fn new(root: &Path, source: &Path, start_ms: f64, duration_ms: f64,
        volume: f64, control: &ExportControl) -> Result<Self> {
        control.check()?;
        if !start_ms.is_finite() || start_ms < 0.0 || !duration_ms.is_finite() || duration_ms <= 0.0
            || !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            bail!("Invalid background audio timing or volume");
        }
        if !source.is_file() { bail!("Background audio file is missing"); }
        let directory=root.join(format!("background-audio-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory)?;
        let prepared=Self {path:directory.join("soundtrack.wav"),directory};
        let start=(start_ms*48.0).round() as u64;
        let samples=(duration_ms*48.0).ceil() as u64;
        let end=start.checked_add(samples).ok_or_else(||anyhow::anyhow!("Background audio range is too large"))?;
        // Trim after looping, preserving music phase when exporting a selection.
        // This clock is independent of recording cuts and clip playback rates.
        let filter=format!("aresample=48000,aformat=sample_fmts=flt:channel_layouts=stereo,atrim=start_sample={start}:end_sample={end},asetpts=PTS-STARTPTS,volume={volume:.8}");
        let result=control.output(Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-y","-stream_loop","-1","-i"])
            .arg(source).args(["-vn","-af",&filter,"-c:a","pcm_f32le","-ar","48000","-ac","2"]).arg(&prepared.path))?;
        if !result.status.success() { bail!("Could not prepare background audio: {}",String::from_utf8_lossy(&result.stderr)); }
        Ok(prepared)
    }
}
impl Drop for PreparedBackgroundAudio {
    fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.directory); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_audio_parameters_before_starting_decoder() {
        let root=std::env::temp_dir();
        let control=ExportControl::default();
        for (start,duration,volume) in [(f64::NAN,1000.0,1.0),(0.0,0.0,1.0),(0.0,1000.0,2.0)] {
            assert!(PreparedBackgroundAudio::new(&root,Path::new("missing.wav"),start,duration,volume,&control).is_err());
        }
        control.cancel();
        assert!(PreparedBackgroundAudio::new(&root,Path::new("missing.wav"),0.0,1000.0,1.0,&control).unwrap_err().to_string().contains("cancelled"));
    }
    #[test]
    fn looping_soundtrack_preserves_phase_duration_volume_and_cleanup() {
        if std::env::var_os("TARANTINO_TEST_FFMPEG").is_none() {return;}
        let root=std::env::temp_dir().join(format!("background-test-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source=root.join("music.wav");
        let status=Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i","sine=frequency=220:sample_rate=48000:duration=0.5",
            "-f","lavfi","-i","sine=frequency=880:sample_rate=48000:duration=0.5","-filter_complex","[0:a][1:a]concat=n=2:v=0:a=1",
            "-c:a","pcm_f32le","-y"]).arg(&source).status().unwrap();
        assert!(status.success());
        for volume in [1.0,0.5,0.0] {
            let prepared=PreparedBackgroundAudio::new(&root,&source,750.0,2200.0,volume,&ExportControl::default()).unwrap();
            let decoded=Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-i"]).arg(&prepared.path)
                .args(["-f","f32le","-ac","1","pipe:1"]).output().unwrap();
            assert!(decoded.status.success());
            let samples:Vec<f32>=decoded.stdout.chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())).collect();
            assert_eq!(samples.len(),105600);
            for (time,frequency) in [(0.1,880.0),(0.4,220.0),(1.1,880.0),(1.4,220.0),(2.1,880.0)] {
                let start=(time*48000.0) as usize;
                let slice=&samples[start..start+2400];
                let rms=(slice.iter().map(|v|(*v as f64).powi(2)).sum::<f64>()/slice.len() as f64).sqrt();
                if volume==0.0 {assert!(rms<1e-7);continue;}
                assert!((rms-0.08837*volume).abs()<0.003,"rms={rms} volume={volume}");
                let crossings=slice.windows(2).filter(|p|p[0]<=0.0 && p[1]>0.0).count() as f64;
                assert!((crossings/0.05-frequency).abs()<=20.0);
            }
            let output=prepared.path.clone();drop(prepared);assert!(!output.exists());
        }
        let bad=root.join("invalid.wav");std::fs::write(&bad,b"invalid").unwrap();
        assert!(PreparedBackgroundAudio::new(&root,&bad,0.0,1000.0,1.0,&ExportControl::default()).is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(),2);
        std::fs::remove_dir_all(root).unwrap();
    }
}
