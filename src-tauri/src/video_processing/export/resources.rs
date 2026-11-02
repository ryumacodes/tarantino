//! Owned resources for export success, failure, and future cancellation paths.
use anyhow::Result;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    process::Child,
};

pub(super) struct ExportOutput {
    staging: PathBuf,
    destination: PathBuf,
    committed: bool,
}
impl ExportOutput {
    pub fn new(destination: &Path) -> Result<Self> {
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let extension = destination
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("mp4");
        let staging = parent.join(format!(
            ".tarantino-export-{}.{}",
            uuid::Uuid::new_v4(),
            extension
        ));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        Ok(Self {
            staging,
            destination: destination.into(),
            committed: false,
        })
    }
    pub fn path(&self) -> &Path {
        &self.staging
    }
    pub fn commit(mut self) -> Result<PathBuf> {
        File::open(&self.staging)?.sync_all()?;
        // Both paths share a filesystem. Failed replacement leaves the destination intact.
        std::fs::rename(&self.staging, &self.destination)?;
        self.committed = true;
        Ok(self.destination.clone())
    }
}
impl Drop for ExportOutput {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.staging);
        }
    }
}

pub(crate) struct ExportChild(std::sync::Arc<parking_lot::Mutex<Child>>);
impl ExportChild {
    pub fn new(child: Child, control: &super::super::export_control::ExportControl) -> Self {
        let child = std::sync::Arc::new(parking_lot::Mutex::new(child));
        control.register(&child);
        Self(child)
    }
    pub fn stdout(&self) -> Option<std::process::ChildStdout> {
        self.0.lock().stdout.take()
    }
    pub fn stderr(&self) -> Option<std::process::ChildStderr> {
        self.0.lock().stderr.take()
    }
    pub fn stdin(&self) -> Option<std::process::ChildStdin> {
        self.0.lock().stdin.take()
    }
    pub fn kill(&self) -> std::io::Result<()> {
        self.0.lock().kill()
    }
    pub fn try_wait(&self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.0.lock().try_wait()
    }
    pub fn wait(&self) -> std::io::Result<std::process::ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    fn terminate(&self) {
        let mut child = self.0.lock();
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Drop for ExportChild {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_export_preserves_existing_destination_and_removes_only_its_staging_file() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let destination = root.join("recording.mp4");
        let first = ExportOutput::new(&destination).unwrap();
        let second = ExportOutput::new(&destination).unwrap();
        assert_ne!(first.path(), second.path());
        std::fs::write(&destination, b"original recording").unwrap();
        std::fs::write(first.path(), b"partial video").unwrap();
        let staging = first.path().to_owned();
        drop(first);
        assert!(!staging.exists());
        assert!(second.path().exists());
        assert_eq!(std::fs::read(&destination).unwrap(), b"original recording");
        std::fs::write(second.path(), b"completed video").unwrap();
        second.commit().unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"completed video");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_promotion_cleans_staging_and_keeps_destination() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let destination = root.join("existing.mp4");
        let output = ExportOutput::new(&destination).unwrap();
        let staging = output.path().to_owned();
        std::fs::write(&staging, b"completed bytes").unwrap();
        std::fs::create_dir(&destination).unwrap();
        assert!(output.commit().is_err());
        assert!(!staging.exists());
        assert!(destination.is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancellation_interrupts_a_blocked_pipe_read() {
        use std::{
            io::Read,
            process::{Command, Stdio},
            sync::{Arc, mpsc},
            time::Duration,
        };
        let control = Arc::new(super::super::super::export_control::ExportControl::default());
        let child = Command::new("python3")
            .args([
                "-c",
                "import sys,time; sys.stdout.write('x'); sys.stdout.flush(); time.sleep(60)",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let child = ExportChild::new(child, &control);
        let mut stdout = child.stdout().unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            stdout.read_exact(&mut [0]).unwrap();
            ready_tx.send(()).unwrap();
            let result = stdout.read_exact(&mut [0]);
            drop(child);
            done_tx.send(result.is_err()).unwrap();
        });
        ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(control.cancel());
        assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn owned_process_cleanup_terminates_and_reaps_it() {
        let child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let child = ExportChild::new(child, &Default::default());
        child.terminate();
        assert!(child.0.lock().try_wait().unwrap().is_some());
        child.terminate(); // Repeated cleanup must not target an unrelated process.
    }
}
