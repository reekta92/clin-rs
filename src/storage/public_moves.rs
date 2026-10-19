//! Guarded public file moves. Staged links keep rollback data until durable commit.
use super::Storage;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMove {
    from: String,
    to: String,
    digest: [u8; 32],
    len: u64,
    modified: Option<SystemTime>,
    mode: Option<u32>,
    stage: [u8; 16],
}
impl FileMove {
    pub fn source_id(&self) -> &str {
        &self.from
    }
    pub fn destination_id(&self) -> &str {
        &self.to
    }
    pub fn history_bytes(&self) -> usize {
        self.from.len() + self.to.len() + 128
    }
    #[must_use]
    pub fn reversed(&self) -> Self {
        Self {
            from: self.to.clone(),
            to: self.from.clone(),
            stage: uuid::Uuid::new_v4().into_bytes(),
            ..self.clone()
        }
    }
}
fn check_cancel(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        bail!("public file move cancelled");
    }
    Ok(())
}
fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path.parent().context("file has no parent")?)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn digest(path: &Path, cancelled: &AtomicBool) -> Result<([u8; 32], u64)> {
    let mut file = fs::File::open(path)?;
    let before = file.metadata()?;
    if !before.is_file() || fs::symlink_metadata(path)?.file_type().is_symlink() {
        bail!("public move source must be a regular non-symlink file");
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 16 * 1024];
    let mut len = 0;
    loop {
        check_cancel(cancelled)?;
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        len += read as u64;
    }
    let after = file.metadata()?;
    if before.len() != len || after.len() != len || before.modified().ok() != after.modified().ok()
    {
        bail!("public file changed during snapshot; original retained");
    }
    Ok((hash.finalize().into(), len))
}
fn copy_new(
    source: &Path,
    target: &Path,
    modified: Option<SystemTime>,
    cancelled: &AtomicBool,
) -> Result<()> {
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    output.set_permissions(input.metadata()?.permissions())?;
    let mut buffer = [0; 16 * 1024];
    loop {
        check_cancel(cancelled)?;
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
    }
    if let Some(modified) = modified {
        output.set_times(fs::FileTimes::new().set_modified(modified))?;
    }
    output.sync_all()?;
    sync_parent(target)
}

