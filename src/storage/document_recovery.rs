//! Private plaintext recovery for guarded documents. Legacy encrypted records stay untouched.

use super::{Storage, documents::DocumentSnapshot};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 40 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
enum Record<T> {
    Pending(Vec<T>),
    Committed(Vec<T>),
    Draft(T),
    NewDraft((String, String)),
    Moves {
        entries: Vec<super::public_moves::FileMove>,
        committed: bool,
    },
}
type Entry = (String, String, String);
pub(crate) struct DocumentTransaction(PathBuf);

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
impl Storage {
    fn document_recovery_dir(&self, create: bool) -> Result<PathBuf> {
        let root = self.data_dir.canonicalize()?;
        let mut directory = root;
        for part in [".clin", "document-recovery-v2"] {
            directory.push(part);
            match fs::symlink_metadata(&directory) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() || !metadata.is_dir() {
                        bail!("document recovery directory is not a private real directory");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !create {
                        return Ok(directory);
                    }
                    fs::create_dir(&directory)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
                    }
                    sync_directory(
                        directory
                            .parent()
                            .context("recovery directory has no parent")?,
                    )?;
                }
                Err(error) => return Err(error.into()),
            }
            #[cfg(unix)]
            if create {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
            }
        }
        Ok(directory)
    }
    fn write_document_record<T: Serialize>(&self, path: &Path, record: &Record<T>) -> Result<()> {
        let bytes = bincode::serde::encode_to_vec((2u8, record), bincode::config::standard())?;
        if bytes.len() > MAX_BYTES {
            bail!("document recovery record exceeds safety limit");
        }
        crate::fsutil::atomic_write_with_mode(path, &bytes, 0o600)?;
        sync_directory(path.parent().context("recovery record has no directory")?)
    }
    fn remove_document_record(&self, path: &Path) -> Result<()> {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        sync_directory(path.parent().context("recovery record has no directory")?)
    }
    pub(crate) fn begin_document_transaction(
        &self,
        plans: &[(DocumentSnapshot, String)],
    ) -> Result<DocumentTransaction> {
        let entries = plans
            .iter()
            .map(|(snapshot, after)| {
                self.document_path(snapshot.id())?;
                Ok((snapshot.id(), snapshot.source(), after.as_str()))
            })
            .collect::<Result<Vec<_>>>()?;
        let path = self
            .document_recovery_dir(true)?
            .join(format!("{}.bin", uuid::Uuid::new_v4()));
        self.write_document_record(&path, &Record::Pending(entries))?;
        Ok(DocumentTransaction(path))
    }
    pub(crate) fn begin_public_move_transaction(
        &self,
        entries: &[super::public_moves::FileMove],
    ) -> Result<DocumentTransaction> {
        let path = self
            .document_recovery_dir(true)?
            .join(format!("{}.bin", uuid::Uuid::new_v4()));
        self.write_document_record(
            &path,
            &Record::<Entry>::Moves {
                entries: entries.to_vec(),
                committed: false,
            },
        )?;
        Ok(DocumentTransaction(path))
    }
    pub(crate) fn commit_public_move_transaction(
        &self,
        journal: &DocumentTransaction,
        entries: &[super::public_moves::FileMove],
    ) -> Result<()> {
        self.write_document_record(
            &journal.0,
            &Record::<Entry>::Moves {
                entries: entries.to_vec(),
                committed: true,
            },
        )
    }
    pub(crate) fn finish_document_transaction(
        &self,
        journal: &DocumentTransaction,
        entries: Vec<(&str, &str, &str)>,
    ) -> Result<()> {
        // Retain prior bytes in the durable marker too, including failed cleanup/rollback paths.
        self.write_document_record(&journal.0, &Record::Committed(entries))?;
        self.remove_document_record(&journal.0)
    }
    pub(crate) fn abort_document_transaction(&self, journal: DocumentTransaction) -> Result<()> {
        self.remove_document_record(&journal.0)
    }
    pub fn write_document_draft(&self, baseline: &DocumentSnapshot, source: &str) -> Result<()> {
        self.document_path(baseline.id())?;
        let path = self
            .document_recovery_dir(true)?
            .join(format!("draft-{}.bin", baseline.recovery_id()));
        if baseline.exists() {
            self.write_document_record(
                &path,
                &Record::Draft((baseline.id(), baseline.source(), source)),
            )
        } else {
            self.write_document_record::<Entry>(
                &path,
                &Record::NewDraft((baseline.id().to_owned(), source.to_owned())),
            )
        }
    }
    pub fn clear_document_draft(&self, baseline: &DocumentSnapshot) -> Result<()> {
        let directory = self.document_recovery_dir(false)?;
        if directory.exists() {
            self.remove_document_record(
                &directory.join(format!("draft-{}.bin", baseline.recovery_id())),
            )?;
        }
        Ok(())
    }
    /// Startup storage recovery, before catalogs/queries. Conflicts and corrupt records remain for inspection.
    pub fn recover_documents(&self) -> Result<Vec<String>> {
        let directory = self.document_recovery_dir(false)?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut warnings = Vec::new();
        let mut paths = fs::read_dir(&directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();
        for path in paths {
            let result = (|| {
                let metadata = fs::symlink_metadata(&path)?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    bail!("recovery record is not a real file");
                }
                let mut bytes = Vec::new();
                fs::File::open(&path)?
                    .take((MAX_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > MAX_BYTES || bytes.is_empty() {
                    bail!("invalid recovery record size");
                }
                let ((version, record), used) =
                    bincode::serde::decode_from_slice::<(u8, Record<Entry>), _>(
                        &bytes,
                        bincode::config::standard().with_limit::<MAX_BYTES>(),
                    )?;
                if version != 2 || used != bytes.len() {
                    bail!("unsupported document recovery record");
                }
                match record {
                    Record::Moves { entries, committed } => {
                        let _guard = self.write_guard()?;
                        self.recover_public_moves(&entries, committed)?;
                    }
                    Record::Committed(entries) => {
                        let mut rollback = false;
                        for (id, before, after) in &entries {
                            let current = self.load_document(id)?;
                            if before != after && current.source() == before {
                                rollback = true;
                            } else if current.source() != after {
                                bail!(
                                    "{id}: committed recovery record conflicts with external changes; record retained"
                                );
                            }
                        }
                        if rollback {
                            for (id, before, after) in entries {
                                let current = self.load_document(&id)?;
                                if current.source() == after {
                                    self.save_document(&current, &before)?;
                                }
                            }
                        }
                    }
                    Record::Pending(entries) => {
                        for (id, before, after) in entries {
                            let current = self.load_document(&id)?;
                            if current.source() == after {
                                self.save_document(&current, &before)?;
                            } else if current.source() != before {
                                bail!(
                                    "{id}: recovery conflicts with external changes; original record retained"
                                );
                            }
                        }
                    }
                    Record::NewDraft((id, after)) => {
                        let path = self.document_path(&id)?;
                        if !path.exists() {
                            self.create_document(&id, &after)?;
                        } else if self.load_document(&id)?.source() != after {
                            bail!(
                                "{id}: new-note recovery conflicts with existing file; draft retained"
                            );
                        }
                    }
                    Record::Draft((id, before, after)) => {
                        let current = self.load_document(&id)?;
                        if current.source() == before {
                            self.save_document(&current, &after)?;
                        } else if current.source() != after {
                            bail!("{id}: draft conflicts with external changes; draft retained");
                        }
                    }
                }
                self.remove_document_record(&path)
            })();
            if let Err(error) = result {
                warnings.push(format!(
                    "Document recovery retained {}: {error:#}",
                    path.display()
                ));
            }
        }
        Ok(warnings)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn vault() -> (tempfile::TempDir, Storage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage {
            data_dir: dir.path().into(),
            config_dir: dir.path().into(),
            notes_dir: dir.path().into(),
            templates_dir: dir.path().join("templates"),
            skip_dir_patterns: Vec::new(),
        };
        (dir, storage)
    }
    #[test]
    fn interrupted_bulk_recovers_exact_public_bytes_from_private_record() {
        let (dir, storage) = vault();
        let a = storage.create_document("a.md", "original a").unwrap();
        let b = storage.create_document("b.md", "original b").unwrap();
        let journal = storage
            .begin_document_transaction(&[
                (a.clone(), "edited a".into()),
                (b.clone(), "edited b".into()),
            ])
            .unwrap();
        let bytes = fs::read(&journal.0).unwrap();
        assert!(bytes.windows(10).any(|window| window == b"original a"));
        storage.save_document(&a, "edited a").unwrap();
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
        assert_eq!(
            storage.load_document("a.md").unwrap().source(),
            "original a"
        );
        assert_eq!(
            storage.load_document("b.md").unwrap().source(),
            "original b"
        );
        assert!(!journal.0.exists());
        assert!(dir.path().join(".clin/document-recovery-v2").exists());
    }
    #[test]
    fn committed_records_never_rollback_and_draft_conflicts_are_retained() {
        let (_dir, storage) = vault();
        let a = storage.create_document("a.md", "original").unwrap();
        let journal = storage
            .begin_document_transaction(&[(a.clone(), "edited".into())])
            .unwrap();
        let after = storage.save_document(&a, "edited").unwrap();
        storage
            .write_document_record(
                &journal.0,
                &Record::Committed(vec![("a.md", "original", "edited")]),
            )
            .unwrap();
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
        assert_eq!(storage.load_document("a.md").unwrap().source(), "edited");
        storage.write_document_draft(&after, "draft").unwrap();
        storage.save_document(&after, "external").unwrap();
        assert_eq!(storage.recover_documents().unwrap().len(), 1);
        assert_eq!(storage.load_document("a.md").unwrap().source(), "external");
        assert!(
            storage
                .document_recovery_dir(false)
                .unwrap()
                .join(format!("draft-{}.bin", after.recovery_id()))
                .exists()
        );
    }
    #[test]
    fn draft_sessions_never_overwrite_each_other_and_cleanup_is_scoped() {
        let (_dir, storage) = vault();
        let a = storage.create_document("a.md", "original a").unwrap();
        let b = storage.create_document("b.md", "original b").unwrap();
        storage.write_document_draft(&a, "draft a").unwrap();
        storage.write_document_draft(&b, "draft b").unwrap();
        let saved_b = storage.save_document(&b, "saved b").unwrap();
        storage.clear_document_draft(&saved_b).unwrap();
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
        assert_eq!(storage.load_document("a.md").unwrap().source(), "draft a");
        assert_eq!(storage.load_document("b.md").unwrap().source(), "saved b");
    }
    #[test]
    fn draft_recovery_never_injects_clin_fields_or_reads_encrypted_note_paths() {
        let (_dir, storage) = vault();
        let source = storage.create_document("note.md", "Body").unwrap();
        storage
            .write_document_draft(&source, "---\nscore: 2\n---\nBody\n")
            .unwrap();
        assert_eq!(storage.recover_documents().unwrap(), Vec::<String>::new());
        assert!(
            !storage
                .load_document("note.md")
                .unwrap()
                .source()
                .contains("updated_at")
        );
        let path = storage
            .document_recovery_dir(true)
            .unwrap()
            .join("forged.bin");
        storage
            .write_document_record(
                &path,
                &Record::Draft(("secret.clin", "ciphertext", "changed")),
            )
            .unwrap();
        assert_eq!(storage.recover_documents().unwrap().len(), 1);
        assert!(path.exists());
    }
}
