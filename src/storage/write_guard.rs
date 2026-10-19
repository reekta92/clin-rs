//! Serialize cooperating clin writers; raw-source baselines detect changes by other tools.
use super::Storage;
use anyhow::{Context, Result, bail};
use std::fs;

impl Storage {
    pub(crate) fn write_guard(&self) -> Result<fs::File> {
        let file = self.coordination_file("write.lock")?;
        file.try_lock()
            .context("another clin write is in progress; retry saving")?;
        Ok(file)
    }

    pub(crate) fn recovery_guard(&self, exclusive: bool) -> Result<fs::File> {
        let file = self.coordination_file("recovery.lock")?;
        if exclusive {
            file.try_lock()?;
        } else {
            file.try_lock_shared()?;
        }
        Ok(file)
    }

    fn coordination_file(&self, name: &str) -> Result<fs::File> {
        let root = self
            .data_dir
            .canonicalize()
            .context("failed to resolve vault")?;
        let directory = root.join(".clin");
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                bail!("write coordination directory must be a real private directory");
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&directory)?;
            }
            Err(error) => return Err(error.into()),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        let path = directory.join(name);
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            bail!("write lock must be a regular non-symlink file");
        }
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).context("failed to open write lock")?;
        Ok(file)
    }
}
