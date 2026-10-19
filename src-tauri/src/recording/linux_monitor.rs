use gst::prelude::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::watch;

pub(super) struct PipelineMonitor {
    bus: gst::Bus,
    stopping: Arc<AtomicBool>,
    failure: watch::Receiver<Option<String>>,
}

impl PipelineMonitor {
    pub fn new(pipeline: &gst::Pipeline) -> Self {
        let bus = pipeline.bus().expect("Pipeline has a bus");
        let stopping = Arc::new(AtomicBool::new(false));
        let is_stopping = stopping.clone();
        let (sender, failure) = watch::channel(None);
        bus.set_sync_handler(move |_, message| {
            let error = match message.view() {
                gst::MessageView::Error(error) => {
                    Some(format!("Recording failed: {}", error.error()))
                }
                gst::MessageView::Eos(..) if !is_stopping.load(Ordering::Acquire) => {
                    Some("The desktop capture stream ended unexpectedly.".to_string())
                }
                _ => None,
            };
            if let Some(error) = error {
                sender.send_if_modified(|current| {
                    if current.is_some() {
                        return false;
                    }
                    *current = Some(error);
                    true
                });
            }
            // Finalization still needs the original EOS/error on the bus.
            gst::BusSyncReply::Pass
        });
        Self {
            bus,
            stopping,
            failure,
        }
    }

    pub fn failure(&self) -> watch::Receiver<Option<String>> {
        self.failure.clone()
    }

    pub fn stopping(&self) {
        self.stopping.store(true, Ordering::Release);
    }
}

impl Drop for PipelineMonitor {
    fn drop(&mut self) {
        self.bus.unset_sync_handler();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pipeline_error_is_reported_without_pressing_stop() {
        gst::init().unwrap();
        let pipeline =
            gst::parse::launch("fakesrc num-buffers=10 ! identity error-after=1 ! fakesink")
                .unwrap()
                .downcast::<gst::Pipeline>()
                .unwrap();
        let monitor = PipelineMonitor::new(&pipeline);
        let mut failure = monitor.failure();
        let _ = pipeline.set_state(gst::State::Playing);
        tokio::time::timeout(std::time::Duration::from_secs(5), failure.changed())
            .await
            .unwrap()
            .unwrap();
        assert!(
            failure
                .borrow()
                .as_ref()
                .unwrap()
                .contains("Recording failed")
        );
        pipeline.set_state(gst::State::Null).unwrap();
    }

    #[test]
    fn normal_stop_preserves_eos_without_reporting_failure() {
        gst::init().unwrap();
        let pipeline = gst::Pipeline::new();
        let monitor = PipelineMonitor::new(&pipeline);
        monitor.stopping();
        pipeline
            .bus()
            .unwrap()
            .post(gst::message::Eos::new())
            .unwrap();
        assert!(monitor.failure().borrow().is_none());
        assert!(pipeline.bus().unwrap().pop().unwrap().type_() == gst::MessageType::Eos);
    }

    #[test]
    fn unexpected_eos_is_reported_and_old_recordings_are_distinct() {
        gst::init().unwrap();
        let first = gst::Pipeline::new();
        let monitor = PipelineMonitor::new(&first);
        let second = gst::Pipeline::new();
        let next = PipelineMonitor::new(&second);
        first.bus().unwrap().post(gst::message::Eos::new()).unwrap();
        assert!(monitor.failure().borrow().is_some());
        assert!(next.failure().borrow().is_none());
        assert!(!monitor.failure().same_channel(&next.failure()));
    }

    #[tokio::test]
    async fn dropping_monitor_releases_waiting_task() {
        gst::init().unwrap();
        let pipeline = gst::Pipeline::new();
        let monitor = PipelineMonitor::new(&pipeline);
        let mut failure = monitor.failure();
        drop(monitor);
        assert!(failure.changed().await.is_err());
    }
}
