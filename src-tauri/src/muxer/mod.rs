//! MP4 muxer for writing encoded video to file
//!
//! This muxer writes H.264 video directly into an MP4 container using the mp4 crate.
//! Flow: VideoToolbox → H.264 NAL units → MP4 muxer → .mp4 file
//!
//! The muxer handles:
//! - SPS/PPS parameter set extraction from H.264 stream
//! - Proper MP4 track configuration
//! - Sample writing with correct timestamps and durations
//! - MP4 finalization with proper headers

use anyhow::Result;
use mp4::{
    AvcConfig, MediaConfig, Mp4Config, Mp4Sample, Mp4Writer, TrackConfig, TrackType,
};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::encoder::macos::EncodedFrame;

/// MP4 muxer for writing H.264 video directly to MP4 container
#[allow(dead_code)]
pub struct Mp4Muxer {
    writer: Option<Mp4Writer<BufWriter<File>>>,
    track_id: Option<u32>,
    frame_count: u32,
    width: u32,
    height: u32,
    fps: u32,
    timescale: u32,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    pending_samples: Vec<(EncodedFrame, u32)>, // Store frames until we have SPS/PPS
}

impl Mp4Muxer {
    /// Create a new MP4 muxer
    ///
    /// Creates an MP4 file and initializes the writer.
    /// The video track will be added when we receive the first frame with SPS/PPS.
    pub fn new(output_path: impl AsRef<Path>, width: u32, height: u32, fps: u32) -> Result<Self> {
        let path = output_path.as_ref();
        let file = File::create(path)?;
        let buf_writer = BufWriter::new(file);

        println!("MP4 muxer created: {}x{} @ {} fps → {}", width, height, fps, path.display());

        // Timescale: use fps * 1000 for sub-frame precision
        let timescale = fps * 1000;

        // Create MP4 writer with config
        let config = Mp4Config {
            major_brand: str::parse("isom")
                .map_err(|_| anyhow::anyhow!("Failed to parse major brand"))?,
            minor_version: 512,
            compatible_brands: vec![
                str::parse("isom").map_err(|_| anyhow::anyhow!("Failed to parse brand"))?,
                str::parse("iso2").map_err(|_| anyhow::anyhow!("Failed to parse brand"))?,
                str::parse("avc1").map_err(|_| anyhow::anyhow!("Failed to parse brand"))?,
                str::parse("mp41").map_err(|_| anyhow::anyhow!("Failed to parse brand"))?,
            ],
            timescale,
        };

        let writer = Mp4Writer::write_start(buf_writer, &config)?;

        Ok(Self {
            writer: Some(writer),
            track_id: None,
            frame_count: 0,
            width,
            height,
            fps,
            timescale,
            sps: None,
            pps: None,
            pending_samples: Vec::new(),
        })
    }

    /// Extract SPS and PPS from H.264 NAL units
    fn extract_parameter_sets(&mut self, data: &[u8]) -> Result<()> {
        // H.264 NAL units start with 0x00 0x00 0x00 0x01 (4-byte start code)
        // or 0x00 0x00 0x01 (3-byte start code)

        let mut pos = 0;
        while pos < data.len() {
            // Find start code
            let start_code_len = if pos + 4 <= data.len()
                && data[pos..pos + 4] == [0, 0, 0, 1] {
                4
            } else if pos + 3 <= data.len()
                && data[pos..pos + 3] == [0, 0, 1] {
                3
            } else {
                pos += 1;
                continue;
            };

            let nal_start = pos + start_code_len;
            if nal_start >= data.len() {
                break;
            }

            // Find next start code
            let mut nal_end = nal_start + 1;
            while nal_end < data.len() {
                if (nal_end + 4 <= data.len() && data[nal_end..nal_end + 4] == [0, 0, 0, 1])
                    || (nal_end + 3 <= data.len() && data[nal_end..nal_end + 3] == [0, 0, 1]) {
                    break;
                }
                nal_end += 1;
            }

            // Extract NAL unit type (lower 5 bits of first byte)
            let nal_type = data[nal_start] & 0x1F;

            match nal_type {
                7 => {
                    // SPS (Sequence Parameter Set)
                    self.sps = Some(data[nal_start..nal_end].to_vec());
                    println!("Extracted SPS: {} bytes", nal_end - nal_start);
                }
                8 => {
                    // PPS (Picture Parameter Set)
                    self.pps = Some(data[nal_start..nal_end].to_vec());
                    println!("Extracted PPS: {} bytes", nal_end - nal_start);
                }
                _ => {}
            }

            pos = nal_end;
        }

        Ok(())
    }

