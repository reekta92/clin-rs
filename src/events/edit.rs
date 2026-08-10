use crate::actions::Action;
use crate::app::{App, EditFocus, EditSidebar};
use crate::keybinds::EditAction;
use crate::text_edit::{MouseTextSelection, apply_text_shortcuts};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui_textarea::Input;

use super::{
    contains_cell, edit_view_input_areas, edit_view_md_preview_area, get_title_text,
    make_title_editor, move_textarea_cursor_to_mouse,
};

fn leave_editor(app: &mut App, focus: &mut EditFocus) {
    app.editor.find_popup = None;
    let prev_id = app.editor.editing_id.clone();
    app.autosave();
    let new_id = app.editor.editing_id.clone();
    app.back_to_list(prev_id.as_deref(), new_id.as_deref());
    *focus = EditFocus::Body;
}

/// Collect all lines containing the query (case-insensitive), returning (line_index, line_text) pairs.
fn collect_find_results(
    document: &crate::editor_document::EditorDocument,
    query_lower: &str,
) -> Vec<(usize, String)> {
    document
        .lines()
        .iter()
        .enumerate()
        .filter(|(_, line)| line.to_lowercase().contains(query_lower))
        .map(|(index, line)| (index, line.to_string()))
        .collect()
}

/// Jump the cursor to the next search match starting at or after the current cursor position.
/// Wraps around to the beginning of the document. Returns true if a match was found.
fn jump_to_next_search_match(
    document: &mut crate::editor_document::EditorDocument,
    query: &str,
) -> bool {
    if query.is_empty() {
        return false;
    }
    let query_lower = query.to_lowercase();
    let cursor = document.cursor();
    let lines = document.lines();
    if lines.is_empty() {
        return false;
    }
    for offset in 0..lines.len() {
        let row = (cursor.row + offset) % lines.len();
        let from_col = if offset == 0 { cursor.col } else { 0 };
        let line_lower = lines[row].to_lowercase();
        for (byte_offset, _) in line_lower.match_indices(&query_lower) {
            let col = line_lower[..byte_offset].chars().count();
            if col >= from_col {
                document.move_cursor(ratatui_textarea::CursorMove::Jump(row as u16, col as u16));
                return true;
            }
        }
    }
    false
}

/// Count total matches and current-match index (1-based) for a plain-text query.
/// Returns `None` when the query is empty or there are zero matches.
fn search_match_stats(
    lines: &[String],
    (row, col): (usize, usize),
    query: &str,
) -> Option<(usize, usize)> {
    if query.is_empty() {
        return None;
    }
    let ql = query.to_lowercase();
    let total: usize = lines
        .iter()
        .map(|l| l.to_lowercase().matches(&ql).count())
        .sum();
    if total == 0 {
        return None;
    }
    let before_row: usize = lines[..row]
        .iter()
        .map(|l| l.to_lowercase().matches(&ql).count())
        .sum();
    let line = &lines[row];
    let line_lower = line.to_lowercase();
    let on_row = line_lower
        .match_indices(&ql)
        .take_while(|(b, _)| line_lower[..*b].chars().count() <= col)
        .count();
    let current = before_row + on_row;
    Some((current.min(total), total))
}

