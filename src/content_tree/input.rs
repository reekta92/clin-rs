use crate::config::ClinConfig;
use crate::content_tree::state::ContentTreeState;
use crate::keybinds::{ContentTreeAction, Keybinds};
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

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
    _area: Rect,
    scrollbars_enabled: bool,
) -> ContentTreeInput {
    // --- Scrollbar handling (drag) ---
    if scrollbars_enabled && let Some(meta) = state.last_tree_scroll {
        let viewport = meta.viewport_len;
        let max_pos = meta.content_len.saturating_sub(viewport);
        let frac = state.tree_scroll_offset as f32 / max_pos.max(1) as f32;
        if let Some(new_frac) =
            crate::ui::scrollbar::handle_scrollbar_mouse(&mouse, meta, frac, &mut state.scroll_drag)
        {
            state.tree_scroll_offset = ((new_frac * max_pos as f32).round() as usize).min(max_pos);
            let visible = state.visible_indices();
            let len = visible.len();
            if let Some(pos) = visible.iter().position(|&x| x == state.selected) {
                state.selected = visible[crate::ui::clamp_selected_to_view(
                    pos,
                    state.tree_scroll_offset,
                    len,
                    viewport,
                )];
            }
            return ContentTreeInput::None;
        }
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.wheel_scroll(-1);
            return ContentTreeInput::None;
        }
        MouseEventKind::ScrollDown => {
            state.wheel_scroll(1);
            return ContentTreeInput::None;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let list_rect = state.tree_list_rect;
            if !crate::events::contains_cell(list_rect, mouse.column, mouse.row) {
                return ContentTreeInput::None;
            }
            let visible = state.visible_indices();
            let Some(rel) = crate::ui::list_index_at(
                mouse.row,
                list_rect.y,
                1,
                state.tree_scroll_offset,
                visible.len(),
            ) else {
                return ContentTreeInput::None;
            };
            let node_idx = visible[rel];
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
