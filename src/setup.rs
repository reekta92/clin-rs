use ratatui_textarea::TextArea;

/// Highest step index (Done). `advance()`/`go_next()` clamp here.
pub const SETUP_TOTAL_STEPS: usize = 9;

/// Step labels for the sidebar. Index == step number.
pub const SETUP_STEPS: &[(&str, &str)] = &[
    ("Welcome", "\u{f015}"),
    ("Theme", "\u{f042}"),
    ("Keybinds", "\u{f11c}"),
    ("Mouse", "\u{f245}"),
    ("Density", "\u{f0c9}"),
    ("Hint Bar", "\u{f0e4}"),
    ("Daily Goals", "\u{f091}"),
    ("Auto-Backup", "\u{f0c0}"),
    ("Vault Path", "\u{f07b}"),
    ("Done", "\u{f00c}"),
];

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
];
pub const SETUP_PRESETS: &[&str] = &["default", "helix", "vim", "emacs"];
pub const SETUP_DENSITIES: &[&str] = &["compact", "comfortable"];
pub const SETUP_HINT_STYLES: &[&str] = &[
    "Classic",
    "Accent",
    "Powerline Sharp",
    "Powerline Rounded",
    "Powerline Slanted",
];

pub fn hint_style_at(idx: usize) -> crate::config::HintBarStyle {
    match idx {
        1 => crate::config::HintBarStyle::Accent,
        2 => crate::config::HintBarStyle::PowerlineSharp,
        3 => crate::config::HintBarStyle::PowerlineRounded,
        4 => crate::config::HintBarStyle::PowerlineSlanted,
        _ => crate::config::HintBarStyle::Classic,
    }
}

pub fn hint_style_index(s: crate::config::HintBarStyle) -> usize {
    match s {
        crate::config::HintBarStyle::Classic => 0,
        crate::config::HintBarStyle::Accent => 1,
        crate::config::HintBarStyle::PowerlineSharp => 2,
        crate::config::HintBarStyle::PowerlineRounded => 3,
        crate::config::HintBarStyle::PowerlineSlanted => 4,
    }
}
#[derive(Debug)]
pub struct SetupState {
    pub step: usize,
    pub cursor: usize,
    /// 0 = toggle, 1 = text — for the two two-field steps (6 Goals, 7 Backup).
    pub focus: usize,
    /// One entry per step; `true` once the user has visited it. Drives sidebar checkmarks.
    pub visited: Vec<bool>,

    pub theme: usize,
    pub background_solid: bool,
    pub keybind_preset: usize,
    pub mouse_enabled: bool,
    pub list_density: usize,
    pub goals_enabled: bool,
    pub backup_enabled: bool,
    pub hint_bar_style: usize,

    pub word_goal_input: TextArea<'static>,
    pub remote_url_input: TextArea<'static>,
    pub storage_path_input: TextArea<'static>,
}

impl SetupState {
    /// Build from the live config so a re-run (`--setup` / palette) pre-fills current values.
    pub fn from_config(
        config: &crate::config::ClinConfig,
        theme: &crate::app_theme::AppThemeColors,
    ) -> Self {
        let mut s = Self {
            step: 0,
            cursor: 0,
            focus: 0,
            visited: vec![false; SETUP_STEPS.len()],
            theme: SETUP_THEMES
                .iter()
                .position(|t| *t == config.ui.theme.to_string())
                .unwrap_or(0),
            background_solid: matches!(config.ui.background, crate::config::Background::Solid),
            keybind_preset: match config.core.keybind_preset {
                crate::config::KeybindPreset::Default => 0,
                crate::config::KeybindPreset::Helix => 1,
                crate::config::KeybindPreset::Vim => 2,
                crate::config::KeybindPreset::Emacs => 3,
            },
            mouse_enabled: config.core.mouse_enabled,
            list_density: match config.list.density {
                crate::config::ListDensity::Compact => 0,
                crate::config::ListDensity::Comfortable => 1,
            },
            goals_enabled: config.goals.enabled,
            backup_enabled: config.backup.enabled,
            hint_bar_style: hint_style_index(config.ui.hint_bar_style),
            word_goal_input: Self::make_input(
                theme,
                "Enter daily word goal (e.g. 500)",
                if config.goals.word_goal > 0 {
                    Some(config.goals.word_goal.to_string())
                } else {
                    None
                },
            ),
            remote_url_input: Self::make_input(
                theme,
                "Git remote URL (optional)",
                config.backup.remote_url.clone(),
            ),
            storage_path_input: Self::make_input(
                theme,
                "Vault path (~ and $VAR supported)",
                config
                    .core
                    .storage_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
            ),
        };
        s.visited[0] = true;
        s
    }

