use anyhow::{Result, anyhow};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;

use super::{Project, ExportConfig, VideoTrack};

/// **FRAME COMPOSITOR**
/// Composites video frames from multiple sources (display, camera, etc.)
/// Uses ffmpeg to decode and composite frames
pub struct FrameCompositor {
    /// Export configuration
    config: ExportConfig,

    /// Video decoders for each track
    decoders: HashMap<String, VideoDecoder>,

    /// Compositor settings
    layout: CompositorLayout,
}

/// Video decoder for a single video track
struct VideoDecoder {
    /// Path to the video file
    video_path: String,

    /// Video dimensions
    width: u32,
    height: u32,

    /// Frame rate
    fps: f64,

    /// Current frame cache
    frame_cache: Option<DecodedFrame>,
}

/// Decoded video frame
struct DecodedFrame {
    /// Raw RGBA pixel data
    data: Vec<u8>,

    /// Timestamp in milliseconds
    timestamp_ms: u64,

    /// Frame dimensions
    width: u32,
    height: u32,
}

/// Compositor layout configuration
#[derive(Debug, Clone)]
pub struct CompositorLayout {
    /// Display video position and scale
    pub display: LayerConfig,

    /// Camera video position and scale (if present)
    pub camera: Option<LayerConfig>,

    /// Background color (RGBA)
    pub background_color: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct LayerConfig {
    /// Position in output frame (x, y)
    pub position: (i32, i32),

    /// Size in output frame (width, height)
    pub size: (u32, u32),

    /// Opacity (0.0 - 1.0)
    pub opacity: f32,

    /// Z-index for layering
    pub z_index: i32,
}

impl FrameCompositor {
    /// Create new frame compositor
    pub fn new(config: &ExportConfig) -> Result<Self> {
        let layout = CompositorLayout::default(config.video.width, config.video.height);

        Ok(Self {
            config: config.clone(),
            decoders: HashMap::new(),
            layout,
        })
    }

    /// Initialize compositor with project data
    pub async fn initialize(&mut self, project: &Project) -> Result<()> {
        println!("Initializing frame compositor with {} clips", project.clips.len());

        // Create decoders for display track
        if let Some(ref display_track) = project.tracks.display {
            self.add_video_decoder("display", display_track)?;
        }

        // Create decoders for camera track
        if let Some(ref camera_track) = project.tracks.camera {
            self.add_video_decoder("camera", camera_track)?;

            // Setup camera-in-corner layout
            self.layout.camera = Some(LayerConfig {
                position: (
                    self.config.video.width as i32 - 320 - 20,
                    self.config.video.height as i32 - 180 - 20,
                ),
                size: (320, 180), // 16:9 camera overlay
                opacity: 1.0,
                z_index: 10,
            });
        }

        Ok(())
    }

    /// Add a video decoder for a track
    fn add_video_decoder(&mut self, id: &str, track: &VideoTrack) -> Result<()> {
        let decoder = VideoDecoder {
            video_path: track.path.clone(),
            width: track.width,
            height: track.height,
            fps: track.fps,
            frame_cache: None,
        };

        self.decoders.insert(id.to_string(), decoder);
        println!("Added video decoder for {}: {}x{} @ {}fps", id, track.width, track.height, track.fps);

        Ok(())
    }

    /// Composite a frame at the given timestamp
    pub async fn composite_frame(&mut self, _project: &Project, timestamp_ms: u64) -> Result<Vec<u8>> {
        // Create output frame buffer (RGBA)
        let width = self.config.video.width;
        let height = self.config.video.height;
        let mut output = vec![0u8; (width * height * 4) as usize];

        // Fill with background color
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                output[idx..idx + 4].copy_from_slice(&self.layout.background_color);
            }
        }

        // Composite display track (bottom layer)
        if let Some(decoder) = self.decoders.get_mut("display") {
            let frame = Self::decode_frame_at_timestamp(decoder, timestamp_ms).await?;
            self.composite_layer(&frame, &self.layout.display, &mut output, width, height)?;
        }

        // Composite camera track (top layer)
        if let Some(ref camera_config) = self.layout.camera.clone() {
            if let Some(decoder) = self.decoders.get_mut("camera") {
                let frame = Self::decode_frame_at_timestamp(decoder, timestamp_ms).await?;
                self.composite_layer(&frame, camera_config, &mut output, width, height)?;
            }
        }

