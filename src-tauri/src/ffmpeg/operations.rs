//! FFmpeg command building and execution logic

use anyhow::{Result, anyhow};
use std::process::Stdio;
use tokio::process::Command as AsyncCommand;

use super::types::{FFmpegOperation, VideoOperation};

/// Build FFmpeg command for the given operation
pub fn build_command(operation: &FFmpegOperation) -> Result<AsyncCommand> {
    let mut cmd = match operation {
        FFmpegOperation::Thumbnail { input, output, time_offset, width } => {
            let mut cmd = AsyncCommand::new("ffmpeg");
            cmd.args(["-y"]); // Overwrite output

            // Explicitly disable hardware acceleration for thumbnails to prevent concurrency issues
            cmd.args(["-hwaccel", "none"]);

            // Seek before input for faster thumbnail generation
            cmd.args(["-ss", &format!("{:.3}", time_offset)]);

            // Input file
            cmd.args(["-i", input.to_string_lossy().as_ref()]);

            // Output options with explicit full-range pixel format for mjpeg compatibility
            cmd.args([
                "-vf", &format!("scale={}:-1:flags=fast_bilinear,format=yuvj420p", width),
                "-vframes", "1",
                "-c:v", "mjpeg",
                "-q:v", "2",
                "-f", "image2",
                "-update", "1",
                output.to_string_lossy().as_ref()
            ]);

            cmd
        },

        FFmpegOperation::Probe { input } => {
            let mut cmd = AsyncCommand::new("ffprobe");
            cmd.args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
                "-select_streams", "v:0",
                input.to_string_lossy().as_ref()
            ]);
            cmd
        },

        FFmpegOperation::Record { input: _, output, config } => {
            let mut cmd = AsyncCommand::new("ffmpeg");
            cmd.arg("-f").arg("avfoundation");
            cmd.arg("-framerate").arg(config.fps.to_string());
            cmd.arg("-capture_cursor").arg("0");
            cmd.arg("-capture_mouse_clicks").arg("0");

            let screen_input = if config.mic_enabled {
                format!("{}:0", config.screen_device_index)
            } else {
                config.screen_device_index.to_string()
            };

            cmd.arg("-i").arg(screen_input);
            cmd.arg("-vcodec").arg(&config.codec);
            cmd.arg("-preset").arg(&config.preset);
            cmd.arg("-crf").arg(config.crf.to_string());

            if config.mic_enabled {
                cmd.arg("-acodec").arg("aac");
                cmd.arg("-ac").arg("2");
                cmd.arg("-ar").arg("44100");
            }

            cmd.arg("-y").arg(output);
            cmd
        },

        FFmpegOperation::Export { input, output, settings: _ } => {
            // Simplified export for now
            let mut cmd = AsyncCommand::new("ffmpeg");
            cmd.args(["-y", "-i", input.to_string_lossy().as_ref()]);
            cmd.arg(output);
            cmd
        },

        FFmpegOperation::Process { input, output, operations } => {
            let mut cmd = AsyncCommand::new("ffmpeg");
            cmd.args(["-y"]);

            #[cfg(target_os = "macos")]
            cmd.args(["-hwaccel", "videotoolbox"]);

            cmd.arg("-i").arg(input);

            // Build filter chain
            let filters = build_filter_chain(operations);

            if !filters.is_empty() {
                cmd.args(["-vf", &filters]);
            }

            // Add faststart flags for web optimization
            cmd.args(["-movflags", "+faststart"]);

            cmd.arg(output);
            cmd
        }
    };

    // Set up stdio
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    Ok(cmd)
}

/// Build FFmpeg command with software decoding only (no hardware acceleration)
pub fn build_command_software_only(operation: &FFmpegOperation) -> Result<AsyncCommand> {
    build_command_software_with_fallback_level(operation, 1)
}

