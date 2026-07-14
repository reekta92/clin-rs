use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

use crate::app::App;
use crate::popups::*;

use super::{contains_cell, move_textarea_cursor_to_mouse};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

enum ListPopupMouseAction {
    None,
    Selected,
    Confirm,
    Dismissed,
}

/// Handle mouse events for popups that display a simple list of options
/// rendered via `render_list_with_selection` / `paint_list_hover`
/// (CreateFormat, Sort, IconMode, HintBarStyle, KeybindPreset).
fn handle_list_popup_mouse(
    mouse: &MouseEvent,
    popup_area: Rect,
    selected: &mut usize,
    item_count: usize,
) -> ListPopupMouseAction {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if !contains_cell(popup_area, mouse.column, mouse.row) {
                return ListPopupMouseAction::Dismissed;
            }
            // list_inner starts at popup_area.y + 1 (content Y + block border)
            let row = mouse.row.saturating_sub(popup_area.y).saturating_sub(1) as usize;
            let clicked = row.min(item_count.saturating_sub(1));
            if *selected == clicked {
                ListPopupMouseAction::Confirm
            } else {
                *selected = clicked;
                ListPopupMouseAction::Selected
            }
        }
        MouseEventKind::ScrollUp if contains_cell(popup_area, mouse.column, mouse.row) => {
            *selected = selected.saturating_sub(1);
            ListPopupMouseAction::Selected
        }
        MouseEventKind::ScrollDown if contains_cell(popup_area, mouse.column, mouse.row) => {
            *selected = (*selected + 1).min(item_count.saturating_sub(1));
            ListPopupMouseAction::Selected
        }
        _ => ListPopupMouseAction::None,
    }
}

/// Handle mouse events for popups that only contain a `TextArea` input
/// (CreateNote, Goals, NoteRename, Import, Folder).
/// Returns `true` if the popup should be dismissed (outside left-click).
fn handle_text_input_popup_mouse(
    mouse: &MouseEvent,
    popup_area: Rect,
    input: &mut ratatui_textarea::TextArea<'static>,
) -> bool {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if !contains_cell(popup_area, mouse.column, mouse.row) {
                return true;
            }
            let inner = popup_area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            move_textarea_cursor_to_mouse(input, inner, mouse.column, mouse.row);
            false
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Confirm popup
// ---------------------------------------------------------------------------

fn handle_confirm_popup_mouse(app: &mut App, mouse: &MouseEvent, terminal_area: Rect) {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return;
    }
    let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Confirm, terminal_area);
    if contains_cell(popup_area, mouse.column, mouse.row) {
        let mid_x = popup_area.x + popup_area.width / 2;
        if mouse.column < mid_x {
            app.confirm_action();
        } else {
            app.cancel_confirm();
        }
    } else {
        app.cancel_confirm();
    }
}

// ---------------------------------------------------------------------------
// Command palette
// ---------------------------------------------------------------------------

fn handle_command_palette_mouse(app: &mut App, mouse: &MouseEvent, terminal_area: Rect) -> bool {
    let Some(mut palette) = app.command_palette.take() else {
        return false;
    };
    let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
    let content = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(popup_area)[0];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(content);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if !contains_cell(popup_area, mouse.column, mouse.row) {
                return true;
            }
            if contains_cell(chunks[0], mouse.column, mouse.row) {
                let inner = chunks[0].inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                });
                move_textarea_cursor_to_mouse(&mut palette.input, inner, mouse.column, mouse.row);
            } else if mouse.row == chunks[1].y {
                let tabs: Vec<(&str, Option<&str>)> =
                    crate::palette::palette_tabs(app.config.ui.icon_mode)
                        .iter()
                        .map(|(l, g, _)| (*l, Some(*g)))
                        .collect();
                if let Some(i) = crate::ui::hit_test_tabs(
                    &tabs,
                    chunks[1].x,
                    chunks[1].width,
                    chunks[1].x,
                    mouse.column,
                    app.config.ui.tab_icons_only,
                    app.config.ui.icon_mode,
                ) {
                    palette.active_tab = i;
                    palette.refresh_items(app);
                    palette.state.select(Some(0));
                }
            } else if contains_cell(chunks[2], mouse.column, mouse.row) {
                let row = mouse.row.saturating_sub(chunks[2].y).saturating_sub(1) as usize;
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
                            app.set_temporary_status(&format!("Action failed: {e}"));
                        }
                        return true;
                    } else {
                        palette.state.select(Some(clicked));
                    }
                }
            }
        }
        MouseEventKind::ScrollUp
            if contains_cell(popup_area, mouse.column, mouse.row) && !palette.items.is_empty() =>
        {
            let current = palette.state.selected().unwrap_or(0);
            palette.state.select(Some(current.saturating_sub(1)));
        }
        MouseEventKind::ScrollDown
            if contains_cell(popup_area, mouse.column, mouse.row) && !palette.items.is_empty() =>
        {
            let current = palette.state.selected().unwrap_or(0);
            let next = (current + 1).min(palette.items.len().saturating_sub(1));
            palette.state.select(Some(next));
        }
        _ => {}
    }

    app.command_palette = Some(palette);
    true
}

