use crate::app::ContextMenu;
use crate::app::{App, EditFocus, HelpTab, ListFocus};
use crate::keybinds::*;
use crossterm::event::*;
use ratatui::prelude::*;
use ratatui_textarea::*;

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

    if let Some(mut popup) = app.note_create_popup.take() {
        if key.code == KeyCode::Esc {
            app.note_create_popup = None;
        } else if key.code == KeyCode::Enter {
            app.note_create_popup = Some(popup);
            app.confirm_create_note();
        } else {
            popup.input.input(Input::from(key));
            app.note_create_popup = Some(popup);
        }
        return false;
    }

    if let Some(mut popup) = app.canvas_create_popup.take() {
        if key.code == KeyCode::Esc {
            app.canvas_create_popup = None;
        } else if key.code == KeyCode::Enter {
            app.canvas_create_popup = Some(popup);
            app.confirm_create_canvas();
        } else {
            popup.input.input(Input::from(key));
            app.canvas_create_popup = Some(popup);
        }
        return false;
    }

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

    if let Some(mut popup) = app.tag_popup.take() {
        if key.code == KeyCode::Esc {
            app.tag_popup = None;
        } else if key.code == KeyCode::Enter {
            app.tag_popup = Some(popup);
            app.confirm_manage_tags();
        } else if key.code == KeyCode::Char('D') && key.modifiers.contains(KeyModifiers::SHIFT) {
            app.tag_popup = Some(popup);
            app.begin_delete_tag();
        } else if key.code == KeyCode::Tab {
            app.tag_popup = Some(popup);
            if app
                .tag_popup
                .as_ref()
                .map_or(false, |p| !p.suggestions.is_empty())
            {
                app.accept_tag_suggestion();
            } else {
                app.cycle_tag_suggestion();
            }
        } else {
            popup.input.input(Input::from(key));
            app.tag_popup = Some(popup);
            app.update_tag_suggestions();
        }
        return false;
    }

    if let Some(mut popup) = app.filter_popup.take() {
        if key.code == KeyCode::Esc {
            app.cancel_filter_tags();
        } else if key.code == KeyCode::Enter {
            app.filter_popup = Some(popup);
            app.confirm_filter_tags();
        } else if key.code == KeyCode::Tab {
            app.filter_popup = Some(popup);
            if app
                .filter_popup
                .as_ref()
                .map_or(false, |p| !p.suggestions.is_empty())
            {
                app.accept_filter_suggestion();
            } else {
                app.cycle_filter_suggestion();
            }
        } else {
            popup.input.input(Input::from(key));
            app.filter_popup = Some(popup);
            app.update_filter_suggestions();
        }
        return false;
    }

    if let Some(mut popup) = app.note_rename_popup.take() {
        if key.code == KeyCode::Esc {
            app.note_rename_popup = None;
        } else if key.code == KeyCode::Enter {
            app.note_rename_popup = Some(popup);
            app.confirm_rename_note();
        } else {
            popup.input.input(Input::from(key));
            app.note_rename_popup = Some(popup);
        }
        return false;
    }

    if let Some(mut popup) = app.search_popup.take() {
        if key.code == KeyCode::Esc {
            app.search_popup = Some(popup);
            app.cancel_search();
        } else if key.code == KeyCode::Enter {
            app.search_popup = Some(popup);
            app.confirm_search();
        } else {
            popup.input.input(Input::from(key));
            app.search_popup = Some(popup);
            app.update_search();
        }
        return false;
    }

    if app.confirm_popup.is_some() {
        if key.code == KeyCode::Left || key.code == KeyCode::Char('h') {
            app.confirm_popup_select_confirm();
        } else if key.code == KeyCode::Right || key.code == KeyCode::Char('l') {
            app.confirm_popup_select_cancel();
        } else if key.code == KeyCode::Tab {
            app.confirm_popup_toggle_button();
        } else if key.code == KeyCode::Enter {
            app.confirm_popup_activate();
        } else if key.code == KeyCode::Esc {
            app.cancel_confirm();
        } else if app.keybinds.matches_list(ListAction::Confirm, &key) {
            app.confirm_action();
        } else if app.keybinds.matches_list(ListAction::Cancel, &key) {
            app.cancel_confirm();
        }
        return false;
    }

    if let Some(ref mut trash) = app.trash_view {
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
        return false;
    }

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
                app.confirm_move();
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

    if let Some(mut popup) = app.theme_popup.take() {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                match popup.focus {
                    crate::app::ThemePopupFocus::ThemeList => {
                        popup.selected = popup.selected.saturating_sub(1);
                        app.theme_popup = Some(popup);
                        app.select_theme();
                        return false;
                    }
                    crate::app::ThemePopupFocus::GeneralBg => {
                        popup.focus = crate::app::ThemePopupFocus::ThemeList;
                        popup.selected = popup.themes.len().saturating_sub(1);
                    }
                    crate::app::ThemePopupFocus::GraphBg => {
                        popup.focus = crate::app::ThemePopupFocus::GeneralBg;
                    }
                }
                app.theme_popup = Some(popup);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match popup.focus {
                    crate::app::ThemePopupFocus::ThemeList => {
                        if popup.selected + 1 < popup.themes.len() {
                            popup.selected += 1;
                            app.theme_popup = Some(popup);
                            app.select_theme();
                            return false;
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
                app.theme_popup = Some(popup);
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
                app.theme_popup = Some(popup);
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Char(' ') => {
                app.theme_popup = Some(popup);
                app.select_theme();
            }
            KeyCode::Esc | KeyCode::Char('h') => {
                app.close_theme_popup();
            }
            _ => {
                app.theme_popup = Some(popup);
            }
        }
        return false;
    }

    if app.keybinds.matches_list(ListAction::CycleFocus, &key) {
        app.list_focus = match app.list_focus {
            ListFocus::Notes => ListFocus::ExternalEditorToggle,
            ListFocus::ExternalEditorToggle => ListFocus::Notes,
        };
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
            app.update_preview();
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveUp, &key) {
        if app.visual_index > 0 {
            app.visual_index -= 1;
            app.update_preview();
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
    if app.keybinds.matches_list(ListAction::CreateNote, &key) {
        app.begin_create_note();
        return false;
    }
    if app.keybinds.matches_list(ListAction::RenameFolder, &key)
        || app.keybinds.matches_list(ListAction::Rename, &key)
    {
        if let Some(item) = app.visual_list.get(app.visual_index) {
            match item {
                crate::app::VisualItem::Folder { .. } => app.begin_rename_folder(),
                crate::app::VisualItem::Note { .. } => app.begin_rename_note(),
                _ => app.set_temporary_status_static("Select a note or folder to rename"),
            }
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveNote, &key) {
        app.begin_move();
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
                    app.command_palette = Some(crate::palette::CommandPalette::new(
                        Some(id.clone()),
                        &app.app_theme,
                    ));
                }
                _ => {
                    app.command_palette =
                        Some(crate::palette::CommandPalette::new(None, &app.app_theme));
                }
            }
        } else {
            app.command_palette = Some(crate::palette::CommandPalette::new(None, &app.app_theme));
        }
        return false;
    }

    if app.keybinds.matches_list(ListAction::Duplicate, &key) {
        app.duplicate_note();
        return false;
    }
    if app.keybinds.matches_list(ListAction::TogglePin, &key) {
        app.toggle_pin();
        return false;
    }
    if app.keybinds.matches_list(ListAction::CycleSort, &key) {
        app.cycle_sort();
        return false;
    }
    if app.keybinds.matches_list(ListAction::Search, &key) {
        app.begin_search();
        return false;
    }
    if app.keybinds.matches_list(ListAction::JumpToTop, &key) {
        app.jump_to_bottom();
        return false;
    }
    if app.keybinds.matches_list(ListAction::PageUp, &key) {
        app.page_up();
        return false;
    }
    if app.keybinds.matches_list(ListAction::PageDown, &key) {
        app.page_down();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenTrash, &key) {
        app.open_trash_view();
        return false;
    }
    if app.keybinds.matches_list(ListAction::TogglePreview, &key) {
        app.toggle_preview();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenGraph, &key) {
        app.open_graph_view();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenCanvas, &key) {
        app.open_canvas_view();
        return false;
    }

    if key.code == KeyCode::Char('g') {
        if app.handle_g_press() {
            return false;
        }
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
    } else {
        match key.code {
            KeyCode::Right | KeyCode::Char('l') => {
                app.switch_help_tab(app.help_tab.next());
            }
            KeyCode::Left | KeyCode::Char('h') => {
                app.switch_help_tab(app.help_tab.prev());
            }
            KeyCode::Tab if !key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                app.switch_help_tab(app.help_tab.next());
            }
            KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                app.switch_help_tab(app.help_tab.prev());
            }
            KeyCode::Char('1') => app.switch_help_tab(HelpTab::Notes),
            KeyCode::Char('2') => app.switch_help_tab(HelpTab::Editor),
            KeyCode::Char('3') => app.switch_help_tab(HelpTab::Graph),
            KeyCode::Char('4') => app.switch_help_tab(HelpTab::Canvas),
            KeyCode::Char('5') => app.switch_help_tab(HelpTab::About),
            _ => {}
        }
    }
}

