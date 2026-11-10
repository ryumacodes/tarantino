use parking_lot::Mutex;
use std::{collections::HashMap, sync::{Arc, LazyLock, atomic::{AtomicBool, Ordering}}};

#[derive(Default)]
pub(super) struct Control {
    cancelled: AtomicBool,
    input: super::cursor_preview_input::InputCache,
}
impl Control {
    pub fn read_input(&self, path: &std::path::Path, retain: bool) -> anyhow::Result<Arc<serde_json::Value>> {
        self.input.read(path, retain, &|| self.check())
    }
    pub fn check(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.cancelled.load(Ordering::Relaxed), "Cursor preview cancelled");
        Ok(())
    }
}
struct Job { id: String, control: Arc<Control> }
static JOBS: LazyLock<Mutex<HashMap<String, Job>>> = LazyLock::new(Default::default);

pub(super) fn begin(owner: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let job = Job { id: id.clone(), control: Arc::new(Control::default()) };
    if let Some(old) = JOBS.lock().insert(owner.into(), job) {
        old.control.cancelled.store(true, Ordering::Relaxed);
    }
    id
}

pub(super) fn acquire(id: &str) -> Result<Arc<Control>, String> {
    JOBS.lock().values().find(|job| job.id == id).map(|job| job.control.clone())
        .ok_or_else(|| "Cursor preview cancelled".into())
}

pub(super) fn finish(id: &str) {
    JOBS.lock().retain(|_, job| job.id != id);
}

fn cancel(owner: &str, id: Option<&str>) {
    let mut jobs = JOBS.lock();
    if jobs.get(owner).is_some_and(|job| id.is_none_or(|id| job.id == id)) {
        let job = jobs.remove(owner).unwrap();
        job.control.cancelled.store(true, Ordering::Relaxed);
    }
}

pub fn cancel_for_window(owner: &str) { cancel(owner, None); }

#[tauri::command]
pub fn begin_cursor_preview(window: tauri::WebviewWindow) -> String { begin(window.label()) }

#[tauri::command]
pub fn cancel_cursor_preview(window: tauri::WebviewWindow, job_id: String) {
    cancel(window.label(), Some(&job_id));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_or_replaced_jobs_cannot_block_or_cancel_their_successors() {
        let owner = uuid::Uuid::new_v4().to_string();
        let first = begin(&owner);
        let old = acquire(&first).unwrap();
        let second = begin(&owner);
        let current = acquire(&second).unwrap();
        assert!(old.check().is_err());
        assert!(acquire(&first).is_err());
        cancel(&owner, Some(&first));
        finish(&first);
        assert!(current.check().is_ok());
        assert!(acquire(&second).is_ok());
        cancel_for_window(&owner);
        assert!(current.check().is_err());
        assert!(acquire(&second).is_err());
    }
    #[test]
    fn cancellation_is_window_scoped_and_completion_releases_the_entry() {
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        let first = begin(&a);
        let second = begin(&b);
        cancel(&a, Some(&second));
        assert!(acquire(&first).unwrap().check().is_ok());
        assert!(acquire(&second).unwrap().check().is_ok());
        finish(&first);
        assert!(acquire(&first).is_err());
        cancel_for_window(&b);
        assert!(acquire(&second).is_err());
    }
}
