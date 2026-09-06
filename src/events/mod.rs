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

pub use edit::handle_edit_keys;
pub(crate) use edit::handle_edit_mouse;
pub use help::{handle_help_keys, handle_help_mouse};
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

/// Route a key through a text-input popup, re-inserting it on Submit/Edited.
/// `on_submit` runs with the popup already re-inserted. Returns `true` if consumed.
fn route_text(
    mut popup: crate::popups::ActivePopup,
    key: KeyEvent,
    app: &mut App,
    on_submit: impl FnOnce(&mut App),
) -> bool {
    let action = match &mut popup {
        crate::popups::ActivePopup::CreateNote(p, _) => {
            route_text_input_popup(&key, &app.keybinds, &mut p.input)
        }
        crate::popups::ActivePopup::Import(p) => {
            route_text_input_popup(&key, &app.keybinds, &mut p.input)
        }
        crate::popups::ActivePopup::Folder(p) => {
            route_text_input_popup(&key, &app.keybinds, &mut p.input)
        }
        crate::popups::ActivePopup::Goals(p) => {
            route_text_input_popup(&key, &app.keybinds, &mut p.input)
        }
        crate::popups::ActivePopup::NoteRename(p) => {
            route_text_input_popup(&key, &app.keybinds, &mut p.input)
        }
        _ => return false,
    };
    match action {
        TextInputPopupAction::Cancel => {}
        TextInputPopupAction::Submit => {
            app.popups.active = Some(popup);
            on_submit(app);
        }
        TextInputPopupAction::Edited => {
            app.popups.active = Some(popup);
        }
    }
    true
}

/// Route a key through a selection-list popup, re-inserting it on nav/confirm.
/// `on_nav`/`on_confirm` run with the popup already re-inserted; `on_cancel`
/// runs without re-inserting. Returns `true` if consumed.
fn route_selection_popup(
    mut popup: crate::popups::ActivePopup,
    key: KeyEvent,
    app: &mut App,
    on_nav: impl FnOnce(&mut App),
    on_confirm: impl FnOnce(&mut App),
    on_cancel: impl FnOnce(&mut App),
) -> bool {
    app.seq_matcher.clear();
    let action = match &mut popup {
        crate::popups::ActivePopup::IconMode(p) => {
            route_selection_list(&key, &app.keybinds, &mut p.selected, 2)
        }
        crate::popups::ActivePopup::HintBarStyle(p) => route_selection_list(
            &key,
            &app.keybinds,
            &mut p.selected,
            crate::config::HintBarStyle::ALL.len() - 1,
        ),
        crate::popups::ActivePopup::KeybindPreset(p) => {
            route_selection_list(&key, &app.keybinds, &mut p.selected, 3)
        }
        crate::popups::ActivePopup::Sort(p) => {
            route_selection_list(&key, &app.keybinds, &mut p.selected, 3)
        }
        crate::popups::ActivePopup::CreateFormat(p) => {
            route_selection_list(&key, &app.keybinds, &mut p.selected, 3)
        }
        _ => return false,
    };
    match action {
        SelListAction::Up | SelListAction::Down => {
            app.popups.active = Some(popup);
            on_nav(app);
        }
        SelListAction::Confirm => {
            app.popups.active = Some(popup);
            on_confirm(app);
        }
        SelListAction::Cancel => on_cancel(app),
        SelListAction::Other => {
            app.popups.active = Some(popup);
        }
    }
    true
}

