use std::path::{Path, PathBuf};

use crate::recording::artifacts::RecordingArtifacts;

use super::types::ExportSettings;

#[derive(Debug, Clone)]
pub struct AudioTrackFiles {
    pub mic: Option<PathBuf>,
    pub system: Option<PathBuf>,
    pub background: Option<PathBuf>,
}

impl AudioTrackFiles {
    pub fn discover(input_path: &Path) -> Self {
        let artifacts = RecordingArtifacts::new(input_path);
        let mic = existing(artifacts.microphone());
        let system = existing(artifacts.system_audio());
        Self { mic, system, background: None }
    }

    pub fn has_any(&self) -> bool {
        self.mic.is_some() || self.system.is_some() || self.background.is_some()
    }
}

pub fn append_audio_inputs(
    args: &mut Vec<String>,
    tracks: &AudioTrackFiles,
    settings: &ExportSettings,
) -> usize {
    let mut count = 0;
    for path in [&tracks.mic, &tracks.system].into_iter().flatten() {
        super::codec_config::add_trim_settings(args, settings);
        args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
        count += 1;
    }
    if let Some(path) = &tracks.background {
        // The soundtrack has already been trimmed on the edited playback clock.
        args.extend(["-i".into(), path.to_string_lossy().into_owned()]);
        count += 1;
    }
    count
}

pub fn append_audio_encode_args(
    args: &mut Vec<String>,
    settings: &ExportSettings,
    tracks: &AudioTrackFiles,
) {
    let mut next_input = 1;
    let mic_idx = tracks.mic.as_ref().map(|_| {
        let idx = next_input;
        next_input += 1;
        idx
    });
    let system_idx = tracks.system.as_ref().map(|_| {
        let idx = next_input;
        next_input += 1;
        idx
    });

    let background_idx = tracks.background.as_ref().map(|_| next_input);

    let dual_track = settings
        .audio_settings
        .as_ref()
        .and_then(|a| a.dual_track)
        .unwrap_or(false);

    args.extend(["-map".to_string(), "0:v".to_string()]);
    if dual_track && tracks.has_any() {
        append_separate_tracks(args, settings, mic_idx, system_idx, background_idx);
    } else if let Some(filter) = build_audio_filter(settings, mic_idx, system_idx, background_idx) {
        args.extend(["-filter_complex".to_string(), filter]);
        args.extend(["-map".to_string(), "[aout]".to_string()]);
        append_audio_codec_args(args, settings);
    } else if let Some(idx) = mic_idx.or(system_idx) {
        args.extend(["-map".to_string(), format!("{}:a", idx)]);
        append_audio_codec_args(args, settings);
    }
}

fn append_separate_tracks(
    args: &mut Vec<String>,
    settings: &ExportSettings,
    mic_idx: Option<usize>,
    system_idx: Option<usize>,
    background_idx: Option<usize>,
) {
    let mut filters = Vec::new();
    let mut labels = Vec::new();
    for (mic, system, background, label) in [(mic_idx, None, None, "mic_out"), (None, system_idx, None, "system_out"), (None, None, background_idx, "background_out")] {
        if let Some(filter) = build_audio_filter(settings, mic, system, background) {
            filters.push(filter.replace("[aout]", &format!("[{label}]")));
            labels.push(format!("[{label}]"));
        }
    }
    if !filters.is_empty() {
        args.extend(["-filter_complex".into(), filters.join(";")]);
        for label in labels {
            args.extend(["-map".into(), label]);
        }
    }
    append_audio_codec_args(args, settings);
}

fn build_audio_filter(
    settings: &ExportSettings,
    mic_idx: Option<usize>,
    system_idx: Option<usize>,
    background_idx: Option<usize>,
) -> Option<String> {
    let audio = settings.audio_settings.as_ref();
    let mic_gain = if audio.and_then(|a| a.mic_muted).unwrap_or(false) {
        0.0
    } else {
        db_to_volume(audio.and_then(|a| a.mic_gain).unwrap_or(0.0))
    };
    let system_gain = if audio.and_then(|a| a.system_muted).unwrap_or(false) {
        0.0
    } else {
        db_to_volume(audio.and_then(|a| a.system_gain).unwrap_or(0.0))
    };
    let noise_gate = audio.and_then(|a| a.noise_gate).unwrap_or(false);

    let mut filters = Vec::new();
    let mut labels = Vec::new();
    if let Some(idx) = mic_idx {
        let gate = if noise_gate {
            ",agate=threshold=0.015:ratio=8:attack=5:release=80"
        } else {
            ""
        };
        filters.push(format!("[{}:a]volume={:.4}{}[a_mic]", idx, mic_gain, gate));
        labels.push("[a_mic]".to_string());
    }
    if let Some(idx) = system_idx {
        filters.push(format!("[{}:a]volume={:.4}[a_system]", idx, system_gain));
        labels.push("[a_system]".to_string());
    }

    if let Some(idx) = background_idx {
        filters.push(format!("[{idx}:a]anull[a_background]"));
        labels.push("[a_background]".into());
    }
    if labels.is_empty() {
        return None;
    }
    if labels.len() == 1 {
        filters.push(format!("{}anull[aout]", labels[0]));
    } else {
        filters.push(format!(
            "{}amix=inputs={}:normalize=0[aout]",
            labels.join(""),
            labels.len()
        ));
    }
    Some(filters.join(";"))
}

fn append_audio_codec_args(args: &mut Vec<String>, settings: &ExportSettings) {
    args.extend([
        "-c:a".to_string(),
        if settings.format.as_deref() == Some("webm") {
            "libopus"
        } else {
            "aac"
        }
        .to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
        "-ac".to_string(),
        "2".to_string(),
    ]);
}

fn db_to_volume(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.exists().then_some(path)
}
