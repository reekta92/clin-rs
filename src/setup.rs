//! First-run setup wizard state.
//!
//! Centered screen: CLIN ASCII logo, vault selection, five cycle-in-place
//! options, help hint, and a Done button. Visual changes are live-applied via
//! `App::apply_setup_live`.

use std::path::PathBuf;

/// Option rows shown below the logo. The last selectable row is the Done
/// button, so selectable indices run `0..=DONE_ROW`.
pub const OPTION_ROWS: usize = 6;
pub const DONE_ROW: usize = 6;
pub const ROW_COUNT: usize = 7;

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
        input: ratatui_textarea::TextArea<'static>,
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

    pub fn row_label(row: usize) -> &'static str {
        match row {
            0 => "Vault",
            1 => "Theme",
            2 => "Background",
            3 => "Hint bar",
            4 => "Icons",
            5 => "Keybinds",
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
            5 => SETUP_PRESETS[self.keybind_preset].to_string(),
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

        // Keybind preset wraps.
        s.selected = 5;
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
        for _ in 0..ROW_COUNT {
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
}
