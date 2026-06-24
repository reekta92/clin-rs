use crate::debug_log;
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

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use crate::app::App;

/// Check if the key event should cancel/close a popup.
/// Returns `true` if the key matches `ListAction::Cancel`, or if it matches
/// `ListAction::Quit` and `!has_text_input` (to avoid stealing printable keys).
///
/// When `has_text_input` is true, Cancel matches are filtered to exclude
/// bare (unmodified) `Char` keypresses, so keys like `n` type into the text
/// field instead of closing the popup. Modifier combos like `Ctrl+N` and
/// non-printable keys like `Esc` still cancel.
pub fn is_cancel_popup(keybinds: &crate::keybinds::Keybinds, key: &crossterm::event::KeyEvent, has_text_input: bool) -> bool {
    let cancel = keybinds.matches_list(crate::keybinds::ListAction::Cancel, key);
    let cancel_triggered = if has_text_input && cancel {
        // In text-input mode, only non-printable keys and modifier combos cancel.
        // Bare Char (letter, digit, symbol) goes to the text input.
        let bare_char = matches!(key.code, KeyCode::Char(_))
            && !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::META);
        !bare_char
    } else {
        cancel
    };
    cancel_triggered || (!has_text_input && keybinds.matches_list(crate::keybinds::ListAction::Quit, key))
}

