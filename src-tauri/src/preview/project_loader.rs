use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::capture::Project;
use super::{VideoFrameData, VideoFormat};

/// Decoder for video tracks using ffmpeg-next
pub struct VideoDecoder {
    decoder_context: Arc<Mutex<Option<ffmpeg_next::codec::decoder::Video>>>,
    current_frame: Arc<Mutex<Option<ffmpeg_next::frame::Video>>>,
    file_path: PathBuf,
    time_base: ffmpeg_next::Rational,
}

/// Decoder for audio tracks using ffmpeg-next
pub struct AudioDecoder {
    decoder_context: Arc<Mutex<Option<ffmpeg_next::codec::decoder::Audio>>>,
    current_frame: Arc<Mutex<Option<ffmpeg_next::frame::Audio>>>,
    file_path: PathBuf,
    time_base: ffmpeg_next::Rational,
}

/// Project loader that manages decoders for all tracks in a project
/// 
/// This loads the project.json and creates decoders for each video/audio track.
/// It handles the clip-based structure where segments can be split across multiple files.
pub struct ProjectLoader {
    /// Currently loaded project
    current_project: Option<Project>,
    
    /// Video decoders for each track
    video_decoders: HashMap<String, VideoDecoder>, // track_id -> decoder
    
    /// Audio decoders for each track  
    audio_decoders: HashMap<String, AudioDecoder>, // track_id -> decoder
    
    /// Project root directory (for resolving relative paths)
    project_root: Option<PathBuf>,
    
    /// Cursor events loaded from cursor.json files
    cursor_events: Vec<crate::capture::CursorEvent>,
}

impl ProjectLoader {
    pub fn new() -> Self {
        Self {
            current_project: None,
            video_decoders: HashMap::new(),
            audio_decoders: HashMap::new(),
            project_root: None,
            cursor_events: Vec::new(),
        }
    }
    
    /// Load a project and initialize all decoders
    pub async fn load_project(&mut self, project: &Project) -> Result<()> {
        println!("Loading project for preview: {}", project.id);
        
        // Store project
        self.current_project = Some(project.clone());
        
        // Clear existing decoders
        self.video_decoders.clear();
        self.audio_decoders.clear();
        self.cursor_events.clear();
        
        // TODO: In real implementation, we'd need the project directory path
        // For now, assume we can construct it from the project ID or pass it separately
        
        // Initialize video decoders for display tracks
        for (track_idx, track) in project.tracks.display.iter().enumerate() {
            let track_id = format!("display_{}", track_idx);
            let decoder = self.create_video_decoder(&track.path).await?;
            self.video_decoders.insert(track_id, decoder);
        }
        
        // Initialize video decoders for camera tracks
        if let Some(camera_tracks) = &project.tracks.camera {
            for (track_idx, track) in camera_tracks.iter().enumerate() {
                let track_id = format!("camera_{}", track_idx);
                let decoder = self.create_video_decoder(&track.path).await?;
                self.video_decoders.insert(track_id, decoder);
            }
        }
        
        // Initialize audio decoders for mic tracks
        if let Some(mic_tracks) = &project.tracks.mic {
            for (track_idx, track) in mic_tracks.iter().enumerate() {
                let track_id = format!("mic_{}", track_idx);
                let decoder = self.create_audio_decoder(&track.path).await?;
                self.audio_decoders.insert(track_id, decoder);
            }
        }
        
        // Initialize audio decoders for system audio tracks
        if let Some(system_tracks) = &project.tracks.system {
            for (track_idx, track) in system_tracks.iter().enumerate() {
                let track_id = format!("system_{}", track_idx);
                let decoder = self.create_audio_decoder(&track.path).await?;
                self.audio_decoders.insert(track_id, decoder);
            }
        }
        
        // Load cursor events from all clips
        self.load_cursor_events(project).await?;
        
        println!("Project loaded: {} video decoders, {} audio decoders, {} cursor events", 
                 self.video_decoders.len(), 
                 self.audio_decoders.len(),
                 self.cursor_events.len());
        
        Ok(())
    }
    
    /// Get video frame for a specific track at a timestamp
    pub async fn get_video_frame(&self, track_type: &str, track_index: usize, timestamp_ms: u64) -> Result<Option<VideoFrameData>> {
        let track_id = format!("{}_{}", track_type, track_index);
        
        if let Some(decoder) = self.video_decoders.get(&track_id) {
            decoder.decode_frame_at_time(timestamp_ms).await
        } else {
            Ok(None)
        }
    }
    
    /// Get audio samples for a specific track at a timestamp range
    pub async fn get_audio_samples(&self, track_type: &str, track_index: usize, start_ms: u64, duration_ms: u64) -> Result<Option<Vec<f32>>> {
        let track_id = format!("{}_{}", track_type, track_index);
        
        if let Some(decoder) = self.audio_decoders.get(&track_id) {
            decoder.decode_samples_in_range(start_ms, duration_ms).await
        } else {
            Ok(None)
        }
    }
    
