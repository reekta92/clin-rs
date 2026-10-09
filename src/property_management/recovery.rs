#[cfg(test)]
mod tests;

use super::{App, PropertyBatch};
use crate::{frontmatter, fsutil, storage::Storage};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparedChange {
    pub(super) source: String,
    pub(super) target: String,
    artifact: String,
    pub(super) folder: bool,
}
#[derive(Serialize, Deserialize)]
struct PreparedPayload {
    vault: PathBuf,
    source: String,
    target: String,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
    tree: Option<Vec<(String, Option<Vec<u8>>)>>,
}

fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path.parent().context("Missing parent")?)?.sync_all()?;
    Ok(())
}
fn create_parent(path: &Path) -> Result<()> {
    let parent = path.parent().context("Missing parent")?;
    let missing = parent
        .ancestors()
        .take_while(|directory| !directory.exists())
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    fs::create_dir_all(parent)?;
    for directory in missing.iter().rev() {
        sync_parent(directory)?;
    }
    Ok(())
}
fn durable_write(path: &Path, bytes: &[u8]) -> Result<()> {
    fsutil::atomic_write(path, bytes)?;
    sync_parent(path)?;
    if let Some(parent) = path.parent().filter(|parent| parent.parent().is_some()) {
        sync_parent(parent)?;
    }
    Ok(())
}
fn safe_child(root: &Path, path: &Path) -> Result<PathBuf> {
    let relative = path
        .strip_prefix(root)
        .context("Recovery target outside allowed root")?;
    ensure!(
        !relative.as_os_str().is_empty(),
        "Recovery cannot replace root"
    );
    let mut current = root.to_path_buf();
    for part in relative.components() {
        let std::path::Component::Normal(name) = part else {
            anyhow::bail!("Invalid recovery path");
        };
        current.push(name);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "Recovery refuses symlink targets"
            );
        }
    }
    Ok(current)
}
fn note_path(storage: &Storage, id: &str) -> Result<PathBuf> {
    storage.property_note_path(id)?;
    let root = storage.notes_dir.canonicalize()?;
    safe_child(&root, &root.join(id))
}
fn internal_path(storage: &Storage, name: &str) -> Result<PathBuf> {
    let root = storage.data_dir.canonicalize()?;
    safe_child(&root, &root.join(".clin").join(name))
}
fn artifact_path(storage: &Storage, name: &str) -> Result<PathBuf> {
    let uuid = name
        .strip_suffix(".bin")
        .context("Invalid recovery artifact")?;
    uuid::Uuid::parse_str(uuid).context("Invalid recovery artifact")?;
    internal_path(storage, &format!("property-recovery/{name}"))
}
fn snapshot(root: &Path) -> Result<Vec<(String, Option<Vec<u8>>)>> {
    let mut pending = vec![root.to_path_buf()];
    let mut entries = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "Folder relocation refuses symlinks"
            );
            let name = path
                .strip_prefix(root)?
                .to_str()
                .context("Non-UTF8 relocation path")?
                .to_owned();
            let content = if metadata.is_dir() {
                pending.push(path.clone());
                None
            } else {
                ensure!(metadata.is_file(), "Unsupported relocation entry");
                Some(fs::read(&path)?)
            };
            entries.push((name, content));
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
impl PreparedChange {
    fn payload(&self, storage: &Storage) -> Result<PreparedPayload> {
        note_path(storage, &self.source)?;
        note_path(storage, &self.target)?;
        let bytes = storage.decrypt(&fs::read(artifact_path(storage, &self.artifact)?)?)?;
        let (payload, used): (PreparedPayload, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
        ensure!(
            used == bytes.len()
                && payload.vault == storage.data_dir.canonicalize()?
                && payload.source == self.source
                && payload.target == self.target
                && payload.tree.is_some() == self.folder,
            "Recovery artifact identity mismatch"
        );
        Ok(payload)
    }
    fn apply(&self, storage: &mut Storage, payload: &PreparedPayload) -> Result<bool> {
        let source = note_path(storage, &self.source)?;
        let target = note_path(storage, &self.target)?;
        if let Some(tree) = &payload.tree {
            if !source.exists() && target.is_dir() && snapshot(&target)? == *tree {
                return Ok(false);
            }
            ensure!(
                source.is_dir() && !target.exists() && snapshot(&source)? == *tree,
                "Folder changed since preview; inspect recovery artifacts"
            );
            create_parent(&target)?;
            fs::rename(&source, &target)?;
            sync_parent(&source)?;
            sync_parent(&target)?;
            return Ok(true);
        }
        let actual_source = read_optional(&source)?;
        let actual_target = read_optional(&target)?;
        if source == target {
            if actual_target.as_ref() == Some(&payload.after) {
                return Ok(false);
            }
            ensure!(
                actual_source == payload.before,
                "Source changed since preview; inspect and retry"
            );
        } else {
            ensure!(
                actual_source == payload.before
                    || (actual_source.is_none() && actual_target.as_ref() == Some(&payload.after)),
                "Source changed since preview; inspect and retry"
            );
            ensure!(
                actual_target.is_none()
                    || actual_target.as_ref() == Some(&payload.after)
                    || (fsutil::is_same_file(&source, &target) && actual_target == payload.before),
                "Destination changed since preview; inspect and retry"
            );
        }
        let mut wrote = false;
        if actual_target.as_ref() != Some(&payload.after) {
            create_parent(&target)?;
            durable_write(&target, &payload.after)?;
            wrote = true;
        }
        if source != target && actual_source.is_some() && !fsutil::is_same_file(&source, &target) {
            fs::remove_file(&source).context("Failed to remove committed relocation source")?;
            sync_parent(&source)?;
            wrote = true;
        }
        Ok(wrote)
    }
}
impl PropertyBatch {
    pub(super) fn recovery_path(app: &App) -> PathBuf {
        app.storage.data_dir.join(".clin/property_batch.toml")
    }
    fn checkpoint(&self, storage: &Storage) -> Result<()> {
        durable_write(
            &internal_path(storage, "property_batch.toml")?,
            toml::to_string(self)?.as_bytes(),
        )
    }
    pub(crate) fn prepare_file(
        &mut self,
        storage: &Storage,
        source: &str,
        target: &str,
        mut after: Vec<u8>,
    ) -> Result<()> {
        let source_path = note_path(storage, source)?;
        note_path(storage, target)?;
        if let Some(change) = self.notes.iter().find(|change| change.id == target) {
            let before =
                frontmatter::checked_parse(change.before.as_deref().unwrap_or("---\n---\n"))?.extra;
            let changed = frontmatter::checked_parse(&change.after)?.extra;
            let edits = changed
                .iter()
                .filter(|(key, value)| before.get(*key) != Some(*value))
                .map(|(key, value)| {
                    crate::property_model::property_edit(
                        key.as_str().context("Reference key must be text")?,
                        Some(value),
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            let (header, payload) = frontmatter::split_header(&after)?;
            let header = frontmatter::apply_edits(header.unwrap_or("---\n---\n"), &edits)?;
            let mut merged = header.into_bytes();
            merged.extend_from_slice(payload);
            after = merged;
        }
        self.prepare(
            storage,
            PreparedPayload {
                vault: storage.data_dir.canonicalize()?,
                source: source.into(),
                target: target.into(),
                before: read_optional(&source_path)?,
                after,
                tree: None,
            },
        )?;
        self.notes.retain(|change| change.id != target);
        Ok(())
    }
    pub(crate) fn prepare_folder(
        &mut self,
        storage: &Storage,
        source: &str,
        target: &str,
    ) -> Result<()> {
        let source_path = note_path(storage, source)?;
        let target_path = note_path(storage, target)?;
        ensure!(
            !target_path.starts_with(&source_path),
            "Cannot move folder into itself"
        );
        self.prepare(
            storage,
            PreparedPayload {
                vault: storage.data_dir.canonicalize()?,
                source: source.into(),
                target: target.into(),
                before: None,
                after: Vec::new(),
                tree: Some(snapshot(&source_path)?),
            },
        )
    }
    fn prepare(&mut self, storage: &Storage, payload: PreparedPayload) -> Result<()> {
        ensure!(
            !internal_path(storage, "property_batch.toml")?.exists(),
            "Resume pending property batch before writing notes"
        );
        let name = format!("{}.bin", uuid::Uuid::new_v4());
        let path = artifact_path(storage, &name)?;
        create_parent(&path)?;
        let mut crypt = storage.clone();
        crypt.ensure_key()?;
        let bytes = bincode::serde::encode_to_vec(&payload, bincode::config::standard())?;
        durable_write(&path, &crypt.encrypt(&bytes)?)?;
        if payload.source != payload.target {
            self.parents
                .insert(payload.source.clone(), payload.target.clone());
        }
        self.version = Some(1);
        self.vault = Some(storage.data_dir.canonicalize()?);
        self.writes.push(PreparedChange {
            source: payload.source,
            target: payload.target,
            artifact: name,
            folder: payload.tree.is_some(),
        });
        Ok(())
    }
    fn binding_path(storage: &Storage, change: &super::BindingChange) -> Result<PathBuf> {
        let path = &change.path;
        let config = crate::config::ClinConfig::config_path()?;
        let definitions = crate::property_model::PropertyDefinitions::path(&storage.data_dir);
        if *path == config {
            let root = path
                .parent()
                .context("Missing config parent")?
                .canonicalize()?;
            let target = root.join(path.file_name().context("Missing config name")?);
            safe_child(&root, &target)?;
            toml::from_str::<crate::config::ClinConfig>(&change.after)?;
            return Ok(target);
        }
        let vault = storage.data_dir.canonicalize()?;
        let target = if *path == definitions {
            let value: crate::property_model::PropertyDefinitions = toml::from_str(&change.after)?;
            value.check()?;
            vault.join(".clin/properties.toml")
        } else {
            let relative = path
                .strip_prefix(&storage.templates_dir)
                .context("Binding must target config, definitions or vault template")?;
            ensure!(
                path.extension().is_some_and(|ext| ext == "toml"),
                "Invalid template binding"
            );
            toml::from_str::<crate::templates::Template>(&change.after)?;
            let templates = storage.templates_dir.canonicalize()?;
            ensure!(
                templates.starts_with(&vault),
                "Template binding outside vault"
            );
            templates.join(relative)
        };
        safe_child(&vault, &target)
    }
    fn validate(&self, storage: &Storage) -> Result<()> {
        ensure!(
            self.version.is_none() || self.version == Some(1),
            "Unsupported recovery version; retain journal and artifacts"
        );
        if self.version.is_none() {
            ensure!(
                self.writes.is_empty() && self.parents.is_empty() && self.vault.is_none(),
                "Versionless recovery supports headers and bindings only"
            );
        } else {
            ensure!(
                self.vault.as_ref() == Some(&storage.data_dir.canonicalize()?),
                "Recovery belongs to another vault"
            );
        }
        for change in &self.notes {
            let path = note_path(storage, &change.id)?;
            ensure!(
                matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("md" | "txt" | "clin")
                ),
                "Header recovery supports note files only"
            );
            if let Some(header) = &change.before {
                frontmatter::validate_header(header)?;
            }
            frontmatter::validate_header(&change.after)?;
        }
        for change in &self.bindings {
            Self::binding_path(storage, change)?;
        }
        for (source, target) in &self.parents {
            note_path(storage, source)?;
            note_path(storage, target)?;
        }
        Ok(())
    }
    pub(crate) fn apply_storage(&mut self, storage: &Storage) -> Result<(Vec<String>, usize)> {
        let mut crypt = storage.clone();
        if !self.writes.is_empty() || !self.parents.is_empty() {
            crypt.ensure_key()?;
        }
        let lock_path = internal_path(storage, "property_batch.lock")?;
        create_parent(&lock_path)?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.try_lock()
            .map_err(|error| anyhow::anyhow!("Recovery busy: {error}"))?;
        self.validate(storage)?;
        let path = internal_path(storage, "property_batch.toml")?;
        if path.exists() {
            let pending: Self = toml::from_str(&fs::read_to_string(&path)?)?;
            ensure!(
                pending == *self,
                "Pending recovery exists; resume it before another mutation"
            );
        }
        let payloads = self
            .writes
            .iter()
            .map(|change| change.payload(&crypt))
            .collect::<Result<Vec<_>>>()?;
        self.version = Some(1);
        self.vault = Some(storage.data_dir.canonicalize()?);
        self.checkpoint(storage)?;
        let mut successes = 0;
        let mut failures = Vec::new();
        for payload in payloads {
            let change = &self.writes[0];
            match change.apply(&mut crypt, &payload) {
                Ok(wrote) => {
                    successes += usize::from(wrote);
                    let artifact = artifact_path(storage, &change.artifact)?;
                    self.writes.remove(0);
                    self.checkpoint(storage)?;
                    fsutil::remove_file_if_exists(&artifact)?;
                }
                Err(error) => {
                    failures.push(format!("{} -> {}: {error:#}", change.source, change.target));
                    break;
                }
            }
        }
        if self.writes.is_empty() {
            for (source, target) in self.parents.clone() {
                let migration = crypt
                    .retarget_editor_draft(&source, &target)
                    .and_then(|()| crypt.migrate_subnotes_parent(&source, &target));
                match migration {
                    Ok(()) => {
                        self.parents.remove(&source);
                        self.checkpoint(storage)?;
                    }
                    Err(error) => {
                        failures.push(format!("Subnotes {source}: {error:#}"));
                        break;
                    }
                }
            }
        }
        if self.writes.is_empty() && self.parents.is_empty() {
            let mut index = 0;
            while index < self.notes.len() {
                let change = &self.notes[index];
                let result = storage.load_frontmatter(&change.id).and_then(|actual| {
                    if actual.as_deref() == Some(change.after.as_str()) {
                        return Ok(false);
                    }
                    storage
                        .replace_property_header(
                            &change.id,
                            change.before.as_deref(),
                            &change.after,
                        )
                        .map(|()| true)
                });
                match result {
                    Ok(wrote) => {
                        successes += usize::from(wrote);
                        self.notes.remove(index);
                        self.checkpoint(storage)?;
                    }
                    Err(error) => {
                        failures.push(format!("{}: {error:#}", change.id));
                        index += 1;
                    }
                }
            }
        }
        if self.writes.is_empty() && self.parents.is_empty() && self.notes.is_empty() {
            let mut index = 0;
            while index < self.bindings.len() {
                let change = &self.bindings[index];
                let result = (|| -> Result<bool> {
                    let target = Self::binding_path(storage, change)?;
                    let actual = read_optional(&target)?;
                    if actual.as_deref() == Some(change.after.as_bytes()) {
                        return Ok(false);
                    }
                    ensure!(
                        if change.create {
                            actual.is_none()
                        } else {
                            actual.as_deref() == Some(change.before.as_bytes())
                        },
                        "Binding changed since preview; inspect and retry"
                    );
                    durable_write(&target, change.after.as_bytes())?;
                    Ok(true)
                })();
                match result {
                    Ok(wrote) => {
                        successes += usize::from(wrote);
                        self.bindings.remove(index);
                        self.checkpoint(storage)?;
                    }
                    Err(error) => {
                        failures.push(format!("{}: {error:#}", change.path.display()));
                        index += 1;
                    }
                }
            }
        }
        if self.writes.is_empty()
            && self.parents.is_empty()
            && self.notes.is_empty()
            && self.bindings.is_empty()
        {
            fsutil::remove_file_if_exists(&path)?;
            sync_parent(&path)?;
        }
        Ok((failures, successes))
    }
    pub fn apply(&mut self, app: &mut App) -> Result<Vec<String>> {
        let (failures, successes) = self.apply_storage(&app.storage)?;
        if successes > 0 {
            app.request_notes_reconcile();
            app.reload_config();
            app.reload_property_definitions();
            app.enqueue_backup("properties: batch edit");
        }
        Ok(failures)
    }
}