/// Handle global popups (tag, search, create_note, folder, goals, import,
/// trash_view, confirm, folder_picker, template, note_rename, theme, sort,
/// create_format) and command palette input.
/// Returns `true` if the event was consumed, `false` otherwise.
pub fn handle_global_popups_and_palette(
    app: &mut App,
    event: crossterm::event::Event,
    _terminal_area: Rect,
) -> bool {
    match event {
        crossterm::event::Event::Key(key) => {
            if key.kind != crossterm::event::KeyEventKind::Press {
                return false;
            }

            // Command palette
            if let Some(mut palette) = app.command_palette.take() {
                if palette.handle_input(key, app) {
                    if key.code == KeyCode::Enter
                        && let Some(selected_idx) = palette.state.selected()
                        && let Some(item) = palette.items.get(selected_idx)
                    {
                        let action_id = item.id.clone();
                        debug_log!(app, Info, "event", "Command palette: {action_id}");
                        let note_id = palette.context_note_id.clone();
                        if let Err(e) =
                            crate::actions::execute_action(&action_id, app, note_id.as_deref())
                        {
                            app.set_temporary_status(&format!("Action failed: {e}"));
                        }
                    }
                    return true;
                }
                app.command_palette = Some(palette);
                return true;
            }


            if let Some((mut popup, format)) = app.popups.create_note.take() {
                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.create_note = None;
                } else if key.code == KeyCode::Enter {
                    app.popups.create_note = Some((popup, format));
                    app.confirm_create_note();
                } else {
                    crate::events::handle_popup_text_input(key, &mut popup.input, &app.keybinds);
                    app.popups.create_note = Some((popup, format));
                }
                return true;
            }

            if let Some(mut popup) = app.popups.import.take() {
                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.import = None;
                } else if key.code == KeyCode::Enter {
                    app.popups.import = Some(popup);
                    app.confirm_import();
                } else {
                    crate::events::handle_popup_text_input(key, &mut popup.input, &app.keybinds);
                    app.popups.import = Some(popup);
                }
                return true;
            }

            if let Some(mut popup) = app.popups.folder.take() {
                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.folder = None;
                } else if key.code == KeyCode::Enter {
                    app.popups.folder = Some(popup);
                    app.confirm_folder_popup();
                } else {
                    crate::events::handle_popup_text_input(key, &mut popup.input, &app.keybinds);
                    app.popups.folder = Some(popup);
                }
                return true;
            }

            if let Some(mut popup) = app.popups.tag.take() {
                if app.popups.confirm.is_some() {
                    app.popups.tag = Some(popup);
                    let confirm_key = key;
                    if confirm_key.code == KeyCode::Left || confirm_key.code == KeyCode::Char('h')
                    {
                        app.confirm_popup_select_confirm();
                    } else if confirm_key.code == KeyCode::Right
                        || confirm_key.code == KeyCode::Char('l')
                    {
                        app.confirm_popup_select_cancel();
                    } else if confirm_key.code == KeyCode::Tab {
                        app.confirm_popup_toggle_button();
                    } else if confirm_key.code == KeyCode::Enter
                        || confirm_key.code == KeyCode::Char('y')
                        || confirm_key.code == KeyCode::Char('Y')
                    {
                        app.confirm_popup_activate();
                    } else if confirm_key.code == KeyCode::Char('n')
                        || confirm_key.code == KeyCode::Char('N')
                        || crate::events::is_cancel_popup(&app.keybinds, &confirm_key, false)
                    {
                        app.cancel_confirm();
                    }
                    return true;
                }

                if key.code == KeyCode::Char('s')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    let tag_text = popup.input.lines().join("");
                    let tag = tag_text
                        .split(',')
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    if let Some(tag) = tag {
                        app.list.tag_to_assign = Some(tag);
                        app.popups.tag = None;
                        app.list.list_mode = crate::list_view::ListMode::Select;
                        app.list.selected_indices.clear();
                        app.list.selected_indices.insert(app.list.visual_index);
                        app.set_temporary_status_static(
                            "TAG MODE: Select notes to apply tag, Enter to confirm, Esc to cancel",
                        );
                    } else {
                        app.popups.tag = None;
                        app.set_temporary_status_static("Enter a tag name first");
                    }
                    return true;
                }

                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.tag = None;
                    return true;
                }
                match key.code {
                    KeyCode::Tab => {
                        if popup.focus == crate::popups::TagPopupFocus::Input {
                            if popup.suggestions.is_empty() {
                                popup.focus = crate::popups::TagPopupFocus::AllTagsList;
                            } else {
                                app.popups.tag = Some(popup);
                                app.accept_tag_suggestion();
                                return true;
                            }
                        } else {
                            popup.focus = crate::popups::TagPopupFocus::Input;
                        }
                        app.popups.tag = Some(popup);
                    }
                    KeyCode::BackTab => {
                        popup.focus = match popup.focus {
                            crate::popups::TagPopupFocus::Input => {
                                crate::popups::TagPopupFocus::AllTagsList
                            }
                            crate::popups::TagPopupFocus::AllTagsList => {
                                crate::popups::TagPopupFocus::Input
                            }
                        };
                        app.popups.tag = Some(popup);
                    }
                    _ => match popup.focus {
                        crate::popups::TagPopupFocus::Input => {
                            if key.code == KeyCode::Enter {
                                app.popups.tag = Some(popup);
                                app.confirm_manage_tags();
                            } else if key.code == KeyCode::Char('D')
                                && key.modifiers.contains(KeyModifiers::SHIFT)
                            {
                                if let Some(tag) =
                                    popup.suggestions.get(popup.suggestion_index).cloned()
                                {
                                    app.begin_delete_tag_with_name(tag);
                                }
                            } else {
                                if !crate::text_edit::apply_text_shortcuts(
                                    &app.keybinds,
                                    &mut popup.input,
                                    key,
                                ) {
                                    popup.input.input(ratatui_textarea::Input::from(key));
                                }
                                app.popups.tag = Some(popup);
                                app.update_tag_suggestions();
                            }
                        }
                        crate::popups::TagPopupFocus::AllTagsList => match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                popup.all_tags_selected = popup.all_tags_selected.saturating_sub(1);
                                app.popups.tag = Some(popup);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if popup.all_tags_selected + 1 < popup.all_tags.len() {
                                    popup.all_tags_selected += 1;
                                }
                                app.popups.tag = Some(popup);
                            }
                            KeyCode::Char('d') | KeyCode::Delete => {
                                if let Some(tag) =
                                    popup.all_tags.get(popup.all_tags_selected).cloned()
                                {
                                    app.popups.tag = Some(popup);
                                    app.begin_delete_tag_with_name(tag);
                                }
                            }
                            _ => {
                                app.popups.tag = Some(popup);
                            }
                        },
                    },
                }
                return true;
            }

            if let Some(mut popup) = app.popups.goals.take() {
                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.goals = None;
                } else if key.code == KeyCode::Enter {
                    app.popups.goals = Some(popup);
                    app.confirm_goals_popup();
                } else {
                    crate::events::handle_popup_text_input(key, &mut popup.input, &app.keybinds);
                    app.popups.goals = Some(popup);
                }
                return true;
            }

            if let Some(mut popup) = app.popups.note_rename.take() {
                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.note_rename = None;
                } else if key.code == KeyCode::Enter {
                    app.popups.note_rename = Some(popup);
                    app.confirm_rename_note();
                } else {
                    if !crate::text_edit::apply_text_shortcuts(&app.keybinds, &mut popup.input, key)
                    {
                        popup.input.input(ratatui_textarea::Input::from(key));
                    }
                    app.popups.note_rename = Some(popup);
                }
                return true;
            }

            if let Some(mut popup) = app.popups.search.take() {
                let has_title = !popup.title_results.is_empty();
                let has_grep = !popup.grep_results.is_empty();
                let has_results = has_title || has_grep;

                let grep_prev_visible =
                    |p: &crate::popups::SearchPopup, cur: usize| -> usize {
                        if cur == 0 {
                            return 0;
                        }
                        let mut i = cur - 1;
                        loop {
                            if p.grep_is_header[i] {
                                return i;
                            }
                            let mut parent = i;
                            while parent > 0 && !p.grep_is_header[parent] {
                                parent -= 1;
                            }
                            if p.grep_expanded.contains(&parent) {
                                return i;
                            }
                            if i == 0 {
                                return 0;
                            }
                            i -= 1;
                        }
                    };
                let grep_next_visible =
                    |p: &crate::popups::SearchPopup, cur: usize| -> usize {
                        let mut i = cur + 1;
                        while i < p.grep_results.len() {
                            if p.grep_is_header[i] {
                                return i;
                            }
                            let mut parent = i;
                            while parent > 0 && !p.grep_is_header[parent] {
                                parent -= 1;
                            }
                            if p.grep_expanded.contains(&parent) {
                                return i;
                            }
                            i += 1;
                        }
                        cur
                    };

                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.search = Some(popup);
                    app.cancel_search();
                    return true;
                }
                match key.code {
                    KeyCode::Tab | KeyCode::BackTab => {
                        popup.focus = match popup.focus {
                            crate::popups::SearchFocus::Input if has_results => {
                                crate::popups::SearchFocus::Results
                            }
                            _ => crate::popups::SearchFocus::Input,
                        };
                        app.popups.search = Some(popup);
                    }
                    KeyCode::Enter => {
                        if popup.focus == crate::popups::SearchFocus::Results && has_results {
                            app.popups.search = Some(popup);
                            app.jump_to_selected_result();
                            app.confirm_search();
                        } else {
                            app.popups.search = Some(popup);
                            app.confirm_search();
                        }
                    }
                    KeyCode::Char('l') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.search = Some(popup);
                            app.update_search();
                        } else if has_grep
                            && popup
                                .grep_is_header
                                .get(popup.grep_selected)
                                .copied()
                                .unwrap_or(false)
                        {
                            if popup.grep_expanded.contains(&popup.grep_selected) {
                                popup.grep_expanded.remove(&popup.grep_selected);
                            } else {
                                popup.grep_expanded.insert(popup.grep_selected);
                            }
                            app.popups.search = Some(popup);
                        } else if has_results {
                            app.popups.search = Some(popup);
                            app.jump_to_selected_result();
                            app.confirm_search();
                        } else {
                            app.popups.search = Some(popup);
                            app.update_search();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.search = Some(popup);
                            app.update_search();
                        } else if has_grep {
                            popup.grep_selected = grep_prev_visible(&popup, popup.grep_selected);
                            app.popups.search = Some(popup);
                        } else if has_title {
                            popup.title_selected = popup.title_selected.saturating_sub(1);
                            app.popups.search = Some(popup);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.search = Some(popup);
                            app.update_search();
                        } else if has_grep {
                            popup.grep_selected = grep_next_visible(&popup, popup.grep_selected);
                            app.popups.search = Some(popup);
                        } else if has_title {
                            if popup.title_selected + 1 < popup.title_results.len() {
                                popup.title_selected += 1;
                            }
                            app.popups.search = Some(popup);
                        }
                    }
                    KeyCode::Right | KeyCode::Char(' ') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.search = Some(popup);
                            app.update_search();
                        } else if has_grep
                            && popup
                                .grep_is_header
                                .get(popup.grep_selected)
                                .copied()
                                .unwrap_or(false)
                        {
                            popup.grep_expanded.insert(popup.grep_selected);
                            app.popups.search = Some(popup);
                        } else {
                            popup.focus = crate::popups::SearchFocus::Input;
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.search = Some(popup);
                            app.update_search();
                        }
                    }
                    KeyCode::Left => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.search = Some(popup);
                            app.update_search();
                        } else if has_grep
                            && popup
                                .grep_is_header
                                .get(popup.grep_selected)
                                .copied()
                                .unwrap_or(false)
                        {
                            popup.grep_expanded.remove(&popup.grep_selected);
                            app.popups.search = Some(popup);
                        } else {
                            popup.focus = crate::popups::SearchFocus::Input;
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.search = Some(popup);
                            app.update_search();
                        }
                    }
                    _ => {
                        popup.focus = crate::popups::SearchFocus::Input;
                        if !crate::text_edit::apply_text_shortcuts(
                            &app.keybinds,
                            &mut popup.input,
                            key,
                        ) {
                            popup.input.input(ratatui_textarea::Input::from(key));
                        }
                        app.popups.search = Some(popup);
                        app.update_search();
                    }
                }
                return true;
            }

            if app.popups.confirm.is_some() {
                app.seq_matcher.clear();
                if key.code == KeyCode::Left || key.code == KeyCode::Char('h') {
                    app.confirm_popup_select_confirm();
                } else if key.code == KeyCode::Right || key.code == KeyCode::Char('l') {
                    app.confirm_popup_select_cancel();
                } else if key.code == KeyCode::Tab {
                    app.confirm_popup_toggle_button();
                } else if key.code == KeyCode::Enter {
                    app.confirm_popup_activate();
                } else if crate::events::is_cancel_popup(&app.keybinds, &key, false) {
                    app.cancel_confirm();
                } else if app.keybinds.matches_list(crate::keybinds::ListAction::Confirm, &key) {
                    app.confirm_action();
                } else if app.keybinds.matches_list(crate::keybinds::ListAction::Cancel, &key) {
                    app.cancel_confirm();
                }
                return true;
            }

            if let Some(trash) = &mut app.popups.trash_view {
                app.seq_matcher.clear();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        trash.selected = trash.selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if trash.selected + 1 < trash.items.len() {
                            trash.selected += 1;
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Enter => {
                        app.restore_from_trash();
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        app.begin_delete_from_trash();
                    }
                    KeyCode::Char('E') => {
                        app.begin_empty_trash();
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.close_trash_view();
                    }
                    _ => {}
                }
                return true;
            }

            if let Some(mut picker) = app.popups.folder_picker.take() {
                app.seq_matcher.clear();
                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.folder_picker = None;
                    return true;
                }
                match key.code {
                    KeyCode::Tab => {
                        picker.focus = match picker.focus {
                            crate::app::FolderPickerFocus::Search => {
                                crate::app::FolderPickerFocus::Results
                            }
                            crate::app::FolderPickerFocus::Results => {
                                crate::app::FolderPickerFocus::Search
                            }
                        };
                        app.popups.folder_picker = Some(picker);
                    }
                    _ => match picker.focus {
                        crate::app::FolderPickerFocus::Results => match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                picker.selected = picker.selected.saturating_sub(1);
                                app.popups.folder_picker = Some(picker);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if picker.selected + 1 < picker.filtered_folders.len() {
                                    picker.selected += 1;
                                }
                                app.popups.folder_picker = Some(picker);
                            }
                            KeyCode::Enter | KeyCode::Char('l') => {
                                app.popups.folder_picker = Some(picker);
                                app.confirm_move();
                            }
                            _ => {
                                app.popups.folder_picker = Some(picker);
                            }
                        },
                        crate::app::FolderPickerFocus::Search => {
                            let old_query = picker.input.lines().join("");
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut picker.input,
                                key,
                            ) {
                                picker.input.input(ratatui_textarea::Input::from(key));
                            }
                            let new_query = picker.input.lines().join("");
                            if old_query != new_query {
                                app.popups.folder_picker = Some(picker);
                                app.update_folder_picker_filter();
                            } else if key.code == KeyCode::Enter {
                                picker.focus = crate::app::FolderPickerFocus::Results;
                                app.popups.folder_picker = Some(picker);
                            } else {
                                app.popups.folder_picker = Some(picker);
                            }
                        }
                    },
                }
                return true;
            }

            if let Some(mut popup) = app.popups.template.take() {
                app.seq_matcher.clear();
                match key.code {
                    KeyCode::Tab | KeyCode::BackTab => {
                        popup.focus = match popup.focus {
                            crate::popups::TemplatePopupFocus::Search => {
                                crate::popups::TemplatePopupFocus::Results
                            }
                            crate::popups::TemplatePopupFocus::Results => {
                                crate::popups::TemplatePopupFocus::Search
                            }
                        };
                        app.popups.template = Some(popup);
                    }
                    KeyCode::Char('?') => {
                        if popup.focus == crate::popups::TemplatePopupFocus::Results {
                            app.popups.template = Some(popup);
                            app.open_help_page_with_tab(crate::app::HelpTab::Templates);
                        } else {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.template = Some(popup);
                            app.update_template_popup_filter();
                        }
                    }
                    KeyCode::Char('n') => {
                        if popup.focus == crate::popups::TemplatePopupFocus::Results {
                            app.popups.template = Some(popup);
                            app.create_template_from_popup();
                        } else {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.template = Some(popup);
                            app.update_template_popup_filter();
                        }
                    }
                    _ if crate::events::is_cancel_popup(&app.keybinds, &key, true) => {
                        app.close_template_popup();
                    }
                    _ => match popup.focus {
                        crate::popups::TemplatePopupFocus::Results => match key.code {
                            _ if app
                                .keybinds
                                .matches_list(crate::keybinds::ListAction::MoveUp, &key) =>
                            {
                                popup.selected = popup.selected.saturating_sub(1);
                                app.popups.template = Some(popup);
                            }
                            _ if app
                                .keybinds
                                .matches_list(crate::keybinds::ListAction::MoveDown, &key) =>
                            {
                                if popup.selected + 1 < popup.filtered_templates.len() {
                                    popup.selected += 1;
                                }
                                app.popups.template = Some(popup);
                            }
                            _ if app.keybinds.matches_list(
                                crate::keybinds::ListAction::Confirm,
                                &key,
                            ) || app.keybinds.matches_list(
                                crate::keybinds::ListAction::Open,
                                &key,
                            ) =>
                            {
                                app.popups.template = Some(popup);
                                app.select_template();
                            }
                            KeyCode::Char(' ') => {
                                app.popups.template = Some(popup);
                                app.edit_selected_template_from_popup();
                            }
                            KeyCode::Char('d') => {
                                app.popups.template = Some(popup);
                                app.begin_delete_selected_template_from_popup();
                            }
                            KeyCode::Char('h') => {
                                app.close_template_popup();
                            }
                            _ => {
                                app.popups.template = Some(popup);
                            }
                        },
                        crate::popups::TemplatePopupFocus::Search => match key.code {
                            _ if key.code == KeyCode::Enter => {
                                popup.focus = crate::popups::TemplatePopupFocus::Results;
                                app.popups.template = Some(popup);
                            }
                            _ => {
                                if !crate::text_edit::apply_text_shortcuts(
                                    &app.keybinds,
                                    &mut popup.input,
                                    key,
                                ) {
                                    popup.input.input(ratatui_textarea::Input::from(key));
                                }
                                app.popups.template = Some(popup);
                                app.update_template_popup_filter();
                            }
                        },
                    },
                }
                return true;
            }

            if let Some(mut popup) = app.popups.theme.take() {
                app.seq_matcher.clear();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        match popup.focus {
                            crate::app::ThemePopupFocus::ThemeList => {
                                popup.selected = popup.selected.saturating_sub(1);
                                app.popups.theme = Some(popup);
                                app.select_theme();
                                return true;
                            }
                            crate::app::ThemePopupFocus::GeneralBg => {
                                popup.focus = crate::app::ThemePopupFocus::ThemeList;
                                popup.selected = popup.themes.len().saturating_sub(1);
                            }
                            crate::app::ThemePopupFocus::GraphBg => {
                                popup.focus = crate::app::ThemePopupFocus::GeneralBg;
                            }
                        }
                        app.popups.theme = Some(popup);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        match popup.focus {
                            crate::app::ThemePopupFocus::ThemeList => {
                                if popup.selected + 1 < popup.themes.len() {
                                    popup.selected += 1;
                                    app.popups.theme = Some(popup);
                                    app.select_theme();
                                    return true;
                                } else {
                                    popup.focus = crate::app::ThemePopupFocus::GeneralBg;
                                }
                            }
                            crate::app::ThemePopupFocus::GeneralBg => {
                                popup.focus = crate::app::ThemePopupFocus::GraphBg;
                            }
                            crate::app::ThemePopupFocus::GraphBg => {
                                popup.focus = crate::app::ThemePopupFocus::ThemeList;
                                popup.selected = 0;
                            }
                        }
                        app.popups.theme = Some(popup);
                    }
                    KeyCode::Tab => {
                        match popup.focus {
                            crate::app::ThemePopupFocus::ThemeList => {
                                popup.focus = crate::app::ThemePopupFocus::GeneralBg
                            }
                            crate::app::ThemePopupFocus::GeneralBg => {
                                popup.focus = crate::app::ThemePopupFocus::GraphBg
                            }
                            crate::app::ThemePopupFocus::GraphBg => {
                                popup.focus = crate::app::ThemePopupFocus::ThemeList
                            }
                        }
                        app.popups.theme = Some(popup);
                    }
                    _ if app
                        .keybinds
                        .matches_list(crate::keybinds::ListAction::Confirm, &key) =>
                    {
                        let is_list =
                            matches!(popup.focus, crate::app::ThemePopupFocus::ThemeList);
                        app.popups.theme = Some(popup);
                        app.select_theme();
                        if is_list {
                            app.close_theme_popup();
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Char(' ') => {
                        app.popups.theme = Some(popup);
                        app.select_theme();
                    }
                    _ if crate::events::is_cancel_popup(&app.keybinds, &key, false) =>
                    {
                        app.close_theme_popup();
                    }
                    _ => {
                        app.popups.theme = Some(popup);
                    }
                }
                return true;
            }


            if let Some(mut popup) = app.popups.icon_mode.take() {
                app.seq_matcher.clear();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        popup.selected = popup.selected.saturating_sub(1);
                        app.popups.icon_mode = Some(popup);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.selected < 2 {
                            popup.selected += 1;
                        }
                        app.popups.icon_mode = Some(popup);
                    }
                    _ if app
                        .keybinds
                        .matches_list(crate::keybinds::ListAction::Confirm, &key) =>
                    {
                        app.popups.icon_mode = Some(popup);
                        app.select_icon_mode();
                    }
                    _ if crate::events::is_cancel_popup(&app.keybinds, &key, false) =>
                    {
                        app.close_icon_mode_popup();
                    }
                    _ => {
                        app.popups.icon_mode = Some(popup);
                    }
                }
                return true;
            }

            if let Some(mut popup) = app.popups.sort.take() {
                app.seq_matcher.clear();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        popup.selected = popup.selected.saturating_sub(1);
                        app.popups.sort = Some(popup);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.selected < 3 {
                            popup.selected += 1;
                        }
                        app.popups.sort = Some(popup);
                    }
                    _ if app
                        .keybinds
                        .matches_list(crate::keybinds::ListAction::Confirm, &key) =>
                    {
                        app.popups.sort = Some(popup);
                        app.select_sort();
                    }
                    _ if crate::events::is_cancel_popup(&app.keybinds, &key, false) =>
                    {
                        app.close_sort_popup();
                    }
                    _ => {
                        app.popups.sort = Some(popup);
                    }
                }
                return true;
            }

            if let Some(mut popup) = app.popups.create_format.take() {
                app.seq_matcher.clear();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        popup.selected = popup.selected.saturating_sub(1);
                        app.popups.create_format = Some(popup);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.selected < 3 {
                            popup.selected += 1;
                        }
                        app.popups.create_format = Some(popup);
                    }
                    _ if app
                        .keybinds
                        .matches_list(crate::keybinds::ListAction::Confirm, &key) =>
                    {
                        app.popups.create_format = Some(popup);
                        app.confirm_create_format();
                    }
                    _ if crate::events::is_cancel_popup(&app.keybinds, &key, false) =>
                    {
                        app.close_create_format_popup();
                    }
                    _ => {
                        app.popups.create_format = Some(popup);
                    }
                }
                return true;
            }


            false
        }
        _ => false,
    }
}
