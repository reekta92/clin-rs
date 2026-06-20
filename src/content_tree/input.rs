use crate::config::ClinConfig;
use crate::content_tree::state::ContentTreeState;
use crate::keybinds::{ContentTreeAction, Keybinds};
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

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
    config: &ClinConfig,
) -> InputResult {
    let seq = config.core.enable_key_sequences;
    match keybinds.resolve_content_tree(&mut state.seq_matcher, key, seq) {
        crate::keybinds::MatchOutcome::Matched(action) => match action {
            ContentTreeAction::Back => return InputResult::Back,
            ContentTreeAction::Open => return InputResult::Open,
            ContentTreeAction::Help => return InputResult::Help,
            ContentTreeAction::MoveUp => {
                state.move_up();
                return InputResult::None;
            }
            ContentTreeAction::MoveDown => {
                state.move_down();
                return InputResult::None;
            }
            ContentTreeAction::ToggleCollapse => {
                state.toggle_collapse();
                return InputResult::None;
            }
            ContentTreeAction::ExpandAll => {
                state.expand_all();
                return InputResult::None;
            }
            ContentTreeAction::CollapseAll => {
                state.collapse_all();
                return InputResult::None;
            }
        },
        crate::keybinds::MatchOutcome::Pending => return InputResult::None,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }

    InputResult::None
}
