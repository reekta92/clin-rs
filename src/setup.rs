//! First-run setup wizard state.
//!
//! Centered screen: CLIN ASCII logo, vault selection, five cycle-in-place
//! options, help hint, and a Done button. Visual changes are live-applied via
//! `App::apply_setup_live`.

use std::path::PathBuf;

/// Option rows shown below the logo. The last selectable row is the Done
/// button, so selectable indices run `0..=DONE_ROW`.
pub const OPTION_ROWS: usize = 8;
pub const DONE_ROW: usize = 8;

pub const SETUP_THEMES: &[&str] = &[
    "default",
    "tokyo_night",
    "catppuccin_mocha",
    "onedark",
    "gruvbox",
    "dracula",
    "nord",
    "rose_pine",
    "everforest",
    "kanagawa",
    "solarized",
    "catppuccin_frappe",
    "catppuccin_macchiato",
    "rose_pine_moon",
    "gruvbox_material",
    "github_dark",
    "ayu_mirage",
    "synthwave",
    "material",
];
pub const SETUP_PRESETS: &[&str] = &["default", "helix", "vim", "emacs"];
pub const SETUP_ICON_MODES: &[&str] = &["nerd_font", "unicode", "none"];
pub const SETUP_HINT_STYLES: &[&str] = &[
    "Classic", "Sharp", "Rounded", "Slanted", "Bubbles", "Blurred", "Chips", "Brackets", "Compact",
];
pub const SETUP_LAYOUTS: &[&str] = &["Grid", "Tree"];
pub const SETUP_FEATURE_PRESETS: &[&str] = &["Default", "Expanded", "Minimal", "Custom"];

/// Feature preset for the setup wizard's Features row. `Custom` keeps the
/// user's per-feature toggles; the others are fixed sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeaturePreset {
    Default,
    Expanded,
    Minimal,
    Custom,
}

/// Display names for the 16 [`crate::config::FeaturesConfig`] fields, in field
/// order. `get_feature`/`set_feature` index into this list.
pub const FEATURE_NAMES: &[&str] = &[
    "Graph View",
    "Canvas View",
    "Draw View",
    "Outline View",
    "Help View",
    "Tags",
    "Trash",
    "Subnotes",
    "Templates",
    "Import",
    "Encryption",
    "Images",
    "Backup",
    "Goals",
    "Calendar",
    "Smart Folders",
];

pub const CLIN_ASCII: &str = concat!(
    "          ██   ██\n",
    "   ████   ██        █████\n",
    " ██       ██   ██   ██   ██\n",
    " ██       ██   ██   ██   ██\n",
    "   ████   ██   ██   ██   ██",
);
pub const LOGO_CURSOR_ASCII: &str = "\
████
████
████
████
████";
const LOGO_BLINK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

pub fn icon_mode_at(idx: usize) -> crate::config::IconMode {
    match idx {
        1 => crate::config::IconMode::Unicode,
        2 => crate::config::IconMode::None,
        _ => crate::config::IconMode::Nerd,
    }
}
pub fn icon_mode_index(m: crate::config::IconMode) -> usize {
    match m {
        crate::config::IconMode::Nerd => 0,
        crate::config::IconMode::Unicode => 1,
        crate::config::IconMode::None => 2,
    }
}

pub fn hint_style_at(idx: usize) -> crate::config::HintBarStyle {
    crate::config::HintBarStyle::from_index(idx)
}
pub fn hint_style_index(s: crate::config::HintBarStyle) -> usize {
    s.index()
}

pub fn layout_at(idx: usize) -> crate::config::NotesLayout {
    match idx {
        1 => crate::config::NotesLayout::Tree,
        _ => crate::config::NotesLayout::Grid,
    }
}
pub fn layout_index(l: &crate::config::NotesLayout) -> usize {
    match l {
        crate::config::NotesLayout::Tree => 1,
        crate::config::NotesLayout::Grid => 0,
    }
}

