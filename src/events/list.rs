use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

use crate::app::App;
use crate::keybinds::ListAction;
use crate::list_view::ListMode;

use super::{contains_cell, move_textarea_cursor_to_mouse};

pub fn handle_list_keys(app: &mut App, key: KeyEvent) -> bool {
    if app.layout_edit {
        match key.code {
            KeyCode::Esc => {
                app.toggle_layout_edit();
                return false;
            }
            KeyCode::Left => {
                let delta = if app.preview_position == crate::config::PreviewPosition::Right {
                    0.02
                } else {
                    -0.02
                };
                app.adjust_preview_width(delta);
            }
            KeyCode::Right => {
                let delta = if app.preview_position == crate::config::PreviewPosition::Right {
                    -0.02
                } else {
                    0.02
                };
                app.adjust_preview_width(delta);
            }
            KeyCode::Up => app.adjust_calendar_height(1),
            KeyCode::Down => app.adjust_calendar_height(-1),
            KeyCode::Char('s') | KeyCode::Char('S') => app.swap_preview_position(),
            KeyCode::Char('h') | KeyCode::Char('H') => {
                let delta = if app.preview_position == crate::config::PreviewPosition::Right {
                    0.02
                } else {
                    -0.02
                };
                app.adjust_preview_width(delta);
            }
            KeyCode::Char('l') | KeyCode::Char('L') => {
                let delta = if app.preview_position == crate::config::PreviewPosition::Right {
                    -0.02
                } else {
                    0.02
                };
                app.adjust_preview_width(delta);
            }
            KeyCode::Char('k') | KeyCode::Char('K') => app.adjust_calendar_height(1),
            KeyCode::Char('j') | KeyCode::Char('J') => app.adjust_calendar_height(-1),
            KeyCode::Char('c') | KeyCode::Char('C') => app.swap_calendar_position(),
            KeyCode::Tab => app.swap_section_order(),
            KeyCode::Char(' ') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.cycle_section(1);
                } else {
                    app.cycle_section(0);
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => app.toggle_section(),
            _ => {}
        }
        return false;
    }

    if app.list.list_mode == ListMode::Select {
        if crate::events::is_cancel_popup(&app.keybinds, &key, false) {
            app.list.tag_to_assign = None;
            app.list.list_mode = ListMode::Normal;
            app.list.selected_indices.clear();
            return false;
        }
        if key.code == KeyCode::Enter {
            if let Some(tag) = app.list.tag_to_assign.take() {
                app.apply_tag_to_selected(tag);
            }
            return false;
        }
    }

    let seq = app.config.sequences_enabled();
    let counts = app.config.counts_enabled();
    match app
        .keybinds
        .resolve_list(&mut app.seq_matcher, key, seq, counts)
    {
        crate::keybinds::MatchOutcome::Matched(action, count) => match action {
            ListAction::CycleFocus => {
                if app.list.notes_layout == crate::config::NotesLayout::Grid {
                    app.cycle_grid_tab();
                }
                return false;
            }
            ListAction::Quit => {
                if app.list.list_mode != ListMode::Select {
                    app.initiate_quit();
                }
                return false;
            }
            ListAction::ToggleExternalEditor => {
                app.toggle_external_editor_mode();
                return false;
            }
            ListAction::ToggleSelectMode => {
                if app.list.tag_to_assign.is_some() {
                    return false;
                }
                app.list.list_mode = match app.list.list_mode {
                    ListMode::Normal => {
                        app.list.selected_indices.clear();
                        app.list.selected_indices.insert(app.list.visual_index);
                        ListMode::Select
                    }
                    ListMode::Select => {
                        app.list.selected_indices.clear();
                        ListMode::Normal
                    }
                };
                return false;
            }
            ListAction::ToggleSelectItem => {
                if app.list.list_mode == ListMode::Select {
                    if app.list.selected_indices.contains(&app.list.visual_index) {
                        app.list.selected_indices.remove(&app.list.visual_index);
                    } else {
                        app.list.selected_indices.insert(app.list.visual_index);
                    }
                }
                return false;
            }
            ListAction::Help => {
                app.open_help_page();
                return false;
            }
            ListAction::OpenLocation => {
                app.open_selected_note_location();
                return false;
            }
            ListAction::Delete => {
                app.begin_delete_selected();
                return false;
            }
            ListAction::MoveLeft => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    app.move_left();
                }
                return false;
            }
            ListAction::MoveRight => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    app.move_right();
                }
                return false;
            }
            ListAction::MoveDown => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    app.move_down();
                }
                return false;
            }
            ListAction::MoveUp => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    app.move_up();
                }
                return false;
            }
            ListAction::CollapseFolder => {
                app.collapse_selected_folder();
                return false;
            }
            ListAction::ExpandFolder => {
                app.expand_selected_folder();
                return false;
            }
            ListAction::Open => {
                app.open_selected();
                return false;
            }
            ListAction::NewFromTemplate => {
                app.open_template_popup();
                return false;
            }
            ListAction::CreateFolder => {
                app.begin_create_folder();
                return false;
            }
            ListAction::CreateNote => {
                app.begin_create_select_format();
                return false;
            }
            ListAction::RenameFolder | ListAction::Rename => {
                if let Some(item) = app.list.visual_list.get(app.list.visual_index) {
                    match item {
                        crate::app::VisualItem::Folder { .. } => app.begin_rename_folder(),
                        crate::app::VisualItem::Note { .. } => app.begin_rename_note(),
                        _ => app.set_temporary_status_static("Select a note or folder to rename"),
                    }
                }
                return false;
            }
            ListAction::MoveNote => {
                app.begin_move();
                return false;
            }
            ListAction::MoveToParent => {
                if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
                    app.list.visual_list.get(app.list.visual_index)
                {
                    let note = &app.notes[*summary_idx];
                    let current_folder = &note.folder;
                    if !current_folder.is_empty() {
                        let parent_folder = if let Some(slash) = current_folder.rfind('/') {
                            &current_folder[..slash]
                        } else {
                            ""
                        };
                        let id = note.id.clone();
                        match app.storage.move_note(&id, parent_folder) {
                            Ok(_) => {
                                let _ = app.refresh_notes();
                                app.set_temporary_status_static("Note moved to parent folder");
                            }
                            Err(e) => {
                                app.set_temporary_status(&format!("Failed to move: {e}"));
                            }
                        }
                    } else {
                        app.set_temporary_status_static("Note is already at Vault root");
                    }
                }
                return false;
            }
            ListAction::ManageTags => {
                app.begin_manage_tags();
                return false;
            }
            ListAction::ManageSubnotes => {
                app.open_subnotes_popup();
                return false;
            }
            ListAction::OpenCommandPalette => {
                if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
                    app.list.visual_list.get(app.list.visual_index)
                {
                    let id = app.notes[*summary_idx].id.clone();
                    app.command_palette = Some(crate::palette::CommandPalette::new(Some(id), app));
                } else {
                    app.command_palette = Some(crate::palette::CommandPalette::new(None, app));
                }
                return false;
            }
            ListAction::Duplicate => {
                app.begin_duplicate();
                return false;
            }
            ListAction::TogglePin => {
                app.toggle_pin();
                return false;
            }
            ListAction::CycleSort => {
                app.cycle_sort();
                return false;
            }
            ListAction::Search => {
                app.begin_search();
                return false;
            }
            ListAction::JumpToTop => {
                app.jump_to(count, true);
                return false;
            }
            ListAction::JumpToBottom => {
                app.jump_to(count, false);
                return false;
            }
            ListAction::PageUp => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    app.page_up();
                }
                return false;
            }
            ListAction::PageDown => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    app.page_down();
                }
                return false;
            }
            ListAction::OpenTrash => {
                app.open_trash_view();
                return false;
            }
            ListAction::TogglePreview => {
                app.toggle_preview();
                return false;
            }
            ListAction::TogglePreviewFullscreen => {
                app.toggle_preview_fullscreen();
                return false;
            }
            ListAction::TogglePreviewWrap => {
                app.toggle_preview_wrap();
                return false;
            }
            ListAction::ToggleCalendar => {
                app.toggle_calendar();
                return false;
            }
            ListAction::ToggleFoldersFirst => {
                app.toggle_folders_first();
                return false;
            }
            ListAction::OpenGraph => {
                app.open_graph_view();
                return false;
            }
            ListAction::OpenCanvas => {
                app.open_draw_view();
                return false;
            }
            ListAction::CollapseAll => {
                app.collapse_all_folders();
                return false;
            }
            ListAction::ExpandAll => {
                app.expand_all_folders();
                return false;
            }
            ListAction::ExpandToLevel => {
                app.expand_to_level(count.unwrap_or(1) as usize);
                return false;
            }
            ListAction::RefreshNotes => {
                app.list.folder_cache = None;
                if let Err(e) = app.refresh_notes() {
                    app.set_temporary_status(&format!("Refresh failed: {e}"));
                } else {
                    app.set_temporary_status_static("Notes refreshed");
                }
                return false;
            }
            ListAction::PreviewPageUp => match &mut app.list.preview_content {
                Some(crate::list_view::PreviewContent::Markdown(renderer)) => {
                    renderer.prev_page();
                }
                Some(
                    crate::list_view::PreviewContent::CanvasGrid(_)
                    | crate::list_view::PreviewContent::DrawGrid(_),
                ) => {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_sub(3);
                }
                None => {}
            },
            ListAction::PreviewPageDown => match &mut app.list.preview_content {
                Some(crate::list_view::PreviewContent::Markdown(renderer)) => {
                    renderer.next_page();
                }
                Some(
                    crate::list_view::PreviewContent::CanvasGrid(_)
                    | crate::list_view::PreviewContent::DrawGrid(_),
                ) => {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_add(3);
                }
                None => {}
            },
            _ => {}
        },
        crate::keybinds::MatchOutcome::Pending => return false,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }

    false
}
pub fn handle_list_mouse(app: &mut App, mouse_event: MouseEvent, terminal_area: Rect) {
    if app.layout_edit {
        handle_layout_edit_mouse(app, mouse_event, terminal_area);
        return;
    }
    if app.popups.confirm.is_some() {
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Confirm, terminal_area);
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
            } else {
                app.cancel_confirm();
            }
        }
        return;
    }
    if let Some(popup) = app.popups.active.take() {
        match popup {
            crate::popups::ActivePopup::Goals(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(area, mouse_event.column, mouse_event.row)
                {
                    return;
                }
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    let inner = area.inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    });
                    move_textarea_cursor_to_mouse(
                        &mut p.input,
                        inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                }
                app.popups.active = Some(crate::popups::ActivePopup::Goals(p));
                return;
            }
            crate::popups::ActivePopup::NoteRename(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(area, mouse_event.column, mouse_event.row)
                {
                    return;
                }
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    let inner = area.inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    });
                    move_textarea_cursor_to_mouse(
                        &mut p.input,
                        inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                }
                app.popups.active = Some(crate::popups::ActivePopup::NoteRename(p));
                return;
            }
            crate::popups::ActivePopup::CreateNote(mut p, format) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(area, mouse_event.column, mouse_event.row)
                {
                    return;
                }
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    let inner = area.inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    });
                    move_textarea_cursor_to_mouse(
                        &mut p.input,
                        inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                }
                app.popups.active = Some(crate::popups::ActivePopup::CreateNote(p, format));
                return;
            }
            crate::popups::ActivePopup::Import(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(area, mouse_event.column, mouse_event.row)
                {
                    return;
                }
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    let inner = area.inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    });
                    move_textarea_cursor_to_mouse(
                        &mut p.input,
                        inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                }
                app.popups.active = Some(crate::popups::ActivePopup::Import(p));
                return;
            }
            crate::popups::ActivePopup::Folder(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(area, mouse_event.column, mouse_event.row)
                {
                    return;
                }
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    let inner = area.inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    });
                    move_textarea_cursor_to_mouse(
                        &mut p.input,
                        inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                }
                app.popups.active = Some(crate::popups::ActivePopup::Folder(p));
                return;
            }
            crate::popups::ActivePopup::Tag(mut p) => {
                let suggestion_height = if p.suggestions.is_empty() {
                    0
                } else {
                    (p.suggestions.len() as u16).clamp(1, 5)
                };
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
                {
                    return;
                }
                let content = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(popup_area)[0];
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3 + suggestion_height),
                        Constraint::Min(3),
                    ])
                    .split(content);
                let input_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(0)])
                    .split(chunks[0]);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    if contains_cell(chunks[1], mouse_event.column, mouse_event.row) {
                        if !p.all_tags.is_empty() {
                            let row = mouse_event
                                .row
                                .saturating_sub(chunks[1].y)
                                .saturating_sub(1) as usize;
                            p.all_tags_selected = row.min(p.all_tags.len() - 1);
                            p.focus = crate::popups::TagPopupFocus::AllTagsList;
                        }
                    } else if !p.suggestions.is_empty()
                        && contains_cell(input_chunks[1], mouse_event.column, mouse_event.row)
                    {
                        let row = mouse_event.row.saturating_sub(input_chunks[1].y) as usize;
                        p.suggestion_index = row.min(p.suggestions.len() - 1);
                        app.popups.active = Some(crate::popups::ActivePopup::Tag(p));
                        app.accept_tag_suggestion();
                        return;
                    } else if contains_cell(input_chunks[0], mouse_event.column, mouse_event.row) {
                        p.focus = crate::popups::TagPopupFocus::Input;
                        move_textarea_cursor_to_mouse(
                            &mut p.input,
                            input_chunks[0],
                            mouse_event.column,
                            mouse_event.row,
                        );
                    }
                }
                app.popups.active = Some(crate::popups::ActivePopup::Tag(p));
                return;
            }
            crate::popups::ActivePopup::Theme(mut p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
                let content = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(popup_area)[0];
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
                {
                    return;
                }
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(0),
                        Constraint::Length(3),
                        Constraint::Length(3),
                    ])
                    .split(content);
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    if contains_cell(chunks[0], mouse_event.column, mouse_event.row) {
                        let row = mouse_event
                            .row
                            .saturating_sub(chunks[0].y)
                            .saturating_sub(1) as usize;
                        if !p.themes.is_empty() {
                            let clicked = row.min(p.themes.len() - 1);
                            let was_selected = p.selected == clicked
                                && matches!(p.focus, crate::app::ThemePopupFocus::ThemeList);
                            p.selected = clicked;
                            p.focus = crate::app::ThemePopupFocus::ThemeList;
                            app.popups.active = Some(crate::popups::ActivePopup::Theme(p));
                            app.select_theme();
                            if was_selected {
                                app.close_theme_popup();
                            }
                            return;
                        }
                    } else if contains_cell(chunks[1], mouse_event.column, mouse_event.row) {
                        p.focus = crate::app::ThemePopupFocus::GeneralBg;
                        app.popups.active = Some(crate::popups::ActivePopup::Theme(p));
                        app.select_theme();
                        return;
                    } else if contains_cell(chunks[2], mouse_event.column, mouse_event.row) {
                        p.focus = crate::app::ThemePopupFocus::GraphBg;
                        app.popups.active = Some(crate::popups::ActivePopup::Theme(p));
                        app.select_theme();
                        return;
                    }
                }
                app.popups.active = Some(crate::popups::ActivePopup::Theme(p));
                return;
            }
            other => {
                app.popups.active = Some(other);
            }
        }
    }

    if let Some(mut palette) = app.command_palette.take() {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(popup_area)[0];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // search input
                Constraint::Length(1), // tab bar
                Constraint::Min(0),    // results list
            ])
            .split(content);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            return;
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            if contains_cell(chunks[0], mouse_event.column, mouse_event.row) {
                let inner = chunks[0].inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                });
                move_textarea_cursor_to_mouse(
                    &mut palette.input,
                    inner,
                    mouse_event.column,
                    mouse_event.row,
                );
            } else if mouse_event.row == chunks[1].y {
                let tabs: Vec<(&str, Option<&str>)> =
                    crate::palette::palette_tabs(app.config.ui.icon_mode)
                        .iter()
                        .map(|(l, g, _)| (*l, Some(*g)))
                        .collect();
                if let Some(i) = crate::ui::hit_test_tabs(
                    &tabs,
                    chunks[1].x,
                    chunks[1].width,
                    chunks[1].x, // no title badge to avoid
                    mouse_event.column,
                    app.config.ui.tab_icons_only,
                    app.config.ui.icon_mode,
                ) {
                    palette.active_tab = i;
                    palette.refresh_items(app);
                    palette.state.select(Some(0));
                }
            } else if contains_cell(chunks[2], mouse_event.column, mouse_event.row) {
                let row = mouse_event
                    .row
                    .saturating_sub(chunks[2].y)
                    .saturating_sub(1) as usize;
                // Palette items are 2 lines each (title + description).
                // Add the list's scroll offset so clicking after a scroll
                // hits the correct item.
                let scroll_offset = palette.state.offset();
                let clicked = scroll_offset + row / 2;
                if clicked < palette.items.len() {
                    if Some(clicked) == palette.state.selected() {
                        let item = &palette.items[clicked];
                        let action_id = item.id.clone();
                        let note_id = palette.context_note_id.clone();
                        if let Err(e) =
                            crate::actions::execute_action(&action_id, app, note_id.as_deref())
                        {
                            app.set_temporary_status(&format!("Action failed: {}", e));
                        }
                        app.command_palette = None;
                        return;
                    } else {
                        palette.state.select(Some(clicked));
                    }
                }
            }
        } else if mouse_event.kind == MouseEventKind::ScrollUp {
            if contains_cell(popup_area, mouse_event.column, mouse_event.row)
                && !palette.items.is_empty()
            {
                let current = palette.state.selected().unwrap_or(0);
                palette.state.select(Some(current.saturating_sub(1)));
            }
        } else if mouse_event.kind == MouseEventKind::ScrollDown
            && contains_cell(popup_area, mouse_event.column, mouse_event.row)
            && !palette.items.is_empty()
        {
            let current = palette.state.selected().unwrap_or(0);
            let next = (current + 1).min(palette.items.len().saturating_sub(1));
            palette.state.select(Some(next));
        }
        app.command_palette = Some(palette);
        return;
    }

    if matches!(
        app.popups.active,
        Some(crate::popups::ActivePopup::Template(_))
    ) {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.active = None;
            return;
        }
    }

    if let Some(crate::popups::ActivePopup::Template(popup)) = &mut app.popups.active {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(popup_area);

        let mut open_selected = false;
        let mut edit_selected = false;

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[0], mouse_event.column, mouse_event.row)
        {
            popup.focus = crate::app::TemplatePopupFocus::Search;
        } else if contains_cell(chunks[1], mouse_event.column, mouse_event.row)
            && (mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                || mouse_event.kind == MouseEventKind::Down(MouseButton::Right))
        {
            popup.focus = crate::app::TemplatePopupFocus::Results;
            if !popup.filtered_templates.is_empty() {
                let row = mouse_event
                    .row
                    .saturating_sub(chunks[1].y.saturating_add(1))
                    as usize;
                let clicked = row.min(popup.filtered_templates.len().saturating_sub(1));
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && clicked == popup.selected
                {
                    open_selected = true;
                }
                popup.selected = clicked;
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
                    edit_selected = true;
                }
            }
        }
        if edit_selected {
            app.edit_selected_template_from_popup();
            return;
        }
        if open_selected {
            app.select_template();
            return;
        }

        return;
    }

    if matches!(
        app.popups.active,
        Some(crate::popups::ActivePopup::FolderPicker(_))
    ) {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.active = None;
            return;
        }
    }

    if let Some(crate::popups::ActivePopup::FolderPicker(picker)) = &mut app.popups.active {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(popup_area);

        let mut confirm_selected = false;

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[0], mouse_event.column, mouse_event.row)
        {
            picker.focus = crate::app::FolderPickerFocus::Search;
            let inner = chunks[0].inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            move_textarea_cursor_to_mouse(
                &mut picker.input,
                inner,
                mouse_event.column,
                mouse_event.row,
            );
        } else if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[1], mouse_event.column, mouse_event.row)
        {
            picker.focus = crate::app::FolderPickerFocus::Results;
            if !picker.filtered_folders.is_empty() {
                let row = mouse_event
                    .row
                    .saturating_sub(chunks[1].y.saturating_add(1))
                    as usize;
                let clicked = row.min(picker.filtered_folders.len().saturating_sub(1));
                if clicked == picker.selected {
                    confirm_selected = true;
                }
                picker.selected = clicked;
            }
        }

        if confirm_selected {
            app.confirm_move();
            return;
        }

        return;
    }

    if matches!(
        app.popups.active,
        Some(crate::popups::ActivePopup::Search(_))
    ) {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.active = None;
            return;
        }
    }

    if let Some(crate::popups::ActivePopup::Search(popup)) = &mut app.popups.active {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let has_filter = popup.focus != crate::popups::SearchFocus::Input
            || !popup.input.lines().join("").trim().is_empty();
        let constraints = if has_filter {
            vec![
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
            ]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(1),
            ]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(popup_area);

        let results_chunk_idx = if has_filter { 2 } else { 1 };
        let has_title = !popup.title_results.is_empty();
        let has_grep = !popup.grep_results.is_empty();
        let mut open_selected = false;

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[0], mouse_event.column, mouse_event.row)
        {
            popup.focus = crate::popups::SearchFocus::Input;
        } else if contains_cell(
            chunks[results_chunk_idx],
            mouse_event.column,
            mouse_event.row,
        ) && (mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            || mouse_event.kind == MouseEventKind::Down(MouseButton::Right))
        {
            popup.focus = crate::popups::SearchFocus::Results;
            let row = mouse_event
                .row
                .saturating_sub(chunks[results_chunk_idx].y.saturating_add(1))
                as usize;
            if has_grep {
                let clicked = row.min(popup.grep_results.len().saturating_sub(1));
                if clicked == popup.grep_selected {
                    open_selected = true;
                }
                popup.grep_selected = clicked;
            } else if has_title {
                let clicked = row.min(popup.title_results.len().saturating_sub(1));
                if clicked == popup.title_selected {
                    open_selected = true;
                }
                popup.title_selected = clicked;
            }
        }

        if open_selected {
            app.jump_to_selected_result();
            app.confirm_search();
            return;
        }

        return;
    }

    if matches!(
        app.popups.active,
        Some(crate::popups::ActivePopup::TrashView(_))
    ) {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.active = None;
            return;
        }
    }

    if let Some(crate::popups::ActivePopup::TrashView(trash)) = &mut app.popups.active {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let mut restore_selected = false;
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(popup_area, mouse_event.column, mouse_event.row)
            && !trash.items.is_empty()
        {
            let row = mouse_event
                .row
                .saturating_sub(popup_area.y.saturating_add(1)) as usize;
            let clicked = row.min(trash.items.len().saturating_sub(1));
            if clicked == trash.selected {
                restore_selected = true;
            }
            trash.selected = clicked;
        }
        if restore_selected {
            app.restore_from_trash();
            return;
        }
        return;
    }

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(terminal_area);

    let main_area = vertical_chunks[1];

    let (list_area, preview_area) = if app.preview_fullscreen {
        (main_area, Some(main_area))
    } else if app.list.preview_enabled {
        let ratio_num = (app.list.preview_width_ratio.clamp(0.2, 0.8) * 100.0).round() as u32;
        let (constraints, list_idx, p_idx) = match app.preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(ratio_num, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(ratio_num, 100),
                ],
                0,
                2,
            ),
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(main_area);
        let list_area = full_cols[list_idx];
        let preview_area = Some(full_cols[p_idx]);
        (list_area, preview_area)
    } else {
        (
            Rect::new(
                terminal_area.x,
                terminal_area.y + 1,
                terminal_area.width,
                main_area.height,
            ),
            None,
        )
    };

    let inner_list_area = Rect::new(
        list_area.x.saturating_add(2),
        list_area.y.saturating_add(1),
        list_area.width.saturating_sub(4),
        list_area.height.saturating_sub(2),
    );

    let preview_active = app.list.preview_enabled || app.preview_fullscreen;
    if preview_active
        && let Some(p_area) = preview_area
        && contains_cell(p_area, mouse_event.column, mouse_event.row)
    {
        match &mut app.list.preview_content {
            Some(crate::list_view::PreviewContent::Markdown(renderer)) => {
                if mouse_event.kind == MouseEventKind::ScrollUp {
                    renderer.prev_page();
                    return;
                }
                if mouse_event.kind == MouseEventKind::ScrollDown {
                    renderer.next_page();
                    return;
                }
            }
            Some(
                crate::list_view::PreviewContent::CanvasGrid(_)
                | crate::list_view::PreviewContent::DrawGrid(_),
            ) => {
                if mouse_event.kind == MouseEventKind::ScrollUp {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_sub(3);
                    return;
                }
                if mouse_event.kind == MouseEventKind::ScrollDown {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_add(3);
                    return;
                }
            }
            None => {}
        }
    }
    if app.preview_fullscreen {
        return;
    }

    if mouse_event.kind == MouseEventKind::ScrollUp {
        let current = app.list.list_state.selected().unwrap_or(0);
        app.list.list_state.select(Some(current.saturating_sub(1)));

        handle_list_keys(app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        return;
    }

    if mouse_event.kind == MouseEventKind::ScrollDown {
        handle_list_keys(app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        return;
    }

    if app.list.notes_layout == crate::config::NotesLayout::Grid {
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            // Vault/Pinned tabs
            if mouse_event.row == terminal_area.y {
                let tabs = [
                    (
                        "Vault",
                        Some(crate::ui::get_icon(
                            "\u{f07b}",
                            "\u{1f4c1}",
                            app.config.ui.icon_mode,
                        )),
                    ),
                    (
                        "Pinned",
                        Some(crate::ui::get_icon(
                            "\u{f4cc}",
                            "\u{1f4cc}",
                            app.config.ui.icon_mode,
                        )),
                    ),
                ];
                let region = crate::ui::title_bar_tabs_region(terminal_area, "Notes");
                if let Some(i) = crate::ui::hit_test_tabs(
                    &tabs,
                    terminal_area.x,
                    terminal_area.width,
                    region.x, // min_x: don't overlap the title badge
                    mouse_event.column,
                    app.config.ui.tab_icons_only,
                    app.config.ui.icon_mode,
                ) {
                    app.list.grid_folder = if i == 1 {
                        crate::app::VIRTUAL_PINNED_PATH.to_string()
                    } else {
                        String::new()
                    };
                    app.list.visual_index = 0;
                    app.refresh_visual_list();
                    return;
                }
            }

            // Breadcrumbs
            if mouse_event.row == list_area.y + 1
                && app.list.grid_folder != crate::app::VIRTUAL_PINNED_PATH
            {
                let mut offset = list_area.x;
                let vault_text = " \u{f07b} Vault";
                let vault_w = vault_text.chars().count() as u16;
                if mouse_event.column >= offset && mouse_event.column < offset + vault_w {
                    app.list.grid_folder = String::new();
                    app.list.visual_index = 0;
                    app.refresh_visual_list();
                    return;
                }
                offset += vault_w;
                if !app.list.grid_folder.is_empty() {
                    let parts: Vec<&str> = app.list.grid_folder.split('/').collect();
                    let mut current_path = String::new();
                    for part in parts {
                        // " / "
                        offset += 3;
                        let part_w = part.chars().count() as u16;
                        if !current_path.is_empty() {
                            current_path.push('/');
                        }
                        current_path.push_str(part);
                        if mouse_event.column >= offset && mouse_event.column < offset + part_w {
                            app.list.grid_folder = current_path;
                            app.list.visual_index = 0;
                            app.refresh_visual_list();
                            return;
                        }
                        offset += part_w;
                    }
                }
            }
        }

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            for tile in &app.list.grid_tiles {
                if contains_cell(tile.rect, mouse_event.column, mouse_event.row) {
                    let clicked = tile.visual_index;
                    let is_select_mode = app.list.list_mode == crate::list_view::ListMode::Select
                        || app.list.tag_to_assign.is_some();
                    if is_select_mode {
                        app.list.visual_index = clicked;
                        if app.list.selected_indices.contains(&clicked) {
                            app.list.selected_indices.remove(&clicked);
                        } else {
                            app.list.selected_indices.insert(clicked);
                        }
                    } else if app.list.visual_index == clicked {
                        app.open_selected();
                    } else {
                        app.list.visual_index = clicked;
                        app.request_preview_update_immediate();
                    }
                    return;
                }
            }
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
            for tile in &app.list.grid_tiles {
                if contains_cell(tile.rect, mouse_event.column, mouse_event.row) {
                    app.list.visual_index = tile.visual_index;
                    app.request_preview_update_immediate();
                    break;
                }
            }
            if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
                app.list.visual_list.get(app.list.visual_index)
            {
                let id = app.notes[*summary_idx].id.clone();
                app.command_palette = Some(crate::palette::CommandPalette::new(Some(id), app));
            } else {
                app.command_palette = Some(crate::palette::CommandPalette::new(None, app));
            }
            return;
        }
        return; // grid never uses the row-based mapping below
    }

    if !contains_cell(inner_list_area, mouse_event.column, mouse_event.row) {
        return;
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
        let visual_row = mouse_event.row.saturating_sub(inner_list_area.y) as usize;
        let clicked_visual_index = app.list.list_state.offset().saturating_add(visual_row);
        if clicked_visual_index < app.list.visual_list.len() {
            app.list.visual_index = clicked_visual_index;
            app.request_preview_update_immediate();
        }
        if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
            app.list.visual_list.get(app.list.visual_index)
        {
            let id = app.notes[*summary_idx].id.clone();
            app.command_palette = Some(crate::palette::CommandPalette::new(Some(id), app));
        } else {
            app.command_palette = Some(crate::palette::CommandPalette::new(None, app));
        }
        return;
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
        let pitch = if app.list.list_density == crate::config::ListDensity::Comfortable {
            2
        } else {
            1
        };
        let visual_row = (mouse_event.row.saturating_sub(inner_list_area.y) as usize) / pitch;
        let clicked_visual_index = app.list.list_state.offset().saturating_add(visual_row);

        if clicked_visual_index < app.list.visual_list.len() {
            if app.list.notes_layout == crate::config::NotesLayout::Tree
                && let Some(crate::list_view::VisualItem::Note { .. }) =
                    app.list.visual_list.get(clicked_visual_index)
            {
                app.list.note_drag = Some(clicked_visual_index);
            }
            let is_select_mode = app.list.list_mode == crate::list_view::ListMode::Select
                || app.list.tag_to_assign.is_some();

            if is_select_mode {
                app.list.visual_index = clicked_visual_index;
                if app.list.selected_indices.contains(&clicked_visual_index) {
                    app.list.selected_indices.remove(&clicked_visual_index);
                } else {
                    app.list.selected_indices.insert(clicked_visual_index);
                }
            } else if app.list.visual_index == clicked_visual_index {
                app.open_selected();
            } else {
                app.list.visual_index = clicked_visual_index;
                app.request_preview_update_immediate();
            }
        }
        // Check strip section clicks (Draw→open draw, Graf→open graph)
        if app.list.calendar_enabled {
            let (_list_area, _preview_area, calendar_area) = crate::ui::list_view_layout(
                terminal_area,
                app.list.preview_enabled,
                app.preview_position,
                app.list.calendar_enabled,
                app.preview_fullscreen,
                app.list.preview_width_ratio,
                app.list.calendar_height,
                app.calendar_position,
            );
            if let Some(cal_rect) = calendar_area {
                let active = app.active_strip_sections_for(cal_rect.width);
                let sec_rects = crate::ui::section_rects(cal_rect, &active);
                for (sec, r) in active.iter().zip(sec_rects.iter().copied()) {
                    if mouse_event.column >= r.x
                        && mouse_event.column < r.right()
                        && mouse_event.row >= r.y
                        && mouse_event.row < r.bottom()
                    {
                        match sec {
                            crate::config::NotesSection::Draw => {
                                app.open_draw_view();
                            }
                            crate::config::NotesSection::Graf => {
                                app.open_graph_view();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    if mouse_event.kind == MouseEventKind::Drag(MouseButton::Left) {
        if app.list.note_drag.is_some() && app.list.notes_layout == crate::config::NotesLayout::Tree
        {
            if contains_cell(inner_list_area, mouse_event.column, mouse_event.row) {
                let pitch = if app.list.list_density == crate::config::ListDensity::Comfortable {
                    2
                } else {
                    1
                };
                let visual_row =
                    (mouse_event.row.saturating_sub(inner_list_area.y) as usize) / pitch;
                let clicked_visual_index = app.list.list_state.offset().saturating_add(visual_row);
                if clicked_visual_index < app.list.visual_list.len() {
                    if let Some(crate::list_view::VisualItem::Folder { .. }) =
                        app.list.visual_list.get(clicked_visual_index)
                    {
                        app.list.drag_hover = Some(clicked_visual_index);
                    } else {
                        app.list.drag_hover = None;
                    }
                } else {
                    app.list.drag_hover = None;
                }
            } else {
                app.list.drag_hover = None;
            }
        }
        return;
    }

    if mouse_event.kind == MouseEventKind::Up(MouseButton::Left) {
        if let Some(dragged_idx) = app.list.note_drag.take()
            && let Some(hovered_idx) = app.list.drag_hover.take()
            && app.list.notes_layout == crate::config::NotesLayout::Tree
            && let Some(crate::list_view::VisualItem::Note { summary_idx, .. }) =
                app.list.visual_list.get(dragged_idx)
            && let Some(crate::list_view::VisualItem::Folder {
                path: target_folder,
                ..
            }) = app.list.visual_list.get(hovered_idx)
        {
            let note = &app.notes[*summary_idx];
            let note_id = note.id.clone();
            if note.folder == *target_folder {
                app.set_temporary_status_static("Note already in this folder");
            } else {
                match app.storage.move_note(&note_id, target_folder) {
                    Ok(_) => {
                        let _ = app.refresh_notes();
                        app.set_temporary_status_static("Note moved");
                    }
                    Err(e) => {
                        app.set_temporary_status(&format!("Failed to move note: {e}"));
                    }
                }
            }
        }
        app.refresh_visual_list();
    }
}

fn handle_layout_edit_mouse(app: &mut App, mouse: MouseEvent, terminal_area: Rect) {
    let (list_area, preview_area, calendar_area) = crate::ui::list_view_layout(
        terminal_area,
        app.list.preview_enabled,
        app.preview_position,
        app.list.calendar_enabled,
        app.preview_fullscreen,
        app.list.preview_width_ratio,
        app.list.calendar_height,
        app.calendar_position,
    );
    // Compute section rects for cycling
    let active = app.active_strip_sections_for(calendar_area.map(|c| c.width).unwrap_or(0));
    let sec_rects = calendar_area
        .map(|c| crate::ui::section_rects(c, &active))
        .unwrap_or_default();

    // Compute vertical divider (list ↔ preview)
    let vdiv_x = if app.list.preview_enabled {
        Some(match app.preview_position {
            crate::config::PreviewPosition::Left => preview_area.expect("value is present").right(),
            crate::config::PreviewPosition::Right => list_area.right(),
        })
    } else {
        None
    };

    // Compute horizontal divider (list ↔ calendar)
    let hdiv_y = calendar_area.map(|c| {
        match app.calendar_position {
            crate::config::CalendarPosition::Bottom => c.y, // top edge of bottom calendar
            crate::config::CalendarPosition::Top => c.bottom(), // bottom edge of top calendar
        }
    });

    // Content row range for the list column
    let col_top = list_area
        .top()
        .min(calendar_area.map(|c| c.top()).unwrap_or(list_area.top()));
    let col_bot = list_area.bottom().max(
        calendar_area
            .map(|c| c.bottom())
            .unwrap_or(list_area.bottom()),
    );

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // 1. Check vertical divider
            if let Some(vx) = vdiv_x
                && (mouse.column as i16 - vx as i16).abs() <= 1
                && mouse.row >= col_top
                && mouse.row < col_bot
            {
                app.layout_drag = Some(crate::app::LayoutDrag::VDivider);
                return;
            }
            // 2. Check horizontal divider
            if let Some(hy) = hdiv_y
                && (mouse.row as i16 - hy as i16).abs() <= 1
                && mouse.column >= list_area.left()
                && mouse.column < list_area.right()
            {
                app.layout_drag = Some(crate::app::LayoutDrag::HDivider);
                return;
            }
            // 3. Check preview area (for swap)
            if let Some(p) = preview_area
                && mouse.column >= p.x
                && mouse.column < p.right()
                && mouse.row >= p.y
                && mouse.row < p.bottom()
            {
                app.layout_drag = Some(crate::app::LayoutDrag::PreviewSwap);
                return;
            }
            // 4. Check strip sections (cycle on click)
            for (i, r) in sec_rects.iter().enumerate() {
                if mouse.column >= r.x
                    && mouse.column < r.right()
                    && mouse.row >= r.y
                    && mouse.row < r.bottom()
                {
                    app.cycle_section(i);
                    return;
                }
            }
            // 4b. Click on empty strip space → add section if only one exists
            if app.list.sections.len() < 2
                && let Some(c) = calendar_area
                && mouse.column >= c.x
                && mouse.column < c.right()
                && mouse.row >= c.y
                && mouse.row < c.bottom()
            {
                app.toggle_section();
                return;
            }
            // 5. Check calendar area (for swap)
            if let Some(c) = calendar_area
                && mouse.column >= c.x
                && mouse.column < c.right()
                && mouse.row >= c.y
                && mouse.row < c.bottom()
            {
                app.layout_drag = Some(crate::app::LayoutDrag::CalendarSwap);
                return;
            }
            app.layout_drag = None;
        }
        MouseEventKind::Drag(MouseButton::Left) => match app.layout_drag {
            Some(crate::app::LayoutDrag::VDivider) => {
                let area_right = terminal_area.x.saturating_add(terminal_area.width);
                let preview_cols = match app.preview_position {
                    crate::config::PreviewPosition::Right => {
                        area_right.saturating_sub(mouse.column)
                    }
                    crate::config::PreviewPosition::Left => {
                        mouse.column.saturating_sub(terminal_area.x)
                    }
                };
                let ratio = preview_cols as f32 / terminal_area.width as f32;
                app.adjust_preview_width_to(ratio);
            }
            Some(crate::app::LayoutDrag::HDivider) => {
                let new_h = match app.calendar_position {
                    crate::config::CalendarPosition::Bottom => col_bot.saturating_sub(mouse.row),
                    crate::config::CalendarPosition::Top => {
                        mouse.row.saturating_sub(col_top).saturating_add(1)
                    }
                };
                app.adjust_calendar_height_to(new_h);
            }
            _ => {}
        },
        MouseEventKind::Up(MouseButton::Left) => {
            match app.layout_drag {
                Some(crate::app::LayoutDrag::PreviewSwap) => {
                    let mid = terminal_area.x.saturating_add(terminal_area.width / 2);
                    let on_left = mouse.column < mid;
                    let start_left =
                        matches!(app.preview_position, crate::config::PreviewPosition::Left);
                    if on_left != start_left {
                        app.swap_preview_position();
                    }
                }
                Some(crate::app::LayoutDrag::CalendarSwap) => {
                    let mid = col_top + (col_bot - col_top) / 2;
                    let on_top = mouse.row < mid;
                    let start_top =
                        matches!(app.calendar_position, crate::config::CalendarPosition::Top);
                    if on_top != start_top {
                        app.swap_calendar_position();
                    }
                }
                Some(crate::app::LayoutDrag::VDivider) | Some(crate::app::LayoutDrag::HDivider) => {
                    app.persist_list_layout();
                }
                None => {}
            }
            app.layout_drag = None;
        }
        _ => {}
    }
}
