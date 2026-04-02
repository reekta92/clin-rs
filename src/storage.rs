use crate::config::BootstrapConfig;
use crate::constants::*;
use crate::frontmatter;
use crate::keybinds::Keybinds;
use crate::templates::TemplateManager;
use anyhow::{Context, Result, anyhow};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

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

#[derive(Debug, bincode::BorrowDecode)]
pub struct NoteBorrowed<'a> {
    pub title: Cow<'a, str>,
    #[allow(dead_code)]
    pub content: Cow<'a, str>,
    pub updated_at: u64,
    // Add default deserialization logic if tags aren't present (for bincode backwards compatibility)
    // Actually, bincode doesn't handle schema changes easily without a specific setup.
    // BUT we decided that tags will be stored in FRONTMATTER, not in the bincode blob!
    // So the bincode blob remains identical.
}

#[derive(Debug, Clone)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    pub folder: String,
    pub tags: Vec<String>,
    pub pinned: bool,
}

#[derive(Clone, Debug)]
#[derive(zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
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

fn extract_frontmatter_from_bytes(bytes: &[u8]) -> Option<frontmatter::Frontmatter> {
    if bytes.starts_with(b"---\n") || bytes.starts_with(b"---\r\n") {
        let end_marker = b"\n---";
        if let Some(end_idx) = bytes[3..]
            .windows(end_marker.len())
            .position(|w| w == end_marker)
            && let Ok(fm_str) = std::str::from_utf8(&bytes[3..3 + end_idx])
            && let Ok(fm) = serde_yml::from_str::<frontmatter::Frontmatter>(fm_str)
        {
            return Some(fm);
        }
    }
    None
}

impl Storage {
    pub fn init() -> Result<Self> {
        // Load bootstrap config to get storage path
        let bootstrap = BootstrapConfig::load().context("failed to load bootstrap config")?;
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

        let key_path = data_dir.join("key.bin");
        let key = if key_path.exists() {
            #[cfg(unix)]
            {
                // Enforce permissions on existing key file
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
            let mut key = [0_u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            fs::create_dir_all(&data_dir).context("failed to create app data directory")?;
            
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
                file.write_all(&key).context("failed to write encryption key")?;
            }
            
            #[cfg(not(unix))]
            {
                fs::write(&key_path, key).context("failed to write encryption key")?;
            }
            
            key
        };

        Ok(Self {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key,
        })
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
                    && (ext == "clin" || ext == "md" || ext == "txt")
                    && let Ok(rel_path) = path.strip_prefix(&self.notes_dir)
                    && let Some(rel_str) = rel_path.to_str()
                {
                    ids.push(rel_str.to_string());
                }
            }
        }
        Ok(ids)
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
            let mut tags = Vec::new();
            let mut pinned = false;
            if let Some(fm) = extract_frontmatter_from_bytes(&file_content) {
                tags = fm.tags;
                pinned = fm.pinned;
            }

