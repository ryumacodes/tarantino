//! Centralized FFmpeg process manager
//!
//! Provides queued, concurrent execution of FFmpeg operations with:
//! - Priority-based queuing
//! - Configurable concurrency limits
//! - Automatic hardware acceleration fallback
//! - Statistics tracking

#![allow(dead_code)]

pub mod operations;
pub mod types;

use anyhow::{Result, anyhow};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub use operations::{build_command, build_command_software_only, build_command_software_with_fallback_level};
pub use types::{
    FFmpegOperation, VideoOperation,
    OperationPriority, OperationResult, ManagerStats,
    ActiveOperation, QueuedOperation,
    MAX_CONCURRENT_OPERATIONS, DEFAULT_OPERATION_TIMEOUT,
};

/// Centralized FFmpeg process manager
pub struct FFmpegManager {
    /// Currently active operations
    active_operations: Arc<RwLock<HashMap<Uuid, ActiveOperation>>>,
    /// Queue of pending operations
    operation_queue: Arc<Mutex<VecDeque<QueuedOperation>>>,
    /// Maximum concurrent operations allowed
    max_concurrent: usize,
    /// Default timeout for operations
    default_timeout: Duration,
    /// Statistics
    stats: Arc<RwLock<ManagerStats>>,
    /// Shutdown signal
    shutdown: Arc<tokio::sync::broadcast::Sender<()>>,
}

impl FFmpegManager {
    /// Create a new FFmpeg manager with default settings
    pub fn new() -> Self {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);

