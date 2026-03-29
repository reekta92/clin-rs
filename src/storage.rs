use crate::config::BootstrapConfig;
use crate::constants::*;
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
pub struct AppSettings {
    #[serde(default = "default_encryption_enabled")]
    pub encryption_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            encryption_enabled: true,
        }
    }
}

fn default_encryption_enabled() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub title: String,
    pub content: String,
    pub updated_at: u64,
}

#[derive(Debug, bincode::BorrowDecode)]
pub struct NoteBorrowed<'a> {
    pub title: Cow<'a, str>,
    #[allow(dead_code)]
    pub content: Cow<'a, str>,
    pub updated_at: u64,
}

#[derive(Debug, Clone)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
}

#[derive(Debug)]
pub struct Storage {
    pub data_dir: PathBuf,
    pub notes_dir: PathBuf,
    pub templates_dir: PathBuf,
    pub key: [u8; 32],
}

impl Storage {
    pub fn init() -> Result<Self> {
        // Load bootstrap config to get storage path
        let bootstrap = BootstrapConfig::load().context("failed to load bootstrap config")?;
        let data_dir = bootstrap
            .effective_storage_path()
            .context("failed to determine storage path")?;

        let notes_dir = data_dir.join("notes");
        let templates_dir = data_dir.join("templates");
        fs::create_dir_all(&notes_dir).context("failed to create notes directory")?;
        fs::create_dir_all(&templates_dir).context("failed to create templates directory")?;

        let key_path = data_dir.join("key.bin");
        let key = if key_path.exists() {
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
            fs::write(&key_path, key).context("failed to write encryption key")?;
            key
        };

        Ok(Self {
            data_dir,
            notes_dir,
            templates_dir,
            key,
        })
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    pub fn keybinds_path(&self) -> PathBuf {
        self.data_dir.join("keybinds.toml")
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

    pub fn load_settings(&self) -> AppSettings {
        let path = self.settings_path();
        if !path.exists() {
            return AppSettings::default();
        }

        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppSettings>(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &AppSettings) {
        if let Ok(raw) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(self.settings_path(), raw);
        }
    }

    pub fn note_path(&self, id: &str) -> PathBuf {
        self.notes_dir.join(id)
    }

    pub fn list_note_ids(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.notes_dir).context("failed reading notes directory")? {
            let entry = entry.context("failed to read note entry")?;
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && (ext == "clin" || ext == "md" || ext == "txt")
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                ids.push(name.to_string());
            }
        }
        Ok(ids)
    }

    pub fn load_note_summary(&self, id: &str) -> Result<NoteSummary> {
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if ext == "clin" {
            let encrypted = fs::read(&path).context("failed to read note")?;
            let plain = self.decrypt(&encrypted)?;
            let (note, _): (NoteBorrowed, usize) =
                bincode::borrow_decode_from_slice(&plain, bincode::config::standard())
                    .context("failed to decode note")?;
            Ok(NoteSummary {
                id: id.to_string(),
                title: note.title.into_owned(),
                updated_at: note.updated_at,
            })
        } else {
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
            })
        }
    }

    pub fn load_note(&self, id: &str) -> Result<Note> {
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if ext == "clin" {
            let encrypted = fs::read(&path).context("failed to read note")?;
            let plain = self.decrypt(&encrypted)?;
            let (note, _) =
                bincode::serde::decode_from_slice::<Note, _>(&plain, bincode::config::standard())
                    .context("failed to decode note")?;
            Ok(note)
        } else {
            let content = fs::read_to_string(&path).context("failed to read plain note")?;
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
                content,
                updated_at,
            })
        }
    }

    pub fn save_note(&self, id: &str, note: &Note, encryption_enabled: bool) -> Result<String> {
        let preferred_stem = self.note_file_stem_from_title(&note.title);

        let old_path = self.note_path(id);
        let old_ext = old_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // If we are currently saving an unencrypted file and encryption is OFF, keep its extension.
        // Otherwise default to .md if no extension matched.
        let target_ext = if encryption_enabled {
            "clin"
        } else if old_ext == "txt" || old_ext == "md" {
            old_ext
        } else {
            "md"
        };

        let target_id = self.unique_note_id(&preferred_stem, target_ext, id);

        if target_ext == "clin" {
            let bytes = bincode::serde::encode_to_vec(note, bincode::config::standard())
                .context("failed to encode note")?;
            let encrypted = self.encrypt(&bytes)?;
            fs::write(self.note_path(&target_id), encrypted).context("failed to write note")?;
        } else {
            fs::write(self.note_path(&target_id), &note.content)
                .context("failed to write plain note")?;
        }

        // We only remove the old file if the target_id differs AND we are not creating an encrypted copy of a plain text file.
        // Wait, the prompt says: "when encryption on if user tries to open a unencrypted file the program should create a encrypted copy... and edit that copy not the original unencrypted file".
        // App handles setting `editing_id` to a newly generated ID when opening to avoid overwriting. But here, just in case, if `id` != `target_id` we delete the old one, UNLESS we specifically want to avoid it.
        // If App changes `editing_id` to `uuid.clin` before saving, then `id` (the current editing state) is already `uuid.clin`, it won't equal `old_path` in name so... Wait.
        // If App changes `app.editing_id = Some(new_id)`, then the first time it autosaves, it calls `save_note(new_id, note, true)`. So `id` == `target_id` (likely), and the old file is untouched because `id` doesn't point to the original file anymore!
        // So `save_note` can safely delete `old_path` if `id != target_id` because `id` represents the file WE ARE CURRENTLY EDITING which is getting renamed.
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

    pub fn new_note_id(&self) -> String {
        Uuid::new_v4().to_string()
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
        let mut candidate_stem = preferred_stem.to_string();
        let mut candidate = format!("{candidate_stem}.{ext}");
        let mut counter = 2_u32;

        while candidate != current_id && self.note_path(&candidate).exists() {
            candidate_stem = format!("{preferred_stem} ({counter})");
            candidate = format!("{candidate_stem}.{ext}");
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

    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() <= FILE_MAGIC.len() + NONCE_LEN {
            anyhow::bail!("note file is too short")
        }
        if &encrypted[..FILE_MAGIC.len()] != FILE_MAGIC {
            anyhow::bail!("invalid note header")
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