            let plain = self.decrypt(&file_content)?;
            let (note, _): (NoteBorrowed, usize) =
                bincode::borrow_decode_from_slice(&plain, bincode::config::standard())
                    .context("failed to decode note")?;
            Ok(NoteSummary {
                id: id.to_string(),
                title: note.title.into_owned(),
                updated_at: note.updated_at,
                folder,
                tags,
                pinned,
            })
        } else {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let (fm, _) = frontmatter::parse(&content);

            let title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled note")
                .to_string();
            let updated_at = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            Ok(NoteSummary {
                id: id.to_string(),
                title,
                updated_at,
                folder,
                tags: fm.tags,
                pinned: fm.pinned,
            })
        }
    }

    pub fn load_note(&self, id: &str) -> Result<Note> {
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if ext == "clin" {
            let file_content = fs::read(&path).context("failed to read note")?;
            let mut tags = Vec::new();
            if let Some(fm) = extract_frontmatter_from_bytes(&file_content) {
                tags = fm.tags;
            }

            let plain = self.decrypt(&file_content)?;
            let (mut note, _) =
                bincode::serde::decode_from_slice::<Note, _>(&plain, bincode::config::standard())
                    .context("failed to decode note")?;
            note.tags = tags;
            Ok(note)
        } else {
            let file_content = fs::read_to_string(&path).context("failed to read plain note")?;
            let (fm, plain_content) = frontmatter::parse(&file_content);

            let title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled note")
                .to_string();
            let updated_at = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            Ok(Note {
                title,
                content: plain_content.to_string(),
                updated_at,
                tags: fm.tags,
            })
        }
    }

    pub fn save_note(&self, id: &str, note: &Note, encryption_enabled: bool) -> Result<String> {
        let preferred_stem = self.note_file_stem_from_title(&note.title);

        let old_path = self.note_path(id);
        let old_ext = old_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let target_ext = if encryption_enabled {
            "clin"
        } else if old_ext == "txt" || old_ext == "md" {
            old_ext
        } else {
            "md"
        };

        let target_id = self.unique_note_id(&preferred_stem, target_ext, id);
        let fm = frontmatter::Frontmatter {
            tags: note.tags.clone(),
            pinned: false,
        };

        let target_path = self.note_path(&target_id);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).unwrap_or_default();
        }

        if target_ext == "clin" {
            let bytes = bincode::serde::encode_to_vec(note, bincode::config::standard())
                .context("failed to encode note")?;
            let encrypted = self.encrypt(&bytes)?;

            // Serialize frontmatter and prepend to encrypted bytes
            let fm_string = frontmatter::serialize(&fm, "");
            let mut final_output = fm_string.into_bytes();
            final_output.extend_from_slice(&encrypted);

            fs::write(target_path, final_output).context("failed to write note")?;
        } else {
            let final_content = frontmatter::serialize(&fm, &note.content);
            fs::write(target_path, final_content).context("failed to write plain note")?;
        }

        if id != target_id {
            let old_path_to_remove = self.note_path(id);
            if old_path_to_remove.exists() {
                fs::remove_file(&old_path_to_remove).context("failed to rename note file")?;
            }
        }

        Ok(target_id)
    }

    pub fn delete_note(&self, id: &str) -> Result<()> {
        fs::remove_file(self.note_path(id)).context("failed to delete note")?;
        Ok(())
    }

    /// Rename a note by updating its title (which changes the filename)
    pub fn rename_note(&self, id: &str, new_title: &str) -> Result<String> {
        let mut note = self.load_note(id)?;
        note.title = new_title.to_string();
        note.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let is_encrypted = id.ends_with(".clin");
        self.save_note(id, &note, is_encrypted)
    }

    /// Duplicate a note with a new title
    pub fn duplicate_note(&self, id: &str) -> Result<String> {
        let note = self.load_note(id)?;
        let new_title = format!("{} (Copy)", note.title);
        let mut new_note = note;
        new_note.title = new_title;
        new_note.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Generate a new unique ID for the duplicate
        let new_id = self.new_note_id();
        let is_encrypted = id.ends_with(".clin");

        // Determine the folder from the original note
        let folder = if let Some(idx) = id.rfind('/') {
            &id[..idx]
        } else {
            ""
        };

        // Create initial ID with folder prefix
        let initial_id = if folder.is_empty() {
            format!("{}.{}", new_id, if is_encrypted { "clin" } else { "md" })
        } else {
            format!("{}/{}.{}", folder, new_id, if is_encrypted { "clin" } else { "md" })
        };

        self.save_note(&initial_id, &new_note, is_encrypted)
    }

    /// Move a note to the trash folder instead of permanently deleting it
    pub fn trash_note(&self, id: &str) -> Result<()> {
        let trash_dir = self.notes_dir.join(".trash");
        fs::create_dir_all(&trash_dir).context("failed to create trash directory")?;

        let src_path = self.note_path(id);
        if !src_path.exists() {
            anyhow::bail!("Note does not exist");
        }

        let file_name = src_path
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        // Add timestamp to avoid conflicts
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let (stem, ext) = if let Some(dot_idx) = file_name.rfind('.') {
            (&file_name[..dot_idx], &file_name[dot_idx..])
        } else {
            (file_name, "")
        };

        let trash_name = format!("{}_deleted_{}{}", stem, timestamp, ext);
        let dest_path = trash_dir.join(&trash_name);

        fs::rename(&src_path, &dest_path).context("failed to move note to trash")?;
        Ok(())
    }

    /// List notes in the trash folder
    pub fn list_trash(&self) -> Result<Vec<NoteSummary>> {
        let trash_dir = self.notes_dir.join(".trash");
        if !trash_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();
        if let Ok(entries) = fs::read_dir(&trash_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "clin" || ext == "md" || ext == "txt" {
                        let id = format!(".trash/{}", path.file_name().unwrap_or_default().to_str().unwrap_or(""));
                        if let Ok(summary) = self.load_note_summary(&id) {
                            summaries.push(summary);
                        }
                    }
                }
            }
        }

        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(summaries)
    }

    /// Restore a note from the trash folder
    pub fn restore_note(&self, trash_id: &str) -> Result<String> {
        let src_path = self.note_path(trash_id);
        if !src_path.exists() {
            anyhow::bail!("Note does not exist in trash");
        }

        let file_name = src_path
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        // Remove the _deleted_timestamp suffix
        let restored_name = if let Some(idx) = file_name.find("_deleted_") {
            let ext_start = file_name.rfind('.').unwrap_or(file_name.len());
            format!("{}{}", &file_name[..idx], &file_name[ext_start..])
        } else {
            file_name.to_string()
        };

        let dest_path = self.notes_dir.join(&restored_name);

        // Ensure unique name
        let final_dest = if dest_path.exists() {
            let stem = dest_path.file_stem().unwrap_or_default().to_str().unwrap_or("");
            let ext = dest_path.extension().unwrap_or_default().to_str().unwrap_or("");
            let mut counter = 1;
            loop {
                let new_name = format!("{}_{}.{}", stem, counter, ext);
                let new_path = self.notes_dir.join(&new_name);
                if !new_path.exists() {
                    break new_path;
                }
                counter += 1;
            }
        } else {
            dest_path
        };

        fs::rename(&src_path, &final_dest).context("failed to restore note from trash")?;

        let restored_id = final_dest
            .strip_prefix(&self.notes_dir)
            .ok()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();

        Ok(restored_id)
    }

    /// Permanently delete a note from trash
    pub fn delete_from_trash(&self, trash_id: &str) -> Result<()> {
        let path = self.note_path(trash_id);
        if !path.exists() {
            anyhow::bail!("Note does not exist in trash");
        }
        fs::remove_file(&path).context("failed to permanently delete note")?;
        Ok(())
    }

    /// Empty the entire trash folder
    pub fn empty_trash(&self) -> Result<usize> {
        let trash_dir = self.notes_dir.join(".trash");
        if !trash_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        if let Ok(entries) = fs::read_dir(&trash_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if fs::remove_file(&path).is_ok() {
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    /// Toggle the pinned status of a note
    pub fn toggle_pin(&self, id: &str) -> Result<bool> {
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if ext == "clin" {
            // For encrypted notes, we need to update frontmatter
            let file_content = fs::read(&path).context("failed to read note")?;
            let mut fm = extract_frontmatter_from_bytes(&file_content).unwrap_or_default();
            fm.pinned = !fm.pinned;
            let new_pinned = fm.pinned;

            // Decrypt, update frontmatter, and re-encrypt
            let plain = self.decrypt(&file_content)?;
            let fm_string = frontmatter::serialize(&fm, "");
            let mut final_output = fm_string.into_bytes();

            // Re-encrypt the original decrypted content
            let encrypted = self.encrypt(&plain)?;
            final_output.extend_from_slice(&encrypted);

            fs::write(&path, final_output).context("failed to write note")?;
            Ok(new_pinned)
        } else {
            // For plain notes, parse and update frontmatter
            let content = fs::read_to_string(&path).context("failed to read note")?;
            let (mut fm, body) = frontmatter::parse(&content);
            fm.pinned = !fm.pinned;
            let new_pinned = fm.pinned;

            let new_content = frontmatter::serialize(&fm, body);
            fs::write(&path, new_content).context("failed to write note")?;
            Ok(new_pinned)
        }
    }

    pub fn new_note_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn create_folder(&self, path: &str) -> Result<()> {
        let full_path = self.validate_path_within_notes_dir(path)
            .ok_or_else(|| anyhow::anyhow!("Invalid folder path"))?;
        fs::create_dir_all(full_path).context("failed to create folder")
    }

    pub fn delete_folder(&self, path: &str) -> Result<()> {
        let full_path = self.validate_path_within_notes_dir(path)
            .ok_or_else(|| anyhow::anyhow!("Invalid folder path"))?;
        fs::remove_dir(full_path).context("failed to delete folder")
    }

    pub fn rename_folder(&self, old_path: &str, new_path: &str) -> Result<()> {
        let old_full = self.validate_path_within_notes_dir(old_path)
            .ok_or_else(|| anyhow::anyhow!("Invalid source folder path"))?;
        let new_full = self.validate_path_within_notes_dir(new_path)
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
            return Ok(id.to_string()); // No change
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

    pub fn load_tag_cache(&self) -> Vec<String> {
        let path = self.data_dir.join("tags.json");
        if let Ok(data) = fs::read_to_string(path)
            && let Ok(tags) = serde_json::from_str::<Vec<String>>(&data)
        {
            return tags;
        }
        Vec::new()
    }

    pub fn save_tag_cache(&self, tags: &[String]) -> Result<()> {
        let path = self.data_dir.join("tags.json");
        let mut unique_tags = tags.to_vec();
        unique_tags.sort();
        unique_tags.dedup();
        let json = serde_json::to_string_pretty(&unique_tags)?;
        fs::write(path, json).context("failed to save tag cache")
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

    pub fn decrypt(&self, file_content: &[u8]) -> Result<Vec<u8>> {
        let magic_pos = file_content
            .windows(FILE_MAGIC.len())
            .position(|w| w == FILE_MAGIC);

        let start = magic_pos.ok_or_else(|| anyhow!("invalid note header, missing CLIN"))?;
        let encrypted = &file_content[start..];

        if encrypted.len() <= FILE_MAGIC.len() + NONCE_LEN {
            anyhow::bail!("note file is too short")
        }
        let nonce_start = FILE_MAGIC.len();
        let nonce_end = nonce_start + NONCE_LEN;
        let nonce = &encrypted[nonce_start..nonce_end];
        let ciphertext = &encrypted[nonce_end..];

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let plain = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("failed to decrypt note file"))?;
        Ok(plain)
    }
}
