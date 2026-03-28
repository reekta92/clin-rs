//! Bootstrap configuration module
//!
//! This module handles the bootstrap config that lives at ~/.config/clin/config.toml
//! and is read before the main storage is initialized. It allows users to customize
//! the storage path for their vault.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// Bootstrap configuration loaded from ~/.config/clin/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootstrapConfig {
    /// Custom storage path for the vault. If None, uses default XDG data directory.
    pub storage_path: Option<PathBuf>,
}

impl BootstrapConfig {
    /// Get the path to the bootstrap config file (~/.config/clin/config.toml)
    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "clin", "clin")
            .context("could not determine config directory")?;
        Ok(proj_dirs.config_dir().join("config.toml"))
    }

    /// Get the default storage path (~/.local/share/clin)
    pub fn default_storage_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "clin", "clin")
            .context("could not determine data directory")?;
        Ok(proj_dirs.data_local_dir().to_path_buf())
    }

    /// Load the bootstrap config from disk, or return defaults if not found
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content =
            fs::read_to_string(&config_path).context("failed to read bootstrap config")?;

        let config: BootstrapConfig =
            toml::from_str(&content).context("failed to parse bootstrap config")?;

        Ok(config)
    }

    /// Save the bootstrap config to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }

        let content =
            toml::to_string_pretty(self).context("failed to serialize bootstrap config")?;

        let mut file = fs::File::create(&config_path).context("failed to create config file")?;
        file.write_all(content.as_bytes())
            .context("failed to write config file")?;

        Ok(())
    }

    /// Get the effective storage path (custom or default)
    pub fn effective_storage_path(&self) -> Result<PathBuf> {
        match &self.storage_path {
            Some(path) => Ok(path.clone()),
            None => Self::default_storage_path(),
        }
    }

    /// Set a custom storage path
    pub fn set_storage_path(&mut self, path: PathBuf) {
        self.storage_path = Some(path);
    }

    /// Reset to default storage path
    pub fn reset_storage_path(&mut self) {
        self.storage_path = None;
    }

    /// Check if a custom storage path is set
    pub fn has_custom_storage_path(&self) -> bool {
        self.storage_path.is_some()
    }
}

/// Migrate notes and key from old storage path to new storage path
pub fn migrate_storage(old_path: &PathBuf, new_path: &PathBuf) -> Result<MigrationResult> {
    let old_notes_dir = old_path.join("notes");
    let old_key_path = old_path.join("key.bin");
    let old_settings_path = old_path.join("settings.json");
    let old_keybinds_path = old_path.join("keybinds.toml");
    let old_templates_dir = old_path.join("templates");

    let new_notes_dir = new_path.join("notes");
    let new_key_path = new_path.join("key.bin");
    let new_settings_path = new_path.join("settings.json");
    let new_keybinds_path = new_path.join("keybinds.toml");
    let new_templates_dir = new_path.join("templates");

    // Create new directories
    fs::create_dir_all(&new_notes_dir).context("failed to create new notes directory")?;

    let mut result = MigrationResult::default();

    // Migrate key file
    if old_key_path.exists() && !new_key_path.exists() {
        fs::copy(&old_key_path, &new_key_path).context("failed to copy encryption key")?;
        result.key_migrated = true;
    }

    // Migrate settings
    if old_settings_path.exists() && !new_settings_path.exists() {
        fs::copy(&old_settings_path, &new_settings_path).context("failed to copy settings")?;
        result.settings_migrated = true;
    }

    // Migrate keybinds
    if old_keybinds_path.exists() && !new_keybinds_path.exists() {
        fs::copy(&old_keybinds_path, &new_keybinds_path).context("failed to copy keybinds")?;
        result.keybinds_migrated = true;
    }

    // Migrate notes
    if old_notes_dir.exists() {
        for entry in fs::read_dir(&old_notes_dir).context("failed to read old notes directory")? {
            let entry = entry.context("failed to read note entry")?;
            let old_note_path = entry.path();
            if let Some(filename) = old_note_path.file_name() {
                let new_note_path = new_notes_dir.join(filename);
                if !new_note_path.exists() {
                    fs::copy(&old_note_path, &new_note_path).context("failed to copy note")?;
                    result.notes_migrated += 1;
                }
            }
        }
    }

    // Migrate templates
    if old_templates_dir.exists() {
        fs::create_dir_all(&new_templates_dir)
            .context("failed to create new templates directory")?;
        for entry in
            fs::read_dir(&old_templates_dir).context("failed to read old templates directory")?
        {
            let entry = entry.context("failed to read template entry")?;
            let old_template_path = entry.path();
            if let Some(filename) = old_template_path.file_name() {
                let new_template_path = new_templates_dir.join(filename);
                if !new_template_path.exists() {
                    fs::copy(&old_template_path, &new_template_path)
                        .context("failed to copy template")?;
                    result.templates_migrated += 1;
                }
            }
        }
    }

    Ok(result)
}

#[derive(Debug, Default)]
pub struct MigrationResult {
    pub key_migrated: bool,
    pub settings_migrated: bool,
    pub keybinds_migrated: bool,
    pub notes_migrated: usize,
    pub templates_migrated: usize,
}

impl MigrationResult {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.key_migrated {
            parts.push("encryption key".to_string());
        }
        if self.settings_migrated {
            parts.push("settings".to_string());
        }
        if self.keybinds_migrated {
            parts.push("keybinds".to_string());
        }
        if self.notes_migrated > 0 {
            parts.push(format!("{} note(s)", self.notes_migrated));
        }
        if self.templates_migrated > 0 {
            parts.push(format!("{} template(s)", self.templates_migrated));
        }

        if parts.is_empty() {
            "No files migrated (destination already has data or source is empty)".to_string()
        } else {
            format!("Migrated: {}", parts.join(", "))
        }
    }

    pub fn has_migrations(&self) -> bool {
        self.key_migrated
            || self.settings_migrated
            || self.keybinds_migrated
            || self.notes_migrated > 0
            || self.templates_migrated > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = BootstrapConfig::default();
        assert!(config.storage_path.is_none());
        assert!(!config.has_custom_storage_path());
    }

    #[test]
    fn test_set_storage_path() {
        let mut config = BootstrapConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));
        assert!(config.has_custom_storage_path());
        assert_eq!(config.storage_path, Some(PathBuf::from("/custom/path")));
    }

    #[test]
    fn test_reset_storage_path() {
        let mut config = BootstrapConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));
        config.reset_storage_path();
        assert!(!config.has_custom_storage_path());
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut config = BootstrapConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: BootstrapConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.storage_path, parsed.storage_path);
    }
}
