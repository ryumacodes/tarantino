//! FFmpeg operation types and enums

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::process::Child;
use uuid::Uuid;

/// Maximum number of concurrent FFmpeg operations allowed
pub const MAX_CONCURRENT_OPERATIONS: usize = 3;

/// Default timeout for FFmpeg operations
pub const DEFAULT_OPERATION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Types of FFmpeg operations that can be managed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FFmpegOperation {
    /// Extract thumbnail from video at specific time offset
    Thumbnail {
        input: PathBuf,
        output: PathBuf,
        time_offset: f64,
        width: u32,
    },
    /// Probe video metadata using FFprobe
    Probe {
        input: PathBuf,
    },
    /// Record screen/audio using FFmpeg
    Record {
        input: String,
        output: PathBuf,
        config: RecordConfig,
    },
    /// Export/process video with effects
    Export {
        input: PathBuf,
        output: PathBuf,
        settings: ExportSettings,
    },
    /// Process video with operations (scale, crop, etc.)
    Process {
        input: PathBuf,
        output: PathBuf,
        operations: Vec<VideoOperation>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordConfig {
    pub fps: u32,
    pub screen_device_index: u32,
    pub mic_enabled: bool,
    pub codec: String,
    pub preset: String,
    pub crf: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    pub format: String,
    pub quality: String,
    pub fps: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VideoOperation {
    Scale { width: u32, height: u32 },
    Crop { x: u32, y: u32, width: u32, height: u32 },
    Rotate { degrees: f64 },
}

/// Represents an active FFmpeg operation
#[derive(Debug)]
pub struct ActiveOperation {
    pub id: Uuid,
    pub operation: FFmpegOperation,
    pub child: Child,
    pub started_at: Instant,
    pub timeout: Duration,
}

/// Represents a queued operation waiting to be executed
#[derive(Debug)]
pub struct QueuedOperation {
    pub id: Uuid,
    pub operation: FFmpegOperation,
    pub priority: OperationPriority,
    pub queued_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperationPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3, // For recording operations
}

/// Result of an FFmpeg operation
#[derive(Debug, Clone)]
pub enum OperationResult {
    Success(Vec<u8>), // stdout data
    Timeout,
    Error(String),
    Cancelled,
}

/// Statistics about the FFmpeg manager
#[derive(Debug, Clone, Serialize)]
pub struct ManagerStats {
    pub active_operations: usize,
    pub queued_operations: usize,
    pub total_completed: u64,
    pub total_failed: u64,
    pub total_timeouts: u64,
    pub average_operation_time_ms: f64,
}

impl Default for ManagerStats {
    fn default() -> Self {
        Self {
            active_operations: 0,
            queued_operations: 0,
            total_completed: 0,
            total_failed: 0,
            total_timeouts: 0,
            average_operation_time_ms: 0.0,
        }
    }
}
