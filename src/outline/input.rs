use crate::config::ClinConfig;
use crate::keybinds::{Keybinds, OutlineAction};
use crate::outline::state::OutlineState;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineInput {
    None,
    Open,
    Back,
    Help,
}

pub fn handle_outline_mouse(
    state: &mut OutlineState,
    mouse: MouseEvent,
    _area: Rect,
    scrollbars_enabled: bool,
) -> OutlineInput {
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
            return OutlineInput::None;
        }
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.wheel_scroll(-1);
            return OutlineInput::None;
        }
        MouseEventKind::ScrollDown => {
            state.wheel_scroll(1);
            return OutlineInput::None;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let list_rect = state.tree_list_rect;
            if !crate::events::contains_cell(list_rect, mouse.column, mouse.row) {
                return OutlineInput::None;
            }
            let visible = state.visible_indices();
            let Some(rel) = crate::ui::list_index_at(
                mouse.row,
                list_rect.y,
                1,
                state.tree_scroll_offset,
                visible.len(),
            ) else {
                return OutlineInput::None;
            };
            let node_idx = visible[rel];
            let was_selected = state.selected == node_idx;
            state.selected = node_idx;
            if state.is_header(node_idx) && state.nodes[node_idx].has_children {
                if was_selected {
                    state.toggle_collapse();
                }
            } else if was_selected {
                return OutlineInput::Open;
            }
        }
        _ => {}
    }

    OutlineInput::None
}

pub fn handle_input(
    state: &mut OutlineState,
    key: KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
) -> OutlineInput {
    if crate::events::is_universal_quit_key(&key) {
        return OutlineInput::Back;
    }

    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    match keybinds.resolve_outline(&mut state.seq_matcher, key, seq, counts) {
        crate::keybinds::MatchOutcome::Matched(action, count) => match action {
            OutlineAction::Back => return OutlineInput::Back,
            OutlineAction::Open => return OutlineInput::Open,
            OutlineAction::Help => return OutlineInput::Help,
            OutlineAction::MoveUp => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.move_up();
                }
                return OutlineInput::None;
            }
            OutlineAction::MoveDown => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.move_down();
                }
                return OutlineInput::None;
            }
            OutlineAction::ToggleCollapse => {
                state.toggle_collapse();
                return OutlineInput::None;
            }
            OutlineAction::ExpandAll => {
                state.expand_all();
                return OutlineInput::None;
            }
            OutlineAction::CollapseAll => {
                state.collapse_all();
                return OutlineInput::None;
            }
        },
        crate::keybinds::MatchOutcome::Pending => return OutlineInput::None,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }

    OutlineInput::None
}
