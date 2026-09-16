use std::path::{Path, PathBuf};

use crate::app::App;

/// F4 overlay: quick vault switcher rows. `vaults` holds expanded, deduped
/// paths with the ACTIVE vault first; `selected` ranges over `0..=vaults.len()`
/// where index `vaults.len()` is the `+ Add new vault…` row.
pub struct VaultSwitcher {
    pub vaults: Vec<PathBuf>,
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
        let active = self
            .config
            .effective_storage_path()
            .unwrap_or_else(|_| self.storage.data_dir.clone());
        let mut vaults = vec![active];
        for v in self.config.expanded_vaults() {
            if !vaults.iter().any(|r| same_vault(r, &v)) {
                vaults.push(v);
            }
        }
        self.vault_switcher = Some(VaultSwitcher {
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
        let previous_config = self.config.clone();
        let mut candidate = self.config.clone();

        let (storage_result, warnings) = if crate::config::has_storage_path_override() {
            crate::storage::Storage::init_with_config_at(&candidate, &path)
        } else {
            candidate.core.storage_path = Some(path.clone());
            if !candidate.core.vaults.iter().any(|v| same_vault(v, &path)) {
                candidate.core.vaults.push(path.clone());
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
        match crate::ui::pick_directory("Select vault directory") {
            Ok(crate::ui::DirectoryPickerOutcome::Selected(picked)) => {
                let path = crate::config::path::expand_path(&picked.to_string_lossy());
                if !crate::config::has_storage_path_override()
                    && !self.config.core.vaults.iter().any(|v| same_vault(v, &path))
                {
                    self.config.core.vaults.push(path.clone());
                    if let Err(error) = self.config.save() {
                        self.config.core.vaults.pop();
                        self.set_temporary_status(&format!("Failed to save config: {error}"));
                        return;
                    }
                }
                if let Some(switcher) = self.vault_switcher.as_mut() {
                    if !switcher.vaults.iter().any(|v| same_vault(v, &path)) {
                        switcher.vaults.push(path);
                    }
                    switcher.selected = switcher.vaults.len().saturating_sub(1);
                }
            }
            Ok(crate::ui::DirectoryPickerOutcome::Cancelled) => {}
            Ok(crate::ui::DirectoryPickerOutcome::Unavailable) => {
                self.set_temporary_status_static(
                    "Directory picker unavailable — install zenity or kdialog, or add vaults in config.toml",
                );
            }
            Err(error) => {
                self.set_temporary_status(&format!("Directory picker failed: {error}"));
            }
        }
    }

    /// Confirmed `ConfirmAction::RemoveVault`: drop the path from `[core]
    /// vaults` (persisted) and from the open overlay. Vault files on disk are
    /// never touched.
    pub fn remove_vault_from_list(&mut self, path: &str) {
        let target = crate::config::path::expand_path(path);
        if !crate::config::has_storage_path_override() {
            let before = self.config.core.vaults.len();
            self.config.core.vaults.retain(|v| !same_vault(v, &target));
            if self.config.core.vaults.len() != before
                && let Err(error) = self.config.save()
            {
                self.set_temporary_status(&format!("Failed to save config: {error}"));
            }
        }
        if let Some(switcher) = self.vault_switcher.as_mut() {
            switcher.vaults.retain(|v| !same_vault(v, &target));
            switcher.selected = switcher.selected.min(switcher.vaults.len());
        }
    }
}