        Ok(output)
    }

    /// Decode a frame at a specific timestamp
    async fn decode_frame_at_timestamp(decoder: &mut VideoDecoder, timestamp_ms: u64) -> Result<DecodedFrame> {
        // Check if we have this frame cached
        if let Some(ref cached) = decoder.frame_cache {
            if cached.timestamp_ms == timestamp_ms {
                return Ok(cached.clone());
            }
        }

        // Decode frame using ffmpeg
        let frame = Self::decode_frame_ffmpeg(&decoder.video_path, timestamp_ms, decoder.width, decoder.height).await?;

        // Cache the decoded frame
        decoder.frame_cache = Some(frame.clone());

        Ok(frame)
    }

    /// Decode a single frame using ffmpeg
    async fn decode_frame_ffmpeg(video_path: &str, timestamp_ms: u64, width: u32, height: u32) -> Result<DecodedFrame> {
        // Check if video file exists
        if !Path::new(video_path).exists() {
            return Err(anyhow!("Video file not found: {}", video_path));
        }

        let timestamp_sec = timestamp_ms as f64 / 1000.0;

        // Use ffmpeg to extract a single frame as raw RGBA data
        let output = Command::new("ffmpeg")
            .arg("-ss").arg(format!("{:.3}", timestamp_sec))
            .arg("-i").arg(video_path)
            .arg("-vframes").arg("1")
            .arg("-f").arg("rawvideo")
            .arg("-pix_fmt").arg("rgba")
            .arg("-")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("FFmpeg decode failed: {}", stderr));
        }

        let expected_size = (width * height * 4) as usize;
        if output.stdout.len() != expected_size {
            // Frame might be different size, resize it
            println!("Warning: decoded frame size mismatch, got {} expected {}", output.stdout.len(), expected_size);

            // For now, create a black frame if decoding failed
            return Ok(DecodedFrame {
                data: vec![0u8; expected_size],
                timestamp_ms,
                width,
                height,
            });
        }

        Ok(DecodedFrame {
            data: output.stdout,
            timestamp_ms,
            width,
            height,
        })
    }

    /// Composite a layer onto the output frame
    fn composite_layer(
        &self,
        source: &DecodedFrame,
        config: &LayerConfig,
        output: &mut [u8],
        output_width: u32,
        output_height: u32,
    ) -> Result<()> {
        // Scale and position the source frame
        for dst_y in 0..config.size.1 {
            for dst_x in 0..config.size.0 {
                // Calculate output position
                let out_x = config.position.0 + dst_x as i32;
                let out_y = config.position.1 + dst_y as i32;

                // Skip if outside output bounds
                if out_x < 0 || out_y < 0 || out_x >= output_width as i32 || out_y >= output_height as i32 {
                    continue;
                }

                // Calculate source position (with scaling)
                let src_x = (dst_x as f32 * source.width as f32 / config.size.0 as f32) as u32;
                let src_y = (dst_y as f32 * source.height as f32 / config.size.1 as f32) as u32;

                if src_x >= source.width || src_y >= source.height {
                    continue;
                }

                // Get source pixel
                let src_idx = ((src_y * source.width + src_x) * 4) as usize;
                if src_idx + 3 >= source.data.len() {
                    continue;
                }

                let src_r = source.data[src_idx];
                let src_g = source.data[src_idx + 1];
                let src_b = source.data[src_idx + 2];
                let src_a = source.data[src_idx + 3];

                // Apply opacity
                let alpha = (src_a as f32 / 255.0) * config.opacity;

                // Get destination pixel
                let dst_idx = ((out_y as u32 * output_width + out_x as u32) * 4) as usize;

                // Alpha blending
                let dst_r = output[dst_idx];
                let dst_g = output[dst_idx + 1];
                let dst_b = output[dst_idx + 2];

                output[dst_idx] = ((src_r as f32 * alpha) + (dst_r as f32 * (1.0 - alpha))) as u8;
                output[dst_idx + 1] = ((src_g as f32 * alpha) + (dst_g as f32 * (1.0 - alpha))) as u8;
                output[dst_idx + 2] = ((src_b as f32 * alpha) + (dst_b as f32 * (1.0 - alpha))) as u8;
                output[dst_idx + 3] = 255; // Output is always fully opaque
            }
        }

        Ok(())
    }

    /// Update compositor layout
    pub fn set_layout(&mut self, layout: CompositorLayout) {
        self.layout = layout;
    }
}

impl CompositorLayout {
    /// Create default layout for the given output dimensions
    pub fn default(width: u32, height: u32) -> Self {
        Self {
            display: LayerConfig {
                position: (0, 0),
                size: (width, height),
                opacity: 1.0,
                z_index: 0,
            },
            camera: None,
            background_color: [0, 0, 0, 255], // Black background
        }
    }
}

impl Clone for DecodedFrame {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            timestamp_ms: self.timestamp_ms,
            width: self.width,
            height: self.height,
        }
    }
}
