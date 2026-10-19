use super::{RecordingAPI, RecordingState};
use tokio::sync::watch;

impl RecordingAPI {
    pub(crate) fn failure_receiver(&self) -> Option<watch::Receiver<Option<String>>> {
        self.linux_recording
            .as_ref()
            .map(|recording| recording.failure())
    }

    pub(crate) async fn abort_failed_recording(
        &mut self,
        failure: &watch::Receiver<Option<String>>,
    ) -> bool {
        if !matches!(
            self.current_state,
            RecordingState::Recording | RecordingState::Paused
        ) || !self
            .failure_receiver()
            .is_some_and(|current| current.same_channel(failure))
            || failure.borrow().is_none()
        {
            return false;
        }
        *self.stop_signal.lock().await = true;
        if let Some(mut recording) = self.linux_recording.take() {
            recording.close().await;
        }
        if let Some(audio) = self.audio_task.take() {
            if let Err(error) = audio.await {
                eprintln!("Microphone cleanup failed: {error}");
            }
        }
        self.current_state = RecordingState::Error;
        true
    }
}
