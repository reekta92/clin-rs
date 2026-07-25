use crate::config::UiConfig;
use crate::config::themes::theme_colors;
use crate::config::{Background, Theme};
use ratatui::style::{Color, Style};

/// App chrome colors (accent/heading/border). Distinct from
/// [`ThemeColors`](crate::config::ThemeColors) (graph viz).
#[derive(Debug, Clone)]
pub struct AppThemeColors {
    pub accent: Color,
    pub heading: Color,
    pub success: Color,
    pub warning: Color,
    pub destructive: Color,
    pub muted: Color,
    pub text: Color,
    pub fg: Color,
    pub bg: Option<Color>,
    pub border: Color,
    pub tag: Color,
    pub folder: Color,
    pub pinned: Color,
    pub smart: Color,
    pub subnote: Color,
    pub highlight_fg: Color,
    pub highlight_bg: Color,
    pub hint_bar_style: crate::config::HintBarStyle,
}

impl Default for AppThemeColors {
    fn default() -> Self {
        Self::from_config(&UiConfig::default(), &mut Vec::new())
    }
}

impl AppThemeColors {
    pub fn from_config(config: &UiConfig, warnings: &mut Vec<String>) -> Self {
        let resolved = crate::config::custom_themes::resolve_theme(&config.theme, warnings);
        let bg_enum = config.background.clone();
        let mut colors = match &resolved {
            crate::config::custom_themes::ResolvedTheme::Custom(file) => {
                Self::from_custom_chrome(&file.chrome, &bg_enum)
            }
            crate::config::custom_themes::ResolvedTheme::Builtin(Theme::Default) => Self {
                accent: Color::Cyan,
                heading: Color::Yellow,
                success: Color::Green,
                warning: Color::Yellow,
                destructive: Color::Red,
                muted: Color::DarkGray,
                text: Color::Reset,
                fg: Color::White,
                bg: match bg_enum {
                    Background::Transparent => None,
                    Background::Solid => Some(Color::Black),
                },
                border: Color::DarkGray,
                tag: Color::LightMagenta,
                folder: Color::Blue,
                pinned: Color::Yellow,
                smart: Color::LightMagenta,
                subnote: Color::LightCyan,
                highlight_fg: Color::Black,
                highlight_bg: Color::Cyan,
                hint_bar_style: crate::config::HintBarStyle::default(),
            },
            crate::config::custom_themes::ResolvedTheme::Builtin(t) => {
                let tc = theme_colors(t, bg_enum.clone());
                Self {
                    accent: tc.node_colors.first().copied().unwrap_or(Color::Cyan),
                    heading: tc.node_colors.get(3).copied().unwrap_or(Color::Yellow),
                    success: tc.node_colors.get(3).copied().unwrap_or(Color::Green),
                    warning: tc.node_colors.get(3).copied().unwrap_or(Color::Yellow),
                    destructive: tc.node_colors.get(5).copied().unwrap_or(Color::Red),
                    muted: tc.border_color,
                    text: tc.label_color,
                    fg: tc.selected_indicator_color,
                    bg: tc.background_color,
                    border: tc.border_color,
                    tag: tc
                        .node_colors
                        .get(1)
                        .copied()
                        .unwrap_or(Color::LightMagenta),
                    folder: tc.node_colors.get(2).copied().unwrap_or(Color::Blue),
                    pinned: tc.node_colors.get(4).copied().unwrap_or(Color::Yellow),
                    smart: tc.node_colors.get(6).copied().unwrap_or(Color::LightMagenta),
                    subnote: tc.node_colors.get(7).copied().unwrap_or(Color::LightCyan),
                    highlight_fg: tc.background_color.unwrap_or(Color::Black),
                    highlight_bg: tc.node_colors.first().copied().unwrap_or(Color::Cyan),
                    hint_bar_style: crate::config::HintBarStyle::default(),
                }
            }
        };
        if let Some(c) = config
            .accent
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.accent = c;
        }
        if let Some(c) = config
            .heading
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.heading = c;
        }
        if let Some(c) = config
            .success
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.success = c;
        }
        if let Some(c) = config
            .destructive
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.destructive = c;
        }
        if let Some(c) = config
            .muted
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.muted = c;
        }
        if let Some(c) = config
            .text
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.text = c;
        }
        if let Some(c) = config
            .border
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.border = c;
        }
        if let Some(c) = config
            .tag
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.tag = c;
        }
        if let Some(c) = config
            .folder
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.folder = c;
        }
        if let Some(c) = config
            .background_color
            .as_ref()
            .and_then(|h| crate::config::parse_hex_color(h))
        {
            colors.bg = Some(c);
        }
        colors.hint_bar_style = config.hint_bar_style;
        colors
    }

    fn from_custom_chrome(c: &crate::config::CustomChrome, bg_enum: &Background) -> Self {
        Self {
            accent: crate::config::parse_hex_color(&c.accent).unwrap_or(Color::Cyan),
            heading: crate::config::parse_hex_color(&c.heading).unwrap_or(Color::Reset),
            success: crate::config::parse_hex_color(&c.success).unwrap_or(Color::Reset),
            warning: crate::config::parse_hex_color(&c.warning).unwrap_or(Color::Reset),
            destructive: crate::config::parse_hex_color(&c.destructive).unwrap_or(Color::Reset),
            muted: crate::config::parse_hex_color(&c.muted).unwrap_or(Color::Reset),
            text: crate::config::parse_hex_color(&c.text).unwrap_or(Color::Reset),
            fg: crate::config::parse_hex_color(&c.fg).unwrap_or(Color::Reset),
            bg: match bg_enum {
                Background::Transparent => None,
                Background::Solid => c
                    .background
                    .as_ref()
                    .and_then(|h| crate::config::parse_hex_color(h)),
            },
            border: crate::config::parse_hex_color(&c.border).unwrap_or(Color::Reset),
            tag: crate::config::parse_hex_color(&c.tag).unwrap_or(Color::Reset),
            folder: crate::config::parse_hex_color(&c.folder).unwrap_or(Color::Reset),
            pinned: crate::config::parse_hex_color(&c.pinned).unwrap_or(Color::Reset),
            smart: crate::config::parse_hex_color(&c.smart).unwrap_or(Color::Reset),
            subnote: crate::config::parse_hex_color(&c.subnote).unwrap_or(Color::Reset),
            highlight_fg: crate::config::parse_hex_color(&c.highlight_fg).unwrap_or(Color::Reset),
            highlight_bg: crate::config::parse_hex_color(&c.highlight_bg).unwrap_or(Color::Reset),
            hint_bar_style: crate::config::HintBarStyle::default(),
        }
    }

    pub fn bg_style(&self) -> Style {
        match self.bg {
            Some(bg) => Style::default().bg(bg),
            None => Style::default(),
        }
    }

    pub fn preview_bg(&self) -> Option<Color> {
        derive_color(self.bg, -15)
    }

    pub fn title_bar_bg(&self) -> Option<Color> {
        derive_color(self.bg, -10)
    }

    pub fn hint_line_bg(&self) -> Option<Color> {
        derive_color(self.bg, -8)
    }

    pub fn preview_bg_style(&self) -> Style {
        match self.preview_bg() {
            Some(c) => Style::default().bg(c),
            None => Style::default(),
        }
    }

    pub fn title_bar_bg_style(&self) -> Style {
        match self.title_bar_bg() {
            Some(c) => Style::default().bg(c),
            None => Style::default(),
        }
    }

    pub fn hint_line_bg_style(&self) -> Style {
        match self.hint_line_bg() {
            Some(c) => Style::default().bg(c),
            None => Style::default(),
        }
    }

    pub fn hover_style(&self) -> Style {
        match self.bg {
            Some(c) => match c {
                Color::Rgb(r, g, b) => {
                    let luminance = (r as f32 * 0.299) + (g as f32 * 0.587) + (b as f32 * 0.114);
                    let delta = if luminance > 128.0 { -15 } else { 15 };
                    Style::default().bg(derive_color(Some(c), delta).unwrap_or(c))
                }
                _ => Style::default().bg(Color::DarkGray),
            },
            None => Style::default().bg(Color::DarkGray),
        }
    }
}

