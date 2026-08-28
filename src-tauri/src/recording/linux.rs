//! Wayland-native Linux recording through xdg-desktop-portal and PipeWire.

use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use ashpd::desktop::{
    PersistMode, Session,
    screencast::{
        CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
    },
};
use enumflags2::BitFlags;

use super::{QualityPreset, RecordingConfig, RecordingTarget, artifacts::RecordingArtifacts};

const CHILD_PIPEWIRE_FD: i32 = 3;

/// Ask the Wayland desktop portal to choose a window before recording starts.
/// The returned restore token lets the real recording session reopen the same
/// approved source without deferring the choice until the Record button.
pub(crate) async fn prepare_window_source() -> Result<()> {
    let portal = Screencast::new()
        .await
        .context("Could not connect to the desktop screen-cast portal")?;
    let session = portal
        .create_session(Default::default())
        .await
        .context("Could not create a desktop portal session")?;
    portal
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_sources(BitFlags::from(SourceType::Window))
                .set_multiple(false)
                .set_cursor_mode(CursorMode::Embedded)
                .set_persist_mode(PersistMode::Application),
        )
        .await
        .context("Could not configure the desktop portal window picker")?;

    let response = portal
        .start(&session, None, Default::default())
        .await
        .context("Could not open the desktop portal window picker")?
        .response()
        .context("Window selection was cancelled or denied")?;
    if response.streams().is_empty() {
        bail!("The desktop portal returned no window stream");
    }
    if let Some(token) = response.restore_token() {
        save_restore_token("window", token)
            .context("Could not save the selected desktop portal window")?;
    } else {
        bail!("The desktop portal did not provide a reusable window selection");
    }

    Ok(())
}

pub struct LinuxRecordingPipeline {
    gst: Option<Child>,
    encoder: Option<Child>,
    audio: Vec<Child>,
    _portal_session: Session<Screencast>,
    output_path: PathBuf,
    stopped: bool,
}

impl LinuxRecordingPipeline {
    pub async fn start(config: &RecordingConfig) -> Result<Self> {
        require_program("gst-launch-1.0")?;
        require_program("gst-inspect-1.0")?;
        require_supported_ffmpeg()?;
        require_gstreamer_element("pipewiresrc")?;
        require_gstreamer_element("videoconvert")?;
        require_gstreamer_element("y4menc")?;

        let portal = Screencast::new()
            .await
            .context("Could not connect to the desktop screen-cast portal")?;
        let session = portal
            .create_session(Default::default())
            .await
            .context("Could not create a desktop portal session")?;

        let (source, token_name) = match config.target {
            RecordingTarget::Window { .. } => (SourceType::Window, "window"),
            RecordingTarget::Desktop { .. } => (SourceType::Monitor, "monitor"),
            RecordingTarget::Device { .. } => {
                bail!("Device capture is not supported by the Linux screen-cast portal")
            }
        };
        let restore_token = load_restore_token(token_name);

        // Embedded is the only mode which guarantees a visible cursor on all
        // Wayland compositors. Privacy-safe input events are still collected
        // separately where the compositor permits global input observation.
        portal
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_sources(BitFlags::from(source))
                    .set_multiple(false)
                    .set_cursor_mode(CursorMode::Embedded)
                    .set_persist_mode(PersistMode::Application)
                    .set_restore_token(restore_token.as_deref()),
            )
            .await
            .context("Could not configure the desktop portal source picker")?;

        let response = portal
            .start(&session, None, Default::default())
            .await
            .context("Could not open the desktop portal source picker")?
            .response()
            .context("Screen sharing was cancelled or denied")?;
        if let Some(token) = response.restore_token()
            && let Err(error) = save_restore_token(token_name, token)
        {
            eprintln!("Could not save the desktop portal selection: {error:#}");
        }
        let stream = response
            .streams()
            .first()
            .ok_or_else(|| anyhow!("The desktop portal returned no screen stream"))?;
        let node_id = stream.pipe_wire_node_id();
        let remote = portal
            .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
            .await
            .context("Could not open the PipeWire screen stream")?;

        let fps = match config.quality {
            QualityPreset::Low | QualityPreset::Medium => 30,
            QualityPreset::High | QualityPreset::Lossless => 60,
        };
        let output_path = PathBuf::from(&config.output_path);

