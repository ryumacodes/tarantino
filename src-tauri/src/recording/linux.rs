//! In-process Linux recording through the XDG ScreenCast portal and PipeWire.

use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ashpd::desktop::{
    PersistMode, Session,
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
};
use gst::prelude::*;

use super::{QualityPreset, RecordingConfig, RecordingTarget};

#[path = "linux_cursor.rs"]
mod cursor;

/// Open the compositor's real window chooser before recording and persist the
/// approved source for the recording session that follows.
pub(crate) async fn prepare_window_source() -> Result<()> {
    let portal = Screencast::new()
        .await
        .context("Failed to connect to xdg-desktop-portal ScreenCast")?;
    let session = portal
        .create_session(Default::default())
        .await
        .context("Failed to create a desktop-portal screen-cast session")?;

    portal
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Embedded)
                .set_sources(Some(SourceType::Window.into()))
                .set_multiple(false)
                .set_persist_mode(PersistMode::ExplicitlyRevoked),
        )
        .await
        .context("Failed to configure the desktop-portal window picker")?;

    let response = portal
        .start(&session, None, Default::default())
        .await
        .context("Failed to open the desktop-portal window picker")?
        .response()
        .context("Window selection was cancelled")?;
    if response.streams().is_empty() {
        anyhow::bail!("The desktop portal returned no window stream");
    }
    let token = response
        .restore_token()
        .context("The desktop portal did not return a reusable window selection")?;
    save_restore_token("window", token)
        .context("Failed to save the selected desktop-portal window")?;
    Ok(())
}

pub struct LinuxRecording {
    pipeline: gst::Pipeline,
    _cursor_reader: Option<cursor::CursorReader>,
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

        runtime_preflight()?;

        let portal = Screencast::new()
            .await
            .context("Failed to connect to xdg-desktop-portal ScreenCast")?;
        let session = portal
            .create_session(Default::default())
            .await
            .context("Failed to create a desktop-portal screen-cast session")?;

        let (source_type, source_key) = match config.target {
            RecordingTarget::Desktop { .. } => (SourceType::Monitor, "display"),
            RecordingTarget::Window { .. } => (SourceType::Window, "window"),
            RecordingTarget::Device { .. } => {
                anyhow::bail!("Device capture is not supported on Linux yet")
            }
        };
        let cursor_metadata = crate::input::pointer_capture_consented()
            && portal
                .available_cursor_modes()
                .await
                .is_ok_and(|modes| modes.contains(CursorMode::Metadata));
        crate::input::configure_stream_pointer(cursor_metadata);
        // With click tracking, the editor/export cursor is driven by the
        // compositor's stream-local coordinates instead of an unrelated HID.
        let cursor_mode = if cursor_metadata {
            CursorMode::Metadata
        } else if config.include_cursor {
            CursorMode::Embedded
        } else {
            CursorMode::Hidden
        };

        let restore_token = load_restore_token(source_key);
        let mut source_options = SelectSourcesOptions::default()
            .set_cursor_mode(cursor_mode)
            .set_sources(Some(source_type.into()))
            .set_multiple(false)
            .set_persist_mode(PersistMode::ExplicitlyRevoked);
        if let Some(token) = restore_token.as_deref() {
            source_options = source_options.set_restore_token(Some(token));
        }

        portal
            .select_sources(&session, source_options)
            .await
            .context("Failed to configure the desktop-portal source picker")?;

        let response = portal
            .start(&session, None, Default::default())
            .await
            .context("Failed to open the desktop-portal source picker")?
            .response()
            .context("Screen or window selection was cancelled")?;
        if let Some(token) = response.restore_token()
            && let Err(error) = save_restore_token(source_key, token)
        {
            eprintln!("Failed to save Linux portal source choice: {error:#}");
        }
        let stream = response
            .streams()
            .first()
            .context("The desktop portal returned no PipeWire stream")?;
        let pipewire_fd = portal
            .open_pipe_wire_remote(&session, Default::default())
            .await
            .context("Failed to open the portal PipeWire remote")?;

