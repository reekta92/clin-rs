use crate::keybinds::Keybinds;
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui_textarea::{CursorMove, Input, TextArea};
use std::borrow::Cow;

mod edit;
mod help;
mod list;
mod popup_mouse;
mod setup;

pub use popup_mouse::handle_global_popup_mouse;

pub use edit::{handle_edit_keys, handle_edit_mouse};
pub use help::handle_help_keys;
pub use list::{handle_list_keys, handle_list_mouse};
pub use setup::{handle_setup_keys, handle_setup_mouse};

pub fn handle_popup_text_input(
    key: KeyEvent,
    input: &mut TextArea<'static>,
    keybinds: &Keybinds,
) -> bool {
    if !apply_text_shortcuts(keybinds, input, key) {
        input.input(Input::from(key));
    }
    true
}

pub fn dismiss_popup_on_outside_click(
    app: &mut crate::app::App,
    mouse: &crossterm::event::MouseEvent,
    area: Rect,
) -> bool {
    if mouse.kind == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        && !contains_cell(area, mouse.column, mouse.row)
    {
        app.popups.active = None;
        return true;
    }
    false
}

pub fn hit_test_list_row(mouse_row: u16, list_top_y: u16, len: usize) -> Option<usize> {
    if len > 0 {
        let row = mouse_row.saturating_sub(list_top_y) as usize;
        Some(row.min(len.saturating_sub(1)))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelListAction {
    Up,
    Down,
    Confirm,
    Cancel,
    Other,
}

pub(crate) fn route_selection_list(
    key: &KeyEvent,
    kb: &Keybinds,
    selected: &mut usize,
    max: usize,
) -> SelListAction {
    if kb.matches_list(crate::keybinds::ListAction::Confirm, key) {
        SelListAction::Confirm
    } else if crate::events::is_cancel_popup(kb, key, false) {
        SelListAction::Cancel
    } else if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
        *selected = selected.saturating_sub(1);
        SelListAction::Up
    } else if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
        if *selected < max {
            *selected += 1;
        }
        SelListAction::Down
    } else {
        SelListAction::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextInputPopupAction {
    Cancel,
    Submit,
    Edited,
}

pub(crate) fn route_text_input_popup(
    key: &KeyEvent,
    kb: &Keybinds,
    input: &mut TextArea<'static>,
) -> TextInputPopupAction {
    if is_cancel_popup(kb, key, true) {
        TextInputPopupAction::Cancel
    } else if key.code == KeyCode::Enter {
        TextInputPopupAction::Submit
    } else {
        handle_popup_text_input(*key, input, kb);
        TextInputPopupAction::Edited
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

    // viewport top in SCREEN coordinates (0-based absolute document screen grid)
    let (scroll_row, scroll_col) = crate::ui::get_textarea_scroll(textarea);

    let target_row = (mouse_row.saturating_sub(body_inner.y) as usize).saturating_add(scroll_row);
    let target_col = (mouse_col.saturating_sub(body_inner.x) as usize).saturating_add(scroll_col);

    let cur = textarea.screen_cursor();
    // vertical: move by screen lines
    let drow = target_row as i64 - cur.row as i64;
    let vmove = if drow >= 0 {
        CursorMove::Down
    } else {
        CursorMove::Up
    };
    for _ in 0..drow.unsigned_abs() {
        textarea.move_cursor(vmove);
    }
    // horizontal: move by screen cells, clamped to the current screen line
    let cur = textarea.screen_cursor();
    let dcol = target_col as i64 - cur.col as i64;
    let hmove = if dcol >= 0 {
        CursorMove::Forward
    } else {
        CursorMove::Back
    };
    for _ in 0..dcol.unsigned_abs() {
        let before = textarea.screen_cursor().row;
        textarea.move_cursor(hmove);
        if textarea.screen_cursor().row != before {
            // stepped across a wrapped screen-line boundary: revert + stop
            textarea.move_cursor(if matches!(hmove, CursorMove::Forward) {
                CursorMove::Back
            } else {
                CursorMove::Forward
            });
            break;
        }
    }
}

use crate::editor::EditSidebar;
use crate::config::PreviewPosition;
use ratatui::layout::{Constraint, Direction, Layout};

/// Shared layout rects for the edit view.
/// All rects are computed from the `body_area` (the area between
/// the status bar and the footer/hint bar).
pub struct EditLayout {
    /// Inner title rect with 2-col, 1-row padding (for hit-testing).
    pub title: Rect,
    /// Editor body container (before gutter offset, for rendering).
    pub body: Rect,
    /// Preview pane rect (outer, for rendering the snapshot widget).
    pub preview: Option<Rect>,
    /// Sidebar pane rect.
    pub sidebar: Option<Rect>,
    /// Vertical splitter line rect between main pane and sidebar/preview.
    pub splitter: Option<Rect>,
}

/// Single source of truth for edit-view layout.
///
/// Splits `body_area` (the region between the header status bar and the
/// footer hint bar) into a 3‑row title area and an editor body, then
/// sub‑divides horizontally based on sidebar, preview, and fullscreen.
///
/// Preview ratio: `Percentage(50)` per user decision (the correct ratio;
/// the old inline render code used `Ratio(43,100)`, causing the hit‑test
/// and render to disagree).
pub fn compute_edit_layout(
    body_area: Rect,
    fullscreen: bool,
    preview_enabled: bool,
    sidebar: EditSidebar,
    preview_position: PreviewPosition,
) -> EditLayout {
    // Vertical split: title (3 rows) | editor (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(body_area);

    let title_outer = chunks[0];
    let editor_area = chunks[1];

    // Inner title rect for hit-testing (matches the padding on the widget)
    let title = Rect::new(
        title_outer.x + 2,
        title_outer.y + 1,
        title_outer.width.saturating_sub(4),
        title_outer.height.saturating_sub(2),
    );

    if fullscreen {
        // Fullscreen preview — editor is hidden
        return EditLayout {
            title,
            body: editor_area,
            preview: Some(editor_area),
            sidebar: None,
            splitter: None,
        };
    }
    if sidebar != EditSidebar::None {
        let (constraints, main_idx, sb_idx) = match preview_position {
            PreviewPosition::Left => (
                [
                    Constraint::Ratio(30, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(30, 100),
                ],
                0,
                2,
            ),
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(editor_area);
        return EditLayout {
            title,
            body: cols[main_idx],
            preview: None,
            sidebar: Some(cols[sb_idx]),
            splitter: Some(cols[1]),
        };
    }

    if preview_enabled {
        let (constraints, main_idx, p_idx) = match preview_position {
            // Preview on left, editor on right
            PreviewPosition::Left => (
                [
                    Constraint::Percentage(50),
                    Constraint::Length(1),
                    Constraint::Percentage(50),
                ],
                2,
                0,
            ),
            // Editor on left, preview on right
            PreviewPosition::Right => (
                [
                    Constraint::Percentage(50),
                    Constraint::Length(1),
                    Constraint::Percentage(50),
                ],
                0,
                2,
            ),
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(editor_area);
        return EditLayout {
            title,
            body: cols[main_idx],
            preview: Some(cols[p_idx]),
            sidebar: None,
            splitter: Some(cols[1]),
        };
    }

    // Plain editor — no sidebar, no preview, no fullscreen
    EditLayout {
        title,
        body: editor_area,
        preview: None,
        sidebar: None,
        splitter: None,
    }
}

pub fn edit_view_input_areas(
    area: Rect,
    md_preview: bool,
    line_count: usize,
    show_line_numbers: bool,
    sidebar: crate::editor::EditSidebar,
    sidebar_position: crate::config::PreviewPosition,
) -> (Rect, Rect, Option<Rect>) {
    // Outer vertical split (pad / title+body / footer) to find the body area.
    // This matches the layout used by draw_edit_view.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    // The body area spans the title row (chunks[1]) and the body row (chunks[2]).
    let body_area = Rect::new(
        area.x,
        chunks[1].y,
        area.width,
        chunks[1].height + chunks[2].height,
    );

    let layout = compute_edit_layout(body_area, false, md_preview, sidebar, sidebar_position);

    // Apply gutter offset to the body rect for mouse hit-testing
    let gutter_width = if show_line_numbers {
        (line_count.max(1).to_string().len() as u16) + 2
    } else {
        0
    };

    let body_inner = Rect::new(
        layout.body.x + gutter_width,
        layout.body.y,
        layout.body.width.saturating_sub(gutter_width + 2),
        layout.body.height,
    );

    (layout.title, body_inner, layout.sidebar)
}

pub fn edit_view_md_preview_area(area: Rect, sidebar: crate::editor::EditSidebar, preview_position: crate::config::PreviewPosition) -> Option<Rect> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let body_area = Rect::new(
        area.x,
        chunks[1].y,
        area.width,
        chunks[1].height + chunks[2].height,
    );

    let layout = compute_edit_layout(body_area, false, true, sidebar, preview_position);

    layout.preview.map(|r| {
        Rect::new(
            r.x + 2,
            r.y + 1,
            r.width.saturating_sub(4),
            r.height.saturating_sub(2),
        )
    })
}
pub fn contains_cell(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
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
    title.set_selection_style(Style::default().fg(highlight_fg).bg(highlight_bg));
    title
}

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

use crate::app::App;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

/// Check if the key event should cancel/close a popup.
/// Returns `true` if the key matches `ListAction::Cancel`, or if it matches
/// `ListAction::Quit` and `!has_text_input` (to avoid stealing printable keys).
///
/// When `has_text_input` is true, Cancel matches are filtered to exclude
/// bare (unmodified) `Char` keypresses, so keys like `n` type into the text
/// field instead of closing the popup. Modifier combos like `Ctrl+N` and
/// non-printable keys like `Esc` still cancel.
pub fn is_cancel_popup(
    keybinds: &crate::keybinds::Keybinds,
    key: &crossterm::event::KeyEvent,
    has_text_input: bool,
) -> bool {
    let cancel = keybinds.matches_list(crate::keybinds::ListAction::Cancel, key);
    let cancel_triggered = if has_text_input && cancel {
        // In text-input mode, only non-printable keys and modifier combos cancel.
        // Bare Char (letter, digit, symbol) goes to the text input.
        let bare_char = matches!(key.code, KeyCode::Char(_))
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::META);
        !bare_char
    } else {
        cancel
    };
    cancel_triggered
        || (!has_text_input && keybinds.matches_list(crate::keybinds::ListAction::Quit, key))
}

// True for a bare (no-modifier) q or Esc — the universal back/quit keys.
/// Used by the override-proof intercept in each view handler. Callers that
/// must exclude q (text entry: Edit) check Esc inline instead.
pub fn is_universal_quit_key(key: &crossterm::event::KeyEvent) -> bool {
    key.modifiers == crossterm::event::KeyModifiers::NONE
        && matches!(
            key.code,
            crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc
        )
}

/// Handle global popups (tag, search, create_note, folder, goals, import,
/// trash_view, confirm, folder_picker, template, note_rename, theme, sort,
/// create_format) and command palette input.
/// Returns `true` if the event was consumed, `false` otherwise.
///
/// Preserves the historical precedence: group-A popups (create_note, import,
/// folder, tag, goals, note_rename, search) shadow the standalone confirm
/// overlay; tag additionally handles a layered confirm inline. The standalone
/// confirm check therefore runs only when no group-A popup is active, and
/// group-B popups (trash, folder_picker, template, theme, …) follow it.
pub fn handle_global_popups_and_palette(
    app: &mut App,
    event: crossterm::event::Event,
    _terminal_area: Rect,
) -> bool {
    let crossterm::event::Event::Key(key) = event else {
        return false;
    };
    if key.kind != crossterm::event::KeyEventKind::Press {
        return false;
    }

    // Command palette
    if let Some(mut palette) = app.command_palette.take() {
        if palette.handle_input(key, app) {
            if key.code == KeyCode::Enter
                && let Some(selected_idx) = palette.state.selected()
                && let Some(item) = palette.items.get(selected_idx)
            {
                let action_id = item.id.clone();

                let note_id = palette.context_note_id.clone();
                if let Err(e) = crate::actions::execute_action(&action_id, app, note_id.as_deref())
                {
                    app.set_temporary_status(&format!("Action failed: {e}"));
                }
            }
            return true;
        }
        app.command_palette = Some(palette);
        return true;
    }

    // Info popup (display-only; Enter/Esc closes, any other key traps)
    if matches!(app.popups.active, Some(crate::popups::ActivePopup::Info(_))) {
        if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
            app.popups.active = None;
        }
        return true;
    }

    // Group A: popups that shadow the standalone confirm check.
    let group_a = matches!(
        app.popups.active,
        Some(crate::popups::ActivePopup::CreateNote(..))
            | Some(crate::popups::ActivePopup::Import(_))
            | Some(crate::popups::ActivePopup::Folder(_))
            | Some(crate::popups::ActivePopup::Tag(_))
            | Some(crate::popups::ActivePopup::Goals(_))
            | Some(crate::popups::ActivePopup::NoteRename(_))
            | Some(crate::popups::ActivePopup::Search(_))
    );
    if group_a {
        let popup = app.popups.active.take().expect("value is present");
        return popup.handle_key(key, app);
    }

    // Standalone confirm overlay (layers over group-B popups or nothing).
    if app.popups.confirm.is_some() {
        app.seq_matcher.clear();
        if key.code == KeyCode::Left || key.code == KeyCode::Char('h') {
            app.confirm_popup_select_confirm();
        } else if key.code == KeyCode::Right || key.code == KeyCode::Char('l') {
            app.confirm_popup_select_cancel();
        } else if key.code == KeyCode::Tab {
            app.confirm_popup_toggle_button();
        } else if key.code == KeyCode::Enter {
            app.confirm_popup_activate();
        } else if crate::events::is_cancel_popup(&app.keybinds, &key, false) {
            app.cancel_confirm();
        } else if app
            .keybinds
            .matches_list(crate::keybinds::ListAction::Confirm, &key)
        {
            app.confirm_action();
        } else if app
            .keybinds
            .matches_list(crate::keybinds::ListAction::Cancel, &key)
        {
            app.cancel_confirm();
        }
        return true;
    }

    // Group B: the remaining popups (and ContextMenu, which falls through).
    if let Some(popup) = app.popups.active.take() {
        return popup.handle_key(key, app);
    }
    false
}

impl crate::popups::ActivePopup {
    /// Handle one key. `self` is the popup taken out of `app.popups.active`;
    /// this method re-inserts it (`app.popups.active = Some(...)`) whenever it
    /// should remain open, and drops it to close. Returns `true` if consumed.
    fn handle_key(self, key: KeyEvent, app: &mut App) -> bool {
        use crate::popups::ActivePopup;
        match self {
            ActivePopup::CreateNote(mut popup, format) => {
                match route_text_input_popup(&key, &app.keybinds, &mut popup.input) {
                    TextInputPopupAction::Cancel => {}
                    TextInputPopupAction::Submit => {
                        app.popups.active = Some(ActivePopup::CreateNote(popup, format));
                        app.confirm_create_note();
                    }
                    TextInputPopupAction::Edited => {
                        app.popups.active = Some(ActivePopup::CreateNote(popup, format));
                    }
                }
                true
            }
            ActivePopup::Import(mut popup) => {
                match route_text_input_popup(&key, &app.keybinds, &mut popup.input) {
                    TextInputPopupAction::Cancel => {}
                    TextInputPopupAction::Submit => {
                        app.popups.active = Some(ActivePopup::Import(popup));
                        app.confirm_import();
                    }
                    TextInputPopupAction::Edited => {
                        app.popups.active = Some(ActivePopup::Import(popup));
                    }
                }
                true
            }
            ActivePopup::Folder(mut popup) => {
                match route_text_input_popup(&key, &app.keybinds, &mut popup.input) {
                    TextInputPopupAction::Cancel => {}
                    TextInputPopupAction::Submit => {
                        app.popups.active = Some(ActivePopup::Folder(popup));
                        app.confirm_folder_popup();
                    }
                    TextInputPopupAction::Edited => {
                        app.popups.active = Some(ActivePopup::Folder(popup));
                    }
                }
                true
            }
            ActivePopup::Tag(mut popup) => {
                if app.popups.confirm.is_some() {
                    app.popups.active = Some(ActivePopup::Tag(popup));
                    if key.code == KeyCode::Left || key.code == KeyCode::Char('h') {
                        app.confirm_popup_select_confirm();
                    } else if key.code == KeyCode::Right || key.code == KeyCode::Char('l') {
                        app.confirm_popup_select_cancel();
                    } else if key.code == KeyCode::Tab {
                        app.confirm_popup_toggle_button();
                    } else if key.code == KeyCode::Enter
                        || key.code == KeyCode::Char('y')
                        || key.code == KeyCode::Char('Y')
                    {
                        app.confirm_popup_activate();
                    } else if key.code == KeyCode::Char('n')
                        || key.code == KeyCode::Char('N')
                        || crate::events::is_cancel_popup(&app.keybinds, &key, false)
                    {
                        app.cancel_confirm();
                    }
                    return true;
                }

                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    let tag_text = popup.input.lines().join("");
                    let tag = tag_text
                        .split(',')
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    if let Some(tag) = tag {
                        app.list.tag_to_assign = Some(tag);
                        app.list.list_mode = crate::list_view::ListMode::Select;
                        app.list.selected_indices.clear();
                        app.list.selected_indices.insert(app.list.visual_index);
                        app.set_temporary_status_static(
                            "TAG MODE: Select notes to apply tag, Enter to confirm, Esc to cancel",
                        );
                    } else {
                        app.set_temporary_status_static("Enter a tag name first");
                    }
                    return true;
                }

                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    return true;
                }
                match key.code {
                    KeyCode::Tab => {
                        if popup.focus == crate::popups::TagPopupFocus::Input {
                            if popup.suggestions.is_empty() {
                                popup.focus = crate::popups::TagPopupFocus::AllTagsList;
                            } else {
                                app.popups.active = Some(ActivePopup::Tag(popup));
                                app.accept_tag_suggestion();
                                return true;
                            }
                        } else {
                            popup.focus = crate::popups::TagPopupFocus::Input;
                        }
                        app.popups.active = Some(ActivePopup::Tag(popup));
                    }
                    KeyCode::BackTab => {
                        popup.focus = match popup.focus {
                            crate::popups::TagPopupFocus::Input => {
                                crate::popups::TagPopupFocus::AllTagsList
                            }
                            crate::popups::TagPopupFocus::AllTagsList => {
                                crate::popups::TagPopupFocus::Input
                            }
                        };
                        app.popups.active = Some(ActivePopup::Tag(popup));
                    }
                    _ => match popup.focus {
                        crate::popups::TagPopupFocus::Input => {
                            if key.code == KeyCode::Enter {
                                app.popups.active = Some(ActivePopup::Tag(popup));
                                app.confirm_manage_tags();
                            } else if key.code == KeyCode::Char('D')
                                && key.modifiers.contains(KeyModifiers::SHIFT)
                            {
                                if let Some(tag) =
                                    popup.suggestions.get(popup.suggestion_index).cloned()
                                {
                                    app.popups.active = Some(ActivePopup::Tag(popup));
                                    app.begin_delete_tag_with_name(tag);
                                }
                            } else {
                                if !crate::text_edit::apply_text_shortcuts(
                                    &app.keybinds,
                                    &mut popup.input,
                                    key,
                                ) {
                                    popup.input.input(ratatui_textarea::Input::from(key));
                                }
                                app.popups.active = Some(ActivePopup::Tag(popup));
                                app.update_tag_suggestions();
                            }
                        }
                        crate::popups::TagPopupFocus::AllTagsList => match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                popup.all_tags_selected = popup.all_tags_selected.saturating_sub(1);
                                app.popups.active = Some(ActivePopup::Tag(popup));
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if popup.all_tags_selected + 1 < popup.all_tags.len() {
                                    popup.all_tags_selected += 1;
                                }
                                app.popups.active = Some(ActivePopup::Tag(popup));
                            }
                            KeyCode::Char('d') | KeyCode::Delete => {
                                if let Some(tag) =
                                    popup.all_tags.get(popup.all_tags_selected).cloned()
                                {
                                    app.popups.active = Some(ActivePopup::Tag(popup));
                                    app.begin_delete_tag_with_name(tag);
                                }
                            }
                            _ => {
                                app.popups.active = Some(ActivePopup::Tag(popup));
                            }
                        },
                    },
                }
                true
            }
            ActivePopup::Goals(mut popup) => {
                match route_text_input_popup(&key, &app.keybinds, &mut popup.input) {
                    TextInputPopupAction::Cancel => {}
                    TextInputPopupAction::Submit => {
                        app.popups.active = Some(ActivePopup::Goals(popup));
                        app.confirm_goals_popup();
                    }
                    TextInputPopupAction::Edited => {
                        app.popups.active = Some(ActivePopup::Goals(popup));
                    }
                }
                true
            }
            ActivePopup::Subnotes(mut popup) => {
                let now_unix_secs = || {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                };

                let is_alt_n = (key.code == KeyCode::Char('n') || key.code == KeyCode::Char('N'))
                    && key.modifiers.contains(KeyModifiers::ALT);

                let is_ctrl_e = (key.code == KeyCode::Char('e') || key.code == KeyCode::Char('E'))
                    && key.modifiers.contains(KeyModifiers::CONTROL);

                if is_ctrl_e {
                    app.popups.active = Some(ActivePopup::Subnotes(popup));
                    app.open_subnote_in_external_editor();
                    return true;
                }

                if is_alt_n {
                    let cur_idx = popup.selected;
                    if !popup.subnotes.is_empty() && cur_idx < popup.subnotes.len() {
                        let new_title = popup.title_input.lines().join("");
                        let new_content = popup.content_input.lines().join("\n");
                        if popup.subnotes[cur_idx].title != new_title
                            || popup.subnotes[cur_idx].content != new_content
                        {
                            popup.subnotes[cur_idx].title = new_title;
                            popup.subnotes[cur_idx].content = new_content;
                            popup.subnotes[cur_idx].updated_at = now_unix_secs();
                            popup.is_dirty = true;
                        }
                    }

                    let new_subnote = crate::storage::SubNote {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: "New Note".to_string(),
                        content: "".to_string(),
                        updated_at: now_unix_secs(),
                    };
                    popup.subnotes.push(new_subnote);
                    popup.selected = popup.subnotes.len().saturating_sub(1);
                    popup.title_input = crate::ui::make_popup_textarea(&app.app_theme, "");
                    popup.title_input.insert_str("New Note");
                    popup.content_input = crate::ui::make_popup_textarea(&app.app_theme, "");
                    popup.is_dirty = true;
                    popup.focus = crate::popups::SubnotesFocus::EditTitle;
                    app.popups.active = Some(ActivePopup::Subnotes(popup));
                    return true;
                }

                match popup.focus {
                    crate::popups::SubnotesFocus::List => {
                        if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
                            if !popup.subnotes.is_empty() {
                                let cur_idx = popup.selected;
                                if cur_idx < popup.subnotes.len() {
                                    let new_title = popup.title_input.lines().join("");
                                    let new_content = popup.content_input.lines().join("\n");
                                    if popup.subnotes[cur_idx].title != new_title
                                        || popup.subnotes[cur_idx].content != new_content
                                    {
                                        popup.subnotes[cur_idx].title = new_title;
                                        popup.subnotes[cur_idx].content = new_content;
                                        popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                        popup.is_dirty = true;
                                    }
                                }
                                popup.selected = popup.selected.saturating_sub(1);
                                popup.title_input =
                                    crate::ui::make_popup_textarea(&app.app_theme, "");
                                popup
                                    .title_input
                                    .insert_str(&popup.subnotes[popup.selected].title);
                                popup.content_input =
                                    crate::ui::make_popup_textarea(&app.app_theme, "");
                                popup
                                    .content_input
                                    .insert_str(&popup.subnotes[popup.selected].content);
                            }
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
                            if !popup.subnotes.is_empty() {
                                let cur_idx = popup.selected;
                                if cur_idx < popup.subnotes.len() {
                                    let new_title = popup.title_input.lines().join("");
                                    let new_content = popup.content_input.lines().join("\n");
                                    if popup.subnotes[cur_idx].title != new_title
                                        || popup.subnotes[cur_idx].content != new_content
                                    {
                                        popup.subnotes[cur_idx].title = new_title;
                                        popup.subnotes[cur_idx].content = new_content;
                                        popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                        popup.is_dirty = true;
                                    }
                                }
                                popup.selected = popup
                                    .selected
                                    .saturating_add(1)
                                    .min(popup.subnotes.len().saturating_sub(1));
                                popup.title_input =
                                    crate::ui::make_popup_textarea(&app.app_theme, "");
                                popup
                                    .title_input
                                    .insert_str(&popup.subnotes[popup.selected].title);
                                popup.content_input =
                                    crate::ui::make_popup_textarea(&app.app_theme, "");
                                popup
                                    .content_input
                                    .insert_str(&popup.subnotes[popup.selected].content);
                            }
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Char('n') {
                            let cur_idx = popup.selected;
                            if !popup.subnotes.is_empty() && cur_idx < popup.subnotes.len() {
                                let new_title = popup.title_input.lines().join("");
                                let new_content = popup.content_input.lines().join("\n");
                                if popup.subnotes[cur_idx].title != new_title
                                    || popup.subnotes[cur_idx].content != new_content
                                {
                                    popup.subnotes[cur_idx].title = new_title;
                                    popup.subnotes[cur_idx].content = new_content;
                                    popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                    popup.is_dirty = true;
                                }
                            }

                            let new_subnote = crate::storage::SubNote {
                                id: uuid::Uuid::new_v4().to_string(),
                                title: "New Note".to_string(),
                                content: "".to_string(),
                                updated_at: now_unix_secs(),
                            };
                            popup.subnotes.push(new_subnote);
                            popup.selected = popup.subnotes.len().saturating_sub(1);
                            popup.title_input = crate::ui::make_popup_textarea(&app.app_theme, "");
                            popup.title_input.insert_str("New Note");
                            popup.content_input =
                                crate::ui::make_popup_textarea(&app.app_theme, "");
                            popup.is_dirty = true;
                            popup.focus = crate::popups::SubnotesFocus::EditTitle;
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Char('d') || key.code == KeyCode::Delete {
                            if !popup.subnotes.is_empty() {
                                popup.subnotes.remove(popup.selected);
                                popup.is_dirty = true;
                                if popup.selected >= popup.subnotes.len() {
                                    popup.selected = popup.subnotes.len().saturating_sub(1);
                                }
                                popup.title_input =
                                    crate::ui::make_popup_textarea(&app.app_theme, "");
                                popup.content_input =
                                    crate::ui::make_popup_textarea(&app.app_theme, "");
                                if !popup.subnotes.is_empty() {
                                    popup
                                        .title_input
                                        .insert_str(&popup.subnotes[popup.selected].title);
                                    popup
                                        .content_input
                                        .insert_str(&popup.subnotes[popup.selected].content);
                                }
                            }
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Enter || key.code == KeyCode::Char('l') {
                            if !popup.subnotes.is_empty() {
                                popup.focus = crate::popups::SubnotesFocus::EditTitle;
                            }
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if crate::events::is_cancel_popup(&app.keybinds, &key, false) {
                            let cur_idx = popup.selected;
                            if !popup.subnotes.is_empty() && cur_idx < popup.subnotes.len() {
                                let new_title = popup.title_input.lines().join("");
                                let new_content = popup.content_input.lines().join("\n");
                                if popup.subnotes[cur_idx].title != new_title
                                    || popup.subnotes[cur_idx].content != new_content
                                {
                                    popup.subnotes[cur_idx].title = new_title;
                                    popup.subnotes[cur_idx].content = new_content;
                                    popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                    popup.is_dirty = true;
                                }
                            }
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                            app.close_subnotes_popup();
                        } else {
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        }
                    }
                    crate::popups::SubnotesFocus::EditTitle => {
                        if key.code == KeyCode::Tab || key.code == KeyCode::Enter {
                            let cur_idx = popup.selected;
                            if cur_idx < popup.subnotes.len() {
                                let new_title = popup.title_input.lines().join("");
                                if popup.subnotes[cur_idx].title != new_title {
                                    popup.subnotes[cur_idx].title = new_title;
                                    popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                    popup.is_dirty = true;
                                }
                            }
                            popup.focus = crate::popups::SubnotesFocus::EditContent;
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Esc {
                            let cur_idx = popup.selected;
                            if cur_idx < popup.subnotes.len() {
                                let new_title = popup.title_input.lines().join("");
                                if popup.subnotes[cur_idx].title != new_title {
                                    popup.subnotes[cur_idx].title = new_title;
                                    popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                    popup.is_dirty = true;
                                }
                            }
                            popup.focus = crate::popups::SubnotesFocus::List;
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else {
                            crate::events::handle_popup_text_input(
                                key,
                                &mut popup.title_input,
                                &app.keybinds,
                            );
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        }
                    }
                    crate::popups::SubnotesFocus::EditContent => {
                        if key.code == KeyCode::BackTab {
                            let cur_idx = popup.selected;
                            if cur_idx < popup.subnotes.len() {
                                let new_content = popup.content_input.lines().join("\n");
                                if popup.subnotes[cur_idx].content != new_content {
                                    popup.subnotes[cur_idx].content = new_content;
                                    popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                    popup.is_dirty = true;
                                }
                            }
                            popup.focus = crate::popups::SubnotesFocus::EditTitle;
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Esc {
                            let cur_idx = popup.selected;
                            if cur_idx < popup.subnotes.len() {
                                let new_content = popup.content_input.lines().join("\n");
                                if popup.subnotes[cur_idx].content != new_content {
                                    popup.subnotes[cur_idx].content = new_content;
                                    popup.subnotes[cur_idx].updated_at = now_unix_secs();
                                    popup.is_dirty = true;
                                }
                            }
                            popup.focus = crate::popups::SubnotesFocus::List;
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else {
                            crate::events::handle_popup_text_input(
                                key,
                                &mut popup.content_input,
                                &app.keybinds,
                            );
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        }
                    }
                }
                true
            }
            ActivePopup::NoteRename(mut popup) => {
                match route_text_input_popup(&key, &app.keybinds, &mut popup.input) {
                    TextInputPopupAction::Cancel => {}
                    TextInputPopupAction::Submit => {
                        app.popups.active = Some(ActivePopup::NoteRename(popup));
                        app.confirm_rename_note();
                    }
                    TextInputPopupAction::Edited => {
                        app.popups.active = Some(ActivePopup::NoteRename(popup));
                    }
                }
                true
            }
            ActivePopup::Search(mut popup) => {
                let has_title = !popup.title_results.is_empty();
                let has_grep = !popup.grep_results.is_empty();
                let has_results = has_title || has_grep;


                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    app.popups.active = Some(ActivePopup::Search(popup));
                    app.cancel_search();
                    return true;
                }
                let reinsert = |p: crate::popups::SearchPopup| ActivePopup::Search(p);
                match key.code {
                    KeyCode::Tab | KeyCode::BackTab => {
                        popup.focus = match popup.focus {
                            crate::popups::SearchFocus::Input if has_results => {
                                crate::popups::SearchFocus::Results
                            }
                            _ => crate::popups::SearchFocus::Input,
                        };
                        app.popups.active = Some(reinsert(popup));
                    }
                    KeyCode::Enter => {
                        if popup.focus == crate::popups::SearchFocus::Results && has_results {
                            app.popups.active = Some(reinsert(popup));
                            app.jump_to_selected_result();
                            app.confirm_search();
                        } else {
                            app.popups.active = Some(reinsert(popup));
                            app.confirm_search();
                        }
                    }
                    KeyCode::Char('l') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep
                            && popup
                                .grep_is_header
                                .get(popup.grep_selected)
                                .copied()
                                .unwrap_or(false)
                        {
                            if popup.grep_expanded.contains(&popup.grep_selected) {
                                popup.grep_expanded.remove(&popup.grep_selected);
                            } else {
                                popup.grep_expanded.insert(popup.grep_selected);
                            }
                            app.popups.active = Some(reinsert(popup));
                        } else if has_results {
                            app.popups.active = Some(reinsert(popup));
                            app.jump_to_selected_result();
                            app.confirm_search();
                        } else {
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep {
                            popup.grep_selected = crate::popups::grep_prev_visible(
                                &popup.grep_is_header, &popup.grep_expanded, popup.grep_selected);
                            app.popups.active = Some(reinsert(popup));
                        } else if has_title {
                            popup.title_selected = popup.title_selected.saturating_sub(1);
                            app.popups.active = Some(reinsert(popup));
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep {
                            popup.grep_selected = crate::popups::grep_next_visible(
                                &popup.grep_is_header, &popup.grep_expanded, popup.grep_selected);
                            app.popups.active = Some(reinsert(popup));
                        } else if has_title {
                            if popup.title_selected + 1 < popup.title_results.len() {
                                popup.title_selected += 1;
                            }
                            app.popups.active = Some(reinsert(popup));
                        }
                    }
                    KeyCode::Right | KeyCode::Char(' ') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep
                            && popup
                                .grep_is_header
                                .get(popup.grep_selected)
                                .copied()
                                .unwrap_or(false)
                        {
                            popup.grep_expanded.insert(popup.grep_selected);
                            app.popups.active = Some(reinsert(popup));
                        } else {
                            popup.focus = crate::popups::SearchFocus::Input;
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        }
                    }
                    KeyCode::Left => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep
                            && popup
                                .grep_is_header
                                .get(popup.grep_selected)
                                .copied()
                                .unwrap_or(false)
                        {
                            popup.grep_expanded.remove(&popup.grep_selected);
                            app.popups.active = Some(reinsert(popup));
                        } else {
                            popup.focus = crate::popups::SearchFocus::Input;
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        }
                    }
                    _ => {
                        popup.focus = crate::popups::SearchFocus::Input;
                        if !crate::text_edit::apply_text_shortcuts(
                            &app.keybinds,
                            &mut popup.input,
                            key,
                        ) {
                            popup.input.input(ratatui_textarea::Input::from(key));
                        }
                        app.popups.active = Some(reinsert(popup));
                        app.update_search();
                    }
                }
                true
            }
            ActivePopup::TrashView(mut trash) => {
                app.seq_matcher.clear();
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
                        app.popups.active = Some(ActivePopup::TrashView(trash));
                        app.restore_from_trash();
                        return true;
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        app.popups.active = Some(ActivePopup::TrashView(trash));
                        app.begin_delete_from_trash();
                        return true;
                    }
                    KeyCode::Char('E') => {
                        app.popups.active = Some(ActivePopup::TrashView(trash));
                        app.begin_empty_trash();
                        return true;
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.close_trash_view();
                        return true;
                    }
                    _ => {}
                }
                app.popups.active = Some(ActivePopup::TrashView(trash));
                true
            }
            ActivePopup::FolderPicker(mut picker) => {
                app.seq_matcher.clear();
                if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
                    return true;
                }
                let reinsert = |p: crate::popups::FolderPicker| ActivePopup::FolderPicker(p);
                match key.code {
                    KeyCode::Tab => {
                        picker.focus = match picker.focus {
                            crate::app::FolderPickerFocus::Search => {
                                crate::app::FolderPickerFocus::Results
                            }
                            crate::app::FolderPickerFocus::Results => {
                                crate::app::FolderPickerFocus::Search
                            }
                        };
                        app.popups.active = Some(reinsert(picker));
                    }
                    _ => match picker.focus {
                        crate::app::FolderPickerFocus::Results => match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                picker.selected = picker.selected.saturating_sub(1);
                                app.popups.active = Some(reinsert(picker));
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if picker.selected + 1 < picker.filtered_folders.len() {
                                    picker.selected += 1;
                                }
                                app.popups.active = Some(reinsert(picker));
                            }
                            KeyCode::Enter | KeyCode::Char('l') => {
                                app.popups.active = Some(reinsert(picker));
                                app.confirm_move();
                            }
                            _ => {
                                app.popups.active = Some(reinsert(picker));
                            }
                        },
                        crate::app::FolderPickerFocus::Search => {
                            let old_query = picker.input.lines().join("");
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut picker.input,
                                key,
                            ) {
                                picker.input.input(ratatui_textarea::Input::from(key));
                            }
                            let new_query = picker.input.lines().join("");
                            if old_query != new_query {
                                app.popups.active = Some(reinsert(picker));
                                app.update_folder_picker_filter();
                            } else if key.code == KeyCode::Enter {
                                picker.focus = crate::app::FolderPickerFocus::Results;
                                app.popups.active = Some(reinsert(picker));
                            } else {
                                app.popups.active = Some(reinsert(picker));
                            }
                        }
                    },
                }
                true
            }
            ActivePopup::Template(mut popup) => {
                app.seq_matcher.clear();
                let reinsert = |p: crate::popups::TemplatePopup| ActivePopup::Template(p);
                match key.code {
                    KeyCode::Tab | KeyCode::BackTab => {
                        popup.focus = match popup.focus {
                            crate::popups::TemplatePopupFocus::Search => {
                                crate::popups::TemplatePopupFocus::Results
                            }
                            crate::popups::TemplatePopupFocus::Results => {
                                crate::popups::TemplatePopupFocus::Search
                            }
                        };
                        app.popups.active = Some(reinsert(popup));
                    }
                    KeyCode::Char('?') => {
                        if popup.focus == crate::popups::TemplatePopupFocus::Results {
                            app.popups.active = Some(reinsert(popup));
                            app.open_help_page_with_tab(crate::app::HelpTab::Templates);
                        } else {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_template_popup_filter();
                        }
                    }
                    KeyCode::Char('n') => {
                        if popup.focus == crate::popups::TemplatePopupFocus::Results {
                            app.popups.active = Some(reinsert(popup));
                            app.create_template_from_popup();
                        } else {
                            if !crate::text_edit::apply_text_shortcuts(
                                &app.keybinds,
                                &mut popup.input,
                                key,
                            ) {
                                popup.input.input(ratatui_textarea::Input::from(key));
                            }
                            app.popups.active = Some(reinsert(popup));
                            app.update_template_popup_filter();
                        }
                    }
                    _ if crate::events::is_cancel_popup(&app.keybinds, &key, true) => {
                        app.close_template_popup();
                    }
                    _ => match popup.focus {
                        crate::popups::TemplatePopupFocus::Results => match key.code {
                            _ if app
                                .keybinds
                                .matches_list(crate::keybinds::ListAction::MoveUp, &key) =>
                            {
                                popup.selected = popup.selected.saturating_sub(1);
                                app.popups.active = Some(reinsert(popup));
                            }
                            _ if app
                                .keybinds
                                .matches_list(crate::keybinds::ListAction::MoveDown, &key) =>
                            {
                                if popup.selected + 1 < popup.filtered_templates.len() {
                                    popup.selected += 1;
                                }
                                app.popups.active = Some(reinsert(popup));
                            }
                            _ if app
                                .keybinds
                                .matches_list(crate::keybinds::ListAction::Confirm, &key)
                                || app
                                    .keybinds
                                    .matches_list(crate::keybinds::ListAction::Open, &key) =>
                            {
                                app.popups.active = Some(reinsert(popup));
                                app.select_template();
                            }
                            KeyCode::Char(' ') => {
                                app.popups.active = Some(reinsert(popup));
                                app.edit_selected_template_from_popup();
                            }
                            KeyCode::Char('d') => {
                                app.popups.active = Some(reinsert(popup));
                                app.begin_delete_selected_template_from_popup();
                            }
                            KeyCode::Char('h') => {
                                app.close_template_popup();
                            }
                            _ => {
                                app.popups.active = Some(reinsert(popup));
                            }
                        },
                        crate::popups::TemplatePopupFocus::Search => match key.code {
                            _ if key.code == KeyCode::Enter => {
                                popup.focus = crate::popups::TemplatePopupFocus::Results;
                                app.popups.active = Some(reinsert(popup));
                            }
                            _ => {
                                if !crate::text_edit::apply_text_shortcuts(
                                    &app.keybinds,
                                    &mut popup.input,
                                    key,
                                ) {
                                    popup.input.input(ratatui_textarea::Input::from(key));
                                }
                                app.popups.active = Some(reinsert(popup));
                                app.update_template_popup_filter();
                            }
                        },
                    },
                }
                true
            }
            ActivePopup::Theme(mut popup) => {
                app.seq_matcher.clear();
                let reinsert = |p: crate::popups::ThemePopup| ActivePopup::Theme(p);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        match popup.focus {
                            crate::app::ThemePopupFocus::ThemeList => {
                                popup.selected = popup.selected.saturating_sub(1);
                                app.popups.active = Some(reinsert(popup));
                                app.select_theme();
                                return true;
                            }
                            crate::app::ThemePopupFocus::GeneralBg => {
                                popup.focus = crate::app::ThemePopupFocus::ThemeList;
                                popup.selected = popup.themes.len().saturating_sub(1);
                            }
                            crate::app::ThemePopupFocus::GraphBg => {
                                popup.focus = crate::app::ThemePopupFocus::GeneralBg;
                            }
                        }
                        app.popups.active = Some(reinsert(popup));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        match popup.focus {
                            crate::app::ThemePopupFocus::ThemeList => {
                                if popup.selected + 1 < popup.themes.len() {
                                    popup.selected += 1;
                                    app.popups.active = Some(reinsert(popup));
                                    app.select_theme();
                                    return true;
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
                        app.popups.active = Some(reinsert(popup));
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
                        app.popups.active = Some(reinsert(popup));
                    }
                    _ if app
                        .keybinds
                        .matches_list(crate::keybinds::ListAction::Confirm, &key) =>
                    {
                        let is_list = matches!(popup.focus, crate::app::ThemePopupFocus::ThemeList);
                        app.popups.active = Some(reinsert(popup));
                        app.select_theme();
                        if is_list {
                            app.close_theme_popup();
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Char(' ') => {
                        app.popups.active = Some(reinsert(popup));
                        app.select_theme();
                    }
                    _ if crate::events::is_cancel_popup(&app.keybinds, &key, false) => {
                        app.close_theme_popup();
                    }
                    _ => {
                        app.popups.active = Some(reinsert(popup));
                    }
                }
                true
            }
            ActivePopup::IconMode(mut popup) => {
                app.seq_matcher.clear();
                match route_selection_list(&key, &app.keybinds, &mut popup.selected, 2) {
                    SelListAction::Confirm => {
                        app.popups.active = Some(ActivePopup::IconMode(popup));
                        app.select_icon_mode();
                    }
                    SelListAction::Cancel => {
                        app.close_icon_mode_popup();
                    }
                    _ => {
                        app.popups.active = Some(ActivePopup::IconMode(popup));
                    }
                }
                true
            }
            ActivePopup::HintBarStyle(mut popup) => {
                app.seq_matcher.clear();
                match route_selection_list(&key, &app.keybinds, &mut popup.selected, 3) {
                    SelListAction::Up | SelListAction::Down => {
                        app.popups.active = Some(ActivePopup::HintBarStyle(popup));
                        app.select_hint_bar_style();
                    }
                    SelListAction::Confirm => {
                        app.popups.active = Some(ActivePopup::HintBarStyle(popup));
                        app.select_hint_bar_style();
                        app.close_hint_bar_style_popup();
                    }
                    SelListAction::Cancel => {
                        app.close_hint_bar_style_popup();
                    }
                    _ => {
                        app.popups.active = Some(ActivePopup::HintBarStyle(popup));
                    }
                }
                true
            }
            ActivePopup::KeybindPreset(mut popup) => {
                app.seq_matcher.clear();
                match route_selection_list(&key, &app.keybinds, &mut popup.selected, 3) {
                    SelListAction::Up | SelListAction::Down => {
                        app.popups.active = Some(ActivePopup::KeybindPreset(popup));
                        app.select_keybind_preset();
                    }
                    SelListAction::Confirm => {
                        app.popups.active = Some(ActivePopup::KeybindPreset(popup));
                        app.select_keybind_preset();
                        app.close_keybind_preset_popup();
                    }
                    SelListAction::Cancel => {
                        app.close_keybind_preset_popup();
                    }
                    _ => {
                        app.popups.active = Some(ActivePopup::KeybindPreset(popup));
                    }
                }
                true
            }
            ActivePopup::Sort(mut popup) => {
                app.seq_matcher.clear();
                match route_selection_list(&key, &app.keybinds, &mut popup.selected, 3) {
                    SelListAction::Confirm => {
                        app.popups.active = Some(ActivePopup::Sort(popup));
                        app.select_sort();
                    }
                    SelListAction::Cancel => {
                        app.close_sort_popup();
                    }
                    _ => {
                        app.popups.active = Some(ActivePopup::Sort(popup));
                    }
                }
                true
            }
            ActivePopup::CreateFormat(mut popup) => {
                app.seq_matcher.clear();
                match route_selection_list(&key, &app.keybinds, &mut popup.selected, 3) {
                    SelListAction::Confirm => {
                        app.popups.active = Some(ActivePopup::CreateFormat(popup));
                        app.confirm_create_format();
                    }
                    SelListAction::Cancel => {
                        app.close_create_format_popup();
                    }
                    _ => {
                        app.popups.active = Some(ActivePopup::CreateFormat(popup));
                    }
                }
                true
            }

            ActivePopup::ContextMenu(menu) => {
                // Context menu keys are handled in the list/edit view handlers;
                // re-insert and report unconsumed so they receive the key.
                app.popups.active = Some(ActivePopup::ContextMenu(menu));
                false
            }

            ActivePopup::Info(popup) => {
                // Info popup is handled in handle_global_popups_and_palette before
                // reaching this match; this arm is for exhaustiveness only.
                app.popups.active = Some(ActivePopup::Info(popup));
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_is_universal_quit_key_true_for_bare_q() {
        assert!(is_universal_quit_key(&key(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn test_is_universal_quit_key_true_for_bare_esc() {
        assert!(is_universal_quit_key(&key(
            KeyCode::Esc,
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn test_is_universal_quit_key_false_for_shift_q() {
        assert!(!is_universal_quit_key(&key(
            KeyCode::Char('Q'),
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn test_is_universal_quit_key_false_for_ctrl_q() {
        assert!(!is_universal_quit_key(&key(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn test_is_universal_quit_key_false_for_ctrl_esc() {
        assert!(!is_universal_quit_key(&key(
            KeyCode::Esc,
            KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn test_is_universal_quit_key_false_for_bare_x() {
        assert!(!is_universal_quit_key(&key(
            KeyCode::Char('x'),
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn test_sidebar_double_click() {
        use crate::app::{App, EditFocus, EditSidebar};
        use crate::editor::LinkItem;
        use crate::storage::Storage;
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");
        app.editor.sidebar = EditSidebar::Links;
        app.editor.links = vec![LinkItem {
            id: "test_note.md".to_string(),
            title: "Test Note".to_string(),
            is_backlink: false,
        }];
        std::fs::write(app.storage.notes_dir.join("test_note.md"), "# Test Note\n")
            .expect("value is present");

        let terminal_area = Rect::new(0, 0, 100, 40);
        let mut focus = EditFocus::Body;
        let mut selecting = false;
        let mut dragged = false;

        let (_, _, sidebar_inner) = crate::events::edit_view_input_areas(
            terminal_area,
            false,
            1,
            false,
            EditSidebar::Links,
            crate::config::PreviewPosition::Right,
        );
        let sb = sidebar_inner.unwrap();

        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sb.x + 1,
            row: sb.y + 3,
            modifiers: KeyModifiers::NONE,
        };

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event,
            terminal_area,
            &mut focus,
            &mut selecting,
            &mut dragged,
        );

        assert_eq!(focus, EditFocus::Sidebar);
        assert_eq!(app.editor.sidebar_selected, 0);
        assert!(app.editor.last_sidebar_click.is_some());

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event,
            terminal_area,
            &mut focus,
            &mut selecting,
            &mut dragged,
        );

        assert_eq!(app.editor.editing_id.as_deref(), Some("test_note.md"));
    }

    #[test]
    fn test_sidebar_double_click_outline() {
        use crate::app::{App, EditFocus, EditSidebar};
        use crate::content_tree::parse::TreeNode;
        use crate::storage::Storage;
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");
        app.editor.sidebar = EditSidebar::Outline;
        app.editor.outline_nodes = vec![TreeNode {
            kind: crate::content_tree::parse::NodeKind::Header {
                level: 1,
                title: "Heading 1".to_string(),
            },
            depth: 1,
            line: 42,
            has_children: false,
        }];

        let mut content = String::new();
        for i in 1..=50 {
            content.push_str(&format!("Line {i}\n"));
        }
        app.editor.editor.insert_str(&content);

        let terminal_area = Rect::new(0, 0, 100, 40);
        let mut focus = EditFocus::Body;
        let mut selecting = false;
        let mut dragged = false;

        let (_, _, sidebar_inner) = crate::events::edit_view_input_areas(
            terminal_area,
            false,
            1,
            false,
            EditSidebar::Outline,
            crate::config::PreviewPosition::Right,
        );
        let sb = sidebar_inner.unwrap();

        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sb.x + 1,
            row: sb.y + 3,
            modifiers: KeyModifiers::NONE,
        };

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event,
            terminal_area,
            &mut focus,
            &mut selecting,
            &mut dragged,
        );

        assert_eq!(focus, EditFocus::Sidebar);
        assert_eq!(app.editor.sidebar_selected, 0);

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event,
            terminal_area,
            &mut focus,
            &mut selecting,
            &mut dragged,
        );

        assert_eq!(focus, EditFocus::Body);
        assert_eq!(app.editor.editor.cursor(), (41, 0));
    }
}
