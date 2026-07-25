//! First-run setup wizard state.
//!
//! Single centered screen: CLIN ASCII logo + 5 cycle-in-place option rows
//! (Theme, Background, Hint bar style, Icon mode, Keybind preset) + a Done
//! button. No title/status bars, no preview pane. Every change is live-applied
//! via `App::apply_setup_live`.

/// Option rows shown below the logo. The last selectable row is the Done
/// button, so selectable indices run `0..=DONE_ROW`.
pub const OPTION_ROWS: usize = 5;
pub const DONE_ROW: usize = 5;
pub const ROW_COUNT: usize = 6;

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
pub const SETUP_HINT_STYLES: &[&str] = &["Classic", "Sharp", "Rounded", "Slanted", "Bubbles", "Blurred", "Chips", "Brackets"];

pub const CLIN_ASCII: &str = "\
 ██████╗██╗     ██╗███╗   ██╗
██╔════╝██║     ██║████╗  ██║
██║     ██║     ██║██╔██╗ ██║
██║     ██║     ██║██║╚██╗██║
╚██████╗███████╗██║██║ ╚████║
 ╚═════╝╚══════╝╚═╝╚═╝  ╚═══╝ ";

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SetupPreviewKey {
    pub cols: u16,
    pub theme: crate::markdown::MarkdownTheme,
    pub opts: crate::markdown::MdRenderOpts,
}

#[derive(Debug)]
pub struct SetupState {
    pub theme: usize,
    pub background_solid: bool,
    pub hint_bar_style: usize,
    pub icon_mode: usize,
    pub keybind_preset: usize,
    pub selected: usize,
    pub confirm_exit: bool,

    pub(crate) preview_renderer: crate::markdown::MarkdownRenderer,
    pub(crate) preview_key: Option<SetupPreviewKey>,
    pub(crate) pending_preview_resize: Option<(u16, std::time::Instant)>,
}

impl SetupState {
    /// Build from the live config so a re-run (`--setup` / palette) pre-fills current values.
    pub fn from_config(
        config: &crate::config::ClinConfig,
        _theme: &crate::app_theme::AppThemeColors,
    ) -> Self {
        Self {
            theme: SETUP_THEMES
                .iter()
                .position(|t| config.ui.theme.as_str() == *t)
                .unwrap_or(0),
            background_solid: matches!(config.ui.background, crate::config::Background::Solid),
            hint_bar_style: hint_style_index(config.ui.hint_bar_style),
            icon_mode: icon_mode_index(config.ui.icon_mode),
            keybind_preset: match config.core.keybind_preset {
                crate::config::KeybindPreset::Default => 0,
                crate::config::KeybindPreset::Helix => 1,
                crate::config::KeybindPreset::Vim => 2,
                crate::config::KeybindPreset::Emacs => 3,
            },
            selected: 0,
            confirm_exit: false,
            preview_renderer: crate::markdown::MarkdownRenderer::new(),
            preview_key: None,
            pending_preview_resize: None,
        }
    }

    pub fn is_done_selected(&self) -> bool {
        self.selected == DONE_ROW
    }

    /// Move selection up/down, clamped to `0..=DONE_ROW`.
    pub fn move_sel(&mut self, down: bool) {
        if down {
            self.selected = (self.selected + 1).min(DONE_ROW);
        } else {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    /// Cycle the currently selected option's value. No-op on the Done row.
    pub fn cycle(&mut self, forward: bool) {
        match self.selected {
            0 => {
                let len = SETUP_THEMES.len();
                self.theme = if forward {
                    (self.theme + 1) % len
                } else {
                    (self.theme + len - 1) % len
                };
            }
            1 => self.background_solid = !self.background_solid,
            2 => {
                let len = SETUP_HINT_STYLES.len();
                self.hint_bar_style = if forward {
                    (self.hint_bar_style + 1) % len
                } else {
                    (self.hint_bar_style + len - 1) % len
                };
            }
            3 => {
                let len = SETUP_ICON_MODES.len();
                self.icon_mode = if forward {
                    (self.icon_mode + 1) % len
                } else {
                    (self.icon_mode + len - 1) % len
                };
            }
            4 => {
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

    /// Display label for a given option row.
    pub fn row_label(row: usize) -> &'static str {
        match row {
            0 => "Theme",
            1 => "Background",
            2 => "Hint bar",
            3 => "Icons",
            4 => "Keybinds",
            _ => "",
        }
    }

    /// Current value string for a given option row.
    pub fn row_value(&self, row: usize) -> String {
        match row {
            0 => SETUP_THEMES[self.theme].to_string(),
            1 => {
                if self.background_solid {
                    "Solid".to_string()
                } else {
                    "Transparent".to_string()
                }
            }
            2 => SETUP_HINT_STYLES[self.hint_bar_style].to_string(),
            3 => SETUP_ICON_MODES[self.icon_mode].to_string(),
            4 => SETUP_PRESETS[self.keybind_preset].to_string(),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_wraps_each_row() {
        let mut s = SetupState {
            theme: 0,
            background_solid: false,
            hint_bar_style: 0,
            icon_mode: 0,
            keybind_preset: 0,
            selected: 0,
            confirm_exit: false,
            preview_renderer: crate::markdown::MarkdownRenderer::new(),
            preview_key: None,
            pending_preview_resize: None,
        };

        // Theme wraps forward
        s.cycle(true);
        assert_eq!(s.theme, 1);
        s.theme = SETUP_THEMES.len() - 1;
        s.cycle(true);
        assert_eq!(s.theme, 0);

        // Theme wraps backward
        s.cycle(false);
        assert_eq!(s.theme, SETUP_THEMES.len() - 1);

        // Background flips
        s.selected = 1;
        s.cycle(true);
        assert!(s.background_solid);
        s.cycle(false);
        assert!(!s.background_solid);

        // Hint bar wraps
        s.selected = 2;
        s.hint_bar_style = SETUP_HINT_STYLES.len() - 1;
        s.cycle(true);
        assert_eq!(s.hint_bar_style, 0);

        // Icon mode wraps
        s.selected = 3;
        s.icon_mode = SETUP_ICON_MODES.len() - 1;
        s.cycle(true);
        assert_eq!(s.icon_mode, 0);

        // Keybind preset wraps
        s.selected = 4;
        s.keybind_preset = SETUP_PRESETS.len() - 1;
        s.cycle(true);
        assert_eq!(s.keybind_preset, 0);

        // Done row: no-op
        s.selected = DONE_ROW;
        s.cycle(true);
        assert_eq!(s.keybind_preset, 0);
    }

    #[test]
    fn move_sel_clamps() {
        let mut s = SetupState {
            theme: 0,
            background_solid: false,
            hint_bar_style: 0,
            icon_mode: 0,
            keybind_preset: 0,
            selected: 0,
            confirm_exit: false,
            preview_renderer: crate::markdown::MarkdownRenderer::new(),
            preview_key: None,
            pending_preview_resize: None,
        };
        s.move_sel(false);
        assert_eq!(s.selected, 0);
        for _ in 0..ROW_COUNT {
            s.move_sel(true);
        }
        assert_eq!(s.selected, DONE_ROW);
    }
}
