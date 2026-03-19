use anyhow::Result;
use cpal::{Device, Stream, StreamConfig, SampleRate};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use tokio::sync::{Mutex, mpsc};

use super::ProjectLoader;

/// Audio mixer that handles real-time audio playback with frame sync
/// 
/// This uses cpal for cross-platform audio output and maintains synchronization
/// with the video frame clock. It mixes multiple audio tracks (mic + system audio)
/// and handles scrubbing, seeking, and variable playback speeds.
pub struct AudioMixer {
    /// Audio output device and stream
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    
    /// Current playback state
    is_playing: Arc<AtomicBool>,
    current_time_ms: Arc<AtomicU64>,
    playback_speed: Arc<Mutex<f64>>,
    
    /// Audio buffer and mixing
    audio_buffer: Arc<Mutex<Vec<f32>>>, // Circular buffer for audio samples
    buffer_write_pos: Arc<AtomicU64>,
    buffer_read_pos: Arc<AtomicU64>,
    
    /// Project loader reference for audio data
    project_loader: Arc<Mutex<Option<Arc<Mutex<ProjectLoader>>>>>,
    
    /// Audio processing thread communication
    audio_command_sender: mpsc::UnboundedSender<AudioCommand>,
    audio_command_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<AudioCommand>>>>,
    
    /// Sample rate and format
    sample_rate: u32,
    channels: u16,
    buffer_size: usize,
}

#[derive(Debug)]
enum AudioCommand {
    StartPlayback(u64), // timestamp_ms
    Pause,
    Seek(u64), // timestamp_ms
    SetSpeed(f64),
    Stop,
}

impl AudioMixer {
    /// Create a new audio mixer with the specified sample rate
    pub async fn new(sample_rate: u32) -> Result<Self> {
        // Initialize cpal and get default output device
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No default audio output device"))?;

        // Get supported config
        let _supported_config = device.default_output_config()
            .map_err(|e| anyhow::anyhow!("Failed to get default output config: {}", e))?;

        // Create stream config with our desired sample rate
        let config = StreamConfig {
            channels: 2, // Stereo
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        
        // Create audio buffer (1 second of audio)
        let buffer_size = (sample_rate * 2) as usize; // 2 channels
        let audio_buffer = Arc::new(Mutex::new(vec![0.0; buffer_size]));
        
        // Create command channel for audio processing thread
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        
        println!("Audio mixer initialized: {}Hz, {} channels", sample_rate, config.channels);
        
        Ok(Self {
            device,
            config: config.clone(),
            stream: None,
            is_playing: Arc::new(AtomicBool::new(false)),
            current_time_ms: Arc::new(AtomicU64::new(0)),
            playback_speed: Arc::new(Mutex::new(1.0)),
            audio_buffer,
            buffer_write_pos: Arc::new(AtomicU64::new(0)),
            buffer_read_pos: Arc::new(AtomicU64::new(0)),
            project_loader: Arc::new(Mutex::new(None)),
            audio_command_sender: command_sender,
            audio_command_receiver: Arc::new(Mutex::new(Some(command_receiver))),
            sample_rate,
            channels: config.channels,
            buffer_size,
        })
    }
    
    /// Set the project loader reference
    pub async fn set_project_loader(&self, loader: Arc<Mutex<ProjectLoader>>) {
        let mut project_loader = self.project_loader.lock().await;
        *project_loader = Some(loader);
    }
    
    /// Start audio playback from a specific timestamp
    pub async fn start_playback(&mut self, start_time_ms: u64) -> Result<()> {
        println!("Starting audio playback from {}ms", start_time_ms);
        
        // Create and start the audio stream if not already running
        if self.stream.is_none() {
            self.create_audio_stream().await?;
        }
        
        // Send start command to audio processing thread
        self.audio_command_sender.send(AudioCommand::StartPlayback(start_time_ms))
            .map_err(|e| anyhow::anyhow!("Failed to send start command: {}", e))?;
        
        // Update state
        self.is_playing.store(true, Ordering::Relaxed);
        self.current_time_ms.store(start_time_ms, Ordering::Relaxed);
        
        // Start the stream
        if let Some(stream) = &self.stream {
            stream.play().map_err(|e| anyhow::anyhow!("Failed to start audio stream: {}", e))?;
        }
        
        Ok(())
    }
    
    /// Pause audio playback
    pub async fn pause(&mut self) -> Result<()> {
        println!("Pausing audio playback");
        
        self.audio_command_sender.send(AudioCommand::Pause)
            .map_err(|e| anyhow::anyhow!("Failed to send pause command: {}", e))?;
        
        self.is_playing.store(false, Ordering::Relaxed);
        
        // Pause the stream
        if let Some(stream) = &self.stream {
            stream.pause().map_err(|e| anyhow::anyhow!("Failed to pause audio stream: {}", e))?;
        }
        
        Ok(())
    }
    
    /// Seek to a specific timestamp
    pub async fn seek(&mut self, time_ms: u64) -> Result<()> {
        println!("Seeking audio to {}ms", time_ms);
        
        self.audio_command_sender.send(AudioCommand::Seek(time_ms))
            .map_err(|e| anyhow::anyhow!("Failed to send seek command: {}", e))?;
        
        self.current_time_ms.store(time_ms, Ordering::Relaxed);
        
        // Clear audio buffer to avoid playing stale audio
        self.clear_audio_buffer().await;
        
        Ok(())
    }
    
    /// Set playback speed
    pub async fn set_playback_speed(&mut self, speed: f64) -> Result<()> {
        println!("Setting audio playback speed to {}", speed);
        
        self.audio_command_sender.send(AudioCommand::SetSpeed(speed))
            .map_err(|e| anyhow::anyhow!("Failed to send speed command: {}", e))?;
        
        let mut playback_speed = self.playback_speed.lock().await;
        *playback_speed = speed;
        
        Ok(())
    }
    
    /// Stop audio playback and cleanup
    pub async fn stop(&mut self) -> Result<()> {
        println!("Stopping audio mixer");
        
        self.audio_command_sender.send(AudioCommand::Stop)
            .map_err(|e| anyhow::anyhow!("Failed to send stop command: {}", e))?;
        
        self.is_playing.store(false, Ordering::Relaxed);
        
        // Stop and drop the stream
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        
        Ok(())
    }
    
    /// Get current playback time
    pub fn get_current_time_ms(&self) -> u64 {
        self.current_time_ms.load(Ordering::Relaxed)
    }
    
    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }
    