pub fn handle_edit_keys(app: &mut App, key: KeyEvent, focus: &mut EditFocus) -> bool {
    // --- Go-to-line input popup ---
    if app.editor.go_to_line_input.is_some() {
        app.seq_matcher.clear();
        let input = app.editor.go_to_line_input.take();
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => {
                if let Some(line_str) = &input
                    && let Ok(line) = line_str.parse::<usize>()
                {
                    let target = line.saturating_sub(1); // 1-based → 0-based
                    let max = app.editor.body.lines().len().saturating_sub(1);
                    let row = target.min(max);
                    app.editor
                        .body
                        .move_cursor(ratatui_textarea::CursorMove::Jump(row as u16, 0));
                    app.request_editor_preview_update();
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let mut s = input.unwrap_or_default();
                s.push(c);
                app.editor.go_to_line_input = Some(s);
            }
            KeyCode::Backspace => {
                let mut s = input.unwrap_or_default();
                s.pop();
                if !s.is_empty() {
                    app.editor.go_to_line_input = Some(s);
                }
            }
            _ => {
                // Re-insert unhandled input
                if let Some(s) = input {
                    app.editor.go_to_line_input = Some(s);
                }
            }
        }
        return false;
    }

    if let Some(popup) = &mut app.editor.find_popup {
        app.seq_matcher.clear();
        match crate::ui::quick_search::handle_quick_search_keys(popup, key, &app.keybinds, 10) {
            crate::ui::quick_search::QuickSearchAction::Submit => {
                if let Some(&(line_idx, ref line_text)) = popup.results.get(popup.selected) {
                    let ql = popup.query().to_lowercase();
                    let col = line_text
                        .to_lowercase()
                        .find(&ql)
                        .map(|b| line_text[..b].chars().count())
                        .unwrap_or(0);
                    app.editor
                        .body
                        .move_cursor(ratatui_textarea::CursorMove::Jump(
                            line_idx as u16,
                            col as u16,
                        ));
                }
                app.editor.find_popup = None;
            }
            crate::ui::quick_search::QuickSearchAction::Cancel => {
                app.editor.find_popup = None;
            }
            crate::ui::quick_search::QuickSearchAction::Edited => {
                let query_lower = popup.query().to_lowercase();
                if query_lower.is_empty() {
                    popup.results.clear();
                } else {
                    popup.results = collect_find_results(&app.editor.body, &query_lower);
                }
                if popup.selected >= popup.results.len() {
                    popup.selected = popup.results.len().saturating_sub(1);
                }
                popup.scroll_to_selected(10);
                let query = popup.query();
                let _ = jump_to_next_search_match(&mut app.editor.body, &query);
                let cursor = app.editor.body.cursor();
                popup.info =
                    search_match_stats(app.editor.body.lines(), (cursor.row, cursor.col), &query)
                        .map(|(n, total)| format!("{n}/{total}"));
            }
            _ => {}
        }
        return false;
    }
    if app.editor.link_preview && crate::events::is_cancel_popup(&app.keybinds, &key, false) {
        app.close_link_preview();
        return false;
    }

    // Esc: leave editor.
    if key.modifiers == KeyModifiers::NONE && key.code == KeyCode::Esc {
        leave_editor(app, focus);
        return false;
    }

    let seq = app.config.sequences_enabled();
    let counts = app.config.counts_enabled();
    match app
        .keybinds
        .resolve_edit(&mut app.seq_matcher, key, seq, counts)
    {
        crate::keybinds::MatchOutcome::Matched(action, _count) => match action {
            EditAction::CycleFocus => {
                *focus = match *focus {
                    EditFocus::Title => EditFocus::Body,
                    EditFocus::Body => {
                        if app.editor.sidebar != EditSidebar::None {
                            EditFocus::Sidebar
                        } else {
                            EditFocus::Title
                        }
                    }
                    EditFocus::Sidebar => EditFocus::Title,
                };
                return false;
            }
            EditAction::Back => {
                leave_editor(app, focus);
                return false;
            }
            EditAction::ToggleMarkdownPreview => {
                app.toggle_markdown_preview();
                if *focus == EditFocus::Sidebar {
                    *focus = EditFocus::Body;
                }
                return false;
            }
            EditAction::ToggleOutline => {
                app.toggle_outline_pane();
                if app.editor.sidebar == EditSidebar::None && *focus == EditFocus::Sidebar {
                    *focus = EditFocus::Body;
                }
                return false;
            }
            EditAction::ToggleLinks => {
                app.toggle_links_pane();
                if app.editor.sidebar == EditSidebar::None && *focus == EditFocus::Sidebar {
                    *focus = EditFocus::Body;
                }
                return false;
            }
            EditAction::PreviewLink => {
                app.open_link_preview();
                return false;
            }
            EditAction::PreviewPageUp => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    renderer.prev_page();
                    return false;
                }
            }
            EditAction::PreviewPageDown => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    renderer.next_page();
                    return false;
                }
            }
            EditAction::ManageSubnotes => {
                app.open_subnotes_popup();
                return false;
            }
            EditAction::PasteImage => {
                let action = &crate::actions::ocr::PasteImageAction;
                if let Err(e) = action.execute(app, None) {
                    app.set_temporary_status(&format!("Paste image failed: {e}"));
                }
                return false;
            }
            EditAction::InsertImageFromFile => {
                let action = &crate::actions::ocr::InsertImageFromFileAction;
                if let Err(e) = action.execute(app, None) {
                    app.set_temporary_status(&format!("Insert image failed: {e}"));
                }
                return false;
            }
            EditAction::GoToLine => {
                app.editor.go_to_line_input = if app.editor.go_to_line_input.is_some() {
                    None
                } else {
                    Some(String::new())
                };
                return false;
            }
            EditAction::InsertDate => {
                let s = chrono::Local::now()
                    .format(&app.config.editor.date_format)
                    .to_string();
                match *focus {
                    EditFocus::Title => {
                        let _ = app.editor.title_editor.insert_str(&s);
                    }
                    EditFocus::Body => {
                        let _ = app.editor.body.insert_str(&s);
                    }
                    EditFocus::Sidebar => {
                        // no-op
                    }
                }
                app.request_editor_preview_update();
                return false;
            }
            EditAction::ToggleWrap => {
                app.toggle_wrap();
                return false;
            }
            EditAction::InsertTab => {
                match *focus {
                    EditFocus::Title => {
                        let _ = app.editor.title_editor.insert_str("\t");
                    }
                    EditFocus::Body => {
                        let _ = app.editor.body.insert_str("\t");
                    }
                    EditFocus::Sidebar => {
                        // no-op
                    }
                }
                app.request_editor_preview_update();
                return false;
            }
            EditAction::Find => {
                let theme = &app.app_theme;
                let mut popup = crate::ui::quick_search::QuickSearch::new("Find", theme);
                let query_lower = popup.query().to_lowercase();
                if !query_lower.is_empty() {
                    popup.results = collect_find_results(&app.editor.body, &query_lower);
                }
                app.editor.find_popup = Some(popup);
                return false;
            }
            _ => {}
        },
        crate::keybinds::MatchOutcome::Pending => return false,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }
    if app
        .keybinds
        .matches_edit(EditAction::TogglePreviewFullscreen, &key)
    {
        app.toggle_preview_fullscreen();
        return false;
    }

    match *focus {
        EditFocus::Sidebar => {
            app.seq_matcher.clear();
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveUp, &key)
            {
                app.sidebar_move(-1);
                return false;
            }
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveDown, &key)
            {
                app.sidebar_move(1);
                return false;
            }
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::Confirm, &key)
            {
                app.sidebar_activate(focus);
                return false;
            }
            // Bare Esc / Back leaves sidebar focus → returns to Body (pane stays visible).
            if key.modifiers == KeyModifiers::NONE && key.code == KeyCode::Esc {
                *focus = EditFocus::Body;
                return false;
            }
            // Any other key is swallowed (no editor insertion while sidebar focused).
            return false;
        }
        EditFocus::Title => {
            app.seq_matcher.clear();
            if key.code == KeyCode::Enter {
                *focus = EditFocus::Body;
                return false;
            }
            if apply_text_shortcuts(&app.keybinds, &mut app.editor.title_editor, key) {
                return false;
            }
            if app.editor.title_editor.input(Input::from(key))
                && app.editor.title_editor.lines().len() > 1
            {
                let normalized =
                    get_title_text(&app.editor.title_editor).replace(['\r', '\n'], " ");
                app.editor.title_editor = make_title_editor(
                    &normalized,
                    app.app_theme.highlight_fg,
                    app.app_theme.highlight_bg,
                );
            }
        }
        EditFocus::Body => {
            app.seq_matcher.clear();
            let revision = app.editor.body.revision();
            if apply_text_shortcuts(&app.keybinds, &mut app.editor.body, key) {
                if app.editor.body.revision() != revision {
                    app.request_editor_preview_update();
                }
                return false;
            }
            if app.editor.body.input(Input::from(key)).content_changed {
                app.request_editor_preview_update();
            }
        }
    }

    false
}

