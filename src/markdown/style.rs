use ratatui::style::{Color, Modifier, Style};

/// A single line in the rendered markdown output.
/// Each cell is a (character, style) pair, matching the grid format
/// consumed by the page system and snapshot renderer.
#[derive(Debug, Clone)]
pub(crate) struct RenderLine {
    pub cells: Vec<(char, Style)>,
}

/// Theme-derived styles for every markdown element type.
/// All colors derive from [`AppThemeColors`](crate::app_theme::AppThemeColors)
/// — no raw `Color::Rgb` except for syntect-highlighted code spans.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct MarkdownTheme {
    // Headings (descending hierarchy)
    pub h1: Style,
    pub h1_banner: Style,
    pub h2: Style,
    pub h3: Style,
    pub h4: Style,
    pub h5: Style,
    pub h6: Style,
    // Body text
    pub paragraph: Style,
    pub code_inline: Style,
    pub code_block_bg: Option<Color>,
    // Links
    pub link_text: Style,
    pub link_url: Style,
    pub wikilink: Style,
    // Blockquote
    pub blockquote: Style,
    pub blockquote_bar: Style,
    // Tables
    pub table_header: Style,
    pub table_cell: Style,
    pub table_border: Style,
    // Misc
    pub hr: Style,
    pub footnote_ref: Style,
    pub footnote_def: Style,
    // Task items
    pub task_unchecked: Style,
    pub task_checked: Style,
}

impl MarkdownTheme {
    pub fn from_app_theme(theme: &crate::app_theme::AppThemeColors) -> Self {
        Self {
            h1: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            h1_banner: Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.heading)
                .add_modifier(Modifier::BOLD),
            h2: Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            h3: Style::default().fg(theme.tag).add_modifier(Modifier::BOLD),
            h4: Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
            h5: Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::ITALIC | Modifier::DIM),
            h6: Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::ITALIC),
            paragraph: Style::default().fg(theme.text),
            code_inline: Style::default()
                .fg(theme.fg)
                .bg(theme.muted),
            code_block_bg: theme.bg,
            link_text: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            link_url: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::UNDERLINED),
            wikilink: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::UNDERLINED),
            blockquote: Style::default().fg(theme.muted),
            blockquote_bar: Style::default().fg(theme.border),
            table_header: Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
            table_cell: Style::default().fg(theme.text),
            table_border: Style::default().fg(theme.muted),
            hr: Style::default().fg(theme.muted),
            footnote_ref: Style::default().fg(theme.tag),
            footnote_def: Style::default().fg(theme.muted),
            task_unchecked: Style::default().fg(theme.warning),
            task_checked: Style::default().fg(theme.success),
        }
    }
}
