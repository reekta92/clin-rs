use crate::keybinds::{EditAction, Keybinds};
use crossterm::event::KeyEvent;
use ratatui_textarea::{CursorMove, TextArea};
use std::cell::RefCell;

thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

pub fn write_system_clipboard(text: &str) {
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
    CLIPBOARD.with(|cb_cell| {
        let mut cb = cb_cell.borrow_mut();
        if cb.is_none() {
            *cb = arboard::Clipboard::new().ok();
        }
        cb.as_mut().and_then(|c| c.get_text().ok())
    })
}

pub fn apply_text_shortcuts(
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
        CLIPBOARD.with(|cb_cell| {
            let mut cb = cb_cell.borrow_mut();
            if cb.is_none() {
                *cb = arboard::Clipboard::new().ok();
            }
            if let Some(clipboard) = cb.as_mut() {
                let _ = clipboard.set_text(textarea.yank_text());
            }
        });
        return true;
    }
    if keybinds.matches_edit(EditAction::Cut, &key) {
        if textarea.cut() {
            let mut wrote = false;
            CLIPBOARD.with(|cb_cell| {
                let mut cb = cb_cell.borrow_mut();
                if cb.is_none() {
                    *cb = arboard::Clipboard::new().ok();
                }
                if let Some(clipboard) = cb.as_mut() {
                    let _ = clipboard.set_text(textarea.yank_text());
                    wrote = true;
                }
            });
            if !wrote {
                // fallback: try ephemeral clipboard
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(textarea.yank_text());
                }
            }
        }
        return true;
    }
    if keybinds.matches_edit(EditAction::Paste, &key) {
        let mut pasted = false;
        CLIPBOARD.with(|cb_cell| {
            let mut cb = cb_cell.borrow_mut();
            if cb.is_none() {
                *cb = arboard::Clipboard::new().ok();
            }
            if let Some(clipboard) = cb.as_mut()
                && let Ok(text) = clipboard.get_text()
            {
                textarea.insert_str(text);
                pasted = true;
            }
        });
        if !pasted {
            // fallback: try ephemeral clipboard
            if let Ok(mut cb) = arboard::Clipboard::new()
                && let Ok(text) = cb.get_text()
            {
                textarea.insert_str(text);
                pasted = true;
            }
        }
        if !pasted {
            let _ = textarea.paste();
        }
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
