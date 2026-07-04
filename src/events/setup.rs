use crate::app::App;
use crate::keybinds::{MatchOutcome, SetupAction};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn handle_setup_keys(app: &mut App, key: KeyEvent) -> bool {
    let mut consumed = false;
    let mut action = None;

    if let Some(state) = app.setup_state.as_mut()
        && state.is_text_focused()
    {
        let is_nav = matches!(
            key.code,
            KeyCode::Esc
                | KeyCode::Enter
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Up
                | KeyCode::Down
        );
        if !is_nav && let Some(input) = state.focused_input_mut() {
            crate::events::handle_popup_text_input(key, input, &app.keybinds);
            consumed = true;
        }
    }

    if consumed {
        return true;
    }

    let mut toggle_action = None;
    if let Some(state) = app.setup_state.as_mut()
        && state.is_toggle_active()
    {
        if key.code == KeyCode::Char(' ') {
            toggle_action = Some(None);
        } else if key.code == KeyCode::Char('h') || key.code == KeyCode::Left {
            toggle_action = Some(Some(false));
        } else if key.code == KeyCode::Char('l') || key.code == KeyCode::Right {
            toggle_action = Some(Some(true));
        }
    }

    if let Some(opt) = toggle_action {
        if let Some(state) = app.setup_state.as_mut() {
            let new_val = match opt {
                Some(v) => v,
                None => match state.step {
                    1 => !state.background_solid,
                    3 => !state.mouse_enabled,
                    6 => !state.goals_enabled,
                    7 => !state.backup_enabled,
                    _ => false,
                },
            };
            match state.step {
                1 => state.background_solid = new_val,
                3 => state.mouse_enabled = new_val,
                6 => state.goals_enabled = new_val,
                7 => state.backup_enabled = new_val,
                _ => {}
            }
        }
        app.apply_setup_live();
        return true;
    }

    let seq = app.config.sequences_enabled();
    if let MatchOutcome::Matched(act, _) =
        app.keybinds
            .resolve_setup(&mut app.seq_matcher, key, seq, false)
    {
        action = Some(act);
    }

    if let Some(act) = action {
        match act {
            SetupAction::Finish => {
                app.finish_setup();
            }
            SetupAction::Next => {
                let mut finish = false;
                if let Some(state) = app.setup_state.as_mut() {
                    if state.step == crate::setup::SETUP_TOTAL_STEPS {
                        finish = true;
                    } else {
                        state.advance();
                    }
                }
                if finish {
                    app.finish_setup();
                } else {
                    app.apply_setup_live();
                }
            }
            SetupAction::Prev => {
                if let Some(state) = app.setup_state.as_mut() {
                    state.go_prev();
                }
            }
            SetupAction::Up => {
                if let Some(state) = app.setup_state.as_mut() {
                    state.move_cursor(false);
                }
                app.apply_setup_live();
            }
            SetupAction::Down => {
                if let Some(state) = app.setup_state.as_mut() {
                    state.move_cursor(true);
                }
                app.apply_setup_live();
            }
            SetupAction::ToggleField => {
                if let Some(state) = app.setup_state.as_mut() {
                    state.toggle_focus();
                }
                app.apply_setup_live();
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
    let mut needs_apply = false;

    // Use a scoped block to mutate state
    {
        let Some(state) = app.setup_state.as_mut() else {
            return;
        };

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(sidebar) = layout.sidebar
                    && crate::events::contains_cell(sidebar, mouse.column, mouse.row)
                {
                    let clicked = mouse.row.saturating_sub(sidebar.y).saturating_sub(1) as usize;
                    let clicked = clicked.min(crate::setup::SETUP_TOTAL_STEPS);
                    state.go_to_step(clicked);
                    needs_apply = true;
                } else if crate::events::contains_cell(layout.content, mouse.column, mouse.row) {
                    use ratatui::layout::{Constraint, Direction, Layout};
                    use ratatui::widgets::{Block, Borders};

                    match state.step {
                        1 => {
                            // Theme & Background
                            let chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Min(0), Constraint::Length(3)])
                                .split(layout.content);
                            if crate::events::contains_cell(chunks[0], mouse.column, mouse.row) {
                                state.focus = 0;
                                let clicked =
                                    (mouse.row.saturating_sub(chunks[0].y).saturating_sub(1))
                                        as usize;
                                let clicked = clicked.min(crate::setup::SETUP_THEMES.len() - 1);
                                state.cursor = clicked;
                                state.theme = clicked;
                                needs_apply = true;
                            } else if crate::events::contains_cell(
                                chunks[1],
                                mouse.column,
                                mouse.row,
                            ) {
                                state.focus = 1;
                                state.background_solid = !state.background_solid;
                                needs_apply = true;
                            }
                        }
                        2 => {
                            // Keybind preset
                            let clicked =
                                (mouse.row.saturating_sub(layout.content.y).saturating_sub(1))
                                    as usize;
                            let clicked = clicked.min(crate::setup::SETUP_PRESETS.len() - 1);
                            state.cursor = clicked;
                            state.keybind_preset = clicked;
                            needs_apply = true;
                        }
                        3 => {
                            // Mouse toggle
                            state.mouse_enabled = !state.mouse_enabled;
                            needs_apply = true;
                        }
                        4 => {
                            // Density
                            let clicked =
                                (mouse.row.saturating_sub(layout.content.y).saturating_sub(1))
                                    as usize;
                            let clicked = clicked.min(crate::setup::SETUP_DENSITIES.len() - 1);
                            state.cursor = clicked;
                            state.list_density = clicked;
                            needs_apply = true;
                        }
                        5 => {
                            // Hint Bar style
                            let clicked =
                                (mouse.row.saturating_sub(layout.content.y).saturating_sub(1))
                                    as usize;
                            let clicked = clicked.min(crate::setup::SETUP_HINT_STYLES.len() - 1);
                            state.cursor = clicked;
                            state.hint_bar_style = clicked;
                            needs_apply = true;
                        }
                        6 => {
                            // Daily Goals
                            let vertical_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Length(3), Constraint::Length(5)])
                                .split(layout.content);
                            if crate::events::contains_cell(
                                vertical_chunks[0],
                                mouse.column,
                                mouse.row,
                            ) {
                                state.focus = 0;
                                state.goals_enabled = !state.goals_enabled;
                                needs_apply = true;
                            } else if crate::events::contains_cell(
                                vertical_chunks[1],
                                mouse.column,
                                mouse.row,
                            ) {
                                state.focus = 1;
                                let ta_chunks = Layout::default()
                                    .direction(Direction::Vertical)
                                    .constraints([Constraint::Length(1), Constraint::Length(3)])
                                    .split(vertical_chunks[1]);
                                let textarea_border_area = ta_chunks[1];
                                let textarea_inner = Block::default()
                                    .borders(Borders::ALL)
                                    .inner(textarea_border_area);
                                crate::events::move_textarea_cursor_to_mouse(
                                    &mut state.word_goal_input,
                                    textarea_inner,
                                    mouse.column,
                                    mouse.row,
                                );
                                needs_apply = true;
                            }
                        }
                        7 => {
                            // Auto-Backup
                            let vertical_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Length(3), Constraint::Length(5)])
                                .split(layout.content);
                            if crate::events::contains_cell(
                                vertical_chunks[0],
                                mouse.column,
                                mouse.row,
                            ) {
                                state.focus = 0;
                                state.backup_enabled = !state.backup_enabled;
                                needs_apply = true;
                            } else if crate::events::contains_cell(
                                vertical_chunks[1],
                                mouse.column,
                                mouse.row,
                            ) {
                                state.focus = 1;
                                let ta_chunks = Layout::default()
                                    .direction(Direction::Vertical)
                                    .constraints([Constraint::Length(1), Constraint::Length(3)])
                                    .split(vertical_chunks[1]);
                                let textarea_border_area = ta_chunks[1];
                                let textarea_inner = Block::default()
                                    .borders(Borders::ALL)
                                    .inner(textarea_border_area);
                                crate::events::move_textarea_cursor_to_mouse(
                                    &mut state.remote_url_input,
                                    textarea_inner,
                                    mouse.column,
                                    mouse.row,
                                );
                                needs_apply = true;
                            }
                        }
                        8 => {
                            // Vault Path
                            let vertical_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Min(4), Constraint::Length(1)])
                                .split(layout.content);
                            let ta_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Length(1), Constraint::Length(3)])
                                .split(vertical_chunks[0]);
                            let textarea_border_area = ta_chunks[1];
                            if crate::events::contains_cell(
                                textarea_border_area,
                                mouse.column,
                                mouse.row,
                            ) {
                                state.focus = 0;
                                let textarea_inner = Block::default()
                                    .borders(Borders::ALL)
                                    .inner(textarea_border_area);
                                crate::events::move_textarea_cursor_to_mouse(
                                    &mut state.storage_path_input,
                                    textarea_inner,
                                    mouse.column,
                                    mouse.row,
                                );
                                needs_apply = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                let on_content =
                    crate::events::contains_cell(layout.content, mouse.column, mouse.row);
                let is_choice_step = state.step == 2
                    || state.step == 4
                    || state.step == 5
                    || (state.step == 1 && state.focus == 0);
                if on_content && is_choice_step {
                    state.move_cursor(true);
                } else {
                    state.advance();
                }
                needs_apply = true;
            }
            MouseEventKind::ScrollUp => {
                let on_content =
                    crate::events::contains_cell(layout.content, mouse.column, mouse.row);
                let is_choice_step = state.step == 2
                    || state.step == 4
                    || state.step == 5
                    || (state.step == 1 && state.focus == 0);
                if on_content && is_choice_step {
                    state.move_cursor(false);
                } else {
                    state.go_prev();
                }
                needs_apply = true;
            }
            _ => {}
        }
    }

    if needs_apply {
        app.apply_setup_live();
    }
}