impl Storage {
    fn move_path(&self, id: &str) -> Result<PathBuf> {
        let path = self.document_path(id)?;
        let mut raw = self.notes_dir.join(id);
        while raw != self.notes_dir {
            match fs::symlink_metadata(&raw) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    bail!("public moves cannot traverse symlinks")
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            raw = raw
                .parent()
                .context("invalid public move path")?
                .to_path_buf();
        }
        Ok(path)
    }
    fn move_stage(&self, plan: &FileMove) -> Result<PathBuf> {
        let target = self.move_path(&plan.to)?;
        Ok(target
            .parent()
            .context("move destination has no parent")?
            .join(format!(
                ".clin-move-{}.tmp",
                uuid::Uuid::from_bytes(plan.stage)
            )))
    }
    fn verify_move_file(&self, plan: &FileMove, id: &str, cancelled: &AtomicBool) -> Result<()> {
        let path = self.move_path(id)?;
        let metadata = fs::metadata(&path)?;
        if metadata.permissions().readonly() {
            bail!("public move source is read-only");
        }
        if metadata.modified().ok() != plan.modified {
            bail!("public file timestamp changed; reload before moving");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if plan.mode != Some(metadata.permissions().mode()) {
                bail!("public file permissions changed; reload before moving");
            }
        }
        if digest(&path, cancelled)? != (plan.digest, plan.len) {
            bail!("public file changed on disk; reload before moving");
        }
        Ok(())
    }
    pub fn prepare_public_file_moves(
        &self,
        ids: &[String],
        folder: &str,
        cancelled: &AtomicBool,
    ) -> Result<Vec<FileMove>> {
        let mut normalized = PathBuf::new();
        for component in Path::new(folder).components() {
            match component {
                std::path::Component::Normal(name) => normalized.push(name),
                std::path::Component::CurDir => {}
                _ => bail!("public folder must stay inside vault"),
            }
        }
        let paths = ids
            .iter()
            .map(|id| {
                let name = Path::new(id)
                    .file_name()
                    .context("public move source has no filename")?;
                let target = normalized
                    .join(name)
                    .to_str()
                    .context("invalid public folder")?
                    .replace('\\', "/");
                Ok((id.clone(), target))
            })
            .collect::<Result<Vec<_>>>()?;
        self.prepare_public_file_paths(&paths, cancelled)
    }
    pub(crate) fn prepare_public_file_paths(
        &self,
        paths: &[(String, String)],
        cancelled: &AtomicBool,
    ) -> Result<Vec<FileMove>> {
        let mut plans = Vec::new();
        for (id, to) in paths {
            check_cancel(cancelled)?;
            let source = self.move_path(id)?;
            let metadata = fs::metadata(&source)?;
            if metadata.permissions().readonly() {
                bail!("public move source is read-only");
            }
            self.move_path(to)?;
            if id == to {
                continue;
            }
            let (digest, len) = digest(&source, cancelled)?;
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                Some(metadata.permissions().mode())
            };
            #[cfg(not(unix))]
            let mode = None;
            plans.push(FileMove {
                from: id.clone(),
                to: to.clone(),
                digest,
                len,
                mode,
                modified: metadata.modified().ok(),
                stage: uuid::Uuid::new_v4().into_bytes(),
            });
        }
        self.preflight_public_moves(&plans, cancelled)?;
        Ok(plans)
    }
    fn preflight_public_moves(&self, plans: &[FileMove], cancelled: &AtomicBool) -> Result<()> {
        let mut sources = std::collections::HashSet::new();
        let mut targets = std::collections::HashSet::new();
        for plan in plans {
            if !sources.insert(&plan.from) || !targets.insert(&plan.to) || plan.from == plan.to {
                bail!("duplicate or overlapping public move targets; originals retained");
            }
            self.verify_move_file(plan, &plan.from, cancelled)?;
            if self.move_path(&plan.to)?.try_exists()? {
                bail!("public move destination already exists; originals retained");
            }
            if self.move_stage(plan)?.try_exists()? {
                bail!("public move staging path already exists; originals retained");
            }
        }
        if sources.iter().any(|source| targets.contains(source)) {
            bail!("overlapping public move paths; originals retained");
        }
        Ok(())
    }
    pub fn apply_public_file_moves(
        &self,
        plans: &[FileMove],
        cancelled: &AtomicBool,
    ) -> Result<()> {
        if plans.is_empty() {
            return Ok(());
        }
        let _guard = self.write_guard()?;
        self.preflight_public_moves(plans, cancelled)?;
        let plans: Vec<_> = plans
            .iter()
            .cloned()
            .map(|mut plan| {
                plan.stage = uuid::Uuid::new_v4().into_bytes();
                plan
            })
            .collect();
        let journal = self.begin_public_move_transaction(&plans)?;
        let result = (|| {
            for plan in &plans {
                check_cancel(cancelled)?;
                self.verify_move_file(plan, &plan.from, cancelled)?;
                let source = self.move_path(&plan.from)?;
                let target = self.move_path(&plan.to)?;
                let stage = self.move_stage(plan)?;
                fs::create_dir_all(target.parent().context("move destination has no parent")?)?;
                match fs::hard_link(&source, &stage) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
                        copy_new(&source, &stage, plan.modified, cancelled)?
                    }
                    Err(error) => return Err(error).context("failed to stage public move"),
                }
                fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&stage)?
                    .sync_all()?;
                sync_parent(&stage)?;
                if digest(&stage, cancelled)? != (plan.digest, plan.len) {
                    bail!("public file changed before publication; original retained");
                }
                fs::hard_link(&stage, &target)
                    .context("failed to publish public move without replacement")?;
                sync_parent(&target)?;
                self.verify_move_file(plan, &plan.from, cancelled)?;
                // Staged bytes remain durable through unlink, rollback, and commit-marker failures.
                crate::fsutil::remove_file_if_exists(&source)?;
                sync_parent(&source)?;
            }
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(error) = result {
            match self.recover_public_moves(&plans, false) {
                Ok(()) => {
                    self.abort_document_transaction(journal)?;
                    return Err(error);
                }
                Err(rollback) => bail!("{error:#}; public move recovery retained: {rollback:#}"),
            }
        }
        self.commit_public_move_transaction(&journal, &plans)
            .context("public moves published; commit state uncertain, staged recovery retained")?;
        // Commit marker precedes stage cleanup. Cleanup failures remain recoverable at startup.
        self.recover_public_moves(&plans, true)?;
        self.abort_document_transaction(journal)
    }
    pub(crate) fn recover_public_moves(&self, plans: &[FileMove], committed: bool) -> Result<()> {
        let cancelled = AtomicBool::new(false);
        for plan in plans.iter().rev() {
            let source = self.move_path(&plan.from)?;
            let target = self.move_path(&plan.to)?;
            let stage = self.move_stage(plan)?;
            if !stage.try_exists()? {
                if committed {
                    if !target.try_exists()? {
                        bail!("committed public move destination missing; record retained");
                    }
                } else {
                    self.verify_move_file(plan, &plan.from, &cancelled)?;
                    if target.try_exists()? {
                        bail!("public move destination ownership uncertain; record retained");
                    }
                }
                continue;
            }
            if fs::symlink_metadata(&stage)?.file_type().is_symlink() {
                bail!("public move staging path is a symlink; record retained");
            }
            if digest(&stage, &cancelled)? != (plan.digest, plan.len) {
                if !committed
                    && !target.try_exists()?
                    && self.verify_move_file(plan, &plan.from, &cancelled).is_ok()
                {
                    // Interrupted streaming copy never published: source remains authoritative.
                    crate::fsutil::remove_file_if_exists(&stage)?;
                    sync_parent(&stage)?;
                    continue;
                }
                bail!("public move staging data changed; record retained");
            }
            if committed {
                if !target.try_exists()? {
                    bail!("committed public move destination missing; stage retained");
                }
                crate::fsutil::remove_file_if_exists(&stage)?;
                sync_parent(&stage)?;
                continue;
            }
            if target.try_exists()? && !crate::fsutil::is_same_file(&stage, &target) {
                bail!("public move destination changed; original recovery retained");
            }
            if source.try_exists()? {
                self.verify_move_file(plan, &plan.from, &cancelled)?;
            } else {
                fs::create_dir_all(source.parent().context("move source has no parent")?)?;
                match fs::hard_link(&stage, &source) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
                        let temporary = source
                            .parent()
                            .context("move source has no parent")?
                            .join(format!(".clin-restore-{}.tmp", uuid::Uuid::new_v4()));
                        let guard = crate::fsutil::TempFileGuard::new(temporary.clone());
                        copy_new(&stage, &temporary, plan.modified, &cancelled)?;
                        fs::hard_link(&temporary, &source)?;
                        drop(guard);
                    }
                    Err(error) => return Err(error.into()),
                }
                sync_parent(&source)?;
                self.verify_move_file(plan, &plan.from, &cancelled)?;
            }
            if target.try_exists()? {
                crate::fsutil::remove_file_if_exists(&target)?;
                sync_parent(&target)?;
            }
            crate::fsutil::remove_file_if_exists(&stage)?;
            sync_parent(&stage)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vault() -> (tempfile::TempDir, Storage) {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage {
            data_dir: root.path().into(),
            notes_dir: root.path().into(),
            config_dir: root.path().into(),
            templates_dir: root.path().join("templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        (root, storage)
    }
    #[test]
    fn binary_moves_preserve_bytes_metadata_and_guarded_replay() {
        let (root, storage) = vault();
        let bytes = [0, 255, 128, 0, 42];
        fs::write(root.path().join("image.png"), bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                root.path().join("image.png"),
                fs::Permissions::from_mode(0o640),
            )
            .unwrap();
        }
        let original = fs::metadata(root.path().join("image.png")).unwrap();
        let cancelled = AtomicBool::new(false);
        let plans = storage
            .prepare_public_file_moves(&["image.png".into()], "folder", &cancelled)
            .unwrap();
        storage.apply_public_file_moves(&plans, &cancelled).unwrap();
        assert!(!root.path().join("image.png").exists());
        assert_eq!(
            fs::read(root.path().join("folder/image.png")).unwrap(),
            bytes
        );
        assert_eq!(
            fs::metadata(root.path().join("folder/image.png"))
                .unwrap()
                .modified()
                .unwrap(),
            original.modified().unwrap()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(root.path().join("folder/image.png"))
                    .unwrap()
                    .permissions()
                    .mode(),
                original.permissions().mode()
            );
        }
        storage
            .apply_public_file_moves(&[plans[0].reversed()], &cancelled)
            .unwrap();
        assert_eq!(fs::read(root.path().join("image.png")).unwrap(), bytes);
        storage.apply_public_file_moves(&plans, &cancelled).unwrap();
        fs::write(root.path().join("folder/image.png"), b"external").unwrap();
        assert!(
            storage
                .apply_public_file_moves(&[plans[0].reversed()], &cancelled)
                .is_err()
        );
        assert_eq!(
            fs::read(root.path().join("folder/image.png")).unwrap(),
            b"external"
        );
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
    }
    #[test]
    fn preflight_rejects_stale_colliding_private_and_cancelled_batches() {
        let (root, storage) = vault();
        fs::write(root.path().join("a.md"), b"a").unwrap();
        fs::write(root.path().join("b.md"), b"b").unwrap();
        fs::write(root.path().join("private.clin"), b"invalid ciphertext").unwrap();
        let cancelled = AtomicBool::new(false);
        let plans = storage
            .prepare_public_file_moves(&["a.md".into(), "b.md".into()], "folder", &cancelled)
            .unwrap();
        fs::write(root.path().join("b.md"), b"external").unwrap();
        assert!(storage.apply_public_file_moves(&plans, &cancelled).is_err());
        assert_eq!(fs::read(root.path().join("a.md")).unwrap(), b"a");
        assert!(!root.path().join("folder/a.md").exists());
        for (id, folder) in [
            ("private.clin", "folder"),
            ("a.md", ".hidden"),
            ("a.md", "../escape"),
        ] {
            assert!(
                storage
                    .prepare_public_file_moves(&[id.into()], folder, &cancelled)
                    .is_err()
            );
        }
        fs::create_dir(root.path().join("folder")).unwrap();
        fs::write(root.path().join("folder/a.md"), b"collision").unwrap();
        assert!(
            storage
                .prepare_public_file_moves(&["a.md".into()], "folder", &cancelled)
                .is_err()
        );
        assert_eq!(
            fs::read(root.path().join("folder/a.md")).unwrap(),
            b"collision"
        );
        assert!(
            storage
                .prepare_public_file_moves(&["a.md".into()], "other", &AtomicBool::new(true))
                .is_err()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.path().join("a.md"), root.path().join("alias.md"))
                .unwrap();
            assert!(
                storage
                    .prepare_public_file_moves(&["alias.md".into()], "other", &cancelled)
                    .is_err()
            );
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cross_device_moves_stream_large_binary_files_and_recover() {
        if !Path::new("/dev/shm").is_dir() {
            return;
        }
        let source = tempfile::Builder::new()
            .prefix("clin-public-move-")
            .tempdir_in("/tmp")
            .unwrap();
        let target = tempfile::Builder::new()
            .prefix("clin-public-move-")
            .tempdir_in("/dev/shm")
            .unwrap();
        let file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(source.path().join("large.png"))
            .unwrap();
        file.set_len(20 * 1024 * 1024).unwrap();
        drop(file);
        let storage = Storage {
            data_dir: source.path().into(),
            notes_dir: PathBuf::from("/"),
            config_dir: source.path().join(".clin"),
            templates_dir: source.path().join("templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        let id = source
            .path()
            .join("large.png")
            .strip_prefix("/")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let folder = target.path().strip_prefix("/").unwrap().to_str().unwrap();
        let cancel = AtomicBool::new(false);
        let plans = storage
            .prepare_public_file_moves(&[id], folder, &cancel)
            .unwrap();
        storage.apply_public_file_moves(&plans, &cancel).unwrap();
        assert_eq!(
            fs::metadata(target.path().join("large.png")).unwrap().len(),
            20 * 1024 * 1024
        );
        assert!(!source.path().join("large.png").exists());
        storage
            .apply_public_file_moves(&[plans[0].reversed()], &cancel)
            .unwrap();
        assert_eq!(
            fs::metadata(source.path().join("large.png")).unwrap().len(),
            20 * 1024 * 1024
        );
        assert!(!target.path().join("large.png").exists());
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
    }
    fn staged(storage: &Storage, root: &Path) -> Vec<FileMove> {
        fs::write(root.join("a.png"), [0, 255, 128]).unwrap();
        let plans = storage
            .prepare_public_file_moves(&["a.png".into()], "folder", &AtomicBool::new(false))
            .unwrap();
        fs::create_dir(root.join("folder")).unwrap();
        let source = storage.move_path("a.png").unwrap();
        let stage = storage.move_stage(&plans[0]).unwrap();
        fs::hard_link(source, &stage).unwrap();
        plans
    }
    #[test]
    fn pending_move_recovery_restores_unlinked_source_from_private_journal() {
        let (root, storage) = vault();
        let plans = staged(&storage, root.path());
        let _journal = storage.begin_public_move_transaction(&plans).unwrap();
        let directory = root.path().join(".clin/document-recovery-v2");
        let record = fs::read(
            fs::read_dir(&directory)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(record.windows(5).any(|bytes| bytes == b"a.png"));
        fs::hard_link(
            storage.move_stage(&plans[0]).unwrap(),
            root.path().join("folder/a.png"),
        )
        .unwrap();
        crate::fsutil::remove_file_if_exists(&root.path().join("a.png")).unwrap();
        drop(_journal);
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
        assert_eq!(fs::read(root.path().join("a.png")).unwrap(), [0, 255, 128]);
        assert!(!root.path().join("folder/a.png").exists());
        assert!(!storage.move_stage(&plans[0]).unwrap().exists());
    }
    #[test]
    fn committed_move_recovery_keeps_destination_and_conflicts_keep_originals() {
        let (root, storage) = vault();
        let plans = staged(&storage, root.path());
        let journal = storage.begin_public_move_transaction(&plans).unwrap();
        fs::hard_link(
            storage.move_stage(&plans[0]).unwrap(),
            root.path().join("folder/a.png"),
        )
        .unwrap();
        crate::fsutil::remove_file_if_exists(&root.path().join("a.png")).unwrap();
        storage
            .commit_public_move_transaction(&journal, &plans)
            .unwrap();
        drop(journal);
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
        assert_eq!(
            fs::read(root.path().join("folder/a.png")).unwrap(),
            [0, 255, 128]
        );
        assert!(!root.path().join("a.png").exists());
        let (root, storage) = vault();
        let plans = staged(&storage, root.path());
        storage.begin_public_move_transaction(&plans).unwrap();
        fs::write(root.path().join("folder/a.png"), b"competing").unwrap();
        assert_eq!(storage.recover_documents().unwrap().len(), 1);
        assert_eq!(
            fs::read(root.path().join("folder/a.png")).unwrap(),
            b"competing"
        );
        assert_eq!(fs::read(root.path().join("a.png")).unwrap(), [0, 255, 128]);
        assert!(storage.move_stage(&plans[0]).unwrap().exists());
    }
    #[test]
    fn interrupted_streaming_stage_is_cleaned_only_while_source_matches() {
        let (root, storage) = vault();
        fs::write(root.path().join("a.png"), [0, 255, 128]).unwrap();
        let plans = storage
            .prepare_public_file_moves(&["a.png".into()], "folder", &AtomicBool::new(false))
            .unwrap();
        storage.begin_public_move_transaction(&plans).unwrap();
        fs::create_dir(root.path().join("folder")).unwrap();
        fs::write(storage.move_stage(&plans[0]).unwrap(), [0]).unwrap();
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
        assert_eq!(fs::read(root.path().join("a.png")).unwrap(), [0, 255, 128]);
        assert!(!storage.move_stage(&plans[0]).unwrap().exists());
    }
}
