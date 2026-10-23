use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

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

    // The widget intentionally shows only tasks with a priority or creation date.
    let mut tasks: Vec<(char, usize, String)> = content
        .lines()
        .map(str::trim)
        .enumerate()
        .filter_map(|(index, line)| {
            let item = crate::markdown::todotxt::parse_todotxt_line(line);
            (!item.completed && (item.priority.is_some() || item.creation_date.is_some()))
                .then(|| (item.priority.unwrap_or('~'), index, line.to_owned()))
        })
        .collect();

    // Sort by priority (A is highest), then original index to keep stable
    tasks.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    state.items = tasks.into_iter().map(|(_, _, task)| task).collect();

    state.last_modified = modified;
}

fn highlight_todo_task<'a>(task: &'a str, theme: &AppThemeColors) -> Line<'a> {
    use crate::markdown::todotxt::{TodoTxtSpan, parse_todotxt_line};
    let item = parse_todotxt_line(task);
    let mut spans = Vec::new();
    let mut cursor = 0;
    if item.priority.is_some() {
        spans.push(Span::styled(&task[..4], Style::default().fg(theme.warning)));
        cursor = 4;
    }
    for token in item.spans {
        let word = token.text();
        let start = word.as_ptr() as usize - task.as_ptr() as usize;
        spans.push(Span::raw(&task[cursor..start]));
        let color = match token {
            TodoTxtSpan::Text(_) => theme.text,
            TodoTxtSpan::Project(_) => theme.success,
            TodoTxtSpan::Context(_) => theme.accent,
            TodoTxtSpan::Tag(_) => theme.tag,
        };
        spans.push(Span::styled(word, Style::default().fg(color)));
        cursor = start + word.len();
    }
    spans.push(Span::raw(&task[cursor..]));
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

    let content_area = crate::ui::widget_strip_area(frame, rect, strip_rect, bottom_border, theme);

    let mut lines = Vec::new();
    let separator_width = content_area.width; // account for padding
    let separator = "─".repeat(separator_width as usize);

    for (i, task) in state.items.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(separator.clone()).style(Style::default().fg(theme.muted)));
        }
        lines.push(highlight_todo_task(task, theme));
    }
    if lines.is_empty() {
        lines.push(Line::from("No pending tasks").style(Style::default().fg(theme.muted)));
    }
    let content_height = Paragraph::new(lines.clone())
        .wrap(Wrap { trim: true })
        .line_count(content_area.width);
    let pad_top = content_area
        .height
        .saturating_sub(content_height.min(u16::MAX as usize) as u16)
        / 2;

    let inner_block = Block::default()
        .style(theme.bg_style())
        .padding(Padding::new(0, 0, pad_top, 0));

    let paragraph = Paragraph::new(lines)
        .style(theme.bg_style())
        .block(inner_block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, content_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_preserves_task_order_filter_and_unicode_source() {
        let dir = tempfile::tempdir().unwrap();
        let storage = crate::storage::Storage {
            data_dir: dir.path().into(),
            config_dir: dir.path().into(),
            notes_dir: dir.path().into(),
            templates_dir: dir.path().into(),
            skip_dir_patterns: vec![],
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        std::fs::write(dir.path().join("todo.txt"),
            "plain task\nx (A) completed\n(é) malformed\n2024-02-30 invalid\n(B) later\n(A) 家  +项目 @iş due:yarın\n(A) second\n2024-02-29 dated\n").unwrap();
        let mut state = TodoState::default();
        update_todo_state(&storage, &mut state);
        assert_eq!(
            state.items,
            [
                "(A) 家  +项目 @iş due:yarın",
                "(A) second",
                "(B) later",
                "2024-02-29 dated"
            ]
        );
        let line = highlight_todo_task(&state.items[0], &AppThemeColors::default());
        assert_eq!(
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            state.items[0]
        );
    }
}
