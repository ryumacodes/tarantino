use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct RecordingArtifacts {
    video: PathBuf,
}

impl RecordingArtifacts {
    pub fn new(video: impl AsRef<Path>) -> Self {
        Self {
            video: video.as_ref().to_path_buf(),
        }
    }

    pub fn auto_zoom(&self) -> PathBuf {
        self.video.with_extension("auto_zoom.json")
    }

    pub fn mouse(&self) -> PathBuf {
        self.video.with_extension("mouse.json")
    }

    pub fn webcam_mp4(&self) -> PathBuf {
        self.video.with_extension("webcam.mp4")
    }

    pub fn webcam_webm(&self) -> PathBuf {
        self.video.with_extension("webcam.webm")
    }

    pub fn microphone(&self) -> PathBuf {
        self.video.with_extension("mic.wav")
    }

    pub fn system_audio(&self) -> PathBuf {
        self.video.with_extension("system.wav")
    }

    pub fn window_mask(&self) -> PathBuf {
        self.video.with_extension("window-mask.png")
    }

    pub fn all_paths(&self) -> [PathBuf; 8] {
        [
            self.video.clone(),
            self.auto_zoom(),
            self.mouse(),
            self.webcam_mp4(),
            self.webcam_webm(),
            self.microphone(),
            self.system_audio(),
            self.window_mask(),
        ]
    }

    pub fn sidecar_pairs(&self, destination: &Self) -> [(PathBuf, PathBuf); 7] {
        [
            (self.window_mask(), destination.window_mask()),
            (self.mouse(), destination.mouse()),
            (self.auto_zoom(), destination.auto_zoom()),
            (self.webcam_mp4(), destination.webcam_mp4()),
            (self.webcam_webm(), destination.webcam_webm()),
            (self.microphone(), destination.microphone()),
            (self.system_audio(), destination.system_audio()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::RecordingArtifacts;

    #[test]
    fn derives_every_sidecar_from_the_video_stem() {
        let artifacts = RecordingArtifacts::new("/tmp/demo.take.mp4");
        assert_eq!(
            artifacts.mouse().to_string_lossy(),
            "/tmp/demo.take.mouse.json"
        );
        assert_eq!(
            artifacts.auto_zoom().to_string_lossy(),
            "/tmp/demo.take.auto_zoom.json"
        );
        assert_eq!(
            artifacts.window_mask().to_string_lossy(),
            "/tmp/demo.take.window-mask.png"
        );
    }
}
