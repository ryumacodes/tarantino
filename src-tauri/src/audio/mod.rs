//! Shared system-audio and microphone recording tasks.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use cpal::Sample;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::sync::Mutex;

struct WavWriter {
    writer: std::io::BufWriter<std::fs::File>,
    data_written: u32,
}

impl WavWriter {
    fn create(path: &PathBuf, sample_rate: u32, channels: u16) -> Result<Self, String> {
        let file = std::fs::File::create(path)
            .map_err(|e| format!("Failed to create WAV file {}: {}", path.display(), e))?;
        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(b"RIFF").map_err(|e| e.to_string())?;
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;
        writer.write_all(b"WAVE").map_err(|e| e.to_string())?;
        writer.write_all(b"fmt ").map_err(|e| e.to_string())?;
        writer
            .write_all(&16u32.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&1u16.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&channels.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&sample_rate.to_le_bytes())
            .map_err(|e| e.to_string())?;
        let byte_rate = sample_rate * channels as u32 * 2;
        writer
            .write_all(&byte_rate.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&(channels * 2).to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&16u16.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer.write_all(b"data").map_err(|e| e.to_string())?;
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;
        Ok(Self {
            writer,
            data_written: 0,
        })
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writer.write_all(bytes).map_err(|e| e.to_string())?;
        self.data_written = self.data_written.saturating_add(bytes.len() as u32);
        Ok(())
    }

    fn write_i16_samples(&mut self, samples: &[i16]) -> Result<(), String> {
        for sample in samples {
            self.writer
                .write_all(&sample.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
        self.data_written = self.data_written.saturating_add((samples.len() * 2) as u32);
        Ok(())
    }

    fn finalize(mut self) -> Result<(), String> {
        use std::io::Seek;
        let file_size = self.data_written + 36;
        self.writer
            .seek(std::io::SeekFrom::Start(4))
            .map_err(|e| e.to_string())?;
        self.writer
            .write_all(&file_size.to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.writer
            .seek(std::io::SeekFrom::Start(40))
            .map_err(|e| e.to_string())?;
        self.writer
            .write_all(&self.data_written.to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.writer.flush().map_err(|e| e.to_string())
    }
}

/// Spawn separate system and microphone audio capture tasks.
pub fn spawn_audio_tasks(
    audio_rx: Option<tokio::sync::broadcast::Receiver<crate::capture::backends::CapturedAudio>>,
    system_audio_path: Option<PathBuf>,
    mic_audio_path: Option<PathBuf>,
    microphone_device: Option<String>,
    stop_signal: Arc<Mutex<bool>>,
) -> Option<tokio::task::JoinHandle<Result<(), String>>> {
    if system_audio_path.is_none() && mic_audio_path.is_none() {
        return None;
    }

    Some(tokio::spawn(async move {
        let system_stop = Arc::clone(&stop_signal);
        let mic_stop = Arc::clone(&stop_signal);

        let system_task =
            system_audio_path.and_then(|path| spawn_system_audio_task(audio_rx, path, system_stop));
        let mic_task =
            mic_audio_path.map(|path| spawn_microphone_task(path, microphone_device, mic_stop));

        if let Some(task) = system_task {
            task.await.map_err(|e| e.to_string())??;
        }
        if let Some(task) = mic_task {
            task.await.map_err(|e| e.to_string())??;
        }
        Ok(())
    }))
}

/// Spawn the system audio capture task.
fn spawn_system_audio_task(
    audio_rx: Option<tokio::sync::broadcast::Receiver<crate::capture::backends::CapturedAudio>>,
    audio_path: PathBuf,
    stop_signal: Arc<Mutex<bool>>,
) -> Option<tokio::task::JoinHandle<Result<(), String>>> {
    let mut audio_rx = audio_rx?;

    Some(tokio::spawn(async move {
        println!(
            "Audio capture task started, writing to: {}",
            audio_path.display()
        );

        let mut wav_file: Option<WavWriter> = None;
        #[allow(unused_assignments)]
        let mut sample_rate: u32 = 0;
        #[allow(unused_assignments)]
        let mut channels: u16 = 0;

        loop {
            // Check stop signal
            if *stop_signal.lock().await {
                println!("Audio stop signal received");
                break;
            }

            // Receive audio with timeout
            let next =
                tokio::time::timeout(std::time::Duration::from_millis(100), audio_rx.recv()).await;

            let audio = match next {
                Ok(Ok(a)) => a,
                Ok(Err(_)) => break, // Channel closed
                Err(_) => continue,  // Timeout, check stop signal again
            };

            // Initialize WAV file on first audio sample
            if wav_file.is_none() {
                sample_rate = audio.sample_rate;
                channels = audio.channels as u16;

                match WavWriter::create(&audio_path, sample_rate, channels) {
                    Ok(writer) => {
                        wav_file = Some(writer);
                        println!(
                            "WAV file initialized: {}Hz, {} channels",
                            sample_rate, channels
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to create audio file: {}", e);
                        break;
                    }
                }
            }

            // Write audio data
            if let Some(writer) = wav_file.as_mut() {
                if let Err(e) = writer.write_bytes(&audio.data) {
                    eprintln!("Failed to write audio data: {}", e);
                }
            }
        }

        // Finalize WAV file - update header with actual sizes
        if let Some(writer) = wav_file {
            let data_written = writer.data_written;
            let _ = writer.finalize();
            println!(
                "Audio capture finished: {} bytes written to WAV",
                data_written
            );
        }

        Ok(())
    }))
}

fn spawn_microphone_task(
    audio_path: PathBuf,
    _device_id: Option<String>,
    stop_signal: Arc<Mutex<bool>>,
) -> tokio::task::JoinHandle<Result<(), String>> {
    tokio::task::spawn_blocking(move || {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No default microphone input device available".to_string())?;
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default microphone config: {}", e))?;
        let sample_rate = config.sample_rate();
        let channels = config.channels();
        let writer = Arc::new(std::sync::Mutex::new(WavWriter::create(
            &audio_path,
            sample_rate,
            channels,
        )?));
        let err_fn = |err| eprintln!("Microphone stream error: {}", err);

        let stream_config: cpal::StreamConfig = config.clone().into();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                build_mic_stream::<f32>(&device, &stream_config, Arc::clone(&writer), err_fn)
            }
            cpal::SampleFormat::I16 => {
                build_mic_stream::<i16>(&device, &stream_config, Arc::clone(&writer), err_fn)
            }
            cpal::SampleFormat::U16 => {
                build_mic_stream::<u16>(&device, &stream_config, Arc::clone(&writer), err_fn)
            }
            sample_format => Err(format!(
                "Unsupported microphone sample format: {:?}",
                sample_format
            )),
        }?;

        stream
            .play()
            .map_err(|e| format!("Failed to start microphone stream: {}", e))?;
        println!(
            "Microphone capture task started, writing to: {}",
            audio_path.display()
        );

        loop {
            if *stop_signal.blocking_lock() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        drop(stream);
        let writer = Arc::try_unwrap(writer)
            .map_err(|_| "Failed to finalize microphone writer".to_string())?
            .into_inner()
            .map_err(|_| "Microphone writer lock poisoned".to_string())?;
        let data_written = writer.data_written;
        writer.finalize()?;
        println!(
            "Microphone capture finished: {} bytes written to WAV",
            data_written
        );
        Ok(())
    })
}

fn build_mic_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    writer: Arc<std::sync::Mutex<WavWriter>>,
    err_fn: impl Fn(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample,
    i16: cpal::FromSample<T>,
{
    device
        .build_input_stream(
            config.clone(),
            move |data: &[T], _| {
                let samples: Vec<i16> = data
                    .iter()
                    .map(|sample| i16::from_sample(*sample))
                    .collect();
                if let Ok(mut writer) = writer.lock() {
                    let _ = writer.write_i16_samples(&samples);
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build microphone input stream: {}", e))
}
