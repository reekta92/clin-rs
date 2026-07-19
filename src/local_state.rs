//! Versioned local state (`state.json`).
//!
//! This replaces ad‑hoc `goals_progress.json`, `CoreConfig.previous_storage_path`,
//! and `ListConfig.expanded_folders` with a single versioned JSON document stored
//! under `<AppPaths::data_local_dir>/state.json`.
//!
//! All writes are atomic (temp‑file + rename) with mode `0o600` on Unix.

use crate::goals::DailyProgress;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LOCAL_STATE_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Top‑level state
// ---------------------------------------------------------------------------

/// The versioned on‑disk state document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalState {
    /// Schema version (currently 1).
    pub version: u32,
    /// Storage‑migration record, set by `storage set`/`storage reset` and
    /// cleared by `storage migrate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_migration: Option<StorageMigrationState>,
    /// Global daily goals progress.
    #[serde(default)]
    pub goals: DailyProgress,
    /// Per‑vault UI state keyed by absolute vault identity path.
    #[serde(default)]
    pub vaults: BTreeMap<String, VaultState>,
}

/// Record of a pending storage‑path migration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageMigrationState {
    /// The previous storage path (from `CoreConfig.previous_storage_path`).
    pub previous_path: PathBuf,
    /// The target (current) storage path.
    pub target_path: PathBuf,
}

/// Per‑vault remembered UI state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct VaultState {
    /// Expanded folder paths for this vault.
    #[serde(default)]
    pub expanded_folders: BTreeSet<String>,
}

impl LocalState {
    /// Load state from `path`, returning v1 defaults if the file is missing.
    ///
    /// * Missing / empty file → returns default v1 state.
    /// * Malformed JSON → quarantines the file and returns defaults.
    /// * Future version → hard error (caller must not silently downgrade).
    pub fn load(path: &Path) -> Result<Self> {
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default_v1());
            }
            Err(e) => return Err(e).context("failed to read state file"),
        };

        if raw.trim().is_empty() {
            return Ok(Self::default_v1());
        }

        // Peek version before full deserialization.
        // Peek the version before typed deserialization so we never overwrite
        // a newer document. Corrupt or versionless state is quarantined and
        // immediately replaced with a clean v1 default.
        let version: VersionOnly = match serde_json::from_str::<VersionOnly>(&raw) {
            Ok(version) if version.version == LOCAL_STATE_VERSION => version,
            Ok(version) if version.version > LOCAL_STATE_VERSION => {
                anyhow::bail!(
                    "state version {} is newer than supported {}",
                    version.version,
                    LOCAL_STATE_VERSION
                );
            }
            Ok(_) | Err(_) => {
                Self::quarantine(path)?;
                let state = Self::default_v1();
                state.save(path)?;
                return Ok(state);
            }
        };

        debug_assert_eq!(version.version, LOCAL_STATE_VERSION);
        match serde_json::from_str::<LocalState>(&raw) {
            Ok(state) => Ok(state),
            Err(_) => {
                Self::quarantine(path)?;
                let state = Self::default_v1();
                state.save(path)?;
                Ok(state)
            }
        }
    }

    /// Save state to `path` atomically with pretty JSON, mode `0o600` on Unix.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("failed to serialize state")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create state directory")?;
        }
        // Validate round‑trip before writing.
        let _: LocalState =
            serde_json::from_str(&json).context("state round‑trip validation failed")?;
        #[cfg(unix)]
        {
            crate::fsutil::atomic_write_with_mode(path, json.as_bytes(), 0o600)
                .context("failed to write state")?;
        }
        #[cfg(not(unix))]
        {
            crate::fsutil::atomic_write(path, json.as_bytes()).context("failed to write state")?;
        }
        Ok(())
    }

    /// Convenience: load, apply `f`, and save.
    ///
    /// `f` receives `&mut Self` (already populated) and should modify in place.
    /// Returns the final state.
    pub fn update<F>(path: &Path, f: F) -> Result<Self>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        let mut state = Self::load(path)?;
        f(&mut state)?;
        state.save(path)?;
        Ok(state)
    }

    // -- Internal helpers ---------------------------------------------------

    fn default_v1() -> Self {
        Self {
            version: LOCAL_STATE_VERSION,
            storage_migration: None,
            goals: DailyProgress::default(),
            vaults: BTreeMap::new(),
        }
    }

    /// Move corrupt state to the first available quarantine path. A failed
    /// move is fatal: callers must not overwrite the original state.
    fn quarantine(path: &Path) -> Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("state path has no file name"))?
            .to_string_lossy();
        for suffix in 0.. {
            let candidate = parent.join(format!(
                "{file_name}.corrupt{}",
                if suffix == 0 {
                    String::new()
                } else {
                    format!(".{suffix}")
                }
            ));
            if !candidate.exists() {
                std::fs::rename(path, &candidate).with_context(|| {
                    format!(
                        "failed to quarantine corrupt state {} to {}",
                        path.display(),
                        candidate.display()
                    )
                })?;
                return Ok(());
            }
        }
        unreachable!("unbounded quarantine suffix loop always returns")
    }
}
/// Canonicalize a vault path for use as a state key.
///
/// Converts to absolute, canonicalises the deepest existing ancestor,
/// appends any nonexistent suffix, and rejects non‑UTF‑8 paths.
pub fn vault_identity_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to get current directory")?
            .join(path)
    };

    // Walk up ancestors to find the deepest existing one.
    let components: Vec<_> = absolute.components().collect();
    let mut existing_end = 0;
    for i in (1..=components.len()).rev() {
        let prefix: PathBuf = components[..i].iter().collect();
        if prefix.exists() {
            existing_end = i;
            break;
        }
    }

    let existing: PathBuf = components[..existing_end].iter().collect();
    let remainder: PathBuf = components[existing_end..].iter().collect();

    let canon_existing = std::fs::canonicalize(&existing)
        .with_context(|| format!("failed to canonicalize {}", existing.display()))?;

    let result = if remainder.as_os_str().is_empty() {
        canon_existing
    } else {
        canon_existing.join(remainder)
    };

    // Reject non‑UTF‑8.
    result
        .to_str()
        .map(|_| result.clone())
        .ok_or_else(|| anyhow::anyhow!("vault path is not valid UTF-8: {}", result.display()))
}

