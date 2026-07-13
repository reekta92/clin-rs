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
    app.editor
        .editor
        .set_search_style(ratatui::style::Style::default());
    let prev_id = app.editor.editing_id.clone();
    app.autosave();
    let new_id = app.editor.editing_id.clone();
    app.back_to_list(prev_id.as_deref(), new_id.as_deref());
    *focus = EditFocus::Body;
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
                if menu.selected < 3 {
                    menu.selected += 1;
                }
                app.popups.active = Some(crate::popups::ActivePopup::ContextMenu(menu));
            }
            _ if app
                .keybinds
                .matches_list(crate::keybinds::ListAction::Confirm, &key) =>
            {
                app.handle_menu_action(menu.selected, focus);
            }
            _ => {
                app.popups.active = Some(crate::popups::ActivePopup::ContextMenu(menu));
            }
        }
        return false;
    }
    if let Some(ref mut popup) = app.editor.find_popup {
        app.seq_matcher.clear();
        match crate::ui::quick_search::handle_quick_search_keys(popup, key, &app.keybinds) {
            crate::ui::quick_search::QuickSearchAction::Submit => {
                if let Some(&(line_idx, _)) = popup.results.get(popup.selected) {
                    app.editor
                        .editor
                        .move_cursor(ratatui_textarea::CursorMove::Jump(line_idx as u16, 0));
                    let _ = app.editor.editor.search_forward(false);
                }
                app.editor
                    .editor
                    .set_search_style(ratatui::style::Style::default());
                app.editor.find_popup = None;
            }
            crate::ui::quick_search::QuickSearchAction::Cancel => {
                app.editor
                    .editor
                    .set_search_style(ratatui::style::Style::default());
                app.editor.find_popup = None;
            }
            crate::ui::quick_search::QuickSearchAction::Edited => {
                let query_lower = popup.query().to_lowercase();
                if query_lower.is_empty() {
                    popup.results.clear();
                } else {
                    popup.results = app
                        .editor
                        .editor
                        .lines()
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| line.to_lowercase().contains(&query_lower))
                        .map(|(i, line)| (i, line.to_string()))
                        .collect();
                }
                if popup.selected >= popup.results.len() {
                    popup.selected = popup.results.len().saturating_sub(1);
                }
                let query = popup.query();
                let result = app.editor.editor.set_search_pattern(&query);
                if result.is_ok() && !query.is_empty() {
                    let _ = app.editor.editor.search_forward(true);
                }
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
                    app.editor
                        .editor
                        .set_search_style(ratatui::style::Style::default());
                    app.editor.find_popup = None;
                } else {
                    app.editor
                        .editor
                        .set_search_style(ratatui::style::Style::default().bg(theme.highlight_bg));
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
    if let Some(crate::popups::ActivePopup::ContextMenu(menu)) = &app.popups.active {
        let menu_rect = Rect::new(menu.x, menu.y, 14, 4);
        if contains_cell(menu_rect, mouse_event.column, mouse_event.row) {
            if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                let clicked_idx = mouse_event.row.saturating_sub(menu.y) as usize;
                if clicked_idx < 4 {
                    app.handle_menu_action(clicked_idx, focus);
                }
                app.popups.active = None;
            } else if mouse_event.kind == MouseEventKind::ScrollUp {
                let mut menu_taken = app
                    .popups
                    .active
                    .take()
                    .expect("context_menu Some — guarded by enclosing if-let");
                if let crate::popups::ActivePopup::ContextMenu(menu) = &mut menu_taken {
                    menu.selected = menu.selected.saturating_sub(1);
                }
                app.popups.active = Some(menu_taken);
            } else if mouse_event.kind == MouseEventKind::ScrollDown {
                let mut menu_taken = app
                    .popups
                    .active
                    .take()
                    .expect("context_menu Some — guarded by enclosing if-let");
                if let crate::popups::ActivePopup::ContextMenu(menu) = &mut menu_taken
                    && menu.selected < 3
                {
                    menu.selected += 1;
                }
                app.popups.active = Some(menu_taken);
            }
            return;
        } else if matches!(mouse_event.kind, MouseEventKind::Down(_)) {
            app.popups.active = None;
            if mouse_event.kind != MouseEventKind::Down(MouseButton::Right) {
                return;
            }
        } else {
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
        );

        if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Title;
            move_textarea_cursor_to_mouse(
                &mut app.editor.title_editor,
                title_inner,
                mouse_event.column,
                mouse_event.row,
            );
        } else if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Body;
            move_textarea_cursor_to_mouse(
                &mut app.editor.editor,
                body_inner,
                mouse_event.column,
                mouse_event.row,
            );
        }

        let max_x = terminal_area.width.saturating_sub(14);
        let max_y = terminal_area.height.saturating_sub(4);
        app.popups.active = Some(crate::popups::ActivePopup::ContextMenu(ContextMenu {
            x: mouse_event.column.min(max_x),
            y: mouse_event.row.min(max_y),
            selected: 0,
        }));
        return;
    }

    let (title_inner, body_inner, sidebar_inner) = edit_view_input_areas(
        terminal_area,
        app.editor.editor_preview_enabled,
        app.editor.editor.lines().len(),
        app.editor.show_line_numbers,
        app.editor.sidebar,
        app.preview_position,
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
        edit_view_md_preview_area(terminal_area)
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
                    let clicked = clicked_row as usize;
                    let len = app.sidebar_len();
                    if clicked < len {
                        app.editor.sidebar_selected = clicked;
                    }
                }
                return;
            }
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
