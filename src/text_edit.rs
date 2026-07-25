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
        // ratatui-textarea 0.9 `copy()` is a no-op without a selection (verified
        // in registry source: guarded by `take_selection_positions`), so without
        // this guard `yank_text()` would return the *previous* yank and clobber
        // the system clipboard with stale text.
        if textarea.selection_range().is_some() {
            textarea.copy();
            write_system_clipboard(&textarea.yank_text());
            set_clipboard_notice("Copied to clipboard");
        }
        return true;
    }
    if keybinds.matches_edit(EditAction::Cut, &key) {
        if textarea.cut() {
            write_system_clipboard(&textarea.yank_text());
            set_clipboard_notice("Cut to clipboard");
        }
        return true;
    }
    if keybinds.matches_edit(EditAction::Paste, &key) {
        match read_system_clipboard() {
            Some(text) if !text.is_empty() => {
                textarea.insert_str(text);
                set_clipboard_notice("Pasted from clipboard");
            }
            // Empty or unavailable system clipboard: fall back to the
            // textarea's internal yank buffer (previous in-app kill).
            _ => {
                if textarea.paste() {
                    set_clipboard_notice("Pasted from clipboard");
                }
            }
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
