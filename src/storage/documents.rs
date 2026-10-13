//! Plaintext document I/O shared by Bases and property editing.

use super::Storage;
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub(crate) fn read_bounded_text(path: &Path, max_bytes: usize) -> Result<String> {
    let file = fs::File::open(path).context("failed to open document")?;
    let mut source = String::new();
    file.take(max_bytes as u64 + 1)
        .read_to_string(&mut source)
        .context("failed to read UTF-8 document")?;
    if source.len() > max_bytes {
        bail!("document exceeds {max_bytes}-byte safety limit");
    }
    Ok(source)
}

#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    id: String,
    source: String,
    recovery_id: uuid::Uuid,
}

impl DocumentSnapshot {
    pub(crate) fn recovery_id(&self) -> uuid::Uuid {
        self.recovery_id
    }
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

impl Storage {
    /// Resolve a public vault path without permitting symlink or hidden-data escapes.
    /// Missing suffixes are allowed for creation; existing ancestors are checked first.
    pub(crate) fn document_path(&self, id: &str) -> Result<PathBuf> {
        let path = self
            .validate_path_within_notes_dir(id)
            .filter(|p| p != &self.notes_dir)
            .context("document path must be a non-empty public vault-relative path")?;
        if Path::new(id)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("clin"))
        {
            bail!("encrypted notes are not available to Bases document storage");
        }
        let root = self
            .notes_dir
            .canonicalize()
            .context("failed to resolve vault")?;
        let mut ancestor = path.as_path();
        let mut suffix = Vec::new();
        loop {
            match fs::symlink_metadata(ancestor) {
                Ok(_) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    suffix.push(ancestor.file_name().context("invalid document path")?);
                    ancestor = ancestor.parent().context("invalid document ancestor")?;
                }
                Err(error) => return Err(error).context("failed to inspect document path"),
            }
        }
        let mut resolved = ancestor
            .canonicalize()
            .context("failed to resolve document path")?;
        let relative = resolved
            .strip_prefix(&root)
            .context("document path resolves outside vault")?;
        if relative.components().any(
            |c| matches!(c, Component::Normal(name) if name.to_string_lossy().starts_with('.')),
        ) || (resolved.is_file()
            && resolved
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("clin")))
        {
            bail!("document path resolves to private vault data");
        }
        for part in suffix.into_iter().rev() {
            resolved.push(part);
        }
        Ok(resolved)
    }

    pub(crate) fn load_public_attachment(&self, id: &str) -> Result<Vec<u8>> {
        let path = self.document_path(id)?;
        let file = fs::File::open(&path).context("failed to open public attachment")?;
        if !file.metadata()?.is_file() {
            bail!("public attachment must be a regular file");
        }
        let mut bytes = Vec::new();
        file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 8 * 1024 * 1024 {
            bail!("public attachment exceeds 8 MiB preview limit");
        }
        Ok(bytes)
    }

    pub fn load_document(&self, id: &str) -> Result<DocumentSnapshot> {
        let path = self.document_path(id)?;
        let source = read_bounded_text(&path, 16 * 1024 * 1024)?;
        Ok(DocumentSnapshot {
            id: id.to_owned(),
            source,
            recovery_id: uuid::Uuid::new_v4(),
        })
    }

    /// Validate the baseline before writing; no title synchronization or clin metadata.
    pub fn save_document(
        &self,
        baseline: &DocumentSnapshot,
        source: &str,
    ) -> Result<DocumentSnapshot> {
        if source.len() > 16 * 1024 * 1024 {
            bail!("document exceeds 16777216-byte safety limit");
        }
        let path = self.document_path(baseline.id())?;
        let current = read_bounded_text(&path, 16 * 1024 * 1024)?;
        if current != baseline.source() {
            bail!("document changed on disk; reload before saving");
        }
        if current != source {
            if fs::metadata(&path)?.permissions().readonly() {
                bail!("document is read-only");
            }
            crate::fsutil::atomic_write_preserving_permissions(&path, source.as_bytes())?;
        }
        Ok(DocumentSnapshot {
            id: baseline.id.clone(),
            source: source.to_owned(),
            recovery_id: baseline.recovery_id,
        })
    }

    pub fn save_properties(
        &self,
        baseline: &DocumentSnapshot,
        changes: &[(String, Option<serde_yaml_ng::Value>)],
    ) -> Result<DocumentSnapshot> {
        if !Path::new(baseline.id())
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            bail!("note properties can only be edited on Markdown files");
        }
        let types = self.load_property_types()?;
        for (key, value) in changes {
            if let Some(value) = value {
                types.validate(key, value)?;
            }
        }
        let updated = crate::frontmatter::properties::edit_properties(baseline.source(), changes)?;
        self.save_document(baseline, &updated)
    }

    /// Publish a new document without replacing an existing file, even on collision.
    pub fn create_document(&self, id: &str, source: &str) -> Result<DocumentSnapshot> {
        if source.len() > 16 * 1024 * 1024 {
            bail!("document exceeds 16777216-byte safety limit");
        }
        let path = self.document_path(id)?;
        let parent = path.parent().context("document has no parent directory")?;
        fs::create_dir_all(parent).context("failed to create document directory")?;
        crate::fsutil::atomic_create(&path, source.as_bytes())?;
        Ok(DocumentSnapshot {
            id: id.to_owned(),
            source: source.to_owned(),
            recovery_id: uuid::Uuid::new_v4(),
        })
    }

    pub fn rename_document(&self, id: &str, title: &str) -> Result<String> {
        let source = self.document_path(id)?;
        let ext = Path::new(id)
            .extension()
            .and_then(|e| e.to_str())
            .context("document has no extension")?;
        let stem = self.note_file_stem_from_title(title);
        let target_id = self.unique_note_id(&stem, ext, id);
        let target = self.document_path(&target_id)?;
        if source != target {
            if target.exists() {
                bail!("rename destination already exists");
            }
            fs::rename(source, target).context("failed to rename document")?;
        }
        Ok(target_id)
    }

    pub fn duplicate_document(&self, id: &str, folder: &str) -> Result<String> {
        let source = self.document_path(id)?;
        let path = Path::new(id);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .context("document has no extension")?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("document has no filename")?;
        let preferred = format!("{stem} (Copy)");
        let initial = Path::new(folder).join(format!("{}.{}", self.new_note_id(), ext));
        let initial = initial.to_str().context("invalid target folder")?;
        // Validate before collision probing, which uses the legacy note-path helper.
        self.document_path(initial)?;
        let target_id = self.unique_note_id(&preferred, ext, initial);
        let target = self.document_path(&target_id)?;
        fs::create_dir_all(target.parent().context("document has no parent")?)?;
        let bytes = fs::read(source).context("failed to read source document")?;
        crate::fsutil::atomic_create(&target, &bytes)?;
        Ok(target_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> (tempfile::TempDir, Storage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage {
            notes_dir: dir.path().to_owned(),
            data_dir: dir.path().to_owned(),
            config_dir: dir.path().to_owned(),
            templates_dir: dir.path().join("templates"),
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
            rename_on_title_change: true,
        };
        (dir, storage)
    }

    #[test]
    fn raw_document_lifecycle_preserves_bytes_and_extension() {
        let (dir, mut storage) = vault();
        let original = "# Keep comment\r\nviews:\r\n  - type: table\r\n    name: 'A'\r\n    pluginKey: {x: 1}\r\n";
        let snapshot = storage.create_document("data.base", original).unwrap();
        storage.save_document(&snapshot, original).unwrap();
        assert!(storage.create_document("data.base", "overwrite").is_err());
        let copy = storage.duplicate_note("data.base", "copies").unwrap();
        assert!(copy.ends_with(".base"));
        assert_eq!(fs::read_to_string(dir.path().join(copy)).unwrap(), original);
        let renamed = storage.rename_note("data.base", "Renamed").unwrap();
        assert_eq!(renamed, "Renamed.base");
        assert_eq!(
            fs::read_to_string(dir.path().join(renamed)).unwrap(),
            original
        );
        assert!(!dir.path().join("data.base").exists());
    }

    #[test]
    fn plaintext_files_in_clin_named_directories_are_allowed() {
        let (_dir, storage) = vault();
        let snapshot = storage
            .create_document("public.clin/new.base", "views: []\n")
            .unwrap();
        assert_eq!(
            storage.load_document(snapshot.id()).unwrap().source(),
            "views: []\n"
        );
    }

    #[test]
    fn stale_save_and_note_operations_cannot_overwrite_base() {
        let (dir, mut storage) = vault();
        let snapshot = storage.create_document("a.base", "views: []\n").unwrap();
        fs::write(dir.path().join("a.base"), "# external\nviews: []\n").unwrap();
        assert!(
            storage
                .save_document(&snapshot, "views: [{type: table}]\n")
                .is_err()
        );
        assert!(storage.load_note("a.base").is_err());
        let note = super::super::Note {
            title: "Wrong".into(),
            content: "bad".into(),
            tags: Vec::new(),
            updated_at: 1,
        };
        assert!(storage.save_note("a.base", &note).is_err());
        assert!(storage.toggle_pin("a.base").is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("a.base")).unwrap(),
            "# external\nviews: []\n"
        );
        assert!(!dir.path().join("Wrong.md").exists());
    }

    #[test]
    fn property_edits_do_not_rename_or_inject_clin_fields() {
        use serde_yaml_ng::Value;
        let (dir, storage) = vault();
        let original = "---\n# Keep\ntitle: 123\nother: 'exact'\n---\nBody\n";
        let snapshot = storage.create_document("original.md", original).unwrap();
        let updated = storage
            .save_properties(
                &snapshot,
                &[("title".into(), Some(Value::String("Different".into())))],
            )
            .unwrap();
        assert_eq!(updated.id(), "original.md");
        assert!(updated.source().contains("# Keep\n"));
        assert!(updated.source().contains("other: 'exact'\n---\nBody\n"));
        assert!(!updated.source().contains("updated_at:"));
        assert!(!dir.path().join("Different.md").exists());
        assert!(
            storage
                .save_properties(&snapshot, &[("title".into(), Some(Value::Null))])
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("original.md")).unwrap(),
            updated.source()
        );
        let base = storage.create_document("view.base", "views: []\n").unwrap();
        assert!(
            storage
                .save_properties(&base, &[("title".into(), Some(Value::Null))])
                .is_err()
        );
    }

    #[test]
    fn invalid_declared_type_is_rejected_before_property_write() {
        let (dir, storage) = vault();
        fs::create_dir(dir.path().join(".obsidian")).unwrap();
        fs::write(
            dir.path().join(".obsidian/types.json"),
            r#"{"types":{"done":"checkbox"}}"#,
        )
        .unwrap();
        let snapshot = storage
            .create_document("a.md", "---\ndone: false\n---\nBody")
            .unwrap();
        assert!(
            storage
                .save_properties(
                    &snapshot,
                    &[(
                        "done".into(),
                        Some(serde_yaml_ng::Value::String("true".into()))
                    )]
                )
                .is_err()
        );
        assert_eq!(
            storage.load_document("a.md").unwrap().source(),
            snapshot.source()
        );
        assert!(
            storage
                .save_properties(
                    &snapshot,
                    &[("done".into(), Some(serde_yaml_ng::Value::Bool(true)))]
                )
                .is_ok()
        );
    }

    #[test]
    fn text_reads_are_bounded_and_invalid_utf8_is_rejected() {
        let (dir, storage) = vault();
        fs::write(dir.path().join("large.md"), b"abcdef").unwrap();
        assert!(read_bounded_text(&dir.path().join("large.md"), 5).is_err());
        assert_eq!(
            read_bounded_text(&dir.path().join("large.md"), 6).unwrap(),
            "abcdef"
        );
        fs::write(dir.path().join("bad.md"), [0xff]).unwrap();
        assert!(storage.load_document("bad.md").is_err());
    }

    #[test]
    fn private_and_outside_paths_are_rejected() {
        let (_dir, storage) = vault();
        for id in [
            "",
            ".",
            "../outside.md",
            "/outside.md",
            ".clin/subnotes.bin",
            ".obsidian/types.json",
            "secret.clin",
            "secret.CLIN",
        ] {
            assert!(storage.document_path(id).is_err(), "{id}");
        }
        assert!(storage.create_document("../outside.base", "").is_err());
        assert!(
            storage
                .duplicate_document("anything.base", "../outside")
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_expose_private_or_external_data() {
        use std::os::unix::fs::symlink;
        let (dir, storage) = vault();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside.md"), "private").unwrap();
        fs::create_dir(dir.path().join(".clin")).unwrap();
        fs::write(dir.path().join(".clin/hidden.md"), "private").unwrap();
        fs::write(dir.path().join("secret.clin"), "ciphertext").unwrap();
        symlink(outside.path(), dir.path().join("escape")).unwrap();
        symlink(dir.path().join(".clin"), dir.path().join("hidden")).unwrap();
        symlink(dir.path().join("secret.clin"), dir.path().join("alias.md")).unwrap();
        for id in [
            "escape/outside.md",
            "escape/new.base",
            "hidden/hidden.md",
            "hidden/new.base",
            "alias.md",
        ] {
            assert!(storage.document_path(id).is_err(), "{id}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn saves_keep_permissions_and_reject_readonly_documents() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, storage) = vault();
        let snapshot = storage.create_document("a.base", "views: []\n").unwrap();
        let path = dir.path().join("a.base");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let snapshot = storage
            .save_document(&snapshot, "# edit\nviews: []\n")
            .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o440)).unwrap();
        assert!(storage.save_document(&snapshot, "views: []\n").is_err());
    }
}
