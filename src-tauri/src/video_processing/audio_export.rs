use std::path::{Path, PathBuf};

use super::types::ExportSettings;

#[derive(Debug, Clone)]
pub struct AudioTrackFiles {
    pub mic: Option<PathBuf>,
    pub system: Option<PathBuf>,
}

impl AudioTrackFiles {
    pub fn discover(input_path: &Path) -> Self {
        let base = input_path.with_extension("");
        let mic = existing(base.with_extension("mic.wav"));
        let system = existing(base.with_extension("system.wav"));
        Self { mic, system }
    }

    pub fn has_any(&self) -> bool {
        self.mic.is_some() || self.system.is_some()
    }
}

pub fn append_audio_inputs(args: &mut Vec<String>, tracks: &AudioTrackFiles) -> usize {
    let mut count = 0;
    for path in [&tracks.mic, &tracks.system].into_iter().flatten() {
        args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
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

    let dual_track = settings
        .audio_settings
        .as_ref()
        .and_then(|a| a.dual_track)
        .unwrap_or(false);

    args.extend(["-map".to_string(), "0:v".to_string()]);
    if dual_track && (mic_idx.is_some() || system_idx.is_some()) {
        append_separate_tracks(args, mic_idx, system_idx);
    } else if let Some(filter) = build_audio_filter(settings, mic_idx, system_idx) {
        args.extend(["-filter_complex".to_string(), filter]);
        args.extend(["-map".to_string(), "[aout]".to_string()]);
        append_aac_args(args);
    } else if let Some(idx) = mic_idx.or(system_idx) {
        args.extend(["-map".to_string(), format!("{}:a", idx)]);
        append_aac_args(args);
    }
}

fn append_separate_tracks(
    args: &mut Vec<String>,
    mic_idx: Option<usize>,
    system_idx: Option<usize>,
) {
    if let Some(idx) = mic_idx {
        args.extend(["-map".to_string(), format!("{}:a", idx)]);
    }
    if let Some(idx) = system_idx {
        args.extend(["-map".to_string(), format!("{}:a", idx)]);
    }
    append_aac_args(args);
}

fn build_audio_filter(
    settings: &ExportSettings,
    mic_idx: Option<usize>,
    system_idx: Option<usize>,
) -> Option<String> {
    let audio = settings.audio_settings.as_ref();
    let mic_gain = db_to_volume(audio.and_then(|a| a.mic_gain).unwrap_or(0.0));
    let system_gain = db_to_volume(audio.and_then(|a| a.system_gain).unwrap_or(0.0));
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

fn append_aac_args(args: &mut Vec<String>) {
    args.extend([
        "-c:a".to_string(),
        "aac".to_string(),
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
