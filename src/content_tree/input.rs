use crate::content_tree::state::ContentTreeState;
use crate::keybinds::{Keybinds, ContentTreeAction};
use crossterm::event::KeyEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputResult {
    None,
    Open,
    Back,
    Help,
}

pub fn handle_input(
    state: &mut ContentTreeState,
    key: KeyEvent,
    keybinds: &Keybinds,
) -> InputResult {
    if keybinds.matches_content_tree(ContentTreeAction::Back, &key) {
        return InputResult::Back;
    }
    if keybinds.matches_content_tree(ContentTreeAction::Open, &key) {
        return InputResult::Open;
    }
    if keybinds.matches_content_tree(ContentTreeAction::Help, &key) {
        return InputResult::Help;
    }
    if keybinds.matches_content_tree(ContentTreeAction::MoveUp, &key) {
        state.move_up();
        return InputResult::None;
    }
    if keybinds.matches_content_tree(ContentTreeAction::MoveDown, &key) {
        state.move_down();
        return InputResult::None;
    }
    if keybinds.matches_content_tree(ContentTreeAction::ToggleCollapse, &key) {
        state.toggle_collapse();
        return InputResult::None;
    }
    if keybinds.matches_content_tree(ContentTreeAction::ExpandAll, &key) {
        state.expand_all();
        return InputResult::None;
    }
    if keybinds.matches_content_tree(ContentTreeAction::CollapseAll, &key) {
        state.collapse_all();
        return InputResult::None;
    }

    InputResult::None
}