/// The fixed [`crate::config::FeaturesConfig`] for a preset. `Custom` is not
/// handled here — callers use `SetupState.custom_features` directly.
pub fn features_for_preset(preset: FeaturePreset) -> crate::config::FeaturesConfig {
    use crate::config::FeatureState::Disabled;
    let mut f = crate::config::FeaturesConfig::default();
    match preset {
        FeaturePreset::Default => {}
        FeaturePreset::Minimal => {
            for state in [
                &mut f.graph_view,
                &mut f.canvas_view,
                &mut f.draw_view,
                &mut f.outline_view,
                &mut f.trash,
                &mut f.subnotes,
                &mut f.import,
                &mut f.encryption,
                &mut f.backup,
                &mut f.goals,
                &mut f.calendar,
                &mut f.smart_folders,
            ] {
                *state = Disabled;
            }
        }
        FeaturePreset::Expanded => {
            f.graph_view = Disabled;
            f.canvas_view = Disabled;
            f.draw_view = Disabled;
            f.backup = Disabled;
        }
        FeaturePreset::Custom => {}
    }
    f
}

/// Detect which preset a live config matches, else `Custom`.
pub fn detect_preset(features: &crate::config::FeaturesConfig) -> FeaturePreset {
    if *features == crate::config::FeaturesConfig::default() {
        FeaturePreset::Default
    } else if *features == features_for_preset(FeaturePreset::Expanded) {
        FeaturePreset::Expanded
    } else if *features == features_for_preset(FeaturePreset::Minimal) {
        FeaturePreset::Minimal
    } else {
        FeaturePreset::Custom
    }
}

/// Feature at `idx` in [`FEATURE_NAMES`] order. `idx >= 16` yields `None`.
pub fn get_feature(
    f: &crate::config::FeaturesConfig,
    idx: usize,
) -> Option<crate::config::FeatureState> {
    Some(match idx {
        0 => f.graph_view,
        1 => f.canvas_view,
        2 => f.draw_view,
        3 => f.outline_view,
        4 => f.help_view,
        5 => f.tags,
        6 => f.trash,
        7 => f.subnotes,
        8 => f.templates,
        9 => f.import,
        10 => f.encryption,
        11 => f.images,
        12 => f.backup,
        13 => f.goals,
        14 => f.calendar,
        15 => f.smart_folders,
        _ => return None,
    })
}

/// Set feature at `idx` in [`FEATURE_NAMES`] order. Out-of-range is a no-op.
pub fn set_feature(
    f: &mut crate::config::FeaturesConfig,
    idx: usize,
    state: crate::config::FeatureState,
) {
    match idx {
        0 => f.graph_view = state,
        1 => f.canvas_view = state,
        2 => f.draw_view = state,
        3 => f.outline_view = state,
        4 => f.help_view = state,
        5 => f.tags = state,
        6 => f.trash = state,
        7 => f.subnotes = state,
        8 => f.templates = state,
        9 => f.import = state,
        10 => f.encryption = state,
        11 => f.images = state,
        12 => f.backup = state,
        13 => f.goals = state,
        14 => f.calendar = state,
        15 => f.smart_folders = state,
        _ => {}
    }
}

/// Feature rows visible in the Custom preview panel: total inner height minus
/// the bottom hint line and one padding row. Shared by the input handler and
/// `draw_preview_features` so the scroll window matches the rendered window.
pub fn feature_visible_rows(inner_h: u16) -> usize {
    (inner_h.saturating_sub(2) as usize).max(1)
}

