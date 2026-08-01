//! Linux recording through the XDG ScreenCast portal and PipeWire.

use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
};
use tokio::process::{Child, Command};

use super::{QualityPreset, RecordingConfig, RecordingTarget};

const PIPEWIRE_CHILD_FD: i32 = 3;

pub struct LinuxRecording {
    child: Child,
    // The portal objects and file descriptor must remain alive until GStreamer exits.
    _portal: Screencast,
    _session: ashpd::desktop::Session<Screencast>,
    _pipewire_fd: std::os::fd::OwnedFd,
}

impl LinuxRecording {
    pub async fn start(config: &RecordingConfig, output_path: &Path) -> Result<Self> {
        check_runtime_dependencies().await?;

        if config.include_system_audio {
            anyhow::bail!(
                "System audio capture is not available on Linux yet; disable system audio and try again"
            );
        }

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
                    .set_sources(source_type)
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
        let node_id = stream.pipe_wire_node_id();
        let pipewire_fd = portal
            .open_pipe_wire_remote(&session, Default::default())
            .await
            .context("Failed to open the portal PipeWire remote")?;

        let mut command = build_gstreamer_command(config, output_path, node_id);
        let portal_fd = pipewire_fd.as_raw_fd();
        // SAFETY: this closure only calls async-signal-safe libc functions between fork and exec.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(portal_fd, PIPEWIRE_CHILD_FD) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::fcntl(PIPEWIRE_CHILD_FD, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let mut child = command
            .spawn()
            .context("Failed to start gst-launch-1.0 for Linux recording")?;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if let Some(status) = child
            .try_wait()
            .context("Failed to inspect the GStreamer recorder")?
        {
            anyhow::bail!("GStreamer recorder exited during startup with {status}");
        }

        Ok(Self {
            child,
            _portal: portal,
            _session: session,
            _pipewire_fd: pipewire_fd,
        })
    }

    pub fn signal_stop(&mut self) -> Result<()> {
        let Some(pid) = self.child.id() else {
            return Ok(());
        };
        // gst-launch -e converts SIGINT into EOS, allowing mp4mux to finalize the file.
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGINT) };
        if result == -1 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error).context("Failed to stop the GStreamer recorder");
            }
        }
        Ok(())
    }

    pub async fn wait(mut self) -> Result<()> {
        let status = self
            .child
            .wait()
            .await
            .context("Failed to wait for the GStreamer recorder")?;
        if !status.success() {
            anyhow::bail!("GStreamer recorder exited with {status}");
        }
        Ok(())
    }
}

async fn check_runtime_dependencies() -> Result<()> {
    let gst = Command::new("gst-launch-1.0")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context(
            "gst-launch-1.0 is required for Linux recording; install the GStreamer tools and plugins",
        )?;
    if !gst.success() {
        anyhow::bail!("gst-launch-1.0 is installed but could not start");
    }

    let pipewire_plugin = Command::new("gst-inspect-1.0")
        .arg("pipewiresrc")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("gst-inspect-1.0 is required to verify the PipeWire plugin")?;
    if !pipewire_plugin.success() {
        anyhow::bail!(
            "The GStreamer PipeWire source plugin is missing; install the PipeWire GStreamer plugin"
        );
    }

    let h264_encoder = Command::new("gst-inspect-1.0")
        .arg("x264enc")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("gst-inspect-1.0 is required to verify the H.264 encoder")?;
    if !h264_encoder.success() {
        anyhow::bail!(
            "The GStreamer x264 encoder is missing; install the GStreamer ugly plugin collection"
        );
    }
    Ok(())
}

fn build_gstreamer_command(config: &RecordingConfig, output_path: &Path, node_id: u32) -> Command {
    let (bitrate_kbps, speed_preset) = match config.quality {
        QualityPreset::Lossless => (32_000, "medium"),
        QualityPreset::High => (16_000, "fast"),
        QualityPreset::Medium => (8_000, "veryfast"),
        QualityPreset::Low => (4_000, "superfast"),
    };

    let mut command = Command::new("gst-launch-1.0");
    command
        .arg("-e")
        .arg("pipewiresrc")
        .arg(format!("fd={PIPEWIRE_CHILD_FD}"))
        .arg(format!("path={node_id}"))
        .arg("do-timestamp=true")
        .arg("!")
        .arg("queue")
        .arg("!")
        .arg("videoconvert")
        .arg("!")
        .arg("video/x-raw,format=I420")
        .arg("!")
        .arg("x264enc")
        .arg(format!("bitrate={bitrate_kbps}"))
        .arg(format!("speed-preset={speed_preset}"))
        .arg("tune=zerolatency")
        .arg("key-int-max=120")
        .arg("!")
        .arg("h264parse")
        .arg("!")
        .arg("mp4mux")
        .arg("faststart=true")
        .arg("!")
        .arg("filesink")
        .arg(format!("location={}", output_path.to_string_lossy()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_uses_portal_fd_and_selected_node() {
        let config = RecordingConfig::default();
        let command = build_gstreamer_command(&config, Path::new("/tmp/test recording.mp4"), 42);
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.contains(&"fd=3".to_string()));
        assert!(args.contains(&"path=42".to_string()));
        assert!(args.contains(&"location=/tmp/test recording.mp4".to_string()));
        assert!(!args.iter().any(|arg| arg.contains("x11grab")));
    }
}