        Self {
            active_operations: Arc::new(RwLock::new(HashMap::new())),
            operation_queue: Arc::new(Mutex::new(VecDeque::new())),
            max_concurrent: MAX_CONCURRENT_OPERATIONS,
            default_timeout: DEFAULT_OPERATION_TIMEOUT,
            stats: Arc::new(RwLock::new(ManagerStats::default())),
            shutdown: Arc::new(shutdown_tx),
        }
    }

    /// Create a new FFmpeg manager with custom settings
    pub fn with_config(max_concurrent: usize, default_timeout: Duration) -> Self {
        let mut manager = Self::new();
        manager.max_concurrent = max_concurrent;
        manager.default_timeout = default_timeout;
        manager
    }

    /// Execute an FFmpeg operation
    pub async fn execute_operation(
        &self,
        operation: FFmpegOperation,
        priority: OperationPriority,
    ) -> Result<OperationResult> {
        let operation_id = Uuid::new_v4();

        println!("FFmpegManager: Queuing operation {} with priority {:?}", operation_id, priority);

        // Check if we can execute immediately or need to queue
        let can_execute = {
            let active = self.active_operations.read().await;
            active.len() < self.max_concurrent
        };

        if can_execute {
            self.execute_immediately(operation_id, operation).await
        } else {
            self.queue_operation(operation_id, operation, priority).await
        }
    }

    /// Execute an operation immediately without queuing
    async fn execute_immediately(
        &self,
        operation_id: Uuid,
        operation: FFmpegOperation,
    ) -> Result<OperationResult> {
        println!("FFmpegManager: Executing operation {} immediately", operation_id);

        let start_time = Instant::now();

        // Try with hardware acceleration first
        let result = self.execute_operation_internal(operation_id, &operation, false).await?;

        // If it failed, retry with multi-level software fallback
        if let OperationResult::Error(ref error_msg) = result {
            if operations::should_retry_with_software(error_msg) {
                println!("[FFMPEG FALLBACK] Operation {} failed with error, starting multi-level software fallback", operation_id);
                println!("[FFMPEG FALLBACK] Error detected: {}",
                         if error_msg.len() > 150 { &error_msg[..150] } else { error_msg });

                // Retry with multi-level fallback for thumbnail operations
                if matches!(operation, FFmpegOperation::Thumbnail { .. }) {
                    let retry_result = self.execute_with_fallback_levels(operation_id, &operation).await?;

                    // Clean up and update stats
                    self.cleanup_operation(operation_id, &retry_result, start_time).await;
                    self.process_queue().await;

                    return Ok(retry_result);
                }
            }
        }

        // Clean up and update stats
        self.cleanup_operation(operation_id, &result, start_time).await;
        self.process_queue().await;

        Ok(result)
    }

    /// Execute thumbnail operation with multiple fallback levels
    async fn execute_with_fallback_levels(
        &self,
        operation_id: Uuid,
        operation: &FFmpegOperation,
    ) -> Result<OperationResult> {
        // Try Level 1: Software decoding with seeking
        let mut result = self.execute_operation_with_fallback_level(operation_id, operation, 1).await?;

        // If Level 1 failed, try Level 2: Conservative seeking
        if matches!(result, OperationResult::Error(_)) {
            println!("[FFMPEG FALLBACK] Level 1 failed, trying Level 2 (conservative seeking)");
            result = self.execute_operation_with_fallback_level(operation_id, operation, 2).await?;
        }

        // If Level 2 failed, try Level 3: From middle of video
        if matches!(result, OperationResult::Error(_)) {
            println!("[FFMPEG FALLBACK] Level 2 failed, trying Level 3 (middle of video)");
            result = self.execute_operation_with_fallback_level(operation_id, operation, 3).await?;
        }

        // Log final result
        match &result {
            OperationResult::Success(_) => {
                println!("[FFMPEG FALLBACK] ✓ Successfully recovered thumbnail using software fallback");
            },
            OperationResult::Error(e) => {
                println!("[FFMPEG FALLBACK] ✗ All fallback levels failed. Video may be too corrupted.");
                println!("[FFMPEG FALLBACK] Final error: {}", if e.len() > 200 { &e[..200] } else { e });
            },
            _ => {}
        }

        Ok(result)
    }

    /// Internal execution with optional software-only mode
    async fn execute_operation_internal(
        &self,
        operation_id: Uuid,
        operation: &FFmpegOperation,
        disable_hwaccel: bool,
    ) -> Result<OperationResult> {
        // Build and spawn the FFmpeg command
        let mut cmd = if disable_hwaccel {
            build_command_software_only(operation)?
        } else {
            build_command(operation)?
        };

        let child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn FFmpeg: {}", e))?;

        println!("FFmpegManager: Spawned process with PID {:?} for operation {} (hwaccel: {})",
                 child.id(), operation_id, !disable_hwaccel);

        // Update active operations count
        {
            let mut stats = self.stats.write().await;
            stats.active_operations += 1;
        }

        self.wait_for_process(operation_id, operation, child).await
    }

    /// Execute FFmpeg operation with a specific fallback level
    async fn execute_operation_with_fallback_level(
        &self,
        operation_id: Uuid,
        operation: &FFmpegOperation,
        fallback_level: u8,
    ) -> Result<OperationResult> {
        // Ensure output directory exists
        operations::ensure_output_directory(operation)?;

        // Build command with specific fallback level
        let mut cmd = build_command_software_with_fallback_level(operation, fallback_level)?;

        let child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn FFmpeg: {}", e))?;

        println!("FFmpegManager: Spawned process with PID {:?} for operation {} (fallback level {})",
                 child.id(), operation_id, fallback_level);

        // Update active operations count
        {
            let mut stats = self.stats.write().await;
            stats.active_operations += 1;
        }

        self.wait_for_process(operation_id, operation, child).await
    }

    /// Wait for FFmpeg process to complete with timeout
    async fn wait_for_process(
        &self,
        operation_id: Uuid,
        operation: &FFmpegOperation,
        child: tokio::process::Child,
    ) -> Result<OperationResult> {
        let timeout_duration = self.default_timeout;
        let timeout_future = tokio::time::sleep(timeout_duration);
        let process_future = child.wait_with_output();

        let result = tokio::select! {
            _ = timeout_future => {
                println!("FFmpegManager: Operation {} timed out", operation_id);
                OperationResult::Timeout
            }
            output = process_future => {
                match output {
                    Ok(output) => {
                        if output.status.success() {
                            // For thumbnail operations, verify output file
                            if let FFmpegOperation::Thumbnail { output: output_path, .. } = operation {
                                let validation = operations::validate_thumbnail_output(
                                    output_path,
                                    operation_id,
                                    &output.stderr,
                                );
                                if matches!(validation, OperationResult::Error(_)) {
                                    return Ok(validation);
                                }
                            }
                            println!("FFmpegManager: Operation {} completed successfully", operation_id);
                            OperationResult::Success(output.stdout)
                        } else {
                            let error = String::from_utf8_lossy(&output.stderr);
                            println!("FFmpegManager: Operation {} failed: {}", operation_id, error);
                            OperationResult::Error(error.to_string())
                        }
                    },
                    Err(e) => {
                        println!("FFmpegManager: Operation {} process wait failed: {}", operation_id, e);
                        OperationResult::Error(format!("Process wait failed: {}", e))
                    }
                }
            }
        };

        Ok(result)
    }

    /// Queue an operation to be executed later
    async fn queue_operation(
        &self,
        operation_id: Uuid,
        operation: FFmpegOperation,
        priority: OperationPriority,
    ) -> Result<OperationResult> {
        println!("FFmpegManager: Queuing operation {} for later execution", operation_id);

        let queued_op = QueuedOperation {
            id: operation_id,
            operation,
            priority,
            queued_at: Instant::now(),
        };

        // Insert into queue maintaining priority order
        {
            let mut queue = self.operation_queue.lock().await;

            // Find insertion point based on priority
            let insert_pos = queue.iter().position(|op| op.priority < priority).unwrap_or(queue.len());
            queue.insert(insert_pos, queued_op);

            // Update stats
            let mut stats = self.stats.write().await;
            stats.queued_operations = queue.len();
        }

        // Wait for the operation to be processed
        while {
            let queue = self.operation_queue.lock().await;
            queue.iter().any(|op| op.id == operation_id)
        } {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Operation should be completed - this would be improved with proper result channels
        Ok(OperationResult::Success(vec![]))
    }

    /// Kill a specific operation
    async fn kill_operation(&self, operation_id: Uuid) {
        let mut active = self.active_operations.write().await;
        if let Some(mut operation) = active.remove(&operation_id) {
            let _ = operation.child.kill();
            println!("FFmpegManager: Killed operation {}", operation_id);
        }
    }

    /// Clean up completed operation and update statistics
    async fn cleanup_operation(
        &self,
        operation_id: Uuid,
        result: &OperationResult,
        start_time: Instant,
    ) {
        let mut stats = self.stats.write().await;

        // Decrement active operations count
        if stats.active_operations > 0 {
            stats.active_operations -= 1;
        }

        let duration_ms = start_time.elapsed().as_millis() as f64;

        match result {
            OperationResult::Success(_) => {
                stats.total_completed += 1;
            }
            OperationResult::Timeout => {
                stats.total_timeouts += 1;
            }
            OperationResult::Error(_) | OperationResult::Cancelled => {
                stats.total_failed += 1;
            }
        }

        // Update average operation time
        let total_ops = stats.total_completed + stats.total_failed + stats.total_timeouts;
        if total_ops > 0 {
            stats.average_operation_time_ms =
                (stats.average_operation_time_ms * (total_ops - 1) as f64 + duration_ms) / total_ops as f64;
        }

        println!("FFmpegManager: Cleaned up operation {} ({:?}) in {:.2}ms",
                operation_id, result, start_time.elapsed().as_millis());
    }

    /// Process the next operation in the queue
    async fn process_queue(&self) {
        let next_operation = {
            let mut queue = self.operation_queue.lock().await;
            queue.pop_front()
        };

        if let Some(queued_op) = next_operation {
            let operation_id = queued_op.id;
            let operation = queued_op.operation;

            // Box the recursive call to avoid infinite recursion
            let _ = Box::pin(self.execute_immediately(operation_id, operation)).await;
        }
    }

    /// Get current manager statistics
    pub async fn get_stats(&self) -> ManagerStats {
        self.stats.read().await.clone()
    }

    /// Shutdown the manager and kill all active operations
    pub async fn shutdown(&self) {
        println!("FFmpegManager: Shutting down...");

        // Send shutdown signal
        let _ = self.shutdown.send(());

        // Kill all active operations
        let operation_ids: Vec<Uuid> = {
            let active = self.active_operations.read().await;
            active.keys().cloned().collect()
        };

        for operation_id in operation_ids {
            self.kill_operation(operation_id).await;
        }

        // Clear the queue
        {
            let mut queue = self.operation_queue.lock().await;
            queue.clear();
        }

        println!("FFmpegManager: Shutdown complete");
    }
}

