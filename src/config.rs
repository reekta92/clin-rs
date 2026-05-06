





use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GraphLabelMode {
    
    #[default]
    Selected,
    
    Neighbors,
    
    All,
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeConfig {
    
    #[serde(default)]
    pub theme: String,
    
    #[serde(default)]
    pub background: String,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootstrapConfig {
    
    pub storage_path: Option<PathBuf>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_storage_path: Option<PathBuf>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_editor: Option<String>,
    
    #[serde(default)]
    pub external_editor_enabled: bool,
    
    #[serde(default = "default_preview_enabled")]
    pub preview_enabled: bool,
    
    #[serde(default)]
    pub markdown_preview_enabled: bool,
    
    #[serde(default)]
    pub graph_label_mode: GraphLabelMode,
    
    #[serde(default)]
    pub theme: ThemeConfig,
}

fn default_preview_enabled() -> bool {
    true
}

impl BootstrapConfig {
    
    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "clin", "clin")
            .context("could not determine config directory")?;
        Ok(proj_dirs.config_dir().join("config.toml"))
    }

    
    pub fn default_storage_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "clin", "clin")
            .context("could not determine data directory")?;
        Ok(proj_dirs.data_local_dir().to_path_buf())
    }

    
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

    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        
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

    
    pub fn effective_storage_path(&self) -> Result<PathBuf> {
        match &self.storage_path {
            Some(path) => Ok(path.clone()),
            None => Self::default_storage_path(),
        }
    }

    
    pub fn set_storage_path(&mut self, path: PathBuf) {
        self.storage_path = Some(path);
    }

    
    pub fn reset_storage_path(&mut self) {
        self.storage_path = None;
    }

    
    pub fn has_custom_storage_path(&self) -> bool {
        self.storage_path.is_some()
    }

    
    pub fn set_previous_storage_path(&mut self, path: PathBuf) {
        self.previous_storage_path = Some(path);
    }

    
    pub fn clear_previous_storage_path(&mut self) {
        self.previous_storage_path = None;
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