        let mut gst_command = Command::new("gst-launch-1.0");
        gst_command
            .args(["-q", "-e", "pipewiresrc"])
            .arg(format!("fd={CHILD_PIPEWIRE_FD}"))
            .arg(format!("path={node_id}"))
            .arg("do-timestamp=true")
            .args(["!", "videoconvert", "!", "videorate", "!"])
            // y4menc's portable 8-bit 4:2:0 format is I420. FFmpeg converts
            // this once to NV12 while uploading it to the VA-API device.
            .arg(format!("video/x-raw,format=I420,framerate={fps}/1"))
            .args(["!", "y4menc", "!", "fdsink", "fd=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let portal_fd = remote.as_raw_fd();
        // SAFETY: this closure only performs async-signal-safe fd operations
        // between fork and exec. The parent retains ownership of `remote` until
        // spawn has completed.
        unsafe {
            gst_command.pre_exec(move || {
                if libc::dup2(portal_fd, CHILD_PIPEWIRE_FD) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::fcntl(CHILD_PIPEWIRE_FD, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let mut gst = gst_command
            .spawn()
            .context("Failed to start the native PipeWire capture pipeline")?;
        let video_pipe = gst
            .stdout
            .take()
            .ok_or_else(|| anyhow!("PipeWire capture did not expose a video stream"))?;

        let encoder = match spawn_encoder(video_pipe, &output_path, fps, &config.quality) {
            Ok(child) => child,
            Err(error) => {
                let _ = gst.kill();
                return Err(error);
            }
        };

        let artifacts = RecordingArtifacts::new(&output_path);
        let mut audio = Vec::new();
        if config.include_system_audio {
            audio.push(spawn_pulse_audio(
                "@DEFAULT_MONITOR@",
                &artifacts.system_audio(),
            )?);
        }
        if config.include_microphone {
            let source = config
                .microphone_device
                .as_deref()
                .unwrap_or("@DEFAULT_SOURCE@");
            audio.push(spawn_pulse_audio(source, &artifacts.microphone())?);
        }

        println!(
            "Linux native recording started: portal node {}, {} fps, output {}",
            node_id,
            fps,
            output_path.display()
        );
        Ok(Self {
            gst: Some(gst),
            encoder: Some(encoder),
            audio,
            _portal_session: session,
            output_path,
            stopped: false,
        })
    }

    pub fn signal_stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        if let Some(child) = &self.gst {
            signal(child, "INT");
        }
        for child in &self.audio {
            signal(child, "INT");
        }
    }

    pub fn pause(&self) {
        self.signal_all("STOP");
    }

    pub fn resume(&self) {
        self.signal_all("CONT");
    }

    fn signal_all(&self, name: &str) {
        if let Some(child) = &self.gst {
            signal(child, name);
        }
        if let Some(child) = &self.encoder {
            signal(child, name);
        }
        for child in &self.audio {
            signal(child, name);
        }
    }

    pub async fn wait(mut self) -> Result<()> {
        if !self.stopped {
            self.signal_stop();
        }
        tokio::task::spawn_blocking(move || {
            if let Some(mut gst) = self.gst.take() {
                let status = gst.wait().context("Failed waiting for PipeWire capture")?;
                // gst-launch returns 130 for an interrupt after writing EOS.
                if !status.success() && status.code() != Some(130) {
                    bail!("PipeWire capture exited with {status}");
                }
            }

            if let Some(mut encoder) = self.encoder.take() {
                let status = encoder.wait().context("Failed waiting for video encoder")?;
                if !status.success() {
                    bail!("Video encoder exited with {status}");
                }
            }

            for mut child in self.audio {
                let status = child.wait().context("Failed waiting for audio capture")?;
                if !status.success() && status.code() != Some(255) {
                    eprintln!("Linux audio capture exited with {status}");
                }
            }

            let size = fs::metadata(&self.output_path)
                .with_context(|| {
                    format!(
                        "Recording was not created at {}",
                        self.output_path.display()
                    )
                })?
                .len();
            if size <= 1024 {
                bail!("Recording is empty ({} bytes)", size);
            }
            Ok(())
        })
        .await
        .context("Linux recording finalizer panicked")?
    }
}

fn restore_token_path(source: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(
        base.join("tarantino")
            .join(format!("portal-{source}-restore-token")),
    )
}

fn load_restore_token(source: &str) -> Option<String> {
    let token = fs::read_to_string(restore_token_path(source)?).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

fn save_restore_token(source: &str, token: &str) -> Result<()> {
    let path = restore_token_path(source)
        .ok_or_else(|| anyhow!("No user configuration directory is available"))?;
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| anyhow!("Invalid portal token path"))?,
    )?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(token.as_bytes())?;
    Ok(())
}

fn spawn_encoder(
    input: std::process::ChildStdout,
    output: &Path,
    fps: u32,
    quality: &QualityPreset,
) -> Result<Child> {
    let ffmpeg_info = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .context("Failed to inspect FFmpeg encoders")?;
    let encoders = String::from_utf8_lossy(&ffmpeg_info.stdout);
    let render_node = fs::read_dir("/dev/dri")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        });

    let mut command = Command::new("ffmpeg");
    command
        .args(["-hide_banner", "-loglevel", "warning", "-y"])
        .args(["-f", "yuv4mpegpipe", "-i", "pipe:0", "-an"]);

    if encoders.contains("h264_vaapi") && render_node.is_some() {
        let qp = match quality {
            QualityPreset::Lossless => "12",
            QualityPreset::High => "18",
            QualityPreset::Medium => "23",
            QualityPreset::Low => "28",
        };
        command
            .arg("-vaapi_device")
            .arg(render_node.as_ref().unwrap())
            .args(["-vf", "format=nv12,hwupload", "-c:v", "h264_vaapi"])
            .args(["-rc_mode", "CQP", "-qp", qp]);
        println!(
            "Linux encoder: VA-API hardware H.264 on {}",
            render_node.unwrap().display()
        );
    } else {
        let crf = match quality {
            QualityPreset::Lossless => "12",
            QualityPreset::High => "18",
            QualityPreset::Medium => "23",
            QualityPreset::Low => "28",
        };
        command.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", crf]);
        println!("Linux encoder: libx264 fallback (no usable VA-API render node)");
    }

    command
        .args(["-g", &(fps * 2).to_string(), "-bf", "0"])
        .args(["-movflags", "+faststart"])
        .arg(output)
        .stdin(Stdio::from(input))
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to start the H.264 encoder")
}

fn spawn_pulse_audio(source: &str, output: &Path) -> Result<Child> {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "warning", "-y"])
        .args(["-f", "pulse", "-i", source, "-c:a", "pcm_s16le"])
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to capture audio source {source}"))
}

