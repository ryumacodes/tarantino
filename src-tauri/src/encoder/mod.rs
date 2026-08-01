use anyhow::Result;
use std::path::Path;

mod types;
pub use types::{EncodedFrame, EncoderConfig};

// Platform-specific encoder implementations
#[cfg(target_os = "macos")]
pub mod macos;

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
            Self::Uninitialized {
                config,
                output_path,
            } => (config.clone(), output_path.clone()),
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
    pub fn encode_frame(
        &mut self,
        frame_data: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        pixel_format: &str,
        timestamp_us: u64,
    ) -> Result<()> {
        match self {
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(encoder) => encoder.encode_frame(
                frame_data,
                width,
                height,
                stride,
                pixel_format,
                timestamp_us,
            ),
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
    pub fn try_receive_frame(&self) -> Option<EncodedFrame> {
        match self {
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(encoder) => encoder.try_receive_frame(),
            _ => None,
        }
    }
}
