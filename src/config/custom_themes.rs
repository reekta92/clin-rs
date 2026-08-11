//! Custom theme support: load TOML theme files from `~/.config/clin/themes/<name>.toml`.
//!
//! Custom theme files override built-in themes when present. A custom theme name that
//! matches a built-in name takes priority (custom-first lookup).
//!
//! # Schema
//!
//! ```toml
//! [chrome]
//! accent = "#7aa2f7"
//! heading = "#e0af68"
//! success = "#9ece6a"
//! warning = "#e0af68"
//! destructive = "#f7768e"
//! muted = "#565f89"
//! text = "#c0caf5"
//! fg = "#ffffff"
//! border = "#414868"
//! tag = "#bb9af7"
//! folder = "#7dcfff"
//! highlight_fg = "#1a1b26"
//! highlight_bg = "#7aa2f7"
//! background = "#1a1b26"   # optional → transparent when absent
//!
//! [graph]
//! nodes = ["#7aa2f7", "#bb9af7", "#7dcfff", "#e0af68", ...]
//! chrome = "#565f89"
//! title  = "#bb9af7"
//! text   = "#cbccd5"
//! fg     = "#ffffff"
//! grid   = "#383c5f"
//! bg     = "#1a1b26"       # optional
//! ```

use serde::Deserialize;
use std::path::PathBuf;

use super::structs::ClinConfig;
use super::types::Theme;
use anyhow::Context;

// ---------------------------------------------------------------------------
// Data structs
// ---------------------------------------------------------------------------

/// Chrome/appearance colors for a custom theme (maps to `[chrome]` in TOML).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CustomChrome {
    pub accent: String,
    pub heading: String,
    pub success: String,
    pub warning: String,
    pub destructive: String,
    pub muted: String,
    pub text: String,
    pub fg: String,
    pub border: String,
    pub tag: String,
    pub folder: String,
    #[serde(default)]
    pub pinned: String,
    #[serde(default)]
    pub smart: String,
    #[serde(default)]
    pub subnote: String,
    pub highlight_fg: String,
    pub highlight_bg: String,
    /// Optional; `None` means transparent background.
    #[serde(default)]
    pub background: Option<String>,
}

/// Graph palette colors for a custom theme (maps to `[graph]` in TOML).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CustomGraph {
    #[serde(default)]
    pub nodes: Vec<String>,
    #[serde(default)]
    pub chrome: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub fg: String,
    #[serde(default)]
    pub grid: String,
    #[serde(default)]
    pub bg: Option<String>,
}

/// A complete custom theme file — chrome + graph palette.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CustomThemeFile {
    #[serde(default)]
    pub chrome: CustomChrome,
    #[serde(default)]
    pub graph: CustomGraph,
}

/// Either a built-in `Theme` variant or a fully loaded custom theme.
#[derive(Debug, Clone)]
pub enum ResolvedTheme {
    Builtin(Theme),
    Custom(Box<CustomThemeFile>),
}

// ---------------------------------------------------------------------------
// Directory & file helpers
// ---------------------------------------------------------------------------

/// Return the path to the custom themes directory,
/// derived from the config file's parent + `themes/`.
pub fn custom_themes_dir() -> anyhow::Result<PathBuf> {
    Ok(ClinConfig::config_path()?
        .parent()
        .context("config path has no parent")?
        .join("themes"))
}

// ---------------------------------------------------------------------------
// Listing & loading
// ---------------------------------------------------------------------------

/// List all usable custom theme names (sorted `.toml` stems in the themes dir).
///
/// * Only `*.toml` files are considered.
/// * The stem must be a non-empty ASCII alphanumeric + underscore string.
/// * Missing or invalid directory → empty vec (never panic).
pub fn list_custom_themes() -> Vec<String> {
    let dir = match custom_themes_dir() {
        Ok(d) if d.is_dir() => d,
        _ => return Vec::new(),
    };

    let mut names: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return names;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.is_empty() || !stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        names.push(stem.to_string());
    }

    names.sort();
    names
}

