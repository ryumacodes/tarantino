//! Encoder loop management for recording
//!
//! Handles spawning and managing the video and audio encoding tasks.

#[cfg(target_os = "macos")]
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(target_os = "macos")]
use crate::encoder::{Container, Encoder, EncoderConfig, VideoCodec};
#[cfg(target_os = "macos")]
use crate::muxer::Mp4Muxer;

/// Spawn the audio capture task
#[cfg(target_os = "macos")]
pub fn spawn_audio_task(
    audio_rx: Option<tokio::sync::broadcast::Receiver<crate::capture::backends::CapturedAudio>>,
    audio_path: PathBuf,
    stop_signal: Arc<Mutex<bool>>,
) -> Option<tokio::task::JoinHandle<Result<(), String>>> {
    let mut audio_rx = audio_rx?;

    Some(tokio::spawn(async move {
        println!(
            "Audio capture task started, writing to: {}",
            audio_path.display()
        );

        // WAV file header will be written after we know sample rate/channels
        let mut wav_file: Option<std::io::BufWriter<std::fs::File>> = None;
        #[allow(unused_assignments)]
        let mut sample_rate: u32 = 0;
        #[allow(unused_assignments)]
        let mut channels: u16 = 0;
        let mut data_written: u32 = 0;

        loop {
            // Check stop signal
            if *stop_signal.lock().await {
                println!("Audio stop signal received");
                break;
            }

            // Receive audio with timeout
            let next = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                audio_rx.recv(),
            )
            .await;

            let audio = match next {
                Ok(Ok(a)) => a,
                Ok(Err(_)) => break, // Channel closed
                Err(_) => continue,  // Timeout, check stop signal again
            };

            // Initialize WAV file on first audio sample
            if wav_file.is_none() {
                sample_rate = audio.sample_rate;
                channels = audio.channels as u16;

                match std::fs::File::create(&audio_path) {
                    Ok(file) => {
                        let mut writer = std::io::BufWriter::new(file);
                        // Write WAV header
                        let _ = writer.write_all(b"RIFF");
                        let _ = writer.write_all(&[0u8; 4]); // File size placeholder
                        let _ = writer.write_all(b"WAVE");
                        let _ = writer.write_all(b"fmt ");
                        let _ = writer.write_all(&16u32.to_le_bytes()); // fmt chunk size
                        let _ = writer.write_all(&1u16.to_le_bytes()); // PCM format
                        let _ = writer.write_all(&channels.to_le_bytes());
                        let _ = writer.write_all(&sample_rate.to_le_bytes());
                        let byte_rate = sample_rate * channels as u32 * 2; // 16-bit
                        let _ = writer.write_all(&byte_rate.to_le_bytes());
                        let block_align = channels * 2;
                        let _ = writer.write_all(&block_align.to_le_bytes());
                        let _ = writer.write_all(&16u16.to_le_bytes()); // bits per sample
                        let _ = writer.write_all(b"data");
                        let _ = writer.write_all(&[0u8; 4]); // Data size placeholder
                        wav_file = Some(writer);
                        println!(
                            "WAV file initialized: {}Hz, {} channels",
                            sample_rate, channels
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to create audio file: {}", e);
                        break;
                    }
                }
            }

            // Write audio data
            if let Some(writer) = wav_file.as_mut() {
                if let Err(e) = writer.write_all(&audio.data) {
                    eprintln!("Failed to write audio data: {}", e);
                } else {
                    data_written += audio.data.len() as u32;
                }
            }
        }

        // Finalize WAV file - update header with actual sizes
        if let Some(mut writer) = wav_file {
            use std::io::Seek;
            let file_size = data_written + 36; // Total size minus 8 bytes for RIFF header
            let _ = writer.seek(std::io::SeekFrom::Start(4));
            let _ = writer.write_all(&file_size.to_le_bytes());
            let _ = writer.seek(std::io::SeekFrom::Start(40));
            let _ = writer.write_all(&data_written.to_le_bytes());
            let _ = writer.flush();
            println!(
                "Audio capture finished: {} bytes written to WAV",
                data_written
            );
        }

        Ok(())
    }))
}

/// Spawn the video recording task
#[cfg(target_os = "macos")]
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

        loop {
            // Check stop signal
            if *stop_signal.lock().await {
                println!("Stop signal received, draining remaining frames before finalizing...");
                should_stop = true;
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
                    codec: VideoCodec::H264,
                    container: Container::Mp4,
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
            if let Some(err) = super::finalization::flush_encoder(
                &mut enc,
                muxer.as_mut(),
                &mut last_pts_us,
                fps,
            ) {
                if error_msg.is_none() {
                    error_msg = Some(err);
                }
            }
        }

        // Finalize muxer
        if let Some(mux) = muxer {
            if let Some(err) = super::finalization::finalize_muxer(mux, &output_path) {
                if error_msg.is_none() {
                    error_msg = Some(err);
                }
            }
        }

        // Check if recording was successful
        let is_successful = output_path.exists() && frame_count > 0;

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
