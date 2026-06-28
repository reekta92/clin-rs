use crate::app_theme::AppThemeColors;
use ratatui::style::{Color, Modifier, Style};

pub struct RenderLine {
    pub cells: Vec<(char, Style)>,
}

pub struct MarkdownTheme {
    pub heading_1: Style,
    pub heading_2: Style,
    pub heading_3: Style,
    pub heading_4: Style,
    pub heading_5: Style,
    pub heading_6: Style,
    pub paragraph: Style,
    pub code_inline: Style,
    pub code_block: Style,
    pub link: Style,
    pub wikilink: Style,
    pub blockquote: Style,
    pub table_header: Style,
    pub table_border: Style,
    pub hr: Style,
    pub footnote_ref: Style,
    pub task_checkbox_checked: Style,
    pub task_checkbox_unchecked: Style,
    pub bg: Color,
    pub fg: Color,
}

impl MarkdownTheme {
    pub fn from_app_theme(colors: &AppThemeColors) -> Self {
        let bg = colors.bg.unwrap_or(Color::Reset);
        let fg = colors.fg;

        Self {
            heading_1: Style::reset()
                .fg(colors.heading)
                .add_modifier(Modifier::BOLD),
            heading_2: Style::reset()
                .fg(colors.accent)
                .add_modifier(Modifier::BOLD),
            heading_3: Style::reset().fg(colors.tag).add_modifier(Modifier::BOLD),
            heading_4: Style::reset().fg(colors.muted).add_modifier(Modifier::BOLD),
            heading_5: Style::reset().fg(colors.muted).add_modifier(Modifier::BOLD),
            heading_6: Style::reset().fg(colors.muted).add_modifier(Modifier::BOLD),
            paragraph: Style::reset().fg(fg).bg(bg),
            code_inline: Style::reset()
                .fg(colors.warning)
                .bg(colors.bg.unwrap_or(Color::Black)),
            code_block: Style::reset().fg(fg).bg(colors.bg.unwrap_or(Color::Black)),
            link: Style::reset()
                .fg(colors.accent)
                .add_modifier(Modifier::UNDERLINED),
            wikilink: Style::reset()
                .fg(colors.accent)
                .add_modifier(Modifier::UNDERLINED),
            blockquote: Style::reset().fg(colors.muted),
            table_header: Style::reset()
                .fg(colors.heading)
                .add_modifier(Modifier::BOLD),
            table_border: Style::reset().fg(colors.muted),
            hr: Style::reset().fg(colors.muted),
            footnote_ref: Style::reset().fg(colors.tag),
            task_checkbox_checked: Style::reset().fg(colors.success),
            task_checkbox_unchecked: Style::reset().fg(colors.warning),
            bg,
            fg,
        }
    }
}
