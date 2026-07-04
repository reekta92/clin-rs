use crate::base_view::state::BaseState;
use crate::config::ClinConfig;
use crate::keybinds::{BaseAction, Keybinds};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseInput {
    None,
    Open,
    Back,
    Help,
    Refresh,
    NewNote,
}

fn run_common_action(
    state: &mut BaseState,
    action: BaseAction,
    count: Option<u32>,
) -> Option<BaseInput> {
    match action {
        BaseAction::Back => Some(BaseInput::Back),
        BaseAction::Open => Some(BaseInput::Open),
        BaseAction::Help => Some(BaseInput::Help),
        BaseAction::Refresh => {
            state.refresh();
            Some(BaseInput::None)
        }
        BaseAction::MoveUp => {
            for _ in 0..count.unwrap_or(1) as usize {
                state.move_up();
            }
            Some(BaseInput::None)
        }
        BaseAction::MoveDown => {
            for _ in 0..count.unwrap_or(1) as usize {
                state.move_down();
            }
            Some(BaseInput::None)
        }
        BaseAction::CycleView => {
            state.cycle_view();
            Some(BaseInput::None)
        }
        BaseAction::CycleMarker => {
            state.cycle_list_marker();
            Some(BaseInput::None)
        }
        BaseAction::EditBase => {
            state.start_raw_edit();
            Some(BaseInput::None)
        }
        BaseAction::ExportCsv => {
            match state.export_csv() {
                Ok(p) => state.status = Some(format!("Exported to {}", p.display())),
                Err(e) => state.status = Some(format!("Export failed: {e}")),
            }
            Some(BaseInput::None)
        }
        BaseAction::CopyTable => {
            let n = state.copy_table();
            state.status = Some(format!("Copied {} rows", n));
            Some(BaseInput::None)
        }
        BaseAction::PageUp => {
            for _ in 0..count.unwrap_or(1) as usize {
                state.page_up();
            }
            Some(BaseInput::None)
        }
        BaseAction::PageDown => {
            for _ in 0..count.unwrap_or(1) as usize {
                state.page_down();
            }
            Some(BaseInput::None)
        }
        BaseAction::JumpToTop => {
            state.jump_to_top();
            Some(BaseInput::None)
        }
        BaseAction::JumpToBottom => {
            state.jump_to_bottom();
            Some(BaseInput::None)
        }
        BaseAction::NewNote => Some(BaseInput::NewNote),
        _ => None, // table-only actions (MoveLeft/Right/EditCell/CommitEdit/CancelEdit/SortAsc/SortDesc/NewBase/SaveBase)
    }
}

pub fn handle_base_mouse(state: &mut BaseState, mouse: MouseEvent, area: Rect) -> BaseInput {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return BaseInput::None;
    }

    let vt = state.active_view().map(|v| v.r#type);
    match vt {
        Some(crate::base::model::ViewType::List) => {
            if let Some(row) = crate::base_view::render::hit_test_list(state, area, mouse.row) {
                state.cursor_row = row;
            }
        }
        Some(crate::base::model::ViewType::Cards) => {
            if let Some(row) =
                crate::base_view::render::hit_test_cards(state, area, mouse.row, mouse.column)
            {
                state.cursor_row = row;
            }
        }
        Some(crate::base::model::ViewType::Map) => {} // keyboard-only
        _ => {
            if let Some((row, col)) =
                crate::base_view::render::hit_test(state, area, mouse.row, mouse.column)
            {
                state.cursor_row = row;
                state.cursor_col = col;
            }
        }
    }
    BaseInput::None
}

pub fn handle_input(
    state: &mut BaseState,
    key: KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
) -> BaseInput {
    // Raw edit overlay gets first priority
    if state.raw_edit.is_some() {
        // Intercept only save/cancel by specific action. Do NOT call resolve_base
        // here — it would consume typing keys (r, s, e, j, k, ...) as base actions.
        if keybinds.matches_base(BaseAction::SaveBase, &key) {
            if let Err(e) = state.save_raw_edit() {
                state.status = Some(format!("Invalid base: {}", e));
            }
            return BaseInput::None;
        }
        if keybinds.matches_base(BaseAction::CancelEdit, &key) {
            state.cancel_raw_edit();
            return BaseInput::None;
        }
        // Everything else goes to the textarea: try clipboard/cursor shortcuts,
        // and if none matched, insert the keystroke.
        if let Some(ta) = &mut state.raw_edit
            && !crate::text_edit::apply_text_shortcuts(keybinds, ta, key)
        {
            ta.input(key);
        }
        return BaseInput::None;
    }
    if state.edit.is_some() {
        match key.code {
            KeyCode::Enter => {
                let _ = state.commit_edit();
            }
            KeyCode::Esc => {
                state.cancel_edit();
            }
            _ => {
                if let Some(edit) = &mut state.edit {
                    edit.input.input(key);
                }
            }
        }
        return BaseInput::None;
    }

    // In single-row layouts (List, Cards, Map), only allow row navigation and view-level actions
    if state.active_view().is_some_and(|v| {
        matches!(
            v.r#type,
            crate::base::model::ViewType::List
                | crate::base::model::ViewType::Cards
                | crate::base::model::ViewType::Map
        )
    }) {
        let seq = config.sequences_enabled();
        let counts = config.counts_enabled();
        match keybinds.resolve_base(&mut state.seq_matcher, key, seq, counts) {
            crate::keybinds::MatchOutcome::Matched(action, count) => {
                if let Some(input) = run_common_action(state, action, count) {
                    return input;
                }
                // table-only actions are ignored in List/Cards/Map
            }
            crate::keybinds::MatchOutcome::Pending => return BaseInput::None,
            crate::keybinds::MatchOutcome::NoMatch => {}
        }
        return BaseInput::None;
    }

    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    match keybinds.resolve_base(&mut state.seq_matcher, key, seq, counts) {
        crate::keybinds::MatchOutcome::Matched(action, count) => {
            if let Some(input) = run_common_action(state, action, count) {
                return input;
            }
            // table-only actions:
            match action {
                BaseAction::MoveLeft => {
                    for _ in 0..count.unwrap_or(1) as usize {
                        state.move_left();
                    }
                }
                BaseAction::MoveRight => {
                    for _ in 0..count.unwrap_or(1) as usize {
                        state.move_right();
                    }
                }
                BaseAction::EditCell => {
                    state.start_edit();
                }
                BaseAction::CommitEdit => {
                    let _ = state.commit_edit();
                }
                BaseAction::CancelEdit => {
                    state.cancel_edit();
                }
                BaseAction::SortAsc => {
                    state.set_sort(state.cursor_col, crate::base::model::SortDirection::Asc);
                }
                BaseAction::SortDesc => {
                    state.set_sort(state.cursor_col, crate::base::model::SortDirection::Desc);
                }
                BaseAction::NewBase => {}  // palette-only; no-op in-view
                BaseAction::SaveBase => {} // only meaningful in raw-edit mode (handled earlier)
                _ => {}
            }
        }
        crate::keybinds::MatchOutcome::Pending => return BaseInput::None,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }

    BaseInput::None
}