    fn make_input(
        theme: &crate::app_theme::AppThemeColors,
        placeholder: &str,
        initial: Option<String>,
    ) -> TextArea<'static> {
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_style(theme.bg_style());
        input.set_placeholder_text(placeholder);
        if let Some(text) = initial
            && !text.is_empty()
        {
            input.insert_str(&text);
        }
        input
    }

    fn mark_visited(&mut self) {
        if self.step < self.visited.len() {
            self.visited[self.step] = true;
        }
    }

    pub fn advance(&mut self) {
        self.go_to_step((self.step + 1).min(SETUP_TOTAL_STEPS));
    }
    pub fn go_next(&mut self) {
        self.advance();
    }
    pub fn go_prev(&mut self) {
        self.go_to_step(self.step.saturating_sub(1));
    }
    pub fn go_to_step(&mut self, step: usize) {
        self.step = step.min(SETUP_TOTAL_STEPS);
        self.cursor = self.default_cursor_for_step();
        self.focus = 0;
        self.mark_visited();
    }

    pub fn move_cursor(&mut self, down: bool) {
        match self.step {
            1 => {
                if self.focus == 0 {
                    self.cursor = if down {
                        (self.cursor + 1).min(SETUP_THEMES.len() - 1)
                    } else {
                        self.cursor.saturating_sub(1)
                    };
                    self.theme = self.cursor;
                } else {
                    self.background_solid = !self.background_solid;
                }
            }
            2 => {
                self.cursor = if down {
                    (self.cursor + 1).min(SETUP_PRESETS.len() - 1)
                } else {
                    self.cursor.saturating_sub(1)
                };
                self.keybind_preset = self.cursor;
            }
            3 => {
                self.mouse_enabled = !self.mouse_enabled;
            }
            4 => {
                self.cursor = if down {
                    (self.cursor + 1).min(SETUP_DENSITIES.len() - 1)
                } else {
                    self.cursor.saturating_sub(1)
                };
                self.list_density = self.cursor;
            }
            5 => {
                self.cursor = if down {
                    (self.cursor + 1).min(SETUP_HINT_STYLES.len() - 1)
                } else {
                    self.cursor.saturating_sub(1)
                };
                self.hint_bar_style = self.cursor;
            }
            6 => {
                if self.focus == 0 {
                    self.goals_enabled = !self.goals_enabled;
                }
            }
            7 => {
                if self.focus == 0 {
                    self.backup_enabled = !self.backup_enabled;
                }
            }
            _ => {}
        }
    }

    fn default_cursor_for_step(&self) -> usize {
        match self.step {
            1 => self.theme,
            2 => self.keybind_preset,
            4 => self.list_density,
            5 => self.hint_bar_style,
            _ => 0,
        }
    }

    pub fn is_toggle_active(&self) -> bool {
        (self.step == 1 && self.focus == 1)
            || self.step == 3
            || (self.step == 6 && self.focus == 0)
            || (self.step == 7 && self.focus == 0)
    }

    pub fn is_text_focused(&self) -> bool {
        ((self.step == 6 || self.step == 7) && self.focus == 1) || self.step == 8
    }
    pub fn focused_input_mut(&mut self) -> Option<&mut TextArea<'static>> {
        match self.step {
            6 if self.focus == 1 => Some(&mut self.word_goal_input),
            7 if self.focus == 1 => Some(&mut self.remote_url_input),
            8 => Some(&mut self.storage_path_input),
            _ => None,
        }
    }
    pub fn toggle_focus(&mut self) {
        if self.step == 1 || self.step == 6 || self.step == 7 {
            self.focus = 1 - self.focus;
        }
    }
}

/// Welcome note markdown seeded on first-run/finish.
pub const WELCOME_NOTE_MD: &str = r#"# Welcome to clin

Your encrypted, git-backed terminal notebook. A few things to try:

- Press `?` any time for the full keybind reference.
- Press `:` (or `Ctrl+p`) to open the command palette.
- Press `n` to create a note, `t` to create one from a template.
- Press `Ctrl+g` for the graph view, `.` to manage tags.

Notes are saved to your vault. Edit `~/.config/clin/config.toml` any time, or re-run `clin --setup`.

Happy writing.
"#;