fn derive_color(base: Option<Color>, delta: i16) -> Option<Color> {
    base.map(|c| match c {
        Color::Rgb(r, g, b) => {
            let clamp = |v: i16| v.clamp(0, 255) as u8;
            Color::Rgb(
                clamp(r as i16 + delta),
                clamp(g as i16 + delta),
                clamp(b as i16 + delta),
            )
        }
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_chrome_accent_parsed() {
        let chrome = crate::config::CustomChrome {
            accent: "#ff0000".to_string(),
            heading: "#00ff00".to_string(),
            success: "#0000ff".to_string(),
            warning: "#ffff00".to_string(),
            destructive: "#ff00ff".to_string(),
            muted: "#888888".to_string(),
            text: "#ffffff".to_string(),
            fg: "#ffffff".to_string(),
            border: "#444444".to_string(),
            tag: "#ffa500".to_string(),
            folder: "#00ffff".to_string(),
            highlight_fg: "#000000".to_string(),
            highlight_bg: "#ff0000".to_string(),
            background: Some("#000000".to_string()),
            ..Default::default()
        };
        let colors = AppThemeColors::from_custom_chrome(&chrome, &Background::Solid);
        assert_eq!(colors.accent, Color::Rgb(255, 0, 0));
        assert_eq!(colors.heading, Color::Rgb(0, 255, 0));
        assert_eq!(colors.success, Color::Rgb(0, 0, 255));
        assert_eq!(colors.bg, Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn custom_chrome_missing_bg_transparent() {
        let chrome = crate::config::CustomChrome {
            accent: "#ff0000".to_string(),
            heading: "#00ff00".to_string(),
            success: "#0000ff".to_string(),
            warning: "#ffff00".to_string(),
            destructive: "#ff00ff".to_string(),
            muted: "#888888".to_string(),
            text: "#ffffff".to_string(),
            fg: "#ffffff".to_string(),
            border: "#444444".to_string(),
            tag: "#ffa500".to_string(),
            folder: "#00ffff".to_string(),
            highlight_fg: "#000000".to_string(),
            highlight_bg: "#ff0000".to_string(),
            background: None,
            ..Default::default()
        };
        let colors = AppThemeColors::from_custom_chrome(&chrome, &Background::Transparent);
        assert_eq!(colors.accent, Color::Rgb(255, 0, 0));
        assert_eq!(colors.bg, None);
    }

    #[test]
    fn custom_chrome_bg_toggle_respected() {
        let chrome = crate::config::CustomChrome {
            accent: "#ff0000".to_string(),
            heading: "#00ff00".to_string(),
            success: "#0000ff".to_string(),
            warning: "#ffff00".to_string(),
            destructive: "#ff00ff".to_string(),
            muted: "#888888".to_string(),
            text: "#ffffff".to_string(),
            fg: "#ffffff".to_string(),
            border: "#444444".to_string(),
            tag: "#ffa500".to_string(),
            folder: "#00ffff".to_string(),
            highlight_fg: "#000000".to_string(),
            highlight_bg: "#ff0000".to_string(),
            background: Some("#1a1b26".to_string()),
            ..Default::default()
        };
        let transparent = AppThemeColors::from_custom_chrome(&chrome, &Background::Transparent);
        assert_eq!(transparent.bg, None);
        let solid = AppThemeColors::from_custom_chrome(&chrome, &Background::Solid);
        assert_eq!(solid.bg, Some(Color::Rgb(0x1a, 0x1b, 0x26)));
    }
}
