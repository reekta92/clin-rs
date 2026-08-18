use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use regex::Regex;

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
    let strict_pattern =
        Regex::new(r#"^(?:\([A-Z]\)\s+|\d{4}-\d{2}-\d{2}\s+)"#).expect("valid regex");

    state.items = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("x ") && !l.starts_with("X "))
        .filter(|l| strict_pattern.is_match(l))
        .map(|l| l.to_string())
        .collect();

    state.last_modified = modified;
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

    let inner_block = Block::default()
        .style(theme.bg_style())
        .padding(Padding::new(2, 2, 1, 1));

    let mut lines = Vec::new();
    let separator_width = content_area.width.saturating_sub(4); // account for padding
    let separator = "─".repeat(separator_width as usize);

    for (i, task) in state.items.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(separator.clone()).style(Style::default().fg(theme.muted)));
        }
        lines.push(Line::from(task.clone()).style(Style::default().fg(theme.text)));
    }

    if lines.is_empty() {
        lines.push(Line::from("No pending tasks").style(Style::default().fg(theme.muted)));
    }

    let paragraph = Paragraph::new(lines)
        .style(theme.bg_style())
        .block(inner_block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, content_area);
}
