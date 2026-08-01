//! In-process Linux recording through the XDG ScreenCast portal and PipeWire.

use std::os::fd::{AsRawFd, OwnedFd};
use std::path::Path;

use anyhow::{Context, Result};
use ashpd::desktop::{
    PersistMode, Session,
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
};
use gst::prelude::*;

use super::{QualityPreset, RecordingConfig, RecordingTarget};

pub struct LinuxRecording {
    pipeline: gst::Pipeline,
    // Portal ownership grants access to the PipeWire node for the recording lifetime.
    _portal: Screencast,
    _session: Session<Screencast>,
    _pipewire_fd: OwnedFd,
}

impl LinuxRecording {
    pub async fn start(config: &RecordingConfig, output_path: &Path) -> Result<Self> {
        if config.include_system_audio {
            anyhow::bail!(
                "System audio capture is not available on Linux yet; disable system audio and try again"
            );
        }

        gst::init().context("Failed to initialize the native GStreamer runtime")?;
        verify_runtime_elements()?;

        let portal = Screencast::new()
            .await
            .context("Failed to connect to xdg-desktop-portal ScreenCast")?;
        let session = portal
            .create_session(Default::default())
            .await
            .context("Failed to create a desktop-portal screen-cast session")?;

        let source_type = match config.target {
            RecordingTarget::Desktop { .. } => SourceType::Monitor,
            RecordingTarget::Window { .. } => SourceType::Window,
            RecordingTarget::Device { .. } => {
                anyhow::bail!("Device capture is not supported on Linux yet")
            }
        };
        let cursor_mode = if config.include_cursor {
            CursorMode::Embedded
        } else {
            CursorMode::Hidden
        };

        portal
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(cursor_mode)
                    .set_sources(Some(source_type.into()))
                    .set_multiple(false)
                    .set_restore_token(None)
                    .set_persist_mode(PersistMode::DoNot),
            )
            .await
            .context("Failed to configure the desktop-portal source picker")?;

        let response = portal
            .start(&session, None, Default::default())
            .await
            .context("Failed to open the desktop-portal source picker")?
            .response()
            .context("Screen or window selection was cancelled")?;
        let stream = response
            .streams()
            .first()
            .context("The desktop portal returned no PipeWire stream")?;
        let pipewire_fd = portal
            .open_pipe_wire_remote(&session, Default::default())
            .await
            .context("Failed to open the portal PipeWire remote")?;

        let pipeline = build_pipeline(
            config,
            output_path,
            stream.pipe_wire_node_id(),
            pipewire_fd.as_raw_fd(),
        )?;
        pipeline
            .set_state(gst::State::Playing)
            .context("Failed to start the native Linux recording pipeline")?;

        Ok(Self {
            pipeline,
            _portal: portal,
            _session: session,
            _pipewire_fd: pipewire_fd,
        })
    }

    pub fn signal_stop(&mut self) -> Result<()> {
        if !self.pipeline.send_event(gst::event::Eos::new()) {
            anyhow::bail!("Failed to send end-of-stream to the Linux recorder");
        }
        Ok(())
    }

    pub async fn wait(self) -> Result<()> {
        let pipeline = self.pipeline.clone();
        let wait_result = tokio::task::spawn_blocking(move || wait_for_pipeline(&pipeline))
            .await
            .context("Linux recording finalization task panicked")?;
        self.pipeline
            .set_state(gst::State::Null)
            .context("Failed to release the native Linux recording pipeline")?;
        wait_result
    }
}

impl Drop for LinuxRecording {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn verify_runtime_elements() -> Result<()> {
    for element in [
        "pipewiresrc",
        "queue",
        "videoconvert",
        "capsfilter",
        "x264enc",
        "h264parse",
        "mp4mux",
        "filesink",
    ] {
        if gst::ElementFactory::find(element).is_none() {
            anyhow::bail!("Required native GStreamer element is missing: {element}");
        }
    }
    Ok(())
}

fn build_pipeline(
    config: &RecordingConfig,
    output_path: &Path,
    node_id: u32,
    pipewire_fd: i32,
) -> Result<gst::Pipeline> {
    let (bitrate_kbps, speed_preset) = encoding_settings(&config.quality);

    let source = gst::ElementFactory::make("pipewiresrc")
        .property("fd", pipewire_fd)
        .property("path", node_id.to_string())
        .property("do-timestamp", true)
        .build()
        .context("Failed to create the PipeWire source")?;
    let queue = make_element("queue")?;
    let convert = make_element("videoconvert")?;
    let caps_filter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("format", "I420")
                .build(),
        )
        .build()
        .context("Failed to create the raw-video format filter")?;
    let encoder = gst::ElementFactory::make("x264enc")
        .property("bitrate", bitrate_kbps)
        .property("key-int-max", 120u32)
        .build()
        .context("Failed to create the native H.264 encoder")?;
    encoder.set_property_from_str("speed-preset", speed_preset);
    encoder.set_property_from_str("tune", "zerolatency");
    let parser = make_element("h264parse")?;
    let muxer = gst::ElementFactory::make("mp4mux")
        .property("faststart", true)
        .build()
        .context("Failed to create the MP4 muxer")?;
    let sink = gst::ElementFactory::make("filesink")
        .property("location", output_path.to_string_lossy().as_ref())
        .build()
        .context("Failed to create the recording file sink")?;

    let pipeline = gst::Pipeline::new();
    pipeline
        .add_many([
            &source,
            &queue,
            &convert,
            &caps_filter,
            &encoder,
            &parser,
            &muxer,
            &sink,
        ])
        .context("Failed to assemble the native Linux recording pipeline")?;
    gst::Element::link_many([
        &source,
        &queue,
        &convert,
        &caps_filter,
        &encoder,
        &parser,
        &muxer,
        &sink,
    ])
    .context("Failed to link the native Linux recording pipeline")?;
    Ok(pipeline)
}

fn make_element(name: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(name)
        .build()
        .with_context(|| format!("Failed to create native GStreamer element: {name}"))
}

fn encoding_settings(quality: &QualityPreset) -> (u32, &'static str) {
    match quality {
        QualityPreset::Lossless => (32_000, "medium"),
        QualityPreset::High => (16_000, "fast"),
        QualityPreset::Medium => (8_000, "veryfast"),
        QualityPreset::Low => (4_000, "superfast"),
    }
}

fn wait_for_pipeline(pipeline: &gst::Pipeline) -> Result<()> {
    let bus = pipeline
        .bus()
        .context("Linux recording pipeline has no bus")?;
    for message in bus.iter_timed(gst::ClockTime::from_seconds(15)) {
        match message.view() {
            gst::MessageView::Eos(..) => return Ok(()),
            gst::MessageView::Error(error) => {
                anyhow::bail!(
                    "Native Linux recording failed in {}: {} ({})",
                    error
                        .src()
                        .map(|source| source.path_string().to_string())
                        .unwrap_or_else(|| "unknown element".to_string()),
                    error.error(),
                    error.debug().unwrap_or_default()
                );
            }
            _ => {}
        }
    }
    anyhow::bail!("Timed out while finalizing the native Linux recording")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_presets_have_deterministic_native_encoder_settings() {
        assert_eq!(encoding_settings(&QualityPreset::High), (16_000, "fast"));
        assert_eq!(encoding_settings(&QualityPreset::Low), (4_000, "superfast"));
    }
}