/// Route a terminal bracketed-paste (`Event::Paste`) into the currently focused
/// text field. This is the ONLY paste delivery on terminals (kitty, most VTE
/// emulators) that intercept Ctrl+Shift+V themselves. Returns true if a field
/// accepted the text.
pub fn handle_bracketed_paste(
    app: &mut crate::app::App,
    data: String,
    focus: &mut crate::editor::EditFocus,
) -> bool {
    use crate::app::ViewMode;
    use crate::editor::EditFocus;
    use crate::popups::ActivePopup;
    use crate::popups::SubnotesFocus;

    // 1. Active popup
    if let Some(ref mut popup) = app.popups.active {
        match popup {
            ActivePopup::CreateNote(p, _) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::Import(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::Folder(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::FolderPicker(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::NoteRename(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::Search(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::Goals(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::Template(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::Tag(p) => {
                p.input.insert_str(&data);
                return true;
            }
            ActivePopup::Subnotes(p) => match p.focus {
                SubnotesFocus::EditTitle => {
                    p.title_input.insert_str(&data);
                    return true;
                }
                SubnotesFocus::EditContent => {
                    p.content_input.insert_str(&data);
                    return true;
                }
                SubnotesFocus::List => return false,
            },
            // Non-text popups — ignore paste
            ActivePopup::Theme(_)
            | ActivePopup::Info(_)
            | ActivePopup::IconMode(_)
            | ActivePopup::HintBarStyle(_)
            | ActivePopup::KeybindPreset(_)
            | ActivePopup::Sort(_)
            | ActivePopup::CreateFormat(_)
            | ActivePopup::RemoveTags(_)
            | ActivePopup::TrashView(_) => return false,
        }
    }

    // 2. Command palette
    if let Some(ref mut palette) = app.command_palette {
        palette.input.insert_str(&data);
        return true;
    }
    // 3. Help search popup
    if let Some(ref mut popup) = app.help_search.popup {
        popup.input.insert_str(&data);
        return true;
    }
    // 4. Editor find popup
    if let Some(ref mut popup) = app.editor.find_popup {
        popup.input.insert_str(&data);
        return true;
    }

    // 5. Per view mode
    match app.mode {
        ViewMode::Edit => match focus {
            EditFocus::Title => {
                let normalized = data.replace(['\r', '\n'], " ");
                app.editor.title_editor.insert_str(normalized);
                true
            }
            EditFocus::Body => {
                app.editor.body.insert_str(&data);
                app.request_editor_preview_update();
                true
            }
            EditFocus::Sidebar => false,
        },
        ViewMode::Canvas => {
            if let Some(canvas) = &mut app.canvas_state {
                let canvas = &mut canvas.state;
                if let Some(ta) = &mut canvas.rename_popup {
                    ta.insert_str(&data);
                    return true;
                }
                if let Some(ta) = &mut canvas.floating_editor {
                    ta.insert_str(&data);
                    // Mirror the node-sync: write editor text into selected node
                    if let Some(node_id) = &canvas.selection.primary {
                        let text = ta.lines().join("\n");
                        for node in &mut canvas.data.nodes {
                            if node.id() == node_id {
                                node.set_text(text);
                                break;
                            }
                        }
                        let _ = canvas.save();
                    }
                    return true;
                }
                if canvas.editor_focus {
                    canvas.raw_editor.insert_str(&data);
                    // Sync canvas from raw JSON pane without touching note editor state.
                    let content = canvas.raw_editor.lines().join("\n");
                    if let Ok(parsed) = serde_json::from_str::<pinstar::data::CanvasData>(&content)
                    {
                        canvas.data = parsed;
                        let _ = canvas.save();
                    }
                    return true;
                }
            }
            false
        }
        ViewMode::Draw => {
            if let Some(draw) = &mut app.draw_state
                && let Some((_, ta)) = &mut draw.text_editor
            {
                ta.insert_str(&data);
                true
            } else {
                false
            }
        }
        ViewMode::Backup => {
            if let Some(backup) = &mut app.backup_state {
                match backup.input_mode {
                    crate::backup::state::BackupInputMode::EditCommitMessage => {
                        backup.commit_textarea.insert_str(&data);
                        return true;
                    }
                    crate::backup::state::BackupInputMode::EditSettingsField => {
                        match backup.settings.focused_field {
                            crate::backup::state::SettingsField::RemoteUrl => {
                                backup.settings.remote_url.insert_str(&data);
                            }
                            crate::backup::state::SettingsField::RemoteName => {
                                backup.settings.remote_name.insert_str(&data);
                            }
                            _ => return false,
                        }
                        return true;
                    }
                    _ => return false,
                }
            }
            false
        }
        _ => false,
    }
}

pub fn move_textarea_cursor_to_mouse(
    textarea: &mut TextArea,
    body_inner: Rect,
    mouse_col: u16,
    mouse_row: u16,
    scroll_row: usize,
    scroll_col: usize,
) {
    if textarea.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }

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

use crate::config::PreviewPosition;
use crate::editor::EditSidebar;
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
    let editor_area = body_area;
    let title = Rect::default();

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
    fullscreen: bool,
    md_preview: bool,
    line_count: usize,
    show_line_numbers: bool,
    sidebar: crate::editor::EditSidebar,
    sidebar_position: crate::config::PreviewPosition,
    header_title_rect: Rect,
) -> (Rect, Rect, Option<Rect>) {
    // Outer vertical split (pad / body / footer) to find the body area.
    // This matches the layout used by draw_edit_view.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // spacer (matches draw_edit_view)
            Constraint::Min(8),    // body
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    let body_area = chunks[2];

    let layout = compute_edit_layout(body_area, fullscreen, md_preview, sidebar, sidebar_position);
    // Apply gutter offset to the body rect for mouse hit-testing.
    // In fullscreen (READ) mode the preview has no editor gutter.
    let gutter_width = if fullscreen {
        0
    } else if show_line_numbers {
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

    (header_title_rect, body_inner, layout.sidebar)
}

pub fn edit_view_md_preview_area(
    area: Rect,
    sidebar: crate::editor::EditSidebar,
    preview_position: crate::config::PreviewPosition,
) -> Option<Rect> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // spacer (matches draw_edit_view)
            Constraint::Min(8),    // body
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    let body_area = chunks[2];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmKeyAction {
    SelectConfirm,
    SelectCancel,
    ToggleSelection,
    ActivateSelection,
    Confirm,
    Cancel,
    Unhandled,
}

fn route_confirm_key(
    key: &KeyEvent,
    keybinds: &crate::keybinds::Keybinds,
    text_input_context: bool,
) -> ConfirmKeyAction {
    if text_input_context {
        return match key.code {
            KeyCode::Enter => ConfirmKeyAction::Confirm,
            KeyCode::Esc => ConfirmKeyAction::Cancel,
            _ => ConfirmKeyAction::Unhandled,
        };
    }

    let literal_modifier = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
    if literal_modifier && matches!(key.code, KeyCode::Char('y' | 'Y')) {
        ConfirmKeyAction::Confirm
    } else if literal_modifier && matches!(key.code, KeyCode::Char('n' | 'N')) {
        ConfirmKeyAction::Cancel
    } else if key.code == KeyCode::Left || key.code == KeyCode::Char('h') {
        ConfirmKeyAction::SelectConfirm
    } else if key.code == KeyCode::Right || key.code == KeyCode::Char('l') {
        ConfirmKeyAction::SelectCancel
    } else if key.code == KeyCode::Tab {
        ConfirmKeyAction::ToggleSelection
    } else if key.code == KeyCode::Enter {
        ConfirmKeyAction::ActivateSelection
    } else if is_cancel_popup(keybinds, key, false) {
        ConfirmKeyAction::Cancel
    } else if keybinds.matches_list(crate::keybinds::ListAction::Confirm, key) {
        ConfirmKeyAction::Confirm
    } else {
        ConfirmKeyAction::Unhandled
    }
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
    // Message overlay blockade — when a fatal message is active or the
    // overlay is force-opened, intercept most keys for scrolling / dismissal.
    if app.messages.has_fatal() || app.messages.force_open {
        return match key.code {
            crossterm::event::KeyCode::Esc => {
                if app.messages.has_fatal() {
                    app.should_quit = true;
                } else {
                    app.messages.force_open = false;
                    app.messages.scroll = 0;
                }
                true
            }
            crossterm::event::KeyCode::Char('q') => {
                app.should_quit = true;
                true
            }
            crossterm::event::KeyCode::Down => {
                app.messages.scroll = app.messages.scroll.saturating_add(1);
                true
            }
            crossterm::event::KeyCode::Up => {
                app.messages.scroll = app.messages.scroll.saturating_sub(1);
                true
            }
            crossterm::event::KeyCode::PageDown => {
                app.messages.scroll = app.messages.scroll.saturating_add(10);
                true
            }
            crossterm::event::KeyCode::PageUp => {
                app.messages.scroll = app.messages.scroll.saturating_sub(10);
                true
            }
            crossterm::event::KeyCode::F(3) => {
                app.messages.force_open = !app.messages.force_open;
                app.messages.scroll = 0;
                true
            }
            crossterm::event::KeyCode::F(2) => {
                app.quick_keybinds_open = !app.quick_keybinds_open;
                true
            }
            _ => true, // swallow everything else
        };
    }

    // QuickKeybinds toggle — identical combo in every view. F2 is unbound in
    // all 9 keybind scopes (verified in src/keybinds/defaults.rs); raw check
    // follows the is_universal_quit_key precedent (global keys live outside
    // the per-scope enum system). Only toggles when no popup/palette is open
    // so it never fights modal input.
    if app.popups.active.is_none()
        && app.command_palette.is_none()
        && key.code == crossterm::event::KeyCode::F(2)
    {
        app.quick_keybinds_open = !app.quick_keybinds_open;
        return true;
    }
    // Message overlay toggle — F3 force-opens/closes the message overlay.
    if key.code == crossterm::event::KeyCode::F(3) {
        app.messages.force_open = !app.messages.force_open;
        return true;
    }
    // F1 — global help toggle. Opens help at the tab related to the current
    // view. Skipped in Help view (let HelpAction::Close handle F1 so it toggles
    // closed — bound at src/keybinds/defaults.rs:351) and Setup view (no help
    // path, per design). Raw check mirrors F2/F3 precedent.
    if app.popups.active.is_none()
        && app.command_palette.is_none()
        && key.code == crossterm::event::KeyCode::F(1)
        && !matches!(
            app.mode,
            crate::app::ViewMode::Help | crate::app::ViewMode::Setup
        )
        && let Some(tab) = app.mode.help_tab()
    {
        app.open_help_page_with_tab(tab);
        return true;
    }

    // F5 — global full view redraw. Sets the existing `needs_full_redraw`
    // flag; the main loop then calls terminal.clear() and forces
    // list_dirty/graph_dirty=true so the next frame repaints every view from
    // scratch. Active from every view.
    if app.popups.active.is_none()
        && app.command_palette.is_none()
        && key.code == crossterm::event::KeyCode::F(5)
    {
        app.needs_full_redraw = true;
        app.set_temporary_status_static("View redrawn");
        return true;
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
                    app.messages.push(
                        format!("Action failed: {e}"),
                        crate::app::messages::MessageSeverity::Warning,
                    );
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
        match route_confirm_key(&key, &app.keybinds, false) {
            ConfirmKeyAction::SelectConfirm => app.confirm_popup_select_confirm(),
            ConfirmKeyAction::SelectCancel => app.confirm_popup_select_cancel(),
            ConfirmKeyAction::ToggleSelection => app.confirm_popup_toggle_button(),
            ConfirmKeyAction::ActivateSelection => app.confirm_popup_activate(),
            ConfirmKeyAction::Confirm => app.confirm_action(),
            ConfirmKeyAction::Cancel => app.cancel_confirm(),
            ConfirmKeyAction::Unhandled => {}
        }
        return true;
    }

    // Group B: remaining popups.
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
            ActivePopup::CreateNote(popup, format) => {
                route_text(ActivePopup::CreateNote(popup, format), key, app, |app| {
                    app.confirm_create_note();
                })
            }
            ActivePopup::Import(popup) => route_text(ActivePopup::Import(popup), key, app, |app| {
                app.confirm_import();
            }),
            ActivePopup::Folder(popup) => route_text(ActivePopup::Folder(popup), key, app, |app| {
                app.confirm_folder_popup();
            }),
            ActivePopup::Tag(mut popup) => {
                if app.popups.confirm.is_some() {
                    app.popups.active = Some(ActivePopup::Tag(popup));
                    match route_confirm_key(&key, &app.keybinds, true) {
                        ConfirmKeyAction::SelectConfirm => app.confirm_popup_select_confirm(),
                        ConfirmKeyAction::SelectCancel => app.confirm_popup_select_cancel(),
                        ConfirmKeyAction::ToggleSelection => app.confirm_popup_toggle_button(),
                        ConfirmKeyAction::ActivateSelection => app.confirm_popup_activate(),
                        ConfirmKeyAction::Confirm => app.confirm_action(),
                        ConfirmKeyAction::Cancel => app.cancel_confirm(),
                        ConfirmKeyAction::Unhandled => {}
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
                                crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
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
                            KeyCode::Enter => {
                                app.popups.active = Some(ActivePopup::Tag(popup));
                                app.accept_tag_from_all_tags();
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
            ActivePopup::Goals(popup) => route_text(ActivePopup::Goals(popup), key, app, |app| {
                app.confirm_goals_popup();
            }),
            ActivePopup::Subnotes(mut popup) => {
                let commit_edits = |popup: &mut crate::popups::SubnotesPopup| {
                    let cur_idx = popup.selected;
                    if !popup.subnotes.is_empty() && cur_idx < popup.subnotes.len() {
                        let new_title = popup.title_input.lines().join("");
                        let new_content = popup.content_input.lines().join("\n");
                        if popup.subnotes[cur_idx].title != new_title
                            || popup.subnotes[cur_idx].content != new_content
                        {
                            popup.subnotes[cur_idx].title = new_title;
                            popup.subnotes[cur_idx].content = new_content;
                            popup.subnotes[cur_idx].updated_at = crate::ui::now_unix_secs();
                            popup.is_dirty = true;
                        }
                    }
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
                    commit_edits(&mut popup);

                    let new_subnote = crate::storage::SubNote {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: "New Note".to_string(),
                        content: "".to_string(),
                        updated_at: crate::ui::now_unix_secs(),
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
                                commit_edits(&mut popup);
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
                                commit_edits(&mut popup);
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
                            commit_edits(&mut popup);

                            let new_subnote = crate::storage::SubNote {
                                id: uuid::Uuid::new_v4().to_string(),
                                title: "New Note".to_string(),
                                content: "".to_string(),
                                updated_at: crate::ui::now_unix_secs(),
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
                            commit_edits(&mut popup);
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                            let _ = app.close_subnotes_popup();
                        } else {
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        }
                    }
                    crate::popups::SubnotesFocus::EditTitle => {
                        if key.code == KeyCode::Tab || key.code == KeyCode::Enter {
                            commit_edits(&mut popup);
                            popup.focus = crate::popups::SubnotesFocus::EditContent;
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Esc {
                            commit_edits(&mut popup);
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
                            commit_edits(&mut popup);
                            popup.focus = crate::popups::SubnotesFocus::EditTitle;
                            app.popups.active = Some(ActivePopup::Subnotes(popup));
                        } else if key.code == KeyCode::Esc {
                            commit_edits(&mut popup);
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
            ActivePopup::NoteRename(popup) => {
                route_text(ActivePopup::NoteRename(popup), key, app, |app| {
                    app.confirm_rename_note();
                })
            }
            ActivePopup::Search(mut popup) => {
                let has_title = !popup.title_result_ids.is_empty();
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
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep {
                            let r = popup.grep_selected;
                            let hit_idx = match popup.grep_row_offsets.binary_search(&r) {
                                Ok(i) => i,
                                Err(i) => i.saturating_sub(1),
                            };
                            let base = popup.grep_row_offsets.get(hit_idx).copied().unwrap_or(0);
                            if r == base {
                                if let Some(hit) = popup.grep_results.get(hit_idx) {
                                    if popup.grep_expanded.contains(&hit.note_id) {
                                        popup.grep_expanded.remove(&hit.note_id);
                                    } else {
                                        popup.grep_expanded.insert(hit.note_id.clone());
                                    }
                                    popup.rebuild_grep_offsets();
                                }
                            } else {
                                app.popups.active = Some(reinsert(popup));
                                app.jump_to_selected_result();
                                app.confirm_search();
                                return true;
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
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep {
                            popup.grep_selected = popup.grep_selected.saturating_sub(1);
                            app.popups.active = Some(reinsert(popup));
                        } else if has_title {
                            popup.title_selected = popup.title_selected.saturating_sub(1);
                            app.popups.active = Some(reinsert(popup));
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep {
                            popup.grep_selected = (popup.grep_selected + 1)
                                .min(popup.total_grep_rows().saturating_sub(1));
                            app.popups.active = Some(reinsert(popup));
                        } else if has_title {
                            if popup.title_selected + 1 < popup.title_result_ids.len() {
                                popup.title_selected += 1;
                            }
                            app.popups.active = Some(reinsert(popup));
                        }
                    }
                    KeyCode::Right | KeyCode::Char(' ') => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep {
                            let r = popup.grep_selected;
                            let hit_idx = match popup.grep_row_offsets.binary_search(&r) {
                                Ok(i) => i,
                                Err(i) => i.saturating_sub(1),
                            };
                            let base = popup.grep_row_offsets.get(hit_idx).copied().unwrap_or(0);
                            if r == base
                                && let Some(hit) = popup.grep_results.get(hit_idx)
                            {
                                popup.grep_expanded.insert(hit.note_id.clone());
                                popup.rebuild_grep_offsets();
                            }
                            app.popups.active = Some(reinsert(popup));
                        } else {
                            popup.focus = crate::popups::SearchFocus::Input;
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        }
                    }
                    KeyCode::Left => {
                        if popup.focus == crate::popups::SearchFocus::Input {
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        } else if has_grep {
                            let r = popup.grep_selected;
                            let hit_idx = match popup.grep_row_offsets.binary_search(&r) {
                                Ok(i) => i,
                                Err(i) => i.saturating_sub(1),
                            };
                            let base = popup.grep_row_offsets.get(hit_idx).copied().unwrap_or(0);
                            if r == base
                                && let Some(hit) = popup.grep_results.get(hit_idx)
                            {
                                popup.grep_expanded.remove(&hit.note_id);
                                popup.rebuild_grep_offsets();
                            }
                            app.popups.active = Some(reinsert(popup));
                        } else {
                            popup.focus = crate::popups::SearchFocus::Input;
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_search();
                        }
                    }
                    _ => {
                        popup.focus = crate::popups::SearchFocus::Input;
                        crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
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
                            crate::text_edit::feed_key(&app.keybinds, &mut picker.input, key);
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
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
                            app.popups.active = Some(reinsert(popup));
                            app.update_template_popup_filter();
                        }
                    }
                    KeyCode::Char('n') => {
                        if popup.focus == crate::popups::TemplatePopupFocus::Results {
                            app.popups.active = Some(reinsert(popup));
                            app.create_template_from_popup();
                        } else {
                            crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
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
                                crate::text_edit::feed_key(&app.keybinds, &mut popup.input, key);
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
            ActivePopup::IconMode(popup) => route_selection_popup(
                ActivePopup::IconMode(popup),
                key,
                app,
                |_| {},
                |app| app.select_icon_mode(),
                |app| app.close_icon_mode_popup(),
            ),
            ActivePopup::HintBarStyle(popup) => route_selection_popup(
                ActivePopup::HintBarStyle(popup),
                key,
                app,
                |app| app.select_hint_bar_style(),
                |app| {
                    app.select_hint_bar_style();
                    app.close_hint_bar_style_popup();
                },
                |app| app.close_hint_bar_style_popup(),
            ),
            ActivePopup::KeybindPreset(popup) => route_selection_popup(
                ActivePopup::KeybindPreset(popup),
                key,
                app,
                |app| app.select_keybind_preset(),
                |app| {
                    app.select_keybind_preset();
                    app.close_keybind_preset_popup();
                },
                |app| app.close_keybind_preset_popup(),
            ),
            ActivePopup::Sort(popup) => route_selection_popup(
                ActivePopup::Sort(popup),
                key,
                app,
                |_| {},
                |app| app.select_sort(),
                |app| app.close_sort_popup(),
            ),
            ActivePopup::CreateFormat(popup) => route_selection_popup(
                ActivePopup::CreateFormat(popup),
                key,
                app,
                |_| {},
                |app| app.confirm_create_format(),
                |app| app.close_create_format_popup(),
            ),

            ActivePopup::RemoveTags(mut popup) => {
                // Handle confirm overlay for "remove all tags"
                if popup.confirm.is_some() {
                    let action = {
                        let confirm = popup
                            .confirm
                            .as_mut()
                            .expect("confirm popup should be Some");
                        match route_confirm_key(&key, &app.keybinds, false) {
                            ConfirmKeyAction::SelectConfirm => {
                                confirm.selected_button = 0;
                                None
                            }
                            ConfirmKeyAction::SelectCancel => {
                                confirm.selected_button = 1;
                                None
                            }
                            ConfirmKeyAction::ToggleSelection => {
                                confirm.selected_button = (confirm.selected_button + 1) % 2;
                                None
                            }
                            ConfirmKeyAction::ActivateSelection => {
                                Some(confirm.selected_button == 0)
                            }
                            ConfirmKeyAction::Confirm => Some(true),
                            ConfirmKeyAction::Cancel => Some(false),
                            ConfirmKeyAction::Unhandled => None,
                        }
                    };
                    match action {
                        Some(true) => {
                            popup.confirm = None;
                            app.popups.active = Some(ActivePopup::RemoveTags(popup));
                            app.confirm_remove_all_tags();
                        }
                        Some(false) => {
                            popup.confirm = None;
                            app.popups.active = Some(ActivePopup::RemoveTags(popup));
                        }
                        None => {
                            app.popups.active = Some(ActivePopup::RemoveTags(popup));
                        }
                    }
                    return true;
                }

                app.seq_matcher.clear();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        popup.cursor = popup.cursor.saturating_sub(1);
                        app.popups.active = Some(ActivePopup::RemoveTags(popup));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.cursor + 1 < popup.tags.len() {
                            popup.cursor += 1;
                        }
                        app.popups.active = Some(ActivePopup::RemoveTags(popup));
                    }
                    KeyCode::Char(' ') => {
                        if popup.selected.contains(&popup.cursor) {
                            popup.selected.remove(&popup.cursor);
                        } else {
                            popup.selected.insert(popup.cursor);
                        }
                        app.popups.active = Some(ActivePopup::RemoveTags(popup));
                    }
                    KeyCode::Char('a') => {
                        if popup.selected.len() == popup.tags.len() {
                            popup.selected.clear();
                        } else {
                            for i in 0..popup.tags.len() {
                                popup.selected.insert(i);
                            }
                        }
                        app.popups.active = Some(ActivePopup::RemoveTags(popup));
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        app.popups.active = Some(ActivePopup::RemoveTags(popup));
                        app.begin_remove_all_tags_from_selected();
                    }
                    KeyCode::Enter => {
                        app.popups.active = Some(ActivePopup::RemoveTags(popup));
                        app.confirm_remove_tags_from_selected();
                        return true;
                    }
                    KeyCode::Esc => {
                        // Close popup
                        return true;
                    }
                    _ => {
                        app.popups.active = Some(ActivePopup::RemoveTags(popup));
                        return true;
                    }
                }
                true
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

    fn test_app() -> (tempfile::TempDir, crate::app::App) {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = crate::storage::Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir: temp_dir.path().join("config"),
            notes_dir: temp_dir.path().join("notes"),
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        for path in [
            &storage.data_dir,
            &storage.config_dir,
            &storage.notes_dir,
            &storage.templates_dir,
        ] {
            std::fs::create_dir_all(path).unwrap();
        }
        (temp_dir, crate::app::App::new(storage).unwrap())
    }

    #[test]
    fn confirm_key_router_reserves_literal_yes_no() {
        let keybinds = crate::keybinds::Keybinds::default();
        for (code, modifiers) in [
            (KeyCode::Char('y'), KeyModifiers::NONE),
            (KeyCode::Char('Y'), KeyModifiers::SHIFT),
        ] {
            assert_eq!(
                route_confirm_key(&key(code, modifiers), &keybinds, false),
                ConfirmKeyAction::Confirm
            );
        }
        for (code, modifiers) in [
            (KeyCode::Char('n'), KeyModifiers::NONE),
            (KeyCode::Char('N'), KeyModifiers::SHIFT),
        ] {
            assert_eq!(
                route_confirm_key(&key(code, modifiers), &keybinds, false),
                ConfirmKeyAction::Cancel
            );
        }
        for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT, KeyModifiers::META] {
            assert_eq!(
                route_confirm_key(&key(KeyCode::Char('y'), modifier), &keybinds, false),
                ConfirmKeyAction::Unhandled
            );
            assert_eq!(
                route_confirm_key(&key(KeyCode::Char('n'), modifier), &keybinds, false),
                ConfirmKeyAction::Unhandled
            );
        }
    }

    #[test]
    fn confirm_key_router_preserves_selection_controls() {
        let keybinds = crate::keybinds::Keybinds::default();
        assert_eq!(
            route_confirm_key(&key(KeyCode::Left, KeyModifiers::NONE), &keybinds, false),
            ConfirmKeyAction::SelectConfirm
        );
        assert_eq!(
            route_confirm_key(
                &key(KeyCode::Char('h'), KeyModifiers::NONE),
                &keybinds,
                false
            ),
            ConfirmKeyAction::SelectConfirm
        );
        assert_eq!(
            route_confirm_key(&key(KeyCode::Right, KeyModifiers::NONE), &keybinds, false),
            ConfirmKeyAction::SelectCancel
        );
        assert_eq!(
            route_confirm_key(
                &key(KeyCode::Char('l'), KeyModifiers::NONE),
                &keybinds,
                false
            ),
            ConfirmKeyAction::SelectCancel
        );
        assert_eq!(
            route_confirm_key(&key(KeyCode::Tab, KeyModifiers::NONE), &keybinds, false),
            ConfirmKeyAction::ToggleSelection
        );
        assert_eq!(
            route_confirm_key(&key(KeyCode::Enter, KeyModifiers::NONE), &keybinds, false),
            ConfirmKeyAction::ActivateSelection
        );
    }

    #[test]
    fn confirm_key_router_keeps_configured_fallbacks() {
        use crate::keybinds::{KeyCombo, ListAction};

        let mut keybinds = crate::keybinds::Keybinds::default();
        keybinds
            .list
            .insert(ListAction::Confirm, vec![KeyCombo::simple(KeyCode::F(6))]);
        keybinds
            .list
            .insert(ListAction::Cancel, vec![KeyCombo::simple(KeyCode::F(7))]);
        assert_eq!(
            route_confirm_key(&key(KeyCode::F(6), KeyModifiers::NONE), &keybinds, false),
            ConfirmKeyAction::Confirm
        );
        assert_eq!(
            route_confirm_key(&key(KeyCode::F(7), KeyModifiers::NONE), &keybinds, false),
            ConfirmKeyAction::Cancel
        );

        keybinds
            .list
            .insert(ListAction::Confirm, vec![KeyCombo::simple(KeyCode::F(8))]);
        keybinds
            .list
            .insert(ListAction::Cancel, vec![KeyCombo::simple(KeyCode::F(8))]);
        assert_eq!(
            route_confirm_key(&key(KeyCode::F(8), KeyModifiers::NONE), &keybinds, false),
            ConfirmKeyAction::Cancel
        );

        keybinds.list.insert(
            ListAction::Cancel,
            vec![KeyCombo::simple(KeyCode::Char('y'))],
        );
        keybinds.list.insert(
            ListAction::Confirm,
            vec![KeyCombo::simple(KeyCode::Char('n'))],
        );
        assert_eq!(
            route_confirm_key(
                &key(KeyCode::Char('y'), KeyModifiers::NONE),
                &keybinds,
                false
            ),
            ConfirmKeyAction::Confirm
        );
        assert_eq!(
            route_confirm_key(
                &key(KeyCode::Char('n'), KeyModifiers::NONE),
                &keybinds,
                false
            ),
            ConfirmKeyAction::Cancel
        );
    }

    #[test]
    fn confirm_key_router_text_input_requires_enter_or_escape() {
        let keybinds = crate::keybinds::Keybinds::default();
        assert_eq!(
            route_confirm_key(
                &key(KeyCode::Char('y'), KeyModifiers::NONE),
                &keybinds,
                true
            ),
            ConfirmKeyAction::Unhandled
        );
        assert_eq!(
            route_confirm_key(
                &key(KeyCode::Char('n'), KeyModifiers::NONE),
                &keybinds,
                true
            ),
            ConfirmKeyAction::Unhandled
        );
        assert_eq!(
            route_confirm_key(&key(KeyCode::Enter, KeyModifiers::NONE), &keybinds, true),
            ConfirmKeyAction::Confirm
        );
        assert_eq!(
            route_confirm_key(&key(KeyCode::Esc, KeyModifiers::NONE), &keybinds, true),
            ConfirmKeyAction::Cancel
        );
    }

    #[test]
    fn confirm_standalone_yes_bypasses_cancel_selection() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let (_temp_dir, mut app) = test_app();
        app.show_confirm(crate::popups::ConfirmAction::QuitApp);
        assert_eq!(app.popups.confirm.as_ref().unwrap().selected_button, 1);
        handle_global_popups_and_palette(
            &mut app,
            crossterm::event::Event::Key(key(KeyCode::Char('y'), KeyModifiers::NONE)),
            Rect::default(),
        );
        assert!(app.should_quit);

        app.should_quit = false;
        app.show_confirm(crate::popups::ConfirmAction::QuitApp);
        handle_global_popups_and_palette(
            &mut app,
            crossterm::event::Event::Key(key(KeyCode::Char('n'), KeyModifiers::NONE)),
            Rect::default(),
        );
        assert!(app.popups.confirm.is_none());
        assert!(!app.should_quit);

        app.show_confirm(crate::popups::ConfirmAction::QuitApp);
        handle_global_popups_and_palette(
            &mut app,
            crossterm::event::Event::Key(key(KeyCode::Enter, KeyModifiers::NONE)),
            Rect::default(),
        );
        assert!(app.popups.confirm.is_none());
        assert!(!app.should_quit);
    }

    #[test]
    fn confirm_tag_uses_enter_escape_over_literal_yes_no() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let (_temp_dir, mut app) = test_app();
        let note = crate::storage::Note {
            title: "Tagged".into(),
            content: String::new(),
            updated_at: 0,
            tags: vec!["obsolete".into()],
        };
        let note_id = app.storage.save_note("tagged.md", &note).unwrap();
        app.popups.active = Some(crate::popups::ActivePopup::Tag(crate::popups::TagPopup {
            note_id: note_id.clone(),
            batch_note_ids: None,
            input: ratatui_textarea::TextArea::default(),
            all_tags: vec![],
            suggestions: vec![],
            suggestion_index: 0,
            focus: crate::popups::TagPopupFocus::Input,
            all_tags_selected: 0,
            scroll_offset: 0,
            last_scroll: None,
        }));

        app.begin_delete_tag_with_name("obsolete".into());
        assert_eq!(app.popups.confirm.as_ref().unwrap().selected_button, 0);
        for key_code in [KeyCode::Char('y'), KeyCode::Char('n')] {
            handle_global_popups_and_palette(
                &mut app,
                crossterm::event::Event::Key(key(key_code, KeyModifiers::NONE)),
                Rect::default(),
            );
            assert!(app.popups.confirm.is_some());
            assert_eq!(app.storage.load_note(&note_id).unwrap().tags, ["obsolete"]);
        }
        handle_global_popups_and_palette(
            &mut app,
            crossterm::event::Event::Key(key(KeyCode::Esc, KeyModifiers::NONE)),
            Rect::default(),
        );
        assert!(app.popups.confirm.is_none());
        assert_eq!(app.storage.load_note(&note_id).unwrap().tags, ["obsolete"]);

        app.begin_delete_tag_with_name("obsolete".into());
        handle_global_popups_and_palette(
            &mut app,
            crossterm::event::Event::Key(key(KeyCode::Enter, KeyModifiers::NONE)),
            Rect::default(),
        );
        assert!(app.storage.load_note(&note_id).unwrap().tags.is_empty());
    }

    fn remove_tags_popup() -> crate::popups::RemoveTagsPopup {
        crate::popups::RemoveTagsPopup {
            tags: vec![],
            selected: std::collections::HashSet::new(),
            cursor: 0,
            scroll_offset: 0,
            last_scroll: None,
            confirm: Some(crate::popups::ConfirmPopup {
                action: crate::popups::ConfirmAction::RemoveAllTagsFromSelected,
                message: "Remove ALL tags from selected notes?".into(),
                detail: Some("This cannot be undone.".into()),
                confirm_label: "Remove All".into(),
                is_destructive: true,
                selected_button: 1,
            }),
            tag_counts: vec![],
            total_selected: 0,
        }
    }

    #[test]
    fn confirm_remove_tags_yes_bypasses_cancel_selection() {
        let _lock = crate::config::ConfigTestGuard::lock();
        for (key_code, expect_closed) in [
            (KeyCode::Char('y'), true),
            (KeyCode::Char('n'), false),
            (KeyCode::Enter, false),
        ] {
            let (_temp_dir, mut app) = test_app();
            app.popups.active = Some(crate::popups::ActivePopup::RemoveTags(remove_tags_popup()));
            handle_global_popups_and_palette(
                &mut app,
                crossterm::event::Event::Key(key(key_code, KeyModifiers::NONE)),
                Rect::default(),
            );
            assert_eq!(app.popups.active.is_none(), expect_closed);
            if !expect_closed {
                assert!(matches!(
                    &app.popups.active,
                    Some(crate::popups::ActivePopup::RemoveTags(popup))
                        if popup.confirm.is_none()
                ));
            }
        }
    }

    #[test]
    fn test_sidebar_double_click() {
        let _lock = crate::config::ConfigTestGuard::lock();
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
            skip_dir_patterns: Vec::new(),
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
        let mut selection = crate::text_edit::MouseTextSelection::default();

        let (_, _, sidebar_inner) = crate::events::edit_view_input_areas(
            terminal_area,
            false,
            false,
            1,
            false,
            EditSidebar::Links,
            crate::config::PreviewPosition::Right,
            Rect::default(),
        );
        let sb = sidebar_inner.unwrap();
        app.editor.sidebar_list_rect = Rect::new(0, sb.y + 3, 100, 10);

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
            &mut selection,
        );

        assert_eq!(focus, EditFocus::Sidebar);
        assert_eq!(app.editor.sidebar_selected, 0);
        assert!(app.editor.last_sidebar_click.is_some());

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event,
            terminal_area,
            &mut focus,
            &mut selection,
        );

        assert_eq!(app.editor.editing_id.as_deref(), Some("test_note.md"));
    }

    #[test]
    fn test_sidebar_double_click_outline() {
        let _lock = crate::config::ConfigTestGuard::lock();
        use crate::app::{App, EditFocus, EditSidebar};
        use crate::outline::parse::TreeNode;
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
            skip_dir_patterns: Vec::new(),
        };
        let mut app = App::new(storage).expect("value is present");
        app.editor.sidebar = EditSidebar::Outline;
        app.editor.outline_nodes = vec![TreeNode {
            kind: crate::outline::parse::NodeKind::Header {
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
        app.editor.body.insert_str(&content);

        let terminal_area = Rect::new(0, 0, 100, 40);
        let mut focus = EditFocus::Body;
        let mut selection = crate::text_edit::MouseTextSelection::default();

        let (_, _, sidebar_inner) = crate::events::edit_view_input_areas(
            terminal_area,
            false,
            false,
            1,
            false,
            EditSidebar::Outline,
            crate::config::PreviewPosition::Right,
            Rect::default(),
        );
        let sb = sidebar_inner.unwrap();
        app.editor.sidebar_list_rect = Rect::new(0, sb.y + 3, 100, 10);

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
            &mut selection,
        );

        assert_eq!(focus, EditFocus::Sidebar);
        assert_eq!(app.editor.sidebar_selected, 0);

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event,
            terminal_area,
            &mut focus,
            &mut selection,
        );

        assert_eq!(focus, EditFocus::Body);
        assert_eq!(
            app.editor.body.cursor(),
            crate::editor_document::TextPosition { row: 41, col: 0 }
        );
    }
    #[test]
    fn right_click_text_does_not_open_context_menu() {
        let _lock = crate::config::ConfigTestGuard::lock();
        use crate::app::{App, EditFocus};
        use crate::storage::Storage;
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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
            skip_dir_patterns: Vec::new(),
        };
        let mut app = App::new(storage).expect("value is present");
        app.editor
            .body
            .insert_str("Hello world\nThis is a test\nSome more text\n");

        let terminal_area = Rect::new(0, 0, 80, 24);
        let mut focus = EditFocus::Body;
        let mut selection = crate::text_edit::MouseTextSelection::default();

        let (_, body_inner, _) = crate::events::edit_view_input_areas(
            terminal_area,
            false,
            app.editor.editor_preview_enabled,
            app.editor.body.lines().len(),
            app.editor.show_line_numbers,
            app.editor.sidebar,
            app.preview_position,
            app.editor.header_title_rect,
        );

        // Put cursor at the start
        app.editor
            .body
            .move_cursor(ratatui_textarea::CursorMove::Top);

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        // Scenario 1: right-click without selection has no text-edit effect.
        let click_col = body_inner.x + 5;
        let click_row = body_inner.y + 1;
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::NONE,
        };

        terminal
            .draw(|frame| {
                crate::ui::render_editor_document_with_theme(
                    frame,
                    &mut app.editor.body,
                    body_inner,
                    &app.app_theme,
                    true,
                    false,
                    ratatui::widgets::Block::default(),
                    app.app_theme.bg_style(),
                );
            })
            .unwrap();

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event,
            terminal_area,
            &mut focus,
            &mut selection,
        );
        assert_eq!(
            app.editor.body.cursor(),
            crate::editor_document::TextPosition { row: 0, col: 0 }
        );
        assert!(app.popups.active.is_none());

        // Reset cursor to (0, 0)
        app.editor
            .body
            .move_cursor(ratatui_textarea::CursorMove::Top);

        // Scenario 2: Right-click with selection.
        // Start selection, move cursor to create a selection.
        app.editor.body.start_selection();
        app.editor
            .body
            .move_cursor(ratatui_textarea::CursorMove::WordForward);
        assert!(app.editor.body.selection_range().is_some());
        let orig_cursor = app.editor.body.cursor();

        // Right-click inside the body_inner area
        let mouse_event_with_sel = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: body_inner.x + 8,
            row: body_inner.y + 1,
            modifiers: KeyModifiers::NONE,
        };

        terminal
            .draw(|frame| {
                crate::ui::render_editor_document_with_theme(
                    frame,
                    &mut app.editor.body,
                    body_inner,
                    &app.app_theme,
                    true,
                    false,
                    ratatui::widgets::Block::default(),
                    app.app_theme.bg_style(),
                );
            })
            .unwrap();

        super::edit::handle_edit_mouse(
            &mut app,
            mouse_event_with_sel,
            terminal_area,
            &mut focus,
            &mut selection,
        );

        // Existing selection remains untouched and no popup appears.
        assert_eq!(app.editor.body.cursor(), orig_cursor);
        assert!(app.editor.body.selection_range().is_some());
        assert!(app.popups.active.is_none());
    }

    #[test]
    fn find_highlight_overlay_wraps_without_panic() {
        let _lock = crate::config::ConfigTestGuard::lock();
        use crate::app::App;
        use crate::storage::Storage;
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
            skip_dir_patterns: Vec::new(),
        };
        let mut app = App::new(storage).expect("value is present");
        app.editor.body =
            crate::editor_document::EditorDocument::from_text("the quick brown fox jumps over");
        app.editor
            .body
            .set_wrap_mode(ratatui_textarea::WrapMode::WordOrGlyph);
        app.editor.show_line_numbers = true;

        let mut p = crate::ui::quick_search::QuickSearch::new(" Find ", &app.app_theme);
        p.input.insert_str("the");
        app.editor.find_popup = Some(p);

        let backend = ratatui::backend::TestBackend::new(12, 6);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                crate::ui::render_editor_document_with_theme(
                    frame,
                    &mut app.editor.body,
                    ratatui::layout::Rect::new(0, 0, 12, 6),
                    &app.app_theme,
                    true,
                    true,
                    ratatui::widgets::Block::default(),
                    app.app_theme.bg_style(),
                );
                crate::ui::overlay_search_highlights(
                    frame,
                    &app,
                    ratatui::layout::Rect::new(0, 0, 12, 6),
                );
            })
            .unwrap();

        let content_left = 1u16 + 2; // num_digits(1) + 2
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((content_left, 0))
                .expect("cell")
                .style()
                .bg,
            Some(app.app_theme.highlight_bg),
        );
    }
    #[test]
    fn test_message_overlay_scrolling_and_dismissal() {
        let _lock = crate::config::ConfigTestGuard::lock();
        use crate::app::App;
        use crate::storage::Storage;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("value is present");
        let storage = Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir: temp_dir.path().join("config"),
            notes_dir: temp_dir.path().join("notes"),
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(&storage.data_dir).unwrap();
        std::fs::create_dir_all(&storage.config_dir).unwrap();
        std::fs::create_dir_all(&storage.notes_dir).unwrap();
        std::fs::create_dir_all(&storage.templates_dir).unwrap();

        let mut app = App::new(storage).expect("value is present");

        // 1. Initially scroll is 0, force_open is false
        assert_eq!(app.messages.scroll, 0);
        assert!(!app.messages.force_open);

        // 2. Press F3 to toggle force_open
        let f3_event = Event::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE));
        let consumed = handle_global_popups_and_palette(&mut app, f3_event, Rect::default());
        assert!(consumed);
        assert!(app.messages.force_open);

        // 3. Pushing messages to scroll
        app.messages.push(
            "Warning 1".to_string(),
            crate::app::messages::MessageSeverity::Warning,
        );
        app.messages.push(
            "Warning 2".to_string(),
            crate::app::messages::MessageSeverity::Warning,
        );

        // 4. Press Down key — scroll increases to 1
        let down_event = Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let consumed = handle_global_popups_and_palette(&mut app, down_event, Rect::default());
        assert!(consumed);
        assert_eq!(app.messages.scroll, 1);

        // 5. Press Up key — scroll decreases to 0
        let up_event = Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let consumed = handle_global_popups_and_palette(&mut app, up_event, Rect::default());
        assert!(consumed);
        assert_eq!(app.messages.scroll, 0);

        // 6. Press PageDown — scroll increases to 10
        let pagedown_event = Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        let consumed = handle_global_popups_and_palette(&mut app, pagedown_event, Rect::default());
        assert!(consumed);
        assert_eq!(app.messages.scroll, 10);

        // 7. Press PageUp — scroll decreases to 0
        let pageup_event = Event::Key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        let consumed = handle_global_popups_and_palette(&mut app, pageup_event, Rect::default());
        assert!(consumed);
        assert_eq!(app.messages.scroll, 0);

        // 8. Press Esc key — force_open becomes false
        let esc_event = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let consumed = handle_global_popups_and_palette(&mut app, esc_event, Rect::default());
        assert!(consumed);
        assert!(!app.messages.force_open);
        assert_eq!(app.messages.scroll, 0);
    }
}
