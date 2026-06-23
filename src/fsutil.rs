use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp"),
        uuid::Uuid::new_v4()
    ));
    fs::write(&tmp, data).with_context(|| format!("failed to write {}", tmp.display()))?;

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

#[cfg(unix)]
pub fn atomic_write_with_mode(path: &Path, data: &[u8], mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp"),
        uuid::Uuid::new_v4()
    ));

    fs::write(&tmp, data).with_context(|| format!("failed to write {}", tmp.display()))?;

    let mut perms = fs::metadata(&tmp)?.permissions();
    perms.set_mode(mode);
    fs::set_permissions(&tmp, perms).context("failed to set permissions on temp file")?;

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

/// Best-effort removal of orphaned clin plaintext temp files from a previous
/// crashed session. Removes files in temp_dir matching prefix `clin_` whose
/// mtime is older than 24h. Ignores all errors. Safe under concurrent clin
/// instances (24h threshold never touches an active session's fresh file).
pub fn cleanup_orphaned_temp_files() {
    const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 3600);
    let now = std::time::SystemTime::now();
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else { continue };
        if !name.starts_with("clin_") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                if now.duration_since(mtime).map(|d| d < MAX_AGE).unwrap_or(true) {
                    continue; // too fresh — may belong to a running session
                }
            }
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
