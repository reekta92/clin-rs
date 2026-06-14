use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::content_tree::state::ContentTreeState;
use crate::keybinds::{ContentTreeAction, Keybinds};
use crossterm::event::KeyEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputResult {
    None,
    Open,
    Back,
    Help,
}

pub fn handle_content_tree_mouse(
    state: &mut ContentTreeState,
    mouse: MouseEvent,
    area: Rect,
) -> InputResult {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(0),    // Tree + Side Pane
            Constraint::Length(1), // Hint line
        ])
        .split(area);

    let main_area = chunks[1];

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(45, 100), // Left: Tree
            Constraint::Length(1),      // Separator
            Constraint::Min(0),         // Right: Full Content
        ])
        .split(main_area);

    let left_area = content_chunks[0];

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.move_up();
            return InputResult::None;
        }
        MouseEventKind::ScrollDown => {
            state.move_down();
            return InputResult::None;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if !crate::events::contains_cell(left_area, mouse.column, mouse.row) {
                return InputResult::None;
            }

            let visible = state.visible_indices();
            let row = mouse.row.saturating_sub(left_area.y) as usize;

            if row >= visible.len() {
                return InputResult::None;
            }

            let node_idx = visible[row];
            let was_selected = state.selected == node_idx;
            state.selected = node_idx;

            if state.is_header(node_idx) && state.nodes[node_idx].has_children {
                if was_selected {
                    state.toggle_collapse();
                }
            } else if was_selected {
                return InputResult::Open;
            }
        }
        _ => {}
    }

    InputResult::None
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
