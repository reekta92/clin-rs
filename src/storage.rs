use crate::config::ClinConfig;
use rand::RngExt;
const FILE_MAGIC: &[u8; 5] = b"CLIN1";
const NONCE_LEN: usize = 12;
use crate::frontmatter;
use crate::templates::TemplateManager;
use anyhow::{Context, Result, anyhow};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
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
    Encrypted(Vec<u8>), // bincode + chacha20poly1305 ciphertext
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileStamp {
    pub modified_nanos: Option<u128>,
    pub len: u64,
}

pub(crate) struct NoteFileEntry {
    pub id: String,
    pub stamp: FileStamp,
}

pub(crate) struct VaultScan {
    pub files: Vec<NoteFileEntry>,
    pub folders: Vec<String>,
    pub complete: bool,
    pub warnings: Vec<String>,
}

pub fn extract_wikilinks(content: &str) -> Vec<String> {
    let mut links = Vec::new();
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

            if !link_part.is_empty() && !link_part.contains(']') {
                links.push(link_part.trim().to_string());
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

#[derive(Clone, Debug, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct Storage {
    #[zeroize(skip)]
    pub data_dir: PathBuf,
    #[zeroize(skip)]
    pub config_dir: PathBuf,
    #[zeroize(skip)]
    pub notes_dir: PathBuf,
    #[zeroize(skip)]
    pub templates_dir: PathBuf,
    pub key: [u8; 32],
    #[zeroize(skip)]
    pub skip_dir_patterns: Vec<regex::Regex>,
}

pub(crate) fn split_frontmatter_payload(bytes: &[u8]) -> (Option<frontmatter::Frontmatter>, &[u8]) {
    if !bytes.starts_with(b"---\n") && !bytes.starts_with(b"---\r\n") {
        return (None, bytes);
    }

    let end_marker = b"\n---";
    if let Some(end_idx) = bytes[3..]
        .windows(end_marker.len())
        .position(|w| w == end_marker)
    {
        let fm_bytes = &bytes[3..3 + end_idx];
        let remaining_start = 3 + end_idx + end_marker.len();
        let mut content_start = remaining_start;

        if bytes[remaining_start..].starts_with(b"\r\n") {
            content_start += 2;
        } else if bytes[remaining_start..].starts_with(b"\n") {
            content_start += 1;
        }

        if let Ok(fm_str) = std::str::from_utf8(fm_bytes)
            && let Ok(fm) = serde_yaml_ng::from_str::<frontmatter::Frontmatter>(fm_str)
        {
            return (Some(fm), &bytes[content_start..]);
        }
    }

    (None, bytes)
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
            }
            false
        }
        Err(_) => false,
    }
}

