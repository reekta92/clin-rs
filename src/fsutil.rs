use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Core atomic write: write data to a temp file in the same directory,
/// sync it (unix only), optionally set permissions (unix only), then
/// atomically rename over `path`.
fn atomic_write_impl(path: &Path, data: &[u8], mode: Option<u32>) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp"),
        uuid::Uuid::new_v4()
    ));
    fs::write(&tmp, data).with_context(|| format!("failed to write {}", tmp.display()))?;

    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(mode);
        fs::set_permissions(&tmp, perms).context("failed to set permissions on temp file")?;
    }

    #[cfg(unix)]
    {
        use std::fs::File;
        let f = File::open(&tmp).context("failed to open temp file for syncing")?;
        f.sync_all().context("failed to sync temp file")?;
    }

    fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to rename temp file {} to {}",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}

/// Write `data` to `path` atomically using a temp file + rename.
/// On unix the temp file is synced before rename; permissions are umask-default.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    atomic_write_impl(path, data, None)
}
/// Write the string `s` to `path` atomically via [`atomic_write`].
pub fn atomic_write_str(path: &Path, s: &str) -> Result<()> {
    atomic_write(path, s.as_bytes())
}

/// Like [`atomic_write`] but sets file permissions to `mode` before syncing.
#[cfg(unix)]
pub fn atomic_write_with_mode(path: &Path, data: &[u8], mode: u32) -> Result<()> {
    atomic_write_impl(path, data, Some(mode))
}

/// Best-effort removal of orphaned clin plaintext temp files from a previous
/// crashed session. Removes files in temp_dir matching prefix `clin_` whose
/// mtime is older than 24h. Ignores all errors. Safe under concurrent clin
/// instances (24h threshold never touches an active session's fresh file).
pub fn cleanup_orphaned_temp_files() {
    const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 3600);
    let now = std::time::SystemTime::now();
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !name.starts_with("clin_") {
            continue;
        }
        if let Ok(meta) = entry.metadata()
            && let Ok(mtime) = meta.modified()
                && now
                    .duration_since(mtime)
                    .map(|d| d < MAX_AGE)
                    .unwrap_or(true)
                {
                    continue; // too fresh — may belong to a running session
                }
        // matches both `clin_{uuid}.md` and `clin_md_<rand>` (tempfile prefix)
        let _ = std::fs::remove_file(entry.path());
    }
}

/// RAII guard: zero-fills then removes a file containing secret plaintext on drop.
pub struct SecretTempFile(PathBuf);

impl SecretTempFile {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

impl Drop for SecretTempFile {
    fn drop(&mut self) {
        if let Ok(len) = std::fs::metadata(&self.0).map(|m| m.len()) {
            let _ = std::fs::write(&self.0, vec![0u8; len as usize]);
        }
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_with_mode_sets_perms_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("clin_fsutil_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secret.txt");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            atomic_write_with_mode(&path, b"hello", 0o600).unwrap();
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        }

        #[cfg(not(unix))]
        {
            atomic_write(&path, b"hello").unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
