//! Setup wizard input: row navigation, value cycling, Done activation, and an
//! inline Esc→confirm overlay. No text inputs.

use crate::app::App;
use crate::keybinds::{MatchOutcome, SetupAction};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn handle_setup_keys(app: &mut App, key: KeyEvent) -> bool {
    // Esc→confirm overlay absorbs all keys until resolved.
    if let Some(state) = app.setup_state.as_mut()
        && state.confirm_exit
    {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                app.finish_setup();
                return true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Char('q') => {
                state.confirm_exit = false;
                return true;
            }
            _ => return true,
        }
    }

    let seq = app.config.sequences_enabled();
    let action = if let MatchOutcome::Matched(act, _) =
        app.keybinds
            .resolve_setup(&mut app.seq_matcher, key, seq, false)
    {
        Some(act)
    } else {
        None
    };

    let Some(act) = action else {
        return true;
    };

    match act {
        SetupAction::Up => {
            if let Some(state) = app.setup_state.as_mut() {
                state.move_sel(false);
            }
        }
        SetupAction::Down => {
            if let Some(state) = app.setup_state.as_mut() {
                state.move_sel(true);
            }
        }
        SetupAction::CycleNext | SetupAction::Activate => {
            let finish = app
                .setup_state
                .as_ref()
                .map(|s| s.is_done_selected())
                .unwrap_or(false);
            if finish {
                app.finish_setup();
            } else if let Some(state) = app.setup_state.as_mut() {
                state.cycle(true);
                app.apply_setup_live();
            }
        }
        SetupAction::CyclePrev => {
            if let Some(state) = app.setup_state.as_mut() {
                state.cycle(false);
                app.apply_setup_live();
            }
        }
        SetupAction::Finish => {
            if let Some(state) = app.setup_state.as_mut() {
                state.confirm_exit = true;
            }
        }
    }
    true
}

pub fn handle_setup_mouse(app: &mut App, mouse: MouseEvent, terminal_area: Rect) {
    if !app.config.core.mouse_enabled {
        return;
    }

    let layout = crate::ui::setup::setup_layout(terminal_area);

    enum MouseAction {
        Finish,
        CycleOption(usize),
        MoveSel(bool),
    }
    let action = match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if crate::events::contains_cell(layout.done, mouse.column, mouse.row) {
                Some(MouseAction::Finish)
            } else if crate::events::contains_cell(layout.options, mouse.column, mouse.row) {
                let row_index = mouse
                    .row
                    .saturating_sub(layout.options.y)
                    .min((crate::setup::OPTION_ROWS - 1) as u16);
                Some(MouseAction::CycleOption(row_index as usize))
            } else {
                None
            }
        }
        MouseEventKind::ScrollDown => Some(MouseAction::MoveSel(true)),
        MouseEventKind::ScrollUp => Some(MouseAction::MoveSel(false)),
        _ => None,
    };

    let Some(action) = action else {
        return;
    };

    // Resolve selection mutation while borrowing state, then release before
    // calling App methods that need &mut self.
    let finish = matches!(action, MouseAction::Finish);
    if let Some(state) = app.setup_state.as_mut() {
        match action {
            MouseAction::Finish => state.selected = crate::setup::DONE_ROW,
            MouseAction::CycleOption(row) => {
                state.selected = row;
                state.cycle(true);
            }
            MouseAction::MoveSel(down) => state.move_sel(down),
        }
    }

    if finish {
        app.finish_setup();
    } else {
        app.apply_setup_live();
    }
}