/// Build FFmpeg command with software decoding with specific fallback level
/// Level 1: Software decoding with seeking and simple scaling
/// Level 2: Software decoding with conservative seeking
/// Level 3: Extract from middle of video with maximum error tolerance
pub fn build_command_software_with_fallback_level(
    operation: &FFmpegOperation,
    fallback_level: u8,
) -> Result<AsyncCommand> {
    let mut cmd = match operation {
        FFmpegOperation::Thumbnail { input, output, time_offset, width } => {
            let mut cmd = AsyncCommand::new("ffmpeg");
            cmd.args(["-y"]); // Overwrite output

            // Explicitly disable hardware acceleration
            cmd.args(["-hwaccel", "none"]);

            // Add error resilience flags to handle corrupted NAL units
            cmd.args([
                "-err_detect", "ignore_err",
                "-skip_frame", "nokey",
            ]);

            match fallback_level {
                1 => {
                    // Level 1: Software with seeking (original approach)
                    cmd.args(["-ss", &format!("{:.3}", time_offset)]);
                    println!("[FFMPEG FALLBACK L1] Software decoder with seeking (time offset: {:.3}s)", time_offset);
                },
                2 => {
                    // Level 2: Software with more conservative seeking
                    let conservative_offset = (time_offset - 0.5).max(0.0);
                    cmd.args(["-ss", &format!("{:.3}", conservative_offset)]);
                    println!("[FFMPEG FALLBACK L2] Software decoder with conservative seeking (time offset: {:.3}s -> {:.3}s)", time_offset, conservative_offset);
                },
                3 => {
                    // Level 3: Try from middle of video instead
                    let middle_time = time_offset / 2.0;
                    cmd.args(["-ss", &format!("{:.3}", middle_time)]);
                    println!("[FFMPEG FALLBACK L3] Software decoder from middle (time offset: {:.3}s)", middle_time);
                },
                _ => {
                    println!("[FFMPEG FALLBACK] Unknown level {}, using L1", fallback_level);
                    cmd.args(["-ss", &format!("{:.3}", time_offset)]);
                }
            }

            // Input file
            cmd.args(["-i", input.to_string_lossy().as_ref()]);

            // Output options
            cmd.args([
                "-vf", &format!("scale={}:-1:flags=fast_bilinear,format=yuvj420p", width),
                "-vframes", "1",
                "-c:v", "mjpeg",
                "-q:v", "2",
                "-f", "image2",
                "-update", "1",
                output.to_string_lossy().as_ref()
            ]);

            cmd
        },

        // For other operations, just use the regular build_command
        _ => return build_command(operation),
    };

    // Set up stdio
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    Ok(cmd)
}

/// Build a filter chain string from video operations
fn build_filter_chain(operations: &[VideoOperation]) -> String {
    let filters: Vec<String> = operations.iter().map(|op| {
        match op {
            VideoOperation::Scale { width, height } => {
                format!("scale={}:{}", width, height)
            }
            VideoOperation::Crop { x, y, width, height } => {
                format!("crop={}:{}:{}:{}", width, height, x, y)
            }
            VideoOperation::Rotate { degrees } => {
                format!("rotate={}*PI/180", degrees)
            }
        }
    }).collect();

    filters.join(",")
}

/// Check if an error message indicates a need for software fallback
pub fn should_retry_with_software(error_msg: &str) -> bool {
    error_msg.contains("hardware accelerator failed")
        || error_msg.contains("vt decoder cb: output image buffer is null")
        || error_msg.contains("Error submitting packet to decoder")
        || error_msg.contains("No frame decoded")
        // H.264 NAL corruption errors
        || error_msg.contains("missing picture in access unit")
        || error_msg.contains("no frame!")
        || error_msg.contains("data partitioning not implemented")
        || (error_msg.contains("SEI type") && error_msg.contains("truncated"))
        // General H.264 decoding errors
        || error_msg.contains("decode_slice_header error")
        || error_msg.contains("error while decoding MB")
        || error_msg.contains("concealing")
        // File I/O errors
        || error_msg.contains("Output file not created")
        || error_msg.contains("Could not open file")
}

/// Ensure output directory exists for an operation
pub fn ensure_output_directory(operation: &FFmpegOperation) -> Result<()> {
    if let FFmpegOperation::Thumbnail { output, .. } = operation {
        if let Some(parent) = output.parent() {
            if !parent.exists() {
                println!("FFmpegManager: Creating output directory: {:?}", parent);
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("Failed to create output directory: {}", e))?;
            }
        }
    }
    Ok(())
}

/// Validate thumbnail output file
pub fn validate_thumbnail_output(output_path: &std::path::Path, operation_id: uuid::Uuid, stderr: &[u8]) -> super::types::OperationResult {
    use super::types::OperationResult;

    if !output_path.exists() {
        let stderr_str = String::from_utf8_lossy(stderr);
        println!("FFmpegManager: Operation {} reported success but output file not created. stderr: {}",
                 operation_id, stderr_str);
        return OperationResult::Error(format!("Output file not created. FFmpeg stderr: {}", stderr_str));
    }

    if let Ok(metadata) = std::fs::metadata(output_path) {
        if metadata.len() < 100 {
            println!("FFmpegManager: Operation {} created suspiciously small file ({} bytes)",
                     operation_id, metadata.len());
            return OperationResult::Error("Output file too small (possibly corrupted)".to_string());
        }
    }

    OperationResult::Success(vec![])
}
