use std::borrow::Cow;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui_textarea::{TextArea, CursorMove, Input};
use crate::keybinds::Keybinds;
use crate::text_edit::apply_text_shortcuts;

mod list;
mod edit;
mod help;

pub use list::{handle_list_keys, handle_list_mouse};
pub use edit::{handle_edit_keys, handle_edit_mouse};
pub use help::handle_help_keys;

pub fn handle_popup_text_input(
    key: KeyEvent,
    input: &mut TextArea<'static>,
    keybinds: &Keybinds,
) -> bool {
    if !apply_text_shortcuts(keybinds, input, key) {
        input.input(Input::from(key));
    }
    true
}

pub fn move_textarea_cursor_to_mouse(
    textarea: &mut TextArea,
    body_inner: Rect,
    mouse_col: u16,
    mouse_row: u16,
) {
    if textarea.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }

    let (scroll_row, scroll_col) = crate::ui::get_textarea_scroll(textarea);

    let row = mouse_row.saturating_sub(body_inner.y) as usize + scroll_row;
    let col = mouse_col.saturating_sub(body_inner.x) as usize + scroll_col;

    let max_row = textarea.lines().len().saturating_sub(1);
    let target_row = row.min(max_row);
    let max_col = textarea.lines()[target_row].chars().count();
    let target_col = col.min(max_col);

    textarea.move_cursor(CursorMove::Jump(target_row as u16, target_col as u16));
}

pub fn edit_view_input_areas(
    area: Rect,
    md_preview: bool,
    line_count: usize,
    show_line_numbers: bool,
) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let title_inner = Rect::new(
        chunks[1].x + 2,
        chunks[1].y + 1,
        chunks[1].width.saturating_sub(4),
        chunks[1].height.saturating_sub(2),
    );

    let body_area = if md_preview {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        Rect::new(
            content_chunks[0].x,
            chunks[2].y,
            content_chunks[0].width,
            chunks[2].height,
        )
    } else {
        chunks[2]
    };

    let gutter_width = if show_line_numbers {
        (line_count.max(1).to_string().len() as u16) + 1
    } else {
        0
    };

    let body_inner = Rect::new(
        body_area.x + gutter_width,
        body_area.y,
        body_area.width.saturating_sub(gutter_width + 2),
        body_area.height,
    );

    (title_inner, body_inner)
}

pub fn edit_view_md_preview_area(area: Rect) -> Option<Rect> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(area);

    let preview_area = Rect::new(
        content_chunks[2].x,
        chunks[2].y,
        content_chunks[2].width,
        chunks[2].height,
    );

    Some(Rect::new(
        preview_area.x + 2,
        preview_area.y + 1,
        preview_area.width.saturating_sub(4),
        preview_area.height.saturating_sub(2),
    ))
}

pub fn contains_cell(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

pub fn make_title_editor(
    initial: &str,
    highlight_fg: Color,
    highlight_bg: Color,
) -> TextArea<'static> {
    let mut title = if initial.is_empty() {
        TextArea::default()
    } else {
        TextArea::from([initial.to_string()])
    };
    title.set_cursor_style(Style::default().fg(highlight_fg).bg(highlight_bg));
    title
}

pub fn get_title_text<'a>(title_editor: &'a TextArea<'static>) -> Cow<'a, str> {
    let lines = title_editor.lines();

    if lines.len() == 1 {
        let line = lines[0].trim();
        if !line.contains(['\r', '\n']) {
            return Cow::Borrowed(line);
        }
    }

    Cow::Owned(
        lines
            .join(" ")
            .replace(['\r', '\n'], " ")
            .trim()
            .to_string(),
    )
}
