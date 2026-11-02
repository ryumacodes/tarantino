//! Local speech helper lifecycle. No user-provided command strings are executed.
mod media;
mod process;
pub(crate) mod response;
use anyhow::{Context, Result, ensure};
pub(crate) use process::{Cancellation, Limits, run_json};
use std::path::{Path, PathBuf};

pub(crate) async fn transcribe_media(
    helper: &Path,
    source: &Path,
    audio_track: u32,
    locale: &str,
    cancel: &Cancellation,
) -> Result<response::Transcript> {
    let audio = media::prepare_audio(source, audio_track, cancel).await?;
    transcribe_audio(helper, &audio.path, locale, audio.duration_ms, cancel).await
}

pub(crate) async fn transcribe_audio(
    helper: &Path,
    audio: &Path,
    locale: &str,
    duration_ms: f64,
    cancel: &Cancellation,
) -> Result<response::Transcript> {
    ensure!(audio.is_file(), "The selected audio file is missing");
    ensure!(
        !locale.is_empty() && locale.len() <= 128,
        "Invalid speech locale"
    );
    ensure!(
        duration_ms.is_finite() && duration_ms >= 0.0,
        "Invalid audio duration"
    );
    let value = run_json(
        helper,
        &["transcribe".into(), audio.as_os_str().into(), locale.into()],
        cancel,
        Limits::for_timeout(std::time::Duration::from_secs(3600)),
    )
    .await?;
    response::Transcript::parse(value, duration_ms)
}

fn adjacent_helper(executable: &Path) -> Result<PathBuf> {
    let helper = executable
        .parent()
        .context("Cannot locate application directory")?
        .join("tarantino-speech");
    ensure!(
        helper.is_file(),
        "The speech helper is missing. Reinstall or rebuild Tarantino."
    );
    Ok(helper)
}
pub(crate) fn helper_path() -> Result<PathBuf> {
    ensure!(
        cfg!(target_os = "macos"),
        "Local speech recognition currently requires macOS 26 or later."
    );
    adjacent_helper(&std::env::current_exe().context("Cannot locate Tarantino")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires packaged speech helper, installed model and synthetic audio"]
    async fn packaged_helper_runs_through_rust_bridge() {
        let helper = PathBuf::from(
            std::env::var_os("TARANTINO_SPEECH_HELPER").expect("Set packaged helper path"),
        );
        let audio = std::env::var_os("TARANTINO_SPEECH_AUDIO").expect("Set synthetic audio path");
        assert_eq!(
            adjacent_helper(&helper.parent().unwrap().join("tarantino")).unwrap(),
            helper
        );
        let cancel = Cancellation::default();
        let limits = Limits::for_timeout(std::time::Duration::from_secs(30));
        let status = run_json(&helper, &["status".into(), "en-US".into()], &cancel, limits)
            .await
            .unwrap();
        assert_eq!(status["available"], true);
        assert_eq!(status["modelStatus"], "installed");
        let transcript = transcribe_media(&helper, Path::new(&audio), 0, "en-US", &cancel)
            .await
            .unwrap();
        let words = &transcript.words;
        assert_eq!(words.len(), 12);
        for word in words {
            assert!(word.end_ms > word.start_ms);
        }
        println!(
            "Rust bridge received {} timestamped words from packaged helper",
            words.len()
        );
    }
}
