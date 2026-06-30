use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use directories::ProjectDirs;

#[cfg(test)]
use parking_lot::Mutex;

pub mod de;
pub mod defaults;
pub mod merge;
pub mod path;
pub mod structs;
pub mod types;

pub use {de::*, defaults::*, merge::*, path::*, structs::*, types::*};

#[path = "../graf/themes.rs"]
pub mod themes;

// ── Path overrides ──────────────────────────────────────────────────────────

static CONFIG_PATH_OVERRIDE: OnceLock<Option<PathBuf>> = OnceLock::new();

#[cfg(test)]
pub(crate) static CONFIG_TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Set once at startup from the parsed `--config` value. Panics-free: later calls are no-ops.
pub fn set_config_path_override(path: PathBuf) {
    let _ = CONFIG_PATH_OVERRIDE.set(Some(path));
}

static STORAGE_PATH_OVERRIDE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Set once at startup from the parsed `--vault` value. No-op on later calls.
pub fn set_storage_path_override(path: PathBuf) {
    let _ = STORAGE_PATH_OVERRIDE.set(Some(path));
}

/// Effective storage-path override for this run, if `--vault` was passed.
fn storage_path_override() -> Option<PathBuf> {
    STORAGE_PATH_OVERRIDE.get().and_then(|opt| opt.clone())
}

// ── ClinConfig impl ─────────────────────────────────────────────────────────

impl ClinConfig {
    /// Returns true if key sequences are enabled: either explicitly via config
    /// or because the active keybind preset uses multi-key sequences.
    pub fn sequences_enabled(&self) -> bool {
        self.core.enable_key_sequences || self.core.keybind_preset.uses_sequences()
    }
    /// Returns true if count-prefix is enabled for the active keybind preset
    /// (Vim and Helix only — matching `:q`/`gg`/`ge` count semantics).
    pub fn counts_enabled(&self) -> bool {
        matches!(
            self.core.keybind_preset,
            KeybindPreset::Vim | KeybindPreset::Helix
        )
    }

