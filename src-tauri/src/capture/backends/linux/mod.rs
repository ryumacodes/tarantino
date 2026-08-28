//! Linux source discovery facade.
//!
//! Wayland deliberately prevents applications from silently enumerating other
//! applications' windows.  The desktop portal owns that chooser, so these
//! entries represent the two portal choices and the real source is selected in
//! the compositor-provided dialog when recording starts.

use anyhow::Result;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use super::*;

/// PipeWire backend for Linux
pub struct PipeWireBackend {
    frame_sender: Arc<Mutex<Option<broadcast::Sender<CapturedFrame>>>>,
    is_active: Arc<Mutex<bool>>,
}

impl PipeWireBackend {
    pub fn new() -> Result<Self> {
        // TODO: Connect to PipeWire daemon
        Ok(Self {
            frame_sender: Arc::new(Mutex::new(None)),
            is_active: Arc::new(Mutex::new(false)),
        })
    }

    fn enumerate_displays() -> Vec<CaptureSourceInfo> {
        let output = Command::new("xrandr").arg("--query").output();
        let stdout = match output {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).into_owned()
            }
            _ => {
                return vec![CaptureSourceInfo {
                    id: 1,
                    name: "Primary display".to_string(),
                    source_type: CaptureSourceType::Display,
                    width: 0,
                    height: 0,
                    x: 0,
                    y: 0,
                    scale_factor: 1.0,
                    is_primary: true,
                    owner_name: "Desktop Portal".to_string(),
                }];
            }
        };

        let mut displays = stdout
            .lines()
            .filter(|line| line.contains(" connected "))
            .enumerate()
            .filter_map(|(index, line)| parse_xrandr_display(line, index as u64 + 1))
            .collect::<Vec<_>>();

        if !displays.is_empty() && !displays.iter().any(|display| display.is_primary) {
            displays[0].is_primary = true;
        }

        // Keep the default selection stable across upgrades from the former
        // placeholder source: display 1 is always the compositor's primary
        // output, followed by the remaining outputs in xrandr order.
        displays.sort_by_key(|display| !display.is_primary);
        for (index, display) in displays.iter_mut().enumerate() {
            display.id = index as u64 + 1;
        }
        displays
    }
}

fn parse_xrandr_display(line: &str, id: u64) -> Option<CaptureSourceInfo> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let name = fields.first()?.to_string();
    let geometry = fields.iter().find(|field| {
        field.contains('x')
            && (field.contains('+') || field.rmatches('-').next().is_some_and(|_| true))
            && field.chars().next().is_some_and(|ch| ch.is_ascii_digit())
    })?;
    let normalized = geometry.replace('-', "+-");
    let mut parts = normalized.split('+');
    let size = parts.next()?;
    let (width, height) = size.split_once('x')?;
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;

    Some(CaptureSourceInfo {
        id,
        name,
        source_type: CaptureSourceType::Display,
        width: width.parse().ok()?,
        height: height.parse().ok()?,
        x,
        y,
        scale_factor: 1.0,
        is_primary: fields.contains(&"primary"),
        owner_name: "KDE Display Configuration".to_string(),
    })
}

#[async_trait::async_trait]
impl NativeCaptureBackend for PipeWireBackend {
    async fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>> {
        let mut sources = Self::enumerate_displays();
        sources.push(CaptureSourceInfo {
            id: 10_000,
            name: "Window picker".to_string(),
            source_type: CaptureSourceType::Window,
            width: 0,
            height: 0,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            is_primary: false,
            owner_name: "Desktop Portal".to_string(),
        });
        Ok(sources)
    }

    async fn check_permissions(&self) -> Result<PermissionStatus> {
        // Linux permissions handled via xdg-desktop-portal
        Ok(PermissionStatus {
            screen_recording: true,
            microphone: true,
            camera: true,
        })
    }

    async fn request_permissions(&self) -> Result<PermissionStatus> {
        self.check_permissions().await
    }

    async fn start_capture(&mut self, _config: CaptureConfig) -> Result<()> {
        anyhow::bail!(
            "Linux recording is managed by the portal pipeline; frame-stream capture is unavailable"
        )
    }

    async fn stop_capture(&mut self) -> Result<()> {
        *self.is_active.lock().unwrap() = false;
        Ok(())
    }

    fn frame_receiver(&self) -> Option<broadcast::Receiver<CapturedFrame>> {
        self.frame_sender
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.subscribe())
    }

    fn audio_receiver(&self) -> Option<broadcast::Receiver<CapturedAudio>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse_xrandr_display;

    #[test]
    fn parses_primary_and_scaled_xwayland_displays() {
        let display = parse_xrandr_display(
            "DP-2 connected primary 2194x1234+0+0 (normal left inverted right x axis y axis)",
            2,
        )
        .unwrap();
        assert_eq!(display.name, "DP-2");
        assert_eq!((display.width, display.height), (2194, 1234));
        assert!(display.is_primary);
    }

    #[test]
    fn parses_negative_display_offsets() {
        let display = parse_xrandr_display("DP-1 connected 1920x1080-1920+0", 1).unwrap();
        assert_eq!((display.x, display.y), (-1920, 0));
    }
}