// ---------------------------------------------------------------------------
// Helper struct for version‑only peek
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct VersionOnly {
    #[serde(default)]
    version: u32,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn state_path(tmp: &TempDir) -> PathBuf {
        tmp.path().join("state.json")
    }

    #[test]
    fn test_load_missing_returns_defaults() {
        let tmp = TempDir::new().unwrap();
        let state = LocalState::load(&state_path(&tmp)).unwrap();
        assert_eq!(state.version, 1);
        assert!(state.storage_migration.is_none());
        assert!(state.vaults.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = state_path(&tmp);
        let mut state = LocalState::load(&path).unwrap();
        state.storage_migration = Some(StorageMigrationState {
            previous_path: PathBuf::from("/old/path"),
            target_path: PathBuf::from("/new/path"),
        });
        state.goals.words_written = 42;
        state.vaults.insert(
            "/vault/path".into(),
            VaultState {
                expanded_folders: ["a", "b"].into_iter().map(Into::into).collect(),
            },
        );
        state.save(&path).unwrap();

        let loaded = LocalState::load(&path).unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(
            loaded.storage_migration,
            Some(StorageMigrationState {
                previous_path: PathBuf::from("/old/path"),
                target_path: PathBuf::from("/new/path"),
            })
        );
        assert_eq!(loaded.goals.words_written, 42);
        assert_eq!(loaded.vaults.len(), 1);
        let vs = loaded.vaults.get("/vault/path").unwrap();
        assert!(vs.expanded_folders.contains("a"));
        assert!(vs.expanded_folders.contains("b"));
    }

    #[test]
    fn test_update_modifies_state() {
        let tmp = TempDir::new().unwrap();
        let path = state_path(&tmp);
        let state = LocalState::update(&path, |s| {
            s.goals.words_written = 100;
            Ok(())
        })
        .unwrap();
        assert_eq!(state.goals.words_written, 100);

        // Reload from disk
        let loaded = LocalState::load(&path).unwrap();
        assert_eq!(loaded.goals.words_written, 100);
    }

    #[test]
    fn test_future_version_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = state_path(&tmp);
        let bad = r#"{"version": 99}"#;
        std::fs::write(&path, bad).unwrap();
        let err = LocalState::load(&path).unwrap_err();
        assert!(err.to_string().contains("version 99"));
    }

    #[test]
    fn test_corrupt_json_quarantined() {
        let tmp = TempDir::new().unwrap();
        let path = state_path(&tmp);
        std::fs::write(&path, "not valid json").unwrap();

        let state = LocalState::load(&path).unwrap();
        assert_eq!(state.version, LOCAL_STATE_VERSION);

        let quarantine = tmp.path().join("state.json.corrupt");
        assert_eq!(
            std::fs::read_to_string(&quarantine).unwrap(),
            "not valid json"
        );
        assert_eq!(
            LocalState::load(&path).unwrap().version,
            LOCAL_STATE_VERSION
        );
    }

    #[test]
    fn test_vault_identity_path() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        let id = vault_identity_path(&sub).unwrap();
        assert!(id.to_string_lossy().ends_with("/a/b"));
        assert!(id.is_absolute());

        // Non‑existent suffix is preserved
        let non_existent = tmp.path().join("a").join("nonexistent");
        let id2 = vault_identity_path(&non_existent).unwrap();
        assert!(id2.to_string_lossy().ends_with("/a/nonexistent"));
    }
}