// ---------------------------------------------------------------------------
// ActivePopup::handle_mouse
// ---------------------------------------------------------------------------

impl crate::popups::ActivePopup {
    fn handle_mouse(self, app: &mut App, mouse: &MouseEvent, terminal_area: Rect) -> bool {
        use crate::popups::ActivePopup::{
            ContextMenu, CreateFormat, CreateNote, Folder, FolderPicker, Goals, HintBarStyle,
            IconMode, Import, Info, KeybindPreset, NoteRename, Search, Sort, Subnotes, Tag,
            Template, Theme, TrashView,
        };
        match self {
            // === Group A: Simple list-style popups ===
            CreateFormat(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
                match handle_list_popup_mouse(mouse, area, &mut p.selected, 4) {
                    ListPopupMouseAction::Selected | ListPopupMouseAction::None => {
                        app.popups.active = Some(CreateFormat(p));
                    }
                    ListPopupMouseAction::Confirm => {
                        app.popups.active = Some(CreateFormat(p));
                        app.confirm_create_format();
                    }
                    ListPopupMouseAction::Dismissed => {}
                }
                true
            }
            Sort(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
                match handle_list_popup_mouse(mouse, area, &mut p.selected, 4) {
                    ListPopupMouseAction::Selected | ListPopupMouseAction::None => {
                        app.popups.active = Some(Sort(p));
                    }
                    ListPopupMouseAction::Confirm => {
                        app.popups.active = Some(Sort(p));
                        app.select_sort();
                    }
                    ListPopupMouseAction::Dismissed => {}
                }
                true
            }
            IconMode(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
                match handle_list_popup_mouse(mouse, area, &mut p.selected, 3) {
                    ListPopupMouseAction::Selected | ListPopupMouseAction::None => {
                        app.popups.active = Some(IconMode(p));
                    }
                    ListPopupMouseAction::Confirm => {
                        app.popups.active = Some(IconMode(p));
                        app.select_icon_mode();
                    }
                    ListPopupMouseAction::Dismissed => {}
                }
                true
            }
            HintBarStyle(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
                match handle_list_popup_mouse(mouse, area, &mut p.selected, 4) {
                    ListPopupMouseAction::Selected => {
                        app.popups.active = Some(HintBarStyle(p));
                        app.select_hint_bar_style();
                    }
                    ListPopupMouseAction::Confirm => {
                        app.popups.active = Some(HintBarStyle(p));
                        app.select_hint_bar_style();
                        app.close_hint_bar_style_popup();
                    }
                    ListPopupMouseAction::None => {
                        app.popups.active = Some(HintBarStyle(p));
                    }
                    ListPopupMouseAction::Dismissed => {}
                }
                true
            }
            KeybindPreset(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
                match handle_list_popup_mouse(mouse, area, &mut p.selected, 4) {
                    ListPopupMouseAction::Selected => {
                        app.popups.active = Some(KeybindPreset(p));
                        app.select_keybind_preset();
                    }
                    ListPopupMouseAction::Confirm => {
                        app.popups.active = Some(KeybindPreset(p));
                        app.select_keybind_preset();
                        app.close_keybind_preset_popup();
                    }
                    ListPopupMouseAction::None => {
                        app.popups.active = Some(KeybindPreset(p));
                    }
                    ListPopupMouseAction::Dismissed => {}
                }
                true
            }

            // === Group B: Text-input popups ===
            CreateNote(mut p, format) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if !handle_text_input_popup_mouse(mouse, area, &mut p.input) {
                    app.popups.active = Some(CreateNote(p, format));
                }
                true
            }
            Goals(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if !handle_text_input_popup_mouse(mouse, area, &mut p.input) {
                    app.popups.active = Some(Goals(p));
                }
                true
            }
            NoteRename(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if !handle_text_input_popup_mouse(mouse, area, &mut p.input) {
                    app.popups.active = Some(NoteRename(p));
                }
                true
            }
            Import(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if !handle_text_input_popup_mouse(mouse, area, &mut p.input) {
                    app.popups.active = Some(Import(p));
                }
                true
            }
            Folder(mut p) => {
                let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
                if !handle_text_input_popup_mouse(mouse, area, &mut p.input) {
                    app.popups.active = Some(Folder(p));
                }
                true
            }

            // === Group C1: Tag (multi-region) ===
            Tag(mut p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if super::dismiss_popup_on_outside_click(app, mouse, popup_area) {
                    return true;
                }
                let suggestion_height = if p.suggestions.is_empty() {
                    0
                } else {
                    (p.suggestions.len() as u16).clamp(1, 5)
                };
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
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    if contains_cell(chunks[1], mouse.column, mouse.row) {
                        if !p.all_tags.is_empty() {
                            let row =
                                mouse.row.saturating_sub(chunks[1].y).saturating_sub(1) as usize;
                            p.all_tags_selected = row.min(p.all_tags.len().saturating_sub(1));
                            p.focus = TagPopupFocus::AllTagsList;
                        }
                    } else if !p.suggestions.is_empty()
                        && contains_cell(input_chunks[1], mouse.column, mouse.row)
                    {
                        let row = mouse.row.saturating_sub(input_chunks[1].y) as usize;
                        p.suggestion_index = row.min(p.suggestions.len().saturating_sub(1));
                        app.popups.active = Some(Tag(p));
                        app.accept_tag_suggestion();
                        return true;
                    } else if contains_cell(input_chunks[0], mouse.column, mouse.row) {
                        p.focus = TagPopupFocus::Input;
                        move_textarea_cursor_to_mouse(
                            &mut p.input,
                            input_chunks[0],
                            mouse.column,
                            mouse.row,
                        );
                    }
                }
                app.popups.active = Some(Tag(p));
                true
            }

