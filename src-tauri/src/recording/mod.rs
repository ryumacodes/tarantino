pub mod artifacts;
pub mod types;

mod encoder_loop;
mod finalization;
#[cfg(target_os = "linux")]
mod linux;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

pub use types::*;

#[cfg(target_os = "macos")]
use crate::capture::backends::{
    CaptureBackendFactory, CaptureConfig, CaptureSourceType, NativeCaptureBackend,
};

/// Recording API for managing recording sessions
pub struct RecordingAPI {
    current_state: RecordingState,
    temp_path: Option<PathBuf>,
    #[cfg(target_os = "windows")]
    child: Option<tokio::process::Child>,
    #[cfg(target_os = "linux")]
    linux_recording: Option<linux::LinuxRecording>,
    #[cfg(target_os = "macos")]
    capture_backend: Option<Box<dyn NativeCaptureBackend>>,
    #[cfg(target_os = "macos")]
    recording_task: Option<tokio::task::JoinHandle<Result<(), String>>>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    audio_task: Option<tokio::task::JoinHandle<Result<(), String>>>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    stop_signal: Arc<Mutex<bool>>,
    #[cfg(target_os = "macos")]
    video_start_time: Arc<parking_lot::Mutex<Option<SystemTime>>>,
}

impl RecordingAPI {
    pub fn new() -> Result<Self> {
        Ok(Self {
            current_state: RecordingState::Idle,
            temp_path: None,
            #[cfg(target_os = "windows")]
            child: None,
            #[cfg(target_os = "linux")]
            linux_recording: None,
            #[cfg(target_os = "macos")]
            capture_backend: None,
            #[cfg(target_os = "macos")]
            recording_task: None,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            audio_task: None,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            stop_signal: Arc::new(Mutex::new(false)),
            #[cfg(target_os = "macos")]
            video_start_time: Arc::new(parking_lot::Mutex::new(None)),
        })
    }

