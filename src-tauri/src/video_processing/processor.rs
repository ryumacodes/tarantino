//! Video processing and editing capabilities with caching and memory optimizations

use crate::ffmpeg_manager::{
    FFmpegOperation, OperationPriority, OperationResult, get_ffmpeg_manager,
};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

// Re-export types from the types module
pub use super::types::{ExportSettings, ProcessingProgress, VideoInfo};

// Import submodules
use super::export;
use super::thumbnails;

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

        let result = manager
            .execute_operation(probe_operation, OperationPriority::Normal)
            .await
            .map_err(|e| anyhow!("Failed to execute ffprobe: {}", e))?;

        let output_data = match result {
            OperationResult::Success(data) => data,
            OperationResult::Timeout => return Err(anyhow!("FFprobe operation timed out")),
            OperationResult::Error(err) => return Err(anyhow!("FFprobe failed: {}", err)),
        };

        let probe_result: serde_json::Value = serde_json::from_slice(&output_data)
            .map_err(|e| anyhow!("Failed to parse ffprobe JSON: {}", e))?;

        // Extract video stream information
        let video_stream = probe_result["streams"][0]
            .as_object()
            .ok_or_else(|| anyhow!("No video stream found"))?;

        let format_info = probe_result["format"]
            .as_object()
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
            .unwrap_or(5000000)
            / 1000; // Convert to kbps

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
        )
        .await
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
        )
        .await
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