    /// Create the audio output stream
    async fn create_audio_stream(&mut self) -> Result<()> {
        let audio_buffer = self.audio_buffer.clone();
        let buffer_read_pos = self.buffer_read_pos.clone();
        let buffer_write_pos = self.buffer_write_pos.clone();
        let is_playing = self.is_playing.clone();
        let current_time_ms = self.current_time_ms.clone();
        let sample_rate = self.sample_rate;
        
        // Create the audio stream - use F32 format for now
        let stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _info| {
                fill_audio_buffer_f32(
                    data,
                    &audio_buffer,
                    &buffer_read_pos,
                    &buffer_write_pos,
                    &is_playing,
                    &current_time_ms,
                    sample_rate,
                );
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        ).map_err(|e| anyhow::anyhow!("Failed to create audio stream: {}", e))?;
        
        self.stream = Some(stream);
        
        // Start audio processing task
        self.start_audio_processing_task().await;
        
        Ok(())
    }
    
    /// Start the background audio processing task
    async fn start_audio_processing_task(&mut self) {
        let command_receiver = {
            let mut receiver_guard = self.audio_command_receiver.lock().await;
            receiver_guard.take()
        };
        
        if let Some(mut receiver) = command_receiver {
            let audio_buffer = self.audio_buffer.clone();
            let buffer_write_pos = self.buffer_write_pos.clone();
            let project_loader = self.project_loader.clone();
            let current_time_ms = self.current_time_ms.clone();
            let playback_speed = self.playback_speed.clone();
            let sample_rate = self.sample_rate;
            
            tokio::spawn(async move {
                let mut is_processing = false;
                let mut current_time = 0u64;
                
                while let Some(command) = receiver.recv().await {
                    match command {
                        AudioCommand::StartPlayback(start_time_ms) => {
                            is_processing = true;
                            current_time = start_time_ms;
                            println!("Audio processing started from {}ms", start_time_ms);
                        }
                        AudioCommand::Pause => {
                            is_processing = false;
                            println!("Audio processing paused");
                        }
                        AudioCommand::Seek(time_ms) => {
                            current_time = time_ms;
                            println!("Audio processing seeked to {}ms", time_ms);
                        }
                        AudioCommand::SetSpeed(_speed) => {
                            // Speed change handled in playback loop
                            println!("Audio processing speed updated");
                        }
                        AudioCommand::Stop => {
                            println!("Audio processing stopped");
                            break;
                        }
                    }
                    
                    // Process audio if playing
                    if is_processing {
                        Self::process_audio_chunk(
                            &audio_buffer,
                            &buffer_write_pos,
                            &project_loader,
                            &current_time_ms,
                            &playback_speed,
                            sample_rate,
                            current_time,
                        ).await;
                    }
                }
            });
        }
    }
    
