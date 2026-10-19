//! Linux system-output capture, finalized with the video pipeline.

use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use anyhow::{Context, Result};
use gst::prelude::*;

const ELEMENTS: &[&str] = &[
    "pulsesrc",
    "queue",
    "audioconvert",
    "audioresample",
    "capsfilter",
    "wavenc",
    "filesink",
];

pub(super) fn available() -> bool {
    gst::init().is_ok()
        && ELEMENTS
            .iter()
            .all(|name| gst::ElementFactory::find(name).is_some())
}

pub(super) fn attach(
    pipeline: &gst::Pipeline,
    output_path: &Path,
    video_started: Arc<OnceLock<Instant>>,
) -> Result<()> {
    anyhow::ensure!(
        available(),
        "System audio requires the GStreamer PulseAudio and WAV plugins"
    );
    // PipeWire's PulseAudio service exposes the same default-output monitor.
    // Never fall back to the default source: that would record a microphone.
    let source = gst::ElementFactory::make("pulsesrc")
        .property("device", "@DEFAULT_MONITOR@")
        .property("client-name", "Tarantino system audio")
        .property("provide-clock", false)
        .build()
        .context("Failed to create the system-output monitor")?;
    attach_source(pipeline, source, output_path, video_started)
}

fn attach_source(
    pipeline: &gst::Pipeline,
    source: gst::Element,
    output_path: &Path,
    video_started: Arc<OnceLock<Instant>>,
) -> Result<()> {
    let queue = gst::ElementFactory::make("queue").build()?;
    let convert = gst::ElementFactory::make("audioconvert").build()?;
    let resample = gst::ElementFactory::make("audioresample").build()?;
    let caps = gst::Caps::builder("audio/x-raw")
        .field("format", "S16LE")
        .field("rate", 48_000i32)
        .field("channels", 2i32)
        .field("layout", "interleaved")
        .build();
    let filter = gst::ElementFactory::make("capsfilter")
        .property("caps", caps)
        .build()?;
    let encoder = gst::ElementFactory::make("wavenc").build()?;
    let sink = gst::ElementFactory::make("filesink")
        .property("location", output_path.to_string_lossy().as_ref())
        .property("async", false)
        .build()?;

    // Portal/video startup can lag behind the sound server. Start the sidecar
    // with the first captured video frame, rather than recording that lead-in.
    source
        .static_pad("src")
        .context("Audio source has no output pad")?
        .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
            if video_started.get().is_some() {
                gst::PadProbeReturn::Ok
            } else {
                gst::PadProbeReturn::Drop
            }
        });
    pipeline.add_many([
        &source, &queue, &convert, &resample, &filter, &encoder, &sink,
    ])?;
    gst::Element::link_many([
        &source, &queue, &convert, &resample, &filter, &encoder, &sink,
    ])
    .context("Failed to link the Linux system-audio sidecar")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GStreamer audio plugins and FFmpeg"]
    fn linux_system_audio_runtime_smoke() -> Result<()> {
        gst::init()?;
        let path =
            std::env::temp_dir().join(format!("tarantino-audio-{}.wav", uuid::Uuid::new_v4()));
        let pipeline = gst::Pipeline::new();
        let result = (|| -> Result<()> {
            let source = gst::ElementFactory::make("audiotestsrc")
                .property("num-buffers", 20i32)
                .property("samplesperbuffer", 480i32)
                .build()?;
            let started = Arc::new(OnceLock::new());
            started.set(Instant::now()).unwrap();
            attach_source(&pipeline, source, &path, started)?;
            pipeline.set_state(gst::State::Playing)?;
            super::super::wait_for_pipeline(&pipeline)?;
            pipeline.set_state(gst::State::Null)?;
            let probe = std::process::Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-show_entries",
                    "stream=codec_name,sample_rate,channels:format=duration",
                    "-of",
                    "json",
                ])
                .arg(&path)
                .output()?;
            anyhow::ensure!(probe.status.success(), "Audio sidecar is not playable");
            let value: serde_json::Value = serde_json::from_slice(&probe.stdout)?;
            let stream = &value["streams"][0];
            assert_eq!(stream["codec_name"], "pcm_s16le");
            assert_eq!(stream["sample_rate"], "48000");
            assert_eq!(stream["channels"], 2);
            let duration: f64 = value["format"]["duration"].as_str().unwrap().parse()?;
            assert!(
                (duration - 0.2).abs() < 0.01,
                "Unexpected audio duration: {duration}"
            );
            Ok(())
        })();
        let _ = pipeline.set_state(gst::State::Null);
        let _ = std::fs::remove_file(path);
        result
    }
}
