#![allow(dead_code)]

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use wgpu::{Device, Queue};

// Sub-modules
mod audio_mixer;
mod compositor;
mod project_loader;
mod textures;
mod timeline_resolver;
mod types;
mod uniforms;

pub use audio_mixer::*;
pub use compositor::*;
pub use project_loader::*;
pub use timeline_resolver::*;
pub use types::*;

use crate::capture::Project;

/// Native GPU preview engine using wgpu
/// 
/// This is our source-of-truth renderer that ensures perfect parity between
/// preview and export. It uses the same GPU shaders and compositing pipeline
/// for both real-time preview and offline rendering.
pub struct PreviewEngine {
    /// wgpu device and queue for GPU operations
    device: Arc<Device>,
    queue: Arc<Queue>,
    
    /// GPU compositor for rendering frames
    compositor: Arc<Mutex<WgpuCompositor>>,
    
    /// Project loader for decoding video/audio tracks
    project_loader: Arc<Mutex<ProjectLoader>>,
    
    /// Timeline resolver for clip boundaries and time mapping
    timeline_resolver: Arc<TimelineResolver>,
    
    /// Audio mixer with cpal output
    audio_mixer: Arc<Mutex<AudioMixer>>,
    
    /// Current project being previewed
    current_project: Arc<Mutex<Option<Project>>>,
    
    /// Playback state
    playback_state: Arc<Mutex<PlaybackState>>,
}

#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub current_time_ms: u64,
    pub start_time_ms: u64,
    pub loop_selection: Option<(u64, u64)>, // (start, end) in ms
    pub playback_speed: f64, // 1.0 = normal, 0.5 = half speed, 2.0 = double speed
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            current_time_ms: 0,
            start_time_ms: 0,
            loop_selection: None,
            playback_speed: 1.0,
        }
    }
}

/// Preview options for real-time playback
#[derive(Debug, Clone)]
pub struct PreviewOptions {
    /// Base resolution for preview (e.g., 1920x1080)
    pub base_resolution: (u32, u32),
    
    /// Background blur radius (0.0 = no blur)
    pub background_blur: f32,
    
    /// Camera PIP options
    pub camera_pip: CameraPipOptions,
    
    /// Cursor rendering options
    pub cursor_options: CursorOptions,
    
    /// Audio options
    pub audio_options: AudioOptions,
}

#[derive(Debug, Clone)]
pub struct CameraPipOptions {
    pub enabled: bool,
    pub size: PipSize,
    pub position: PipPosition,
    pub roundness: f32, // 0.0 = square, 1.0 = fully rounded
    pub shadow: ShadowOptions,
}

#[derive(Debug, Clone)]
pub enum PipSize {
    Small,   // 160x120
    Medium,  // 320x240  
    Large,   // 640x480
    Custom(u32, u32),
}

#[derive(Debug, Clone)]
pub enum PipPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Custom(f32, f32), // Normalized coordinates (0.0-1.0)
}

#[derive(Debug, Clone)]
pub struct ShadowOptions {
    pub enabled: bool,
    pub blur_radius: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone)]
pub struct CursorOptions {
    pub enabled: bool,
    pub scale: f32,          // 1.0 = normal size
    pub motion_blur: bool,   // Enable motion blur for fast movements
}

#[derive(Debug, Clone)]
pub struct AudioOptions {
    pub sample_rate: u32,    // 48000 Hz typical
    pub buffer_size: u32,    // Audio buffer size for low latency
    pub scrub_audio: bool,   // Enable audio during scrubbing
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            base_resolution: (1920, 1080),
            background_blur: 0.0,
            camera_pip: CameraPipOptions {
                enabled: true,
                size: PipSize::Medium,
                position: PipPosition::BottomRight,
                roundness: 0.1,
                shadow: ShadowOptions {
                    enabled: true,
                    blur_radius: 10.0,
                    offset_x: 0.0,
                    offset_y: 4.0,
                    opacity: 0.3,
                },
            },
            cursor_options: CursorOptions {
                enabled: true,
                scale: 1.0,
                motion_blur: false,
            },
            audio_options: AudioOptions {
                sample_rate: 48000,
                buffer_size: 512,
                scrub_audio: false,
            },
        }
    }
}

/// Frame data from the GPU compositor
#[derive(Debug)]
pub struct CompositorFrame {
    pub timestamp_ms: u64,
    pub texture_data: Vec<u8>, // RGBA8 pixel data
    pub width: u32,
    pub height: u32,
}

/// Audio frame from the mixer
#[derive(Debug)]
pub struct AudioFrame {
    pub timestamp_ms: u64,
    pub samples: Vec<f32>, // Interleaved stereo samples
    pub sample_rate: u32,
    pub channels: u32,
}

