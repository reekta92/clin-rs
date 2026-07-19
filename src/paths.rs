//! Platform-aware application path boundaries.
//!
//! AppPaths establishes the canonical directory layout:
//!
//! ```text
//! <config_dir>/
//!   config.toml
//!   keybinds/{default,helix,vim,emacs}.toml
//!   themes/*.toml
//!   legacy/graf.toml                 # only after legacy migration
//! <data_local_dir>/
//!   key.bin
//!   state.json
//! <cache_dir>/
//!   note_cache.bin
//! <effective storage root>/.clin/
//!   subnotes.bin
//!   templates/                       # custom-vault mode, unchanged
//! <effective storage root>/templates/ # native mode, unchanged
//! <notes_dir>/<attachments_subdir>/
//!   <generated image files>
//! ```
//!
//! The effective config root is the parent of `ClinConfig::config_path()`,
//! so `--config` matches existing `custom_themes_dir()` behaviour.
//! Data/cache roots always come from `ProjectDirs`.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// AppPaths
// ---------------------------------------------------------------------------

/// All well-known support paths derived from config, data-local, and cache roots.
#[derive(Clone, Debug)]
pub struct AppPaths {
    /// Directory containing `config.toml` (parent of `--config` path, or XDG config).
    config_dir: PathBuf,
    /// The default XDG config directory (used as fallback for legacy lookups).
    default_config_dir: PathBuf,
    /// Durable local data directory (XDG data-local).
    data_local_dir: PathBuf,
    /// Cache directory (XDG cache).
    cache_dir: PathBuf,
}

impl AppPaths {
    /// Discover paths from the active config path.
    ///
    /// `config_file` is the active config file path (from `ClinConfig::config_path()`).
    /// The effective config root is its parent (so `--config` relocates
    /// `config.toml`, `keybinds/`, `themes/`, and same-root legacy files).
    /// Data/cache roots always come from `ProjectDirs`.
    pub fn discover(config_file: PathBuf) -> Result<Self> {
        let proj = directories::ProjectDirs::from("com", "clin", "clin")
            .context("could not determine application directories")?;

        let config_dir = config_file
            .parent()
            .context("config path has no parent")?
            .to_path_buf();

        Ok(Self {
            config_dir,
            default_config_dir: proj.config_dir().to_path_buf(),
            data_local_dir: proj.data_local_dir().to_path_buf(),
            cache_dir: proj.cache_dir().to_path_buf(),
        })
    }

    /// Construct from explicit roots (for testing).
    #[cfg(test)]
    pub fn from_roots(
        config_file: PathBuf,
        default_config_dir: PathBuf,
        data_local_dir: PathBuf,
        cache_dir: PathBuf,
    ) -> Result<Self> {
        let config_dir = config_file
            .parent()
            .context("config path has no parent")?
            .to_path_buf();
        Ok(Self {
            config_dir,
            default_config_dir,
            data_local_dir,
            cache_dir,
        })
    }

    // -- Borrowed getters ---------------------------------------------------

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn default_config_dir(&self) -> &Path {
        &self.default_config_dir
    }

