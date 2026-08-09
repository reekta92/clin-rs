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
            let Some(clicked) =
                crate::ui::list_index_at(mouse.row, popup_area.y + 1, 1, 0, item_count)
            else {
                return ListPopupMouseAction::Dismissed;
            };
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
            let (sr, sc) = crate::ui::get_textarea_scroll(input);
            move_textarea_cursor_to_mouse(input, inner, mouse.column, mouse.row, sr, sc);
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

    // --- Scrollbar handling ---
    if app.config.ui.scrollbars
        && let Some(meta) = palette.last_scroll
    {
        let max_pos = meta.content_len.saturating_sub(1);
        let frac = palette.state.selected().unwrap_or(0) as f32 / max_pos.max(1) as f32;
        if let Some(new_frac) = crate::ui::scrollbar::handle_scrollbar_mouse(
            mouse,
            meta,
            frac,
            &mut palette.scroll_drag,
        ) {
            let pos = (new_frac * max_pos as f32).round() as usize;
            palette.state.select(Some(pos.min(max_pos)));
            app.command_palette = Some(palette);
            return true;
        }
    }

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
                let (sr, sc) = crate::ui::get_textarea_scroll(&palette.input);
                move_textarea_cursor_to_mouse(
                    &mut palette.input,
                    inner,
                    mouse.column,
                    mouse.row,
                    sr,
                    sc,
                );
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
                let Some(clicked) = crate::ui::list_index_at(
                    mouse.row,
                    chunks[2].y + 1,
                    2,
                    palette.state.offset(),
                    palette.items.len(),
                ) else {
                    return true;
                };
                if Some(clicked) == palette.state.selected() {
                    let item = &palette.items[clicked];
                    let action_id = item.id.clone();
                    let note_id = palette.context_note_id.clone();
                    if let Err(e) =
                        crate::actions::execute_action(&action_id, app, note_id.as_deref())
                    {
                        app.set_temporary_status(&format!("Action failed: {e}"));
                        app.messages.push(
                            format!("Action failed: {e}"),
                            crate::app::messages::MessageSeverity::Warning,
                        );
                    }
                    return true;
                } else {
                    palette.state.select(Some(clicked));
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
// Scrollbar helpers
// ---------------------------------------------------------------------------

fn handle_popup_scrollbar(
    last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    scroll_drag: &mut Option<crate::ui::scrollbar::ScrollDrag>,
    selected: &mut usize,
    content_len: usize,
    mouse: &MouseEvent,
) -> bool {
    if let Some(meta) = last_scroll
        && content_len > meta.viewport_len
    {
        let max_pos = content_len.saturating_sub(1);
        let frac = *selected as f32 / max_pos.max(1) as f32;
        if let Some(new_frac) =
            crate::ui::scrollbar::handle_scrollbar_mouse(mouse, meta, frac, scroll_drag)
        {
            *selected = (new_frac * max_pos as f32).round() as usize;
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn handle_search_popup_scrollbar(
    popup: &mut crate::popups::SearchPopup,
    scroll_drag: &mut Option<crate::ui::scrollbar::ScrollDrag>,
    mouse: &MouseEvent,
) -> bool {
    if let Some(meta) = popup.last_scroll {
        let has_grep = !popup.grep_results.is_empty();
        let total_items = if has_grep {
            popup.total_grep_rows()
        } else {
            popup.title_result_ids.len()
        };
        if total_items > meta.viewport_len {
            let max_pos = total_items.saturating_sub(1);
            let vis_pos = if has_grep {
                popup.grep_selected
            } else {
                popup.title_selected
            };
            let frac = vis_pos as f32 / max_pos.max(1) as f32;
            if let Some(new_frac) =
                crate::ui::scrollbar::handle_scrollbar_mouse(mouse, meta, frac, scroll_drag)
            {
                let new_vis = (new_frac * max_pos as f32).round() as usize;
                if has_grep {
                    popup.grep_selected = new_vis.min(max_pos);
                } else {
                    popup.title_selected = new_vis.min(max_pos);
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}
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
                match handle_list_popup_mouse(
                    mouse,
                    area,
                    &mut p.selected,
                    crate::config::HintBarStyle::ALL.len(),
                ) {
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
                            let Some(clicked) = crate::ui::list_index_at(
                                mouse.row,
                                chunks[1].y + 1,
                                1,
                                p.scroll_offset,
                                p.all_tags.len(),
                            ) else {
                                return true;
                            };
                            let open_selected = clicked == p.all_tags_selected;
                            p.all_tags_selected = clicked;
                            p.focus = TagPopupFocus::AllTagsList;
                            if open_selected {
                                app.popups.active = Some(Tag(p));
                                app.accept_tag_from_all_tags();
                                return true;
                            }
                        }
                    } else if !p.suggestions.is_empty()
                        && contains_cell(input_chunks[1], mouse.column, mouse.row)
                    {
                        let Some(suggestion_index) = crate::ui::list_index_at(
                            mouse.row,
                            input_chunks[1].y,
                            1,
                            0,
                            p.suggestions.len(),
                        ) else {
                            return true;
                        };
                        p.suggestion_index = suggestion_index;
                        app.popups.active = Some(Tag(p));
                        app.accept_tag_suggestion();
                        return true;
                    } else if contains_cell(input_chunks[0], mouse.column, mouse.row) {
                        p.focus = TagPopupFocus::Input;
                        let (sr, sc) = crate::ui::get_textarea_scroll(&p.input);
                        move_textarea_cursor_to_mouse(
                            &mut p.input,
                            input_chunks[0],
                            mouse.column,
                            mouse.row,
                            sr,
                            sc,
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
                        let Some(clicked) = crate::ui::list_index_at(
                            mouse.row,
                            chunks[0].y + 1,
                            1,
                            p.scroll_offset,
                            p.themes.len(),
                        ) else {
                            return true;
                        };
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
                    let old_focus = p.focus;
                    p.focus = crate::popups::TemplatePopupFocus::Results;
                    if !p.filtered_templates.is_empty() {
                        let Some(clicked) = crate::ui::list_index_at(
                            mouse.row,
                            chunks[1].y + 1,
                            1,
                            p.scroll_offset,
                            p.filtered_templates.len(),
                        ) else {
                            return false;
                        };
                        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                            && clicked == p.selected
                            && old_focus == crate::popups::TemplatePopupFocus::Results
                        {
                            open_selected = true;
                        }
                        p.selected = clicked;
                        if mouse.kind == MouseEventKind::Down(MouseButton::Right) {
                            edit_selected = true;
                        }
                    }
                }
                app.popups.active = Some(Template(p));
                if edit_selected {
                    app.edit_selected_template_from_popup();
                    return true;
                }
                if open_selected {
                    app.select_template();
                    return true;
                }
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

                // Scroll wheel
                match mouse.kind {
                    MouseEventKind::ScrollUp
                        if contains_cell(popup_area, mouse.column, mouse.row) =>
                    {
                        p.selected = p.selected.saturating_sub(1);
                        app.popups.active = Some(FolderPicker(p));
                        return true;
                    }
                    MouseEventKind::ScrollDown
                        if contains_cell(popup_area, mouse.column, mouse.row) =>
                    {
                        if p.selected + 1 < p.filtered_folders.len() {
                            p.selected += 1;
                        }
                        app.popups.active = Some(FolderPicker(p));
                        return true;
                    }
                    _ => {}
                }

                let mut confirm_selected = false;
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(chunks[0], mouse.column, mouse.row)
                {
                    p.focus = crate::app::FolderPickerFocus::Search;
                    let inner = chunks[0].inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    });
                    let (sr, sc) = crate::ui::get_textarea_scroll(&p.input);
                    move_textarea_cursor_to_mouse(
                        &mut p.input,
                        inner,
                        mouse.column,
                        mouse.row,
                        sr,
                        sc,
                    );
                } else if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(chunks[1], mouse.column, mouse.row)
                {
                    p.focus = crate::app::FolderPickerFocus::Results;
                    if !p.filtered_folders.is_empty() {
                        let Some(clicked) = crate::ui::list_index_at(
                            mouse.row,
                            chunks[1].y + 1,
                            1,
                            p.scroll_offset,
                            p.filtered_folders.len(),
                        ) else {
                            return false;
                        };
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
                let has_title = !p.title_result_ids.is_empty();
                let has_grep = !p.grep_results.is_empty();
                let total_items = if has_grep {
                    p.total_grep_rows()
                } else if has_title {
                    p.title_result_ids.len()
                } else {
                    0
                };
                if contains_cell(chunks[results_chunk_idx], mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            p.focus = SearchFocus::Results;
                            if has_grep {
                                p.grep_selected = p.grep_selected.saturating_sub(1);
                            } else if has_title {
                                p.title_selected = p.title_selected.saturating_sub(1);
                            }
                            app.popups.active = Some(Search(p));
                            return true;
                        }
                        MouseEventKind::ScrollDown => {
                            p.focus = SearchFocus::Results;
                            if has_grep {
                                p.grep_selected =
                                    (p.grep_selected + 1).min(total_items.saturating_sub(1));
                            } else if has_title && p.title_selected + 1 < p.title_result_ids.len() {
                                p.title_selected += 1;
                            }
                            app.popups.active = Some(Search(p));
                            return true;
                        }
                        _ => {}
                    }
                }
                let mut open_result = false;
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(chunks[0], mouse.column, mouse.row)
                {
                    p.focus = SearchFocus::Input;
                } else if contains_cell(chunks[results_chunk_idx], mouse.column, mouse.row)
                    && (mouse.kind == MouseEventKind::Down(MouseButton::Left)
                        || mouse.kind == MouseEventKind::Down(MouseButton::Right))
                {
                    p.focus = SearchFocus::Results;
                    let Some(target_vis) = crate::ui::list_index_at(
                        mouse.row,
                        chunks[results_chunk_idx].y + 1,
                        1,
                        p.results_scroll_offset,
                        total_items,
                    ) else {
                        return true;
                    };
                    if has_grep {
                        let target_vis = target_vis.min(total_items.saturating_sub(1));
                        let already_selected = target_vis == p.grep_selected;
                        p.grep_selected = target_vis;
                        if already_selected && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                        {
                            let hit_idx = match p.grep_row_offsets.binary_search(&target_vis) {
                                Ok(i) => i,
                                Err(i) => i.saturating_sub(1),
                            };
                            let base = p.grep_row_offsets.get(hit_idx).copied().unwrap_or(0);
                            let hit = p.grep_results.get(hit_idx);
                            if target_vis == base {
                                if let Some(hit) = hit {
                                    if p.grep_expanded.contains(&hit.note_id) {
                                        p.grep_expanded.remove(&hit.note_id);
                                    } else {
                                        p.grep_expanded.insert(hit.note_id.clone());
                                    }
                                    p.rebuild_grep_offsets();
                                }
                            } else {
                                open_result = true;
                            }
                        }
                    } else if has_title {
                        let flat = target_vis.min(p.title_result_ids.len().saturating_sub(1));
                        let already_selected = flat == p.title_selected;
                        p.title_selected = flat;
                        if already_selected && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                        {
                            open_result = true;
                        }
                    }
                }
                if open_result {
                    app.popups.active = Some(Search(p));
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
                match mouse.kind {
                    MouseEventKind::ScrollUp
                        if contains_cell(popup_area, mouse.column, mouse.row) =>
                    {
                        trash.selected = trash.selected.saturating_sub(1);
                    }
                    MouseEventKind::ScrollDown
                        if contains_cell(popup_area, mouse.column, mouse.row) =>
                    {
                        trash.selected = trash
                            .selected
                            .saturating_add(1)
                            .min(trash.items.len().saturating_sub(1));
                    }
                    _ => {}
                }
                let mut restore_selected = false;
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && contains_cell(popup_area, mouse.column, mouse.row)
                    && !trash.items.is_empty()
                {
                    let Some(clicked) = crate::ui::list_index_at(
                        mouse.row,
                        popup_area.y + 1,
                        1,
                        trash.scroll_offset,
                        trash.items.len(),
                    ) else {
                        return true;
                    };
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
                            let Some(clicked) = crate::ui::list_index_at(
                                mouse.row,
                                main_chunks[0].y + 1,
                                1,
                                p.scroll_offset,
                                p.subnotes.len(),
                            ) else {
                                return false;
                            };
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
                        let (sr, sc) = crate::ui::get_textarea_scroll(&p.title_input);
                        move_textarea_cursor_to_mouse(
                            &mut p.title_input,
                            inner,
                            mouse.column,
                            mouse.row,
                            sr,
                            sc,
                        );
                    } else if contains_cell(edit_chunks[1], mouse.column, mouse.row) {
                        p.focus = SubnotesFocus::EditContent;
                        let inner = edit_chunks[1].inner(Margin {
                            vertical: 1,
                            horizontal: 1,
                        });
                        let (sr, sc) = crate::ui::get_textarea_scroll(&p.content_input);
                        move_textarea_cursor_to_mouse(
                            &mut p.content_input,
                            inner,
                            mouse.column,
                            mouse.row,
                            sr,
                            sc,
                        );
                    }
                }
                app.popups.active = Some(Subnotes(p));
                true
            }

            // RemoveTags popup: simple list, mouse handled naturally
            crate::popups::ActivePopup::RemoveTags(_) => true,
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
            let w = menu.items.iter().map(|l| l.len() as u16).max().unwrap_or(0);
            let h = menu.items.len() as u16;
            Rect::new(menu.x, menu.y, w, h)
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
                    && m.selected + 1 < m.items.len()
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
    // 4. Active popup — scrollbar pre-pass (list-style popups only)
    if app.config.ui.scrollbars {
        let consumed = match app.popups.active.as_mut() {
            Some(ActivePopup::Template(p)) => handle_popup_scrollbar(
                p.last_scroll,
                &mut app.popups.scroll_drag,
                &mut p.selected,
                p.filtered_templates.len(),
                mouse,
            ),
            Some(ActivePopup::Theme(p)) => handle_popup_scrollbar(
                p.last_scroll,
                &mut app.popups.scroll_drag,
                &mut p.selected,
                p.themes.len(),
                mouse,
            ),
            Some(ActivePopup::Tag(p)) => handle_popup_scrollbar(
                p.last_scroll,
                &mut app.popups.scroll_drag,
                &mut p.all_tags_selected,
                p.all_tags.len(),
                mouse,
            ),
            Some(ActivePopup::FolderPicker(p)) => {
                // Scrollbar interaction implies user wants to browse the list,
                // so switch focus to Results for the list to track selection.
                p.focus = crate::app::FolderPickerFocus::Results;
                handle_popup_scrollbar(
                    p.last_scroll,
                    &mut app.popups.scroll_drag,
                    &mut p.selected,
                    p.filtered_folders.len(),
                    mouse,
                )
            }
            Some(ActivePopup::TrashView(p)) => handle_popup_scrollbar(
                p.last_scroll,
                &mut app.popups.scroll_drag,
                &mut p.selected,
                p.items.len(),
                mouse,
            ),
            Some(ActivePopup::Subnotes(p)) => handle_popup_scrollbar(
                p.last_scroll,
                &mut app.popups.scroll_drag,
                &mut p.selected,
                p.subnotes.len(),
                mouse,
            ),
            Some(ActivePopup::Search(p)) => {
                handle_search_popup_scrollbar(p, &mut app.popups.scroll_drag, mouse)
            }
            Some(_) => false,
            None => false,
        };
        if consumed {
            return true;
        }
    }

    // 5. Active popup (take + route through handle_mouse)
    if let Some(popup) = app.popups.active.take() {
        popup.handle_mouse(app, mouse, terminal_area)
    } else {
        false
    }
}
