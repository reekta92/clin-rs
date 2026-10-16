use crate::actions::Action;
use crate::app::{App, EditFocus, EditSidebar};
use crate::keybinds::EditAction;
use crate::text_edit::{MouseTextSelection, apply_text_shortcuts, update_selection_for_move};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui_textarea::Input;

use super::{
    contains_cell, edit_view_input_areas, edit_view_md_preview_area, get_title_text,
    make_title_editor, move_textarea_cursor_to_mouse,
};

fn leave_editor(app: &mut App, focus: &mut EditFocus) {
    if let Err(e) = app.autosave() {
        app.set_temporary_status(&e);
        return;
    }
    app.editor.find_popup = None;
    app.editor.link_complete = None;
    let prev_id = app.editor.editing_id.clone();
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

    // Completion only lives in the body; any other focus drops it.
    if *focus != EditFocus::Body {
        app.editor.link_complete = None;
    }
    // Inline `[[...]]` completion owns Up/Down/Tab/Enter/Esc while open.
    // Other keys fall through to normal handling; the Body arm refreshes.
    if *focus == EditFocus::Body
        && app.popups.confirm.is_none()
        && app.editor.link_complete.is_some()
    {
        let bare = key.modifiers == KeyModifiers::NONE;
        match key.code {
            KeyCode::Up if bare => {
                app.link_complete_move(-1);
                return false;
            }
            KeyCode::Down if bare => {
                app.link_complete_move(1);
                return false;
            }
            KeyCode::Tab if bare => {
                if app.accept_link_completion() {
                    return false;
                }
            }
            KeyCode::Enter if bare => {
                if app.accept_link_completion() {
                    return false;
                }
            }
            KeyCode::Esc if bare => {
                app.editor.link_complete = None;
                return false;
            }
            _ => {}
        }
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
                        if app.editor.file_tree_visible {
                            EditFocus::FileTree
                        } else if app.editor.sidebar != EditSidebar::None {
                            EditFocus::Sidebar
                        } else {
                            EditFocus::Title
                        }
                    }
                    EditFocus::FileTree => {
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
            EditAction::Save => {
                if app.autosave().is_ok() {
                    app.editor.autosave_status = crate::editor::AutosaveStatus::RecentlySaved;
                    app.editor.last_saved_time = Some(std::time::Instant::now());
                    app.editor.autosave_timer = None;
                }
                return true;
            }
            EditAction::ToggleMermaidRendering => {
                app.toggle_mermaid_rendering();
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
                if app.feature_disabled(
                    app.config.features.outline_view.is_enabled(),
                    "Outline view",
                    "outline_view",
                ) {
                    return false;
                }
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
            EditAction::ToggleFileTree => {
                app.toggle_file_tree();
                if !app.editor.file_tree_visible && *focus == EditFocus::FileTree {
                    *focus = EditFocus::Body;
                }
                return false;
            }
            EditAction::NewTab => {
                app.new_editor_tab();
                *focus = EditFocus::Title;
                return false;
            }
            EditAction::CloseTab => {
                let idx = app.editor.active_tab;
                app.close_editor_tab(idx);
                if app.mode != crate::app::ViewMode::Edit {
                    *focus = EditFocus::Body;
                }
                return false;
            }
            EditAction::NextTab => {
                app.next_editor_tab();
                return false;
            }
            EditAction::PrevTab => {
                app.prev_editor_tab();
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
                if app.feature_disabled(app.config.features.import.is_enabled(), "Import", "import")
                {
                    return false;
                }
                let action = &crate::actions::ocr::PasteImageAction;
                if let Err(e) = action.execute(app, None) {
                    app.set_temporary_status(&format!("Paste image failed: {e}"));
                }
                return false;
            }
            EditAction::InsertImageFromFile => {
                if app.feature_disabled(app.config.features.import.is_enabled(), "Import", "import")
                {
                    return false;
                }
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
                    EditFocus::Sidebar | EditFocus::FileTree => {
                        // no-op
                    }
                }
                app.request_editor_preview_update();
                return false;
            }
            EditAction::ToggleZenMode => {
                app.toggle_zen_mode();
                return false;
            }
            EditAction::ToggleWrap => {
                app.toggle_wrap();
                return false;
            }
            EditAction::CycleAlignment => {
                app.cycle_text_alignment();
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
                    EditFocus::Sidebar | EditFocus::FileTree => {
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
        EditFocus::FileTree => {
            app.seq_matcher.clear();
            // Same keys as the notes list tree view.
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveUp, &key)
            {
                app.file_tree_move(-1);
                return false;
            }
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveDown, &key)
            {
                app.file_tree_move(1);
                return false;
            }
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveLeft, &key)
                || app
                    .keybinds
                    .matches_list(crate::keybinds::ListAction::CollapseFolder, &key)
            {
                app.file_tree_collapse();
                return false;
            }
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::MoveRight, &key)
                || app
                    .keybinds
                    .matches_list(crate::keybinds::ListAction::ExpandFolder, &key)
            {
                app.file_tree_expand();
                return false;
            }
            if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::Confirm, &key)
            {
                app.file_tree_activate();
                return false;
            }
            if key.modifiers == KeyModifiers::NONE && key.code == KeyCode::Esc {
                *focus = EditFocus::Body;
                return false;
            }
            return false;
        }
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
            update_selection_for_move(&mut app.editor.title_editor, &key);
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
            app.write_draft();
        }
        EditFocus::Body => {
            app.seq_matcher.clear();
            // Context-aware Enter: on a [[wikilink]], follow it (open or
            // offer to create) instead of inserting a newline.
            if key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE {
                // Don't hijack Enter while a confirm popup is up.
                if app.popups.confirm.is_none() && app.follow_wikilink_under_cursor() {
                    app.request_editor_preview_update();
                    app.write_draft();
                    app.sync_link_completion(false);
                    return false;
                }
            }
            let revision = app.editor.body.revision();
            update_selection_for_move(&mut app.editor.body, &key);
            if apply_text_shortcuts(&app.keybinds, &mut app.editor.body, key) {
                let changed = app.editor.body.revision() != revision;
                if changed {
                    app.request_editor_preview_update();
                    app.write_draft();
                }
                app.sync_link_completion(changed);
                return false;
            }
            // Pair-aware Backspace: between an empty auto-inserted pair,
            // remove both halves in one keystroke.
            if app.config.editor.auto_close_pairs
                && key.code == KeyCode::Backspace
                && key.modifiers == KeyModifiers::NONE
                && crate::auto_pairs::is_empty_pair_at_cursor(&app.editor.body)
            {
                let changed = app.editor.body.input(Input::from(key)).content_changed;
                if changed {
                    app.editor.body.delete_next_chars(1);
                    app.request_editor_preview_update();
                    app.write_draft();
                }
                app.sync_link_completion(changed);
                return false;
            }
            // VS Code-style auto-close for brackets + quotes.
            if app.config.editor.auto_close_pairs
                && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
                && let KeyCode::Char(typed) = key.code
                && crate::auto_pairs::handle_body_char(&mut app.editor.body, typed)
            {
                // Skip-over moves the cursor without editing; only real
                // inserts keep the completion alive.
                let changed = app.editor.body.revision() != revision;
                if changed {
                    app.request_editor_preview_update();
                    app.write_draft();
                }
                app.sync_link_completion(changed);
                return false;
            }
            let changed = app.editor.body.input(Input::from(key)).content_changed;
            if changed {
                app.request_editor_preview_update();
                app.write_draft();
            }
            app.sync_link_completion(changed);
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

    // Focus (zen) mode hides the tab bar and file sidebar; hit-testing
    // follows the same flag so hidden panes never intercept mouse.
    let file_tree_visible = app.editor.file_tree_visible && !app.zen_mode;
    let (title_inner, body_inner, sidebar_inner, file_tree_inner) = edit_view_input_areas(
        terminal_area,
        app.preview_fullscreen,
        app.editor.editor_preview_enabled,
        app.editor.body.lines().len(),
        app.editor_show_line_numbers(),
        app.editor.sidebar,
        app.preview_position,
        app.editor.header_title_rect,
        app.zen_padding(),
        file_tree_visible,
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
        edit_view_md_preview_area(
            terminal_area,
            app.editor.sidebar,
            app.preview_position,
            app.zen_padding(),
            file_tree_visible,
        )
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
                    app.editor.preview_scroll_overridden = true;
                }
                return;
            }
            MouseEventKind::ScrollDown => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    let h = app.editor.last_body_height.max(1) as usize;
                    renderer.scroll_down(3.min(h), h);
                    app.editor.preview_scroll_overridden = true;
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

    // Tab bar: `+` opens the new-file selector, `×` closes, click switches,
    // middle-click closes. Hidden in focus (zen) mode.
    if !app.zen_mode && contains_cell(app.editor.tab_bar_rect, mouse_event.column, mouse_event.row)
    {
        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if contains_cell(app.editor.new_tab_rect, mouse_event.column, mouse_event.row) {
                    app.new_editor_tab();
                    *focus = EditFocus::Title;
                    return;
                }
                for (i, rect) in app.editor.tab_close_rects.clone().iter().enumerate() {
                    if contains_cell(*rect, mouse_event.column, mouse_event.row) {
                        app.close_editor_tab(i);
                        return;
                    }
                }
                for (i, rect) in app.editor.tab_bar_rects.clone().iter().enumerate() {
                    if contains_cell(*rect, mouse_event.column, mouse_event.row) {
                        if i == app.editor.active_tab {
                            // The active tab carries the title: clicking it
                            // focuses the title field (merged header).
                            *focus = EditFocus::Title;
                            move_textarea_cursor_to_mouse(
                                &mut app.editor.title_editor,
                                *rect,
                                mouse_event.column,
                                mouse_event.row,
                                app.editor.title_viewport_row as usize,
                                app.editor.title_viewport_col as usize,
                            );
                        } else {
                            app.switch_editor_tab(i);
                        }
                        return;
                    }
                }
                return;
            }
            MouseEventKind::Down(MouseButton::Middle) => {
                for (i, rect) in app.editor.tab_bar_rects.clone().iter().enumerate() {
                    if contains_cell(*rect, mouse_event.column, mouse_event.row) {
                        app.close_editor_tab(i);
                        return;
                    }
                }
            }
            _ => {}
        }
    }
    // File-tree pane: scroll + click to select / double-click to open.
    if let Some(ft) = file_tree_inner
        && contains_cell(ft, mouse_event.column, mouse_event.row)
    {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                app.file_tree_move(-1);
                return;
            }
            MouseEventKind::ScrollDown => {
                app.file_tree_move(1);
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
            if let Some(ft) = file_tree_inner
                && contains_cell(ft, mouse_event.column, mouse_event.row)
            {
                *focus = EditFocus::FileTree;
                // Same row pitch as the notes list tree (Comfortable = 2).
                let pitch = crate::app::tabs::tree_row_pitch(app).max(1);
                let len = app.file_tree_len();
                if let Some(clicked) = crate::ui::list_index_at(
                    mouse_event.row,
                    app.editor.file_tree_list_rect.y,
                    pitch,
                    app.editor.file_tree_scroll,
                    len,
                ) {
                    app.editor.file_tree_selected = clicked;
                    let opened_note = matches!(
                        app.list.visual_list.get(clicked),
                        Some(crate::list_view::VisualItem::Note { .. })
                    );
                    app.file_tree_activate();
                    if opened_note {
                        *focus = EditFocus::Body;
                    }
                }
                return;
            }
            if let Some(sb) = sidebar_inner
                && contains_cell(sb, mouse_event.column, mouse_event.row)
            {
                *focus = EditFocus::Sidebar;
                app.editor.last_body_click = None;
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
            // Click a completion row to accept it; clicks elsewhere
            // (borders, hint) fall through to normal body handling.
            let mut accept_idx = None;
            if let Some(comp) = app.editor.link_complete.as_mut()
                && contains_cell(
                    app.editor.link_complete_rect,
                    mouse_event.column,
                    mouse_event.row,
                )
            {
                let idx = mouse_event
                    .row
                    .saturating_sub(app.editor.link_complete_rect.y + 1)
                    as usize;
                if idx < comp.matches.len() {
                    accept_idx = Some(idx);
                }
            }
            if let Some(idx) = accept_idx {
                *focus = EditFocus::Body;
                if let Some(comp) = app.editor.link_complete.as_mut() {
                    comp.selected = idx;
                }
                app.accept_link_completion();
                return;
            }
            if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Body;
                let is_double_click = if let Some((lx, ly, lt)) = app.editor.last_body_click {
                    lx == mouse_event.column
                        && ly == mouse_event.row
                        && lt.elapsed().as_millis() < 500
                } else {
                    false
                };
                let align = app.editor.text_align;
                let wrap_mode = app.editor.body.textarea().wrap_mode();
                if align != crate::config::TextAlignment::Left
                    && wrap_mode != ratatui_textarea::WrapMode::None
                {
                    let gutter: u16 = if app.editor_show_line_numbers() {
                        app.editor.body.lines().len().max(1).to_string().len() as u16 + 2
                    } else {
                        0
                    };
                    let text_left = body_inner.x + gutter;
                    let text_width = body_inner.width.saturating_sub(gutter);
                    let screen_row = mouse_event.row.saturating_sub(body_inner.y);
                    let rel_col = mouse_event.column.saturating_sub(text_left);
                    let tab_len = app.editor.body.textarea().tab_length();

                    if let Some((r, c)) = crate::ui::aligned_cursor_target(
                        app.editor.body.lines(),
                        &app.editor.visual_row_cache.rows,
                        app.editor.body_viewport_row,
                        screen_row,
                        rel_col,
                        text_width,
                        align,
                        tab_len,
                    ) {
                        app.editor
                            .body
                            .move_cursor(ratatui_textarea::CursorMove::Jump(r as u16, c as u16));
                    }
                } else {
                    let _ = app.editor.body.hit_test_cursor(
                        body_inner,
                        mouse_event.column,
                        mouse_event.row,
                        app.editor.body_viewport_row,
                        app.editor.body_viewport_col,
                    );
                }
                // Double-click on a [[wikilink]] follows it (open or offer
                // to create); otherwise record the click for the next pair.
                if is_double_click {
                    app.editor.last_body_click = None;
                    if app.popups.confirm.is_none() && app.follow_wikilink_under_cursor() {
                        app.request_editor_preview_update();
                        app.write_draft();
                        return;
                    }
                } else {
                    app.editor.last_body_click = Some((
                        mouse_event.column,
                        mouse_event.row,
                        std::time::Instant::now(),
                    ));
                }
                mouse_selection.begin(&mut app.editor.body);
                // Clicks only move the cursor; suggestions return on the
                // next actual edit inside the link.
                app.editor.link_complete = None;
            } else if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Title;
                app.editor.last_body_click = None;
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
                    let align = app.editor.text_align;
                    let wrap_mode = app.editor.body.textarea().wrap_mode();
                    if align != crate::config::TextAlignment::Left
                        && wrap_mode != ratatui_textarea::WrapMode::None
                    {
                        let gutter: u16 = if app.editor_show_line_numbers() {
                            app.editor.body.lines().len().max(1).to_string().len() as u16 + 2
                        } else {
                            0
                        };
                        let text_left = body_inner.x + gutter;
                        let text_width = body_inner.width.saturating_sub(gutter);
                        let screen_row = mouse_event.row.saturating_sub(body_inner.y);
                        let rel_col = mouse_event.column.saturating_sub(text_left);
                        let tab_len = app.editor.body.textarea().tab_length();

                        if let Some((r, c)) = crate::ui::aligned_cursor_target(
                            app.editor.body.lines(),
                            &app.editor.visual_row_cache.rows,
                            app.editor.body_viewport_row,
                            screen_row,
                            rel_col,
                            text_width,
                            align,
                            tab_len,
                        ) {
                            app.editor
                                .body
                                .move_cursor(ratatui_textarea::CursorMove::Jump(
                                    r as u16, c as u16,
                                ));
                        }
                    } else {
                        let _ = app.editor.body.hit_test_cursor(
                            body_inner,
                            mouse_event.column,
                            mouse_event.row,
                            app.editor.body_viewport_row,
                            app.editor.body_viewport_col,
                        );
                    }
                    app.editor.link_complete = None;
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
                mouse_selection.finish(&mut app.editor.body, app.config.editor.copy_on_select)
            } else {
                mouse_selection.finish(
                    &mut app.editor.title_editor,
                    app.config.editor.copy_on_select,
                )
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
