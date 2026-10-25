use std::path::{Path, PathBuf};

use crate::app::App;

/// F4 overlay: quick vault switcher rows. `vaults` holds expanded, deduped
/// paths with the ACTIVE vault first; `selected` ranges over `0..=vaults.len()`
/// where index `vaults.len()` is the `+ Add new vault…` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultRow {
    pub raw_path: String,
    pub path: PathBuf,
}

pub struct VaultSwitcher {
    pub vaults: Vec<VaultRow>,
    pub selected: usize,
}

/// Identity comparison via canonicalized vault identity, falling back to raw
/// equality when canonicalization fails (missing ancestors, non-UTF-8).
pub(crate) fn same_vault(a: &Path, b: &Path) -> bool {
    match (
        crate::local_state::vault_identity_path(a),
        crate::local_state::vault_identity_path(b),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

impl App {
    /// Open the F4 vault switcher overlay. Rows: active vault first, then
    /// `[core] vaults` entries (expanded, deduped), then the implicit
    /// `+ Add new vault…` row rendered at index `vaults.len()`.
    pub fn open_vault_switcher(&mut self) {
        if self.command_palette.is_none() {
            self.record_command_usage("vault.switch");
        }
        let active = self
            .config
            .effective_storage_path()
            .unwrap_or_else(|_| self.storage.data_dir.clone());
        let active_raw = self
            .config
            .core
            .storage_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| active.to_string_lossy().into_owned());

        let mut vaults = vec![VaultRow {
            raw_path: active_raw,
            path: active,
        }];
        for (v, raw) in self.config.expanded_vaults().into_iter().zip(
            self.config
                .core
                .vaults
                .iter()
                .map(|p| p.to_string_lossy().into_owned()),
        ) {
            if !vaults.iter().any(|r| same_vault(&r.path, &v)) {
                vaults.push(VaultRow {
                    raw_path: raw,
                    path: v,
                });
            }
        }
        self.present_vault_switcher(VaultSwitcher {
            vaults,
            selected: 0,
        });
    }

    /// Switch the active vault via the in-process rebootstrap (same machinery
    /// as the Setup wizard vault change). Under `--vault` override the switch
    /// is session-only: config is never rewritten.
    pub fn switch_vault(&mut self, path: PathBuf) {
        if same_vault(&path, &self.storage.data_dir) {
            self.set_temporary_status_static("Vault already active");
            return;
        }
        if let Err(error) = self.save_before_session_replace() {
            self.set_temporary_status(&format!("Vault switch blocked: {error:#}"));
            return;
        }
        let previous_config = self.config.clone();
        let mut candidate = self.config.clone();

        let (storage_result, warnings) = if crate::config::has_storage_path_override() {
            crate::storage::Storage::init_with_config_at(&candidate, &path)
        } else {
            candidate.core.storage_path = Some(path.clone());
            for vault in [&self.storage.data_dir, &path] {
                if !candidate
                    .expanded_vaults()
                    .iter()
                    .any(|v| same_vault(v, vault))
                {
                    candidate.core.vaults.push(vault.clone());
                }
            }
            crate::storage::Storage::init_with_config(&candidate)
        };

        let storage = match storage_result {
            Ok(storage) => storage,
            Err(error) => {
                self.set_temporary_status(&format!("Vault switch failed: {error}"));
                return;
            }
        };

        if !crate::config::has_storage_path_override() {
            if let Err(error) = candidate.save() {
                self.set_temporary_status(&format!("Failed to save config: {error}"));
                return;
            }
            self.config = candidate;
        }

        self.vault_switcher = None;
        self.setup_rebootstrap = Some(crate::setup::SetupRebootstrapRequest {
            storage,
            warnings,
            previous_config,
            previous_path: self.storage.data_dir.clone(),
            selected_path: path,
        });
        self.should_quit = true;
    }

    /// `+ Add new vault…` row action: pick a directory with the native OS
    /// picker and permanently append it to `[core] vaults` (unless `--vault`
    /// override is active, in which case the addition is session-only).
    /// Adding never auto-switches.
    pub fn begin_add_vault(&mut self) {
        match self.pick_and_add_vault() {
            Ok(Some(row)) => {
                if let Some(switcher) = &mut self.vault_switcher {
                    if !switcher
                        .vaults
                        .iter()
                        .any(|r| same_vault(&r.path, &row.path))
                    {
                        switcher.vaults.push(row);
                    }
                    switcher.selected = switcher.vaults.len().saturating_sub(1);
                }
            }
            Ok(None) => {}
            Err(error) => self.set_temporary_status(&format!("Could not add vault: {error:#}")),
        }
    }

    pub(crate) fn pick_and_add_vault(&mut self) -> anyhow::Result<Option<VaultRow>> {
        let path = match crate::ui::pick_directory("Select vault directory")? {
            crate::ui::DirectoryPickerOutcome::Selected(path) => {
                crate::config::path::expand_path(&path.to_string_lossy())
            }
            crate::ui::DirectoryPickerOutcome::Cancelled => return Ok(None),
            crate::ui::DirectoryPickerOutcome::Unavailable => anyhow::bail!(
                "Directory picker unavailable — install zenity or kdialog, or add vaults in config.toml"
            ),
        };
        if !crate::config::has_storage_path_override()
            && !self.config.core.vaults.iter().any(|v| same_vault(v, &path))
        {
            let mut config = self.config.clone();
            config.core.vaults.push(path.clone());
            config.save()?;
            self.config = config;
        }
        Ok(Some(VaultRow {
            raw_path: path.to_string_lossy().into_owned(),
            path,
        }))
    }

    /// Confirmed `ConfirmAction::RemoveVault`: drop the path from `[core]
    /// vaults` (persisted) and from the open overlay. Vault files on disk are
    /// never touched.
    pub fn remove_vault_from_list(&mut self, path: &str) {
        let target = crate::config::path::expand_path(path);
        if !crate::config::has_storage_path_override() {
            let mut config = self.config.clone();
            let before = config.core.vaults.len();
            config.core.vaults.retain(|v| !same_vault(v, &target));
            if config.core.vaults.len() != before
                && let Err(error) = config.save()
            {
                self.set_temporary_status(&format!("Failed to save config: {error}"));
                return;
            }
            self.config = config;
        }
        if let Some(switcher) = self.vault_switcher.as_mut() {
            switcher.vaults.retain(|r| !same_vault(&r.path, &target));
            switcher.selected = switcher.selected.min(switcher.vaults.len());
        }
        self.remove_palette_vault(path);
    }

    /// Confirm visual renaming of a vault in the application. Updates
    /// `[core.vault_names]` map and saves the configuration.
    pub fn confirm_rename_vault(&mut self) {
        let Some(crate::popups::ActivePopup::VaultRename(popup)) = self.popups.active.take() else {
            return;
        };
        if let Err(error) =
            self.apply_vault_alias(&popup.raw_path_key, &popup.input.lines().join(" "))
        {
            self.set_temporary_status(&format!("Failed to save config: {error:#}"));
        }
    }

    pub(crate) fn apply_vault_alias(
        &mut self,
        raw_path_key: &str,
        name: &str,
    ) -> anyhow::Result<()> {
        let mut config = self.config.clone();
        let name = name.trim();
        if name.is_empty() {
            config.core.vault_names.remove(raw_path_key);
        } else {
            config
                .core
                .vault_names
                .insert(raw_path_key.to_owned(), name.to_owned());
        }
        if !crate::config::has_storage_path_override() {
            config.save()?;
        }
        self.config = config;
        Ok(())
    }
}
