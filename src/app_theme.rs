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
    pub highlight_fg: Color,
    pub highlight_bg: Color,
    pub hint_bar_style: crate::config::HintBarStyle,
}

impl Default for AppThemeColors {
    fn default() -> Self {
        Self::from_config(&UiConfig::default())
    }
}

impl AppThemeColors {
    pub fn from_config(config: &UiConfig) -> Self {
        let theme_enum = config.theme.clone();
        let bg_enum = config.background.clone();

        let t = theme_colors(&theme_enum, bg_enum.clone());

        let mut colors = if matches!(theme_enum, Theme::Default) {
            Self {
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
                highlight_fg: Color::Black,
                highlight_bg: Color::Cyan,
                hint_bar_style: crate::config::HintBarStyle::default(),
            }
        } else {
            Self {
                accent: t.node_colors.first().copied().unwrap_or(Color::Cyan),
                heading: t.node_colors.get(3).copied().unwrap_or(Color::Yellow),
                success: t.node_colors.get(4).copied().unwrap_or(Color::Green),
                warning: t.node_colors.get(3).copied().unwrap_or(Color::Yellow),
                destructive: t.node_colors.get(5).copied().unwrap_or(Color::Red),
                muted: t.border_color,
                text: t.label_color,
                fg: t.selected_indicator_color,
                bg: t.background_color,
                border: t.border_color,
                tag: t.node_colors.get(1).copied().unwrap_or(Color::LightMagenta),
                folder: t.node_colors.get(2).copied().unwrap_or(Color::Blue),
                highlight_fg: t.background_color.unwrap_or(Color::Black),
                highlight_bg: t.node_colors.first().copied().unwrap_or(Color::Cyan),
                hint_bar_style: crate::config::HintBarStyle::default(),
            }
        };

        if let Some(c) = config.accent.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.accent = c;
        }
        if let Some(c) = config.heading.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.heading = c;
        }
        if let Some(c) = config.success.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.success = c;
        }
        if let Some(c) = config.destructive.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.destructive = c;
        }
        if let Some(c) = config.muted.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.muted = c;
        }
        if let Some(c) = config.text.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.text = c;
        }
        if let Some(c) = config.border.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.border = c;
        }
        if let Some(c) = config.tag.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
            colors.tag = c;
        }
        if let Some(c) = config.folder.as_ref().and_then(|h| crate::config::parse_hex_color(h)) {
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
