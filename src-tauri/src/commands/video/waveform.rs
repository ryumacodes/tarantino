//! Stream waveform peaks without retaining decoded audio in memory.
use std::{process::Stdio,time::Duration};
use tokio::{io::AsyncReadExt,sync::Semaphore};

static EXTRACTIONS:Semaphore=Semaphore::const_new(2);
const SAMPLE_RATE:u64=8000;
const MAX_PEAKS:usize=10_000_000;

struct Peaks {
    rms:bool,
    sums:[f64;2],
    counts:[u64;2],
    channels:u64,
    rate:u64,
    samples:u64,
    values:Vec<f32>,
}
impl Peaks {
    fn new(rate:u32,rms:bool,channels:u64)->Self{Self{rate:rate.into(),rms,sums:[0.0;2],counts:[0;2],channels,samples:0,values:Vec::new()}}
    fn finish_bin(&mut self){
        if self.rms {
            let maximum=(0..self.channels as usize).map(|channel|{
                if self.counts[channel]==0 {0.0} else {(self.sums[channel]/self.counts[channel] as f64).sqrt() as f32}
            }).fold(0.0f32,f32::max);
            if let Some(last)=self.values.last_mut(){*last=maximum;}
        }
    }
    fn push(&mut self,sample:f32)->Result<(),String>{
        let index=(self.samples/self.channels*self.rate/SAMPLE_RATE) as usize;
        if index>=MAX_PEAKS{return Err("Audio is too long for waveform analysis".into());}
        if index==self.values.len(){self.finish_bin();self.values.push(0.0);self.sums=[0.0;2];self.counts=[0;2];}
        if self.rms {
            let channel=(self.samples%self.channels) as usize;
            if sample.is_finite(){self.sums[channel]+=(sample as f64).powi(2);}
            self.counts[channel]+=1;
        } else if sample.is_finite(){self.values[index]=self.values[index].max(sample.abs().min(1.0));}
        self.samples+=1;
        Ok(())
    }
}

#[derive(serde::Serialize)]
pub struct Waveform {pub peaks:Vec<f32>,pub duration_ms:f64}
#[tauri::command]
pub async fn extract_audio_waveform_details(audio_path:String,samples_per_second:u32)->Result<Waveform,String>{
    extract(&audio_path,samples_per_second).await
}
pub async fn extract(path:&str,rate:u32)->Result<Waveform,String>{
    extract_levels(path,rate,false).await
}
pub async fn extract_rms(path:&str,rate:u32)->Result<Waveform,String>{
    extract_levels(path,rate,true).await
}
async fn extract_levels(path:&str,rate:u32,rms:bool)->Result<Waveform,String>{
    if !(1..=1000).contains(&rate){return Err("Waveform sample rate must be between 1 and 1000".into());}
    let _permit=EXTRACTIONS.acquire().await.map_err(|error|error.to_string())?;
    tokio::time::timeout(Duration::from_secs(120),decode(path,rate,rms)).await
        .map_err(|_|"Waveform analysis timed out".to_string())?
}
async fn decode(path:&str,rate:u32,rms:bool)->Result<Waveform,String>{
    // Keep stereo channels separate. Mono maps to left + silent right without
    // attenuation; per-channel RMS maxima avoid phase cancellation and dilution.
    let mut child=tokio::process::Command::new(crate::ffmpeg::binary("ffmpeg"))
        .args(["-v","error","-nostdin","-i",path,"-map","0:a:0","-vn","-af","pan=stereo|c0=c0|c1=c1","-ar","8000","-f","f32le","pipe:1"])
        .stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn()
        .map_err(|error|format!("Could not start waveform analysis: {error}"))?;
    let mut stdout=child.stdout.take().ok_or("Missing waveform output")?;
    let mut peaks=Peaks::new(rate,rms,2);
    let mut buffer=[0u8;32_768];
    let mut remaining=0;
    loop {
        let read=stdout.read(&mut buffer[remaining..]).await.map_err(|error|error.to_string())?;
        if read==0{break;}
        let available=remaining+read;
        let complete=available/4*4;
        for bytes in buffer[..complete].chunks_exact(4){
            peaks.push(f32::from_le_bytes(bytes.try_into().unwrap()))?;
        }
        buffer.copy_within(complete..available,0);
        remaining=available-complete;
    }
    let status=child.wait().await.map_err(|error|error.to_string())?;
    if !status.success() || peaks.values.is_empty() || remaining!=0 || peaks.samples%peaks.channels!=0 {
        return Err("Could not decode audio for waveform analysis".into());
    }
    peaks.finish_bin();
    Ok(Waveform{duration_ms:peaks.samples as f64*1000.0/(SAMPLE_RATE*peaks.channels) as f64,peaks:peaks.values})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_fractional_bins_and_the_final_partial_bin(){
        let mut peaks=Peaks::new(3,false,1);
        for index in 0..8001 {peaks.push(if [2666,5333,7999,8000].contains(&index){0.8}else{0.0}).unwrap();}
        assert_eq!(peaks.values,vec![0.8,0.8,0.8,0.8]);
        let mut short=Peaks::new(100,false,1);
        for sample in [f32::NAN,f32::INFINITY,-0.7,2.0]{short.push(sample).unwrap();}
        assert_eq!(short.values,vec![1.0]);
    }
    #[test]
    fn rms_windows_measure_energy_and_flush_the_partial_tail(){
        let mut levels=Peaks::new(20,true,1);
        for i in 0..600 {levels.push(if i<200{0.5}else if i<400{0.0}else{-0.25}).unwrap();}
        levels.finish_bin();
        assert_eq!(levels.values.len(),2);
        assert!((levels.values[0]-(0.125f32).sqrt()).abs()<0.00001);
        assert!((levels.values[1]-0.25).abs()<0.00001);
    }
    #[tokio::test]
    async fn decodes_real_audio_and_rejects_invalid_inputs(){
        if std::env::var_os("TARANTINO_TEST_FFMPEG").is_none(){return;}
        assert!(extract("missing.wav",0).await.is_err());
        assert!(extract("missing.wav",1001).await.is_err());
        assert!(extract("missing.wav",100).await.is_err());
        let path=std::env::temp_dir().join(format!("waveform-{}.wav",uuid::Uuid::new_v4()));
        let result=std::process::Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i","aevalsrc=if(lt(t\\,0.01)\\,0.25\\,0.75):s=8000:d=0.025","-c:a","pcm_f32le"])
            .arg(&path).status().unwrap();assert!(result.success());
        let data=extract(path.to_str().unwrap(),100).await.unwrap();
        assert!((data.duration_ms-25.0).abs()<0.01);
        let peaks=data.peaks;
        std::fs::remove_file(path).unwrap();
        assert_eq!(peaks.len(),3);
        assert!((peaks[0]-0.25).abs()<0.001);
        assert!((peaks[1]-0.75).abs()<0.001);
        assert!((peaks[2]-0.75).abs()<0.001);
    }
}