    /// Initialize the video track once we have SPS/PPS
    fn initialize_track(&mut self) -> Result<()> {
        if self.track_id.is_some() {
            return Ok(()); // Already initialized
        }

        let sps = self.sps.as_ref()
            .ok_or_else(|| anyhow::anyhow!("SPS not available"))?;
        let pps = self.pps.as_ref()
            .ok_or_else(|| anyhow::anyhow!("PPS not available"))?;

        let track_config = TrackConfig {
            track_type: TrackType::Video,
            timescale: self.timescale,
            language: String::from("und"),
            media_conf: MediaConfig::AvcConfig(AvcConfig {
                width: self.width as u16,
                height: self.height as u16,
                seq_param_set: sps.clone(),
                pic_param_set: pps.clone(),
            }),
        };

        let writer = self.writer.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Writer not available"))?;

        writer.add_track(&track_config)?;
        // Track IDs start at 1 and increment sequentially
        let track_id = 1;
        self.track_id = Some(track_id);

        println!("Video track initialized: track_id={}, timescale={}", track_id, self.timescale);

        // Write any pending samples
        if !self.pending_samples.is_empty() {
            println!("Writing {} pending samples", self.pending_samples.len());
            let samples: Vec<_> = self.pending_samples.drain(..).collect();
            for (frame, duration_ms) in samples {
                self.write_frame_internal(&frame, duration_ms)?;
            }
        }

        Ok(())
    }

    /// Process H.264 NAL units for MP4 muxing
    ///
    /// VideoToolbox outputs H.264 in AVCC format (4-byte length prefixes) by default.
    /// This function filters out SPS/PPS NAL units from the AVCC data.
    ///
    /// CRITICAL: This function filters out SPS (type 7) and PPS (type 8) NAL units.
    /// These parameter sets should ONLY be in the AvcC configuration box, NOT in sample data.
    /// Including them in samples causes decoder errors and corruption.
    ///
    /// NOTE: Format detection was removed because it had a false positive bug.
    /// When AVCC has a 1-byte NAL unit, the length prefix [0x00, 0x00, 0x00, 0x01]
    /// looks identical to an Annex B start code, causing incorrect format detection.
    /// Since VideoToolbox ALWAYS outputs AVCC format, we always treat input as AVCC.
    fn convert_to_avcc(&self, data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }

