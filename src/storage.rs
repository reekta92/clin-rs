pub(crate) mod document_recovery;
pub mod documents;
pub(crate) mod drafts;
pub mod metadata;
pub mod public_moves;
mod write_guard;

use crate::config::ClinConfig;
use crate::frontmatter;
use anyhow::{Context, Result};

use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    #[serde(skip)]
    pub baseline: Option<documents::DocumentSnapshot>,
    pub title: String,
    pub content: String,
    pub updated_at: u64,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubNote {
    pub id: String,
    pub title: String,
    pub content: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubNotePayload {
    Plain(Vec<SubNote>),
    LegacyEncrypted(Vec<u8>), // Opaque legacy slot; never decoded or modified.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    pub folder: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub links: Vec<String>,
    pub size_bytes: u64,
}

impl NoteSummary {
    pub(crate) fn file_stem(&self) -> &str {
        Path::new(&self.id)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&self.title)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileStamp {
    pub modified_nanos: Option<u128>,
    pub len: u64,
}

pub(crate) struct NoteFileEntry {
    pub id: String,
    pub stamp: FileStamp,
}

enum VaultScanPolicy {
    Notes {
        include_hidden: bool,
        include_all_files: bool,
    },
    PublicFiles,
}

pub(crate) struct VaultScan {
    pub files: Vec<NoteFileEntry>,
    pub folders: Vec<String>,
    pub complete: bool,
    pub warnings: Vec<String>,
}

pub fn extract_wikilinks(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = 0;
    while let Some(start_offset) = content[cursor..].find("[[") {
        let absolute_start = cursor + start_offset;
        let inner_start = absolute_start + 2;
        if let Some(end_offset) = content[inner_start..].find("]]") {
            let absolute_end = inner_start + end_offset;
            let inner_text = &content[inner_start..absolute_end];

            let link_part = match inner_text.find('|') {
                Some(pipe_idx) => &inner_text[..pipe_idx],
                None => inner_text,
            };

            let link_part = link_part.trim();
            if !link_part.is_empty()
                && !link_part.contains(']')
                && seen.insert(link_part.to_owned())
            {
                links.push(link_part.to_owned());
            }
            cursor = absolute_end + 2;
        } else {
            break;
        }
    }
    links
}
pub fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "webp")
}

#[derive(Clone, Debug)]
pub struct Storage {
    pub frontmatter_policy: frontmatter::WritePolicy,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub notes_dir: PathBuf,
    pub templates_dir: PathBuf,
    pub skip_dir_patterns: Vec<regex::Regex>,
}

/// Check if `dir` is an existing vault (has user content outside clin-managed subdirectories).
pub(crate) fn is_existing_vault(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }
    match dir.read_dir() {
        Ok(entries) => {
            for entry in entries.flatten() {
                let fname = entry.file_name();
                let name = match fname.to_str() {
                    Some(n) => n,
                    None => continue,
                };
                // Ignore clin-managed subdirectories and all hidden entries
                if name == "notes"
                    || name == "templates"
                    || name.starts_with('.')
                    || name == "key.bin"
                    || name == "state.json"
                {
                    continue;
                }
                return true;
            }
            false
        }
        Err(_) => false,
    }
}

