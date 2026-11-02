use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{
    ffi::OsString,
    path::Path,
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::watch,
};

#[derive(Clone)]
pub(crate) struct Cancellation {
    sender: watch::Sender<bool>,
}
impl Default for Cancellation {
    fn default() -> Self {
        let (sender, _) = watch::channel(false);
        Self { sender }
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }
    pub fn check(&self) -> Result<()> {
        ensure!(!*self.sender.borrow(), "Transcription cancelled");
        Ok(())
    }
    async fn cancelled(&self) {
        let mut receiver = self.sender.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub timeout: Duration,
    pub stdout: usize,
    pub stderr: usize,
}
impl Limits {
    pub fn for_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            stdout: 64 * 1024 * 1024,
            stderr: 64 * 1024,
        }
    }
}
async fn read_bounded(reader: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(
        bytes.len() <= limit,
        "Speech helper output exceeded its memory limit"
    );
    Ok(bytes)
}
pub(super) async fn run(
    command: &mut Command,
    cancel: &Cancellation,
    limits: Limits,
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
    cancel.check()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Could not start speech helper")?;
    let stdout = child.stdout.take().context("Missing helper output pipe")?;
    let stderr = child.stderr.take().context("Missing helper error pipe")?;
    let result = tokio::select! {
        biased;
        _=cancel.cancelled()=>Err(anyhow::anyhow!("Transcription cancelled")),
        _=tokio::time::sleep(limits.timeout)=>Err(anyhow::anyhow!("Speech helper timed out")),
        result=async {
            let(status,stdout,stderr)=tokio::try_join!(
                async {child.wait().await.map_err(anyhow::Error::from)},
                read_bounded(stdout,limits.stdout),read_bounded(stderr,limits.stderr))?;
            Ok((status,stdout,stderr))
        }=>result,
    };
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    cancel.check()?;
    result
}
pub(crate) async fn run_json(
    path: &Path,
    args: &[OsString],
    cancel: &Cancellation,
    limits: Limits,
) -> Result<Value> {
    let (status, stdout, stderr) = run(Command::new(path).args(args), cancel, limits).await?;
    let value = serde_json::from_slice::<Value>(&stdout);
    if !status.success() {
        if let Ok(Value::Object(ref object)) = value {
            if let Some(error) = object.get("error").and_then(Value::as_str) {
                bail!("{error}");
            }
        }
        bail!(
            "Speech helper failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        );
    }
    let value = value.context("Speech helper returned invalid JSON")?;
    ensure!(
        value.is_object(),
        "Speech helper returned an invalid response"
    );
    Ok(value)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    async fn shell(script: &str, cancel: &Cancellation, limits: Limits) -> Result<Value> {
        run_json(
            Path::new("/bin/sh"),
            &[OsString::from("-c"), OsString::from(script)],
            cancel,
            limits,
        )
        .await
    }
    fn limits() -> Limits {
        Limits {
            timeout: Duration::from_secs(2),
            stdout: 256,
            stderr: 256,
        }
    }
    #[tokio::test]
    async fn parses_json_and_preserves_structured_failure() {
        assert_eq!(
            shell(
                "printf '{\"words\":[]}'",
                &Cancellation::default(),
                limits()
            )
            .await
            .unwrap()["words"],
            serde_json::json!([])
        );
        let error = shell(
            "printf '{\"error\":\"Model unavailable\"}'; exit 1",
            &Cancellation::default(),
            limits(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "Model unavailable");
        assert!(
            shell("printf '[]'", &Cancellation::default(), limits())
                .await
                .is_err()
        );
        assert!(
            shell("printf 'broken'", &Cancellation::default(), limits())
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn bounds_both_pipes_without_deadlocking() {
        for script in [
            "while :; do printf '0123456789'; done",
            "while :; do printf '0123456789' >&2; done",
        ] {
            assert!(
                shell(script, &Cancellation::default(), limits())
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("memory limit")
            );
        }
    }
    #[tokio::test]
    async fn cancels_running_and_pre_cancelled_jobs_and_times_out() {
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(
            shell("exit 0", &cancel, limits())
                .await
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        let cancel = Cancellation::default();
        let signal = cancel.clone();
        let trigger = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            signal.cancel();
        });
        assert!(
            shell("exec sleep 30", &cancel, limits())
                .await
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        trigger.await.unwrap();
        let mut short = limits();
        short.timeout = Duration::from_millis(30);
        assert!(
            shell("exec sleep 30", &Cancellation::default(), short)
                .await
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
    }
}