/// Build the full theme list for the setup wizard: built-in baseline ordered by
/// `SETUP_THEMES`, then custom themes from `~/.config/clin/themes/`. Returns
/// `(themes, is_custom)` where `is_custom[i]` is true for user-installed themes.
pub fn build_theme_list() -> (Vec<String>, Vec<bool>) {
    let builtin_count = SETUP_THEMES.len();
    let mut themes: Vec<String> = SETUP_THEMES.iter().map(|s| s.to_string()).collect();
    themes.extend(crate::config::custom_themes::list_custom_themes());
    let is_custom: Vec<bool> = (0..themes.len()).map(|i| i >= builtin_count).collect();
    (themes, is_custom)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SetupPreviewKey {
    pub cols: u16,
    pub theme: crate::markdown::MarkdownTheme,
    pub opts: crate::markdown::MdRenderOpts,
}

pub(crate) struct SetupRebootstrapRequest {
    pub storage: crate::storage::Storage,
    pub warnings: Vec<String>,
    pub previous_config: crate::config::ClinConfig,
    pub previous_path: PathBuf,
    pub selected_path: PathBuf,
}
#[derive(Debug)]
pub enum SetupVaultModal {
    PathInput {
        input: Box<ratatui_textarea::TextArea<'static>>,
        notice: Option<String>,
    },
    ConfirmNonEmpty {
        path: PathBuf,
    },
}

#[derive(Debug)]
pub struct SetupState {
    pub theme: usize,
    pub themes: Vec<String>,
    pub is_custom: Vec<bool>,
    pub background_solid: bool,
    pub hint_bar_style: usize,
    pub icon_mode: usize,
    pub keybind_preset: usize,
    pub notes_layout: usize,
    pub feature_preset: usize,
    pub custom_features: crate::config::FeaturesConfig,
    pub feature_scroll: usize,
    pub feature_cursor: usize,
    pub selected: usize,
    pub confirm_exit: bool,
    pub vault_path: PathBuf,
    pub initial_vault_path: PathBuf,
    pub vault_cli_override: bool,
    pub vault_modal: Option<SetupVaultModal>,
    pub confirmed_nonempty_path: Option<PathBuf>,
    pub vault_error: Option<String>,

    pub(crate) preview_renderer: crate::markdown::MarkdownRenderer,
    pub(crate) preview_key: Option<SetupPreviewKey>,
    pub(crate) pending_preview_resize: Option<(u16, std::time::Instant)>,
    pub(crate) logo_blink_started: std::time::Instant,
}

impl SetupState {
    /// Build from live config and active vault so re-runs preserve current choices.
    pub fn from_config(
        config: &crate::config::ClinConfig,
        _theme: &crate::app_theme::AppThemeColors,
        vault_path: PathBuf,
        vault_cli_override: bool,
    ) -> Self {
        let (themes, is_custom) = build_theme_list();
        let theme = themes
            .iter()
            .position(|t| config.ui.theme.as_str() == t.as_str())
            .unwrap_or(0);
        Self {
            theme,
            themes,
            is_custom,
            background_solid: matches!(config.ui.background, crate::config::Background::Solid),
            hint_bar_style: hint_style_index(config.ui.hint_bar_style),
            icon_mode: icon_mode_index(config.ui.icon_mode),
            keybind_preset: match config.core.keybind_preset {
                crate::config::KeybindPreset::Default => 0,
                crate::config::KeybindPreset::Helix => 1,
                crate::config::KeybindPreset::Vim => 2,
                crate::config::KeybindPreset::Emacs => 3,
            },
            notes_layout: layout_index(&config.list.default_view),
            feature_preset: match detect_preset(&config.features) {
                FeaturePreset::Default => 0,
                FeaturePreset::Expanded => 1,
                FeaturePreset::Minimal => 2,
                FeaturePreset::Custom => 3,
            },
            custom_features: config.features.clone(),
            feature_scroll: 0,
            feature_cursor: 0,
            selected: usize::from(vault_cli_override),
            confirm_exit: false,
            initial_vault_path: vault_path.clone(),
            vault_path,
            vault_cli_override,
            vault_modal: None,
            confirmed_nonempty_path: None,
            vault_error: None,
            preview_renderer: crate::markdown::MarkdownRenderer::new(),
            preview_key: None,
            pending_preview_resize: None,
            logo_blink_started: std::time::Instant::now(),
        }
    }

    /// Whether the five-row terminal block cursor is visible in this frame.
    pub fn logo_cursor_visible_at(&self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.logo_blink_started);
        (elapsed.as_millis() / LOGO_BLINK_INTERVAL.as_millis()).is_multiple_of(2)
    }

    pub fn is_done_selected(&self) -> bool {
        self.selected == DONE_ROW
    }

    pub fn vault_selected(&self) -> bool {
        self.selected == 0 && !self.vault_cli_override
    }

    /// Move selection up/down, skipping disabled Vault row under `--vault`.
    pub fn move_sel(&mut self, down: bool) {
        if down {
            self.selected = (self.selected + 1).min(DONE_ROW);
        } else {
            self.selected = self.selected.saturating_sub(1);
        }
        if self.vault_cli_override && self.selected == 0 {
            self.selected = 1;
        }
    }

    /// Cycle selected option. Vault and Done have no cycle operation.
    pub fn cycle(&mut self, forward: bool) {
        match self.selected {
            1 => {
                let len = self.themes.len();
                self.theme = if forward {
                    (self.theme + 1) % len
                } else {
                    (self.theme + len - 1) % len
                };
            }
            2 => self.background_solid = !self.background_solid,
            3 => {
                let len = SETUP_HINT_STYLES.len();
                self.hint_bar_style = if forward {
                    (self.hint_bar_style + 1) % len
                } else {
                    (self.hint_bar_style + len - 1) % len
                };
            }
            4 => {
                let len = SETUP_ICON_MODES.len();
                self.icon_mode = if forward {
                    (self.icon_mode + 1) % len
                } else {
                    (self.icon_mode + len - 1) % len
                };
            }
            5 => {
                let len = SETUP_LAYOUTS.len();
                self.notes_layout = if forward {
                    (self.notes_layout + 1) % len
                } else {
                    (self.notes_layout + len - 1) % len
                };
            }
            6 => {
                let len = SETUP_FEATURE_PRESETS.len();
                self.feature_preset = if forward {
                    (self.feature_preset + 1) % len
                } else {
                    (self.feature_preset + len - 1) % len
                };
            }
            7 => {
                let len = SETUP_PRESETS.len();
                self.keybind_preset = if forward {
                    (self.keybind_preset + 1) % len
                } else {
                    (self.keybind_preset + len - 1) % len
                };
            }
            _ => {}
        }
    }

    /// Whether the Features row is selected and its preset is Custom.
    pub fn custom_features_active(&self) -> bool {
        self.selected == 6 && self.feature_preset == 3
    }

    /// Move the Custom-mode feature cursor. Returns `false` at a boundary so
    /// the caller can fall through to row navigation.
    pub fn move_feature_cursor(&mut self, down: bool, visible: usize) -> bool {
        if down {
            if self.feature_cursor + 1 < FEATURE_NAMES.len() {
                self.feature_cursor += 1;
            } else {
                return false;
            }
        } else if self.feature_cursor > 0 {
            self.feature_cursor -= 1;
        } else {
            return false;
        }
        self.clamp_feature_scroll(visible);
        true
    }

    /// Keep the cursor inside the rendered scroll window.
    fn clamp_feature_scroll(&mut self, visible: usize) {
        let visible = visible.max(1);
        if self.feature_cursor < self.feature_scroll {
            self.feature_scroll = self.feature_cursor;
        } else if self.feature_cursor >= self.feature_scroll + visible {
            self.feature_scroll = self.feature_cursor + 1 - visible;
        }
    }

    /// Toggle the feature at the cursor (Enabled↔Disabled; Deleted→Enabled).
    pub fn toggle_feature_cursor(&mut self) {
        if let Some(state) = get_feature(&self.custom_features, self.feature_cursor) {
            set_feature(&mut self.custom_features, self.feature_cursor, !state);
        }
    }

    /// Select `idx` (mouse click) and toggle it, keeping it in the window.
    pub fn toggle_feature_at(&mut self, idx: usize, visible: usize) {
        self.feature_cursor = idx;
        self.clamp_feature_scroll(visible);
        self.toggle_feature_cursor();
    }

    pub fn row_label(row: usize) -> &'static str {
        match row {
            0 => "Vault",
            1 => "Theme",
            2 => "Background",
            3 => "Hint bar",
            4 => "Icons",
            5 => "Layout",
            6 => "Features",
            7 => "Keybinds",
            _ => "",
        }
    }

    pub fn row_value(&self, row: usize) -> String {
        match row {
            0 if self.vault_cli_override => format!("{} [CLI override]", self.vault_path.display()),
            0 => self.vault_path.display().to_string(),
            1 => {
                let name = self.themes[self.theme].clone();
                if *self.is_custom.get(self.theme).unwrap_or(&false) {
                    format!("{name} [custom]")
                } else {
                    name
                }
            }
            2 => {
                if self.background_solid {
                    "Solid".to_string()
                } else {
                    "Transparent".to_string()
                }
            }
            3 => SETUP_HINT_STYLES[self.hint_bar_style].to_string(),
            4 => SETUP_ICON_MODES[self.icon_mode].to_string(),
            5 => SETUP_LAYOUTS[self.notes_layout].to_string(),
            6 => SETUP_FEATURE_PRESETS[self.feature_preset].to_string(),
            7 => SETUP_PRESETS[self.keybind_preset].to_string(),
            _ => String::new(),
        }
    }
}