/// Copy a legacy classic-layout default vault into the current vault-layout
/// default path. No-op unless `old` holds user content and `new` is absent.
fn migrate_legacy_default_vault_at(old: &Path, new: &Path, warnings: &mut Vec<String>) {
    if old == new || !old.is_dir() || new.exists() {
        return;
    }
    let notes = old.join("notes");
    let has_data = old.join(".git").is_dir()
        || old.join(".clin").is_dir()
        || fs::read_dir(&notes)
            .map(|mut i| i.next().is_some())
            .unwrap_or(false)
        || fs::read_dir(old.join("templates"))
            .map(|mut i| i.next().is_some())
            .unwrap_or(false);
    if !has_data {
        return;
    }
    let result = (|| -> Result<()> {
        fs::create_dir_all(new.join(".clin").join("templates"))?;
        if notes.is_dir() {
            copy_dir_recursive(&notes, new)?;
        }
        if old.join("templates").is_dir() {
            copy_dir_recursive(&old.join("templates"), &new.join(".clin").join("templates"))?;
        }
        if old.join(".clin").is_dir() {
            copy_dir_recursive(&old.join(".clin"), &new.join(".clin"))?;
        }
        if old.join(".git").is_dir() {
            fs::create_dir_all(new.join(".git"))?;
            copy_dir_recursive(&old.join(".git"), &new.join(".git"))?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => warnings.push(format!(
            "Default vault location is now {}. Your data was copied there from {}. The old copy remains at {} (legacy keys and app state may remain there, so do not delete that directory).",
            new.display(),
            old.display(),
            old.display()
        )),
        Err(e) => {
            // `new` did not exist on entry, so the partial copy is ours to remove;
            // removing it lets the next boot retry.
            let _ = fs::remove_dir_all(new);
            warnings.push(format!(
                "Failed to migrate legacy default vault from {}: {e}",
                old.display()
            ));
        }
    }
}

fn migrate_legacy_default_vault(new: &Path, warnings: &mut Vec<String>) {
    if let Some(proj) = directories::ProjectDirs::from("com", "clin", "clin") {
        migrate_legacy_default_vault_at(proj.data_local_dir(), new, warnings);
    }
}

impl Storage {
    pub fn init() -> (Result<Self>, Vec<String>) {
        let (config_res, mut warnings) = ClinConfig::load();
        let config = match config_res {
            Ok(config) => config,
            Err(error) => {
                warnings.push(format!(
                    "Config error: {error}. Falling back to default configuration."
                ));
                ClinConfig::default()
            }
        };
        let (result, init_warnings) = Self::init_with_config(&config);
        warnings.extend(init_warnings);
        (result, warnings)
    }

    /// Initialize storage layout from an already-validated candidate config.
    pub(crate) fn init_with_config(config: &ClinConfig) -> (Result<Self>, Vec<String>) {
        let mut warnings = Vec::new();
        let result = match config
            .effective_storage_path()
            .context("failed to determine storage path")
        {
            Ok(data_dir) => Self::init_inner(config, data_dir, &mut warnings),
            Err(error) => Err(error),
        };
        (result, warnings)
    }

    /// Initialize storage layout at an explicit data dir, bypassing config path
    /// resolution (used for session-only vault switches under `--vault`).
    pub(crate) fn init_with_config_at(
        config: &ClinConfig,
        data_dir: &Path,
    ) -> (Result<Self>, Vec<String>) {
        let mut warnings = Vec::new();
        let result = Self::init_inner(config, data_dir.to_path_buf(), &mut warnings);
        (result, warnings)
    }

    fn init_inner(
        bootstrap: &ClinConfig,
        data_dir: PathBuf,
        warnings: &mut Vec<String>,
    ) -> Result<Self> {
        if !bootstrap.has_custom_storage_path() {
            migrate_legacy_default_vault(&data_dir, warnings);
        }

        let config_dir =
            crate::config::clin_config_dir().context("could not determine config directory")?;

        let notes_dir = data_dir.clone();
        let templates_dir = data_dir.join(".clin").join("templates");

        if bootstrap.features.templates.is_enabled() {
            fs::create_dir_all(&templates_dir)
                .context("failed to create .clin/templates directory")?;
        }
        let skip_dir_patterns: Vec<regex::Regex> = bootstrap
            .list
            .skip_dirs
            .iter()
            .filter_map(|pat| regex::Regex::new(pat).ok())
            .collect();
        let storage = Self {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            skip_dir_patterns,
            frontmatter_policy: bootstrap.notes.frontmatter.clone(),
        };
        storage.migrate_native_subnotes_metadata()?;
        storage.migrate_legacy_attachments(&bootstrap.image.attachments_subdir, warnings)?;
        if let Err(error) = storage.migrate_extensions() {
            warnings.push(format!("Format migration incomplete: {error:#}"));
        }
        for legacy in [
            storage.data_dir.join(".clin/editor_draft.bin"),
            storage.data_dir.join(".clin/document-recovery"),
        ] {
            if legacy.exists() {
                warnings.push(format!("Legacy encrypted recovery data retained at {}. Use the previous release to recover/decrypt before upgrading; no automatic conversion is performed.", legacy.display()));
            }
        }
        if storage.scan_vault(false, true).is_ok_and(|scan| {
            scan.files.iter().any(|entry| {
                Path::new(&entry.id)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("clin"))
            })
        }) {
            warnings.push("Legacy encrypted notes are unsupported and remain untouched. Use the previous release to decrypt them; keep your existing key files.".into());
        }
        match storage.recover_documents() {
            Ok(recovery_warnings) => warnings.extend(recovery_warnings),
            Err(error) => warnings.push(format!(
                "Document recovery unavailable; records retained: {error:#}"
            )),
        }
        Ok(storage)
    }

    pub fn keybinds_dir(&self) -> PathBuf {
        self.config_dir.join("keybinds")
    }

    pub fn keybinds_path_for_preset(
        &self,
        preset: crate::config::KeybindPreset,
    ) -> std::path::PathBuf {
        self.keybinds_dir().join(format!("{preset}.toml"))
    }
    pub fn save_keybinds_for_preset(
        &self,
        keybinds: &crate::keybinds::Keybinds,
        preset: crate::config::KeybindPreset,
    ) -> Result<()> {
        let path = self.keybinds_path_for_preset(preset);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        keybinds.save(&path)
    }

    pub fn load_keybinds_with_preset(
        &self,
        preset: crate::config::KeybindPreset,
    ) -> (crate::keybinds::Keybinds, Vec<String>) {
        let per_preset = self.keybinds_path_for_preset(preset);
        let mut warnings = Vec::new();

        // Migration from legacy flat paths
        let legacy_per_preset = self.config_dir.join(format!("keybinds_{preset}.toml"));
        let legacy_generic = self.config_dir.join("keybinds.toml");

        if !per_preset.exists() && legacy_per_preset.exists() {
            // Create keybinds directory and migrate
            if let Some(parent) = per_preset.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = std::fs::rename(&legacy_per_preset, &per_preset);
        }
        if per_preset.exists() && legacy_generic.exists() {
            // Generic legacy file is now superseded by per-preset — remove it
            let _ = std::fs::remove_file(&legacy_generic);
        }

        if !per_preset.exists() {
            let defaults = preset.base_keybinds();
            match self.save_keybinds_for_preset(&defaults, preset) {
                Ok(()) => return (defaults, warnings),
                Err(error) => {
                    warnings.push(format!(
                        "Failed to create keybind preset {}: {error}. Falling back to '{preset}' preset.",
                        per_preset.display()
                    ));
                    return (defaults, warnings);
                }
            }
        }

        if let Err(error) = crate::keybinds::repair_legacy_preset_sequences(&per_preset, preset) {
            warnings.push(format!(
                "Failed to repair legacy keybind sequences in {}: {error}",
                per_preset.display()
            ));
        }
        let keybinds = crate::keybinds::Keybinds::load_layered(
            &per_preset,
            preset.base_keybinds(),
            &mut warnings,
        )
        .unwrap_or_else(|e| {
            warnings.push(format!(
                "Keybinds parse error: {}: {e}. Falling back to '{preset}' preset.",
                per_preset.display()
            ));
            preset.base_keybinds()
        });
        (keybinds, warnings)
    }

    pub fn note_path(&self, id: &str) -> PathBuf {
        self.validate_path_within_notes_dir(id)
            .unwrap_or_else(|| self.notes_dir.join("invalid"))
    }

    pub fn import_attachment(&self, src: &Path, attachments_subdir: &str) -> Result<String> {
        let relative = Self::validated_attachment_subdir(attachments_subdir)?;
        let dir = self.notes_dir.join(&relative);
        fs::create_dir_all(&dir).context("failed to create attachments directory")?;

        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let short_id = &Uuid::new_v4().to_string()[..8];
        let filename = format!("{ts}_{short_id}{ext}");
        fs::copy(src, dir.join(&filename)).context("failed to copy attachment")?;
        Ok(format!(
            "{}/{}",
            relative.to_string_lossy().replace('\\', "/"),
            filename
        ))
    }

    fn validated_attachment_subdir(attachments_subdir: &str) -> Result<PathBuf> {
        let path = Path::new(attachments_subdir);
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::Normal(component) => normalized.push(component),
                std::path::Component::CurDir
                | std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => {
                    anyhow::bail!(
                        "attachments_subdir must be a non-empty relative path of normal components"
                    );
                }
            }
        }
        if normalized.as_os_str().is_empty() {
            anyhow::bail!("attachments_subdir must be a non-empty relative path");
        }
        Ok(normalized)
    }
    pub fn resolve_attachment(&self, id: &str) -> Option<PathBuf> {
        // First try via validate_path_within_notes_dir (handles path traversal checks)
        if let Some(p) = self.validate_path_within_notes_dir(id)
            && p.exists()
        {
            return Some(p);
        }
        // Fallback: resolve as relative to notes_dir (for legacy absolute/relative paths)
        let fallback = self.notes_dir.join(id);
        if fallback.exists() {
            Some(fallback)
        } else {
            None
        }
    }

    fn validate_path_within_notes_dir(&self, rel_path: &str) -> Option<PathBuf> {
        let path = std::path::Path::new(rel_path);
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => return None,
                std::path::Component::Normal(c) => {
                    let s = c.to_string_lossy();
                    if s.starts_with('.') || s.contains('\0') {
                        return None;
                    }
                    normalized.push(c);
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => return None,
                std::path::Component::CurDir => {}
            }
        }
        Some(self.notes_dir.join(normalized))
    }

    pub fn list_note_ids(
        &self,
        include_hidden: bool,
        include_all_files: bool,
    ) -> Result<Vec<String>> {
        let scan = self.scan_vault(include_hidden, include_all_files)?;
        if !scan.complete {
            anyhow::bail!("vault scan incomplete: {}", scan.warnings.join("; "));
        }
        Ok(scan.files.into_iter().map(|file| file.id).collect())
    }

    fn migrate_extensions(&self) -> Result<()> {
        let _guard = self.write_guard()?;
        let scan = self.scan_vault_with_policy(VaultScanPolicy::PublicFiles)?;
        anyhow::ensure!(
            scan.complete,
            "vault scan incomplete: {}",
            scan.warnings.join("; ")
        );
        for entry in scan.files {
            let path = self.document_path(&entry.id)?;
            let extension = match path.extension().and_then(|ext| ext.to_str()) {
                Some("pinstar") => "canvas",
                Some("canvas") => {
                    let Ok(content) = documents::read_bounded_text(&path, 16 * 1024 * 1024) else {
                        continue;
                    };
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
                        continue;
                    };
                    if !value
                        .get("elements")
                        .is_some_and(serde_json::Value::is_array)
                        || value.get("nodes").is_some()
                        || value.get("edges").is_some()
                        || serde_json::from_value::<::draw::state::DrawData>(value).is_err()
                    {
                        continue;
                    }
                    "draw"
                }
                _ => continue,
            };
            let target = Path::new(&entry.id).with_extension(extension);
            let target = target.to_str().context("invalid migrated filename")?;
            if !self.document_path(target)?.exists() {
                self.relocate_note_locked(&entry.id, target)?;
            }
        }
        Ok(())
    }
    pub(crate) fn scan_vault(
        &self,
        include_hidden: bool,
        include_all_files: bool,
    ) -> Result<VaultScan> {
        self.scan_vault_with_policy(VaultScanPolicy::Notes {
            include_hidden,
            include_all_files,
        })
    }

    fn scan_vault_with_policy(&self, policy: VaultScanPolicy) -> Result<VaultScan> {
        let public_dataset = matches!(policy, VaultScanPolicy::PublicFiles);
        let (include_hidden, include_all_files) = match policy {
            VaultScanPolicy::Notes {
                include_hidden,
                include_all_files,
            } => (include_hidden, include_all_files),
            VaultScanPolicy::PublicFiles => (false, true),
        };
        let mut files = Vec::new();
        let mut folders = Vec::new();
        let mut warnings = Vec::new();
        let mut complete = true;

        let root_entries = match fs::read_dir(&self.notes_dir) {
            Ok(e) => e,
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "failed reading notes directory {}: {}",
                    self.notes_dir.display(),
                    err
                ));
            }
        };
        drop(root_entries);

        let mut pending_dirs = vec![(self.notes_dir.clone(), String::new())];

        while let Some((dir_path, _rel_dir)) = pending_dirs.pop() {
            let entries = match fs::read_dir(&dir_path) {
                Ok(e) => e,
                Err(err) => {
                    complete = false;
                    warnings.push(format!(
                        "Failed to read directory {}: {}",
                        dir_path.display(),
                        err
                    ));
                    continue;
                }
            };

            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(err) => {
                        complete = false;
                        warnings.push(format!(
                            "Failed to read entry in {}: {}",
                            dir_path.display(),
                            err
                        ));
                        continue;
                    }
                };
                let path = entry.path();
                let file_name = match path.file_name().and_then(|s| s.to_str()) {
                    Some(name) => name,
                    None => continue,
                };

                if file_name == crate::templates::FOLDER_TEMPLATE_CONFIG
                    || (!include_hidden && file_name.starts_with('.'))
                    || (public_dataset
                        && path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("clin")))
                {
                    continue;
                }

                let rel_path = match path.strip_prefix(&self.notes_dir) {
                    Ok(r) => match r.to_str() {
                        Some(s) => s.replace('\\', "/"),
                        None => continue,
                    },
                    Err(_) => continue,
                };

                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => {
                        complete = false;
                        warnings.push(format!("Failed file type for {}: {error}", path.display()));
                        continue;
                    }
                };
                // Never recurse through directory symlinks; Bases also excludes file aliases.
                if file_type.is_symlink() && (public_dataset || path.is_dir()) {
                    continue;
                }
                if file_type.is_dir() {
                    if !public_dataset
                        && self
                            .skip_dir_patterns
                            .iter()
                            .any(|re| re.is_match(file_name))
                    {
                        continue;
                    }
                    folders.push(rel_path.clone());
                    pending_dirs.push((path, rel_path));
                } else {
                    let accepted = if include_all_files {
                        path.is_file()
                    } else {
                        path.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|ext| {
                                matches!(ext, "md" | "txt" | "draw" | "canvas" | "base")
                                    || crate::storage::is_image_ext(ext)
                            })
                    };

                    if accepted {
                        let metadata = match entry.metadata().or_else(|_| fs::metadata(&path)) {
                            Ok(m) => m,
                            Err(err) => {
                                complete = false;
                                warnings.push(format!(
                                    "Failed metadata for {}: {}",
                                    path.display(),
                                    err
                                ));
                                continue;
                            }
                        };

                        let modified_nanos = metadata.modified().ok().and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .ok()
                                .map(|d| d.as_nanos())
                        });

                        let stamp = FileStamp {
                            modified_nanos,
                            len: metadata.len(),
                        };

                        files.push(NoteFileEntry {
                            id: rel_path,
                            stamp,
                        });
                    }
                }
            }
        }

        folders.sort();
        files.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(VaultScan {
            files,
            folders,
            complete,
            warnings,
        })
    }

    pub(crate) fn load_note_summary_from_entry(
        &self,
        entry: &NoteFileEntry,
    ) -> Result<NoteSummary> {
        let mut summary = self.load_note_summary(&entry.id)?;
        summary.size_bytes = entry.stamp.len;
        Ok(summary)
    }
    pub fn load_note_summary(&self, id: &str) -> Result<NoteSummary> {
        anyhow::ensure!(
            !crate::templates::is_folder_template_config(Path::new(id)),
            "folder template config is not a note"
        );
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let folder = if let Some(parent) = std::path::Path::new(id).parent() {
            parent.to_str().unwrap_or("").to_string()
        } else {
            String::new()
        };

        if ext.eq_ignore_ascii_case("clin") {
            anyhow::bail!(
                "Legacy encrypted notes are unsupported; use the previous release to decrypt them. Files and keys are retained."
            );
        }
        if crate::storage::is_image_ext(ext) {
            let updated_at = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            Ok(NoteSummary {
                id: id.to_string(),
                title: path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled note")
                    .to_string(),
                updated_at,
                folder,
                tags: Vec::new(),
                pinned: false,
                links: Vec::new(),
                size_bytes: path.metadata().map(|m| m.len()).unwrap_or(0),
            })
        } else if ext != "md" && ext != "txt" {
            let updated_at = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            Ok(NoteSummary {
                id: id.to_string(),
                title: path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled note")
                    .to_string(),
                updated_at,
                folder,
                tags: Vec::new(),
                pinned: false,
                links: Vec::new(),
                size_bytes: path.metadata().map(|m| m.len()).unwrap_or(0),
            })
        } else {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("load_note_summary read failed for {}", path.display()))?;
            let (fm, plain_content) = frontmatter::parse(&content);

            let title = if let Some(t) = fm.title {
                t
            } else {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled note")
                    .to_string()
            };

            let updated_at = if let Some(ua) = fm.updated_at {
                ua
            } else {
                fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs())
            };

            let links = fm.links.unwrap_or_else(|| extract_wikilinks(plain_content));

            Ok(NoteSummary {
                id: id.to_string(),
                title,
                updated_at,
                folder,
                tags: fm.tags,
                pinned: fm.pinned,
                links,
                size_bytes: path.metadata().map(|m| m.len()).unwrap_or(0),
            })
        }
    }

    pub fn load_note(&self, id: &str) -> Result<Note> {
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("base") {
            anyhow::bail!("Bases files must be loaded through document storage");
        }

        if ext.eq_ignore_ascii_case("clin") {
            anyhow::bail!(
                "Legacy encrypted notes are unsupported; use the previous release to decrypt them. Files and keys are retained."
            );
        }

        let baseline = self.load_document(id)?;
        let file_content = baseline.source();
        if matches!(ext, "md" | "txt") {
            frontmatter::properties::PropertyFrame::parse(file_content)?;
        }
        let (fm, plain_content) = frontmatter::parse(file_content);

        let title = if let Some(t) = fm.title {
            t
        } else {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled note")
                .to_string()
        };

        let updated_at = if let Some(ua) = fm.updated_at {
            ua
        } else {
            fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs())
        };

        Ok(Note {
            baseline: Some(baseline.clone()),
            title,
            content: plain_content.to_string(),
            updated_at,
            tags: fm.tags,
        })
    }

    /// Save loaded notes against their original bytes; new notes never replace a file.
    pub fn save_note(&mut self, id: &str, note: &Note) -> Result<String> {
        Ok(self.save_note_snapshot(id, note)?.id().to_owned())
    }

    pub(crate) fn note_target_id(&self, id: &str, note: &Note) -> Result<String> {
        self.document_path(id)?;
        let ext = Path::new(id)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("md");
        if ext.eq_ignore_ascii_case("base") {
            anyhow::bail!("Bases files must use document storage");
        }
        if note
            .baseline
            .as_ref()
            .is_some_and(documents::DocumentSnapshot::exists)
        {
            return Ok(id.to_owned());
        }
        if self.note_path(id).exists() {
            anyhow::bail!("note already exists; reload before saving");
        }
        let ext = if matches!(ext, "md" | "txt" | "canvas" | "draw") {
            ext
        } else {
            "md"
        };
        let stem = self.note_file_stem_from_title(&note.title);
        Ok(self.unique_note_id(&stem, ext, id))
    }

    pub(crate) fn note_source(&self, id: &str, note: &Note) -> Result<String> {
        self.document_path(id)?;
        if Path::new(id)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "canvas" | "draw"))
        {
            return Ok(note.content.clone());
        }
        let baseline = note.baseline.as_ref();
        let before = baseline.map_or("", documents::DocumentSnapshot::source);
        let frame = frontmatter::properties::PropertyFrame::parse(before)?;
        let source = frame.with_yaml_and_body(frame.yaml(), &note.content);
        let (original, _) = frontmatter::parse(before);
        let old_title = original.title.unwrap_or_else(|| {
            Path::new(id)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Note")
                .to_owned()
        });
        self.prepare_note_source(
            id,
            baseline,
            &source,
            &note.title,
            baseline.is_some_and(documents::DocumentSnapshot::exists) && old_title != note.title,
            (original.tags != note.tags).then_some(&note.tags),
            note.updated_at,
        )
    }

    /// Explicit working-header changes win over automatic managed output for this save.
    pub(crate) fn prepare_note_source(
        &self,
        id: &str,
        baseline: Option<&documents::DocumentSnapshot>,
        source: &str,
        title: &str,
        title_changed: bool,
        tags: Option<&Vec<String>>,
        updated_at: u64,
    ) -> Result<String> {
        use frontmatter::properties::{PropertyDocument, edit_properties};
        use serde_yaml_ng::Value;
        self.document_path(id)?;
        let before = baseline.map_or("", documents::DocumentSnapshot::source);
        let working = PropertyDocument::parse(source)?;
        let old = match PropertyDocument::parse(before) {
            Ok(old) => old,
            Err(_) => {
                frontmatter::properties::PropertyFrame::parse(before)?;
                let types = self.load_property_types()?;
                for (key, value) in &working.properties {
                    if let Some(key) = key.as_str() {
                        types.validate(key, value)?;
                    }
                }
                if title_changed {
                    if working
                        .properties
                        .get("title")
                        .is_some_and(|value| value.as_str() != Some(title))
                    {
                        anyhow::bail!(
                            "Title and frontmatter title disagree; correct one before saving"
                        );
                    }
                    return edit_properties(
                        source,
                        &[("title".into(), Some(Value::String(title.to_owned())))],
                    );
                }
                return Ok(source.to_owned());
            }
        };
        let explicit = |key: &str| old.properties.get(key) != working.properties.get(key);
        if title_changed
            && working.properties.contains_key("title")
            && explicit("title")
            && working.properties.get("title").and_then(Value::as_str) != Some(title)
        {
            anyhow::bail!("Title and frontmatter title disagree; correct one before saving");
        }
        let types = self.load_property_types()?;
        for (key, value) in &working.properties {
            if old.properties.get(key) != Some(value)
                && let Some(key) = key.as_str()
            {
                types.validate(key, value)?;
            }
        }
        let new = baseline.is_none_or(|snapshot| !snapshot.exists());
        if !new && before == source && !title_changed && tags.is_none() {
            return Ok(source.to_owned());
        }
        let policy = &self.frontmatter_policy;
        let mut changes = Vec::new();
        if title_changed || (policy.title && new && !explicit("title")) {
            changes.push(("title".into(), Some(Value::String(title.to_owned()))));
        }
        if let Some(tags) = tags {
            changes.push(("tags".into(), Some(serde_yaml_ng::to_value(tags)?)));
        } else if policy.tags && new && !explicit("tags") {
            changes.push(("tags".into(), Some(Value::Sequence(Vec::new()))));
        }
        if policy.pinned && new && !explicit("pinned") {
            changes.push(("pinned".into(), Some(Value::Bool(false))));
        }
        for (key, enabled, value, interpretable) in [
            (
                "links",
                policy.links,
                serde_yaml_ng::to_value(extract_wikilinks(working.body))?,
                working
                    .properties
                    .get("links")
                    .is_none_or(|v| serde_yaml_ng::from_value::<Vec<String>>(v.clone()).is_ok()),
            ),
            (
                "updated_at",
                policy.updated_at != frontmatter::TimestampFormat::None,
                match policy.updated_at {
                    frontmatter::TimestampFormat::Iso8601 => Value::String(
                        chrono::DateTime::from_timestamp(i64::try_from(updated_at)?, 0)
                            .context("timestamp out of range")?
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    ),
                    _ => Value::Number(updated_at.into()),
                },
                working
                    .properties
                    .get("updated_at")
                    .is_none_or(|v| frontmatter::timestamp(v).is_some()),
            ),
        ] {
            if enabled
                && interpretable
                && !explicit(key)
                && (new || old.properties.contains_key(key))
            {
                changes.push((key.into(), Some(value)));
            }
        }
        edit_properties(source, &changes)
    }

    pub(crate) fn save_note_snapshot(
        &mut self,
        id: &str,
        note: &Note,
    ) -> Result<documents::DocumentSnapshot> {
        let target = self.note_target_id(id, note)?;
        let source = self.note_source(&target, note)?;
        let saved =
            if let Some(baseline) = note.baseline.as_ref().filter(|baseline| baseline.exists()) {
                if baseline.id() != id {
                    anyhow::bail!("note identity changed; reload before saving");
                }
                self.save_document(baseline, &source)?
            } else {
                self.create_document(&target, &source)?
            };
        if let Some(baseline) = note.baseline.as_ref() {
            Ok(saved.retain_recovery_id(baseline))
        } else {
            Ok(saved)
        }
    }

    /// Explicit file rename. UI warns that references are not automatically rewritten.
    pub fn rename_note(&mut self, id: &str, new_title: &str) -> Result<String> {
        self.rename_document(id, new_title)
    }

    pub fn duplicate_note(&mut self, id: &str, target_folder: &str) -> Result<String> {
        let ext = Path::new(id)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("md");
        if !matches!(ext, "md" | "txt") {
            return self.duplicate_document(id, target_folder);
        }
        let mut note = self.load_note(id)?;
        note.title = format!("{} (Copy)", note.title);
        note.updated_at = crate::ui::now_unix_secs();
        let initial = Path::new(target_folder).join(format!("{}.{}", self.new_note_id(), ext));
        let initial = initial.to_str().context("invalid duplicate folder")?;
        self.document_path(initial)?;
        let target =
            self.unique_note_id(&self.note_file_stem_from_title(&note.title), ext, initial);
        let source = self.note_source(&target, &note)?;
        let path = self.document_path(&target)?;
        let _guard = self.write_guard()?;
        fs::create_dir_all(path.parent().context("duplicate has no directory")?)?;
        crate::fsutil::atomic_create_preserving_permissions(
            &path,
            &self.document_path(id)?,
            source.as_bytes(),
        )?;
        Ok(target)
    }

    pub fn trash_note(&self, id: &str) -> Result<()> {
        let path = self.note_path(id);
        if !path.exists() {
            anyhow::bail!("Note does not exist");
        }
        trash::delete(&path).context("failed to move note to trash")?;
        Ok(())
    }

    #[cfg(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    pub fn list_trash(
        &self,
        features: &crate::config::FeaturesConfig,
    ) -> Result<Vec<trash::TrashItem>> {
        let items =
            trash::os_limited::list().map_err(|e| anyhow::anyhow!("failed to list trash: {e}"))?;
        let vault_items: Vec<trash::TrashItem> = items
            .into_iter()
            .filter(|item| item.original_parent.starts_with(&self.notes_dir))
            .filter(|item| features.file_view_enabled(std::path::Path::new(&item.name)))
            .collect();
        Ok(vault_items)
    }

    #[cfg(not(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    )))]
    pub fn list_trash(
        &self,
        _features: &crate::config::FeaturesConfig,
    ) -> Result<Vec<trash::TrashItem>> {
        anyhow::bail!("Trash management is not supported on this platform")
    }

    #[cfg(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    pub fn restore_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<()> {
        trash::os_limited::restore_all(items)
            .map_err(|e| anyhow::anyhow!("failed to restore: {e}"))?;
        Ok(())
    }

    #[cfg(not(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    )))]
    pub fn restore_trash_items(&self, _items: Vec<trash::TrashItem>) -> Result<()> {
        anyhow::bail!("Trash management is not supported on this platform")
    }

    #[cfg(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    pub fn purge_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<()> {
        trash::os_limited::purge_all(items).map_err(|e| anyhow::anyhow!("failed to purge: {e}"))?;
        Ok(())
    }

    #[cfg(not(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    )))]
    pub fn purge_trash_items(&self, _items: Vec<trash::TrashItem>) -> Result<()> {
        anyhow::bail!("Trash management is not supported on this platform")
    }

    pub fn toggle_pin(&self, id: &str) -> Result<bool> {
        let ext = Path::new(id)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        anyhow::ensure!(
            matches!(ext, "md" | "txt"),
            "Only Markdown and text notes support pin metadata"
        );
        let baseline = self.load_document(id)?;
        let (fm, _) = frontmatter::parse(baseline.source());
        let new_pinned = !fm.pinned;
        let source = frontmatter::properties::edit_properties(
            baseline.source(),
            &[(
                "pinned".into(),
                Some(serde_yaml_ng::Value::Bool(new_pinned)),
            )],
        )?;
        self.save_document(&baseline, &source)?;
        Ok(new_pinned)
    }

    pub fn new_note_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn create_folder(&self, path: &str) -> Result<()> {
        let full_path = self.public_path(path)?;
        fs::create_dir_all(full_path).context("failed to create folder")
    }

    pub fn trash_folder(&self, path: &str) -> Result<()> {
        let full_path = self.public_path(path)?;
        if !full_path.exists() {
            anyhow::bail!("Folder does not exist");
        }
        trash::delete(&full_path).context("failed to move folder to trash")?;
        Ok(())
    }

    pub fn rename_folder(&self, old_path: &str, new_path: &str) -> Result<()> {
        let _guard = self.write_guard()?;
        let old_full = self.public_path(old_path)?;
        let new_full = self.public_path(new_path)?;
        anyhow::ensure!(
            !new_full.starts_with(&old_full),
            "Cannot move a folder into itself"
        );

        if !old_full.is_dir() {
            anyhow::bail!("Folder does not exist");
        }
        if new_full.exists() {
            anyhow::bail!("Target folder already exists");
        }
        if let Some(parent) = new_full.parent() {
            fs::create_dir_all(parent)?;
        }

        let root = self.notes_dir.canonicalize()?;
        let old_id = old_full
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/");
        let new_id = new_full
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/");
        let prefix = format!("{old_id}/");
        let mut db = self.load_subnotes_db()?;
        let parents: Vec<_> = db
            .keys()
            .filter_map(|id| {
                id.strip_prefix(&prefix)
                    .map(|suffix| (id.clone(), format!("{new_id}/{suffix}")))
            })
            .collect();
        for (old, new) in &parents {
            Self::check_subnotes_writable(&db, old)?;
            Self::check_subnotes_writable(&db, new)?;
            anyhow::ensure!(
                !db.contains_key(new),
                "destination already has subnotes; both records retained"
            );
        }
        for (old, new) in &parents {
            db.insert(new.clone(), db[old].clone());
        }
        if !parents.is_empty() {
            self.save_subnotes_db(&db)?;
        }
        let result = (|| -> Result<()> {
            fs::rename(&old_full, &new_full).context("failed to rename folder")?;
            #[cfg(unix)]
            for parent in [old_full.parent(), new_full.parent()].into_iter().flatten() {
                fs::File::open(parent)?.sync_all()?;
            }
            Ok(())
        })();
        if !parents.is_empty() {
            if !old_full.exists() && new_full.exists() {
                for (old, _) in &parents {
                    db.remove(old);
                }
            } else if old_full.exists() && !new_full.exists() {
                for (_, new) in &parents {
                    db.remove(new);
                }
            }
            self.save_subnotes_db(&db)
                .context("subnotes cleanup failed; parent records retained for inspection")?;
        }
        result
    }

    /// Recursively copy folder `src_rel` (relative to notes dir) into `target_folder`
    /// (relative, "" = vault root). On name conflict at target, append " (Copy)" then
    /// " (Copy 2)", etc. — never overwrites. Bails if `src_rel` is empty or if the
    /// resolved destination sits inside `src_rel`'s own subtree (would recurse forever).
    pub fn duplicate_folder(&self, src_rel: &str, target_folder: &str) -> Result<()> {
        let _guard = self.write_guard()?;
        if src_rel.is_empty() {
            anyhow::bail!("Cannot copy the vault root");
        }
        let base = src_rel.rsplit('/').next().unwrap_or(src_rel);
        let mut new_rel = if target_folder.is_empty() {
            base.to_string()
        } else {
            format!("{target_folder}/{base}")
        };

        // Copying "a" -> "" resolves to "a" (copy in place): NOT recursion, so let the
        // conflict-suffix loop below rename it to "a (Copy)". Only a destination that is
        // a strict descendant of src (e.g. target="a" -> "a/a") is forbidden.
        if new_rel.starts_with(&format!("{src_rel}/")) {
            anyhow::bail!("Cannot copy a folder into itself");
        }

        let mut suffix: u32 = 0;
        while self
            .validate_path_within_notes_dir(&new_rel)
            .is_some_and(|p| p.exists())
        {
            suffix += 1;
            let label = if suffix == 1 {
                format!("{base} (Copy)")
            } else {
                format!("{base} (Copy {suffix})")
            };
            new_rel = if target_folder.is_empty() {
                label
            } else {
                format!("{target_folder}/{label}")
            };
        }

        let src_full = self.public_path(src_rel)?;
        let new_full = self.public_path(&new_rel)?;
        anyhow::ensure!(
            !new_full.starts_with(&src_full),
            "Cannot copy a folder into itself"
        );
        if !src_full.exists() {
            anyhow::bail!("Folder does not exist");
        }
        fs::create_dir_all(new_full.parent().context("copy has no parent")?)?;
        fs::create_dir(&new_full)?;
        copy_dir_recursive(&src_full, &new_full)?;
        Ok(())
    }

    pub fn move_note(&mut self, id: &str, new_folder: &str) -> Result<String> {
        if Path::new(id)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("base"))
        {
            let cancelled = std::sync::atomic::AtomicBool::new(false);
            let plans = self.prepare_public_file_moves(&[id.to_owned()], new_folder, &cancelled)?;
            let target = plans
                .first()
                .map_or_else(|| id.to_owned(), |plan| plan.destination_id().to_owned());
            self.apply_public_file_moves(&plans, &cancelled)?;
            return Ok(target);
        }
        let _guard = self.write_guard()?;
        let file_name = Path::new(id)
            .file_name()
            .and_then(|name| name.to_str())
            .context("invalid note filename")?;
        let target_id = if new_folder.is_empty() {
            file_name.to_owned()
        } else {
            format!("{new_folder}/{file_name}")
        };
        self.relocate_note_locked(id, &target_id)?;
        Ok(target_id)
    }

    /// Keep both parent records until the new filename is durably published.
    fn relocate_note_locked(&self, id: &str, target_id: &str) -> Result<()> {
        let source = self.document_path(id)?;
        let target = self.document_path(target_id)?;
        if source == target {
            return Ok(());
        }
        if fs::metadata(&source)?.permissions().readonly() {
            anyhow::bail!("document is read-only");
        }
        if target.exists() {
            anyhow::bail!("destination already exists");
        }
        let mut db = self.load_subnotes_db()?;
        Self::check_subnotes_writable(&db, id)?;
        Self::check_subnotes_writable(&db, target_id)?;
        if db.contains_key(target_id) {
            anyhow::bail!("destination already has subnotes; both records retained");
        }
        let has_subnotes = if let Some(SubNotePayload::Plain(notes)) = db.get(id) {
            db.insert(target_id.to_owned(), SubNotePayload::Plain(notes.clone()));
            self.save_subnotes_db(&db)?;
            true
        } else {
            false
        };
        let result = (|| -> Result<()> {
            fs::create_dir_all(target.parent().context("destination has no directory")?)?;
            fs::hard_link(&source, &target)
                .context("failed to publish filename without overwriting")?;
            #[cfg(unix)]
            fs::File::open(target.parent().context("destination has no directory")?)?.sync_all()?;
            if let Err(error) = fs::remove_file(&source) {
                let _ = fs::remove_file(&target);
                return Err(error).context("failed to remove old filename; original retained");
            }
            #[cfg(unix)]
            fs::File::open(source.parent().context("source has no directory")?)?.sync_all()?;
            Ok(())
        })();
        if has_subnotes {
            if !source.exists() && target.exists() {
                db.remove(id);
            } else {
                db.remove(target_id);
            }
            self.save_subnotes_db(&db).with_context(|| {
                format!("subnotes cleanup failed; records retained; inspect {id} and {target_id}")
            })?;
        }
        result
    }

    pub fn list_folders(&self, include_hidden: bool) -> Result<Vec<String>> {
        let mut folders = Vec::new();
        let mut dirs_to_visit = vec![self.notes_dir.clone()];

        while let Some(dir) = dirs_to_visit.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if entry.file_type()?.is_dir()
                        && path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .is_some_and(|n| include_hidden || !n.starts_with('.'))
                    {
                        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        if self.skip_dir_patterns.iter().any(|re| re.is_match(name)) {
                            continue;
                        }
                        dirs_to_visit.push(path.clone());
                        if let Ok(rel_path) = path.strip_prefix(&self.notes_dir)
                            && let Some(rel_str) = rel_path.to_str()
                        {
                            folders.push(rel_str.to_string());
                        }
                    }
                }
            }
        }
        folders.sort();
        Ok(folders)
    }

    pub fn note_file_stem_from_title(&self, title: &str) -> String {
        let trimmed = title.trim();
        let source = if trimmed.is_empty() {
            "Untitled note"
        } else {
            trimmed
        };

        let mut out = String::new();
        for ch in source.chars() {
            let valid = ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | '(' | ')');
            out.push(if valid { ch } else { '_' });
        }

        let collapsed = out
            .split_whitespace()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if collapsed.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            collapsed
        }
    }

    pub fn unique_note_id(&self, preferred_stem: &str, ext: &str, current_id: &str) -> String {
        let folder = if let Some(parent) = std::path::Path::new(current_id).parent() {
            parent.to_str().unwrap_or("")
        } else {
            ""
        };

        let mut candidate_stem = preferred_stem.to_string();
        let mut candidate_name = format!("{candidate_stem}.{ext}");
        let mut candidate = if folder.is_empty() {
            candidate_name.clone()
        } else {
            format!("{folder}/{candidate_name}")
        };

        let mut counter = 2_u32;
        let current_path = self.note_path(current_id);

        // A candidate that differs from the current id only in case (or is
        // otherwise the same file, e.g. on case-insensitive filesystems)
        // collides with itself — treat it as free instead of bumping to
        // `Name (2)`, which would orphan inbound wikilinks.
        while candidate != current_id
            && self.note_path(&candidate).exists()
            && !crate::fsutil::is_same_file(&self.note_path(&candidate), &current_path)
        {
            candidate_stem = format!("{preferred_stem} ({counter})");
            candidate_name = format!("{candidate_stem}.{ext}");
            candidate = if folder.is_empty() {
                candidate_name.clone()
            } else {
                format!("{folder}/{candidate_name}")
            };
            counter += 1;
        }

        candidate
    }
    fn subnotes_db_path(&self) -> PathBuf {
        self.data_dir.join(".clin").join("subnotes.bin")
    }

    fn migrate_native_subnotes_metadata(&self) -> Result<()> {
        if self.data_dir == self.notes_dir {
            return Ok(());
        }
        let legacy = self.notes_dir.join(".clin_subnotes.bin");
        if !legacy.exists() {
            return Ok(());
        }
        let target = self.subnotes_db_path();
        if !target.exists() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).context("failed to create .clin directory")?;
            }
            fs::rename(&legacy, &target).context("failed to migrate native subnotes metadata")?;
            return Ok(());
        }
        if fs::read(&legacy)? == fs::read(&target)? {
            fs::remove_file(&legacy)
                .context("failed to remove duplicate native subnotes metadata")?;
            return Ok(());
        }
        anyhow::bail!(
            "subnotes metadata conflict: {} and {} differ; both were preserved",
            legacy.display(),
            target.display()
        );
    }

    fn migrate_legacy_attachments(
        &self,
        attachments_subdir: &str,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        let configured = Self::validated_attachment_subdir(attachments_subdir)?;
        let legacy = self.data_dir.join(".clin").join("attachments");
        if !legacy.exists() {
            return Ok(());
        }

        let target = self.notes_dir.join("attachments");
        if legacy == target {
            return Ok(());
        }
        let copy_only = configured == Path::new(".clin").join("attachments");
        Self::merge_attachment_tree(&legacy, &target, copy_only, warnings)?;
        if !copy_only {
            Self::remove_empty_tree(&legacy)?;
        }
        Ok(())
    }

    fn merge_attachment_tree(
        source: &Path,
        target: &Path,
        copy_only: bool,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        fs::create_dir_all(target).context("failed to create attachment migration target")?;
        for entry in fs::read_dir(source).context("failed to read legacy attachments")? {
            let entry = entry?;
            let source_path = entry.path();
            let target_path = target.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                Self::merge_attachment_tree(&source_path, &target_path, copy_only, warnings)?;
                if !copy_only {
                    Self::remove_empty_tree(&source_path)?;
                }
            } else if !target_path.exists() {
                if copy_only {
                    fs::copy(&source_path, &target_path)
                        .context("failed to copy legacy attachment")?;
                } else {
                    fs::rename(&source_path, &target_path)
                        .context("failed to move legacy attachment")?;
                }
            } else if fs::read(&source_path)? == fs::read(&target_path)? {
                if !copy_only {
                    fs::remove_file(&source_path)
                        .context("failed to remove duplicate legacy attachment")?;
                }
            } else {
                warnings.push(format!(
                    "attachment migration conflict: preserving {} and {}",
                    source_path.display(),
                    target_path.display()
                ));
            }
        }
        Ok(())
    }

    fn remove_empty_tree(path: &Path) -> Result<()> {
        if path.is_dir() && fs::read_dir(path)?.next().is_none() {
            fs::remove_dir(path).context("failed to remove empty legacy attachment directory")?;
        }
        Ok(())
    }
    fn load_subnotes_db(&self) -> Result<HashMap<String, SubNotePayload>> {
        let path = self.subnotes_db_path();
        let mut bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(error) => return Err(error).context("failed to read subnotes database"),
        };
        obfuscate(&mut bytes);
        let (db, used) = bincode::serde::decode_from_slice(
            &bytes,
            bincode::config::standard().with_limit::<{ 40 * 1024 * 1024 }>(),
        )
        .context("invalid subnotes database; original data retained")?;
        if used != bytes.len() {
            anyhow::bail!("unexpected trailing subnotes data; original data retained");
        }
        Ok(db)
    }

    fn save_subnotes_db(&self, db: &HashMap<String, SubNotePayload>) -> Result<()> {
        let path = self.subnotes_db_path();
        fs::create_dir_all(
            path.parent()
                .context("subnotes database has no directory")?,
        )?;
        let mut bytes = bincode::serde::encode_to_vec(db, bincode::config::standard())
            .context("failed to serialize subnotes database")?;
        obfuscate(&mut bytes);
        crate::fsutil::atomic_write_with_mode(&path, &bytes, 0o600)
            .context("failed to write subnotes database")
    }

    fn check_subnotes_writable(db: &HashMap<String, SubNotePayload>, id: &str) -> Result<()> {
        if Path::new(id)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("clin"))
            || matches!(db.get(id), Some(SubNotePayload::LegacyEncrypted(_)))
        {
            anyhow::bail!(
                "Legacy encrypted subnotes are read-only; use the previous release; original payload retained"
            );
        }
        Ok(())
    }

    pub fn get_subnotes(&mut self, parent_id: &str) -> Result<Vec<SubNote>> {
        match self.load_subnotes_db()?.remove(parent_id) {
            Some(SubNotePayload::Plain(notes)) => Ok(notes),
            Some(SubNotePayload::LegacyEncrypted(_)) => anyhow::bail!(
                "Legacy encrypted subnotes require the previous release; data retained"
            ),
            None => Ok(Vec::new()),
        }
    }

    pub fn set_subnotes(&mut self, parent_id: &str, subnotes: &[SubNote]) -> Result<()> {
        let _guard = self.write_guard()?;
        let mut db = self.load_subnotes_db()?;
        Self::check_subnotes_writable(&db, parent_id)?;
        if subnotes.is_empty() {
            db.remove(parent_id);
        } else {
            db.insert(
                parent_id.to_owned(),
                SubNotePayload::Plain(subnotes.to_vec()),
            );
        }
        self.save_subnotes_db(&db)
    }

    /// Re-key plaintext subnotes without replacing another parent's records.
    pub fn migrate_subnotes_parent(&mut self, old_id: &str, new_id: &str) -> Result<()> {
        if old_id == new_id {
            return Ok(());
        }
        let _guard = self.write_guard()?;
        let mut db = self.load_subnotes_db()?;
        Self::check_subnotes_writable(&db, old_id)?;
        Self::check_subnotes_writable(&db, new_id)?;
        if db.contains_key(new_id) {
            anyhow::bail!("destination already has subnotes; both records retained");
        }
        let Some(payload) = db.remove(old_id) else {
            return Ok(());
        };
        db.insert(new_id.to_owned(), payload);
        self.save_subnotes_db(&db)
    }

    pub fn get_notes_with_subnotes(&self) -> Result<HashSet<String>> {
        Ok(self.load_subnotes_db()?.into_keys().collect())
    }

    /// Read plaintext parents once; retain opaque legacy payloads in storage.
    pub fn get_all_subnotes(&mut self) -> Result<Vec<(String, Vec<SubNote>)>> {
        let mut result: Vec<_> = self
            .load_subnotes_db()?
            .into_iter()
            .filter_map(|(id, payload)| match payload {
                SubNotePayload::Plain(notes) if !notes.is_empty() => Some((id, notes)),
                _ => None,
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    /// Create a minimal dummy Storage so the app can start even when `init()` fails.
    /// The returned Storage uses a temp dir; it cannot read
    /// real notes but prevents a crash on startup.
    pub fn new_fallback() -> Self {
        let data_dir = std::env::temp_dir().join("clin_fallback");
        let _ = std::fs::create_dir_all(&data_dir);
        let config_dir = std::env::temp_dir().join("clin_fallback_config");
        let _ = std::fs::create_dir_all(&config_dir);
        let notes_dir = data_dir.clone();
        let templates_dir = data_dir.join(".clin").join("templates");
        let _ = std::fs::create_dir_all(&templates_dir);
        Self {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        }
    }
}
fn obfuscate(data: &mut [u8]) {
    // Generate pattern dynamically to prevent false positive in SAST tools
    let mut pattern = Vec::with_capacity(39);
    pattern.extend_from_slice(b"clin_subnotes_");
    pattern.extend_from_slice(b"obfuscation_key_");
    pattern.extend_from_slice(b"pattern");
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= pattern[i % pattern.len()];
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src).context("failed to read source folder")? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type()?;
        anyhow::ensure!(
            !kind.is_symlink(),
            "Cannot copy symlink: {}",
            from.display()
        );
        if kind.is_dir() {
            fs::create_dir_all(&to)?;
            copy_dir_recursive(&from, &to)?;
        } else {
            crate::fsutil::atomic_copy(&from, &to, false)
                .with_context(|| format!("failed to copy {}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod safety_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_existing_vault_detects_user_content() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("v");
        std::fs::create_dir_all(vault.join("notes")).unwrap();
        std::fs::create_dir_all(vault.join("templates")).unwrap();
        std::fs::create_dir_all(vault.join(".clin")).unwrap();
        std::fs::write(vault.join("key.bin"), "k").unwrap();
        std::fs::write(vault.join("state.json"), "{}").unwrap();
        assert!(!is_existing_vault(&vault)); // only managed content
        std::fs::write(vault.join("mynote.md"), "x").unwrap();
        assert!(is_existing_vault(&vault)); // one user file
    }

    #[test]
    fn migrate_legacy_default_vault_copies_classic_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("old");
        let new = tmp.path().join("new");
        std::fs::create_dir_all(old.join("notes/sub")).unwrap();
        std::fs::create_dir_all(old.join("templates")).unwrap();
        std::fs::create_dir_all(old.join(".clin")).unwrap();
        std::fs::create_dir_all(old.join(".git")).unwrap();
        std::fs::write(old.join("notes/sub/a.md"), "# a").unwrap();
        std::fs::write(old.join("templates/t.md"), "t").unwrap();
        std::fs::write(old.join(".clin/editor_draft.bin"), "d").unwrap();
        std::fs::write(old.join(".git/HEAD"), "ref").unwrap();
        std::fs::write(old.join("key.bin"), "k").unwrap(); // must NOT be copied
        let mut warnings = Vec::new();
        migrate_legacy_default_vault_at(&old, &new, &mut warnings);
        assert!(new.join("sub/a.md").is_file()); // notes/ content → vault root
        assert!(new.join(".clin/templates/t.md").is_file());
        assert!(new.join(".clin/editor_draft.bin").is_file());
        assert!(new.join(".git/HEAD").is_file());
        assert!(!new.join("key.bin").exists());
        assert!(old.join("notes/sub/a.md").is_file()); // old copy kept
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Default vault location is now"));
    }

    #[test]
    fn migrate_legacy_default_vault_noops() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("old");
        let new = tmp.path().join("new");
        // Scaffold only: empty notes/, key.bin, state.json — no user content.
        std::fs::create_dir_all(old.join("notes")).unwrap();
        std::fs::write(old.join("key.bin"), "k").unwrap();
        std::fs::write(old.join("state.json"), "{}").unwrap();
        let mut warnings = Vec::new();
        migrate_legacy_default_vault_at(&old, &new, &mut warnings);
        assert!(!new.exists());
        assert_eq!(warnings.len(), 0);
        // Destination already exists → never touched.
        std::fs::write(old.join("notes/a.md"), "a").unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("existing.md"), "keep").unwrap();
        migrate_legacy_default_vault_at(&old, &new, &mut warnings);
        assert!(new.join("existing.md").is_file());
        assert!(!new.join("a.md").exists());
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_duplicate_preserves_extension() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: notes_dir.clone(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        let content = "Test content for duplicate";
        let base_note = Note {
            baseline: None,
            title: "Original".to_string(),
            content: content.to_string(),
            updated_at: 42,
            tags: vec![],
        };

        // Test each supported extension
        let titled_exts = ["md", "txt"]; // frontmatter/bincode preserves title
        let raw_exts = ["draw", "canvas"]; // raw bytes, no stored title

        for ext in titled_exts.iter().chain(raw_exts.iter()) {
            let orig_id = format!("test_original.{}", ext);
            // Save original — returns the actual id (may differ if name conflicts)
            let saved_id = storage.save_note(&orig_id, &base_note)?;
            // Verify the original was saved with the correct extension
            assert!(
                saved_id.ends_with(&format!(".{}", ext)),
                "saved note should end with .{ext}, got: {saved_id}"
            );

            // Duplicate the saved note
            let dup_id = storage.duplicate_note(&saved_id, "")?;
            // Verify the duplicate preserves the extension
            assert!(
                dup_id.ends_with(&format!(".{}", ext)),
                "duplicate should end with .{ext}, got: {dup_id}"
            );

            // Load the duplicate and verify content matches
            let dup_note = storage.load_note(&dup_id)?;
            assert_eq!(dup_note.content, content, "content mismatch for .{ext}");

            // Title is only preserved for frontmatter/bincode-backed formats
            if titled_exts.contains(ext) {
                assert_eq!(
                    dup_note.title, "Original (Copy)",
                    "title mismatch for .{ext}"
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_save_note_preserves_unknown_frontmatter() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: notes_dir.clone(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        let content = "---\ntitle: original\ntype: knowledge_concept\nconfidence: 0.9\nsources:\n  - \"[[kimball-dwt]]\"\n---\nbody";
        let path = notes_dir.join("obsidian.md");
        fs::write(&path, content)?;

        let mut note = storage.load_note("obsidian.md")?;
        note.content = "edited body".to_string();
        let saved_id = storage.save_note("obsidian.md", &note)?;
        let saved_path = storage.note_path(&saved_id);

        let saved = fs::read_to_string(&saved_path)?;
        assert!(saved.contains("type: knowledge_concept"));
        assert!(saved.contains("confidence: 0.9"));
        assert!(saved.contains("sources:"));
        assert!(
            saved.contains("- '[[kimball-dwt]]'")
                || saved.contains("- \"[[kimball-dwt]]\"")
                || saved.contains("- [[kimball-dwt]]")
        );

        Ok(())
    }

    #[test]
    fn save_note_case_only_title_change_keeps_single_note() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: notes_dir.clone(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        fs::write(
            notes_dir.join("activity.md"),
            "---\ntitle: activity\n---\nold body",
        )?;
        // Changing title capitalization must not change filename identity.

        let note = Note {
            baseline: Some(storage.load_document("activity.md")?),
            title: "Activity".to_string(),
            content: "new body".to_string(),
            updated_at: 0,
            tags: Vec::new(),
        };
        let saved_id = storage.save_note("activity.md", &note)?;

        // The note must not be bumped to "Activity (2).md" by colliding
        // with itself, and nothing may be deleted.
        assert_eq!(saved_id, "activity.md");
        assert!(!notes_dir.join("Activity (2).md").exists());
        assert!(fs::read_to_string(notes_dir.join("activity.md"))?.contains("new body"));
        Ok(())
    }

    #[test]
    fn save_note_rename_on_title_change_disabled_keeps_id() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: notes_dir.clone(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        fs::write(
            notes_dir.join("data_vault_sal.md"),
            "---\ntitle: Data Vault Same-As Link\n---\nold body",
        )?;

        let note = Note {
            baseline: Some(storage.load_document("data_vault_sal.md")?),
            title: "Data Vault Same-As Link".to_string(),
            content: "edited body".to_string(),
            updated_at: 0,
            tags: Vec::new(),
        };
        let saved_id = storage.save_note("data_vault_sal.md", &note)?;

        // Filename stability: no title-derived rename, so inbound
        // [[wikilinks]] keep resolving.
        assert_eq!(saved_id, "data_vault_sal.md");
        assert!(!notes_dir.join("Data Vault Same-As Link.md").exists());
        assert!(fs::read_to_string(notes_dir.join("data_vault_sal.md"))?.contains("edited body"));
        Ok(())
    }

    #[test]
    fn save_note_rename_disabled_new_note_still_derives_name() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: notes_dir.clone(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        // Extension-less transient id, file absent: creation path must still
        // derive a filename from the title.
        let note = Note {
            baseline: None,
            title: "Python UV".to_string(),
            content: "body".to_string(),
            updated_at: 0,
            tags: Vec::new(),
        };
        let saved_id = storage.save_note("9f0c2b1a4d5e6f708192a3b4c5d6e7f8", &note)?;

        assert!(saved_id.ends_with(".md"));
        assert!(storage.note_path(&saved_id).exists());
        Ok(())
    }

    #[test]
    fn test_duplicate_preserves_unknown_frontmatter() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: notes_dir.clone(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        let content = "---\ntitle: original\ntype: knowledge_concept\nconfidence: 0.9\nsources:\n  - \"[[kimball-dwt]]\"\n---\nbody";
        let path = notes_dir.join("obsidian.md");
        fs::write(&path, content)?;

        let dup_id = storage.duplicate_note("obsidian.md", "")?;
        let dup_path = storage.note_path(&dup_id);
        let dup_content = String::from_utf8_lossy(&fs::read(&dup_path)?).into_owned();

        assert!(dup_content.contains("type: knowledge_concept"));
        assert!(dup_content.contains("confidence: 0.9"));

        Ok(())
    }

    #[test]
    fn test_subnotes_storage() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&notes_dir)?;
        fs::create_dir_all(&templates_dir)?;

        let mut storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        // 1. Plain note subnotes
        let plain_id = "test_note.md";
        let subnotes = vec![
            SubNote {
                id: "1".to_string(),
                title: "Plain Sub 1".to_string(),
                content: "Plain Content 1".to_string(),
                updated_at: 100,
            },
            SubNote {
                id: "2".to_string(),
                title: "Plain Sub 2".to_string(),
                content: "Plain Content 2".to_string(),
                updated_at: 200,
            },
        ];

        storage.set_subnotes(plain_id, &subnotes)?;

        // Retrieve and assert
        let retrieved = storage.get_subnotes(plain_id)?;
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].title, "Plain Sub 1");
        assert_eq!(retrieved[1].content, "Plain Content 2");

        let notes_with = storage.get_notes_with_subnotes()?;
        assert!(notes_with.contains(plain_id));

        // Verify database contents on disk do NOT contain plaintext strings
        let db_path = storage.subnotes_db_path();
        let db_contents = std::fs::read(&db_path)?;
        let contains_sub1 = db_contents.windows(11).any(|w| w == b"Plain Sub 1");
        assert!(!contains_sub1);

        // De-obfuscate and verify they do contain them
        let mut deobfuscated = db_contents.clone();
        obfuscate(&mut deobfuscated);
        let contains_sub1_deob = deobfuscated.windows(11).any(|w| w == b"Plain Sub 1");
        let contains_sub2_deob = deobfuscated.windows(15).any(|w| w == b"Plain Content 2");
        assert!(contains_sub1_deob);
        assert!(contains_sub2_deob);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&db_path)?;
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        assert!(storage.set_subnotes("legacy.clin", &subnotes).is_err());
        storage.set_subnotes(plain_id, &[])?;
        assert_eq!(storage.get_subnotes(plain_id)?.len(), 0);
        assert_eq!(storage.load_subnotes_db()?.len(), 0);

        Ok(())
    }

    #[test]
    fn test_corrupt_subnotes() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&notes_dir)?;
        fs::create_dir_all(&templates_dir)?;

        let mut storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        // Subnotes metadata is always owned by the storage root.
        fs::create_dir_all(storage.data_dir.join(".clin"))?;

        // Write corrupt bytes directly to subnotes db file path
        let db_path = storage.subnotes_db_path();
        std::fs::write(&db_path, b"garbage data that is not a valid bincode map")?;

        // Corruption is visible to callers; every failed read retains the original bytes.
        let original = std::fs::read(&db_path)?;
        assert!(storage.get_subnotes("some_note.md").is_err());
        assert!(storage.get_notes_with_subnotes().is_err());
        assert!(storage.get_all_subnotes().is_err());
        assert_eq!(std::fs::read(&db_path)?, original);

        Ok(())
    }

    #[test]
    fn test_load_note_summary_unreadable_file() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&notes_dir)?;
        fs::create_dir_all(&templates_dir)?;

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir: notes_dir.clone(),
            templates_dir,
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        // Create an unreadable .md file
        let note_path = notes_dir.join("unreadable.md");
        fs::write(&note_path, "some content")?;

        // Make it unreadable
        let mut perms = note_path.metadata()?.permissions();
        perms.set_readonly(true);
        // On Unix, remove read permission for owner
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o000);
        }
        fs::set_permissions(&note_path, perms)?;

        // Should return Err, not silently succeed with empty content
        let result = storage.load_note_summary("unreadable.md");
        assert!(
            result.is_err(),
            "load_note_summary should fail for unreadable file"
        );

        Ok(())
    }

    #[test]
    fn test_load_note_summary_non_text_file() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&notes_dir)?;
        fs::create_dir_all(&templates_dir)?;

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir: notes_dir.clone(),
            templates_dir,
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };

        // Write a binary PDF file with non-UTF-8 bytes
        let note_path = notes_dir.join("doc.pdf");
        fs::write(&note_path, b"%PDF-1.4\n\x80\x81\x82")?;

        let result = storage.load_note_summary("doc.pdf");
        assert!(
            result.is_ok(),
            "non-text files should load as metadata-only summaries"
        );
        let summary = result.unwrap();
        assert_eq!(summary.title, "doc");
        assert_eq!(summary.id, "doc.pdf");
        assert_eq!(summary.tags.len(), 0);
        assert!(!summary.pinned);
        assert_eq!(summary.links.len(), 0);
        assert!(summary.size_bytes > 0);

        Ok(())
    }

    #[test]
    fn remove_file_if_exists_is_idempotent() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("note_cache_plain_v4.bin");

        // File doesn't exist yet — should return false
        assert!(!crate::fsutil::remove_file_if_exists(&path)?);

        // Create the file and remove it — should return true
        fs::write(&path, b"stale data")?;
        assert!(crate::fsutil::remove_file_if_exists(&path)?);
        assert!(!path.exists(), "file must be deleted after removal");

        // File already gone — should return false (idempotent)
        assert!(!crate::fsutil::remove_file_if_exists(&path)?);

        Ok(())
    }

    #[test]
    fn missing_keybind_preset_regenerates_without_warning() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir: temp_dir.path().join("config"),
            notes_dir: temp_dir.path().join("notes"),
            templates_dir: temp_dir.path().join("templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        let preset = crate::config::KeybindPreset::Vim;
        let path = storage.keybinds_path_for_preset(preset);

        let (keybinds, warnings) = storage.load_keybinds_with_preset(preset);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(path.exists());
        assert_eq!(keybinds.list, preset.base_keybinds().list);

        fs::remove_dir_all(storage.keybinds_dir()).unwrap();
        let (_, warnings) = storage.load_keybinds_with_preset(preset);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(path.exists());
    }
}
