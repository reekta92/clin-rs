use crate::editor_document::EditorDocument;
use crate::keybinds::{EditAction, Keybinds};
use crossterm::event::KeyEvent;
use ratatui_textarea::{CursorMove, TextArea};
use std::cell::RefCell;
use std::io::Write;
use std::process::{Command, Stdio};

thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
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
    if is_wayland()
        && let Ok(out) = Command::new("wl-paste").output()
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
        if target.has_selection() {
            target.copy();
            write_system_clipboard(&target.yank_text());
            set_clipboard_notice("Copied to clipboard");
        }
        return true;
    }
    if keybinds.matches_edit(EditAction::Cut, &key) {
        if target.cut() {
            write_system_clipboard(&target.yank_text());
            set_clipboard_notice("Cut to clipboard");
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
    false
}

pub(crate) fn apply_context_menu_action<T: TextEditTarget>(
    target: &mut T,
    label: &str,
) -> Option<&'static str> {
    match label {
        " Copy " if target.has_selection() => {
            target.copy();
            write_system_clipboard(&target.yank_text());
            Some("Copied to clipboard")
        }
        " Cut " if target.cut() => {
            write_system_clipboard(&target.yank_text());
            Some("Cut to clipboard")
        }
        " Paste " => read_system_clipboard()
            .filter(|text| !text.is_empty())
            .map(|text| {
                target.insert_str(text);
                "Pasted from clipboard"
            }),
        " Select All " => {
            target.select_all();
            None
        }
        _ => None,
    }
}