/// Validate a vault path without resolving symlinks or creating anything.
pub fn validate_vault_path(input: &str) -> anyhow::Result<PathBuf> {
    let path = crate::config::expand_path(input.trim());
    if !path.is_absolute() {
        anyhow::bail!("Storage path must be absolute: {}", input.trim());
    }
    if path.exists() && !path.is_dir() {
        anyhow::bail!("Storage path is not a directory: {}", path.display());
    }
    Ok(path)
}

/// Non-empty directories need confirmation unless clin already owns metadata.
pub fn vault_requires_confirmation(path: &std::path::Path) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    for entry in path.read_dir()? {
        if entry?.file_name() == ".clin" {
            return Ok(false);
        }
    }
    Ok(path.read_dir()?.next().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_wraps_each_row() {
        let mut s = SetupState::from_config(
            &crate::config::ClinConfig::default(),
            &crate::app_theme::AppThemeColors::default(),
            PathBuf::from("/vault"),
            false,
        );

        // Vault does not cycle.
        s.cycle(true);
        assert_eq!(s.vault_path, PathBuf::from("/vault"));

        // Theme wraps forward.
        s.selected = 1;
        s.cycle(true);
        assert_eq!(s.theme, 1);
        s.theme = s.themes.len() - 1;
        s.cycle(true);
        assert_eq!(s.theme, 0);

        // Theme wraps backward.
        s.cycle(false);
        assert_eq!(s.theme, s.themes.len() - 1);

        // Background flips.
        s.selected = 2;
        s.cycle(true);
        assert!(s.background_solid);
        s.cycle(false);
        assert!(!s.background_solid);

        // Hint bar wraps.
        s.selected = 3;
        s.hint_bar_style = SETUP_HINT_STYLES.len() - 1;
        s.cycle(true);
        assert_eq!(s.hint_bar_style, 0);

        // Icon mode wraps.
        s.selected = 4;
        s.icon_mode = SETUP_ICON_MODES.len() - 1;
        s.cycle(true);
        assert_eq!(s.icon_mode, 0);

        // Layout wraps.
        s.selected = 5;
        s.notes_layout = SETUP_LAYOUTS.len() - 1;
        s.cycle(true);
        assert_eq!(s.notes_layout, 0);
        s.cycle(false);
        assert_eq!(s.notes_layout, SETUP_LAYOUTS.len() - 1);

        // Feature preset wraps.
        s.selected = 6;
        s.feature_preset = SETUP_FEATURE_PRESETS.len() - 1;
        s.cycle(true);
        assert_eq!(s.feature_preset, 0);

        // Keybind preset wraps.
        s.selected = 7;
        s.keybind_preset = SETUP_PRESETS.len() - 1;
        s.cycle(true);
        assert_eq!(s.keybind_preset, 0);

        // Done row: no-op.
        s.selected = DONE_ROW;
        s.cycle(true);
        assert_eq!(s.keybind_preset, 0);
    }

    #[test]
    fn move_sel_clamps() {
        let mut s = SetupState::from_config(
            &crate::config::ClinConfig::default(),
            &crate::app_theme::AppThemeColors::default(),
            PathBuf::from("/vault"),
            false,
        );
        s.move_sel(false);
        assert_eq!(s.selected, 0);
        for _ in 0..=DONE_ROW {
            s.move_sel(true);
        }
        assert_eq!(s.selected, DONE_ROW);
    }

    #[test]
    fn logo_cursor_blinks_every_half_second() {
        let state = SetupState::from_config(
            &crate::config::ClinConfig::default(),
            &crate::app_theme::AppThemeColors::default(),
            PathBuf::from("/vault"),
            false,
        );
        let start = state.logo_blink_started;
        assert!(state.logo_cursor_visible_at(start));
        assert!(!state.logo_cursor_visible_at(start + LOGO_BLINK_INTERVAL));
        assert!(state.logo_cursor_visible_at(start + LOGO_BLINK_INTERVAL * 2));
    }

    #[test]
    fn setup_vault_path_rejects_relative() {
        assert!(
            validate_vault_path("relative/path")
                .unwrap_err()
                .to_string()
                .starts_with("Storage path must be absolute:")
        );
    }

    #[test]
    fn setup_vault_confirmation_classifies_empty_clin_and_unfamiliar() {
        let root = tempfile::tempdir().unwrap();
        assert!(!vault_requires_confirmation(root.path()).unwrap());
        std::fs::create_dir(root.path().join(".clin")).unwrap();
        assert!(!vault_requires_confirmation(root.path()).unwrap());
        let unfamiliar = tempfile::tempdir().unwrap();
        std::fs::write(unfamiliar.path().join(".obsidian"), "").unwrap();
        assert!(vault_requires_confirmation(unfamiliar.path()).unwrap());
    }

    #[test]
    fn setup_cli_override_disables_vault_row() {
        let state = SetupState::from_config(
            &crate::config::ClinConfig::default(),
            &crate::app_theme::AppThemeColors::default(),
            PathBuf::from("/override"),
            true,
        );
        assert_eq!(state.selected, 1);
        assert!(!state.vault_selected());
        assert!(state.row_value(0).contains("[CLI override]"));
    }

    #[cfg(unix)]
    #[test]
    fn setup_vault_path_preserves_absolute_symlink() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        let link = root.path().join("link");
        std::fs::create_dir(&target).unwrap();
        symlink(&target, &link).unwrap();
        assert_eq!(
            validate_vault_path(&link.display().to_string()).unwrap(),
            link
        );
    }

    #[test]
    fn feature_presets_roundtrip() {
        use crate::config::FeatureState;
        for preset in [
            FeaturePreset::Default,
            FeaturePreset::Expanded,
            FeaturePreset::Minimal,
        ] {
            assert_eq!(detect_preset(&features_for_preset(preset)), preset);
        }

        let minimal = features_for_preset(FeaturePreset::Minimal);
        for enabled in [
            minimal.help_view,
            minimal.tags,
            minimal.templates,
            minimal.images,
        ] {
            assert!(enabled.is_enabled());
        }
        assert!(!minimal.graph_view.is_enabled());
        assert!(!minimal.backup.is_enabled());

        let expanded = features_for_preset(FeaturePreset::Expanded);
        for disabled in [
            expanded.graph_view,
            expanded.canvas_view,
            expanded.draw_view,
            expanded.backup,
        ] {
            assert!(!disabled.is_enabled());
        }
        assert!(expanded.outline_view.is_enabled());
        assert!(expanded.encryption.is_enabled());

        // A Deleted state never roundtrips to a preset.
        let custom = crate::config::FeaturesConfig {
            graph_view: FeatureState::Deleted,
            ..crate::config::FeaturesConfig::default()
        };
        assert_eq!(detect_preset(&custom), FeaturePreset::Custom);
    }

    #[test]
    fn feature_index_mapping_roundtrip() {
        use crate::config::FeatureState;
        let mut f = crate::config::FeaturesConfig::default();
        assert_eq!(get_feature(&f, 0), Some(FeatureState::Enabled));
        set_feature(&mut f, 0, FeatureState::Disabled);
        assert!(!f.graph_view.is_enabled());
        assert_eq!(get_feature(&f, 15), Some(FeatureState::Enabled));
        set_feature(&mut f, 15, FeatureState::Disabled);
        assert!(!f.smart_folders.is_enabled());
        assert_eq!(get_feature(&f, 16), None);
        set_feature(&mut f, 16, FeatureState::Enabled); // out-of-range no-op
        assert!(f.help_view.is_enabled());
    }
}
