//! Window-scoped export jobs, retained until the worker has released its resources.
use crate::video_processing::export_control::ExportControl;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};
static JOBS: LazyLock<Mutex<HashMap<String, Arc<ExportControl>>>> = LazyLock::new(Default::default);
pub struct ExportJob {
    window: String,
    pub control: Arc<ExportControl>,
}
impl ExportJob {
    pub fn begin(window: &str) -> Result<Self, String> {
        let mut jobs = JOBS.lock();
        if jobs.contains_key(window) {
            return Err("An export is already running in this window".into());
        }
        let control = Arc::new(ExportControl::default());
        jobs.insert(window.into(), control.clone());
        Ok(Self {
            window: window.into(),
            control,
        })
    }
}
impl Drop for ExportJob {
    fn drop(&mut self) {
        JOBS.lock().remove(&self.window);
    }
}
#[tauri::command]
pub fn cancel_export(window: tauri::WebviewWindow) -> bool {
    cancel_for_window(window.label())
}

pub fn cancel_for_window(label: &str) -> bool {
    JOBS.lock().get(label).is_some_and(|job| job.cancel())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jobs_are_window_scoped_and_stay_reserved_until_worker_cleanup() {
        let first = ExportJob::begin("cancel-test-first").unwrap();
        let second = ExportJob::begin("cancel-test-second").unwrap();
        assert!(cancel_for_window("cancel-test-first"));
        assert!(first.control.check().is_err());
        assert!(!cancel_for_window("nonexistent-export-window"));
        assert!(ExportJob::begin("cancel-test-first").is_err());
        assert!(second.control.check().is_ok());
        drop(first);
        assert!(ExportJob::begin("cancel-test-first").is_ok());
    }
}
