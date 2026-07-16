use crate::actions::Action;
use crate::app::{App, ContextMenu, EditFocus, EditSidebar};
use crate::keybinds::EditAction;
use crate::text_edit::apply_text_shortcuts;
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
    editor: &ratatui_textarea::TextArea<'_>,
    query_lower: &str,
) -> Vec<(usize, String)> {
    editor
        .lines()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.to_lowercase().contains(query_lower))
        .map(|(i, l)| (i, l.to_string()))
        .collect()
}

/// Jump the cursor to the next search match starting at or after the current cursor position.
/// Wraps around to the beginning of the document. Returns true if a match was found.
fn jump_to_next_search_match(editor: &mut ratatui_textarea::TextArea<'_>, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }
    let ql = query.to_lowercase();
    let cursor = editor.cursor();
    let cur_row = cursor.0;
    let cur_col = cursor.1;
    let lines = editor.lines();
    let len = lines.len();
    if len == 0 {
        return false;
    }
    for offset in 0..len {
        let row = (cur_row + offset) % len;
        let from_col = if offset == 0 { cur_col } else { 0 };
        let line_lower = lines[row].to_lowercase();
        for (byte_off, _) in line_lower.match_indices(&ql) {
            let col = line_lower[..byte_off].chars().count();
            if col >= from_col {
                editor.move_cursor(ratatui_textarea::CursorMove::Jump(row as u16, col as u16));
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
    if let Some(crate::popups::ActivePopup::ContextMenu(mut menu)) = app.popups.active.take() {
        if crate::events::is_cancel_popup(&app.keybinds, &key, false) {
            app.popups.active = None;
            return false;
        }
        match key.code {
            _ if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveUp, &key) =>
            {
                menu.selected = menu.selected.saturating_sub(1);
                app.popups.active = Some(crate::popups::ActivePopup::ContextMenu(menu));
            }
            _ if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveDown, &key) =>
            {
                if menu.selected + 1 < menu.items.len() {
                    menu.selected += 1;
                }
                app.popups.active = Some(crate::popups::ActivePopup::ContextMenu(menu));
            }
            _ if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::Confirm, &key) =>
            {
                app.handle_menu_action(menu.selected, focus, &menu.items);
                app.popups.active = None;
            }
            _ => {
                app.popups.active = Some(crate::popups::ActivePopup::ContextMenu(menu));
            }
        }
        return false;
    }
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
                    let max = app.editor.editor.lines().len().saturating_sub(1);
                    let row = target.min(max);
                    app.editor
                        .editor
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
                        .editor
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
                    popup.results = collect_find_results(&app.editor.editor, &query_lower);
                }
                if popup.selected >= popup.results.len() {
                    popup.selected = popup.results.len().saturating_sub(1);
                }
                popup.scroll_to_selected(10);
                let query = popup.query();
                let _ = jump_to_next_search_match(&mut app.editor.editor, &query);
                let cursor = app.editor.editor.cursor();
                popup.info =
                    search_match_stats(app.editor.editor.lines(), (cursor.0, cursor.1), &query)
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

    // Universal back (override-proof): bare Esc leaves the editor. q types a letter.
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
                        let _ = app.editor.editor.insert_str(&s);
                    }
                    EditFocus::Sidebar => {}
                }
                app.request_editor_preview_update();
                return false;
            }
            EditAction::ToggleSoftWrap => {
                app.toggle_editor_soft_wrap();
                return false;
            }
            EditAction::Find => {
                let theme = &app.app_theme;
                if app.editor.find_popup.is_some() {
                    app.editor.find_popup = None;
                } else {
                    app.editor.find_popup =
                        Some(crate::ui::quick_search::QuickSearch::new(" Find ", theme));
                }
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
    if app
        .keybinds
        .matches_edit(EditAction::TogglePreviewWrap, &key)
    {
        app.toggle_preview_wrap();
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
                app.request_editor_preview_update();
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
            app.request_editor_preview_update();
        }
        EditFocus::Body => {
            app.seq_matcher.clear();
            if apply_text_shortcuts(&app.keybinds, &mut app.editor.editor, key) {
                app.request_editor_preview_update();
                return false;
            }
            if app.editor.editor.input(Input::from(key)) {
                app.request_editor_preview_update();
            }
        }
    }

    false
}

