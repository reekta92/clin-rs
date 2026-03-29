use crate::app::ContextMenu;
use crate::app::{App, EditFocus, ListFocus};
use crate::keybinds::*;
use crossterm::event::*;
use ratatui::prelude::*;
use tui_textarea::*;

pub fn handle_list_keys(app: &mut App, key: KeyEvent) -> bool {
    // Handle template popup if open
    if let Some(mut popup) = app.template_popup.take() {
        match key.code {
            KeyCode::Up => {
                popup.selected = popup.selected.saturating_sub(1);
                app.template_popup = Some(popup);
            }
            KeyCode::Down => {
                if popup.selected + 1 < popup.templates.len() {
                    popup.selected += 1;
                }
                app.template_popup = Some(popup);
            }
            KeyCode::Enter => {
                app.template_popup = Some(popup);
                app.select_template();
            }
            KeyCode::Esc => {
                app.close_template_popup();
            }
            // Allow creating blank note with 'b'
            KeyCode::Char('b') => {
                app.start_blank_note();
            }
            _ => {
                app.template_popup = Some(popup);
            }
        }
        return false;
    }

    // Handle delete confirmation
    if app.pending_delete_note_id.is_some() {
        if app.keybinds.matches_list(ListAction::ConfirmDelete, &key) {
            app.confirm_delete_selected();
        } else if app.keybinds.matches_list(ListAction::CancelDelete, &key) {
            app.cancel_delete_prompt();
        }
        return false;
    }

    // Cycle focus with Tab
    if app.keybinds.matches_list(ListAction::CycleFocus, &key) {
        app.list_focus = match app.list_focus {
            ListFocus::Notes => ListFocus::EncryptionToggle,
            ListFocus::EncryptionToggle => ListFocus::Notes,
        };
        return false;
    }

    // Handle toggle buttons when focused
    if app.list_focus == ListFocus::EncryptionToggle {
        if app.keybinds.matches_list(ListAction::ToggleButton, &key) {
            app.toggle_encryption_mode();
        } else if app.keybinds.matches_list(ListAction::Quit, &key) {
            return true;
        }
        return false;
    }

    // Standard keybind handling
    if app.keybinds.matches_list(ListAction::Quit, &key) {
        return true;
    }
    if app.keybinds.matches_list(ListAction::Help, &key) {
        app.open_help_page();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenLocation, &key) {
        app.open_selected_note_location();
        return false;
    }
    if app.keybinds.matches_list(ListAction::Delete, &key) {
        app.begin_delete_selected();
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveDown, &key) {
        if app.selected < app.notes.len() {
            app.selected += 1;
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveUp, &key) {
        if app.selected > 0 {
            app.selected -= 1;
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::Open, &key) {
        app.open_selected();
        return false;
    }
    if app.keybinds.matches_list(ListAction::NewFromTemplate, &key) {
        app.open_template_popup();
        return false;
    }

    false
}

pub fn handle_help_keys(app: &mut App, key: KeyEvent) {
    if app.keybinds.matches_help(HelpAction::Close, &key) {
        app.close_help_page();
    } else if app.keybinds.matches_help(HelpAction::ScrollDown, &key) {
        app.help_scroll = app.help_scroll.saturating_add(1);
    } else if app.keybinds.matches_help(HelpAction::ScrollUp, &key) {
        app.help_scroll = app.help_scroll.saturating_sub(1);
    }
}

pub fn handle_edit_keys(app: &mut App, key: KeyEvent, focus: &mut EditFocus) -> bool {
    // Handle context menu if open
    if let Some(mut menu) = app.context_menu.take() {
        match key.code {
            KeyCode::Up => {
                menu.selected = menu.selected.saturating_sub(1);
                app.context_menu = Some(menu);
            }
            KeyCode::Down => {
                if menu.selected < 3 {
                    menu.selected += 1;
                }
                app.context_menu = Some(menu);
            }
            KeyCode::Enter => {
                app.handle_menu_action(menu.selected, focus);
            }
            KeyCode::Esc => {
                app.context_menu = None;
            }
            _ => {
                app.context_menu = Some(menu);
            }
        }
        return false;
    }

    // Quit with save
    if app.keybinds.matches_edit(EditAction::Quit, &key) {
        app.autosave();
        return true;
    }

    // Cycle focus
    if app.keybinds.matches_edit(EditAction::CycleFocus, &key) {
        *focus = match *focus {
            EditFocus::Title => EditFocus::Body,
            EditFocus::Body => EditFocus::EncryptionToggle,
            EditFocus::EncryptionToggle => EditFocus::Title,
        };
        return false;
    }

    // Back to list
    if app.keybinds.matches_edit(EditAction::Back, &key) {
        app.autosave();
        app.back_to_list();
        *focus = EditFocus::Body;
        return false;
    }

    match *focus {
        EditFocus::Title => {
            if key.code == KeyCode::Enter {
                *focus = EditFocus::Body;
                return false;
            }

            if handle_os_shortcuts(&app.keybinds, &mut app.title_editor, key) {
                return false;
            }

            if app.title_editor.input(Input::from(key)) && app.title_editor.lines().len() > 1 {
                let normalized = get_title_text(&app.title_editor).replace(['\r', '\n'], " ");
                app.title_editor = make_title_editor(&normalized);
            }
        }
        EditFocus::Body => {
            if handle_os_shortcuts(&app.keybinds, &mut app.editor, key) {
                return false;
            }
            app.editor.input(Input::from(key));
        }
        EditFocus::EncryptionToggle => {
            if app.keybinds.matches_edit(EditAction::ToggleButton, &key) {
                app.toggle_encryption_mode();
            }
        }
    }

    false
}

pub fn handle_list_mouse(app: &mut App, mouse_event: MouseEvent, terminal_area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(terminal_area);

    let list_area = chunks[1];
    let inner_list_area = Rect::new(
        list_area.x.saturating_add(1),
        list_area.y.saturating_add(1),
        list_area.width.saturating_sub(2),
        list_area.height.saturating_sub(2),
    );

    if mouse_event.kind == MouseEventKind::ScrollUp {
        let current = app.list_state.selected().unwrap_or(0);
        app.list_state.select(Some(current.saturating_sub(1)));
        // Wait, list_state.select automatically scrolls if needed during render.
        // We also want to change app.selected if it corresponds to a note, or we can just scroll the list_state?
        // Actually, in the original TUI, scrolling the list changes the selected NOTE.
        // Let's implement it by simulating UP/DOWN keys!
        handle_list_keys(app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        return;
    }

    if mouse_event.kind == MouseEventKind::ScrollDown {
        handle_list_keys(app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        return;
    }

    if !contains_cell(inner_list_area, mouse_event.column, mouse_event.row) {
        return;
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
        let visual_row = mouse_event.row.saturating_sub(inner_list_area.y) as usize;
        let clicked_visual_index = app.list_state.offset().saturating_add(visual_row);

        // Determine what note corresponds to clicked_visual_index
        let mut last_was_clin: Option<bool> = None;
        let mut visual_i = 0;
        let mut target_note_idx = None;
        let mut is_create_new = false;

        for (i, summary) in app.notes.iter().enumerate() {
            let is_clin = summary.id.ends_with(".clin");
            if last_was_clin != Some(is_clin) {
                if is_clin {
                    visual_i += 1;
                } else {
                    if last_was_clin.is_some() {
                        visual_i += 1;
                    }
                    visual_i += 1;
                }
                last_was_clin = Some(is_clin);
            }

            if visual_i == clicked_visual_index {
                target_note_idx = Some(i);
                break;
            }
            visual_i += 1;
        }

        if target_note_idx.is_none() && visual_i == clicked_visual_index {
            is_create_new = true;
        }

        if let Some(idx) = target_note_idx {
            if app.selected == idx {
                // Phase 2.2: Click on already-selected note to open it
                handle_list_keys(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            } else {
                app.selected = idx;
            }
        } else if is_create_new {
            if app.selected == app.notes.len() {
                handle_list_keys(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            } else {
                app.selected = app.notes.len();
            }
        }
    }
}

pub fn handle_edit_mouse(
    app: &mut App,
    mouse_event: MouseEvent,
    terminal_area: Rect,
    focus: &mut EditFocus,
    mouse_selecting: &mut bool,
    mouse_dragged: &mut bool,
) {
    if let Some(menu) = &app.context_menu {
        let menu_rect = Rect::new(menu.x, menu.y, 14, 6);
        if contains_cell(menu_rect, mouse_event.column, mouse_event.row) {
            if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                let clicked_idx = mouse_event.row.saturating_sub(menu.y).saturating_sub(1) as usize;
                if clicked_idx < 4 {
                    app.handle_menu_action(clicked_idx, focus);
                }
                app.context_menu = None;
            } else if mouse_event.kind == MouseEventKind::ScrollUp {
                let mut menu_copy = app.context_menu.take().unwrap();
                menu_copy.selected = menu_copy.selected.saturating_sub(1);
                app.context_menu = Some(menu_copy);
            } else if mouse_event.kind == MouseEventKind::ScrollDown {
                let mut menu_copy = app.context_menu.take().unwrap();
                if menu_copy.selected < 3 {
                    menu_copy.selected += 1;
                }
                app.context_menu = Some(menu_copy);
            }
            return;
        } else if matches!(mouse_event.kind, MouseEventKind::Down(_)) {
            app.context_menu = None;
            if mouse_event.kind != MouseEventKind::Down(MouseButton::Right) {
                return;
            }
        } else {
            return;
        }
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
        let (title_inner, body_inner) = edit_view_input_areas(terminal_area);

        if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Title;
            move_textarea_cursor_to_mouse(
                &mut app.title_editor,
                title_inner,
                mouse_event.column,
                mouse_event.row,
            );
        } else if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Body;
            move_textarea_cursor_to_mouse(
                &mut app.editor,
                body_inner,
                mouse_event.column,
                mouse_event.row,
            );
        }

        let max_x = terminal_area.width.saturating_sub(14);
        let max_y = terminal_area.height.saturating_sub(6);
        app.context_menu = Some(ContextMenu {
            x: mouse_event.column.min(max_x),
            y: mouse_event.row.min(max_y),
            selected: 0,
        });
        return;
    }

    let (title_inner, body_inner) = edit_view_input_areas(terminal_area);

    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *mouse_selecting = false;
            *mouse_dragged = false;
            if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Body;
                move_textarea_cursor_to_mouse(
                    &mut app.editor,
                    body_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
                app.editor.start_selection();
                *mouse_selecting = true;
            } else if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Title;
                move_textarea_cursor_to_mouse(
                    &mut app.title_editor,
                    title_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
                app.title_editor.start_selection();
                *mouse_selecting = true;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if *mouse_selecting {
                *mouse_dragged = true;
                if *focus == EditFocus::Body {
                    move_textarea_cursor_to_mouse(
                        &mut app.editor,
                        body_inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                } else {
                    move_textarea_cursor_to_mouse(
                        &mut app.title_editor,
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
                    app.editor.cancel_selection();
                } else {
                    app.title_editor.cancel_selection();
                }
            }
            *mouse_selecting = false;
            *mouse_dragged = false;
        }
        MouseEventKind::ScrollDown => {
            if *focus == EditFocus::Body {
                app.editor.scroll((3, 0));
            }
        }
        MouseEventKind::ScrollUp => {
            if *focus == EditFocus::Body {
                app.editor.scroll((-3, 0));
            }
        }
        _ => {}
    }
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

    let mut scroll_row = 0;
    let mut scroll_col = 0;
    let debug_str = format!("{textarea:?}");
    if let Some(start) = debug_str.find("viewport: Viewport(") {
        let after_start = &debug_str[start + "viewport: Viewport(".len()..];
        if let Some(end) = after_start.find(')') {
            let number_str = &after_start[..end];
            if let Ok(number) = number_str.parse::<u64>() {
                scroll_row = ((number >> 16) & 0xFFFF) as usize;
                scroll_col = (number & 0xFFFF) as usize;
            }
        }
    }

    let row = mouse_row.saturating_sub(body_inner.y) as usize + scroll_row;
    let col = mouse_col.saturating_sub(body_inner.x) as usize + scroll_col;

    let max_row = textarea.lines().len().saturating_sub(1);
    let target_row = row.min(max_row);
    let max_col = textarea.lines()[target_row].chars().count();
    let target_col = col.min(max_col);

    textarea.move_cursor(CursorMove::Jump(target_row as u16, target_col as u16));
}

pub fn edit_view_input_areas(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let title_inner = chunks[0].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let body_inner = chunks[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    (title_inner, body_inner)
}

pub fn contains_cell(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

pub fn handle_os_shortcuts(
    keybinds: &Keybinds,
    textarea: &mut TextArea<'static>,
    key: KeyEvent,
) -> bool {
    // Use keybinds for customizable shortcuts
    if keybinds.matches_edit(EditAction::SelectAll, &key) {
        textarea.select_all();
        return true;
    }
    if keybinds.matches_edit(EditAction::Copy, &key) {
        textarea.copy();
        return true;
    }
    if keybinds.matches_edit(EditAction::Cut, &key) {
        let _ = textarea.cut();
        return true;
    }
    if keybinds.matches_edit(EditAction::Paste, &key) {
        let _ = textarea.paste();
        return true;
    }
    if keybinds.matches_edit(EditAction::Undo, &key) {
        let _ = textarea.undo();
        return true;
    }
    if keybinds.matches_edit(EditAction::Redo, &key) {
        let _ = textarea.redo();
        return true;
    }
    if keybinds.matches_edit(EditAction::DeleteWord, &key) {
        let _ = textarea.delete_word();
        return true;
    }
    if keybinds.matches_edit(EditAction::DeleteNextWord, &key) {
        let _ = textarea.delete_next_word();
        return true;
    }
    if keybinds.matches_edit(EditAction::MoveToTop, &key) {
        textarea.move_cursor(CursorMove::Top);
        return true;
    }
    if keybinds.matches_edit(EditAction::MoveToBottom, &key) {
        textarea.move_cursor(CursorMove::Bottom);
        return true;
    }

    false
}

pub fn make_title_editor(initial: &str) -> TextArea<'static> {
    let mut title = if initial.is_empty() {
        TextArea::default()
    } else {
        TextArea::from([initial.to_string()])
    };
    title.set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
    title
}

pub fn get_title_text(title_editor: &TextArea<'static>) -> String {
    title_editor
        .lines()
        .join(" ")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}
