use std::path::PathBuf;

pub fn binary(name: &str) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let directories = std::env::split_paths(&path).chain(
            ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"].map(PathBuf::from),
        );
        return resolve(name, directories);
    }
    #[cfg(not(target_os = "macos"))]
    PathBuf::from(name)
}

#[cfg(target_os = "macos")]
fn resolve(name: &str, directories: impl IntoIterator<Item = PathBuf>) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    directories.into_iter().map(|directory| directory.join(name))
        .find(|candidate| candidate.metadata().is_ok_and(|metadata| {
            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
        }))
        .unwrap_or_else(|| PathBuf::from(name))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn searches_fallbacks_after_path_and_skips_nonexecutables() {
        let root = std::env::temp_dir().join(format!("media-tools-{}", uuid::Uuid::new_v4()));
        let path = root.join("path");
        let fallback = root.join("fallback");
        for directory in [&path, &fallback] {
            std::fs::create_dir_all(directory).unwrap();
            let file = directory.join("ffprobe");
            std::fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        std::fs::set_permissions(fallback.join("ffprobe"), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolve("ffprobe", [path.clone(), fallback.clone()]), fallback.join("ffprobe"));
        std::fs::set_permissions(path.join("ffprobe"), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolve("ffprobe", [path.clone(), fallback.clone()]), path.join("ffprobe"));
        assert_eq!(resolve("ffmpeg", [path, fallback]), PathBuf::from("ffmpeg"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
