//! Linux-only packaged runtime setup and a headless dependency smoke check.
use anyhow::{Context, Result, bail};
use gst::prelude::*;
use std::process::Command;

pub fn configure() {
    // AppImage media helpers live beside the executable. Configure before any
    // threads start so every existing FFmpeg caller uses the bundled tools.
    if std::env::var_os("APPDIR").is_some() {
        if let Ok(executable) = std::env::current_exe() {
            if let Some(bin) = executable.parent() {
                if bin.join("ffmpeg").is_file() && bin.join("ffprobe").is_file() {
                    let mut paths = vec![bin.to_path_buf()];
                    paths.extend(std::env::split_paths(
                        &std::env::var_os("PATH").unwrap_or_default(),
                    ));
                    if let Ok(path) = std::env::join_paths(paths) {
                        unsafe { std::env::set_var("PATH", path) };
                    }
                }
            }
        }
    }

    // Apply the existing KDE Wayland shell workaround to packaged launches too.
    // Preserve an explicit backend choice. PipeWire capture stays unchanged.
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_lowercase();
    if (desktop.contains("kde") || desktop.contains("plasma"))
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_some()
        && std::env::var_os("GDK_BACKEND").is_none()
    {
        unsafe { std::env::set_var("GDK_BACKEND", "x11") };
    }
}

pub fn check() -> Result<()> {
    gst::init()?;
    for tool in ["ffmpeg", "ffprobe"] {
        let output = Command::new(tool)
            .arg("-version")
            .output()
            .with_context(|| format!("Cannot execute {tool}"))?;
        if !output.status.success() {
            bail!("{tool} failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        println!(
            "{}",
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or(tool)
        );
    }
    for name in [
        "v4l2src",
        "pipewiresrc",
        "videorate",
        "h264parse",
        "mp4mux",
        "pulsesrc",
        "audioconvert",
        "audioresample",
        "wavenc",
    ] {
        if gst::ElementFactory::find(name).is_none() {
            bail!("Missing packaged GStreamer element: {name}");
        }
    }
    let encoder = ["x264enc", "openh264enc"]
        .into_iter()
        .find(|name| gst::ElementFactory::find(name).is_some())
        .context("Missing portable H.264 fallback encoder")?;
    // No display, audio device or GPU required; this checks the packaged media
    // stack, not portal access or hardware acceleration on the user's desktop.
    let pipeline = gst::parse::launch(&format!(
        "videotestsrc num-buffers=6 ! video/x-raw,width=64,height=64,framerate=30/1 \
         ! videoconvert ! {encoder} ! h264parse ! mp4mux ! fakesink \
         audiotestsrc num-buffers=6 ! audioconvert ! audioresample ! wavenc ! fakesink"
    ))?
    .downcast::<gst::Pipeline>()
    .map_err(|_| anyhow::anyhow!("Expected media pipeline"))?;
    let result = (|| -> Result<()> {
        pipeline.set_state(gst::State::Playing)?;
        let message = pipeline
            .bus()
            .context("Missing pipeline bus")?
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(30),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .context("Packaged media smoke check timed out")?;
        if let gst::MessageView::Error(error) = message.view() {
            bail!(
                "Packaged media pipeline failed: {} ({:?})",
                error.error(),
                error.debug()
            );
        }
        Ok(())
    })();
    pipeline.set_state(gst::State::Null)?;
    result?;
    println!("Linux packaged runtime check passed (software encoder: {encoder}).");
    Ok(())
}
