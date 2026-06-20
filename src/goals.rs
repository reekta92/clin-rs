use crate::app_theme::AppThemeColors;
use crate::config::GoalsConfig;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyProgress {
    pub date: String, // "YYYY-MM-DD"
    pub words_written: usize,
    pub notes_modified: HashSet<String>,
}

pub fn count_words(text: &str) -> usize {
    text.split_whitespace().filter(|w| !w.is_empty()).count()
}

fn make_progress_bar(
    current: usize,
    target: usize,
    width: usize,
    theme: &AppThemeColors,
) -> Line<'static> {
    if target == 0 {
        return Line::from(vec![Span::styled(
            "Disabled",
            Style::default().fg(theme.muted),
        )]);
    }

    let pct = {
        let p = (current * 100) / target;
        p.min(100)
    };

    let inner_width = width.saturating_sub(4);
    let bar_width = inner_width.saturating_sub(5); // space + 3-char pct + '%'

    let mut spans = Vec::new();

    if bar_width > 0 {
        if current >= target {
            let completed_color = theme.accent;
            let label = "COMPLETED";
            if bar_width >= label.len() {
                let pad_total = bar_width - label.len();
                let pad_left = pad_total / 2;
                let pad_right = pad_total - pad_left;

                if pad_left > 0 {
                    spans.push(Span::styled(
                        "█".repeat(pad_left),
                        Style::default().fg(completed_color),
                    ));
                }
                spans.push(Span::styled(
                    label,
                    Style::default()
                        .bg(completed_color)
                        .fg(theme.bg.unwrap_or(Color::Black))
                        .add_modifier(Modifier::BOLD),
                ));
                if pad_right > 0 {
                    spans.push(Span::styled(
                        "█".repeat(pad_right),
                        Style::default().fg(completed_color),
                    ));
                }
            } else {
                spans.push(Span::styled(
                    label[..bar_width].to_string(),
                    Style::default()
                        .bg(completed_color)
                        .fg(theme.bg.unwrap_or(Color::Black))
                        .add_modifier(Modifier::BOLD),
                ));
            }
        } else {
            let filled_chars = {
                let f = (current * bar_width) / target;
                f.min(bar_width)
            };
            let empty_chars = bar_width.saturating_sub(filled_chars);

            if filled_chars > 0 {
                spans.push(Span::styled(
                    "█".repeat(filled_chars),
                    Style::default().fg(theme.success),
                ));
            }
            if empty_chars > 0 {
                spans.push(Span::styled(
                    "░".repeat(empty_chars),
                    Style::default().fg(theme.muted),
                ));
            }
        }
    }

    spans.push(Span::styled(
        format!(" {:>3}%", pct),
        Style::default().fg(theme.text),
    ));

    Line::from(spans)
}

pub fn draw_goals_progress(
    frame: &mut Frame,
    rect: Rect,
    theme: &AppThemeColors,
    progress: &DailyProgress,
    config: &GoalsConfig,
) {
    if rect.height < 7 || rect.width < 7 {
        return;
    }

    let mut lines = Vec::with_capacity(7);

    // Row 0: Title "Daily Goals"
    lines.push(Line::from(vec![Span::styled(
        "Daily Goals",
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD),
    )]));

    // Row 1: Blank line
    lines.push(Line::from(""));

    // Row 2: Words progress label
    let words_label = if config.word_goal == 0 {
        format!("Words: {}", progress.words_written)
    } else {
        format!("Words: {} / {}", progress.words_written, config.word_goal)
    };
    lines.push(Line::from(vec![Span::styled(
        words_label,
        Style::default().fg(theme.text),
    )]));

    // Row 3: Progress bar for words
    lines.push(make_progress_bar(
        progress.words_written,
        config.word_goal,
        rect.width as usize,
        theme,
    ));

    // Row 4: Blank line
    lines.push(Line::from(""));

    // Row 5: Notes progress label
    let notes_label = if config.note_goal == 0 {
        format!("Notes: {}", progress.notes_modified.len())
    } else {
        format!(
            "Notes: {} / {}",
            progress.notes_modified.len(),
            config.note_goal
        )
    };
    lines.push(Line::from(vec![Span::styled(
        notes_label,
        Style::default().fg(theme.text),
    )]));

    // Row 6: Progress bar for notes
    lines.push(make_progress_bar(
        progress.notes_modified.len(),
        config.note_goal,
        rect.width as usize,
        theme,
    ));

    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme.muted))
        .padding(Padding::new(2, 2, 0, 0));

    let paragraph = Paragraph::new(lines).style(theme.bg_style()).block(block);

    frame.render_widget(paragraph, rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_words() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("hello"), 1);
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("  hello   world  "), 2);
        assert_eq!(count_words("one\ntwo\tthree"), 3);
    }
}
