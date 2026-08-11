//! Setup wizard input: row navigation, value cycling, Done activation, and an
//! inline Esc→confirm overlay. No text inputs.

use crate::app::App;
use crate::keybinds::{MatchOutcome, SetupAction};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn handle_setup_keys(app: &mut App, key: KeyEvent) {
    if let Some(modal) = app
        .setup_state
        .as_mut()
        .and_then(|state| state.vault_modal.as_mut())
    {
        match modal {
            crate::setup::SetupVaultModal::PathInput { input, .. } => match key.code {
                KeyCode::Esc => {
                    app.setup_state.as_mut().unwrap().vault_modal = None;
                    return;
                }
                KeyCode::Enter => {
                    let value = input.lines().join("\n");
                    let path = crate::setup::validate_vault_path(&value);
                    match path {
                        Ok(path) => app.select_setup_vault(path),
                        Err(error) => {
                            if let Some(state) = app.setup_state.as_mut() {
                                state.vault_error = Some(error.to_string());
                            }
                        }
                    }
                    return;
                }
                _ => {
                    crate::events::handle_popup_text_input(key, input, &app.keybinds);
                    return;
                }
            },
            crate::setup::SetupVaultModal::ConfirmNonEmpty { path } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    let path = path.clone();
                    if let Some(state) = app.setup_state.as_mut() {
                        state.vault_path = path.clone();
                        state.confirmed_nonempty_path = Some(path);
                        state.vault_modal = None;
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.setup_state.as_mut().unwrap().vault_modal = None;
                }
                _ => {}
            },
        }
        return;
    }

    // Esc→confirm overlay absorbs all keys until resolved.
    if let Some(state) = app.setup_state.as_mut()
        && state.confirm_exit
    {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                app.finish_setup();
                return;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app.abort_setup();
                return;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                state.confirm_exit = false;
                return;
            }
            _ => return,
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
        return;
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
        SetupAction::Activate => {
            let (finish, vault_selected) = app
                .setup_state
                .as_ref()
                .map(|s| (s.is_done_selected(), s.vault_selected()))
                .unwrap_or((false, false));
            if finish {
                app.finish_setup();
            } else if vault_selected {
                app.begin_setup_vault_selection();
            } else if let Some(state) = app.setup_state.as_mut() {
                state.cycle(true);
                app.apply_setup_live();
            }
        }
        SetupAction::CycleNext => {
            let vault_selected = app
                .setup_state
                .as_ref()
                .is_some_and(crate::setup::SetupState::vault_selected);
            if vault_selected {
                app.begin_setup_vault_selection();
            } else if let Some(state) = app.setup_state.as_mut()
                && !state.is_done_selected()
            {
                state.cycle(true);
                app.apply_setup_live();
            }
        }
        SetupAction::CyclePrev => {
            if let Some(state) = app.setup_state.as_mut()
                && !state.vault_selected()
            {
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
}

pub fn handle_setup_mouse(app: &mut App, mouse: MouseEvent, terminal_area: Rect) {
    if !app.config.core.mouse_enabled {
        return;
    }
    if app
        .setup_state
        .as_ref()
        .is_some_and(|state| state.vault_modal.is_some() || state.confirm_exit)
    {
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
                if row != 0 || state.vault_cli_override {
                    state.cycle(true);
                }
            }
            MouseAction::MoveSel(down) => state.move_sel(down),
        }
    }

    if finish {
        app.finish_setup();
    } else if app
        .setup_state
        .as_ref()
        .is_some_and(crate::setup::SetupState::vault_selected)
    {
        app.begin_setup_vault_selection();
    } else {
        app.apply_setup_live();
    }
}
