//! Video metadata probing and caching

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use anyhow::{Result, anyhow};
use crate::ffmpeg_manager::{get_ffmpeg_manager, FFmpegOperation, OperationPriority, OperationResult};
use super::types::VideoInfo;

/// Get information about a video file using FFprobe with caching
pub async fn get_video_info(
    video_path: impl AsRef<Path>,
    metadata_cache: &Arc<RwLock<HashMap<PathBuf, VideoInfo>>>,
) -> Result<VideoInfo> {
    let path = video_path.as_ref().to_path_buf();

    // Check cache first
    if let Ok(cache) = metadata_cache.read() {
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
    if let Ok(mut cache) = metadata_cache.write() {
        cache.insert(path, video_info.clone());

        // Limit cache size to prevent excessive memory usage
        if cache.len() > 100 {
            let keys_to_remove: Vec<_> = cache.keys().take(cache.len() - 50).cloned().collect();
            for key in keys_to_remove {
                cache.remove(&key);
            }
        }
    }

    Ok(video_info)
}
