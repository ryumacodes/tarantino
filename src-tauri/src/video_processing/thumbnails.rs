//! Thumbnail extraction for video timeline scrubbing

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use anyhow::{Result, anyhow};
use crate::ffmpeg_manager::{get_ffmpeg_manager, FFmpegOperation, OperationPriority, OperationResult};

use super::types::VideoInfo;

/// Extract thumbnail frames from video for timeline scrubbing using hardware acceleration with caching
pub async fn extract_thumbnails(
    temp_dir: &Path,
    thumbnail_cache: &std::sync::Arc<std::sync::RwLock<std::collections::HashMap<(PathBuf, u32, u32), Vec<PathBuf>>>>,
    video_path: impl AsRef<Path>,
    video_info: &VideoInfo,
    thumbnail_count: u32,
    thumbnail_width: u32,
    progress_callback: Option<Arc<dyn Fn(u32, PathBuf) + Send + Sync>>,
) -> Result<Vec<PathBuf>> {
    let video_path = video_path.as_ref().to_path_buf();
    let cache_key = (video_path.clone(), thumbnail_count, thumbnail_width);

    // Check cache first
    if let Ok(cache) = thumbnail_cache.read() {
        if let Some(cached_thumbnails) = cache.get(&cache_key) {
            // Verify all cached thumbnails still exist
            if cached_thumbnails.iter().all(|p| p.exists()) {
                return Ok(cached_thumbnails.clone());
            }
        }
    }

    if !video_path.exists() {
        return Err(anyhow!("Video file not found: {:?}", video_path));
    }

    let duration_seconds = video_info.duration_ms as f64 / 1000.0;

    if duration_seconds <= 0.0 {
        return Err(anyhow!("Invalid video duration"));
    }

    let mut thumbnails = Vec::new();
    let thumb_dir = temp_dir.join("thumbnails");
    std::fs::create_dir_all(&thumb_dir)?;

    // Generate thumbnails using FFmpeg manager (prevents process leaks)
    let manager = get_ffmpeg_manager();
    let mut thumbnail_tasks = Vec::new();

    // Limit concurrent FFmpeg processes to avoid GPU/CPU contention
    let semaphore = Arc::new(Semaphore::new(3));

    for i in 0..thumbnail_count {
        let time_offset = if thumbnail_count > 1 {
            let raw_offset = (i as f64 / (thumbnail_count - 1) as f64) * duration_seconds;
            // Safety margin: prevent seeking past the last frame
            // Use frame count if available for precise calculation, otherwise use 0.2s margin
            let safe_duration = if let Some(frame_count) = video_info.frame_count {
                if frame_count > 0 && video_info.fps > 0.0 {
                    // Calculate duration of last frame: (frame_count - 1) / fps
                    // Use min to cap at the safe duration, not max
                    ((frame_count - 1) as f64 / video_info.fps).min(duration_seconds - 0.2)
                } else {
                    duration_seconds - 0.2
                }
            } else {
                duration_seconds - 0.2
            };
            raw_offset.min(safe_duration).max(0.0)
        } else {
            duration_seconds / 2.0 // Middle frame if only one thumbnail
        };

        let output_path = thumb_dir.join(format!("thumb_{:04}.jpg", i));

        let thumbnail_operation = FFmpegOperation::Thumbnail {
            input: video_path.clone(),
            output: output_path.clone(),
            time_offset,
            width: thumbnail_width,
        };

        let manager_clone = manager.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let callback_clone = progress_callback.clone();

        let task = tokio::spawn(async move {
            // Hold permit until task completes
            let _permit = permit;
            let result = manager_clone.execute_operation(thumbnail_operation, OperationPriority::Low).await;
            match result {
                Ok(OperationResult::Success(_)) => {
                    if output_path.exists() {
                        if let Some(cb) = callback_clone {
                            cb(i as u32, output_path.clone());
                        }
                        Some(output_path)
                    } else {
                        println!("Warning: Thumbnail {} was not created", i);
                        None
                    }
                },
                Ok(OperationResult::Timeout) => {
                    println!("Warning: Thumbnail {} generation timed out", i);
                    None
                },
                Ok(OperationResult::Error(err)) => {
                    println!("Warning: Thumbnail {} generation failed: {}", i, err);
                    None
                },
                Ok(OperationResult::Cancelled) => {
                    println!("Warning: Thumbnail {} generation was cancelled", i);
                    None
                },
                Err(e) => {
                    println!("Warning: Failed to queue thumbnail {} generation: {}", i, e);
                    None
                }
            }
        });

        thumbnail_tasks.push(task);
    }

    // Wait for all thumbnail tasks to complete using join_all to handle out-of-order completion
    let results = futures::future::join_all(thumbnail_tasks).await;

    // Collect all successfully generated thumbnails
    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(Some(thumb_path)) => {
                println!("Thumbnail {} generated: {:?}", i, thumb_path);
                thumbnails.push(thumb_path);
            },
            Ok(None) => {}, // Already logged in the task
            Err(e) => println!("Warning: Thumbnail task {} failed: {}", i, e),
        }
    }

    if thumbnails.is_empty() {
        return Err(anyhow!("No thumbnails were successfully generated"));
    }

    // Sort thumbnails by filename to ensure correct order
    thumbnails.sort_by(|a, b| {
        a.file_name()
            .and_then(|n| n.to_str())
            .cmp(&b.file_name().and_then(|n| n.to_str()))
    });

    println!("Generated {} thumbnails for timeline scrubbing (requested: {})", thumbnails.len(), thumbnail_count);

    // Cache the generated thumbnails
    if let Ok(mut cache) = thumbnail_cache.write() {
        cache.insert(cache_key, thumbnails.clone());

        // Limit cache size to prevent excessive memory usage
        if cache.len() > 20 {
            // Remove oldest entries
            let keys_to_remove: Vec<_> = cache.keys().take(cache.len() - 10).cloned().collect();
            for key in keys_to_remove {
                cache.remove(&key);
            }
        }
    }

    Ok(thumbnails)
}
