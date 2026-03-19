//! Video processing and editing capabilities with caching and memory optimizations

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use anyhow::{Result, anyhow};
use image::RgbaImage;
use crate::ffmpeg_manager::{get_ffmpeg_manager, FFmpegOperation, OperationPriority, OperationResult};

// Re-export types from the types module
pub use super::types::{
    VideoInfo, CursorSettings, ExportSettings, ProcessingProgress,
};

// Re-export VideoOperation from ffmpeg_manager to avoid duplication
pub use crate::ffmpeg_manager::VideoOperation;

// Import submodules
use super::thumbnails;
use super::export;
use super::cursor_compositing;

/// Video processing and editing capabilities with caching and memory optimizations
pub struct VideoProcessor {
    pub temp_dir: PathBuf,
    // Cache for video metadata to avoid redundant FFprobe calls
    metadata_cache: Arc<RwLock<HashMap<PathBuf, VideoInfo>>>,
    // Cache for thumbnail paths to avoid regenerating
    thumbnail_cache: Arc<RwLock<HashMap<(PathBuf, u32, u32), Vec<PathBuf>>>>,
}

impl VideoProcessor {
    pub fn new() -> Result<Self> {
        let temp_dir = std::env::temp_dir().join("tarantino_processing");
        std::fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            temp_dir,
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
            thumbnail_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Clear all caches to free memory
    pub fn clear_caches(&self) {
        if let Ok(mut cache) = self.metadata_cache.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.thumbnail_cache.write() {
            cache.clear();
        }
    }

    /// Get cache statistics for debugging
    pub fn get_cache_stats(&self) -> (usize, usize) {
        let metadata_count = self.metadata_cache.read().map(|c| c.len()).unwrap_or(0);
        let thumbnail_count = self.thumbnail_cache.read().map(|c| c.len()).unwrap_or(0);
        (metadata_count, thumbnail_count)
    }

    /// Get information about a video file using FFprobe with caching
    pub async fn get_video_info(&self, video_path: impl AsRef<Path>) -> Result<VideoInfo> {
        let path = video_path.as_ref().to_path_buf();

        // Check cache first
        if let Ok(cache) = self.metadata_cache.read() {
            if let Some(cached_info) = cache.get(&path) {
                return Ok(cached_info.clone());
            }
        }

        if !path.exists() {
            return Err(anyhow!("Video file not found: {:?}", path));
        }

        let metadata = std::fs::metadata(&path)?;

        // Use FFmpeg manager to probe video information
        let manager = get_ffmpeg_manager();
        let probe_operation = FFmpegOperation::Probe {
            input: path.clone(),
        };

        let result = manager.execute_operation(probe_operation, OperationPriority::Normal).await
            .map_err(|e| anyhow!("Failed to execute ffprobe: {}", e))?;

        let output_data = match result {
            OperationResult::Success(data) => data,
            OperationResult::Timeout => return Err(anyhow!("FFprobe operation timed out")),
            OperationResult::Error(err) => return Err(anyhow!("FFprobe failed: {}", err)),
            OperationResult::Cancelled => return Err(anyhow!("FFprobe operation was cancelled")),
        };

        let probe_result: serde_json::Value = serde_json::from_slice(&output_data)
            .map_err(|e| anyhow!("Failed to parse ffprobe JSON: {}", e))?;

        // Extract video stream information
        let video_stream = probe_result["streams"][0].as_object()
            .ok_or_else(|| anyhow!("No video stream found"))?;

        let format_info = probe_result["format"].as_object()
            .ok_or_else(|| anyhow!("No format information found"))?;

        // Parse duration (in seconds) and convert to milliseconds
        let duration_seconds: f64 = format_info["duration"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let duration_ms = (duration_seconds * 1000.0) as u64;

        // Parse video dimensions
        let width = video_stream["width"].as_u64().unwrap_or(1920) as u32;
        let height = video_stream["height"].as_u64().unwrap_or(1080) as u32;

        // Parse frame rate
        let fps_str = video_stream["r_frame_rate"].as_str().unwrap_or("30/1");
        let fps = if let Some((num, den)) = fps_str.split_once('/') {
            let num: f64 = num.parse().unwrap_or(30.0);
            let den: f64 = den.parse().unwrap_or(1.0);
            if den != 0.0 { num / den } else { 30.0 }
        } else {
            30.0
        };

        // Parse bitrate
        let bitrate = format_info["bit_rate"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5000000) / 1000; // Convert to kbps

        // Get format
        let format = format_info["format_name"]
            .as_str()
            .unwrap_or("unknown")
            .split(',')
            .next()
            .unwrap_or("unknown")
            .to_string();

        // Parse frame count if available
        let frame_count = video_stream["nb_frames"]
            .as_str()
            .and_then(|s| s.parse().ok());

        let video_info = VideoInfo {
            duration_ms,
            width,
            height,
            fps,
            bitrate,
            format,
            size_bytes: metadata.len(),
            frame_count,
        };

        // Cache the result for future use
        if let Ok(mut cache) = self.metadata_cache.write() {
            cache.insert(path, video_info.clone());

            // Limit cache size to prevent excessive memory usage
            if cache.len() > 100 {
                // Remove oldest entries (simple LRU-like behavior)
                let keys_to_remove: Vec<_> = cache.keys().take(cache.len() - 50).cloned().collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
        }

        Ok(video_info)
    }

    /// Extract thumbnail frames from video for timeline scrubbing using hardware acceleration with caching
    pub async fn extract_thumbnails(
        &self,
        video_path: impl AsRef<Path>,
        thumbnail_count: u32,
        thumbnail_width: u32,
        progress_callback: Option<Arc<dyn Fn(u32, PathBuf) + Send + Sync>>,
    ) -> Result<Vec<PathBuf>> {
        let video_path = video_path.as_ref();

        // Get video info first
        let video_info = self.get_video_info(video_path).await?;

        // Delegate to thumbnails module
        thumbnails::extract_thumbnails(
            &self.temp_dir,
            &self.thumbnail_cache,
            video_path,
            &video_info,
            thumbnail_count,
            thumbnail_width,
            progress_callback,
        ).await
    }

    /// Apply video effects and export using FFmpeg
    pub async fn export_video(
        &self,
        input_path: impl AsRef<Path>,
        settings: ExportSettings,
        progress_callback: Option<Box<dyn Fn(ProcessingProgress) + Send + Sync>>,
    ) -> Result<PathBuf> {
        let input_path = input_path.as_ref();

        // Get video info
        let video_info = self.get_video_info(input_path).await?;

        // Delegate to export module
        export::export_video(
            &self.temp_dir,
            input_path,
            settings,
            &video_info,
            progress_callback,
        ).await
    }

    /// Generate mouse overlay track from recorded mouse events
    pub async fn create_mouse_overlay(
        &self,
        mouse_events: &[crate::mouse_tracking::MouseEvent],
        video_width: u32,
        video_height: u32,
        duration_ms: u64,
    ) -> Result<PathBuf> {
        let overlay_path = self.temp_dir.join("mouse_overlay.mov");

        // Suppress unused parameter warnings for now
        let _ = (mouse_events, video_width, video_height, duration_ms);

        // Create a transparent video with mouse cursor movements
        // This is a complex operation that would require:
        // 1. Creating a video with transparent background
        // 2. Drawing mouse cursor at each frame based on mouse events
        // 3. Interpolating between mouse positions for smooth movement
        // 4. Adding click animations and scroll effects

        // TODO: Create a transparent video with mouse cursor movements
        // For now, create a placeholder overlay file
        std::fs::write(&overlay_path, b"placeholder overlay data")?;

        Ok(overlay_path)
    }

    /// Combine video with mouse overlay
    pub async fn composite_with_mouse_overlay(
        &self,
        base_video: impl AsRef<Path>,
        overlay_video: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let base_path = base_video.as_ref();
        let _overlay_path = overlay_video.as_ref();
        let output_path = output_path.as_ref();

        // TODO: Use overlay filter to combine the videos
        // For now, just copy the base video
        std::fs::copy(base_path, output_path)?;

        Ok(())
    }

    /// Generate cursor frames in memory (no disk I/O)
    /// Returns Vec<RgbaImage> for direct compositing in Rust
    pub fn generate_cursor_frames(
        &self,
        mouse_events_path: &Path,
        cursor_settings: &CursorSettings,
        width: u32,
        height: u32,
        fps: f64,
        duration_ms: u64,
        zoom_trajectory: &Option<Vec<super::zoom_trajectory::ZoomFrameState>>,
    ) -> Result<Vec<RgbaImage>> {
        cursor_compositing::generate_cursor_frames(
            mouse_events_path,
            cursor_settings,
            width,
            height,
            fps,
            duration_ms,
            zoom_trajectory,
        )
    }

    /// Export video with cursor composited in Rust (bypasses FFmpeg overlay stall)
    pub fn export_with_cursor_compositing(
        &self,
        input_path: &Path,
        output_path: &Path,
        cursor_frames: &[RgbaImage],
        width: u32,
        height: u32,
        decoder_fps: u32,
        output_fps: u32,
        duration_ms: u64,
        audio_path: Option<&Path>,
        codec_args: &[String],
        pre_filters: Option<&str>,
        zoom_trajectory: &Option<Vec<super::zoom_trajectory::ZoomFrameState>>,
        progress_callback: Option<&Box<dyn Fn(ProcessingProgress) + Send + Sync>>,
    ) -> Result<()> {
        cursor_compositing::export_with_cursor_compositing(
            input_path,
            output_path,
            cursor_frames,
            width,
            height,
            decoder_fps,
            output_fps,
            duration_ms,
            audio_path,
            codec_args,
            pre_filters,
            zoom_trajectory,
            progress_callback,
        )
    }

    /// Clean up temporary files and clear caches
    pub fn cleanup(&self) -> Result<()> {
        self.clear_caches();

        if self.temp_dir.exists() {
            std::fs::remove_dir_all(&self.temp_dir)?;
        }
        Ok(())
    }

    /// Optimized video processing with memory-conscious operations
    pub async fn process_video_optimized(
        &self,
        input_path: impl AsRef<Path>,
        operations: Vec<VideoOperation>,
    ) -> Result<PathBuf> {
        let input_path = input_path.as_ref();

        // Use FFmpeg manager for video processing
        let output_path = self.temp_dir.join("processed_video.mp4");

        let manager = get_ffmpeg_manager();
        let process_operation = FFmpegOperation::Process {
            input: input_path.to_path_buf(),
            output: output_path.clone(),
            operations,
        };

        let result = manager.execute_operation(process_operation, OperationPriority::Normal).await
            .map_err(|e| anyhow!("Failed to execute video processing: {}", e))?;

        match result {
            OperationResult::Success(_) => Ok(output_path),
            OperationResult::Timeout => Err(anyhow!("Video processing operation timed out")),
            OperationResult::Error(err) => Err(anyhow!("Video processing failed: {}", err)),
            OperationResult::Cancelled => Err(anyhow!("Video processing operation was cancelled")),
        }
    }
}

impl Drop for VideoProcessor {
    fn drop(&mut self) {
        // let _ = self.cleanup();
        // Prevent automatic cleanup to keep thumbnails persistent for the editor
        // Cleanup should be handled explicitly when the app closes or starts a new session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_video_processor_creation() {
        let processor = VideoProcessor::new().unwrap();
        assert!(processor.temp_dir.exists());
    }
}