    pub async fn start_recording(&mut self, config: RecordingConfig) -> Result<()> {
        if matches!(
            self.current_state,
            RecordingState::Recording | RecordingState::Paused | RecordingState::Stopping
        ) {
            anyhow::bail!("Recording is already active");
        }

        let out_path = PathBuf::from(&config.output_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        #[cfg(target_os = "macos")]
        {
            *self.video_start_time.lock() = None;
            println!("Starting native ScreenCaptureKit recording");

            // Extract source ID and type from config
            let (source_id, source_type) = match &config.target {
                RecordingTarget::Desktop { display_id, .. } => {
                    (*display_id as u64, CaptureSourceType::Display)
                }
                RecordingTarget::Window { window_id, .. } => {
                    (*window_id, CaptureSourceType::Window)
                }
                RecordingTarget::Device { .. } => {
                    anyhow::bail!("Device capture is not supported yet")
                }
            };

            let fps = match config.quality {
                QualityPreset::Low => 30,
                QualityPreset::Medium => 30,
                QualityPreset::High => 60,
                QualityPreset::Lossless => 60,
            };

            // Create capture backend
            let mut backend = CaptureBackendFactory::create_backend()?;

            // Configure capture
            let capture_config = CaptureConfig {
                source_id,
                source_type,
                fps,
                include_cursor: config.include_cursor,
                include_audio: config.include_system_audio,
                region: None,
                output_path: Some(config.output_path.clone()),
            };

            // Start capture
            backend.start_capture(capture_config).await?;

            // Get frame receiver
            let frame_rx = backend
                .frame_receiver()
                .ok_or_else(|| anyhow::anyhow!("Failed to get frame receiver"))?;

            // Get audio receiver if audio capture is enabled
            let audio_rx = if config.include_system_audio || config.include_microphone {
                backend.audio_receiver()
            } else {
                None
            };

            // Store backend
            self.capture_backend = Some(backend);

            // Create stop signal
            *self.stop_signal.lock().await = false;
            let stop_signal = Arc::clone(&self.stop_signal);
            let output_path = out_path.clone();

            // Spawn separate audio tasks so the editor can show and edit each
            // source independently, then flatten only at export time.
            let audio_stop_signal = Arc::clone(&self.stop_signal);
            let artifacts = artifacts::RecordingArtifacts::new(&out_path);
            let system_audio_path = artifacts.system_audio();
            let mic_audio_path = artifacts.microphone();
            self.audio_task = crate::audio::spawn_audio_tasks(
                audio_rx,
                if config.include_system_audio {
                    Some(system_audio_path)
                } else {
                    None
                },
                if config.include_microphone {
                    Some(mic_audio_path)
                } else {
                    None
                },
                config.microphone_device.clone(),
                audio_stop_signal,
            );

            // Spawn video recording task
            self.recording_task = Some(encoder_loop::spawn_video_task(
                frame_rx,
                output_path,
                fps,
                stop_signal,
                Arc::clone(&self.video_start_time),
            ));
        }

        #[cfg(target_os = "linux")]
        {
            *self.stop_signal.lock().await = false;
            let recording = linux::LinuxRecording::start(&config, &out_path).await?;
            self.linux_recording = Some(recording);

            let artifacts = artifacts::RecordingArtifacts::new(&out_path);
            self.audio_task = crate::audio::spawn_audio_tasks(
                None,
                None,
                if config.include_microphone {
                    Some(artifacts.microphone())
                } else {
                    None
                },
                config.microphone_device.clone(),
                Arc::clone(&self.stop_signal),
            );
        }

        #[cfg(target_os = "windows")]
        {
            // Temporary Windows fallback until the native capture backend is ready.
            use tokio::process::Command;
            let mut cmd = Command::new("ffmpeg");
            cmd.arg("-y")
                .arg("-f")
                .arg("gdigrab")
                .arg("-i")
                .arg("desktop")
                .arg("-c:v")
                .arg("libx264")
                .arg("-preset")
                .arg("veryfast")
                .arg("-crf")
                .arg("20")
                .arg(out_path.to_string_lossy().to_string());

            let child = cmd.spawn()?;
            self.child = Some(child);
        }

        self.current_state = RecordingState::Recording;
        self.temp_path = Some(out_path);
        Ok(())
    }

    pub fn video_start_time(&self) -> Option<SystemTime> {
        #[cfg(target_os = "macos")]
        {
            return *self.video_start_time.lock();
        }

        #[cfg(target_os = "linux")]
        {
            None
        }

        #[cfg(target_os = "windows")]
        {
            None
        }
    }

    pub async fn signal_stop(&mut self) -> Result<String> {
        #[cfg(target_os = "macos")]
        {
            // Signal stop to native recording
            println!("Signaling stop to native recording");
            *self.stop_signal.lock().await = true;

            // Stop the capture source before yielding to the encoder task. The encoder's
            // VideoToolbox flush and MP4 finalization contain synchronous work; yielding
            // here lets that work run first and keeps ScreenCaptureKit recording for
            // seconds after the user clicked Stop. Closing the source immediately gives
            // the encoder a fixed queue to drain and makes the media endpoint match the
            // user's stop action.
            if let Some(backend) = &mut self.capture_backend {
                println!("Stopping capture backend");
                let _ = backend.stop_capture().await;
            }
        }

        #[cfg(target_os = "linux")]
        {
            *self.stop_signal.lock().await = true;
            if let Some(recording) = &mut self.linux_recording {
                recording.signal_stop()?;
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Try to gracefully terminate the ffmpeg recorder
            if let Some(child) = &mut self.child {
                #[cfg(unix)]
                {
                    if let Some(pid) = child.id() {
                        let _ = std::process::Command::new("kill")
                            .arg("-INT")
                            .arg(pid.to_string())
                            .output();
                    }
                }
            }
        }

        self.current_state = RecordingState::Stopping;
        Ok(self
            .temp_path
            .clone()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string())
    }

    pub async fn wait_for_completion(&mut self) -> Result<String> {
        // Join native recording task on macOS
        #[cfg(target_os = "macos")]
        {
            if let Some(handle) = self.recording_task.take() {
                match handle.await {
                    Ok(Ok(())) => {
                        println!("Recording task completed successfully");
                    }
                    Ok(Err(err)) => {
                        let error = format!("Recording failed: {}", err);
                        eprintln!("{}", error);
                        self.current_state = RecordingState::Error;
                        return Err(anyhow::anyhow!(error));
                    }
                    Err(e) => {
                        let error = format!("Recording task panicked: {}", e);
                        eprintln!("{}", error);
                        self.current_state = RecordingState::Error;
                        return Err(anyhow::anyhow!(error));
                    }
                }
            }

            // Wait for audio task to complete
            if let Some(audio_handle) = self.audio_task.take() {
                match audio_handle.await {
                    Ok(Ok(())) => {
                        println!("Audio capture task completed successfully");
                    }
                    Ok(Err(err)) => {
                        eprintln!("Audio capture warning: {}", err);
                    }
                    Err(e) => {
                        eprintln!("Audio capture task panicked: {}", e);
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(recording) = self.linux_recording.take() {
                recording.wait().await?;
            }
            if let Some(audio_handle) = self.audio_task.take() {
                match audio_handle.await {
                    Ok(Ok(())) => println!("Microphone capture task completed successfully"),
                    Ok(Err(error)) => eprintln!("Microphone capture warning: {error}"),
                    Err(error) => eprintln!("Microphone capture task panicked: {error}"),
                }
            }
        }

        // Join the temporary FFmpeg child on Windows.
        #[cfg(target_os = "windows")]
        if let Some(mut child) = self.child.take() {
            match child.wait().await {
                Ok(status) if !status.success() => {
                    let error = format!("FFmpeg exited with non-zero status: {:?}", status);
                    eprintln!("{}", error);
                    self.current_state = RecordingState::Error;
                    return Err(anyhow::anyhow!(error));
                }
                Err(e) => {
                    let error = format!("Failed to wait for FFmpeg: {}", e);
                    eprintln!("{}", error);
                    self.current_state = RecordingState::Error;
                    return Err(anyhow::anyhow!(error));
                }
                _ => {}
            }
        }

        let path = self
            .temp_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No recording path available"))?
            .to_string_lossy()
            .to_string();

        // Verify the file exists
        if !std::path::Path::new(&path).exists() {
            let error = format!("Recording file does not exist: {}", path);
            eprintln!("{}", error);
            self.current_state = RecordingState::Error;
            return Err(anyhow::anyhow!(error));
        }

        self.current_state = RecordingState::Completed;
        println!("Recording finalized successfully: {}", path);
        Ok(path)
    }

    pub async fn pause(&mut self) -> Result<()> {
        // State-only pause for now; native pause can be integrated later
        self.current_state = RecordingState::Paused;
        Ok(())
    }

    pub async fn resume(&mut self) -> Result<()> {
        self.current_state = RecordingState::Recording;
        Ok(())
    }

    pub fn get_state(&self) -> RecordingState {
        self.current_state.clone()
    }
}

/// Recording state enum
#[derive(Debug, Clone)]
pub enum RecordingState {
    Idle,
    Recording,
    Paused,
    Stopping,
    Completed,
    Error,
}