pub fn handle_edit_mouse(
    app: &mut App,
    mouse_event: MouseEvent,
    terminal_area: Rect,
    focus: &mut EditFocus,
    mouse_selecting: &mut bool,
    mouse_dragged: &mut bool,
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
                            .editor
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
                        popup.results = collect_find_results(&app.editor.editor, &query_lower);
                    }
                    if popup.selected >= popup.results.len() {
                        popup.selected = popup.results.len().saturating_sub(1);
                    }
                    popup.scroll_to_selected(10);
                    let query = popup.query();
                    let _ = jump_to_next_search_match(&mut app.editor.editor, &query);
                    let cursor = app.editor.editor.cursor();
                    popup.info =
                        search_match_stats(app.editor.editor.lines(), (cursor.0, cursor.1), &query)
                            .map(|(n, total)| format!("{n}/{total}"));
                }
                crate::ui::quick_search::QuickSearchAction::Navigated => {}
            }
        }
        return;
    }
    if let Some(crate::popups::ActivePopup::ContextMenu(menu)) = &app.popups.active {
        // Only handle left-clicks inside the menu (needs EditFocus).
        // Scroll and outside-dismiss are handled centrally in handle_global_popup_mouse.
        let w = menu.items.iter().map(|l| l.len() as u16).max().unwrap_or(0);
        let h = menu.items.len() as u16;
        let menu_rect = Rect::new(menu.x, menu.y, w, h);
        if contains_cell(menu_rect, mouse_event.column, mouse_event.row)
            && mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
        {
            let clicked_idx = mouse_event.row.saturating_sub(menu.y) as usize;
            if clicked_idx < menu.items.len() {
                let items = menu.items.clone();
                app.handle_menu_action(clicked_idx, focus, &items);
            }
            app.popups.active = None;
            return;
        }
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
        let (title_inner, body_inner, _sidebar_inner) = edit_view_input_areas(
            terminal_area,
            app.editor.editor_preview_enabled,
            app.editor.editor.lines().len(),
            app.editor.show_line_numbers,
            app.editor.sidebar,
            app.preview_position,
            app.editor.header_title_rect,
        );

        if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Title;
            if app.editor.title_editor.selection_range().is_none() {
                move_textarea_cursor_to_mouse(
                    &mut app.editor.title_editor,
                    title_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
            }
        } else if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Body;
            if app.editor.editor.selection_range().is_none() {
                move_textarea_cursor_to_mouse(
                    &mut app.editor.editor,
                    body_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
            }
        }
        let has_selection = match focus {
            EditFocus::Title => app.editor.title_editor.selection_range(),
            EditFocus::Body => app.editor.editor.selection_range(),
            EditFocus::Sidebar => None,
        };
        let items: Vec<&'static str> = if has_selection.is_some() {
            vec![" Copy ", " Cut ", " Paste ", " Select All "]
        } else {
            vec![" Paste ", " Select All "]
        };
        let max_x = terminal_area
            .width
            .saturating_sub(items.iter().map(|i| i.len() as u16).max().unwrap_or(14));
        let max_y = terminal_area.height.saturating_sub(items.len() as u16 + 2);
        app.popups.active = Some(crate::popups::ActivePopup::ContextMenu(ContextMenu {
            x: mouse_event.column.min(max_x),
            y: mouse_event.row.min(max_y),
            selected: 0,
            items,
        }));
    }

    let (title_inner, body_inner, sidebar_inner) = edit_view_input_areas(
        terminal_area,
        app.editor.editor_preview_enabled,
        app.editor.editor.lines().len(),
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
                    renderer.prev_page();
                }
                return;
            }
            MouseEventKind::ScrollDown => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    renderer.next_page();
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
                app.sidebar_move(-1);
                return;
            }
            MouseEventKind::ScrollDown => {
                app.sidebar_move(1);
                return;
            }
            _ => {}
        }
    }

    if app.preview_fullscreen {
        return;
    }

    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *mouse_selecting = false;
            *mouse_dragged = false;
            if let Some(sb) = sidebar_inner
                && contains_cell(sb, mouse_event.column, mouse_event.row)
            {
                *focus = EditFocus::Sidebar;
                let clicked_row = mouse_event.row as i32 - sb.y as i32 - 3;
                if clicked_row >= 0 {
                    let clicked = clicked_row as usize + app.editor.sidebar_scroll_offset;
                    let len = app.sidebar_len();
                    if clicked < len {
                        let is_double_click =
                            if let Some((lx, ly, lt)) = app.editor.last_sidebar_click {
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
                }
                return;
            }
            app.editor.last_sidebar_click = None;
            if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Body;
                move_textarea_cursor_to_mouse(
                    &mut app.editor.editor,
                    body_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
                app.editor.editor.start_selection();
                *mouse_selecting = true;
            } else if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Title;
                move_textarea_cursor_to_mouse(
                    &mut app.editor.title_editor,
                    title_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
                app.editor.title_editor.start_selection();
                *mouse_selecting = true;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if *mouse_selecting {
                *mouse_dragged = true;
                if *focus == EditFocus::Body {
                    move_textarea_cursor_to_mouse(
                        &mut app.editor.editor,
                        body_inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                } else {
                    move_textarea_cursor_to_mouse(
                        &mut app.editor.title_editor,
                        title_inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if *mouse_selecting && !*mouse_dragged {
                if *focus == EditFocus::Body {
                    app.editor.editor.cancel_selection();
                } else {
                    app.editor.title_editor.cancel_selection();
                }
            }
            *mouse_selecting = false;
            *mouse_dragged = false;
        }
        MouseEventKind::ScrollDown => {
            if *focus == EditFocus::Body {
                app.editor.editor.scroll((3, 0));
            }
        }
        MouseEventKind::ScrollUp if *focus == EditFocus::Body => {
            app.editor.editor.scroll((-3, 0));
        }
        _ => {}
    }
}
