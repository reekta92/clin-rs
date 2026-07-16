use crate::config::ClinConfig;
use crate::content_tree::state::ContentTreeState;
use crate::keybinds::{ContentTreeAction, Keybinds};
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentTreeInput {
    None,
    Open,
    Back,
    Help,
}

pub fn handle_content_tree_mouse(
    state: &mut ContentTreeState,
    mouse: MouseEvent,
    area: Rect,
    scrollbars_enabled: bool,
) -> ContentTreeInput {
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
    // --- Scrollbar handling ---
    if scrollbars_enabled && let Some(meta) = state.last_tree_scroll {
        let max_pos = meta.content_len.saturating_sub(1);
        let frac = {
            let visible = state.visible_indices();
            let selected_pos = visible.iter().position(|&x| x == state.selected);
            selected_pos.unwrap_or(0) as f32 / max_pos.max(1) as f32
        };
        if let Some(new_frac) =
            crate::ui::scrollbar::handle_scrollbar_mouse(&mouse, meta, frac, &mut state.scroll_drag)
        {
            let vis_idx = (new_frac * max_pos as f32).round() as usize;
            let visible = state.visible_indices();
            if vis_idx < visible.len() {
                state.selected = visible[vis_idx];
            }
            return ContentTreeInput::None;
        }
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.move_up();
            return ContentTreeInput::None;
        }
        MouseEventKind::ScrollDown => {
            state.move_down();
            return ContentTreeInput::None;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if !crate::events::contains_cell(left_area, mouse.column, mouse.row) {
                return ContentTreeInput::None;
            }

            let visible = state.visible_indices();
            let row = mouse.row.saturating_sub(left_area.y) as usize;

            if row >= visible.len() {
                return ContentTreeInput::None;
            }

            let node_idx = visible[row];
            let was_selected = state.selected == node_idx;
            state.selected = node_idx;

            if state.is_header(node_idx) && state.nodes[node_idx].has_children {
                if was_selected {
                    state.toggle_collapse();
                }
            } else if was_selected {
                return ContentTreeInput::Open;
            }
        }
        _ => {}
    }

    ContentTreeInput::None
}

pub fn handle_input(
    state: &mut ContentTreeState,
    key: KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
) -> ContentTreeInput {
    if crate::events::is_universal_quit_key(&key) {
        return ContentTreeInput::Back;
    }

    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    match keybinds.resolve_content_tree(&mut state.seq_matcher, key, seq, counts) {
        crate::keybinds::MatchOutcome::Matched(action, count) => match action {
            ContentTreeAction::Back => return ContentTreeInput::Back,
            ContentTreeAction::Open => return ContentTreeInput::Open,
            ContentTreeAction::Help => return ContentTreeInput::Help,
            ContentTreeAction::MoveUp => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.move_up();
                }
                return ContentTreeInput::None;
            }
            ContentTreeAction::MoveDown => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.move_down();
                }
                return ContentTreeInput::None;
            }
            ContentTreeAction::ToggleCollapse => {
                state.toggle_collapse();
                return ContentTreeInput::None;
            }
            ContentTreeAction::ExpandAll => {
                state.expand_all();
                return ContentTreeInput::None;
            }
            ContentTreeAction::CollapseAll => {
                state.collapse_all();
                return ContentTreeInput::None;
            }
        },
        crate::keybinds::MatchOutcome::Pending => return ContentTreeInput::None,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }

    ContentTreeInput::None
}
