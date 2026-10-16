use crate::editor_document::EditorDocument;
use crate::keybinds::{EditAction, Keybinds};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::{CursorMove, TextArea};
use std::cell::RefCell;
use std::io::Write;
use std::process::{Command, Stdio};

thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
    #[cfg(test)]
    pub(crate) static TEST_CLIPBOARD: RefCell<Option<String>> = const { RefCell::new(None) };
}

thread_local! {
    static CLIPBOARD_NOTICE: RefCell<Option<&'static str>> = const { RefCell::new(None) };
}

/// Drain the pending clipboard notice set by `apply_text_shortcuts`.
/// The event loop calls this once per iteration and shows it via `set_temporary_status`.
pub fn take_clipboard_notice() -> Option<&'static str> {
    CLIPBOARD_NOTICE.with(|n| n.borrow_mut().take())
}

fn set_clipboard_notice(msg: &'static str) {
    CLIPBOARD_NOTICE.with(|n| *n.borrow_mut() = Some(msg));
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY_NAME").is_ok()
}

pub fn write_system_clipboard(text: &str) {
    #[cfg(test)]
    if TEST_CLIPBOARD.with(|clipboard| {
        let mut clipboard = clipboard.borrow_mut();
        if clipboard.is_some() {
            *clipboard = Some(text.to_owned());
            true
        } else {
            false
        }
    }) {
        return;
    }
    if is_wayland()
        && let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        return;
    }
    CLIPBOARD.with(|cb_cell| {
        let mut cb = cb_cell.borrow_mut();
        if cb.is_none() {
            *cb = arboard::Clipboard::new().ok();
        }
        if let Some(clipboard) = cb.as_mut() {
            let _ = clipboard.set_text(text);
        }
    });
}

pub fn read_system_clipboard() -> Option<String> {
    #[cfg(test)]
    if let Some(text) = TEST_CLIPBOARD.with(|clipboard| clipboard.borrow().clone()) {
        return Some(text);
    }
    if is_wayland()
        && let Ok(out) = Command::new("wl-paste").arg("--no-newline").output()
        && out.status.success()
    {
        return Some(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    CLIPBOARD.with(|cb_cell| {
        let mut cb = cb_cell.borrow_mut();
        if cb.is_none() {
            *cb = arboard::Clipboard::new().ok();
        }
        cb.as_mut().and_then(|c| c.get_text().ok())
    })
}

pub(crate) trait TextEditTarget {
    fn select_all(&mut self);
    fn has_selection(&self) -> bool;
    fn copy(&mut self);
    fn yank_text(&self) -> String;
    fn cut(&mut self) -> bool;
    fn insert_str(&mut self, text: String) -> bool;
    fn paste(&mut self) -> bool;
    fn undo(&mut self) -> bool;
    fn redo(&mut self) -> bool;
    fn delete_word(&mut self) -> bool;
    fn delete_next_word(&mut self) -> bool;
    fn move_cursor(&mut self, movement: CursorMove);
    fn start_selection(&mut self);
    fn cancel_selection(&mut self);
}

impl TextEditTarget for TextArea<'static> {
    fn select_all(&mut self) {
        self.select_all();
    }
    fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }
    fn copy(&mut self) {
        self.copy();
    }
    fn yank_text(&self) -> String {
        self.yank_text()
    }
    fn cut(&mut self) -> bool {
        self.cut()
    }
    fn insert_str(&mut self, text: String) -> bool {
        self.insert_str(text)
    }
    fn paste(&mut self) -> bool {
        self.paste()
    }
    fn undo(&mut self) -> bool {
        self.undo()
    }
    fn redo(&mut self) -> bool {
        self.redo()
    }
    fn delete_word(&mut self) -> bool {
        self.delete_word()
    }
    fn delete_next_word(&mut self) -> bool {
        self.delete_next_word()
    }
    fn move_cursor(&mut self, movement: CursorMove) {
        self.move_cursor(movement);
    }
    fn start_selection(&mut self) {
        self.start_selection();
    }
    fn cancel_selection(&mut self) {
        self.cancel_selection();
    }
}

impl TextEditTarget for EditorDocument {
    fn select_all(&mut self) {
        self.select_all();
    }
    fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }
    fn copy(&mut self) {
        self.copy();
    }
    fn yank_text(&self) -> String {
        self.yank_text()
    }
    fn cut(&mut self) -> bool {
        self.cut().content_changed
    }
    fn insert_str(&mut self, text: String) -> bool {
        self.insert_str(text).content_changed
    }
    fn paste(&mut self) -> bool {
        self.paste().content_changed
    }
    fn undo(&mut self) -> bool {
        self.undo().content_changed
    }
    fn redo(&mut self) -> bool {
        self.redo().content_changed
    }
    fn delete_word(&mut self) -> bool {
        self.delete_word().content_changed
    }
    fn delete_next_word(&mut self) -> bool {
        self.delete_next_word().content_changed
    }
    fn move_cursor(&mut self, movement: CursorMove) {
        self.move_cursor(movement);
    }
    fn start_selection(&mut self) {
        self.start_selection();
    }
    fn cancel_selection(&mut self) {
        self.cancel_selection();
    }
}

