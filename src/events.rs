use crate::app::ContextMenu;
use crate::app::{App, EditFocus, ListFocus};
use crate::keybinds::*;
use crossterm::event::*;
use ratatui::prelude::*;
use tui_textarea::*;

pub fn handle_list_keys(app: &mut App, key: KeyEvent) -> bool {
    if let Some(mut palette) = app.command_palette.take() {
        if palette.handle_input(key) {
            if key.code == KeyCode::Enter {
                if let Some(selected_idx) = palette.state.selected() {
                    if let Some(item) = palette.items.get(selected_idx) {
                        let action_id = item.id.clone();
                        let note_id = palette.context_note_id.clone();
                        if let Err(e) =
                            crate::actions::execute_action(&action_id, app, note_id.as_deref())
                        {
                            app.set_temporary_status(&format!("Action failed: {}", e));
                        }
                    }
                }
            }
            return false;
        }
        app.command_palette = Some(palette);
        return false;
    }

    // Handle folder popup if open
    if let Some(mut popup) = app.folder_popup.take() {
        if key.code == KeyCode::Esc {
            app.folder_popup = None;
        } else if key.code == KeyCode::Enter {
            app.folder_popup = Some(popup);
            app.confirm_folder_popup();
        } else {
            popup.input.input(Input::from(key));
            app.folder_popup = Some(popup);
        }
        return false;
    }

    // Handle tag popup if open
    if let Some(mut popup) = app.tag_popup.take() {
        if key.code == KeyCode::Esc {
            app.tag_popup = None;
        } else if key.code == KeyCode::Enter {
            app.tag_popup = Some(popup);
            app.confirm_manage_tags();
        } else {
            popup.input.input(Input::from(key));
            app.tag_popup = Some(popup);
        }
        return false;
    }

    // Handle filter popup if open
    if let Some(mut popup) = app.filter_popup.take() {
        if key.code == KeyCode::Esc {
            app.cancel_filter_tags();
        } else if key.code == KeyCode::Enter {
            app.filter_popup = Some(popup);
            app.confirm_filter_tags();
        } else {
            popup.input(Input::from(key));
            app.filter_popup = Some(popup);
        }
        return false;
    }

    // Handle folder picker if open
    if let Some(mut picker) = app.folder_picker.take() {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                picker.selected = picker.selected.saturating_sub(1);
                app.folder_picker = Some(picker);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if picker.selected + 1 < picker.folders.len() {
                    picker.selected += 1;
                }
                app.folder_picker = Some(picker);
            }
            KeyCode::Enter | KeyCode::Char('l') => {
                app.folder_picker = Some(picker);
                app.confirm_move_note();
            }
            KeyCode::Esc | KeyCode::Char('h') => {
                app.folder_picker = None;
            }
            _ => {
                app.folder_picker = Some(picker);
            }
        }
        return false;
    }

    // Handle template popup if open
    if let Some(mut popup) = app.template_popup.take() {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                popup.selected = popup.selected.saturating_sub(1);
                app.template_popup = Some(popup);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if popup.selected + 1 < popup.templates.len() {
                    popup.selected += 1;
                }
                app.template_popup = Some(popup);
            }
            KeyCode::Enter | KeyCode::Char('l') => {
                app.template_popup = Some(popup);
                app.select_template();
            }
            KeyCode::Esc | KeyCode::Char('h') => {
                app.close_template_popup();
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

    // Handle encrypt warning confirmation
    if app.pending_encrypt_note_id.is_some() {
        if app.keybinds.matches_list(ListAction::ConfirmEncrypt, &key) {
            app.confirm_encrypt_warning();
        } else if app.keybinds.matches_list(ListAction::CancelEncrypt, &key) {
            app.cancel_encrypt_warning();
        }
        return false;
    }

    // Cycle focus with Tab
    if app.keybinds.matches_list(ListAction::CycleFocus, &key) {
        app.list_focus = match app.list_focus {
            ListFocus::Notes => ListFocus::EncryptionToggle,
            ListFocus::EncryptionToggle => ListFocus::ExternalEditorToggle,
            ListFocus::ExternalEditorToggle => ListFocus::Notes,
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
    if app.list_focus == ListFocus::ExternalEditorToggle {
        if app.keybinds.matches_list(ListAction::ToggleButton, &key) {
            app.toggle_external_editor_mode();
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
        if app.visual_index < app.visual_list.len().saturating_sub(1) {
            app.visual_index += 1;
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveUp, &key) {
        if app.visual_index > 0 {
            app.visual_index -= 1;
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::CollapseFolder, &key) {
        app.collapse_selected_folder();
        return false;
    }
    if app.keybinds.matches_list(ListAction::ExpandFolder, &key) {
        app.expand_selected_folder();
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
    if app.keybinds.matches_list(ListAction::CreateFolder, &key) {
        app.begin_create_folder();
        return false;
    }
    if app.keybinds.matches_list(ListAction::RenameFolder, &key) {
        app.begin_rename_folder();
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveNote, &key) {
        app.begin_move_note();
        return false;
    }
    if app.keybinds.matches_list(ListAction::ManageTags, &key) {
        app.begin_manage_tags();
        return false;
    }
    if app.keybinds.matches_list(ListAction::FilterTags, &key) {
        app.begin_filter_tags();
        return false;
    }
    if app
        .keybinds
        .matches_list(ListAction::OpenCommandPalette, &key)
    {
        if let Some(item) = app.visual_list.get(app.visual_index) {
            match item {
                crate::app::VisualItem::Note { id, .. } => {
                    app.command_palette =
                        Some(crate::palette::CommandPalette::new(Some(id.clone())));
                }
                _ => {
                    app.command_palette = Some(crate::palette::CommandPalette::new(None));
                }
            }
        } else {
            app.command_palette = Some(crate::palette::CommandPalette::new(None));
        }
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
            EditFocus::EncryptionToggle => EditFocus::ExternalEditorToggle,
            EditFocus::ExternalEditorToggle => EditFocus::Title,
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
        EditFocus::ExternalEditorToggle => {
            if app.keybinds.matches_edit(EditAction::ToggleButton, &key) {
                app.toggle_external_editor_mode();
            }
        }
    }

    false
}

pub fn handle_list_mouse(app: &mut App, mouse_event: MouseEvent, terminal_area: Rect) {
    if app.pending_delete_note_id.is_some() || app.pending_encrypt_note_id.is_some() {
        return;
    }

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

        if clicked_visual_index < app.visual_list.len() {
            if app.visual_index == clicked_visual_index {
                // Click on already-selected item to open it
                app.open_selected();
            } else {
                app.visual_index = clicked_visual_index;
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
