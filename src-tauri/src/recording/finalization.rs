//! Encoder finalization and frame draining
//!
//! Handles draining remaining frames from the encoder and finalizing the MP4 muxer.

use std::path::Path;

#[cfg(target_os = "macos")]
use crate::encoder::Encoder;
#[cfg(target_os = "macos")]
use crate::muxer::Mp4Muxer;

/// Drain remaining encoded frames from the encoder
#[cfg(target_os = "macos")]
pub fn drain_encoder_frames(
    encoder: Option<&mut Encoder>,
    muxer: Option<&mut Mp4Muxer>,
    last_pts_us: &mut Option<u64>,
    expected_frames: u64,
    mut written_frame_count: u64,
    fps: u32,
) -> (Option<String>, Option<()>) {
    let (enc, mux) = match (encoder, muxer) {
        (Some(e), Some(m)) => (e, m),
        _ => return (None, None),
    };

    let remaining_frames = expected_frames.saturating_sub(written_frame_count);
    let calculated_timeout = ((remaining_frames as f64 / 30.0).ceil() as u64 + 5).max(5);
    let max_drain_time = std::time::Duration::from_secs(calculated_timeout);

    println!(
        "Draining encoder output queue (expecting {} more frames for total {}, timeout: {}s)...",
        remaining_frames, expected_frames, calculated_timeout
    );

    let mut drained_in_phase = 0;
    let drain_start = std::time::Instant::now();
    let mut consecutive_empty_checks = 0;
    const MAX_CONSECUTIVE_EMPTY: u32 = 50; // 50 * 50ms = 2.5 seconds of no frames

    while drain_start.elapsed() < max_drain_time {
        if let Some(encoded) = enc.try_receive_frame() {
            drained_in_phase += 1;
            written_frame_count += 1;
            consecutive_empty_checks = 0;

            if encoded.data.len() < 5 {
                eprintln!(
                    "WARNING: Drained frame is suspiciously small: {} bytes",
                    encoded.data.len()
                );
            }

            let duration_ms = if let Some(prev) = last_pts_us.replace(encoded.timestamp_us) {
                let delta_us = encoded.timestamp_us.saturating_sub(prev);
                ((delta_us + 500) / 1000) as u32
            } else {
                (1000 / fps.max(1)) as u32
            };

            if let Err(e) = mux.write_frame(&encoded, duration_ms) {
                let err = format!("Failed to write encoded frame during drain: {}", e);
                eprintln!("{}", err);
                return (Some(err), Some(()));
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(50));
            consecutive_empty_checks += 1;

            if written_frame_count >= expected_frames {
                println!("Drained all {} expected frames", expected_frames);
                break;
            }

            if consecutive_empty_checks >= MAX_CONSECUTIVE_EMPTY && written_frame_count > 0 {
                let received_pct = (written_frame_count as f64 / expected_frames as f64) * 100.0;
                if received_pct >= 95.0 {
                    println!(
                        "Drained {:.1}% of frames ({}/{}), considering complete after {:.1}s wait",
                        received_pct,
                        written_frame_count,
                        expected_frames,
                        (consecutive_empty_checks * 50) as f64 / 1000.0
                    );
                    break;
                }
            }
        }
    }

    println!(
        "First drain complete: {} frames in this phase, {}/{} total ({:.1}%)",
        drained_in_phase,
        written_frame_count,
        expected_frames,
        (written_frame_count as f64 / expected_frames as f64) * 100.0
    );

    if written_frame_count < expected_frames {
        eprintln!(
            "WARNING: Only drained {}/{} frames before timeout",
            written_frame_count, expected_frames
        );
    }

    (None, Some(()))
}

/// Flush the encoder and drain any final frames
#[cfg(target_os = "macos")]
pub fn flush_encoder(
    encoder: &mut Encoder,
    muxer: Option<&mut Mp4Muxer>,
    last_pts_us: &mut Option<u64>,
    fps: u32,
) -> Option<String> {
    println!("Flushing encoder...");
    if let Err(e) = encoder.finish() {
        let err = format!("Failed to flush encoder: {}", e);
        eprintln!("{}", err);
        return Some(err);
    }

    // After flushing, drain any final frames the encoder produced
    if let Some(mux) = muxer {
        println!("Draining final frames after encoder flush...");
        let mut final_drain_count = 0;
        let final_drain_start = std::time::Instant::now();

        while final_drain_start.elapsed() < std::time::Duration::from_secs(2) {
            if let Some(encoded) = encoder.try_receive_frame() {
                final_drain_count += 1;

                let duration_ms = if let Some(prev) = last_pts_us.replace(encoded.timestamp_us) {
                    let delta_us = encoded.timestamp_us.saturating_sub(prev);
                    ((delta_us + 500) / 1000) as u32
                } else {
                    (1000 / fps.max(1)) as u32
                };

                if let Err(e) = mux.write_frame(&encoded, duration_ms) {
                    let err = format!("Failed to write final encoded frame: {}", e);
                    eprintln!("{}", err);
                    return Some(err);
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));

                if final_drain_count == 0
                    && final_drain_start.elapsed() > std::time::Duration::from_millis(200)
                {
                    break;
                }
            }
        }

        if final_drain_count > 0 {
            println!(
                "Drained {} final frames after encoder flush",
                final_drain_count
            );
        }
    }

    None
}

/// Finalize the MP4 muxer
#[cfg(target_os = "macos")]
pub fn finalize_muxer(muxer: Mp4Muxer, output_path: &Path) -> Option<String> {
    println!("Finalizing MP4 muxer...");
    if let Err(e) = muxer.finish() {
        let err = format!("Failed to finalize muxer: {}", e);
        eprintln!("{}", err);
        return Some(err);
    }

    // Verify MP4 file was created
    if output_path.exists() {
        match std::fs::metadata(output_path) {
            Ok(metadata) => {
                println!(
                    "MP4 file created successfully: {} ({} bytes)",
                    output_path.display(),
                    metadata.len()
                );
            }
            Err(e) => {
                println!("Warning: Failed to read MP4 metadata: {}", e);
            }
        }
    } else {
        let err = format!(
            "MP4 file not found after muxer finish: {}",
            output_path.display()
        );
        eprintln!("{}", err);
        return Some(err);
    }

    None
}
