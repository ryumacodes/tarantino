use super::{Detector, reader::Reader};
use crate::cursor_renderer::planner::{input::RecordingInput, press::Kind};
use anyhow::{Context, Result, ensure};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

#[derive(Clone, PartialEq, Eq)]
struct VideoIdentity {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

impl VideoIdentity {
    fn read(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .context("Cannot locate hover analysis video")?;
        let metadata = path.metadata()?;
        Ok(Self {
            path,
            size: metadata.len(),
            modified: metadata.modified()?,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Key {
    video: VideoIdentity,
    input_hash: u64,
    source_size: (u32, u32),
}

type Entry = Arc<Mutex<Option<Arc<Vec<f64>>>>>;
#[derive(Default)]
struct Cache(VecDeque<(Key, Entry)>);

impl Cache {
    fn entry(&mut self, key: Key) -> Entry {
        let entry = self
            .0
            .iter()
            .position(|(existing, _)| *existing == key)
            .and_then(|index| self.0.remove(index))
            .map(|(_, entry)| entry)
            .unwrap_or_else(|| Arc::new(Mutex::new(None)));
        while self.0.len() >= 8 {
            let Some(index) = self
                .0
                .iter()
                .position(|(_, entry)| Arc::strong_count(entry) == 1)
            else {
                break;
            };
            self.0.remove(index);
        }
        self.0.push_back((key, entry.clone()));
        entry
    }
}

static CACHE: Lazy<Mutex<Cache>> = Lazy::new(|| Mutex::new(Cache::default()));

pub fn analyze(video: &Path, sidecar: &Path, source_size: (u32, u32)) -> Result<Arc<Vec<f64>>> {
    analyze_with_check(video, sidecar, source_size, &|| Ok(()))
}

pub fn analyze_with_check(video: &Path, sidecar: &Path, source_size: (u32, u32),
    check: &dyn Fn() -> Result<()>) -> Result<Arc<Vec<f64>>> {
    check()?;
    let bytes = std::fs::read(sidecar).context("Cannot read hover cursor input")?;
    let mut hash = DefaultHasher::new();
    bytes.hash(&mut hash);
    let key = Key {
        video: VideoIdentity::read(video)?,
        input_hash: hash.finish(),
        source_size,
    };
    let entry = CACHE.lock().entry(key.clone());
    let mut cached = loop {
        check()?;
        if let Some(cached) = entry.try_lock_for(std::time::Duration::from_millis(25)) {
            break cached;
        }
    };
    check()?;
    if let Some(times) = cached.as_ref() {
        return Ok(times.clone());
    }
    let times = analyze_uncached(video, &bytes, source_size, check)?;
    check()?;
    ensure!(
        VideoIdentity::read(video)? == key.video,
        "Video changed during hover analysis"
    );
    let times = Arc::new(times);
    *cached = Some(times.clone());
    Ok(times)
}

fn analyze_uncached(video: &Path, bytes: &[u8], source_size: (u32, u32),
    check: &dyn Fn() -> Result<()>) -> Result<Vec<f64>> {
    ensure!(
        source_size.0 > 0 && source_size.1 > 0,
        "Invalid hover source size"
    );
    let value = serde_json::from_slice(bytes).context("Invalid hover cursor input")?;
    let (input, samples) = RecordingInput::parse_for_hover(&value, source_size)?;
    if !samples.iter().any(|sample| sample.free) {
        return Ok(Vec::new());
    }
    let downscale = (input.width / source_size.0 as f64).min(1.0);
    let width = (source_size.0 as f64 * downscale).round().max(1.0) as u32;
    let height = (source_size.1 as f64 * downscale).round().max(1.0) as u32;
    let clicks = input
        .press_events
        .iter()
        .filter(|event| matches!(event.kind, Kind::Down | Kind::Up))
        .map(|event| event.time)
        .collect();
    let spans = super::spans::decode_spans(&samples);
    let mut detector = Detector::new(samples, clicks, width as f64 / input.width)?;
    let mut times = Vec::new();
    for [start, end] in spans {
        check()?;
        let mut reader = Reader::open_range(video, width, height, start, end)?;
        loop {
            check()?;
            let Some(frame) = reader.next_reusing(detector.take_recycled())? else { break };
            check()?;
            if let Some(time) = detector.push(frame)? {
                times.push(time);
            }
        }
    }
    Ok(times)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_interrupts_waiting_for_shared_analysis() {
        let root = std::env::temp_dir().join(format!("hover-wait-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let video = root.join("video.mp4");
        let sidecar = root.join("input.json");
        std::fs::write(&video, []).unwrap();
        let bytes = b"[]".to_vec();
        std::fs::write(&sidecar, &bytes).unwrap();
        let mut hash = DefaultHasher::new();
        bytes.hash(&mut hash);
        let entry = CACHE.lock().entry(Key {
            video: VideoIdentity::read(&video).unwrap(),
            input_hash: hash.finish(),
            source_size: (1, 1),
        });
        let held = entry.lock();
        let control = crate::video_processing::export_control::ExportControl::default();
        let calls = std::cell::Cell::new(0);
        let result = analyze_with_check(&video, &sidecar, (1, 1), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 4 { control.cancel(); }
            control.check()
        });
        assert_eq!(result.unwrap_err().to_string(), crate::video_processing::export_control::CANCELLED);
        assert_eq!(calls.get(), 4);
        assert!(held.is_none());
        drop(held);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn key(input_hash: u64) -> Key {
        Key {
            video: VideoIdentity {
                path: PathBuf::from("recording.mp4"),
                size: 100,
                modified: SystemTime::UNIX_EPOCH,
            },
            input_hash,
            source_size: (160, 160),
        }
    }
    #[test]
    fn cache_reuses_matching_work_and_invalidates_inputs_geometry_and_video() {
        let mut cache = Cache::default();
        let original = cache.entry(key(1));
        assert!(Arc::ptr_eq(&original, &cache.entry(key(1))));
        assert!(!Arc::ptr_eq(&original, &cache.entry(key(2))));
        let mut resized = key(1);
        resized.source_size = (320, 320);
        assert!(!Arc::ptr_eq(&original, &cache.entry(resized)));
        let mut replaced = key(1);
        replaced.video.modified += std::time::Duration::from_secs(1);
        assert!(!Arc::ptr_eq(&original, &cache.entry(replaced)));
        for n in 3..20 {
            cache.entry(key(n));
        }
        assert!(Arc::ptr_eq(&original, &cache.entry(key(1))));
        assert!(cache.0.len() <= 8);
    }
}
