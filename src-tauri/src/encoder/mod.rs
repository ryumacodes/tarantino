use anyhow::Result;
use std::path::Path;

// Platform-specific encoder implementations
#[cfg(target_os = "macos")]
pub mod macos;

#[derive(Clone)]
pub struct EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub codec: VideoCodec,
    #[allow(dead_code)] // Reserved for future use - multiple container format support
    pub container: Container,
    pub hardware_accel: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum VideoCodec {
    H264,
    H265,
    ProRes,
    Av1,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum AudioCodec {
    Aac,
    Opus,
    Pcm,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Container {
    Mp4,
    Mov,
    Mkv,
    Webm,
}

/// Platform-agnostic encoder wrapper
pub enum Encoder {
    #[cfg(target_os = "macos")]
    VideoToolbox(macos::VideoToolboxEncoder),
    #[cfg(target_os = "windows")]
    MediaFoundation,
    #[cfg(target_os = "linux")]
    FFmpeg,
    Uninitialized {
        config: EncoderConfig,
        output_path: String,
    },
}

impl Encoder {
    /// Create a new uninitialized encoder
    pub fn new(config: EncoderConfig, output_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::Uninitialized {
            config,
            output_path: output_path.as_ref().to_string_lossy().to_string(),
        })
    }

    /// Start the encoder (initializes platform-specific implementation)
    pub fn start(&mut self) -> Result<()> {
        // Move out of uninitialized state
        let (config, _output_path) = match self {
            Self::Uninitialized { config, output_path } => {
                (config.clone(), output_path.clone())
            }
            _ => return Ok(()), // Already started
        };

        // Initialize platform-specific encoder
        #[cfg(target_os = "macos")]
        {
            let vt_encoder = macos::VideoToolboxEncoder::new(config)?;
            *self = Self::VideoToolbox(vt_encoder);
            println!("VideoToolbox encoder started");
        }

        #[cfg(target_os = "windows")]
        {
            // TODO: Initialize Media Foundation encoder
            anyhow::bail!("Media Foundation encoder not yet implemented");
        }

        #[cfg(target_os = "linux")]
        {
            // TODO: Initialize FFmpeg encoder
            anyhow::bail!("FFmpeg encoder not yet implemented");
        }

        Ok(())
    }

    /// Encode a video frame
    pub fn encode_frame(&mut self, frame_data: &[u8], width: u32, height: u32, stride: u32, pixel_format: &str, timestamp_us: u64) -> Result<()> {
        match self {
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(encoder) => {
                encoder.encode_frame(frame_data, width, height, stride, pixel_format, timestamp_us)
            }
            #[cfg(target_os = "windows")]
            Self::MediaFoundation => {
                anyhow::bail!("Media Foundation encoder not yet implemented")
            }
            #[cfg(target_os = "linux")]
            Self::FFmpeg => {
                anyhow::bail!("FFmpeg encoder not yet implemented")
            }
            Self::Uninitialized { .. } => {
                anyhow::bail!("Encoder not started - call start() first")
            }
        }
    }

    /// Encode audio samples
    #[allow(dead_code)] // Reserved for future use - audio encoding feature
    pub fn encode_audio(&mut self, _audio_data: &[f32], _timestamp: u64) -> Result<()> {
        // TODO: Implement audio encoding
        Ok(())
    }

    /// Flush pending frames and finalize encoding
    pub fn finish(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(encoder) => {
                encoder.flush()?;
                println!("VideoToolbox encoder flushed");
            }
            #[cfg(target_os = "windows")]
            Self::MediaFoundation => {}
            #[cfg(target_os = "linux")]
            Self::FFmpeg => {}
            Self::Uninitialized { .. } => {}
        }

        Ok(())
    }

    /// Try to receive an encoded frame (non-blocking)
    pub fn try_receive_frame(&self) -> Option<macos::EncodedFrame> {
        match self {
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(encoder) => encoder.try_receive_frame(),
            _ => None,
        }
    }

    /// Receive an encoded frame (async, blocking)
    #[allow(dead_code)] // Reserved for future use - streaming/realtime encoding mode
    pub async fn receive_frame(&self) -> Option<macos::EncodedFrame> {
        match self {
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(encoder) => encoder.receive_frame().await,
            _ => None,
        }
    }

    /// Check if hardware encoding is supported on this platform
    #[allow(dead_code)] // Reserved for future use - capability detection API
    pub fn supports_hardware_encoding() -> bool {
        #[cfg(target_os = "macos")]
        {
            // VideoToolbox is always available on macOS
            true
        }

        #[cfg(target_os = "windows")]
        {
            // TODO: Check for NVENC, QuickSync, or AMF
            false
        }

        #[cfg(target_os = "linux")]
        {
            // TODO: Check for VAAPI, NVENC
            false
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            false
        }
    }
}