use crate::fsutil::remove_file_if_exists;

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
        let result = Self::init_inner(config, &mut warnings);
        (result, warnings)
    }

    fn init_inner(bootstrap: &ClinConfig, warnings: &mut Vec<String>) -> Result<Self> {
        let data_dir = bootstrap
            .effective_storage_path()
            .context("failed to determine storage path")?;

        let config_dir =
            crate::config::clin_config_dir().context("could not determine config directory")?;

        let vault_mode = bootstrap.has_custom_storage_path();

        let notes_dir = if vault_mode {
            data_dir.clone()
        } else {
            data_dir.join("notes")
        };

        let templates_dir = if vault_mode {
            data_dir.join(".clin").join("templates")
        } else {
            data_dir.join("templates")
        };

        if vault_mode {
            fs::create_dir_all(data_dir.join(".clin").join("templates"))
                .context("failed to create .clin/templates directory")?;
        } else {
            fs::create_dir_all(&notes_dir).context("failed to create notes directory")?;
            fs::create_dir_all(&templates_dir).context("failed to create templates directory")?;
        }
        // --- Key migration to AppPaths canonical location ---
        let app_paths = crate::paths::AppPaths::discover(ClinConfig::config_path()?)?;
        let target_key_path = app_paths.key_path(); // <data_local_dir>/key.bin
        let config_root_key = app_paths.config_root_key_path();
        let default_config_root_key = app_paths.default_config_root_key_path();
        let data_root_key = data_dir.join("key.bin");

        // Collect legacy candidates with path dedup, excluding the target
        let mut legacy_set = std::collections::BTreeSet::new();
        for p in [config_root_key, default_config_root_key, data_root_key] {
            if p != target_key_path && p.exists() {
                legacy_set.insert(p);
            }
        }
        let legacy_candidates: Vec<PathBuf> = legacy_set.into_iter().collect();

        let mut key = [0_u8; 32];

        if target_key_path.exists() {
            // Target exists — read, validate, clean up matching legacy sources
            let raw = fs::read(&target_key_path).with_context(|| {
                format!(
                    "failed to read encryption key from {}",
                    target_key_path.display()
                )
            })?;
            if raw.len() != 32 {
                anyhow::bail!("invalid encryption key at {}", target_key_path.display());
            }
            key.copy_from_slice(&raw);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = fs::metadata(&target_key_path) {
                    let mut perms = metadata.permissions();
                    if perms.mode() & 0o777 != 0o400 {
                        perms.set_mode(0o400);
                        if let Err(e) = fs::set_permissions(&target_key_path, perms) {
                            warnings.push(format!(
                                "set_permissions failed for {}: {e}",
                                target_key_path.display()
                            ));
                        }
                    }
                }
            }

            for legacy in &legacy_candidates {
                if let Ok(raw2) = fs::read(legacy) {
                    if raw2 == raw {
                        let _ = remove_file_if_exists(legacy);
                    } else if raw2.len() == 32 {
                        anyhow::bail!(
                            "conflicting encryption key at {} differs from target at {}; resolve manually",
                            legacy.display(),
                            target_key_path.display()
                        );
                    }
                    // Invalid stale sources silently preserved (warning only)
                }
            }
        } else {
            // Target absent — look for unique valid legacy key to migrate
            let valid_legacy: Vec<&PathBuf> = legacy_candidates
                .iter()
                .filter(|p| fs::read(p).map(|r| r.len() == 32).unwrap_or(false))
                .collect();

            match valid_legacy.len() {
                0 => { /* handled by ensure_key after init */ }
                1 => {
                    let raw = fs::read(valid_legacy[0])
                        .context("failed to read legacy encryption key")?;
                    key.copy_from_slice(&raw);

                    // Write to target atomically with mode 0o400
                    if let Some(parent) = target_key_path.parent() {
                        fs::create_dir_all(parent).context("failed to create key directory")?;
                    }
                    #[cfg(unix)]
                    {
                        crate::fsutil::atomic_write_with_mode(&target_key_path, &raw, 0o400)
                            .context("failed to write encryption key to canonical location")?;
                    }
                    #[cfg(not(unix))]
                    {
                        crate::fsutil::atomic_write(&target_key_path, &raw)
                            .context("failed to write encryption key to canonical location")?;
                    }

                    // Byte-verify
                    let verify = fs::read(&target_key_path)
                        .context("failed to verify written encryption key")?;
                    if verify != raw {
                        anyhow::bail!(
                            "key verification failed after write to {}",
                            target_key_path.display()
                        );
                    }

                    let _ = remove_file_if_exists(valid_legacy[0]);
                }
                _ => {
                    // Multiple valid legacy sources — require identical content
                    let first = fs::read(valid_legacy[0])
                        .context("failed to read legacy encryption key")?;
                    let all_identical = valid_legacy
                        .iter()
                        .all(|p| fs::read(p).map(|r| r == first).unwrap_or(false));

                    if all_identical {
                        key.copy_from_slice(&first);

                        if let Some(parent) = target_key_path.parent() {
                            fs::create_dir_all(parent).context("failed to create key directory")?;
                        }
                        #[cfg(unix)]
                        {
                            crate::fsutil::atomic_write_with_mode(&target_key_path, &first, 0o400)
                                .context("failed to write encryption key")?;
                        }
                        #[cfg(not(unix))]
                        {
                            crate::fsutil::atomic_write(&target_key_path, &first)
                                .context("failed to write encryption key")?;
                        }

                        let verify = fs::read(&target_key_path)
                            .context("failed to verify written encryption key")?;
                        if verify != first {
                            anyhow::bail!("key verification failed after write");
                        }

                        for p in &valid_legacy {
                            let _ = remove_file_if_exists(p);
                        }
                    } else {
                        anyhow::bail!(
                            "multiple different encryption keys found at {}; resolve manually",
                            valid_legacy
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
            }
        }

        let skip_dir_patterns: Vec<regex::Regex> = bootstrap
            .list
            .skip_dirs
            .iter()
            .filter_map(|pat| regex::Regex::new(pat).ok())
            .collect();
        let mut storage = Self {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key,
            skip_dir_patterns,
        };
        storage.migrate_native_subnotes_metadata()?;
        storage.migrate_legacy_attachments(&bootstrap.image.attachments_subdir, warnings)?;
        if !vault_mode {
            storage.migrate_extensions();
        }
        storage.ensure_key()?;
        Ok(storage)
    }
    /// The canonical encryption-key path under `AppPaths::data_local_dir`.
    fn key_path(&self) -> PathBuf {
        if let Ok(config_path) = ClinConfig::config_path()
            && let Ok(paths) = crate::paths::AppPaths::discover(config_path)
        {
            return paths.key_path();
        }
        self.config_dir.join("key.bin")
    }

    pub fn ensure_key(&mut self) -> Result<()> {
        if self.key != [0_u8; 32] {
            return Ok(());
        }

        let key_path = self.key_path();
        if key_path.exists() {
            let raw = fs::read(&key_path).context("failed to read encryption key")?;
            if raw.len() != 32 {
                anyhow::bail!("invalid key file length");
            }
            self.key.copy_from_slice(&raw);
            return Ok(());
        }

        rand::rng().fill(&mut self.key);
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent).context("failed to create key directory")?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o400)
                .open(&key_path)
                .context("failed to create encryption key file")?;
            use std::io::Write;
            file.write_all(&self.key)
                .context("failed to write encryption key")?;
        }

        #[cfg(not(unix))]
        {
            crate::fsutil::atomic_write(&key_path, &self.key)
                .context("failed to write encryption key")?;
        }

        Ok(())
    }

    pub fn encrypt_note(&mut self, id: &str) -> Result<String> {
        if id.ends_with(".clin") {
            anyhow::bail!("Note is already encrypted");
        }
        let ext = std::path::Path::new(id)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if crate::storage::is_image_ext(ext) {
            anyhow::bail!("Cannot encrypt image files");
        }

        self.ensure_key()?;

        let note = self.load_note(id)?;
        let old_path = self.note_path(id);

        let folder = if let Some(idx) = id.rfind('/') {
            &id[..idx]
        } else {
            ""
        };

        let stem = old_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled note");
        let clin_id = if folder.is_empty() {
            format!("{stem}.clin")
        } else {
            format!("{folder}/{stem}.clin")
        };
        let target_id = self.unique_note_id(stem, "clin", &clin_id);
        let target_path = self.note_path(&target_id);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).context("failed to create note directory")?;
        }

        let original_ext = old_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_string());
        let existing_pinned = self
            .load_note_summary(id)
            .map(|s| s.pinned)
            .unwrap_or(false);
        let fm = frontmatter::Frontmatter {
            title: Some(note.title.clone()),
            updated_at: Some(note.updated_at),
            tags: note.tags.clone(),
            pinned: existing_pinned,
            links: Some(extract_wikilinks(&note.content)),
            original_ext,
        };
        let bytes = bincode::serde::encode_to_vec(&note, bincode::config::standard())
            .context("failed to encode note")?;
        let encrypted = self.encrypt(&bytes)?;
        let fm_string = frontmatter::serialize(&fm, "");
        let mut final_output = fm_string.into_bytes();
        final_output.extend_from_slice(&encrypted);

        crate::fsutil::atomic_write(&target_path, &final_output)
            .context("failed to write encrypted note")?;

        if old_path.exists() {
            fs::remove_file(&old_path).context("failed to remove plain note after encryption")?;
        }

        Ok(target_id)
    }

    pub fn decrypt_note(&mut self, id: &str) -> Result<String> {
        if !id.ends_with(".clin") {
            anyhow::bail!("Note is not encrypted");
        }

        self.ensure_key()?;

        let old_path = self.note_path(id);
        let clin_content = fs::read(&old_path).context("failed to read encrypted note")?;
        let (fm_opt, _) = split_frontmatter_payload(&clin_content);
        let orig_ext = fm_opt
            .and_then(|fm| fm.original_ext)
            .unwrap_or_else(|| "md".to_string());

        let note = self.load_note(id)?;

        let folder = if let Some(idx) = id.rfind('/') {
            &id[..idx]
        } else {
            ""
        };

        let stem = old_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled note");
        let target_id = if folder.is_empty() {
            format!("{stem}.{orig_ext}")
        } else {
            format!("{folder}/{stem}.{orig_ext}")
        };
        let target_id = self.unique_note_id(stem, &orig_ext, &target_id);
        let target_path = self.note_path(&target_id);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).context("failed to create note directory")?;
        }
        let existing_pinned = self
            .load_note_summary(id)
            .map(|s| s.pinned)
            .unwrap_or(false);

        let is_raw = orig_ext == "canvas" || orig_ext == "draw";
        if is_raw {
            crate::fsutil::atomic_write(&target_path, note.content.as_bytes())
                .context("failed to write decrypted note")?;
        } else {
            let fm = frontmatter::Frontmatter {
                title: Some(note.title.clone()),
                updated_at: Some(note.updated_at),
                tags: note.tags.clone(),
                pinned: existing_pinned,
                links: Some(extract_wikilinks(&note.content)),
                original_ext: None,
            };
            let final_content = frontmatter::serialize(&fm, &note.content);
            crate::fsutil::atomic_write(&target_path, final_content.as_bytes())
                .context("failed to write decrypted note")?;
        }

        if old_path.exists() {
            fs::remove_file(&old_path)
                .context("failed to remove encrypted note after decryption")?;
        }

        Ok(target_id)
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

    pub fn template_manager(&self) -> TemplateManager {
        TemplateManager::new(self.templates_dir.clone())
    }

    pub fn note_path(&self, id: &str) -> PathBuf {
        self.validate_path_within_notes_dir(id)
            .unwrap_or_else(|| self.notes_dir.join("invalid"))
    }

    pub fn note_mtime_millis(&self, id: &str) -> u64 {
        fs::metadata(self.note_path(id))
            .and_then(|m| m.modified())
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map_err(std::io::Error::other)
            })
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn attachments_dir(&self, attachments_subdir: &str) -> Result<PathBuf> {
        let relative = Self::validated_attachment_subdir(attachments_subdir)?;
        Ok(self.notes_dir.join(relative))
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
        let mut ids = Vec::new();
        let mut dirs_to_visit = vec![self.notes_dir.clone()];

        while let Some(dir) = dirs_to_visit.pop() {
            for entry in fs::read_dir(&dir).context("failed reading directory")? {
                let entry = entry.context("failed to read entry")?;
                let path = entry.path();

                if path.is_dir()
                    && path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| include_hidden || !n.starts_with('.'))
                {
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if self.skip_dir_patterns.iter().any(|re| re.is_match(name)) {
                        continue;
                    }
                    dirs_to_visit.push(path);
                } else {
                    let accepted = if include_all_files {
                        path.is_file()
                    } else {
                        path.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|ext| {
                                matches!(ext, "clin" | "md" | "txt" | "draw" | "canvas")
                                    || crate::storage::is_image_ext(ext)
                            })
                    };
                    if accepted
                        && let Ok(rel_path) = path.strip_prefix(&self.notes_dir)
                        && let Some(rel_str) = rel_path.to_str()
                    {
                        ids.push(rel_str.to_string());
                    }
                }
            }
        }
        Ok(ids)
    }

    fn migrate_extensions(&self) {
        let mut dirs_to_visit = vec![self.notes_dir.clone()];

        while let Some(dir) = dirs_to_visit.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();

                    if path.is_dir() {
                        dirs_to_visit.push(path);
                    } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        match ext {
                            "pinstar" => {
                                let new_path = path.with_extension("canvas");
                                if !new_path.exists() {
                                    let _ = fs::rename(&path, &new_path);
                                }
                            }
                            "canvas" => {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    let trimmed = content.trim();
                                    let is_draw_format = trimmed.starts_with(
                                        "{\
  \"elements\"",
                                    ) || trimmed.starts_with("{\"elements\"");
                                    let is_new_draw = trimmed.contains("\"version\"");
                                    if is_draw_format || is_new_draw {
                                        let new_path = path.with_extension("draw");
                                        if !new_path.exists() {
                                            let _ = fs::rename(&path, &new_path);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn scan_vault(
        &self,
        include_hidden: bool,
        include_all_files: bool,
    ) -> Result<VaultScan> {
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

                if !include_hidden && file_name.starts_with('.') {
                    continue;
                }

                let rel_path = match path.strip_prefix(&self.notes_dir) {
                    Ok(r) => match r.to_str() {
                        Some(s) => s.replace('\\', "/"),
                        None => continue,
                    },
                    Err(_) => continue,
                };

                if path.is_dir() {
                    if self
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
                                matches!(ext, "clin" | "md" | "txt" | "draw" | "canvas")
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
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let folder = if let Some(parent) = std::path::Path::new(id).parent() {
            parent.to_str().unwrap_or("").to_string()
        } else {
            String::new()
        };

        if ext == "clin" {
            let file_content = fs::read(&path).context("failed to read note")?;
            let (fm, payload) = split_frontmatter_payload(&file_content);

            if let Some(ref fm_val) = fm
                && let (Some(title), Some(updated_at)) = (fm_val.title.clone(), fm_val.updated_at)
            {
                return Ok(NoteSummary {
                    id: id.to_string(),
                    title,
                    updated_at,
                    folder,
                    tags: fm_val.tags.clone(),
                    pinned: fm_val.pinned,
                    links: fm_val.links.clone().unwrap_or_default(),
                    size_bytes: path.metadata().map(|m| m.len()).unwrap_or(0),
                });
            }

            let plain = self.decrypt(payload)?;
            let (note, _): (Note, usize) =
                bincode::serde::decode_from_slice(plain.as_slice(), bincode::config::standard())
                    .context("failed to decode note")?;

            let (tags, pinned, links) = fm
                .map(|f| (f.tags, f.pinned, f.links.unwrap_or_default()))
                .unwrap_or_else(|| (note.tags.clone(), false, extract_wikilinks(&note.content)));

            Ok(NoteSummary {
                id: id.to_string(),
                title: note.title,
                updated_at: note.updated_at,
                folder,
                tags,
                pinned,
                links,
                size_bytes: path.metadata().map(|m| m.len()).unwrap_or(0),
            })
        } else if crate::storage::is_image_ext(ext) {
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

        if ext == "clin" {
            let file_content = fs::read(&path).context("failed to read note")?;
            let (fm, payload) = split_frontmatter_payload(&file_content);

            let plain = self.decrypt(payload)?;
            let (mut note, _) = bincode::serde::decode_from_slice::<Note, _>(
                plain.as_slice(),
                bincode::config::standard(),
            )
            .context("failed to decode note")?;

            if let Some(fm) = fm {
                note.tags = fm.tags;
                if let Some(t) = fm.title {
                    note.title = t;
                }
                if let Some(ua) = fm.updated_at {
                    note.updated_at = ua;
                }
            }
            Ok(note)
        } else {
            let file_content = fs::read_to_string(&path).context("failed to read plain note")?;
            let (fm, plain_content) = frontmatter::parse(&file_content);

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
                title,
                content: plain_content.to_string(),
                updated_at,
                tags: fm.tags,
            })
        }
    }
    pub fn editor_draft_path(&self) -> PathBuf {
        self.data_dir.join(".clin").join("editor_draft.bin")
    }

    pub fn write_editor_draft(&mut self, id: &str, title: &str, content: &str) -> Result<()> {
        self.ensure_key()?;
        let draft = (id.to_string(), title.to_string(), content.to_string());
        let bytes = bincode::serde::encode_to_vec(&draft, bincode::config::standard())
            .context("failed to encode draft")?;
        let encrypted = self.encrypt(&bytes)?;
        let path = self.editor_draft_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create .clin directory")?;
        }
        crate::fsutil::atomic_write(&path, &encrypted).context("failed to write editor draft")?;
        Ok(())
    }

    pub fn delete_editor_draft(&self) {
        let _ = fs::remove_file(self.editor_draft_path());
    }

    pub fn recover_editor_draft(&mut self) -> Result<()> {
        let path = self.editor_draft_path();
        if !path.exists() {
            return Ok(());
        }
        let encrypted = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return Ok(()),
        };
        self.ensure_key()?;
        if let Ok(decrypted) = self.decrypt(&encrypted)
            && let Ok((draft, _)) = bincode::serde::decode_from_slice::<(String, String, String), _>(
                &decrypted,
                bincode::config::standard(),
            )
        {
            let mut note = self.load_note(&draft.0).unwrap_or_else(|_| Note {
                title: draft.1.clone(),
                content: String::new(),
                updated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                tags: vec![],
            });
            if note.title != draft.1 || note.content != draft.2 {
                note.title = draft.1;
                note.content = draft.2;
                note.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let _ = self.save_note(&draft.0, &note);
            }
        }
        self.delete_editor_draft();
        Ok(())
    }

    pub fn save_note(&mut self, id: &str, note: &Note) -> Result<String> {
        let preferred_stem = self.note_file_stem_from_title(&note.title);

        let old_path = self.note_path(id);
        let old_ext = old_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let target_ext = if old_ext == "clin"
            || old_ext == "txt"
            || old_ext == "md"
            || old_ext == "canvas"
            || old_ext == "draw"
        {
            old_ext
        } else {
            "md"
        };

        let target_id = self.unique_note_id(&preferred_stem, target_ext, id);
        let existing_pinned = self
            .load_note_summary(id)
            .map(|s| s.pinned)
            .unwrap_or(false);
        let links = extract_wikilinks(&note.content);
        let fm = frontmatter::Frontmatter {
            title: Some(note.title.clone()),
            updated_at: Some(note.updated_at),
            tags: note.tags.clone(),
            pinned: existing_pinned,
            links: Some(links),
            original_ext: None,
        };

        let target_path = self.note_path(&target_id);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).context("failed to create note directory")?;
        }

        if target_ext == "clin" {
            let bytes = bincode::serde::encode_to_vec(note, bincode::config::standard())
                .context("failed to encode note")?;
            let encrypted = self.encrypt(&bytes)?;

            let fm_string = frontmatter::serialize(&fm, "");
            let mut final_output = fm_string.into_bytes();
            final_output.extend_from_slice(&encrypted);

            crate::fsutil::atomic_write(&target_path, &final_output)
                .context("failed to write note")?;
        } else if target_ext == "canvas" || target_ext == "draw" {
            crate::fsutil::atomic_write(&target_path, note.content.as_bytes())
                .context("failed to write note")?;
        } else {
            let final_content = frontmatter::serialize(&fm, &note.content);
            crate::fsutil::atomic_write(&target_path, final_content.as_bytes())
                .context("failed to write plain note")?;
        }

        if id != target_id {
            let old_path_to_remove = self.note_path(id);
            if old_path_to_remove.exists() {
                fs::remove_file(&old_path_to_remove).context("failed to rename note file")?;
            }
            // Keep subnotes DB key in sync with the note's new id.
            let _ = self.migrate_subnotes_parent(id, &target_id);
        }

        Ok(target_id)
    }

    pub fn rename_note(&mut self, id: &str, new_title: &str) -> Result<String> {
        let old_ext = std::path::Path::new(id)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if crate::storage::is_image_ext(old_ext) {
            let preferred_stem = self.note_file_stem_from_title(new_title);
            let target_id = self.unique_note_id(&preferred_stem, old_ext, id);
            let old_path = self.note_path(id);
            let target_path = self.note_path(&target_id);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).context("failed to create note directory")?;
            }
            fs::rename(&old_path, &target_path).context("failed to rename image")?;
            return Ok(target_id);
        }

        let mut note = self.load_note(id)?;
        note.title = new_title.to_string();
        note.updated_at = crate::ui::now_unix_secs();

        self.save_note(id, &note)
    }

    pub fn duplicate_note(&mut self, id: &str, target_folder: &str) -> Result<String> {
        let source_ext = Path::new(id)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("md");

        if crate::storage::is_image_ext(source_ext) {
            let new_id = self.new_note_id();
            let initial_id = if target_folder.is_empty() {
                format!("{}.{}", new_id, source_ext)
            } else {
                format!("{}/{}.{}", target_folder, new_id, source_ext)
            };
            let source_path = self.note_path(id);
            let target_path = self.note_path(&initial_id);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).context("failed to create note directory")?;
            }
            fs::copy(&source_path, &target_path).context("failed to copy image")?;
            return Ok(initial_id);
        }

        let note = self.load_note(id)?;
        let new_title = format!("{} (Copy)", note.title);
        let mut new_note = note;
        new_note.title = new_title;
        new_note.updated_at = crate::ui::now_unix_secs();

        let new_id = self.new_note_id();

        let initial_id = if target_folder.is_empty() {
            format!("{}.{}", new_id, source_ext)
        } else {
            format!("{}/{}.{}", target_folder, new_id, source_ext)
        };

        self.save_note(&initial_id, &new_note)
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
    pub fn list_trash(&self) -> Result<Vec<trash::TrashItem>> {
        let items =
            trash::os_limited::list().map_err(|e| anyhow::anyhow!("failed to list trash: {e}"))?;
        let vault_items: Vec<trash::TrashItem> = items
            .into_iter()
            .filter(|item| item.original_parent.starts_with(&self.notes_dir))
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
    pub fn list_trash(&self) -> Result<Vec<trash::TrashItem>> {
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
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if crate::storage::is_image_ext(ext) {
            anyhow::bail!("Cannot pin image files");
        }

        if ext == "clin" {
            let file_content = fs::read(&path).context("failed to read note")?;
            let (fm_opt, payload) = split_frontmatter_payload(&file_content);
            let mut fm = fm_opt.unwrap_or_default();
            fm.pinned = !fm.pinned;
            let new_pinned = fm.pinned;

            let plain = self.decrypt(payload)?;
            let fm_string = frontmatter::serialize(&fm, "");
            let mut final_output = fm_string.into_bytes();

            let encrypted = self.encrypt(plain.as_slice())?;
            final_output.extend_from_slice(&encrypted);

            crate::fsutil::atomic_write(&path, &final_output).context("failed to write note")?;
            Ok(new_pinned)
        } else {
            let content = fs::read_to_string(&path).context("failed to read note")?;
            let (mut fm, body) = frontmatter::parse(&content);
            fm.pinned = !fm.pinned;
            let new_pinned = fm.pinned;

            let new_content = frontmatter::serialize(&fm, body);
            crate::fsutil::atomic_write(&path, new_content.as_bytes())
                .context("failed to write note")?;
            Ok(new_pinned)
        }
    }

    pub fn new_note_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn create_folder(&self, path: &str) -> Result<()> {
        let full_path = self
            .validate_path_within_notes_dir(path)
            .ok_or_else(|| anyhow::anyhow!("Invalid folder path"))?;
        fs::create_dir_all(full_path).context("failed to create folder")
    }

    pub fn trash_folder(&self, path: &str) -> Result<()> {
        let full_path = self
            .validate_path_within_notes_dir(path)
            .ok_or_else(|| anyhow::anyhow!("Invalid folder path"))?;
        if !full_path.exists() {
            anyhow::bail!("Folder does not exist");
        }
        trash::delete(&full_path).context("failed to move folder to trash")?;
        Ok(())
    }

    pub fn rename_folder(&self, old_path: &str, new_path: &str) -> Result<()> {
        let old_full = self
            .validate_path_within_notes_dir(old_path)
            .ok_or_else(|| anyhow::anyhow!("Invalid source folder path"))?;
        let new_full = self
            .validate_path_within_notes_dir(new_path)
            .ok_or_else(|| anyhow::anyhow!("Invalid target folder path"))?;

        if !old_full.exists() {
            anyhow::bail!("Folder does not exist");
        }
        if new_full.exists() {
            anyhow::bail!("Target folder already exists");
        }
        if let Some(parent) = new_full.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::rename(old_full, new_full).context("failed to rename folder")
    }

    /// Recursively copy folder `src_rel` (relative to notes dir) into `target_folder`
    /// (relative, "" = vault root). On name conflict at target, append " (Copy)" then
    /// " (Copy 2)", etc. — never overwrites. Bails if `src_rel` is empty or if the
    /// resolved destination sits inside `src_rel`'s own subtree (would recurse forever).
    pub fn duplicate_folder(&self, src_rel: &str, target_folder: &str) -> Result<()> {
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

        let src_full = self
            .validate_path_within_notes_dir(src_rel)
            .ok_or_else(|| anyhow::anyhow!("Invalid source folder path"))?;
        let new_full = self
            .validate_path_within_notes_dir(&new_rel)
            .ok_or_else(|| anyhow::anyhow!("Invalid target folder path"))?;
        if !src_full.exists() {
            anyhow::bail!("Folder does not exist");
        }
        fs::create_dir_all(&new_full)?;
        copy_dir_recursive(&src_full, &new_full)?;
        Ok(())
    }

    pub fn move_note(&mut self, id: &str, new_folder: &str) -> Result<String> {
        let old_path = self.note_path(id);
        if !old_path.exists() {
            anyhow::bail!("Note does not exist");
        }

        let file_name = old_path
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");
        let target_id = if new_folder.is_empty() {
            file_name.to_string()
        } else {
            format!("{new_folder}/{file_name}")
        };

        if id == target_id {
            return Ok(id.to_string());
        }

        let new_path = self.note_path(&target_id);
        if new_path.exists() {
            anyhow::bail!("Note with this name already exists in target folder");
        }

        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::rename(&old_path, &new_path).context("failed to move note")?;
        let _ = self.migrate_subnotes_parent(id, &target_id);
        Ok(target_id)
    }

    pub fn list_folders(&self, include_hidden: bool) -> Result<Vec<String>> {
        let mut folders = Vec::new();
        let mut dirs_to_visit = vec![self.notes_dir.clone()];

        while let Some(dir) = dirs_to_visit.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir()
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
            let valid = ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.');
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

        while candidate != current_id && self.note_path(&candidate).exists() {
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

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let mut nonce = [0_u8; NONCE_LEN];
        rand::rng().fill(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| anyhow!("note encryption failed"))?;

        let mut output = Vec::with_capacity(FILE_MAGIC.len() + NONCE_LEN + ciphertext.len());
        output.extend_from_slice(FILE_MAGIC);
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    pub fn decrypt(&self, payload: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let header_len = FILE_MAGIC.len() + NONCE_LEN;
        if payload.len() < header_len {
            anyhow::bail!("invalid note header, payload too short");
        }
        if !payload.starts_with(FILE_MAGIC) {
            anyhow::bail!("invalid note header, missing CLIN");
        }

        let nonce = &payload[FILE_MAGIC.len()..header_len];
        let ciphertext = &payload[header_len..];

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("note decryption failed"))?;
        Ok(Zeroizing::new(plaintext))
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
    pub fn get_subnotes(&mut self, parent_id: &str) -> Result<Vec<SubNote>> {
        let path = self.subnotes_db_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut bytes = fs::read(&path).context("failed to read subnotes database")?;
        obfuscate(&mut bytes);
        let db: HashMap<String, SubNotePayload> =
            match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
                Ok((map, _)) => map,
                Err(_) => HashMap::new(),
            };
        if let Some(payload) = db.get(parent_id) {
            match payload {
                SubNotePayload::Plain(notes) => Ok(notes.clone()),
                SubNotePayload::Encrypted(bytes) => {
                    self.ensure_key()?;
                    let plain = self
                        .decrypt(bytes)
                        .context("failed to decrypt subnotes payload")?;
                    let (notes, _): (Vec<SubNote>, usize) = bincode::serde::decode_from_slice(
                        plain.as_slice(),
                        bincode::config::standard(),
                    )
                    .context("failed to decode encrypted subnotes")?;
                    Ok(notes)
                }
            }
        } else {
            Ok(Vec::new())
        }
    }

    pub fn set_subnotes(&mut self, parent_id: &str, subnotes: &[SubNote]) -> Result<()> {
        let path = self.subnotes_db_path();
        let mut db: HashMap<String, SubNotePayload> = if path.exists() {
            let mut bytes = fs::read(&path).context("failed to read subnotes database")?;
            obfuscate(&mut bytes);
            match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
                Ok((map, _)) => map,
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };

        if subnotes.is_empty() {
            db.remove(parent_id);
        } else if parent_id.ends_with(".clin") {
            let bytes = bincode::serde::encode_to_vec(subnotes, bincode::config::standard())
                .context("failed to encode subnotes")?;
            self.ensure_key()?;
            let encrypted = self.encrypt(&bytes)?;
            db.insert(parent_id.to_string(), SubNotePayload::Encrypted(encrypted));
        } else {
            db.insert(
                parent_id.to_string(),
                SubNotePayload::Plain(subnotes.to_vec()),
            );
        }

        if db.is_empty() {
            if path.exists() {
                let _ = fs::remove_file(&path);
            }
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).context("failed to create subnotes parent directory")?;
            }
            let mut bytes = bincode::serde::encode_to_vec(&db, bincode::config::standard())
                .context("failed to serialize subnotes database")?;
            obfuscate(&mut bytes);
            #[cfg(unix)]
            {
                crate::fsutil::atomic_write_with_mode(&path, &bytes, 0o600)
                    .context("failed to write subnotes database")?;
            }
            #[cfg(not(unix))]
            {
                crate::fsutil::atomic_write(&path, &bytes)
                    .context("failed to write subnotes database")?;
            }
        }
        Ok(())
    }

    /// Re-key the subnotes DB when a parent note's id changes (save with new
    /// title stem / rename / move). No-op when `old_id == new_id`, when the DB
    /// file is absent, when no entry exists under `old_id`, or when the DB fails
    /// to decode (corrupt DBs are left intact, never destroyed).
    pub fn migrate_subnotes_parent(&mut self, old_id: &str, new_id: &str) -> Result<()> {
        if old_id == new_id {
            return Ok(());
        }
        let path = self.subnotes_db_path();
        if !path.exists() {
            return Ok(());
        }
        let mut bytes = fs::read(&path).context("failed to read subnotes database")?;
        obfuscate(&mut bytes);
        let mut db: HashMap<String, SubNotePayload> =
            match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
                Ok((map, _)) => map,
                Err(_) => return Ok(()),
            };
        let Some(payload) = db.remove(old_id) else {
            return Ok(());
        };
        db.insert(new_id.to_string(), payload);
        let mut out = bincode::serde::encode_to_vec(&db, bincode::config::standard())
            .context("failed to serialize subnotes database")?;
        obfuscate(&mut out);
        #[cfg(unix)]
        {
            crate::fsutil::atomic_write_with_mode(&path, &out, 0o600)
                .context("failed to write subnotes database")?;
        }
        #[cfg(not(unix))]
        {
            crate::fsutil::atomic_write(&path, &out)
                .context("failed to write subnotes database")?;
        }
        Ok(())
    }

    pub fn get_notes_with_subnotes(&self) -> Result<HashSet<String>> {
        let path = self.subnotes_db_path();
        if !path.exists() {
            return Ok(HashSet::new());
        }
        let mut bytes = fs::read(&path).context("failed to read subnotes database")?;
        obfuscate(&mut bytes);
        let db: HashMap<String, SubNotePayload> =
            match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
                Ok((map, _)) => map,
                Err(_) => HashMap::new(),
            };
        Ok(db.keys().cloned().collect())
    }

    /// Returns (parent_id, Vec<SubNote>) for every parent that has subnotes.
    /// Reads the subnotes DB once, decrypts per-parent payloads as needed.
    pub fn get_all_subnotes(&mut self) -> Result<Vec<(String, Vec<SubNote>)>> {
        let path = self.subnotes_db_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut bytes = fs::read(&path).context("failed to read subnotes database")?;
        obfuscate(&mut bytes);
        let db: HashMap<String, SubNotePayload> =
            match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
                Ok((map, _)) => map,
                Err(_) => HashMap::new(),
            };
        let mut result: Vec<(String, Vec<SubNote>)> = Vec::new();
        // Deterministic ordering: sort parent ids
        let mut parent_ids: Vec<&String> = db.keys().collect();
        parent_ids.sort();
        for parent_id in parent_ids {
            match self.get_subnotes(parent_id) {
                Ok(subs) => {
                    if !subs.is_empty() {
                        result.push((parent_id.clone(), subs));
                    }
                }
                Err(_e) => {
                    // Skip parents whose subnotes fail to decrypt (stale key, etc.).
                }
            }
        }
        Ok(result)
    }

    /// Create a minimal dummy Storage so the app can start even when `init()` fails.
    /// The returned Storage uses a temp dir and a zeroed key; it cannot read
    /// real notes but prevents a crash on startup.
    pub fn new_fallback() -> Self {
        let data_dir = std::env::temp_dir().join("clin_fallback");
        let _ = std::fs::create_dir_all(&data_dir);
        let config_dir = std::env::temp_dir().join("clin_fallback_config");
        let _ = std::fs::create_dir_all(&config_dir);
        let notes_dir = data_dir.join("notes");
        let _ = std::fs::create_dir_all(&notes_dir);
        let templates_dir = data_dir.join("templates");
        let _ = std::fs::create_dir_all(&templates_dir);
        Self {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        }
    }
}
fn obfuscate(data: &mut [u8]) {
    let pattern = b"clin_subnotes_obfuscation_key_pattern";
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= pattern[i % pattern.len()];
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src).context("failed to read source folder")? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            fs::create_dir_all(&to)?;
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to).with_context(|| format!("failed to copy {}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter_payload() {
        let content = b"---\ntitle: Hello\n---\nPayload data";
        let (fm, payload) = split_frontmatter_payload(content);
        assert!(fm.is_some());
        assert_eq!(fm.unwrap().title.unwrap(), "Hello");
        assert_eq!(payload, b"Payload data");

        let no_fm = b"Just payload";
        let (fm, payload) = split_frontmatter_payload(no_fm);
        assert!(fm.is_none());
        assert_eq!(payload, b"Just payload");

        let magic_in_fm = b"---\ntitle: CLIN1 magic\n---\nReal payload";
        let (fm, payload) = split_frontmatter_payload(magic_in_fm);
        assert!(fm.is_some());
        assert_eq!(fm.unwrap().title.unwrap(), "CLIN1 magic");
        assert_eq!(payload, b"Real payload");
    }

    #[test]
    fn test_decrypt_logic() -> Result<()> {
        let key = [1u8; 32];
        let storage = Storage {
            data_dir: PathBuf::new(),
            config_dir: PathBuf::new(),
            notes_dir: PathBuf::new(),
            templates_dir: PathBuf::new(),
            key,
            skip_dir_patterns: Vec::new(),
        };

        let plaintext = b"Secret Message";
        let encrypted = storage.encrypt(plaintext)?;
        let decrypted = storage.decrypt(&encrypted)?;
        assert_eq!(decrypted.as_slice(), &plaintext[..]);

        // Test with frontmatter
        let mut file_content = b"---\ntitle: CLIN1 in title\n---\n".to_vec();
        file_content.extend_from_slice(&encrypted);

        let (fm, payload) = split_frontmatter_payload(&file_content);
        assert!(fm.is_some());
        let decrypted = storage.decrypt(payload)?;
        assert_eq!(decrypted.as_slice(), &plaintext[..]);

        Ok(())
    }

    #[test]
    fn test_decrypt_truncated_payload() {
        let storage = Storage {
            data_dir: PathBuf::new(),
            config_dir: PathBuf::new(),
            notes_dir: PathBuf::new(),
            templates_dir: PathBuf::new(),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        // Truncated payload: valid magic but no nonce/ciphertext
        let truncated = b"CLIN1";
        let result = storage.decrypt(truncated);
        assert!(result.is_err(), "truncated payload must error, not panic");
    }

    #[test]
    fn test_mtime_updates_on_save() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: PathBuf::new(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };

        let id = storage.save_note(
            "test_note.clin",
            &Note {
                title: "T1".to_string(),
                content: "Content 1".to_string(),
                updated_at: 1,
                tags: vec![],
            },
        )?;
        let mt1 = storage.note_mtime_millis(&id);
        assert!(mt1 > 0);

        std::thread::sleep(std::time::Duration::from_millis(20));

        let id = storage.save_note(
            &id,
            &Note {
                title: "T1".to_string(),
                content: "Content 2".to_string(),
                updated_at: 2,
                tags: vec![],
            },
        )?;
        let mt2 = storage.note_mtime_millis(&id);
        assert!(mt2 > mt1);

        Ok(())
    }

    #[test]
    fn test_duplicate_preserves_extension() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let notes_dir = temp.path().to_path_buf();
        let mut storage = Storage {
            data_dir: PathBuf::new(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };

        let content = "Test content for duplicate";
        let base_note = Note {
            title: "Original".to_string(),
            content: content.to_string(),
            updated_at: 42,
            tags: vec![],
        };

        // Test each supported extension
        let titled_exts = ["md", "txt", "clin"]; // frontmatter/bincode preserves title
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
            key: [2u8; 32],
            skip_dir_patterns: Vec::new(),
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

        // 2. Encrypted note subnotes
        let encrypted_id = "test_note.clin";
        let enc_subnotes = vec![SubNote {
            id: "3".to_string(),
            title: "Secret Sub 1".to_string(),
            content: "Secret Content 1".to_string(),
            updated_at: 300,
        }];

        storage.set_subnotes(encrypted_id, &enc_subnotes)?;

        // Retrieve and assert
        let retrieved_enc = storage.get_subnotes(encrypted_id)?;
        assert_eq!(retrieved_enc.len(), 1);
        assert_eq!(retrieved_enc[0].title, "Secret Sub 1");
        assert_eq!(retrieved_enc[0].content, "Secret Content 1");

        let notes_with2 = storage.get_notes_with_subnotes()?;
        assert!(notes_with2.contains(encrypted_id));

        // Verify database contents do NOT contain plaintext secret
        let db_contents_after = std::fs::read(&db_path)?;
        let contains_secret = db_contents_after.windows(12).any(|w| w == b"Secret Sub 1");
        assert!(!contains_secret);

        // 3. Deletion / Cleanup
        storage.set_subnotes(plain_id, &[])?;
        let retrieved_deleted = storage.get_subnotes(plain_id)?;
        assert!(retrieved_deleted.is_empty());

        let notes_with3 = storage.get_notes_with_subnotes()?;
        assert!(!notes_with3.contains(plain_id));
        assert!(notes_with3.contains(encrypted_id));

        // Delete all
        storage.set_subnotes(encrypted_id, &[])?;
        let retrieved_deleted_enc = storage.get_subnotes(encrypted_id)?;
        assert!(retrieved_deleted_enc.is_empty());

        // The file should be completely deleted when empty
        assert!(!db_path.exists());

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
            key: [2u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        // Subnotes metadata is always owned by the storage root.
        fs::create_dir_all(storage.data_dir.join(".clin"))?;

        // Write corrupt bytes directly to subnotes db file path
        let db_path = storage.subnotes_db_path();
        std::fs::write(&db_path, b"garbage data that is not a valid bincode map")?;

        // Retrieve subnotes - should return empty vec instead of panicking
        let retrieved = storage.get_subnotes("some_note.md")?;
        assert!(retrieved.is_empty());

        let notes_with = storage.get_notes_with_subnotes()?;
        assert!(notes_with.is_empty());

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
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
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
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
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
        assert!(summary.tags.is_empty());
        assert!(!summary.pinned);
        assert!(summary.links.is_empty());
        assert!(summary.size_bytes > 0);

        Ok(())
    }

    #[test]
    fn remove_file_if_exists_is_idempotent() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("note_cache.bin");

        // File doesn't exist yet — should return false
        assert!(!remove_file_if_exists(&path)?);

        // Create the file and remove it — should return true
        fs::write(&path, b"stale data")?;
        assert!(remove_file_if_exists(&path)?);
        assert!(!path.exists(), "file must be deleted after removal");

        // File already gone — should return false (idempotent)
        assert!(!remove_file_if_exists(&path)?);

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
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
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
