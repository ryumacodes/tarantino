//! Validate local recognition output before it becomes editable project data.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Transcript {
    pub locale: String,
    pub words: Vec<Word>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Word {
    pub id: String,
    pub text: String,
    pub start_ms: f64,
    pub end_ms: f64,
    #[serde(rename = "type")]
    pub kind: WordKind,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WordKind {
    Word,
    SoundLabel,
}

impl Transcript {
    pub fn parse(value: serde_json::Value, duration_ms: f64) -> Result<Self> {
        ensure!(
            duration_ms.is_finite() && duration_ms >= 0.0,
            "Invalid audio duration"
        );
        let transcript: Self = serde_json::from_value(value).context("Invalid speech response")?;
        ensure!(
            !transcript.locale.is_empty() && transcript.locale.len() <= 128,
            "Invalid speech locale"
        );
        ensure!(
            transcript.words.len() <= 1_000_000,
            "Too many transcript words"
        );
        let mut ids = HashSet::new();
        let mut previous_start = 0.0;
        for word in &transcript.words {
            ensure!(
                !word.id.is_empty()
                    && word.id.encode_utf16().count() <= 4096
                    && ids.insert(&word.id),
                "Invalid or duplicate transcript word ID"
            );
            ensure!(
                !word.text.trim().is_empty() && word.text.encode_utf16().count() <= 10_000,
                "Invalid transcript word text"
            );
            ensure!(
                word.start_ms.is_finite()
                    && word.end_ms.is_finite()
                    && word.start_ms >= previous_start
                    && word.end_ms >= word.start_ms
                    && word.end_ms <= duration_ms,
                "Transcript timing is outside its audio source"
            );
            previous_start = word.start_ms;
        }
        Ok(transcript)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> serde_json::Value {
        json!({"locale":"en-US","words":[
            {"id":"0","text":"Hello","startMs":0,"endMs":200,"type":"word"},
            {"id":"1","text":"world","startMs":150,"endMs":400,"type":"word"}
        ]})
    }
    #[test]
    fn accepts_overlapping_words_and_empty_speech() {
        assert_eq!(Transcript::parse(fixture(), 400.0).unwrap().words.len(), 2);
        assert!(Transcript::parse(json!({"locale":"en-US","words":[]}), 0.0).is_ok());
    }
    #[test]
    fn rejects_invalid_identity_text_and_source_timing() {
        for (field, value) in [
            ("id", json!("0")),
            ("text", json!(" ")),
            ("text", json!("😀".repeat(5001))),
            ("startMs", json!(-1)),
            ("endMs", json!(401)),
            ("endMs", json!(100)),
            ("type", json!("unknown")),
        ] {
            let mut input = fixture();
            input["words"][1][field] = value;
            assert!(Transcript::parse(input, 400.0).is_err(), "accepted {field}");
        }
        assert!(Transcript::parse(fixture(), f64::NAN).is_err());
        assert!(Transcript::parse(fixture(), -1.0).is_err());
    }
}