    /// Get cursor events within a time range
    pub fn get_cursor_events(&self, start_ms: u64, end_ms: u64) -> Vec<crate::capture::CursorEvent> {
        self.cursor_events.iter()
            .filter(|event| event.t >= start_ms && event.t <= end_ms)
            .cloned()
            .collect()
    }
    
    /// Get the current project
    pub fn get_project(&self) -> Option<&Project> {
        self.current_project.as_ref()
    }
    
    /// Create a video decoder for a file path
    async fn create_video_decoder(&self, relative_path: &str) -> Result<VideoDecoder> {
        // TODO: Resolve absolute path from project root + relative path
        let file_path = PathBuf::from(relative_path);
        
        println!("Creating video decoder for: {}", file_path.display());
        
        // Initialize ffmpeg if not already done
        ffmpeg_next::init().map_err(|e| anyhow::anyhow!("Failed to initialize ffmpeg: {}", e))?;
        
        // For now, create a placeholder decoder
        // In real implementation, this would:
        // 1. Open the video file with ffmpeg_next::format::input()
        // 2. Find the video stream
        // 3. Create and configure the decoder
        // 4. Store time_base and other metadata
        
        Ok(VideoDecoder {
            decoder_context: Arc::new(Mutex::new(None)),
            current_frame: Arc::new(Mutex::new(None)),
            file_path,
            time_base: ffmpeg_next::Rational(1, 30), // 30 FPS placeholder
        })
    }
    
    /// Create an audio decoder for a file path
    async fn create_audio_decoder(&self, relative_path: &str) -> Result<AudioDecoder> {
        let file_path = PathBuf::from(relative_path);
        
        println!("Creating audio decoder for: {}", file_path.display());
        
        // Initialize ffmpeg if not already done
        ffmpeg_next::init().map_err(|e| anyhow::anyhow!("Failed to initialize ffmpeg: {}", e))?;
        
        // Placeholder decoder - real implementation would set up ffmpeg audio decoder
        Ok(AudioDecoder {
            decoder_context: Arc::new(Mutex::new(None)),
            current_frame: Arc::new(Mutex::new(None)),
            file_path,
            time_base: ffmpeg_next::Rational(1, 48000), // 48kHz placeholder
        })
    }
    
    /// Load cursor events from all clip directories
    async fn load_cursor_events(&mut self, project: &Project) -> Result<()> {
        for clip in &project.clips {
            // Load cursor.json for this clip
            // TODO: Resolve actual file path from project root + clip path
            let cursor_file = format!("clips/{}/cursor.json", clip.id);
            
            if let Ok(cursor_data) = tokio::fs::read_to_string(&cursor_file).await {
                if let Ok(mut clip_events) = serde_json::from_str::<Vec<crate::capture::CursorEvent>>(&cursor_data) {
                    // Adjust timestamps to project time (add clip start time)
                    for event in &mut clip_events {
                        event.t += clip.start_time;
                    }
                    
                    self.cursor_events.extend(clip_events);
                }
            }
        }
        
        // Sort events by timestamp
        self.cursor_events.sort_by_key(|e| e.t);
        
        Ok(())
    }
}

impl VideoDecoder {
    /// Decode a video frame at a specific timestamp
    pub async fn decode_frame_at_time(&self, timestamp_ms: u64) -> Result<Option<VideoFrameData>> {
        // TODO: Real implementation would:
        // 1. Seek to the timestamp in the video file
        // 2. Decode the frame at that position
        // 3. Convert to RGBA8 if needed
        // 4. Return the frame data
        
        // For now, return a placeholder frame
        if timestamp_ms < 60000 { // Only provide frames for first minute
            Ok(Some(VideoFrameData {
                width: 1920,
                height: 1080,
                data: vec![128; 1920 * 1080 * 4], // Gray placeholder
                format: VideoFormat::Rgba8,
            }))
        } else {
            Ok(None)
        }
    }
}

impl AudioDecoder {
    /// Decode audio samples in a time range
    pub async fn decode_samples_in_range(&self, _start_ms: u64, duration_ms: u64) -> Result<Option<Vec<f32>>> {
        // TODO: Real implementation would:
        // 1. Seek to _start_ms in the audio file
        // 2. Decode samples for duration_ms
        // 3. Convert to f32 stereo samples
        // 4. Apply any channel mode conversions
        
        // For now, return silence
        let sample_rate = 48000;
        let samples_needed = ((duration_ms * sample_rate) / 1000) as usize * 2; // stereo
        Ok(Some(vec![0.0; samples_needed]))
    }
}