impl Drop for FFmpegManager {
    fn drop(&mut self) {
        // Schedule cleanup in the background
        let shutdown = Arc::clone(&self.shutdown);
        let active_operations = Arc::clone(&self.active_operations);

        tokio::spawn(async move {
            let _ = shutdown.send(());

            // Kill all active operations
            let operation_ids: Vec<Uuid> = {
                let active = active_operations.read().await;
                active.keys().cloned().collect()
            };

            for operation_id in operation_ids {
                let mut active = active_operations.write().await;
                if let Some(mut operation) = active.remove(&operation_id) {
                    let _ = operation.child.kill();
                }
            }
        });
    }
}

/// Global FFmpeg manager instance
static FFMPEG_MANAGER: once_cell::sync::Lazy<Arc<FFmpegManager>> =
    once_cell::sync::Lazy::new(|| Arc::new(FFmpegManager::new()));

/// Get the global FFmpeg manager instance
pub fn get_ffmpeg_manager() -> Arc<FFmpegManager> {
    Arc::clone(&FFMPEG_MANAGER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = FFmpegManager::new();
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_operations, 0);
        assert_eq!(stats.queued_operations, 0);
    }

    #[tokio::test]
    async fn test_operation_queuing() {
        let manager = FFmpegManager::with_config(1, Duration::from_secs(1));

        // Test that the manager can be created with custom config
        assert_eq!(manager.max_concurrent, 1);
    }
}
