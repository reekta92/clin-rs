use std::sync::Arc;
use std::ops::Range;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StyledSpan {
    pub text: String,
    pub style: ratatui::style::Style,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderLine {
    pub spans: Vec<StyledSpan>,
    pub visual_width: usize,
    pub is_blank: bool,
    pub image_url: Option<Arc<str>>,
    pub source_line: usize,
}

impl RenderLine {
    pub(crate) fn char_index_at_visual_column(&self, column: usize) -> Option<usize> {
        if column >= self.visual_width {
            return None;
        }

        let mut current_col = 0;
        let mut char_idx = 0;
        let mut last_visible_char_idx = None;

        for span in &self.spans {
            for c in span.text.chars() {
                let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if w > 0 {
                    let start = current_col;
                    let end = current_col + w;
                    if column >= start && column < end {
                        return Some(char_idx);
                    }
                    current_col = end;
                    last_visible_char_idx = Some(char_idx);
                } else {
                    // width-0 character: attaches to preceding visible character
                }
                char_idx += 1;
            }
        }

        // If it didn't match a specific visible character start/end (e.g. because of width-0 chars),
        // we can default to the last visible char seen.
        last_visible_char_idx
    }

    pub(crate) fn visual_column_of_char(&self, char_index: usize) -> usize {
        let mut current_col = 0;
        let mut char_idx = 0;

        for span in &self.spans {
            for c in span.text.chars() {
                if char_idx == char_index {
                    return current_col;
                }
                let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                current_col += w;
                char_idx += 1;
            }
        }

        current_col // Clamped to line end
    }

    pub(crate) fn text_range(&self, chars: Range<usize>) -> String {
        let mut result = String::new();
        let mut char_idx = 0;

        for span in &self.spans {
            if char_idx >= chars.end {
                break;
            }

            let span_len = span.text.chars().count();
            if char_idx + span_len <= chars.start {
                char_idx += span_len;
                continue;
            }

            for c in span.text.chars() {
                if char_idx >= chars.start && char_idx < chars.end {
                    result.push(c);
                }
                char_idx += 1;
                if char_idx >= chars.end {
                    break;
                }
            }
        }

        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedDocument {
    pub(crate) lines: Vec<RenderLine>,
    content_empty: bool,
    last_non_blank_line: Option<usize>,
    estimated_bytes: usize,
}

impl RenderedDocument {
    pub(crate) fn new(mut lines: Vec<RenderLine>) -> Self {
        for line in &mut lines {
            line.is_blank = line.spans.iter().all(|span| {
                span.text.chars().all(char::is_whitespace)
            });
        }

        let content_empty = lines.is_empty() || lines.iter().all(|l| l.is_blank);
        let last_non_blank_line = lines.iter().rposition(|l| !l.is_blank);

        let mut estimated_bytes = std::mem::size_of::<Self>();
        estimated_bytes += lines.capacity() * std::mem::size_of::<RenderLine>();
        for line in &lines {
            estimated_bytes += line.spans.capacity() * std::mem::size_of::<StyledSpan>();
            for span in &line.spans {
                estimated_bytes += span.text.capacity();
            }
            if let Some(url) = &line.image_url {
                estimated_bytes += url.len();
            }
        }

        Self {
            lines,
            content_empty,
            last_non_blank_line,
            estimated_bytes,
        }
    }

    pub(crate) fn lines(&self) -> &[RenderLine] {
        &self.lines
    }

    pub(crate) fn lines_mut(&mut self) -> &mut Vec<RenderLine> {
        &mut self.lines
    }

    pub(crate) fn line(&self, index: usize) -> Option<&RenderLine> {
        self.lines.get(index)
    }

    pub(crate) fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn is_content_empty(&self) -> bool {
        self.content_empty
    }

    pub(crate) fn last_non_blank_line(&self) -> Option<usize> {
        self.last_non_blank_line
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }

    pub(crate) fn image_slots(
        &self,
        range: Range<usize>,
    ) -> impl Iterator<Item = (usize, &str)> {
        let start = range.start.min(self.lines.len());
        let end = range.end.min(self.lines.len());
        self.lines[start..end]
            .iter()
            .enumerate()
            .filter_map(move |(idx, line)| {
                line.image_url.as_ref().map(|url| (idx, &**url))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MarkdownTheme {
    pub h1: Style,
    pub h1_banner: Style,
    pub h2: Style,
    pub h3: Style,
    pub h4: Style,
    pub h5: Style,
    pub h6: Style,
    pub paragraph: Style,
    pub code_inline: Style,
    pub code_block: Style,
    pub code_block_bg: Option<Color>,
    pub link_text: Style,
    pub link_url: Style,
    pub wikilink: Style,
    pub blockquote: Style,
    pub blockquote_bar: Style,
    pub table_header: Style,
    pub table_cell: Style,
    pub table_border: Style,
    pub hr: Style,
    pub footnote_ref: Style,
    pub footnote_def: Style,
    pub task_unchecked: Style,
    pub task_checked: Style,
    pub ghost_syntax: Style,
}

pub(crate) fn faint_background(fg: Color) -> Color {
    match fg {
        Color::Rgb(r, g, b) => Color::Rgb(r / 7, g / 7, b / 7),
        Color::Red | Color::LightRed => Color::Rgb(40, 10, 10),
        Color::Green | Color::LightGreen => Color::Rgb(10, 40, 10),
        Color::Yellow | Color::LightYellow => Color::Rgb(35, 30, 10),
        Color::Blue | Color::LightBlue => Color::Rgb(10, 10, 45),
        Color::Magenta | Color::LightMagenta => Color::Rgb(35, 10, 35),
        Color::Cyan | Color::LightCyan => Color::Rgb(10, 35, 35),
        Color::White | Color::Gray | Color::DarkGray => Color::Rgb(20, 20, 20),
        _ => Color::Reset,
    }
}

impl MarkdownTheme {
    pub fn from_app_theme(theme: &crate::app_theme::AppThemeColors) -> Self {
        Self {
            h1: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            h1_banner: Style::default()
                .fg(theme.accent)
                .bg(faint_background(theme.accent))
                .add_modifier(Modifier::BOLD),
            h2: Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
            h3: Style::default().fg(theme.tag).add_modifier(Modifier::BOLD),
            h4: Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
            h5: Style::default()
                .fg(theme.folder)
                .add_modifier(Modifier::BOLD),
            h6: Style::default()
                .fg(theme.destructive)
                .add_modifier(Modifier::BOLD),
            paragraph: Style::default().fg(theme.text),
            code_inline: Style::default().fg(theme.fg).bg(theme.muted),
            code_block: Style::default().fg(theme.muted),
            code_block_bg: Some(faint_background(theme.muted)),
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
            ghost_syntax: Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::DIM),
        }
    }
}
