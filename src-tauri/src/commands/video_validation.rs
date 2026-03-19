// Video validation utilities

use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Wait for a video file to be ready and validated
pub async fn wait_for_file_ready(path: &Path, max_wait: tokio::time::Duration) -> bool {
    let start = std::time::Instant::now();
    let mut last_probe_attempt = std::time::Instant::now();
    let mut file_found = false;

    loop {
        if let Ok(meta) = tokio::fs::metadata(path).await {
            if meta.len() > 0 {
                if !file_found {
                    println!("File found: {} ({} bytes), validating with ffprobe...", path.display(), meta.len());
                    file_found = true;
                }

                if last_probe_attempt.elapsed() >= tokio::time::Duration::from_millis(500) {
                    let probe_result = tokio::process::Command::new("ffprobe")
                        .args(["-v", "error", "-select_streams", "v:0", "-count_packets",
                               "-show_entries", "stream=nb_read_packets", "-of", "csv=p=0",
                               path.to_string_lossy().as_ref()])
                        .output()
                        .await;

                    match probe_result {
                        Ok(output) if output.status.success() => {
                            println!("File is ready and validated: {} ({} bytes)", path.display(), meta.len());
                            return true;
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            println!("File validation failed, retrying: {}", stderr.lines().next().unwrap_or("unknown error"));
                        }
                        Err(e) => println!("Failed to run ffprobe validation: {}", e),
                    }
                    last_probe_attempt = std::time::Instant::now();
                }
            }
        }

        if start.elapsed() > max_wait {
            println!("Timeout waiting for file{}: {}", if file_found { " validation" } else { "" }, path.display());
            return false;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
}

/// Validate a video file exists and is readable
pub fn validate_video_file(path: &str) -> Result<bool> {
    let path = Path::new(path);

    if !path.exists() {
        println!("Video file does not exist: {}", path.display());
        return Ok(false);
    }

    // Check file size
    let metadata = std::fs::metadata(path)?;
    if metadata.len() == 0 {
        println!("Video file is empty: {}", path.display());
        return Ok(false);
    }

    // Try to probe the file with ffprobe
    let probe_result = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=codec_name,width,height,duration",
            "-of", "default=noprint_wrappers=1",
            &path.to_string_lossy()
        ])
        .output();

    match probe_result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("Video file validated: {}", stdout.lines().next().unwrap_or("unknown"));
            Ok(true)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("Video validation failed: {}", stderr);
            Ok(false)
        }
        Err(e) => {
            println!("Failed to run ffprobe: {}", e);
            Ok(false)
        }
    }
}