fn require_program(name: &str) -> Result<()> {
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Required Linux runtime '{name}' is not installed"))?;
    Ok(())
}

fn require_supported_ffmpeg() -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .context("Required Linux runtime 'ffmpeg' is not installed")?;
    let banner = String::from_utf8_lossy(&output.stdout);
    let major = parse_ffmpeg_major(&banner)
        .ok_or_else(|| anyhow!("Could not determine the installed FFmpeg version"))?;
    if !matches!(major, 8 | 9) {
        bail!("Tarantino for Linux supports FFmpeg 8 or 9; found FFmpeg {major}");
    }
    println!("Linux media runtime: FFmpeg {major}");
    Ok(())
}

fn parse_ffmpeg_major(banner: &str) -> Option<u32> {
    let version = banner
        .lines()
        .next()?
        .strip_prefix("ffmpeg version ")?
        .split_whitespace()
        .next()?
        .trim_start_matches(|character: char| !character.is_ascii_digit());
    version
        .split(|character: char| !character.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

fn require_gstreamer_element(name: &str) -> Result<()> {
    let status = Command::new("gst-inspect-1.0")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Could not inspect GStreamer element '{name}'"))?;
    if !status.success() {
        bail!("Required GStreamer element '{name}' is not installed");
    }
    Ok(())
}

fn signal(child: &Child, name: &str) {
    let _ = Command::new("kill")
        .arg(format!("-{name}"))
        .arg(child.id().to_string())
        .status();
}

#[cfg(test)]
mod tests {
    use super::parse_ffmpeg_major;

    #[test]
    fn accepts_ffmpeg_8_and_9_banner_formats() {
        assert_eq!(parse_ffmpeg_major("ffmpeg version 8.1 Copyright"), Some(8));
        assert_eq!(
            parse_ffmpeg_major("ffmpeg version n9.0.1 Copyright"),
            Some(9)
        );
    }
}