    pub fn data_local_dir(&self) -> &Path {
        &self.data_local_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    // -- Derived paths ------------------------------------------------------

    /// Path to the encryption key (`<data_local_dir>/key.bin`).
    pub fn key_path(&self) -> PathBuf {
        self.data_local_dir.join("key.bin")
    }

    /// Path to versioned local state (`<data_local_dir>/state.json`).
    pub fn state_path(&self) -> PathBuf {
        self.data_local_dir.join("state.json")
    }

    /// Path to the note-summary cache (`<cache_dir>/note_cache.bin`).
    pub fn summary_cache_path(&self) -> PathBuf {
        self.cache_dir.join("note_cache.bin")
    }

    /// Keybinds directory (`<config_dir>/keybinds/`).
    pub fn keybinds_dir(&self) -> PathBuf {
        self.config_dir.join("keybinds")
    }

    /// Path for a specific preset's keybind file (`<config_dir>/keybinds/<preset>.toml`).
    pub fn keybinds_path_for_preset(&self, preset: &str) -> PathBuf {
        self.keybinds_dir().join(format!("{preset}.toml"))
    }

    /// Legacy flat keybind file in the effective config root
    /// (`<config_dir>/keybinds_<preset>.toml`).
    pub fn legacy_flat_keybinds_path_for_preset(&self, preset: &str) -> PathBuf {
        self.config_dir.join(format!("keybinds_{preset}.toml"))
    }

    /// Legacy generic keybind file (`<config_dir>/keybinds.toml`).
    pub fn legacy_generic_keybinds_path(&self) -> PathBuf {
        self.config_dir.join("keybinds.toml")
    }

    /// Legacy keybinds file in the *default* config root
    /// (`<default_config_dir>/keybinds_<preset>.toml`).
    pub fn default_root_legacy_keybinds_path_for_preset(&self, preset: &str) -> PathBuf {
        self.default_config_dir
            .join(format!("keybinds_{preset}.toml"))
    }

    /// Themes directory (`<config_dir>/themes/`).
    pub fn themes_dir(&self) -> PathBuf {
        self.config_dir.join("themes")
    }

    /// Legacy graf file (`<config_dir>/graf.toml`).
    pub fn legacy_graf_path(&self) -> PathBuf {
        self.config_dir.join("graf.toml")
    }

    /// Default-root legacy graf file (`<default_config_dir>/graf.toml`).
    pub fn default_root_legacy_graf_path(&self) -> PathBuf {
        self.default_config_dir.join("graf.toml")
    }

    /// Legacy key file in the effective config root (`<config_dir>/key.bin`).
    pub fn config_root_key_path(&self) -> PathBuf {
        self.config_dir.join("key.bin")
    }

    /// Legacy key file in the default config root (`<default_config_dir>/key.bin`).
    pub fn default_config_root_key_path(&self) -> PathBuf {
        self.default_config_dir.join("key.bin")
    }

    /// Legacy goals file (`<config_dir>/goals_progress.json`).
    pub fn legacy_goals_path(&self) -> PathBuf {
        self.config_dir.join("goals_progress.json")
    }

    /// Default-root legacy goals file (`<default_config_dir>/goals_progress.json`).
    pub fn default_root_legacy_goals_path(&self) -> PathBuf {
        self.default_config_dir.join("goals_progress.json")
    }

    /// Legacy note cache in the effective config root (`<config_dir>/note_cache.bin`).
    pub fn config_root_cache_path(&self) -> PathBuf {
        self.config_dir.join("note_cache.bin")
    }

    /// Legacy note cache in the default config root (`<default_config_dir>/note_cache.bin`).
    pub fn default_config_root_cache_path(&self) -> PathBuf {
        self.default_config_dir.join("note_cache.bin")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_dirs() -> (TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let config = tmp.path().join("config");
        let default_config = tmp.path().join("default_config");
        let data = tmp.path().join("data");
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&default_config).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        (tmp, config.join("config.toml"), default_config, data, cache)
    }

    #[test]
    fn test_discover_from_roots() {
        let (_tmp, config_file, default_config, data, cache) = make_dirs();
        let paths = AppPaths::from_roots(
            config_file.clone(),
            default_config,
            data.clone(),
            cache.clone(),
        )
        .unwrap();
        assert_eq!(paths.config_dir(), config_file.parent().unwrap());
        assert_eq!(paths.data_local_dir(), data);
        assert_eq!(paths.cache_dir(), cache);
        assert_eq!(paths.key_path(), data.join("key.bin"));
        assert_eq!(paths.state_path(), data.join("state.json"));
        assert_eq!(paths.summary_cache_path(), cache.join("note_cache.bin"));
        assert_eq!(
            paths.keybinds_path_for_preset("default"),
            config_file.parent().unwrap().join("keybinds/default.toml")
        );
    }

    #[test]
    fn test_derived_paths_are_absolute() {
        let (_tmp, config_file, default_config, data, cache) = make_dirs();
        let paths = AppPaths::from_roots(config_file, default_config, data, cache).unwrap();
        assert!(paths.key_path().is_absolute());
        assert!(paths.state_path().is_absolute());
        assert!(paths.summary_cache_path().is_absolute());
        assert!(paths.keybinds_dir().is_absolute());
        assert!(paths.themes_dir().is_absolute());
        assert!(paths.legacy_graf_path().is_absolute());
    }
}
