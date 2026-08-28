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
        let cursor_mode = if config.include_cursor {
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
    let (nvidia_device, va_device) = hardware_encoder_devices();
    select_h264_encoder(
        |name| gst::ElementFactory::find(name).is_some(),
        nvidia_device,
        va_device,
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
    let (bitrate_kbps, speed_preset) = encoding_settings(&config.quality);
    let fps = recording_fps(&config.quality);

    let (nvidia_device, va_device) = hardware_encoder_devices();
    let encoder_kind = select_h264_encoder(
        |name| gst::ElementFactory::find(name).is_some(),
        nvidia_device,
        va_device,
    )?;
    println!(
        "Linux recording encoder selected: {} at {} fps",
        encoder_kind.factory_name(),
        fps
    );

    let source = gst::ElementFactory::make("pipewiresrc")
        .property("fd", pipewire_fd)
        .property("path", node_id.to_string())
        .property("do-timestamp", true)
        .build()
        .context("Failed to create the PipeWire source")?;
    let queue = make_element("queue")?;
    queue.set_property("max-size-buffers", 4u32);
    queue.set_property("max-size-bytes", 0u32);
    queue.set_property("max-size-time", 0u64);
    queue.set_property_from_str("leaky", "downstream");
    let rate = gst::ElementFactory::make("videorate")
        .property("drop-only", true)
        .build()
        .context("Failed to create the Linux frame-rate limiter")?;
    let convert = make_element("videoconvert")?;
    let caps_filter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("format", encoder_kind.raw_format())
                .field("framerate", gst::Fraction::new(fps as i32, 1))
                .build(),
        )
        .build()
        .context("Failed to create the raw-video format filter")?;
    let encoder = make_h264_encoder(encoder_kind, bitrate_kbps, speed_preset)?;
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
    anyhow::bail!("No supported native H.264 GStreamer encoder is installed")
}

fn hardware_encoder_devices() -> (bool, bool) {
    let virtual_machine = [
        "/sys/class/dmi/id/product_name",
        "/sys/class/dmi/id/sys_vendor",
        "/sys/class/dmi/id/board_vendor",
    ]
    .into_iter()
    .filter_map(|path| std::fs::read_to_string(path).ok())
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase();
    if ["vmware", "virtualbox", "qemu", "kvm", "hyper-v"]
        .iter()
        .any(|vendor| virtual_machine.contains(vendor))
    {
        return (false, false);
    }

    let nvidia = Path::new("/dev/nvidia0").exists();
    let va = std::fs::read_dir("/dev/dri")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"));
    (nvidia, va)
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
