//! Native screen capture for Tarantino
//!
//! # Architecture Migration (v0.2.0)
//!
//! The capture system has been redesigned to use native OS APIs instead of FFmpeg:
//!
//! ## Old Architecture (REMOVED)
//! - FFmpeg CLI-based capture via AVFoundation (macOS)
//! - `CaptureSession` trait with FFmpeg implementation
//! - High latency, process management issues
//!
//! ## New Architecture (CURRENT)
//! - **Native backends** per OS:
//!   - macOS: ScreenCaptureKit (SCK) - `src/capture/backends/macos/`
//!   - Windows: DXGI Desktop Duplication (stub) - `src/capture/backends/windows/`
//!   - Linux: PipeWire (stub) - `src/capture/backends/linux/`
//! - Low-level frame capture via `NativeCaptureBackend` trait
//! - Low latency (< 33ms), native cursor, proper permissions
//!
//! ## Usage
//!
//! ```rust
//! use crate::capture::backends::{CaptureBackendFactory, CaptureConfig, CaptureSourceType};
//!
//! let mut backend = CaptureBackendFactory::create_backend()?;
//! let sources = backend.enumerate_sources().await?;
//!
//! // Start capture
//! let config = CaptureConfig {
//!     source_id: sources[0].id,
//!     source_type: CaptureSourceType::Display,
//!     fps: 60,
//!     include_cursor: true,
//!     include_audio: false,
//!     region: None,
//! };
//! let handle = backend.start_capture(config).await?;
//!
//! // Receive frames
//! if let Some(mut rx) = backend.frame_receiver() {
//!     while let Ok(frame) = rx.recv().await {
//!         // Process frame...
//!     }
//! }
//! ```

// Native capture backends
pub mod backends;

// Re-export new backend architecture as primary API
pub use backends::CaptureSourceType;