            // === Group C2: Theme (multi-region) ===
            Theme(mut p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
                let content = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(popup_area)[0];
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(popup_area, mouse.column, mouse.row)
                {
                    return true;
                }
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(0),
                        Constraint::Length(3),
                        Constraint::Length(3),
                    ])
                    .split(content);
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    if contains_cell(chunks[0], mouse.column, mouse.row) {
                        let row = mouse.row.saturating_sub(chunks[0].y).saturating_sub(1) as usize;
                        if !p.themes.is_empty() {
                            let clicked = row.min(p.themes.len().saturating_sub(1));
                            let was_selected = p.selected == clicked
                                && matches!(p.focus, crate::app::ThemePopupFocus::ThemeList);
                            p.selected = clicked;
                            p.focus = crate::app::ThemePopupFocus::ThemeList;
                            app.popups.active = Some(Theme(p));
                            app.select_theme();
                            if was_selected {
                                app.close_theme_popup();
                            }
                            return true;
                        }
                    } else if contains_cell(chunks[1], mouse.column, mouse.row) {
                        p.focus = crate::app::ThemePopupFocus::GeneralBg;
                        app.popups.active = Some(Theme(p));
                        app.select_theme();
                        return true;
                    } else if contains_cell(chunks[2], mouse.column, mouse.row) {
                        p.focus = crate::app::ThemePopupFocus::GraphBg;
                        app.popups.active = Some(Theme(p));
                        app.select_theme();
                        return true;
                    }
                }
                app.popups.active = Some(Theme(p));
                true
            }

            // === Group C3: Template ===
            Template(mut p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if super::dismiss_popup_on_outside_click(app, mouse, popup_area) {
                    return true;
                }
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
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(chunks[0], mouse.column, mouse.row)
                {
                    p.focus = crate::popups::TemplatePopupFocus::Search;
                } else if contains_cell(chunks[1], mouse.column, mouse.row)
                    && (mouse.kind == MouseEventKind::Down(MouseButton::Left)
                        || mouse.kind == MouseEventKind::Down(MouseButton::Right))
                {
                    p.focus = crate::popups::TemplatePopupFocus::Results;
                    if !p.filtered_templates.is_empty() {
                        let row = mouse.row.saturating_sub(chunks[1].y.saturating_add(1)) as usize;
                        let clicked = row.min(p.filtered_templates.len().saturating_sub(1));
                        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                            && clicked == p.selected
                        {
                            open_selected = true;
                        }
                        p.selected = clicked;
                        if mouse.kind == MouseEventKind::Down(MouseButton::Right) {
                            edit_selected = true;
                        }
                    }
                }
                if edit_selected {
                    app.edit_selected_template_from_popup();
                    return true;
                }
                if open_selected {
                    app.select_template();
                    return true;
                }
                app.popups.active = Some(Template(p));
                true
            }

            // === Group C4: FolderPicker ===
            FolderPicker(mut p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if super::dismiss_popup_on_outside_click(app, mouse, popup_area) {
                    return true;
                }
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1)])
                    .split(popup_area);
                let mut confirm_selected = false;
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(chunks[0], mouse.column, mouse.row)
                {
                    p.focus = crate::app::FolderPickerFocus::Search;
                    let inner = chunks[0].inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    });
                    move_textarea_cursor_to_mouse(&mut p.input, inner, mouse.column, mouse.row);
                } else if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(chunks[1], mouse.column, mouse.row)
                {
                    p.focus = crate::app::FolderPickerFocus::Results;
                    if !p.filtered_folders.is_empty() {
                        let row = mouse.row.saturating_sub(chunks[1].y.saturating_add(1)) as usize;
                        let clicked = row.min(p.filtered_folders.len().saturating_sub(1));
                        if clicked == p.selected {
                            confirm_selected = true;
                        }
                        p.selected = clicked;
                    }
                }
                if confirm_selected {
                    app.confirm_move();
                    return true;
                }
                app.popups.active = Some(FolderPicker(p));
                true
            }

            // === Group C5: Search ===
            Search(mut p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if super::dismiss_popup_on_outside_click(app, mouse, popup_area) {
                    return true;
                }
                let query_text = p.input.lines().join("");
                let parsed = crate::app::parse_search_query(&query_text);
                let has_filter = parsed.folder_filter.is_some()
                    || parsed.pinned_only
                    || parsed.tag_filter.is_some()
                    || parsed.grep_mode;
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
                let has_title = !p.title_results.is_empty();
                let has_grep = !p.grep_results.is_empty();
                // Scroll over results chunk
                if contains_cell(chunks[results_chunk_idx], mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            p.focus = SearchFocus::Results;
                            if has_grep {
                                if p.grep_selected > 0 {
                                    // skip collapsed header children
                                    let mut i = p.grep_selected - 1;
                                    p.grep_selected = loop {
                                        if p.grep_is_header[i] {
                                            break i;
                                        }
                                        let mut parent = i;
                                        while parent > 0 && !p.grep_is_header[parent] {
                                            parent -= 1;
                                        }
                                        if p.grep_expanded.contains(&parent) {
                                            break i;
                                        }
                                        if i == 0 {
                                            break 0;
                                        }
                                        i -= 1;
                                    };
                                }
                            } else if has_title {
                                p.title_selected = p.title_selected.saturating_sub(1);
                            }
                            app.popups.active = Some(Search(p));
                            return true;
                        }
                        MouseEventKind::ScrollDown => {
                            p.focus = SearchFocus::Results;
                            if has_grep {
                                if p.grep_selected + 1 < p.grep_results.len() {
                                    let mut i = p.grep_selected + 1;
                                    p.grep_selected = loop {
                                        if p.grep_is_header[i] {
                                            break i;
                                        }
                                        let mut parent = i;
                                        while parent > 0 && !p.grep_is_header[parent] {
                                            parent -= 1;
                                        }
                                        if p.grep_expanded.contains(&parent) {
                                            break i;
                                        }
                                        i += 1;
                                        if i >= p.grep_results.len() {
                                            break p.grep_selected;
                                        }
                                    };
                                }
                            } else if has_title && p.title_selected + 1 < p.title_results.len() {
                                p.title_selected += 1;
                            }
                            app.popups.active = Some(Search(p));
                            return true;
                        }
                        _ => {}
                    }
                }
                let mut open_selected = false;
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(chunks[0], mouse.column, mouse.row)
                {
                    p.focus = SearchFocus::Input;
                } else if contains_cell(chunks[results_chunk_idx], mouse.column, mouse.row)
                    && (mouse.kind == MouseEventKind::Down(MouseButton::Left)
                        || mouse.kind == MouseEventKind::Down(MouseButton::Right))
                {
                    p.focus = SearchFocus::Results;
                    let row = mouse
                        .row
                        .saturating_sub(chunks[results_chunk_idx].y.saturating_add(1))
                        as usize;
                    if has_grep {
                        let clicked = row.min(p.grep_results.len().saturating_sub(1));
                        if clicked == p.grep_selected {
                            open_selected = true;
                        }
                        p.grep_selected = clicked;
                    } else if has_title {
                        let clicked = row.min(p.title_results.len().saturating_sub(1));
                        if clicked == p.title_selected {
                            open_selected = true;
                        }
                        p.title_selected = clicked;
                    }
                }
                if open_selected {
                    app.jump_to_selected_result();
                    app.confirm_search();
                    return true;
                }
                app.popups.active = Some(Search(p));
                true
            }

            // === Group C6: TrashView ===
            TrashView(mut trash) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if super::dismiss_popup_on_outside_click(app, mouse, popup_area) {
                    return true;
                }
                let mut restore_selected = false;
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(popup_area, mouse.column, mouse.row)
                    && !trash.items.is_empty()
                {
                    let row = mouse.row.saturating_sub(popup_area.y.saturating_add(1)) as usize;
                    let clicked = row.min(trash.items.len().saturating_sub(1));
                    if clicked == trash.selected {
                        restore_selected = true;
                    }
                    trash.selected = clicked;
                }
                if restore_selected {
                    app.restore_from_trash();
                    return true;
                }
                app.popups.active = Some(TrashView(trash));
                true
            }

            // === Group D: Info (display-only) ===
            Info(p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(popup_area, mouse.column, mouse.row)
                {
                    // dismissed
                } else {
                    app.popups.active = Some(Info(p));
                }
                true
            }

            // === Group D: Subnotes ===
            Subnotes(mut p) => {
                let popup_area =
                    crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
                // Outside click → dismiss (save if dirty first)
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && !contains_cell(popup_area, mouse.column, mouse.row)
                {
                    if p.is_dirty
                        && let Err(e) = app.storage.set_subnotes(&p.parent_id, &p.subnotes)
                    {
                        app.set_temporary_status(&format!("Failed to save subnotes: {e}"));
                    }
                    return true;
                }
                let content = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(popup_area)[0];
                let main_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(34), Constraint::Min(0)])
                    .split(content);
                let edit_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(0)])
                    .split(main_chunks[1]);

                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    // Click in the list area (left pane)
                    if contains_cell(main_chunks[0], mouse.column, mouse.row) {
                        if !p.subnotes.is_empty() {
                            let row = mouse.row.saturating_sub(main_chunks[0].y).saturating_sub(1)
                                as usize;
                            let clicked = row.min(p.subnotes.len().saturating_sub(1));
                            if clicked == p.selected && p.focus == SubnotesFocus::List {
                                p.focus = SubnotesFocus::EditTitle;
                            } else {
                                p.selected = clicked;
                                p.focus = SubnotesFocus::List;
                                p.title_input = crate::ui::make_popup_textarea(&app.app_theme, "");
                                p.title_input.insert_str(&p.subnotes[p.selected].title);
                                p.content_input =
                                    crate::ui::make_popup_textarea(&app.app_theme, "");
                                p.content_input.insert_str(&p.subnotes[p.selected].content);
                            }
                        }
                    } else if contains_cell(edit_chunks[0], mouse.column, mouse.row) {
                        p.focus = SubnotesFocus::EditTitle;
                        let inner = edit_chunks[0].inner(Margin {
                            vertical: 1,
                            horizontal: 1,
                        });
                        move_textarea_cursor_to_mouse(
                            &mut p.title_input,
                            inner,
                            mouse.column,
                            mouse.row,
                        );
                    } else if contains_cell(edit_chunks[1], mouse.column, mouse.row) {
                        p.focus = SubnotesFocus::EditContent;
                        let inner = edit_chunks[1].inner(Margin {
                            vertical: 1,
                            horizontal: 1,
                        });
                        move_textarea_cursor_to_mouse(
                            &mut p.content_input,
                            inner,
                            mouse.column,
                            mouse.row,
                        );
                    }
                }
                app.popups.active = Some(Subnotes(p));
                true
            }

            // ContextMenu is handled in handle_global_popup_mouse, not here
            ContextMenu(_) => {
                unreachable!(
                    "ContextMenu should be handled by handle_global_popup_mouse \
                     before reaching handle_mouse"
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level dispatch
// ---------------------------------------------------------------------------

/// Entry point called from `src/lib.rs` for all mouse events.
///
/// Returns `true` if the event was consumed, `false` otherwise (so the
/// view-specific handler can process it).
pub fn handle_global_popup_mouse(app: &mut App, mouse: &MouseEvent, terminal_area: Rect) -> bool {
    // 1. Confirm overlay (highest priority)
    if app.popups.confirm.is_some() {
        handle_confirm_popup_mouse(app, mouse, terminal_area);
        return true;
    }

    // 2. Command palette
    if app.command_palette.is_some() {
        return handle_command_palette_mouse(app, mouse, terminal_area);
    }

    // 3. ContextMenu — special case: left-click inside menu needs EditFocus,
    //    so the centralized handler only handles scroll and outside dismiss.
    if matches!(app.popups.active, Some(ActivePopup::ContextMenu(_))) {
        let menu_rect = {
            let Some(ActivePopup::ContextMenu(menu)) = &app.popups.active else {
                unreachable!()
            };
            Rect::new(menu.x, menu.y, 14, 4)
        };
        return match mouse.kind {
            MouseEventKind::ScrollUp if contains_cell(menu_rect, mouse.column, mouse.row) => {
                let mut m = app.popups.active.take().expect("ContextMenu must be Some");
                if let ActivePopup::ContextMenu(m) = &mut m {
                    m.selected = m.selected.saturating_sub(1);
                }
                app.popups.active = Some(m);
                true
            }
            MouseEventKind::ScrollDown if contains_cell(menu_rect, mouse.column, mouse.row) => {
                let mut m = app.popups.active.take().expect("ContextMenu must be Some");
                if let ActivePopup::ContextMenu(m) = &mut m
                    && m.selected < 3
                {
                    m.selected += 1;
                }
                app.popups.active = Some(m);
                true
            }
            MouseEventKind::Down(btn) if !contains_cell(menu_rect, mouse.column, mouse.row) => {
                app.popups.active = None;
                btn != MouseButton::Right // true for left, false for right
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Left-click inside menu → let edit handler process
                // (it has EditFocus for handle_menu_action)
                false
            }
            _ => true, // consume other events while menu is open
        };
    }

    // 4. Active popup (take + route through handle_mouse)
    if let Some(popup) = app.popups.active.take() {
        popup.handle_mouse(app, mouse, terminal_area)
    } else {
        false
    }
}