pub(crate) fn copy_mouse_selection<T: TextEditTarget>(target: &mut T) -> Option<&'static str> {
    if !target.has_selection() {
        return None;
    }
    target.copy();
    write_system_clipboard(&target.yank_text());
    Some("Copied to clipboard")
}

/// Extend or collapse the selection for a cursor-movement key before it is fed
/// to the text target. SHIFT+move (optionally with CTRL for word-wise moves)
/// starts a selection if none is active; an unmodified move collapses it.
pub(crate) fn update_selection_for_move<T: TextEditTarget>(target: &mut T, key: &KeyEvent) {
    if !matches!(
        key.code,
        KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
    ) {
        return;
    }
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let rest = key.modifiers - KeyModifiers::SHIFT;
    if shift && (rest.is_empty() || rest == KeyModifiers::CONTROL) {
        if !target.has_selection() {
            target.start_selection();
        }
    } else if key.modifiers.is_empty() {
        target.cancel_selection();
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct MouseTextSelection {
    pub active: bool,
    pub dragged: bool,
}

impl MouseTextSelection {
    pub fn begin<T: TextEditTarget>(&mut self, target: &mut T) {
        target.start_selection();
        self.active = true;
        self.dragged = false;
    }

    pub fn mark_drag(&mut self) {
        if self.active {
            self.dragged = true;
        }
    }

    pub fn finish<T: TextEditTarget>(
        &mut self,
        target: &mut T,
        copy_on_select: bool,
    ) -> Option<&'static str> {
        if !self.active {
            return None;
        }
        let dragged = self.dragged;
        self.active = false;
        self.dragged = false;
        if !dragged {
            target.cancel_selection();
            return None;
        }
        if !copy_on_select {
            return None;
        }
        copy_mouse_selection(target)
    }
}

/// Apply keybind shortcuts; if none matched, feed the key to the textarea.
pub(crate) fn feed_key(keybinds: &Keybinds, input: &mut TextArea<'static>, key: KeyEvent) {
    if !apply_text_shortcuts(keybinds, input, key) {
        input.input(ratatui_textarea::Input::from(key));
    }
}

pub(crate) fn apply_text_shortcuts<T: TextEditTarget>(
    keybinds: &Keybinds,
    target: &mut T,
    key: KeyEvent,
) -> bool {
    if keybinds.matches_edit(EditAction::SelectAll, &key) {
        target.select_all();
        return true;
    }
    if keybinds.matches_edit(EditAction::Copy, &key) {
        if let Some(notice) = copy_mouse_selection(target) {
            set_clipboard_notice(notice);
        } else {
            set_clipboard_notice("Nothing selected");
        }
        return true;
    }
    if keybinds.matches_edit(EditAction::Cut, &key) {
        if target.cut() {
            write_system_clipboard(&target.yank_text());
            set_clipboard_notice("Cut to clipboard");
        } else {
            set_clipboard_notice("Nothing selected");
        }
        return true;
    }
    if keybinds.matches_edit(EditAction::Paste, &key) {
        match read_system_clipboard() {
            Some(text) if !text.is_empty() => {
                target.insert_str(text);
                set_clipboard_notice("Pasted from clipboard");
            }
            _ if target.paste() => set_clipboard_notice("Pasted from clipboard"),
            _ => {}
        }
        return true;
    }
    if keybinds.matches_edit(EditAction::Undo, &key) {
        let _ = target.undo();
        return true;
    }
    if keybinds.matches_edit(EditAction::Redo, &key) {
        let _ = target.redo();
        return true;
    }
    if keybinds.matches_edit(EditAction::DeleteWord, &key) {
        let _ = target.delete_word();
        return true;
    }
    if keybinds.matches_edit(EditAction::DeleteNextWord, &key) {
        let _ = target.delete_next_word();
        return true;
    }
    if keybinds.matches_edit(EditAction::MoveToTop, &key) {
        target.move_cursor(CursorMove::Top);
        return true;
    }
    if keybinds.matches_edit(EditAction::MoveToBottom, &key) {
        target.move_cursor(CursorMove::Bottom);
        return true;
    }
    // Kitty reserves Ctrl+Shift+Left/Right for tab switching. Offer a word
    // selection alias without overriding configured edit shortcuts.
    if key.modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT {
        let movement = match key.code {
            KeyCode::Left => CursorMove::WordBack,
            KeyCode::Right => CursorMove::WordForward,
            _ => return false,
        };
        if !target.has_selection() {
            target.start_selection();
        }
        target.move_cursor(movement);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn text_area(text: &str) -> TextArea<'static> {
        TextArea::new(vec![text.to_owned()])
    }

    #[test]
    fn mouse_selection_lifecycle_copies_drag_and_cancels_click() {
        let mut area = text_area("hello world");
        area.move_cursor(CursorMove::End);
        let mut selection = MouseTextSelection::default();
        selection.begin(&mut area);
        area.move_cursor(CursorMove::WordBack);
        selection.mark_drag();
        assert_eq!(
            selection.finish(&mut area, true),
            Some("Copied to clipboard")
        );

        selection.begin(&mut area);
        assert_eq!(selection.finish(&mut area, true), None);
        assert!(!area.has_selection());
    }

    #[test]
    fn mouse_selection_without_copy_keeps_selection() {
        let mut area = text_area("hello world");
        area.move_cursor(CursorMove::End);
        let mut selection = MouseTextSelection::default();
        selection.begin(&mut area);
        area.move_cursor(CursorMove::WordBack);
        selection.mark_drag();
        assert_eq!(selection.finish(&mut area, false), None);
        assert!(area.has_selection());
        area.copy();
        assert_eq!(area.yank_text(), "world");
    }

    #[test]
    fn keyboard_shift_move_selects_and_plain_move_collapses() {
        let mut area = text_area("hello world");
        let shift_right = KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT);
        for _ in 0..5 {
            update_selection_for_move(&mut area, &shift_right);
            area.input(ratatui_textarea::Input::from(shift_right));
        }
        assert!(area.has_selection());
        area.copy();
        assert_eq!(area.yank_text(), "hello");
    }

    #[test]
    fn keyboard_word_selection_in_title_and_body() {
        use crossterm::event::KeyEventKind;

        let keybinds = Keybinds::default();
        for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            for (direction, start, ranges) in [
                (KeyCode::Right, CursorMove::Head, [(0, 4), (0, 8)]),
                (KeyCode::Left, CursorMove::End, [(8, 13), (4, 13)]),
            ] {
                let mut title = text_area("one two three");
                let mut body = EditorDocument::from_lines(["one two three".to_owned()]);
                title.move_cursor(start);
                body.move_cursor(start);
                let revision = body.revision();

                for (kind, expected) in [KeyEventKind::Press, KeyEventKind::Repeat]
                    .into_iter()
                    .zip(ranges)
                {
                    let key =
                        KeyEvent::new_with_kind(direction, modifier | KeyModifiers::SHIFT, kind);
                    update_selection_for_move(&mut title, &key);
                    feed_key(&keybinds, &mut title, key);
                    update_selection_for_move(&mut body, &key);
                    if !apply_text_shortcuts(&keybinds, &mut body, key) {
                        body.input(ratatui_textarea::Input::from(key));
                    }
                    assert_eq!(
                        title.selection_range(),
                        Some(((0, expected.0), (0, expected.1))),
                        "{key:?}",
                    );
                    assert_eq!(
                        body.selection_range()
                            .map(|(start, end)| { ((start.row, start.col), (end.row, end.col)) }),
                        title.selection_range(),
                        "{key:?}",
                    );
                    assert_eq!(body.revision(), revision);
                    assert_eq!(title.lines(), ["one two three"]);
                }

                let key = KeyEvent::new(direction, KeyModifiers::NONE);
                update_selection_for_move(&mut title, &key);
                feed_key(&keybinds, &mut title, key);
                update_selection_for_move(&mut body, &key);
                body.input(ratatui_textarea::Input::from(key));
                assert!(!title.has_selection());
                assert!(!body.has_selection());
                assert_eq!(body.revision(), revision);
            }
        }
    }

    #[test]
    fn shortcut_copy_without_selection_reports_notice() {
        let mut area = text_area("hello");
        assert!(apply_text_shortcuts(
            &Keybinds::default(),
            &mut area,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ));
        assert_eq!(take_clipboard_notice(), Some("Nothing selected"));
    }
    #[test]
    fn copy_mouse_selection_ignores_empty_selection() {
        let mut area = text_area("hello");
        assert_eq!(copy_mouse_selection(&mut area), None);
    }

    #[test]
    fn shortcut_copy_sets_notice() {
        let mut area = text_area("hello");
        area.select_all();
        assert!(apply_text_shortcuts(
            &Keybinds::default(),
            &mut area,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ));
        assert_eq!(take_clipboard_notice(), Some("Copied to clipboard"));
    }

    #[test]
    fn shortcut_cut_without_selection_reports_notice() {
        let mut area = text_area("hello");
        assert!(apply_text_shortcuts(
            &Keybinds::default(),
            &mut area,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        ));
        assert_eq!(take_clipboard_notice(), Some("Nothing selected"));
    }
}