pub(crate) fn handle_edit_mouse(
    app: &mut App,
    mouse_event: MouseEvent,
    terminal_area: Rect,
    focus: &mut EditFocus,
    mouse_selection: &mut MouseTextSelection,
) {
    if let Some(popup) = &mut app.editor.find_popup {
        if let Some(action) = crate::ui::quick_search::handle_quick_search_mouse(
            popup,
            mouse_event,
            terminal_area,
            10,
            app.config.ui.icon_mode,
        ) {
            match action {
                crate::ui::quick_search::QuickSearchAction::Submit => {
                    if let Some(&(line_idx, ref line_text)) = popup.results.get(popup.selected) {
                        let ql = popup.query().to_lowercase();
                        let col = line_text
                            .to_lowercase()
                            .find(&ql)
                            .map(|b| line_text[..b].chars().count())
                            .unwrap_or(0);
                        app.editor
                            .body
                            .move_cursor(ratatui_textarea::CursorMove::Jump(
                                line_idx as u16,
                                col as u16,
                            ));
                    }
                    app.editor.find_popup = None;
                }
                crate::ui::quick_search::QuickSearchAction::Cancel => {
                    app.editor.find_popup = None;
                }
                crate::ui::quick_search::QuickSearchAction::Edited => {
                    let query_lower = popup.query().to_lowercase();
                    if query_lower.is_empty() {
                        popup.results.clear();
                    } else {
                        popup.results = collect_find_results(&app.editor.body, &query_lower);
                    }
                    if popup.selected >= popup.results.len() {
                        popup.selected = popup.results.len().saturating_sub(1);
                    }
                    popup.scroll_to_selected(10);
                    let query = popup.query();
                    let _ = jump_to_next_search_match(&mut app.editor.body, &query);
                    let cursor = app.editor.body.cursor();
                    popup.info = search_match_stats(
                        app.editor.body.lines(),
                        (cursor.row, cursor.col),
                        &query,
                    )
                    .map(|(n, total)| format!("{n}/{total}"));
                }
                crate::ui::quick_search::QuickSearchAction::Navigated => {}
            }
        }
        return;
    }

    let (title_inner, body_inner, sidebar_inner) = edit_view_input_areas(
        terminal_area,
        app.preview_fullscreen,
        app.editor.editor_preview_enabled,
        app.editor.body.lines().len(),
        app.editor.show_line_numbers,
        app.editor.sidebar,
        app.preview_position,
        app.editor.header_title_rect,
    );

    let md_area = if app.preview_fullscreen {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(terminal_area);
        Some(chunks[1])
    } else if app.editor.editor_preview_enabled {
        edit_view_md_preview_area(terminal_area, app.editor.sidebar, app.preview_position)
    } else {
        None
    };

    if let Some(md_area) = md_area
        && contains_cell(md_area, mouse_event.column, mouse_event.row)
    {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    let h = app.editor.last_body_height.max(1) as usize;
                    renderer.scroll_up(3.min(h));
                }
                return;
            }
            MouseEventKind::ScrollDown => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    let h = app.editor.last_body_height.max(1) as usize;
                    renderer.scroll_down(3.min(h), h);
                }
                return;
            }
            _ => {}
        }
    }
    if let Some(sb) = sidebar_inner
        && contains_cell(sb, mouse_event.column, mouse_event.row)
    {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                let len = app.sidebar_len();
                let viewport = app.editor.sidebar_list_rect.height as usize;
                app.editor.sidebar_scroll_offset =
                    crate::ui::scroll_viewport(app.editor.sidebar_scroll_offset, -1, len, viewport);
                app.editor.sidebar_selected = crate::ui::clamp_selected_to_view(
                    app.editor.sidebar_selected,
                    app.editor.sidebar_scroll_offset,
                    len,
                    viewport,
                );
                return;
            }
            MouseEventKind::ScrollDown => {
                let len = app.sidebar_len();
                let viewport = app.editor.sidebar_list_rect.height as usize;
                app.editor.sidebar_scroll_offset =
                    crate::ui::scroll_viewport(app.editor.sidebar_scroll_offset, 1, len, viewport);
                app.editor.sidebar_selected = crate::ui::clamp_selected_to_view(
                    app.editor.sidebar_selected,
                    app.editor.sidebar_scroll_offset,
                    len,
                    viewport,
                );
                return;
            }
            _ => {}
        }
    }

    if app.preview_fullscreen {
        return;
    }

    // Scrollbar drag/click on editor body
    if app.config.ui.scrollbars
        && let Some(meta) = app.editor.last_scroll
    {
        let max_pos = meta.content_len.saturating_sub(meta.viewport_len);
        let frac = app.editor.body_viewport_row as f32 / max_pos.max(1) as f32;
        if let Some(new_frac) = crate::ui::scrollbar::handle_scrollbar_mouse(
            &mouse_event,
            meta,
            frac,
            &mut app.editor.scroll_drag,
        ) {
            let new_pos = ((new_frac * max_pos as f32).round() as usize).min(max_pos);
            let delta = new_pos as i16 - app.editor.body_viewport_row as i16;
            if delta != 0 {
                app.scroll_editor(delta, 0);
            }
            return;
        }
    }

    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            mouse_selection.active = false;
            mouse_selection.dragged = false;
            if let Some(sb) = sidebar_inner
                && contains_cell(sb, mouse_event.column, mouse_event.row)
            {
                *focus = EditFocus::Sidebar;
                let len = app.sidebar_len();
                if let Some(clicked) = crate::ui::list_index_at(
                    mouse_event.row,
                    app.editor.sidebar_list_rect.y,
                    1,
                    app.editor.sidebar_scroll_offset,
                    len,
                ) {
                    let is_double_click = if let Some((lx, ly, lt)) = app.editor.last_sidebar_click
                    {
                        lx == mouse_event.column
                            && ly == mouse_event.row
                            && lt.elapsed().as_millis() < 500
                    } else {
                        false
                    };
                    app.editor.sidebar_selected = clicked;
                    if is_double_click {
                        app.sidebar_activate(focus);
                        app.editor.last_sidebar_click = None;
                    } else {
                        app.editor.last_sidebar_click = Some((
                            mouse_event.column,
                            mouse_event.row,
                            std::time::Instant::now(),
                        ));
                    }
                }
                return;
            }
            app.editor.last_sidebar_click = None;
            if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Body;
                let _ = app.editor.body.hit_test_cursor(
                    body_inner,
                    mouse_event.column,
                    mouse_event.row,
                    app.editor.body_viewport_row,
                    app.editor.body_viewport_col,
                );
                mouse_selection.begin(&mut app.editor.body);
            } else if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Title;
                move_textarea_cursor_to_mouse(
                    &mut app.editor.title_editor,
                    title_inner,
                    mouse_event.column,
                    mouse_event.row,
                    app.editor.title_viewport_row as usize,
                    app.editor.title_viewport_col as usize,
                );
                mouse_selection.begin(&mut app.editor.title_editor);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if mouse_selection.active {
                mouse_selection.mark_drag();
                if *focus == EditFocus::Body {
                    let _ = app.editor.body.hit_test_cursor(
                        body_inner,
                        mouse_event.column,
                        mouse_event.row,
                        app.editor.body_viewport_row,
                        app.editor.body_viewport_col,
                    );
                } else {
                    move_textarea_cursor_to_mouse(
                        &mut app.editor.title_editor,
                        title_inner,
                        mouse_event.column,
                        mouse_event.row,
                        app.editor.title_viewport_row as usize,
                        app.editor.title_viewport_col as usize,
                    );
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let notice = if *focus == EditFocus::Body {
                mouse_selection.finish(&mut app.editor.body)
            } else {
                mouse_selection.finish(&mut app.editor.title_editor)
            };
            if let Some(notice) = notice {
                app.set_temporary_status(notice);
            }
        }
        MouseEventKind::ScrollDown => {
            if *focus == EditFocus::Body {
                app.scroll_editor(3, 0);
            }
        }
        MouseEventKind::ScrollUp if *focus == EditFocus::Body => {
            app.scroll_editor(-3, 0);
        }
        _ => {}
    }
}