    /// Process a chunk of audio samples and write to the circular buffer
    async fn process_audio_chunk(
        audio_buffer: &Arc<Mutex<Vec<f32>>>,
        buffer_write_pos: &Arc<AtomicU64>,
        project_loader: &Arc<Mutex<Option<Arc<Mutex<ProjectLoader>>>>>,
        current_time_ms: &Arc<AtomicU64>,
        playback_speed: &Arc<Mutex<f64>>,
        sample_rate: u32,
        start_time: u64,
    ) {
        // Get audio samples from project loader
        let loader = {
            let loader_guard = project_loader.lock().await;
            loader_guard.as_ref().cloned()
        };
        
        if let Some(loader) = loader {
            // Calculate chunk duration (e.g., 10ms)
            let chunk_duration_ms = 10;
            let speed = *playback_speed.lock().await;
            
            // Get mic and system audio samples
            let loader_guard = loader.lock().await;
            let mic_samples = loader_guard.get_audio_samples("mic", 0, start_time, chunk_duration_ms).await.unwrap_or_default();
            let system_samples = loader_guard.get_audio_samples("system", 0, start_time, chunk_duration_ms).await.unwrap_or_default();
            drop(loader_guard);
            
            // Mix the audio tracks
            let mixed_samples = mix_audio_tracks(mic_samples, system_samples);
            
            // Apply speed adjustment if needed
            let final_samples = if let Some(samples) = mixed_samples {
                if (speed - 1.0).abs() > 0.001 {
                    apply_speed_adjustment(&samples, speed)
                } else {
                    samples
                }
            } else {
                // No audio available - generate silence
                vec![0.0; (sample_rate as u64 * chunk_duration_ms / 1000 * 2) as usize]
            };
            
            // Write samples to circular buffer
            {
                let mut buffer = audio_buffer.lock().await;
                let write_pos = buffer_write_pos.load(Ordering::Relaxed) as usize;
                
                for (i, &sample) in final_samples.iter().enumerate() {
                    let pos = (write_pos + i) % buffer.len();
                    buffer[pos] = sample;
                }
                
                buffer_write_pos.store((write_pos + final_samples.len()) as u64, Ordering::Relaxed);
            }
            
            // Update current time
            current_time_ms.store(start_time + chunk_duration_ms, Ordering::Relaxed);
        }
    }
    
    /// Clear the audio buffer
    async fn clear_audio_buffer(&self) {
        let mut buffer = self.audio_buffer.lock().await;
        buffer.fill(0.0);
        self.buffer_write_pos.store(0, Ordering::Relaxed);
        self.buffer_read_pos.store(0, Ordering::Relaxed);
    }
}

/// Fill audio buffer for f32 output format
fn fill_audio_buffer_f32(
    output: &mut [f32],
    audio_buffer: &Arc<Mutex<Vec<f32>>>,
    buffer_read_pos: &Arc<AtomicU64>,
    buffer_write_pos: &Arc<AtomicU64>,
    is_playing: &Arc<AtomicBool>,
    current_time_ms: &Arc<AtomicU64>,
    sample_rate: u32,
) {
    if !is_playing.load(Ordering::Relaxed) {
        // Fill with silence if not playing
        output.fill(0.0);
        return;
    }
    
    // Try to get audio buffer lock without blocking
    if let Ok(buffer) = audio_buffer.try_lock() {
        let read_pos = buffer_read_pos.load(Ordering::Relaxed) as usize;
        let write_pos = buffer_write_pos.load(Ordering::Relaxed) as usize;
        
        // Calculate available samples
        let available_samples = if write_pos >= read_pos {
            write_pos - read_pos
        } else {
            buffer.len() - read_pos + write_pos
        };
        
        if available_samples >= output.len() {
            // Copy samples from circular buffer
            for (i, sample) in output.iter_mut().enumerate() {
                let pos = (read_pos + i) % buffer.len();
                *sample = buffer[pos];
            }
            
            // Update read position
            buffer_read_pos.store((read_pos + output.len()) as u64, Ordering::Relaxed);
            
            // Update current time based on samples consumed
            let samples_per_ms = sample_rate as f32 / 1000.0;
            let time_advance = (output.len() / 2) as f32 / samples_per_ms; // /2 for stereo
            let new_time = current_time_ms.load(Ordering::Relaxed) + time_advance as u64;
            current_time_ms.store(new_time, Ordering::Relaxed);
        } else {
            // Not enough samples available - fill with silence
            output.fill(0.0);
        }
    } else {
        // Buffer locked - fill with silence
        output.fill(0.0);
    }
}

/// Mix multiple audio tracks together
fn mix_audio_tracks(mic_samples: Option<Vec<f32>>, system_samples: Option<Vec<f32>>) -> Option<Vec<f32>> {
    match (mic_samples, system_samples) {
        (Some(mic), Some(system)) => {
            // Mix both tracks
            let len = mic.len().min(system.len());
            let mut mixed = Vec::with_capacity(len);
            
            for i in 0..len {
                // Simple additive mixing with volume adjustment
                let mixed_sample = (mic[i] + system[i]) * 0.5; // Prevent clipping
                mixed.push(mixed_sample.clamp(-1.0, 1.0));
            }
            
            Some(mixed)
        }
        (Some(mic), None) => Some(mic),
        (None, Some(system)) => Some(system),
        (None, None) => None,
    }
}

/// Apply speed adjustment to audio samples (simple linear interpolation)
fn apply_speed_adjustment(samples: &[f32], speed: f64) -> Vec<f32> {
    if (speed - 1.0).abs() < 0.001 {
        return samples.to_vec();
    }
    
    let input_len = samples.len();
    let output_len = (input_len as f64 / speed) as usize;
    let mut output = Vec::with_capacity(output_len);
    
    for i in 0..output_len {
        let src_pos = (i as f64 * speed) as usize;
        if src_pos < input_len {
            output.push(samples[src_pos]);
        } else {
            output.push(0.0);
        }
    }
    
    output
}