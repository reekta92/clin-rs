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
