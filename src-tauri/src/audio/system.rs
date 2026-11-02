//! Timestamp-aligned PCM recording, sharing the first encoded video frame's clock.
use super::{WavWriter, PathBuf, Arc, Mutex};
use crate::capture::backends::CapturedAudio;
use tokio::sync::{broadcast, watch};

pub(super) fn spawn(
    audio_rx: Option<broadcast::Receiver<CapturedAudio>>,
    path: PathBuf,
    stop: Arc<Mutex<bool>>,
    mut video_origin: Option<watch::Receiver<Option<u64>>>,
) -> Option<tokio::task::JoinHandle<Result<(), String>>> {
    let mut receiver = audio_rx?;
    Some(tokio::spawn(async move {
        let mut writer: Option<WavWriter> = None;
        let mut format = None;
        let mut origin = None;
        let mut written_frames = 0_u64;
        let result = async {
            loop {
                let stopping = *stop.lock().await;
                let next = tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv()).await;
                let audio = match next {
                    Ok(Ok(audio)) => audio,
                    Ok(Err(broadcast::error::RecvError::Lagged(count))) => {
                        eprintln!("System audio skipped {count} buffers; retaining timestamp gap");
                        continue;
                    }
                    Ok(Err(broadcast::error::RecvError::Closed)) => break,
                    Err(_) if stopping => break,
                    Err(_) => continue,
                };
                if audio.sample_rate == 0 || audio.channels == 0 || audio.channels > 32 {
                    return Err("Invalid system audio format".to_string());
                }
                let packet_format = (audio.sample_rate, audio.channels);
                if format.is_some_and(|previous| previous != packet_format) {
                    return Err("System audio format changed during recording".to_string());
                }
                format = Some(packet_format);
                let frame_bytes = audio.channels as usize * 2;
                if audio.data.len() % frame_bytes != 0 {
                    return Err("Incomplete system audio PCM frame".to_string());
                }
                if origin.is_none() {
                    origin = Some(if let Some(clock) = video_origin.as_mut() {
                        (*clock.wait_for(|time| time.is_some()).await
                            .map_err(|_| "Video ended before its first frame")?).unwrap()
                    } else { audio.timestamp_us });
                }
                let (silence, skip) = placement(origin.unwrap(), audio.timestamp_us, audio.sample_rate,
                    written_frames, audio.data.len() / frame_bytes);
                if writer.is_none() {
                    writer = Some(WavWriter::create(&path, audio.sample_rate, audio.channels as u16)?);
                }
                let wav = writer.as_mut().unwrap();
                let data = &audio.data[skip * frame_bytes..];
                let silence_bytes = silence.checked_mul(frame_bytes as u64).ok_or("Audio gap too large")?;
                // RIFF's 32-bit sizes must stay valid even for a corrupt timestamp.
                if u64::from(wav.data_written) + silence_bytes + data.len() as u64 > u64::from(u32::MAX - 36) {
                    return Err("System audio exceeds WAV size limit".to_string());
                }
                let zeros = [0_u8; 8192];
                let mut remaining = silence_bytes;
                while remaining > 0 {
                    let bytes = remaining.min(zeros.len() as u64) as usize;
                    wav.write_bytes(&zeros[..bytes])?;
                    remaining -= bytes as u64;
                }
                wav.write_bytes(data)?;
                written_frames += silence + (data.len() / frame_bytes) as u64;
            }
            Ok(())
        }.await;
        // Finalize even when a later packet fails validation, retaining playable data.
        if let Some(wav) = writer { wav.finalize()?; }
        result
    }))
}

fn placement(origin: u64, timestamp: u64, rate: u32, written: u64, packet_frames: usize) -> (u64, usize) {
    let relative = (i128::from(timestamp) - i128::from(origin)) * i128::from(rate);
    let start = (relative + 500_000).div_euclid(1_000_000);
    let delta = start - i128::from(written);
    if delta > 0 { (delta as u64, 0) }
    else { (0, (-delta).min(packet_frames as i128) as usize) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn positions_preserve_preroll_gaps_overlaps_and_long_clock_precision() {
        assert_eq!(placement(1_000_000, 999_000, 48_000, 0, 96), (0,48));
        assert_eq!(placement(1_000_000, 999_000, 48_000, 0, 24), (0,24));
        assert_eq!(placement(1_000_000, 1_010_000, 48_000, 0, 96), (480,0));
        assert_eq!(placement(1_000_000, 1_010_000, 48_000, 500, 96), (0,20));
        let origin = 987_654_321_000;
        for frame in [0, 1, 1024, 48_000 * 3600] {
            let timestamp = origin + (frame * 1_000_000 + 24_000) / 48_000;
            assert_eq!(placement(origin, timestamp, 48_000, frame, 1024), (0,0));
        }
    }

    #[tokio::test]
    async fn lagged_receiver_and_stop_drain_write_silence_then_remaining_pcm() {
        let path = std::env::temp_dir().join(format!("audio-clock-{}.wav", uuid::Uuid::new_v4()));
        let (sender, receiver) = broadcast::channel(2);
        // Overflow before the consumer starts, then stop with two buffers queued.
        for time in [0, 1000, 2000, 3000] {
            sender.send(CapturedAudio { data: Bytes::from_static(&[1,0,2,0]),
                sample_rate: 1000, channels: 2, timestamp_us: 1_000_000 + time }).unwrap();
        }
        let (_clock, clock_rx) = watch::channel(Some(1_000_000));
        let task = spawn(Some(receiver), path.clone(), Arc::new(Mutex::new(true)), Some(clock_rx)).unwrap();
        drop(sender);
        task.await.unwrap().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 16);
        assert_eq!(&bytes[44..], &[0,0,0,0,0,0,0,0,1,0,2,0,1,0,2,0]);
        std::fs::remove_file(path).unwrap();
    }
}