pub fn handle_edit_keys(app: &mut App, key: KeyEvent, focus: &mut EditFocus) -> bool {
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

    if app.keybinds.matches_edit(EditAction::Quit, &key) {
        app.autosave();
        return true;
    }

    if app.keybinds.matches_edit(EditAction::CycleFocus, &key) {
        *focus = match *focus {
            EditFocus::Title => EditFocus::Body,
            EditFocus::Body => EditFocus::Title,
            _ => EditFocus::Title,
        };
        return false;
    }

    if app.keybinds.matches_edit(EditAction::Back, &key) {
        app.autosave();
        app.back_to_list();
        *focus = EditFocus::Body;
        return false;
    }

    if app
        .keybinds
        .matches_edit(EditAction::ToggleMarkdownPreview, &key)
    {
        app.toggle_markdown_preview();
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
                app.title_editor = make_title_editor(
                    &normalized,
                    app.app_theme.highlight_fg,
                    app.app_theme.highlight_bg,
                );
            }
        }
        EditFocus::Body => {
            if handle_os_shortcuts(&app.keybinds, &mut app.editor, key) {
                return false;
            }
            app.editor.input(Input::from(key));
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
    if app.confirm_popup.is_some() {
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            let popup_area = crate::ui::centered_rect(50, 30, terminal_area);
            let click_x = mouse_event.column;
            let click_y = mouse_event.row;

            if click_x >= popup_area.x
                && click_x < popup_area.x + popup_area.width
                && click_y >= popup_area.y
                && click_y < popup_area.y + popup_area.height
            {
                let mid_x = popup_area.x + popup_area.width / 2;
                if click_x < mid_x {
                    app.confirm_action();
                } else {
                    app.cancel_confirm();
                }
            }
        }
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(terminal_area);

    let list_area = chunks[0];
    // List block uses Padding::new(2, 2, 1, 1) with Borders::NONE
    let inner_list_area = Rect::new(
        list_area.x.saturating_add(2),
        list_area.y.saturating_add(1),
        list_area.width.saturating_sub(4),
        list_area.height.saturating_sub(2),
    );

    if app.preview_enabled {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(terminal_area);
        let preview_area = Rect::new(main_chunks[2].x, main_chunks[2].y, main_chunks[2].width, chunks[0].height);

        if contains_cell(preview_area, mouse_event.column, mouse_event.row) {
            if mouse_event.kind == MouseEventKind::ScrollUp {
                if let Some(renderer) = &mut app.preview_renderer {
                    renderer.scroll_up(3);
                }
                return;
            }
            if mouse_event.kind == MouseEventKind::ScrollDown {
                if let Some(renderer) = &mut app.preview_renderer {
                    renderer.scroll_down(3, preview_area.height.saturating_sub(2));
                }
                return;
            }
        }
    }

    if mouse_event.kind == MouseEventKind::ScrollUp {
        let current = app.list_state.selected().unwrap_or(0);
        app.list_state.select(Some(current.saturating_sub(1)));

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
        let (title_inner, body_inner) =
            edit_view_input_areas(terminal_area, app.editor_preview_enabled);

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

    let (title_inner, body_inner) =
        edit_view_input_areas(terminal_area, app.editor_preview_enabled);

    if app.editor_preview_enabled {
        if let Some(md_area) = edit_view_md_preview_area(terminal_area) {
            if contains_cell(md_area, mouse_event.column, mouse_event.row) {
                match mouse_event.kind {
                    MouseEventKind::ScrollUp => {
                        if let Some(renderer) = &mut app.md_preview_renderer {
                            renderer.scroll_up(3);
                        }
                        return;
                    }
                    MouseEventKind::ScrollDown => {
                        if let Some(renderer) = &mut app.md_preview_renderer {
                            renderer.scroll_down(3, md_area.height.saturating_sub(2));
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }
    }

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

pub fn edit_view_input_areas(area: Rect, md_preview: bool) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    // Title block uses Padding::new(2, 1, 1, 1) with Borders::NONE
    let title_inner = Rect::new(
        chunks[0].x + 2,
        chunks[0].y + 1,
        chunks[0].width.saturating_sub(4),
        chunks[0].height.saturating_sub(2),
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
        // Content is in the left column, clipped to chunks[1] height
        Rect::new(content_chunks[0].x, chunks[1].y, content_chunks[0].width, chunks[1].height)
    } else {
        chunks[1]
    };

    // Body block uses Padding::new(2, 2, 1, 0) with Borders::NONE
    let body_inner = Rect::new(
        body_area.x + 2,
        body_area.y + 1,
        body_area.width.saturating_sub(4),
        body_area.height.saturating_sub(1),
    );

    (title_inner, body_inner)
}

pub fn edit_view_md_preview_area(area: Rect) -> Option<Rect> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
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

    // Preview is in the right column, clipped to chunks[1] height, padded
    let preview_area = Rect::new(content_chunks[2].x, chunks[1].y, content_chunks[2].width, chunks[1].height);
    // Preview block uses Padding::new(2, 2, 1, 1) with Borders::NONE
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

pub fn handle_os_shortcuts(
    keybinds: &Keybinds,
    textarea: &mut TextArea<'static>,
    key: KeyEvent,
) -> bool {
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

use std::borrow::Cow;

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
