//! Platform-neutral video recording and encoding loop.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::encoder::{Encoder, EncoderConfig};
use crate::muxer::Mp4Muxer;

/// Spawn the video recording task
pub fn spawn_video_task(
    mut frame_rx: tokio::sync::broadcast::Receiver<crate::capture::backends::CapturedFrame>,
    output_path: PathBuf,
    fps: u32,
    stop_signal: Arc<Mutex<bool>>,
) -> tokio::task::JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        println!("Recording task started");
        let mut frame_count = 0u64;

        // Lazily initialize encoder + muxer on first frame
        let mut encoder: Option<Encoder> = None;
        let mut muxer: Option<Mp4Muxer> = None;
        let mut last_pts_us: Option<u64> = None;
        let mut error_msg: Option<String> = None;

        let mut written_frame_count = 0u64;
        let mut should_stop = false;
        let mut first_frame_received_at: Option<std::time::Instant> = None;
        let mut stop_requested_at: Option<std::time::Instant> = None;

        loop {
            // Check stop signal
            if *stop_signal.lock().await {
                println!("Stop signal received, draining remaining frames before finalizing...");
                should_stop = true;
                stop_requested_at.get_or_insert_with(std::time::Instant::now);
            }

            // Receive frame with timeout
            let timeout_ms = if should_stop { 50 } else { 100 };
            let next = tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                frame_rx.recv(),
            )
            .await;

            let frame = match next {
                Ok(Ok(f)) => f,
                Ok(Err(e)) => {
                    let stopping = should_stop || *stop_signal.lock().await;
                    if stopping {
                        println!("Frame channel closed during shutdown");
                    } else {
                        let err = format!("Frame receiver error: {}", e);
                        eprintln!("{}", err);
                        error_msg = Some(err);
                    }
                    break;
                }
                Err(_) => {
                    if should_stop {
                        println!("Frame drain timeout, no more frames pending");
                        break;
                    }
                    continue;
                }
            };

            frame_count += 1;
            first_frame_received_at.get_or_insert_with(std::time::Instant::now);
            if frame_count % 60 == 0 {
                println!("Captured {} frames", frame_count);
            }

            // Initialize encoder and muxer on first frame
            if encoder.is_none() || muxer.is_none() {
                let cfg = EncoderConfig {
                    width: frame.width,
                    height: frame.height,
                    fps,
                    bitrate: 0,
                    hardware_accel: true,
                };
                let mut enc = match Encoder::new(cfg, &output_path) {
                    Ok(e) => e,
                    Err(e) => {
                        let err = format!("Failed to create encoder: {}", e);
                        eprintln!("{}", err);
                        error_msg = Some(err);
                        break;
                    }
                };
                if let Err(e) = enc.start() {
                    let err = format!("Failed to start encoder: {}", e);
                    eprintln!("{}", err);
                    error_msg = Some(err);
                    break;
                }

                let m = match Mp4Muxer::new(&output_path, frame.width, frame.height, fps) {
                    Ok(m) => m,
                    Err(e) => {
                        let err = format!("Failed to create muxer: {}", e);
                        eprintln!("{}", err);
                        error_msg = Some(err);
                        break;
                    }
                };

                encoder = Some(enc);
                muxer = Some(m);
            }

            // Encode raw frame
            if let Some(enc) = encoder.as_mut() {
                if let Err(e) = enc.encode_frame(
                    &frame.data,
                    frame.width,
                    frame.height,
                    frame.stride,
                    &frame.pixel_format,
                    frame.timestamp_us,
                ) {
                    let err = format!("Failed to encode frame: {}", e);
                    eprintln!("{}", err);
                    error_msg = Some(err);
                    break;
                }
            }

            // Drain any available encoded frames and write to muxer
            if let (Some(enc), Some(mux)) = (encoder.as_mut(), muxer.as_mut()) {
                while let Some(encoded) = enc.try_receive_frame() {
                    if encoded.data.len() < 5 {
                        eprintln!(
                            "WARNING: Encoded frame is suspiciously small: {} bytes",
                            encoded.data.len()
                        );
                    }

                    if encoded.is_keyframe && (encoded.sps.is_some() || encoded.pps.is_some()) {
                        println!(
                            "Keyframe with parameter sets: SPS={} bytes, PPS={} bytes",
                            encoded.sps.as_ref().map_or(0, |s| s.len()),
                            encoded.pps.as_ref().map_or(0, |p| p.len())
                        );
                    }

                    let duration_ms = if let Some(prev) = last_pts_us.replace(encoded.timestamp_us)
                    {
                        let delta_us = encoded.timestamp_us.saturating_sub(prev);
                        ((delta_us + 500) / 1000) as u32
                    } else {
                        (1000 / fps.max(1)) as u32
                    };

                    if let Err(e) = mux.write_frame(&encoded, duration_ms) {
                        let err = format!("Failed to write encoded frame: {}", e);
                        eprintln!("{}", err);
                        error_msg = Some(err);
                        break;
                    }
                    written_frame_count += 1;
                }
                if error_msg.is_some() {
                    break;
                }
            }
        }

        println!(
            "Recording task finishing, captured {} frames total, written {} frames so far",
            frame_count, written_frame_count
        );

        // Drain and finalize
        let (finalize_result, _enc_for_flush) = super::finalization::drain_encoder_frames(
            encoder.as_mut(),
            muxer.as_mut(),
            &mut last_pts_us,
            frame_count,
            written_frame_count,
            fps,
        );

        if let Some(err) = finalize_result {
            if error_msg.is_none() {
                error_msg = Some(err);
            }
        }

        // Flush encoder and drain final frames
        if let Some(mut enc) = encoder {
            if let Some(err) =
                super::finalization::flush_encoder(&mut enc, muxer.as_mut(), &mut last_pts_us, fps)
            {
                if error_msg.is_none() {
                    error_msg = Some(err);
                }
            }
        }

        // Finalize muxer
        if let Some(mux) = muxer {
            let target_duration_ms = first_frame_received_at.map(|started_at| {
                stop_requested_at
                    .unwrap_or_else(std::time::Instant::now)
                    .saturating_duration_since(started_at)
                    .as_millis()
                    .min(u64::MAX as u128) as u64
            });
            if let Some(err) =
                super::finalization::finalize_muxer(mux, &output_path, target_duration_ms)
            {
                if error_msg.is_none() {
                    error_msg = Some(err);
                }
            }
        }

        let is_successful = output_path.exists() && frame_count > 0;
        if !is_successful && error_msg.is_none() {
            error_msg = Some(format!(
                "Screen capture ended without frames; no recording file was created at {}",
                output_path.display()
            ));
        }

        if let Some(err) = error_msg {
            if is_successful {
                println!(
                    "Recording task completed successfully ({} frames written, MP4 created)",
                    frame_count
                );
                Ok(())
            } else {
                eprintln!("Recording task completed with error: {}", err);
                Err(err)
            }
        } else {
            println!("Recording task completed successfully");
            Ok(())
        }
    })
}
