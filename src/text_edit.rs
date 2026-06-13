use crate::keybinds::{EditAction, Keybinds};
use crossterm::event::KeyEvent;
use ratatui_textarea::{CursorMove, TextArea};

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
