use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("tmp")
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
        ".{}.tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("tmp")
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
