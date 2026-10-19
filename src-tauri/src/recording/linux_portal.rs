use anyhow::{Context, Result};
use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
};
use std::path::{Path, PathBuf};

pub(crate) async fn portal_source_types() -> Result<u32> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let portal = Screencast::new()
            .await
            .context("ScreenCast portal is unavailable")?;
        let sources = portal
            .available_source_types()
            .await
            .context("Could not query ScreenCast source types")?
            .bits();
        anyhow::ensure!(
            sources & 3 != 0,
            "The desktop portal supports neither display nor window capture"
        );
        Ok(sources)
    })
    .await
    .context("ScreenCast portal did not respond")?
}

/// Choose a window and save its restore token when the portal grants persistence.
pub(crate) async fn prepare_window_source() -> Result<()> {
    let portal = Screencast::new()
        .await
        .context("Failed to connect to xdg-desktop-portal ScreenCast")?;
    let session = portal
        .create_session(Default::default())
        .await
        .context("Failed to create a desktop-portal screen-cast session")?;

    let result = async {
        portal
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(CursorMode::Embedded)
                    .set_sources(Some(SourceType::Window.into()))
                    .set_multiple(false)
                    .set_persist_mode(PersistMode::ExplicitlyRevoked),
            )
            .await
            .context("Failed to configure the desktop-portal window picker")?;

        let response = portal
            .start(&session, None, Default::default())
            .await
            .context("Failed to open the desktop-portal window picker")?
            .response()
            .context("Window selection was cancelled")?;
        if response.streams().is_empty() {
            anyhow::bail!("The desktop portal returned no window stream");
        }
        // Without persistence, Start will ask the user to select the window again.
        // Clear the previous choice so it cannot silently select a different window.
        save_restore_token("window", response.restore_token())?;
        Ok(())
    }
    .await;
    let _ = session.close().await;
    result
}

fn restore_token_path(source_key: &str) -> Option<PathBuf> {
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(
        config_root
            .join("tarantino")
            .join(format!("linux-screencast-{source_key}.token")),
    )
}

pub(super) fn load_restore_token(source_key: &str) -> Option<String> {
    let token = std::fs::read_to_string(restore_token_path(source_key)?).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

pub(super) fn save_restore_token(source_key: &str, token: Option<&str>) -> Result<()> {
    let path = restore_token_path(source_key)
        .context("Neither XDG_CONFIG_HOME nor HOME is available for portal persistence")?;
    replace_restore_token(&path, token)
}

fn replace_restore_token(path: &Path, token: Option<&str>) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(error).with_context(|| format!("Failed to replace {}", path.display()));
    }
    let Some(token) = token else { return Ok(()) };
    let parent = path.parent().context("Portal token path has no parent")?;
    std::fs::create_dir_all(parent)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(token.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    #[ignore = "run with DBUS_SESSION_BUS_ADDRESS pointing to an unavailable bus"]
    async fn missing_portal_disables_recording() {
        let capabilities = crate::recording_commands::get_recording_capabilities().await;
        let value = serde_json::to_value(capabilities).unwrap();
        assert_eq!(value["recordingAvailable"], false);
        assert!(
            value["unavailableReason"]
                .as_str()
                .unwrap()
                .contains("portal")
        );
        use crate::capture::backends::NativeCaptureBackend;
        let backend = crate::capture::backends::linux::PipeWireBackend::new().unwrap();
        assert!(backend.enumerate_sources().await.is_err());
    }

    #[test]
    fn nonpersistent_selection_clears_previous_window() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let path = root.join("window.token");
        replace_restore_token(&path, Some("previous-window")).unwrap();
        replace_restore_token(&path, None).unwrap();
        assert!(!path.exists());
        replace_restore_token(&path, None).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn kde_persistent_selection_replaces_single_use_token_privately() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let path = root.join("window.token");
        replace_restore_token(&path, Some("old-token")).unwrap();
        replace_restore_token(&path, Some("new-token")).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new-token");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