        // VideoToolbox ALWAYS outputs AVCC format (4-byte length prefixes)
        // Just filter out SPS/PPS NAL units and return
        self.filter_parameter_sets_from_avcc(data)
    }

    /// Filter SPS/PPS NAL units from AVCC format data with enhanced validation
    fn filter_parameter_sets_from_avcc(&self, data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut pos = 0;
        let mut sps_filtered = 0;
        let mut pps_filtered = 0;
        let mut invalid_nals_skipped = 0;

        while pos + 4 < data.len() {
            // Read 4-byte length prefix (big-endian)
            let nal_len = u32::from_be_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
            ]) as usize;

            pos += 4;

            // Validate NAL length with stricter checks
            if nal_len == 0 {
                // Don't log individual NAL issues - aggregate them in summary below
                invalid_nals_skipped += 1;
                continue;
            }

            // Check for obviously invalid NAL lengths (too large)
            if nal_len > 10_000_000 {  // 10MB is way too large for a single NAL unit in screen recording
                // Don't log individual NAL issues - aggregate them in summary below
                invalid_nals_skipped += 1;
                // Try to recover by advancing 1 byte and searching for next valid NAL
                pos = pos - 3;  // Backup to try next position
                continue;
            }

            if pos + nal_len > data.len() {
                // Don't log individual NAL issues - aggregate them in summary below
                invalid_nals_skipped += 1;
                break;  // Stop processing this frame
            }

            // Validate NAL unit header byte
            let nal_header = data[pos];
            let nal_type = nal_header & 0x1F;
            let forbidden_bit = (nal_header & 0x80) != 0;

            // Check forbidden bit (must be 0 in valid H.264)
            if forbidden_bit {
                // Don't log individual NAL issues - aggregate them in summary below
                invalid_nals_skipped += 1;
                pos += nal_len;
                continue;
            }

            // Check for invalid NAL types
            if nal_type == 0 || nal_type == 13 || nal_type > 20 {
                // Don't log individual NAL issues - aggregate them in summary below
                invalid_nals_skipped += 1;
                pos += nal_len;
                continue;
            }

            // Filter out SPS (7) and PPS (8) - these should only be in AvcC config
            if nal_type == 7 {
                sps_filtered += 1;
                pos += nal_len;
                continue;
            } else if nal_type == 8 {
                pps_filtered += 1;
                pos += nal_len;
                continue;
            }

            // Keep all other valid NAL units (copy length prefix + data)
            result.extend_from_slice(&(nal_len as u32).to_be_bytes());
            result.extend_from_slice(&data[pos..pos + nal_len]);

            pos += nal_len;
        }

        // Only log if there were invalid NAL units (actual corruption)
        // SPS/PPS filtering is expected and normal, so we only log that at debug level
        if invalid_nals_skipped > 0 {
            println!("[MUXER WARNING] Skipped {} corrupted/invalid NAL units from AVCC sample", invalid_nals_skipped);
        }

        // Debug-level logging for normal parameter set filtering (only if filtering occurred)
        if sps_filtered > 0 || pps_filtered > 0 {
            // Silently filter SPS/PPS - this is expected behavior
            // Only uncomment for debugging:
            // println!("[MUXER DEBUG] Filtered {} SPS, {} PPS from AVCC sample", sps_filtered, pps_filtered);
        }

        result
    }

    /// Internal frame writing after track is initialized
    fn write_frame_internal(&mut self, frame: &EncodedFrame, duration_ms: u32) -> Result<()> {
        let track_id = self.track_id
            .ok_or_else(|| anyhow::anyhow!("Track not initialized"))?;

        // Convert from Annex B (start codes) to AVCC (length prefixes)
        let avcc_data = self.convert_to_avcc(&frame.data);

        // Calculate sample duration in timescale units
        // duration_ms is in milliseconds, timescale is fps * 1000
        let sample_duration = (duration_ms as u64 * self.timescale as u64) / 1000;

        let sample = Mp4Sample {
            start_time: (self.frame_count as u64 * sample_duration),
            duration: sample_duration as u32,
            rendering_offset: 0,
            is_sync: frame.is_keyframe,
            bytes: bytes::Bytes::from(avcc_data),
        };

        let writer = self.writer.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Writer not available"))?;

        writer.write_sample(track_id, &sample)?;
        self.frame_count += 1;

        if self.frame_count % 100 == 0 {
            println!("Muxed {} frames to MP4", self.frame_count);
        }

        Ok(())
    }

    /// Write an encoded frame to the MP4 file
    pub fn write_frame(&mut self, frame: &EncodedFrame, duration_ms: u32) -> Result<()> {
        // Extract SPS/PPS from frame metadata if available (preferred method)
        // VideoToolbox provides these via format description - more reliable than bitstream scanning
        if self.sps.is_none() {
            if let Some(sps) = &frame.sps {
                println!("Got SPS from format description: {} bytes", sps.len());
                self.sps = Some(sps.to_vec());
            }
        }
        if self.pps.is_none() {
            if let Some(pps) = &frame.pps {
                println!("Got PPS from format description: {} bytes", pps.len());
                self.pps = Some(pps.to_vec());
            }
        }

        // Fallback: Extract SPS/PPS from bitstream if not in metadata
        if self.sps.is_none() || self.pps.is_none() {
            self.extract_parameter_sets(&frame.data)?;
        }

        // Try to initialize track if we have SPS/PPS
        if self.sps.is_some() && self.pps.is_some() && self.track_id.is_none() {
            self.initialize_track()?;
        }

        // If track is initialized, write the frame
        if self.track_id.is_some() {
            self.write_frame_internal(frame, duration_ms)?;
        } else {
            // Otherwise, queue it for later
            self.pending_samples.push((frame.clone(), duration_ms));
        }

        Ok(())
    }

    /// Finalize and close the MP4 file
    pub fn finish(mut self) -> Result<()> {
        // Initialize track if we have pending samples
        if !self.pending_samples.is_empty() && self.track_id.is_none() {
            if let Err(e) = self.initialize_track() {
                eprintln!("Failed to initialize track during finish: {}", e);
                return Err(e);
            }
        }

        if let Some(mut writer) = self.writer.take() {
            writer.write_end()?;

            // CRITICAL: Ensure all data is flushed and synced to disk
            // The BufWriter might still have buffered data, and the OS might not have
            // fully written to disk even after write_end(). This causes issues when
            // ffprobe tries to read the file immediately after - it may see incomplete data.
            //
            // Unfortunately, write_end() consumes the writer, so we can't directly flush it.
            // The mp4 crate's write_end() should flush the BufWriter, but we add a small delay
            // to allow the OS to complete disk writes before other processes access the file.
            std::thread::sleep(std::time::Duration::from_millis(100));

            println!("MP4 muxer finished: {} frames written", self.frame_count);
        }

        Ok(())
    }

    /// Get the number of frames written
    #[allow(dead_code)]
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }
}
