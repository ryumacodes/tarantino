//! Cancellation owns process handles, so blocked pipe reads/writes can be interrupted.
use anyhow::{Result, bail};
use parking_lot::Mutex;
use std::{
    io::Read,
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Weak},
};

pub const CANCELLED: &str = "Export cancelled";
#[derive(Debug, Default)]
struct Status {
    cancelled: bool,
    completed: bool,
    children: Vec<Weak<Mutex<Child>>>,
}
#[derive(Debug, Default)]
pub struct ExportControl {
    status: Mutex<Status>,
}
impl ExportControl {
    pub fn check(&self) -> Result<()> {
        if self.status.lock().cancelled {
            bail!(CANCELLED);
        }
        Ok(())
    }
    /// Drain both pipes concurrently, while cancellation can kill the registered child.
    pub fn output(&self, command: &mut Command) -> Result<Output> {
        self.check()?;
        let child = super::export::resources::ExportChild::new(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
            self,
        );
        let mut stdout = child.stdout().expect("stdout was piped");
        let mut stderr = child.stderr().expect("stderr was piped");
        let (status, stdout, stderr) = std::thread::scope(|scope| {
            let out = scope.spawn(move || {
                let mut bytes = Vec::new();
                stdout.read_to_end(&mut bytes).map(|_| bytes)
            });
            let err = scope.spawn(move || {
                let mut bytes = Vec::new();
                stderr.read_to_end(&mut bytes).map(|_| bytes)
            });
            let status = child.wait();
            if status.is_err() {
                let _ = child.kill();
            }
            (status, out.join(), err.join())
        });
        self.check()?;
        Ok(Output {
            status: status?,
            stdout: stdout.map_err(|_| anyhow::anyhow!("Probe stdout reader panicked"))??,
            stderr: stderr.map_err(|_| anyhow::anyhow!("Probe stderr reader panicked"))??,
        })
    }
    pub fn register(&self, child: &Arc<Mutex<Child>>) {
        let mut state = self.status.lock();
        if state.cancelled {
            let _ = child.lock().kill();
        }
        state.children.retain(|child| child.strong_count() > 0);
        state.children.push(Arc::downgrade(child));
    }
    pub fn cancel(&self) -> bool {
        let mut state = self.status.lock();
        if state.completed {
            return false;
        }
        state.cancelled = true;
        for child in state.children.iter().filter_map(Weak::upgrade) {
            let mut child = child.lock();
            if !matches!(child.try_wait(), Ok(Some(_))) {
                let _ = child.kill();
            }
        }
        true
    }
    /// Serialize cancellation with promotion: never report cancellation after replacing a file.
    pub fn commit<T>(&self, promote: impl FnOnce() -> Result<T>) -> Result<T> {
        let mut state = self.status.lock();
        if state.cancelled {
            bail!(CANCELLED);
        }
        let result = promote()?;
        state.completed = true;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn probe_drains_both_pipes_without_deadlocking() {
        let control = ExportControl::default();
        let output = control
            .output(Command::new("python3").args([
                "-c",
                "import sys; sys.stderr.write('e'*262144); sys.stdout.write('o'*262144)",
            ]))
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, vec![b'o'; 262144]);
        assert_eq!(output.stderr, vec![b'e'; 262144]);
    }

    #[test]
    fn cancellation_interrupts_probe_and_reaps_process() {
        let control = Arc::new(ExportControl::default());
        let worker_control = control.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = worker_control
                .output(Command::new("python3").args(["-c", "import time; time.sleep(60)"]));
            tx.send(result).unwrap();
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let child = loop {
            if let Some(child) = control
                .status
                .lock()
                .children
                .iter()
                .find_map(Weak::upgrade)
            {
                break child;
            }
            assert!(std::time::Instant::now() < deadline, "probe did not start");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert!(control.cancel());
        let result = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        assert_eq!(result.unwrap_err().to_string(), CANCELLED);
        worker.join().unwrap();
        assert!(child.lock().try_wait().unwrap().is_some());
    }

    #[test]
    fn cancelled_jobs_cannot_promote_and_completed_jobs_cannot_be_cancelled() {
        let cancelled = ExportControl::default();
        assert!(cancelled.cancel());
        assert!(cancelled.commit(|| Ok(())).is_err());
        let finished = ExportControl::default();
        assert_eq!(finished.commit(|| Ok(42)).unwrap(), 42);
        assert!(!finished.cancel());
        assert!(finished.check().is_ok());
    }
}
