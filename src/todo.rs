use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};

use crate::app_theme::AppThemeColors;

#[derive(Default)]
pub struct TodoState {
    pub last_modified: Option<std::time::SystemTime>,
    pub items: Vec<String>,
}

pub fn update_todo_state(storage: &crate::storage::Storage, state: &mut TodoState) {
    let path = storage.notes_dir.join("todo.txt");
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

    if modified.is_none() {
        state.last_modified = None;
        state.items.clear();
        return;
    }

    if state.last_modified == modified {
        return;
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        state.last_modified = None;
        state.items.clear();
        return;
    };

    // Strict parsing: lines must start with priority (e.g. `(A) `) or date (e.g. `2023-01-01 `)
    // and MUST NOT start with `x ` (which marks completion).

    let mut tasks: Vec<(char, usize, String)> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("x ") && !l.starts_with("X "))
        .filter(|l| is_strict_todo_line(l))
        .enumerate()
        .map(|(orig_idx, l)| {
            let priority = if l.len() >= 4 && l.starts_with('(') && l[2..4] == *") " {
                let c = l.chars().nth(1).expect("len >= 4");
                if c.is_ascii_uppercase() {
                    c
                } else {
                    '~' // lowest priority fallback
                }
            } else {
                '~' // lowest priority for no priority
            };
            (priority, orig_idx, l.to_string())
        })
        .collect();

    // Sort by priority (A is highest), then original index to keep stable
    tasks.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    state.items = tasks.into_iter().map(|(_, _, task)| task).collect();

    state.last_modified = modified;
}

/// True if `l` starts with a todo.txt priority `(A) ` or date `YYYY-MM-DD `.
fn is_strict_todo_line(l: &str) -> bool {
    let b = l.as_bytes();
    if b.len() >= 4
        && b[0] == b'('
        && b[1].is_ascii_uppercase()
        && b[2] == b')'
        && b[3].is_ascii_whitespace()
    {
        return true;
    }
    b.len() >= 11
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10].is_ascii_whitespace()
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
}

fn highlight_todo_task<'a>(task: &'a str, theme: &AppThemeColors) -> Line<'a> {
    let chars: Vec<char> = task.chars().collect();
    if chars.is_empty() {
        return Line::default();
    }
    let mut styles = vec![Style::default().fg(theme.text); chars.len()];

    let mut i = 0;
    if i + 3 < chars.len()
        && chars[i] == '('
        && chars[i + 2] == ')'
        && chars[i + 3] == ' '
        && chars[i + 1].is_ascii_uppercase()
    {
        let p_style = Style::default().fg(theme.warning);
        styles[i] = p_style;
        styles[i + 1] = p_style;
        styles[i + 2] = p_style;
        styles[i + 3] = p_style;
        i += 4;
    }

    for _ in 0..2 {
        if i + 10 <= chars.len() {
            let is_date = chars[i..i + 10]
                .iter()
                .all(|&c| c.is_ascii_digit() || c == '-')
                && chars[i + 4] == '-'
                && chars[i + 7] == '-';
            let is_valid_end = i + 10 == chars.len() || chars[i + 10].is_whitespace();
            if is_date && is_valid_end {
                i += 10;
                if i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let word_len = i - start;
        if word_len > 1 {
            let word_style = if chars[start] == '+' {
                Style::default().fg(theme.success)
            } else if chars[start] == '@' {
                Style::default().fg(theme.accent)
            } else if let Some(colon_pos) = chars[start..i].iter().position(|&c| c == ':') {
                if colon_pos > 0 && colon_pos < word_len - 1 {
                    Style::default().fg(theme.tag)
                } else {
                    Style::default().fg(theme.text)
                }
            } else {
                Style::default().fg(theme.text)
            };

            if word_style != Style::default().fg(theme.text) {
                for style in &mut styles[start..i] {
                    *style = word_style;
                }
            }
        }
    }

    let mut spans = Vec::new();
    let mut current_style = styles[0];
    let mut current_text = String::new();
    for (c, s) in chars.iter().zip(styles.iter()) {
        if *s == current_style {
            current_text.push(*c);
        } else {
            spans.push(Span::styled(current_text.clone(), current_style));
            current_style = *s;
            current_text = c.to_string();
        }
    }
    spans.push(Span::styled(current_text, current_style));

    Line::from(spans)
}

#[allow(clippy::implicit_hasher)]
pub fn draw_todo(
    frame: &mut Frame,
    rect: Rect,
    theme: &AppThemeColors,
    state: &TodoState,
    bottom_border: bool,
    strip_rect: Rect,
) {
    if rect.height < 3 || rect.width < 5 {
        return;
    }

    let border = if bottom_border {
        Borders::TOP | Borders::BOTTOM
    } else {
        Borders::TOP
    };
    let border_bg = theme.bg.unwrap_or(ratatui::style::Color::Reset);
    let strip_block = Block::default()
        .style(theme.bg_style())
        .borders(border)
        .border_style(Style::default().fg(theme.muted).bg(border_bg));
    let inner = strip_block.inner(strip_rect);
    frame.render_widget(&strip_block, strip_rect);

    let content_x = rect.x.max(inner.x);
    let content_y = rect.y.max(inner.y);
    let content_w = (rect.right().min(inner.right())).saturating_sub(content_x);
    let content_h = (rect.bottom().min(inner.bottom())).saturating_sub(content_y);
    let content_area = Rect::new(content_x, content_y, content_w, content_h);

    let mut lines = Vec::new();
    let separator_width = content_area.width.saturating_sub(4); // account for padding
    let separator = "─".repeat(separator_width as usize);

    let mut content_height = 0;
    for (i, task) in state.items.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(separator.clone()).style(Style::default().fg(theme.muted)));
            content_height += 1;
        }
        lines.push(highlight_todo_task(task, theme));

        let char_count = task.chars().count() as u16;
        let wrapped_lines = char_count
            .saturating_sub(1)
            .checked_div(separator_width)
            .unwrap_or(0)
            + 1;
        content_height += wrapped_lines;
    }

    if lines.is_empty() {
        lines.push(Line::from("No pending tasks").style(Style::default().fg(theme.muted)));
        content_height = 1;
    }

    let pad_top = content_area.height.saturating_sub(content_height) / 2;

    let inner_block = Block::default()
        .style(theme.bg_style())
        .padding(Padding::new(2, 2, pad_top, 0));

    let paragraph = Paragraph::new(lines)
        .style(theme.bg_style())
        .block(inner_block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, content_area);
}