impl PreviewEngine {
    /// Create a new preview engine
    pub async fn new() -> Result<Self> {
        // Initialize wgpu device
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;
        
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Tarantino Preview Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    // memory_hints removed in newer wgpu versions
                },
                None,
            )
            .await?;
        
        let device = Arc::new(device);
        let queue = Arc::new(queue);
        
        // Initialize compositor
        let compositor = Arc::new(Mutex::new(
            WgpuCompositor::new(device.clone(), queue.clone()).await?
        ));
        
        // Initialize project loader
        let project_loader = Arc::new(Mutex::new(
            ProjectLoader::new()
        ));
        
        // Initialize timeline resolver
        let timeline_resolver = Arc::new(
            TimelineResolver::new()
        );
        
        // Initialize audio mixer
        let audio_mixer = Arc::new(Mutex::new(
            AudioMixer::new(48000).await?
        ));
        
        Ok(Self {
            device,
            queue,
            compositor,
            project_loader,
            timeline_resolver,
            audio_mixer,
            current_project: Arc::new(Mutex::new(None)),
            playback_state: Arc::new(Mutex::new(PlaybackState::default())),
        })
    }
    
    /// Load a project for preview
    pub async fn load_project(&self, project: Project) -> Result<()> {
        println!("Loading project for preview: {}", project.id);
        
        // Load project into project loader
        {
            let mut loader = self.project_loader.lock().await;
            loader.load_project(&project).await?;
        }
        
        // Update timeline resolver
        self.timeline_resolver.set_project(&project).await?;
        
        // Store current project
        {
            let mut current_project = self.current_project.lock().await;
            *current_project = Some(project);
        }
        
        println!("Project loaded successfully for preview");
        Ok(())
    }
    
    /// Start playback from a specific time
    pub async fn play(&self, start_time_ms: u64) -> Result<()> {
        let mut state = self.playback_state.lock().await;
        state.is_playing = true;
        state.start_time_ms = start_time_ms;
        state.current_time_ms = start_time_ms;
        
        // Start audio mixer
        {
            let mut mixer = self.audio_mixer.lock().await;
            mixer.start_playback(start_time_ms).await?;
        }
        
        println!("Preview playback started from {}ms", start_time_ms);
        Ok(())
    }
    
    /// Pause playback
    pub async fn pause(&self) -> Result<()> {
        let mut state = self.playback_state.lock().await;
        state.is_playing = false;
        
        // Pause audio mixer
        {
            let mut mixer = self.audio_mixer.lock().await;
            mixer.pause().await?;
        }
        
        println!("Preview playback paused at {}ms", state.current_time_ms);
        Ok(())
    }
    
    /// Seek to a specific time
    pub async fn seek(&self, time_ms: u64) -> Result<()> {
        let mut state = self.playback_state.lock().await;
        state.current_time_ms = time_ms;
        
        // Seek audio mixer
        {
            let mut mixer = self.audio_mixer.lock().await;
            mixer.seek(time_ms).await?;
        }
        
        println!("Preview seeked to {}ms", time_ms);
        Ok(())
    }
    
    /// Render a single frame at the current time
    pub async fn render_frame(&self, options: &PreviewOptions) -> Result<CompositorFrame> {
        let state = self.playback_state.lock().await;
        let current_time = state.current_time_ms;
        drop(state);
        
        // Get frame data from timeline resolver
        let frame_data = self.timeline_resolver.resolve_frame(current_time).await?;
        
        // Render using GPU compositor
        let mut compositor = self.compositor.lock().await;
        let rendered_frame = compositor.render_frame(&frame_data, options).await?;
        
        Ok(rendered_frame)
    }
    
    /// Update preview options (background blur, camera PIP, etc.)
    pub async fn update_options(&self, options: PreviewOptions) -> Result<()> {
        let mut compositor = self.compositor.lock().await;
        compositor.update_options(options).await?;
        
        println!("Preview options updated");
        Ok(())
    }
    
    /// Get current playback state
    pub async fn get_playback_state(&self) -> PlaybackState {
        self.playback_state.lock().await.clone()
    }
    
    /// Set loop selection for playback
    pub async fn set_loop(&self, start_ms: Option<u64>, end_ms: Option<u64>) -> Result<()> {
        let mut state = self.playback_state.lock().await;
        state.loop_selection = if let (Some(start), Some(end)) = (start_ms, end_ms) {
            Some((start, end))
        } else {
            None
        };
        
        println!("Loop selection set: {:?}", state.loop_selection);
        Ok(())
    }
    
    /// Set playback speed
    pub async fn set_speed(&self, speed: f64) -> Result<()> {
        let mut state = self.playback_state.lock().await;
        state.playback_speed = speed.clamp(0.1, 4.0); // Reasonable speed limits
        
        // Update audio mixer speed
        {
            let mut mixer = self.audio_mixer.lock().await;
            mixer.set_playback_speed(state.playback_speed).await?;
        }
        
        println!("Playback speed set to: {}", state.playback_speed);
        Ok(())
    }
}