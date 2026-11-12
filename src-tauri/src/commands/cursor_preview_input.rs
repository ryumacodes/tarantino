use parking_lot::Mutex;
use serde_json::Value;
use std::{path::{Path, PathBuf}, sync::{Arc, Weak}, time::SystemTime};

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    modified: SystemTime,
    len: u64,
    value: Weak<Value>,
    retained: Option<Arc<Value>>,
}

/// Bounded requests and handoffs retain input; full requests leave only a weak reference.
#[derive(Default)]
pub(super) struct InputCache(Mutex<Option<Entry>>);

impl InputCache {
    pub fn fork(&self) -> Self {
        let mut entry = self.0.lock().clone();
        if let Some(entry) = entry.as_mut() { entry.retained = entry.value.upgrade(); }
        Self(Mutex::new(entry))
    }

    pub fn read(&self, path: &Path, retain: bool, check: &dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<Arc<Value>> {
        check()?;
        let metadata = path.metadata()?;
        let modified = metadata.modified()?;
        {
            let mut cached = self.0.lock();
            if let Some(entry) = cached.as_mut() {
                if entry.path == path && entry.modified == modified && entry.len == metadata.len() {
                    if let Some(value) = entry.value.upgrade() {
                        entry.retained = retain.then(|| value.clone());
                        return Ok(value);
                    }
                }
            }
            *cached = None;
        }
        let value: Arc<Value> = Arc::new(serde_json::from_slice(&std::fs::read(path)?)?);
        check()?;
        *self.0.lock() = Some(Entry { path: path.into(), modified, len: metadata.len(),
            value: Arc::downgrade(&value), retained: retain.then(|| value.clone()) });
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_parsed_input_but_observes_file_changes_and_paths() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        let a = root.join("a.json");
        let b = root.join("b.json");
        std::fs::write(&a, "[1]").unwrap();
        std::fs::write(&b, "[2]").unwrap();
        let cache = InputCache::default();
        let first = cache.read(&a, true, &|| Ok(())).unwrap();
        assert!(Arc::ptr_eq(&first, &cache.read(&a, true, &|| Ok(())).unwrap()));
        assert!(Arc::ptr_eq(&first, &cache.read(&a, false, &|| Ok(())).unwrap()));
        assert!(cache.0.lock().as_ref().unwrap().retained.is_none());
        cache.read(&a, true, &|| Ok(())).unwrap();
        std::fs::write(&a, "[1,3]").unwrap();
        assert_eq!(*cache.read(&a, true, &|| Ok(())).unwrap(), serde_json::json!([1,3]));
        assert_eq!(*cache.read(&b, true, &|| Ok(())).unwrap(), serde_json::json!([2]));
        cache.read(&a, true, &|| Ok(())).unwrap();
        std::fs::remove_file(a.clone()).unwrap();
        assert!(cache.read(&a, true, &|| Ok(())).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fork_reuses_live_input_without_extending_full_request_retention() {
        let path = std::env::temp_dir().join(format!("{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, "[1]").unwrap();
        let cache = InputCache::default();
        let value = cache.read(&path, false, &|| Ok(())).unwrap();
        let weak = Arc::downgrade(&value);
        let next = cache.fork();
        drop(value);
        assert!(weak.upgrade().is_some(), "replacement must retain input until its request starts");
        let reused = next.read(&path, false, &|| Ok(())).unwrap();
        assert!(Arc::ptr_eq(&weak.upgrade().unwrap(), &reused));
        drop(reused);
        assert!(weak.upgrade().is_none());
        assert_eq!(*next.read(&path, false, &|| Ok(())).unwrap(), serde_json::json!([1]));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cancellation_does_not_return_or_retain_an_input() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        let path = root.join("input.json");
        std::fs::write(&path, "[1]").unwrap();
        let cache = InputCache::default();
        let checks = std::cell::Cell::new(0);
        assert!(cache.read(&path, true, &|| {
            checks.set(checks.get() + 1);
            anyhow::ensure!(checks.get() < 2, "cancelled");
            Ok(())
        }).is_err());
        assert!(cache.0.lock().is_none());
        cache.read(&path, true, &|| Ok(())).unwrap();
        assert!(cache.read(&path, true, &|| anyhow::bail!("cancelled")).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
