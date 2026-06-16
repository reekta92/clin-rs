use crate::config::ClinConfig;
use crate::constants::*;
use crate::frontmatter;
use crate::keybinds::Keybinds;
use crate::templates::TemplateManager;
use anyhow::{Context, Result, anyhow};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub title: String,
    pub content: String,
    pub updated_at: u64,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
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
}

fn split_frontmatter_payload(bytes: &[u8]) -> (Option<frontmatter::Frontmatter>, &[u8]) {
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

impl Storage {
    pub fn init() -> Result<Self> {
        let bootstrap = ClinConfig::load().context("failed to load config")?;
        let data_dir = bootstrap
            .effective_storage_path()
            .context("failed to determine storage path")?;

        let proj_dirs = directories::ProjectDirs::from("com", "clin", "clin")
            .context("could not determine config directory")?;
        let config_dir = proj_dirs.config_dir().to_path_buf();

        let notes_dir = data_dir.join("notes");
        let templates_dir = data_dir.join("templates");
        fs::create_dir_all(&notes_dir).context("failed to create notes directory")?;
        fs::create_dir_all(&templates_dir).context("failed to create templates directory")?;

        let old_key_path = data_dir.join("key.bin");
        let key_path = config_dir.join("key.bin");

        // Migration: move key from data_dir to config_dir if it exists in old location and not in new
        if !key_path.exists()
            && old_key_path.exists()
            && let Ok(raw) = fs::read(&old_key_path)
            && raw.len() == 32
        {
            #[cfg(unix)]
            {
                let _ = crate::fsutil::atomic_write_with_mode(&key_path, &raw, 0o400);
            }
            #[cfg(not(unix))]
            {
                let _ = crate::fsutil::atomic_write(&key_path, &raw);
            }
            let _ = fs::remove_file(&old_key_path);
        }

        let key = if key_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = fs::metadata(&key_path) {
                    let mut perms = metadata.permissions();
                    if perms.mode() & 0o777 != 0o400 {
                        perms.set_mode(0o400);
                        let _ = fs::set_permissions(&key_path, perms);
                    }
                }
            }

            let raw = fs::read(&key_path).context("failed to read encryption key")?;
            if raw.len() != 32 {
                anyhow::bail!("invalid key file length")
            }
            let mut key = [0_u8; 32];
            key.copy_from_slice(&raw);
            key
        } else {
            [0_u8; 32]
        };

        let storage = Self {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key,
        };
        storage.migrate_extensions();
        Ok(storage)
    }

    fn key_path(&self) -> PathBuf {
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

        rand::rngs::OsRng.fill_bytes(&mut self.key);
        fs::create_dir_all(&self.config_dir).context("failed to create config directory")?;

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
            fs::create_dir_all(parent).unwrap_or_default();
        }

        let original_ext = old_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_string());
        let fm = frontmatter::Frontmatter {
            title: Some(note.title.clone()),
            updated_at: Some(note.updated_at),
            tags: note.tags.clone(),
            pinned: false,
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
            fs::create_dir_all(parent).unwrap_or_default();
        }

        let is_raw = orig_ext == "canvas" || orig_ext == "draw";
        if is_raw {
            crate::fsutil::atomic_write(&target_path, note.content.as_bytes())
                .context("failed to write decrypted note")?;
        } else {
            let fm = frontmatter::Frontmatter {
                title: Some(note.title.clone()),
                updated_at: Some(note.updated_at),
                tags: note.tags.clone(),
                pinned: false,
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

    pub fn keybinds_path(&self) -> PathBuf {
        self.config_dir.join("keybinds.toml")
    }

    pub fn load_keybinds(&self) -> Keybinds {
        Keybinds::load(&self.keybinds_path()).unwrap_or_default()
    }

    pub fn save_keybinds(&self, keybinds: &Keybinds) -> Result<()> {
        keybinds.save(&self.keybinds_path())
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

    pub fn list_note_ids(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        let mut dirs_to_visit = vec![self.notes_dir.clone()];

        while let Some(dir) = dirs_to_visit.pop() {
            for entry in fs::read_dir(&dir).context("failed reading directory")? {
                let entry = entry.context("failed to read entry")?;
                let path = entry.path();

                if path.is_dir() {
                    dirs_to_visit.push(path);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && (ext == "clin"
                        || ext == "md"
                        || ext == "txt"
                        || ext == "draw"
                        || ext == "canvas")
                    && let Ok(rel_path) = path.strip_prefix(&self.notes_dir)
                    && let Some(rel_str) = rel_path.to_str()
                {
                    ids.push(rel_str.to_string());
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
                bincode::serde::decode_from_slice(&plain, bincode::config::standard())
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
        } else {
            let content = fs::read_to_string(&path).unwrap_or_default();
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
            let (mut note, _) =
                bincode::serde::decode_from_slice::<Note, _>(&plain, bincode::config::standard())
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

    pub fn save_note(&self, id: &str, note: &Note) -> Result<String> {
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
            fs::create_dir_all(parent).unwrap_or_default();
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
        }

        Ok(target_id)
    }

    pub fn rename_note(&self, id: &str, new_title: &str) -> Result<String> {
        let mut note = self.load_note(id)?;
        note.title = new_title.to_string();
        note.updated_at = crate::ui::now_unix_secs();

        self.save_note(id, &note)
    }

    pub fn duplicate_note(&self, id: &str, target_folder: &str) -> Result<String> {
        let note = self.load_note(id)?;
        let new_title = format!("{} (Copy)", note.title);
        let mut new_note = note;
        new_note.title = new_title;
        new_note.updated_at = crate::ui::now_unix_secs();

        let new_id = self.new_note_id();
        let is_encrypted = id.ends_with(".clin");

        let initial_id = if target_folder.is_empty() {
            format!("{}.{}", new_id, if is_encrypted { "clin" } else { "md" })
        } else {
            format!(
                "{}/{}.{}",
                target_folder,
                new_id,
                if is_encrypted { "clin" } else { "md" }
            )
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

        if ext == "clin" {
            let file_content = fs::read(&path).context("failed to read note")?;
            let (fm_opt, payload) = split_frontmatter_payload(&file_content);
            let mut fm = fm_opt.unwrap_or_default();
            fm.pinned = !fm.pinned;
            let new_pinned = fm.pinned;

            let plain = self.decrypt(payload)?;
            let fm_string = frontmatter::serialize(&fm, "");
            let mut final_output = fm_string.into_bytes();

            let encrypted = self.encrypt(&plain)?;
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

    pub fn move_note(&self, id: &str, new_folder: &str) -> Result<String> {
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
        Ok(target_id)
    }

    pub fn list_folders(&self) -> Result<Vec<String>> {
        let mut folders = Vec::new();
        let mut dirs_to_visit = vec![self.notes_dir.clone()];

        while let Some(dir) = dirs_to_visit.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
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
            let valid = ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.');
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
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| anyhow!("note encryption failed"))?;

        let mut output = Vec::with_capacity(FILE_MAGIC.len() + NONCE_LEN + ciphertext.len());
        output.extend_from_slice(FILE_MAGIC);
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    pub fn decrypt(&self, payload: &[u8]) -> Result<Vec<u8>> {
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
        cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("note decryption failed"))
    }
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
        };

        let plaintext = b"Secret Message";
        let encrypted = storage.encrypt(plaintext)?;
        let decrypted = storage.decrypt(&encrypted)?;
        assert_eq!(decrypted, plaintext);

        // Test with frontmatter
        let mut file_content = b"---\ntitle: CLIN1 in title\n---\n".to_vec();
        file_content.extend_from_slice(&encrypted);

        let (fm, payload) = split_frontmatter_payload(&file_content);
        assert!(fm.is_some());
        let decrypted = storage.decrypt(payload)?;
        assert_eq!(decrypted, plaintext);

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
        let storage = Storage {
            data_dir: PathBuf::new(),
            config_dir: PathBuf::new(),
            notes_dir: notes_dir.clone(),
            templates_dir: PathBuf::new(),
            key: [0u8; 32],
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
}