        let cursor_reader = if cursor_metadata {
            // Each consumer needs its own portal connection. Duplicating the
            // recorder socket would make two clients read one protocol stream.
            let remote = portal
                .open_pipe_wire_remote(&session, Default::default())
                .await
                .context("Failed to open the cursor metadata remote")?;
            Some(cursor::CursorReader::start(
                &remote,
                stream.pipe_wire_node_id(),
            )?)
        } else {
            None
        };

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
            _cursor_reader: cursor_reader,
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

fn restore_token_path(source_key: &str) -> Option<PathBuf> {
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(
        config_root
            .join("tarantino")
            .join(format!("linux-screencast-{source_key}.token")),
    )
}

fn load_restore_token(source_key: &str) -> Option<String> {
    let token = std::fs::read_to_string(restore_token_path(source_key)?).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

fn save_restore_token(source_key: &str, token: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let path = restore_token_path(source_key)
        .context("Neither XDG_CONFIG_HOME nor HOME is available for portal persistence")?;
    let parent = path.parent().context("Portal token path has no parent")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create {}", parent.display()))?;
    if let Err(error) = std::fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(error).with_context(|| format!("Failed to replace {}", path.display()));
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("Failed to create {}", path.display()))?;
    file.write_all(token.as_bytes())
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

impl Drop for LinuxRecording {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

pub(crate) fn runtime_preflight() -> Result<()> {
    gst::init().context("Failed to initialize the native GStreamer runtime")?;
    verify_runtime_elements()
}

fn verify_runtime_elements() -> Result<()> {
    for element in [
        "pipewiresrc",
        "videotestsrc",
        "fakesink",
        "queue",
        "videorate",
        "videoconvert",
        "capsfilter",
        "h264parse",
        "mp4mux",
        "filesink",
    ] {
        if gst::ElementFactory::find(element).is_none() {
            anyhow::bail!("Required native GStreamer element is missing: {element}");
        }
    }
    select_h264_encoder(
        |name| gst::ElementFactory::find(name).is_some(),
        true,
        true,
    )
    .context(
        "A native H.264 encoder is required; install a VA-API, NVIDIA, x264, or OpenH264 GStreamer plugin",
    )?;
    Ok(())
}

fn build_pipeline(
    config: &RecordingConfig,
    output_path: &Path,
    node_id: u32,
    pipewire_fd: i32,
) -> Result<gst::Pipeline> {
    let (encoder_kind, gpu_conversion) = select_working_encoder(config)?;
    println!(
        "Linux recording encoder selected: {} at {} fps",
        encoder_kind.factory_name(),
        recording_fps(&config.quality)
    );

    let source = gst::ElementFactory::make("pipewiresrc")
        .property("fd", pipewire_fd)
        .property("path", node_id.to_string())
        .property("do-timestamp", true)
        // Wayland may send no new pixels for a static window. Preserve its
        // elapsed duration and the final hold instead of saving one frame.
        .property("keepalive-time", 100i32)
        .property("resend-last", true)
        .build()
        .context("Failed to create the PipeWire source")?;
    // Some compositors reuse the image's original PTS for static/cursor-only
    // updates. A live recording must follow elapsed time, not that stale PTS.
    // Make only the buffer header writable; pixel memory remains shared.
    let first_frame = std::sync::OnceLock::<std::time::Instant>::new();
    source
        .static_pad("src")
        .context("PipeWire source has no output pad")?
        .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
            if let Some(buffer) = info.buffer_mut() {
                let elapsed = first_frame.get_or_init(std::time::Instant::now).elapsed();
                let time =
                    gst::ClockTime::from_nseconds(elapsed.as_nanos().min(u64::MAX as u128) as u64);
                let buffer = buffer.make_mut();
                buffer.set_pts(time);
                buffer.set_dts(time);
            }
            gst::PadProbeReturn::Ok
        });
    let sink = gst::ElementFactory::make("filesink")
        .property("location", output_path.to_string_lossy().as_ref())
        .build()
        .context("Failed to create the recording file sink")?;
    build_pipeline_with_source(config, source, sink, encoder_kind, gpu_conversion)
}

// Production and runtime probes use the same conversion, rate control, encoder,
// parser and muxer. Only the frame source and output sink differ.
fn build_pipeline_with_source(
    config: &RecordingConfig,
    source: gst::Element,
    sink: gst::Element,
    encoder_kind: H264Encoder,
    gpu_conversion: bool,
) -> Result<gst::Pipeline> {
    let (bitrate_kbps, speed_preset) = encoding_settings(&config.quality);
    let fps = recording_fps(&config.quality);
    let queue = make_element("queue")?;
    queue.set_property("max-size-buffers", 4u32);
    queue.set_property("max-size-bytes", 0u32);
    queue.set_property("max-size-time", 0u64);
    queue.set_property_from_str("leaky", "downstream");
    let rate = gst::ElementFactory::make("videorate")
        .property("drop-only", true)
        .build()
        .context("Failed to create the Linux frame-rate limiter")?;
    let convert = make_element(if gpu_conversion {
        "vapostproc"
    } else {
        "videoconvert"
    })?;
    let raw_caps = if gpu_conversion {
        gst::Caps::builder("video/x-raw")
            .features(["memory:VAMemory"])
            .field("format", encoder_kind.raw_format())
            .field("framerate", gst::Fraction::new(fps as i32, 1))
            .build()
    } else {
        gst::Caps::builder("video/x-raw")
            .field("format", encoder_kind.raw_format())
            .field("framerate", gst::Fraction::new(fps as i32, 1))
            .build()
    };
    println!(
        "Linux recording conversion: {}",
        if gpu_conversion {
            "VA surface path"
        } else {
            "system-memory fallback"
        }
    );
    let caps_filter = gst::ElementFactory::make("capsfilter")
        .property("caps", raw_caps)
        .build()
        .context("Failed to create the raw-video format filter")?;
    let encoder = make_h264_encoder(encoder_kind, bitrate_kbps, speed_preset)?;
    let parser = make_element("h264parse")?;
    let muxer = gst::ElementFactory::make("mp4mux")
        .property("faststart", true)
        .build()
        .context("Failed to create the MP4 muxer")?;
    let pipeline = gst::Pipeline::new();
    pipeline
        .add_many([
            &source,
            &queue,
            &rate,
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
        &rate,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum H264Encoder {
    Nvidia,
    Va,
    Vaapi,
    X264,
    OpenH264,
}

impl H264Encoder {
    fn factory_name(self) -> &'static str {
        match self {
            Self::Nvidia => "nvh264enc",
            Self::Va => "vah264enc",
            Self::Vaapi => "vaapih264enc",
            Self::X264 => "x264enc",
            Self::OpenH264 => "openh264enc",
        }
    }

    fn raw_format(self) -> &'static str {
        match self {
            Self::Va | Self::Vaapi => "NV12",
            Self::Nvidia | Self::X264 | Self::OpenH264 => "I420",
        }
    }

    fn uses_va_memory(self) -> bool {
        matches!(self, Self::Va)
    }
}

fn select_h264_encoder(
    mut available: impl FnMut(&str) -> bool,
    nvidia_device: bool,
    va_device: bool,
) -> Result<H264Encoder> {
    if nvidia_device && available("nvh264enc") {
        return Ok(H264Encoder::Nvidia);
    }
    if va_device && available("vah264enc") {
        return Ok(H264Encoder::Va);
    }
    if va_device && available("vaapih264enc") {
        return Ok(H264Encoder::Vaapi);
    }
    if available("x264enc") {
        return Ok(H264Encoder::X264);
    }
    if available("openh264enc") {
        return Ok(H264Encoder::OpenH264);
    }
    anyhow::bail!(
        "No usable native H.264 GStreamer encoder was found (missing plugin or failed encoder probe)"
    )
}

// A device node or VM vendor string does not prove encoder support. Probe
// registered factories instead, including GPUs passed through to a VM/container.
fn select_working_encoder(config: &RecordingConfig) -> Result<(H264Encoder, bool)> {
    let mut gpu_conversion = false;
    let encoder = select_h264_encoder(
        |name| {
            let kind = match name {
                "nvh264enc" => H264Encoder::Nvidia,
                "vah264enc" => H264Encoder::Va,
                "vaapih264enc" => H264Encoder::Vaapi,
                "x264enc" => H264Encoder::X264,
                _ => H264Encoder::OpenH264,
            };
            if gst::ElementFactory::find(name).is_none() {
                return false;
            }
            if kind.uses_va_memory()
                && gst::ElementFactory::find("vapostproc").is_some()
                && probe_encoder(config, kind, true).is_ok()
            {
                gpu_conversion = true;
                return true;
            }
            match probe_encoder(config, kind, false) {
                Ok(()) => {
                    gpu_conversion = false;
                    true
                }
                Err(error) => {
                    eprintln!("Linux encoder {name} is installed but unusable: {error:#}");
                    false
                }
            }
        },
        true,
        true,
    )?;
    Ok((encoder, gpu_conversion))
}

fn probe_encoder(config: &RecordingConfig, kind: H264Encoder, gpu: bool) -> Result<()> {
    let source = gst::ElementFactory::make("videotestsrc")
        .property("num-buffers", 3i32)
        .build()
        .context("Failed to create encoder probe source")?;
    let sink = make_element("fakesink")?;
    let pipeline = build_pipeline_with_source(config, source, sink, kind, gpu)?;
    let result = (|| {
        pipeline.set_state(gst::State::Playing)?;
        let bus = pipeline.bus().context("Encoder probe has no bus")?;
        let message = bus
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(3),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .context("Encoder probe timed out")?;
        match message.view() {
            gst::MessageView::Eos(..) => Ok(()),
            gst::MessageView::Error(error) => anyhow::bail!("{}", error.error()),
            _ => unreachable!(),
        }
    })();
    let _ = pipeline.set_state(gst::State::Null);
    result
}

fn make_h264_encoder(
    encoder_kind: H264Encoder,
    bitrate_kbps: u32,
    speed_preset: &str,
) -> Result<gst::Element> {
    match encoder_kind {
        H264Encoder::Nvidia | H264Encoder::Va | H264Encoder::Vaapi => {
            let encoder = make_element(encoder_kind.factory_name())?;
            if encoder.find_property("bitrate").is_some() {
                encoder.set_property_from_str("bitrate", &bitrate_kbps.to_string());
            }
            if encoder.find_property("gop-size").is_some() {
                encoder.set_property_from_str("gop-size", "120");
            }
            if encoder.find_property("keyframe-period").is_some() {
                encoder.set_property_from_str("keyframe-period", "120");
            }
            if encoder.find_property("zerolatency").is_some() {
                encoder.set_property("zerolatency", true);
            }
            if encoder.find_property("target-usage").is_some() {
                encoder.set_property_from_str("target-usage", "7");
            }
            Ok(encoder)
        }
        H264Encoder::X264 => {
            let encoder = gst::ElementFactory::make("x264enc")
                .property("bitrate", bitrate_kbps)
                .property("key-int-max", 120u32)
                .build()
                .context("Failed to create the x264 H.264 encoder")?;
            encoder.set_property_from_str("speed-preset", speed_preset);
            encoder.set_property_from_str("tune", "zerolatency");
            Ok(encoder)
        }
        H264Encoder::OpenH264 => {
            let encoder = gst::ElementFactory::make("openh264enc")
                .property("bitrate", bitrate_kbps.saturating_mul(1_000))
                .property("gop-size", 120u32)
                .property("enable-frame-skip", false)
                .build()
                .context("Failed to create the OpenH264 encoder")?;
            encoder.set_property_from_str("rate-control", "bitrate");
            encoder.set_property_from_str("usage-type", "screen");
            Ok(encoder)
        }
    }
}

fn encoding_settings(quality: &QualityPreset) -> (u32, &'static str) {
    match quality {
        QualityPreset::Lossless => (32_000, "medium"),
        QualityPreset::High => (16_000, "fast"),
        QualityPreset::Medium => (8_000, "veryfast"),
        QualityPreset::Low => (4_000, "superfast"),
    }
}

fn recording_fps(quality: &QualityPreset) -> u32 {
    match quality {
        QualityPreset::Low | QualityPreset::Medium => 30,
        QualityPreset::High | QualityPreset::Lossless => 60,
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
        assert_eq!(recording_fps(&QualityPreset::Low), 30);
        assert_eq!(recording_fps(&QualityPreset::Medium), 30);
        assert_eq!(recording_fps(&QualityPreset::High), 60);
        assert_eq!(recording_fps(&QualityPreset::Lossless), 60);
    }

    #[test]
    fn encoder_selection_prefers_available_hardware_then_software() {
        assert_eq!(
            select_h264_encoder(|name| name == "nvh264enc" || name == "x264enc", true, false)
                .unwrap(),
            H264Encoder::Nvidia
        );
        assert_eq!(
            select_h264_encoder(|name| name == "vah264enc" || name == "x264enc", false, true)
                .unwrap(),
            H264Encoder::Va
        );
        assert_eq!(
            select_h264_encoder(|name| name == "x264enc", false, false).unwrap(),
            H264Encoder::X264
        );
        assert_eq!(
            select_h264_encoder(|name| name == "openh264enc", false, false).unwrap(),
            H264Encoder::OpenH264
        );
        assert!(select_h264_encoder(|_| false, true, true).is_err());
    }
}

#[cfg(test)]
#[path = "linux_tests.rs"]
mod runtime_tests;