/// Load a custom theme file by name (stem only, no `.toml` extension).
///
/// Returns `None` if the file doesn't exist. Pushes parse/read errors to
/// `warnings` instead of logging to stderr (never panic).
pub fn load_custom_theme(name: &str, warnings: &mut Vec<String>) -> Option<CustomThemeFile> {
    let dir = match custom_themes_dir() {
        Ok(d) => d,
        Err(_) => return None,
    };

    let path = dir.join(format!("{name}.toml"));
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            warnings.push(format!(
                "Theme parse error: Failed to load '{name}' from {}: {e}. Falling back to default.",
                path.display()
            ));
            return None;
        }
    };

    match toml::from_str::<CustomThemeFile>(&content) {
        Ok(theme) => Some(theme),
        Err(e) => {
            warnings.push(format!(
                "Theme parse error: Failed to load '{name}' from {}: {e}. Falling back to default.",
                path.display()
            ));
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Resolve a theme name to either a custom file or a built-in variant.
///
/// Lookup order:
/// 1. Custom theme dir (<name>.toml) — if found and valid, wins (overrides built-in).
/// 2. Built-in `Theme::from_str(name)` — if matches a known name.
/// 3. Fallback → `Theme::Default` (silent, never panics).
pub fn resolve_theme(name: &str, warnings: &mut Vec<String>) -> ResolvedTheme {
    // Custom first
    if let Some(custom) = load_custom_theme(name, warnings) {
        return ResolvedTheme::Custom(Box::new(custom));
    }
    // Built-in
    if let Ok(t) = name.parse::<Theme>() {
        return ResolvedTheme::Builtin(t);
    }
    // Fallback
    ResolvedTheme::Builtin(Theme::Default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigTestGuard;
    use crate::config::set_config_path_override;

    /// Filesystem-dependent tests are combined in one function because
    /// `CONFIG_PATH_OVERRIDE` is a `OnceLock` — only the first call to
    /// `set_config_path_override` takes effect, so all assertions share the
    /// same override temp directory.
    #[test]
    fn custom_theme_resolve_and_list() {
        let _lock = ConfigTestGuard::lock();

        let dir = tempfile::tempdir().expect("temp dir");
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, b"[ui]\ntheme = \"default\"\n").unwrap();
        set_config_path_override(config_path);

        let themes_dir = dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        // foo.toml — valid custom theme (used by resolve_custom_first)
        std::fs::write(
            themes_dir.join("foo.toml"),
            br##"[chrome]
accent = "#ff0000"
heading = "#00ff00"
success = "#0000ff"
warning = "#ffff00"
destructive = "#ff00ff"
muted = "#888888"
text = "#ffffff"
fg = "#ffffff"
border = "#444444"
tag = "#ffa500"
folder = "#00ffff"
highlight_fg = "#000000"
highlight_bg = "#ff0000"
background = "#000000"
"##,
        )
        .unwrap();

        // a_first.toml + z_last.toml — for list ordering
        std::fs::write(
            themes_dir.join("a_first.toml"),
            b"[chrome]\naccent = \"#ff0000\"\n",
        )
        .unwrap();
        std::fs::write(
            themes_dir.join("z_last.toml"),
            b"[chrome]\naccent = \"#ff0000\"\n",
        )
        .unwrap();
        // Non-.toml files must be ignored.
        std::fs::write(themes_dir.join("ignore.txt"), b"not a theme").unwrap();

        // === resolve_custom_first ===
        match resolve_theme("foo", &mut Vec::new()) {
            ResolvedTheme::Custom(_) => {} // expected
            other => panic!("expected Custom, got {other:?}"),
        }

        // === resolve_builtin_fallback ===
        match resolve_theme("gruvbox", &mut Vec::new()) {
            ResolvedTheme::Builtin(Theme::Gruvbox) => {} // expected
            other => panic!("expected Builtin(Gruvbox), got {other:?}"),
        }

        // === resolve_unknown_to_default ===
        match resolve_theme("nope", &mut Vec::new()) {
            ResolvedTheme::Builtin(Theme::Default) => {} // expected
            other => panic!("expected Builtin(Default), got {other:?}"),
        }

        // === list_custom_themes_sorted ===
        let names = list_custom_themes();
        assert_eq!(names, vec!["a_first", "foo", "z_last"]);
    }

    #[test]
    fn custom_theme_appearance_end_to_end() {
        // Full pipeline: custom theme file → AppThemeColors → ThemeColors
        let _lock = ConfigTestGuard::lock();
        let dir = tempfile::tempdir().expect("temp dir");
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, b"[ui]\ntheme = \"redtest\"\n").unwrap();
        set_config_path_override(config_path);

        let themes_dir = dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(
            themes_dir.join("redtest.toml"),
            br##"[chrome]
accent = "#ff0000"
heading = "#00ff00"
success = "#0000ff"
warning = "#ffff00"
destructive = "#ff00ff"
muted = "#888888"
text = "#ffffff"
fg = "#ffffff"
border = "#444444"
tag = "#ffa500"
folder = "#00ffff"
highlight_fg = "#000000"
highlight_bg = "#ff0000"
background = "#000000"

[graph]
nodes = ["#ff0000","#00ff00","#0000ff","#ffff00","#ff00ff","#00ffff","#ffffff","#888888"]
chrome = "#444444"
title = "#ffa500"
text = "#cccccc"
fg = "#ffffff"
grid = "#222222"
bg = "#000000"
"##,
        )
        .unwrap();

        // 1. ThemeColors via ClinConfig
        let mut config: crate::config::ClinConfig = toml::from_str(
            r#"[ui]
theme = "redtest"
"#,
        )
        .unwrap();
        config.graf.visual.graph_background = crate::config::Background::Solid;
        let tc = config.theme_colors();
        assert_eq!(
            tc.node_colors[0],
            ratatui::style::Color::Rgb(255, 0, 0),
            "first node color from custom theme"
        );
        assert_eq!(
            tc.edge_color,
            ratatui::style::Color::Rgb(0x44, 0x44, 0x44),
            "chrome color"
        );
        assert_eq!(
            tc.title_color,
            ratatui::style::Color::Rgb(0xff, 0xa5, 0x00),
            "title color"
        );
        assert_eq!(
            tc.background_color,
            Some(ratatui::style::Color::Rgb(0, 0, 0)),
            "background from custom theme (solid)"
        );

        // 2. AppThemeColors via from_config
        let ui_config = crate::config::UiConfig {
            theme: "redtest".to_string(),
            background: crate::config::Background::Solid,
            ..Default::default()
        };
        let app_colors = crate::app_theme::AppThemeColors::from_config(&ui_config, &mut Vec::new());
        assert_eq!(
            app_colors.accent,
            ratatui::style::Color::Rgb(255, 0, 0),
            "accent from custom theme"
        );
        assert_eq!(
            app_colors.heading,
            ratatui::style::Color::Rgb(0, 255, 0),
            "heading from custom theme"
        );
        assert_eq!(
            app_colors.bg,
            Some(ratatui::style::Color::Rgb(0, 0, 0)),
            "background from custom theme"
        );
    }
}