    pub fn config_path() -> Result<PathBuf> {
        if let Some(p) = CONFIG_PATH_OVERRIDE.get().and_then(|opt| opt.as_ref()) {
            return Ok(p.clone());
        }
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
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent).context("failed to create config directory")?;
            }

            let proj_dirs = ProjectDirs::from("com", "clin", "clin")
                .ok_or_else(|| anyhow::anyhow!("no home dir"))?;
            let graf_path = proj_dirs.config_dir().join("graf.toml");
            let mut config = Self::default();

            if graf_path.exists() {
                if let Ok(content) = fs::read_to_string(&graf_path)
                    && let Ok(graf_config) = toml::from_str::<merge::GrafConfigOnly>(&content)
                {
                    config.graf.visual = graf_config.visual;
                    config.graf.physics = graf_config.physics;
                    config.graf.interaction = graf_config.interaction;
                    config.ui = graf_config.ui;
                    config.graf.filter = graf_config.filter;
                    config.graf.search = graf_config.search;
                }
                let _ = fs::rename(&graf_path, graf_path.with_extension("toml.migrated"));
            }

            let content = merge::default_config_content();
            #[cfg(unix)]
            crate::fsutil::atomic_write_with_mode(&config_path, content.as_bytes(), 0o600)
                .context("failed to write config file")?;
            #[cfg(not(unix))]
            crate::fsutil::atomic_write_str(&config_path, &content)
                .context("failed to write config file")?;

            return Ok(config);
        }

        let content = fs::read_to_string(&config_path).context("failed to read config")?;

        // Phase E: Migration (visual.notes_layout -> default_view, and flat graf/list/editor keys to nested namespaces)
        let mut value: toml::Value =
            toml::from_str(&content).context("failed to parse config for migration")?;
        let mut changed = false;
        let mut core_table = toml::value::Table::new();
        let core_legacy_keys = [
            "storage_path",
            "previous_storage_path",
            "mouse_enabled",
            "default_folder",
            "confirm_on_delete",
            "confirm_on_quit",
        ];
        if let Some(root) = value.as_table_mut() {
            for key in &core_legacy_keys {
                if let Some(v) = root.remove(*key) {
                    core_table.insert(key.to_string(), v);
                    changed = true;
                }
            }
        }
        if !core_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_core) = root.get_mut("core").and_then(|c| c.as_table_mut()) {
                for (k, v) in core_table {
                    existing_core.insert(k, v);
                }
            } else {
                root.insert("core".to_string(), toml::Value::Table(core_table));
            }
        }

        let mut ui_table = toml::value::Table::new();
        if let Some(root) = value.as_table_mut() {
            if let Some(theme) = root.remove("theme") {
                if let Some(t) = theme.as_table() {
                    for (k, v) in t {
                        ui_table.insert(k.clone(), v.clone());
                    }
                }
                changed = true;
            }
            if let Some(display) = root.remove("display") {
                if let Some(d) = display.as_table() {
                    for (k, v) in d {
                        ui_table.insert(k.clone(), v.clone());
                    }
                }
                changed = true;
            }
        }
        if !ui_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_ui) = root.get_mut("ui").and_then(|u| u.as_table_mut()) {
                for (k, v) in ui_table {
                    existing_ui.insert(k, v);
                }
            } else {
                root.insert("ui".to_string(), toml::Value::Table(ui_table));
            }
        }

        if let Some(visual) = value.get_mut("visual").and_then(|v| v.as_table_mut())
            && let Some(notes_layout) = visual.remove("notes_layout")
            && value.get("default_view").is_none()
            && let Some(root) = value.as_table_mut()
        {
            root.insert("default_view".to_string(), notes_layout);
            changed = true;
        }

        let mut editor_table = toml::value::Table::new();
        if let Some(root) = value.as_table_mut() {
            if let Some(v) = root.remove("external_editor") {
                editor_table.insert("external_command".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("external_editor_enabled") {
                editor_table.insert("external_enabled".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("editor_preview_enabled") {
                editor_table.insert("preview_enabled".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("show_line_numbers") {
                editor_table.insert("show_line_numbers".to_string(), v);
                changed = true;
            }
        }
        if !editor_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_editor) = root.get_mut("editor").and_then(|e| e.as_table_mut()) {
                for (k, v) in editor_table {
                    existing_editor.insert(k, v);
                }
            } else {
                root.insert("editor".to_string(), toml::Value::Table(editor_table));
            }
            changed = true;
        }

        let mut list_table = toml::value::Table::new();
        let list_legacy_keys = [
            ("preview_enabled", "preview_enabled"),
            ("preview_position", "preview_position"),
            ("preview_encryption", "preview_encryption"),
            ("date_format", "date_format"),
            ("list_density", "density"),
            ("show_file_size", "show_file_size"),
            ("show_date_in_list", "show_date_in_list"),
            ("default_view", "default_view"),
            ("default_sort_field", "default_sort_field"),
            ("default_sort_order", "default_sort_order"),
            ("pinned_on_top", "pinned_on_top"),
        ];
        if let Some(root) = value.as_table_mut() {
            for (old_key, new_key) in &list_legacy_keys {
                if let Some(v) = root.remove(*old_key) {
                    list_table.insert(new_key.to_string(), v);
                    changed = true;
                }
            }
        }
        if !list_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_list) = root.get_mut("list").and_then(|l| l.as_table_mut()) {
                for (k, v) in list_table {
                    existing_list.insert(k, v);
                }
            } else {
                root.insert("list".to_string(), toml::Value::Table(list_table));
            }
        }

        let mut graf_addons = toml::value::Table::new();
        if let Some(root) = value.as_table_mut() {
            if let Some(v) = root.remove("search") {
                graf_addons.insert("search".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("graph_preview_enabled") {
                graf_addons.insert("preview_enabled".to_string(), v);
                changed = true;
            }
        }

        let graf_keys = ["visual", "physics", "interaction", "filter"];
        for key in &graf_keys {
            if let Some(val) = value.as_table_mut().and_then(|t| t.remove(*key)) {
                graf_addons.insert(key.to_string(), val);
                changed = true;
            }
        }
        if !graf_addons.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_graf) = root.get_mut("graf").and_then(|g| g.as_table_mut()) {
                for (k, v) in graf_addons {
                    existing_graf.insert(k, v);
                }
            } else {
                root.insert("graf".to_string(), toml::Value::Table(graf_addons));
            }
        }
        if changed {
            let migrated_content =
                toml::to_string_pretty(&value).context("failed to serialize migrated config")?;
            let _ = crate::fsutil::atomic_write(&config_path, migrated_content.as_bytes());
            let mut config: ClinConfig =
                toml::from_str(&migrated_content).context("failed to parse migrated config")?;
            config.normalize_sections();
            return Ok(config);
        }

        let mut config: ClinConfig = toml::from_str(&content).context("failed to parse config")?;
        config.normalize_sections();
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }

        let mut doc = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            content
                .parse::<toml_edit::DocumentMut>()
                .context("failed to parse existing config")?
        } else {
            merge::default_config_content()
                .parse::<toml_edit::DocumentMut>()
                .expect("default config must be valid TOML")
        };

        let self_toml_str = toml::to_string(self).context("failed to serialize config")?;
        let self_value: toml::Value =
            toml::from_str(&self_toml_str).expect("serialized config must be valid TOML");

        if let toml::Value::Table(toml_tbl) = self_value {
            for (k, v) in toml_tbl {
                if doc.contains_key(&k) {
                    merge::merge_toml_value(
                        doc.get_mut(&k).expect("key presence already checked"),
                        &v,
                    );
                } else {
                    doc.insert(&k, merge::toml_value_to_item(&v));
                }
            }
        }

        crate::fsutil::atomic_write(&config_path, doc.to_string().as_bytes())?;
        Ok(())
    }

    pub fn effective_storage_path(&self) -> Result<PathBuf> {
        if let Some(p) = storage_path_override() {
            return Ok(p);
        }
        match &self.core.storage_path {
            Some(path) => Ok(path.clone()),
            None => Self::default_storage_path(),
        }
    }

    pub fn set_storage_path(&mut self, path: PathBuf) {
        self.core.storage_path = Some(path);
    }

    pub fn reset_storage_path(&mut self) {
        self.core.storage_path = None;
    }

    pub fn has_custom_storage_path(&self) -> bool {
        storage_path_override().is_some() || self.core.storage_path.is_some()
    }

    pub fn set_previous_storage_path(&mut self, path: PathBuf) {
        self.core.previous_storage_path = Some(path);
    }

    pub fn clear_previous_storage_path(&mut self) {
        self.core.previous_storage_path = None;
    }

    pub fn theme_colors(&self) -> ThemeColors {
        let mut colors =
            themes::theme_colors(&self.ui.theme, self.graf.visual.graph_background.clone());

        if let Some(ref c) = self.graf.visual.colors.node_color {
            colors.node_colors = vec![*c];
        }
        if let Some(c) = self.graf.visual.colors.edge_color {
            colors.edge_color = c;
        }
        if let Some(c) = self.graf.visual.colors.label_color {
            colors.label_color = c;
        }
        if let Some(c) = self.graf.visual.colors.selection_ring_color {
            colors.selected_indicator_color = c;
        }
        if let Some(c) = self.graf.visual.colors.border_color {
            colors.border_color = c;
            colors.legend_border_color = c;
            colors.minimap_border_color = c;
        }
        if let Some(c) = self.graf.visual.colors.title_color {
            colors.title_color = c;
        }
        if let Some(c) = self.graf.visual.colors.grid_color {
            colors.grid_color = c;
        }
        if let Some(c) = self.graf.visual.colors.legend_text_color {
            colors.legend_text_color = c;
        }
        if let Some(c) = self.graf.visual.colors.status_bar_color {
            colors.status_bar_color = c;
        }
        if let Some(c) = self.graf.visual.colors.background_color {
            colors.background_color = Some(c);
            colors.minimap_bg_color = Some(c);
        }

        colors
    }

    fn normalize_sections(&mut self) {
        let secs = &mut self.list.sections;
        let mut seen = std::collections::HashSet::new();
        secs.retain(|s| seen.insert(*s));
        secs.truncate(2);
        if secs.is_empty() {
            secs.extend(default_sections());
        }
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        if self.graf.visual.label_max_length < 1 || self.graf.visual.label_max_length > 60 {
            errs.push(format!(
                "graf.visual.label_max_length must be 1-60, got {}",
                self.graf.visual.label_max_length
            ));
        }
        if self.graf.visual.node_size < 1.0 || self.graf.visual.node_size > 5.0 {
            errs.push(format!(
                "graf.visual.node_size must be 1.0-5.0, got {}",
                self.graf.visual.node_size
            ));
        }
        if self.graf.visual.edge_thickness < 1 || self.graf.visual.edge_thickness > 3 {
            errs.push(format!(
                "graf.visual.edge_thickness must be 1-3, got {}",
                self.graf.visual.edge_thickness
            ));
        }
        if self.list.sections.len() > 2 {
            errs.push(format!(
                "list.sections has {} entries, max is 2",
                self.list.sections.len()
            ));
        }
        {
            let mut seen = std::collections::HashSet::new();
            for s in &self.list.sections {
                if !seen.insert(s) {
                    errs.push(format!("list.sections contains duplicate: {s}"));
                }
            }
        }
        if self.list.sections.is_empty() {
            errs.push("list.sections is empty, will use defaults".to_string());
        }
        errs
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_default_config() {
        let config = ClinConfig::default();
        assert!(config.core.storage_path.is_none());
        assert!(!config.has_custom_storage_path());
    }

    #[test]
    fn test_set_storage_path() {
        let mut config = ClinConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));
        assert!(config.has_custom_storage_path());
        assert_eq!(
            config.core.storage_path,
            Some(PathBuf::from("/custom/path"))
        );
    }

    #[test]
    fn test_reset_storage_path() {
        let mut config = ClinConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));
        config.reset_storage_path();
        assert!(!config.has_custom_storage_path());
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut config = ClinConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ClinConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.core.storage_path, parsed.core.storage_path);
    }

    #[test]
    fn test_serde_defaults() {
        let toml_str = r#"
[graf.visual]
# all fields omitted
"#;
        let config: ClinConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.graf.visual.show_minimap);
        assert_eq!(config.graf.visual.node_color_mode, NodeColorMode::Folder);
        assert_eq!(config.graf.visual.edge_color_mode, EdgeColorMode::Uniform);
        assert_eq!(config.graf.visual.graph_background, Background::Solid);
    }

    #[test]
    fn test_unknown_field_tolerance() {
        let toml_str = r#"
[graf.physics]
damping = 0.5
unknown_field = "ignore me"
"#;
        let config: ClinConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.graf.physics.ideal_distance, 80.0);
    }

    #[test]
    fn test_new_fields_roundtrip() {
        let mut config = ClinConfig::default();
        config.core.mouse_enabled = false;
        config.list.date_format = "%d/%m/%Y".to_string();
        config.list.density = ListDensity::Compact;
        config.list.show_file_size = true;
        config.list.show_date_in_list = false;
        config.list.default_view = NotesLayout::Tree;
        config.list.calendar_enabled = false;
        config.backup.auto_backup_interval = Some(60);

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ClinConfig = toml::from_str(&toml_str).unwrap();

        assert!(!parsed.core.mouse_enabled);
        assert_eq!(parsed.list.date_format, "%d/%m/%Y");
        assert_eq!(parsed.list.density, ListDensity::Compact);
        assert!(parsed.list.show_file_size);
        assert!(!parsed.list.show_date_in_list);
        assert_eq!(parsed.list.default_view, NotesLayout::Tree);
        assert!(!parsed.list.calendar_enabled);
        assert_eq!(parsed.backup.auto_backup_interval, Some(60));
    }

    #[test]
    fn calendar_defaults_enabled_when_key_omitted() {
        // A [list] section that omits calendar_enabled must deserialize to true
        // (visible by default), matching #[serde(default = "default_true")].
        // (Like preview_enabled/show_date_in_list, ListConfig's derived Default
        // yields false for bools — the on-disk/serde path is what users hit.)
        let cfg: ClinConfig = toml::from_str("[list]\npreview_enabled = false\n").unwrap();
        assert!(cfg.list.calendar_enabled);

        // Explicitly setting it false also survives a round-trip.
        let cfg2: ClinConfig = toml::from_str("[list]\ncalendar_enabled = false\n").unwrap();
        assert!(!cfg2.list.calendar_enabled);
    }

    #[test]
    fn backup_defaults_disabled_when_keys_omitted() {
        // A [backup] section that omits the enable flags must default to off.
        let cfg: ClinConfig = toml::from_str("[backup]\nauto_push = false\n").unwrap();
        assert!(!cfg.backup.enabled);
        assert!(!cfg.backup.backup_on_save);
        assert!(!cfg.backup.backup_on_quit);
    }

    #[test]
    fn test_migration_logic() {
        let toml_str = r###"
external_editor = "nvim"
preview_position = "left"
mouse_enabled = false
confirm_on_quit = true

[visual]
notes_layout = "tree"

[physics]
ideal_distance = 120.0

[theme]
theme = "tokyo_night"

[display]
show_status_bar = false
"###;
        let mut value: toml::Value = toml::from_str(toml_str).unwrap();

        // 1. Move notes_layout to default_view
        if let Some(visual) = value.get_mut("visual").and_then(|v| v.as_table_mut())
            && let Some(notes_layout) = visual.remove("notes_layout")
            && value.get("default_view").is_none()
            && let Some(root) = value.as_table_mut()
        {
            root.insert("default_view".to_string(), notes_layout);
        }

        // 2. Map legacy editor keys to editor namespace
        let mut editor_table = toml::value::Table::new();
        if let Some(root) = value.as_table_mut()
            && let Some(v) = root.remove("external_editor")
        {
            editor_table.insert("external_command".to_string(), v);
        }
        if !editor_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_editor) = root.get_mut("editor").and_then(|e| e.as_table_mut()) {
                for (k, v) in editor_table {
                    existing_editor.insert(k, v);
                }
            } else {
                root.insert("editor".to_string(), toml::Value::Table(editor_table));
            }
        }

        // 3. Map legacy list keys to list namespace
        let mut list_table = toml::value::Table::new();
        let list_legacy_keys = [
            ("preview_position", "preview_position"),
            ("default_view", "default_view"),
        ];
        if let Some(root) = value.as_table_mut() {
            for (old_key, new_key) in &list_legacy_keys {
                if let Some(v) = root.remove(*old_key) {
                    list_table.insert(new_key.to_string(), v);
                }
            }
        }
        if !list_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_list) = root.get_mut("list").and_then(|l| l.as_table_mut()) {
                for (k, v) in list_table {
                    existing_list.insert(k, v);
                }
            } else {
                root.insert("list".to_string(), toml::Value::Table(list_table));
            }
        }

        // 4. Map legacy core keys to core namespace
        let mut core_table = toml::value::Table::new();
        let core_legacy_keys = [
            "storage_path",
            "previous_storage_path",
            "mouse_enabled",
            "default_folder",
            "confirm_on_delete",
            "confirm_on_quit",
        ];
        if let Some(root) = value.as_table_mut() {
            for key in &core_legacy_keys {
                if let Some(v) = root.remove(*key) {
                    core_table.insert(key.to_string(), v);
                }
            }
        }
        if !core_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_core) = root.get_mut("core").and_then(|c| c.as_table_mut()) {
                for (k, v) in core_table {
                    existing_core.insert(k, v);
                }
            } else {
                root.insert("core".to_string(), toml::Value::Table(core_table));
            }
        }

        // 5. Nest visual, physics, interaction, filter under graf
        let mut graf_addons = toml::value::Table::new();
        let graf_keys = ["visual", "physics", "interaction", "filter"];
        for key in &graf_keys {
            if let Some(val) = value.as_table_mut().and_then(|t| t.remove(*key)) {
                graf_addons.insert(key.to_string(), val);
            }
        }
        if !graf_addons.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_graf) = root.get_mut("graf").and_then(|g| g.as_table_mut()) {
                for (k, v) in graf_addons {
                    existing_graf.insert(k, v);
                }
            } else {
                root.insert("graf".to_string(), toml::Value::Table(graf_addons));
            }
        }

        // 6. Map legacy theme and display tables to ui namespace
        let mut ui_table = toml::value::Table::new();
        if let Some(root) = value.as_table_mut() {
            if let Some(theme) = root.remove("theme")
                && let Some(t) = theme.as_table()
            {
                for (k, v) in t {
                    ui_table.insert(k.clone(), v.clone());
                }
            }
            if let Some(display) = root.remove("display")
                && let Some(d) = display.as_table()
            {
                for (k, v) in d {
                    ui_table.insert(k.clone(), v.clone());
                }
            }
        }
        if !ui_table.is_empty()
            && let Some(root) = value.as_table_mut()
        {
            if let Some(existing_ui) = root.get_mut("ui").and_then(|u| u.as_table_mut()) {
                for (k, v) in ui_table {
                    existing_ui.insert(k, v);
                }
            } else {
                root.insert("ui".to_string(), toml::Value::Table(ui_table));
            }
        }

        let migrated_toml = toml::to_string(&value).unwrap();
        let config: ClinConfig = toml::from_str(&migrated_toml).unwrap();
        assert_eq!(config.list.default_view, NotesLayout::Tree);
        assert_eq!(config.graf.physics.ideal_distance, 120.0);
        assert_eq!(config.editor.external_command, Some("nvim".to_string()));
        assert_eq!(config.list.preview_position, PreviewPosition::Left);
        assert!(!config.core.mouse_enabled);
        assert!(config.core.confirm_on_quit);
        assert_eq!(config.ui.theme, Theme::TokyoNight);
        assert!(!config.ui.show_status_bar);
    }

    #[test]
    fn default_config_template_parses_and_calendar_visible_by_default() {
        // The embedded default template is what a first-run user gets. It must
        // be valid ClinConfig TOML and ship with the calendar visible.
        let config: ClinConfig = toml::from_str(merge::default_config_content()).unwrap();
        assert!(config.list.calendar_enabled);
        // Sanity: a few other shipped defaults still hold.
        assert!(config.list.preview_enabled);
    }

    #[test]
    fn test_goals_config_deserialization() {
        let config: ClinConfig = toml::from_str(merge::default_config_content()).unwrap();
        assert!(config.goals.enabled);
        assert_eq!(config.goals.word_goal, 500);
        assert_eq!(config.goals.note_goal, 3);

        let empty_config: ClinConfig = toml::from_str("").unwrap();
        assert!(empty_config.goals.enabled);
        assert_eq!(empty_config.goals.word_goal, 500);
        assert_eq!(empty_config.goals.note_goal, 3);
    }

    #[test]
    fn test_merge_toml_value_preserves_comments() {
        let initial_toml = merge::default_config_content();

        let mut doc = initial_toml.parse::<toml_edit::DocumentMut>().unwrap();

        let mut config = ClinConfig::default();
        config.core.mouse_enabled = false;
        config.ui.show_status_bar = false;
        config.list.preview_enabled = false;
        config.graf.visual.show_grid = true;

        let self_toml_str = toml::to_string(&config).unwrap();
        let self_value: toml::Value = toml::from_str(&self_toml_str).unwrap();

        if let toml::Value::Table(toml_tbl) = self_value {
            for (k, v) in toml_tbl {
                if doc.contains_key(&k) {
                    merge::merge_toml_value(doc.get_mut(&k).unwrap(), &v);
                }
            }
        }

        let merged_str = doc.to_string();
        assert!(merged_str.contains("# Clin Configuration File"));
        assert!(merged_str.contains("# Enable mouse support (clicking, scrolling, panning)."));
        assert!(merged_str.contains("# Show the status bar at the bottom of the screen."));
        assert!(merged_str.contains("# Show background grid."));
        assert!(merged_str.contains("mouse_enabled = false"));
        assert!(merged_str.contains("show_status_bar = false"));
        assert!(merged_str.contains("preview_enabled = false"));
        assert!(merged_str.contains("show_grid = true"));
    }

    #[test]
    fn test_actual_save_preserves_comments() {
        let _lock = CONFIG_TEST_MUTEX.lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file_path = temp_dir.path().join("config.toml");

        set_config_path_override(config_file_path.clone());

        let mut config = ClinConfig::load().unwrap();
        assert!(config_file_path.exists());

        let initial_content = fs::read_to_string(&config_file_path).unwrap();
        assert!(initial_content.contains("# Enable mouse support (clicking, scrolling, panning)."));
        assert!(initial_content.contains("mouse_enabled = true"));

        config.core.mouse_enabled = false;
        config.save().unwrap();

        let saved_content = fs::read_to_string(&config_file_path).unwrap();
        assert!(saved_content.contains("# Enable mouse support (clicking, scrolling, panning)."));
        assert!(saved_content.contains("mouse_enabled = false"));
    }

    #[test]
    fn test_sections_default() {
        let toml_str = r#"
[list]
"#;
        let config: ClinConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.list.sections,
            vec![NotesSection::Calendar, NotesSection::Goals]
        );
    }

    #[test]
    fn test_sections_roundtrip() {
        let toml_str = r#"
[list]
sections = ["draw", "graf"]
"#;
        let config: ClinConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.list.sections,
            vec![NotesSection::Draw, NotesSection::Graf]
        );

        let serialized = toml::to_string(&config).unwrap();
        let parsed: ClinConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(
            parsed.list.sections,
            vec![NotesSection::Draw, NotesSection::Graf]
        );
    }

    #[test]
    fn test_sections_clamp() {
        let toml_str = r#"
[list]
sections = ["calendar", "goals", "draw"]
"#;
        let mut config: ClinConfig = toml::from_str(toml_str).unwrap();
        config.normalize_sections();
        assert_eq!(config.list.sections.len(), 2);
        assert_eq!(config.list.sections[0], NotesSection::Calendar);
        assert_eq!(config.list.sections[1], NotesSection::Goals);
    }

    #[test]
    fn test_sections_empty_fallback() {
        let toml_str = r#"
[list]
sections = []
"#;
        let mut config: ClinConfig = toml::from_str(toml_str).unwrap();
        config.normalize_sections();
        assert_eq!(
            config.list.sections,
            vec![NotesSection::Calendar, NotesSection::Goals]
        );
    }

    #[test]
    fn test_sections_duplicates_removed() {
        let toml_str = r#"
[list]
sections = ["draw", "draw", "graf"]
"#;
        let mut config: ClinConfig = toml::from_str(toml_str).unwrap();
        config.normalize_sections();
        // "draw" kept once, truncated to 2 → [draw, graf]
        assert_eq!(config.list.sections.len(), 2);
        assert_eq!(config.list.sections[0], NotesSection::Draw);
        assert_eq!(config.list.sections[1], NotesSection::Graf);
    }
}
