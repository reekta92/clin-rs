use crate::backup::state::{BackupInputMode, BackupSection, BackupState, SettingsField};
use crate::config::ClinConfig;
use crate::keybinds::{BackupAction, Keybinds};
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui_textarea::TextArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputResult {
    None,
    Back,
    Refresh,
    Help,
}

pub fn handle_input(
    state: &mut BackupState,
    event: KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
) -> InputResult {
    if state.input_mode == BackupInputMode::Normal {
        state.status_message = None;
    }

    match state.input_mode {
        BackupInputMode::Normal => handle_normal_input(state, event, keybinds, config),
        BackupInputMode::EditCommitMessage => {
            state.seq_matcher.clear();
            handle_commit_input(state, event, keybinds)
        }
        BackupInputMode::EditSettings => {
            state.seq_matcher.clear();
            handle_settings_input(state, event, keybinds)
        }
        BackupInputMode::EditSettingsField => {
            state.seq_matcher.clear();
            handle_settings_field_input(state, event, keybinds)
        }
    }
}

fn handle_normal_input(
    state: &mut BackupState,
    event: KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
) -> InputResult {
    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    match keybinds.resolve_backup(&mut state.seq_matcher, event, seq, counts) {
        crate::keybinds::MatchOutcome::Matched(action, count) => {
            if !state.settings.enabled {
                match action {
                    BackupAction::Back
                    | BackupAction::CancelCommit
                    | BackupAction::CloseSettings
                    | BackupAction::CancelEditField => return InputResult::Back,
                    BackupAction::OpenSettings => {
                        state.settings_open = true;
                        state.input_mode = BackupInputMode::EditSettings;
                        return InputResult::None;
                    }
                    BackupAction::Help => return InputResult::Help,
                    _ => return InputResult::None,
                }
            }
            match action {
                BackupAction::Back
                | BackupAction::CancelCommit
                | BackupAction::CloseSettings
                | BackupAction::CancelEditField => return InputResult::Back,
                BackupAction::MoveDown | BackupAction::NextField => {
                    let n = count.unwrap_or(1) as usize;
                    for _ in 0..n {
                        if state.selected_section == BackupSection::History {
                            if !state.commits.is_empty() {
                                state.selected_commit_index =
                                    (state.selected_commit_index + 1) % state.commits.len();
                                state.load_commit_diff();
                            }
                        } else if !state.selectable_files.is_empty() {
                            state.selected_index =
                                (state.selected_index + 1) % state.selectable_files.len();
                            state.selected_file =
                                Some(state.selectable_files[state.selected_index].clone());
                            state.load_selected_diff();
                        }
                    }
                }
                BackupAction::MoveUp | BackupAction::PrevField => {
                    let n = count.unwrap_or(1) as usize;
                    for _ in 0..n {
                        if state.selected_section == BackupSection::History {
                            if !state.commits.is_empty() {
                                state.selected_commit_index = if state.selected_commit_index == 0 {
                                    state.commits.len() - 1
                                } else {
                                    state.selected_commit_index - 1
                                };
                                state.load_commit_diff();
                            }
                        } else if !state.selectable_files.is_empty() {
                            state.selected_index = if state.selected_index == 0 {
                                state.selectable_files.len() - 1
                            } else {
                                state.selected_index - 1
                            };
                            state.selected_file =
                                Some(state.selectable_files[state.selected_index].clone());
                            state.load_selected_diff();
                        }
                    }
                }
                BackupAction::ScrollDiffDown => {
                    let n = count.unwrap_or(1) as usize;
                    for _ in 0..n {
                        state.diff_scroll = state.diff_scroll.saturating_add(10);
                        let max = state
                            .diff_lines
                            .len()
                            .saturating_sub(state.last_diff_height as usize);
                        state.diff_scroll = state.diff_scroll.min(max as u16);
                    }
                }
                BackupAction::ScrollDiffUp => {
                    let n = count.unwrap_or(1) as usize;
                    for _ in 0..n {
                        state.diff_scroll = state.diff_scroll.saturating_sub(10);
                    }
                }
                BackupAction::Refresh => return InputResult::Refresh,
                BackupAction::EnterCommit => {
                    if state.status.is_some() {
                        state.input_mode = BackupInputMode::EditCommitMessage;
                        state.commit_textarea = TextArea::default();
                    }
                }
                BackupAction::Push => {
                    state.push_to_remote();
                    return InputResult::Refresh;
                }
                BackupAction::Pull => {
                    state.pull_from_remote();
                    return InputResult::Refresh;
                }
                BackupAction::StageFile => {
                    if state.selected_section == BackupSection::Status
                        && let Some(file) = state.selected_file.clone()
                    {
                        let is_unstaged = state.status.as_ref().is_some_and(|s| {
                            s.unstaged.iter().any(|f| f.path == file) || s.untracked.contains(&file)
                        });
                        let is_staged = state
                            .status
                            .as_ref()
                            .is_some_and(|s| s.staged.iter().any(|f| f.path == file));
                        if is_unstaged {
                            state.stage_file(&file);
                            return InputResult::Refresh;
                        } else if is_staged {
                            state.unstage_file(&file);
                            return InputResult::Refresh;
                        }
                    }
                }
                BackupAction::UnstageFile => {
                    if state.selected_section == BackupSection::Status
                        && let Some(file) = state.selected_file.clone()
                    {
                        let is_staged = state
                            .status
                            .as_ref()
                            .is_some_and(|s| s.staged.iter().any(|f| f.path == file));
                        if is_staged {
                            state.unstage_file(&file);
                            return InputResult::Refresh;
                        }
                    }
                }
                BackupAction::StageAll => {
                    if state.selected_section == BackupSection::Status {
                        state.stage_all();
                        return InputResult::Refresh;
                    }
                }
                BackupAction::Help => {
                    return InputResult::Help;
                }
                BackupAction::OpenSettings => {
                    state.settings_open = true;
                    state.input_mode = BackupInputMode::EditSettings;
                }
                BackupAction::CycleSection => {
                    state.selected_section = match state.selected_section {
                        BackupSection::Status => {
                            state.load_commit_diff();
                            BackupSection::History
                        }
                        BackupSection::History => {
                            state.load_selected_diff();
                            BackupSection::Status
                        }
                    };
                }
                _ => {}
            }
        }
        crate::keybinds::MatchOutcome::Pending => return InputResult::None,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }
    InputResult::None
}

fn handle_commit_input(
    state: &mut BackupState,
    event: KeyEvent,
    keybinds: &Keybinds,
) -> InputResult {
    match event.code {
        _ if keybinds.matches_backup(BackupAction::CancelCommit, &event) => {
            state.input_mode = BackupInputMode::Normal;
        }
        _ if keybinds.matches_backup(BackupAction::ConfirmCommit, &event)
            && !event.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            let msg = state.commit_textarea.lines().join("\n").trim().to_string();
            if !msg.is_empty() {
                state.do_commit(&msg);
                state.input_mode = BackupInputMode::Normal;
                state.commit_textarea = TextArea::default();
                return InputResult::Refresh;
            }
        }
        _ => {
            if !apply_text_shortcuts(keybinds, &mut state.commit_textarea, event) {
                state.commit_textarea.input(event);
            }
        }
    }
    InputResult::None
}

fn handle_settings_input(
    state: &mut BackupState,
    event: KeyEvent,
    keybinds: &Keybinds,
) -> InputResult {
    match event.code {
        _ if keybinds.matches_backup(BackupAction::CloseSettings, &event) => {
            state.settings_open = false;
            state.input_mode = BackupInputMode::Normal;
        }
        _ if keybinds.matches_backup(BackupAction::NextField, &event) => {
            state.settings.focused_field = state.settings.focused_field.next();
        }
        _ if keybinds.matches_backup(BackupAction::PrevField, &event) => {
            state.settings.focused_field = state.settings.focused_field.prev();
        }
        _ if keybinds.matches_backup(BackupAction::ActivateField, &event) => {
            match state.settings.focused_field {
                SettingsField::Enabled => state.settings.enabled = !state.settings.enabled,
                SettingsField::BackupOnSave => {
                    state.settings.backup_on_save = !state.settings.backup_on_save
                }
                SettingsField::BackupOnQuit => {
                    state.settings.backup_on_quit = !state.settings.backup_on_quit
                }
                SettingsField::AutoPush => state.settings.auto_push = !state.settings.auto_push,
                SettingsField::RemoteUrl | SettingsField::RemoteName => {
                    state.input_mode = BackupInputMode::EditSettingsField;
                }
                SettingsField::SaveButton => {
                    state.save_settings();
                    state.settings_open = false;
                    state.input_mode = BackupInputMode::Normal;
                    return InputResult::Refresh;
                }
            }
        }
        _ => {}
    }
    InputResult::None
}

fn handle_settings_field_input(
    state: &mut BackupState,
    event: KeyEvent,
    keybinds: &Keybinds,
) -> InputResult {
    match event.code {
        _ if keybinds.matches_backup(BackupAction::CancelEditField, &event) => {
            state.input_mode = BackupInputMode::EditSettings;
        }
        _ if keybinds.matches_backup(BackupAction::ConfirmEditField, &event)
            && !event.modifiers.contains(KeyModifiers::CONTROL)
            && !event.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            state.input_mode = BackupInputMode::EditSettings;
        }
        _ => match state.settings.focused_field {
            SettingsField::RemoteUrl => {
                if !apply_text_shortcuts(keybinds, &mut state.settings.remote_url, event) {
                    state.settings.remote_url.input(event);
                }
            }
            SettingsField::RemoteName
                if !apply_text_shortcuts(keybinds, &mut state.settings.remote_name, event) =>
            {
                state.settings.remote_name.input(event);
            }
            _ => {}
        },
    }
    InputResult::None
}

pub fn handle_mouse(
    state: &mut BackupState,
    event: MouseEvent,
    icon_mode: crate::config::IconMode,
) -> InputResult {
    if state.settings_open {
        return handle_settings_mouse(state, event);
    }

    if state.input_mode == BackupInputMode::EditCommitMessage
        && let MouseEventKind::Down(MouseButton::Left) = event.kind
        && let Some(area) = state.last_area
    {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, area);
        if !crate::events::contains_cell(popup_area, event.column, event.row) {
            state.input_mode = BackupInputMode::Normal;
            return InputResult::None;
        }
    }

    if let MouseEventKind::Down(MouseButton::Left) = event.kind {
        let x = event.column;
        let y = event.row;

        if let Some(area) = state.last_area {
            if y == area.y.saturating_sub(1) {
                let backup_tabs_array = crate::backup::render::backup_tabs(icon_mode);
                let tabs: Vec<(&str, Option<&str>)> = backup_tabs_array
                    .iter()
                    .map(|&(l, g)| (l, Some(g)))
                    .collect();
                let header_area = Rect {
                    y: area.y.saturating_sub(1),
                    height: 1,
                    ..area
                };
                let region = crate::ui::title_bar_tabs_region(header_area, "Backup");
                if let Some(i) = crate::ui::hit_test_tabs(
                    &tabs,
                    area.x,
                    area.width,
                    region.x,
                    x,
                    state.tab_icons_only,
                    icon_mode,
                ) {
                    state.selected_section = match i {
                        1 => BackupSection::History,
                        _ => BackupSection::Status,
                    };
                }
                return InputResult::None;
            }

            let has_diff = !state.diff_lines.is_empty();
            let list_width = if has_diff {
                (area.width as f32 * 0.43) as u16
            } else {
                area.width
            };

            if x >= area.x && x < area.x + list_width && y > area.y && y < area.y + area.height - 1
            {
                if state.selected_section == BackupSection::Status {
                    let line_idx = y.saturating_sub(area.y).saturating_sub(1) as usize
                        + state.list_state.offset();
                    if let Some(file_idx) = state.file_index_at_rendered_line(line_idx) {
                        state.selected_index = file_idx;
                        state.selected_file = Some(state.selectable_files[file_idx].clone());
                        state.load_selected_diff();
                    }
                } else if state.selected_section == BackupSection::History {
                    let line_idx = y.saturating_sub(area.y).saturating_sub(1) as usize
                        + state.history_list_state.offset();
                    if line_idx > 0 {
                        let commit_idx = line_idx - 1;
                        if commit_idx < state.commits.len() {
                            state.selected_commit_index = commit_idx;
                            state.load_commit_diff();
                        }
                    }
                }
            }
        }
    } else if let MouseEventKind::ScrollDown = event.kind {
        if let Some(area) = state.last_area {
            let is_history = state.selected_section == BackupSection::History;
            let has_diff = !state.diff_lines.is_empty();
            let list_width = if has_diff {
                (area.width as f32 * 0.43) as u16
            } else {
                area.width
            };
            if event.column < area.x + list_width {
                if is_history {
                    if !state.commits.is_empty() {
                        state.selected_commit_index =
                            (state.selected_commit_index + 1).min(state.commits.len() - 1);
                        state.load_commit_diff();
                    }
                } else if !state.selectable_files.is_empty() {
                    state.selected_index =
                        (state.selected_index + 1).min(state.selectable_files.len() - 1);
                    state.selected_file =
                        Some(state.selectable_files[state.selected_index].clone());
                    state.load_selected_diff();
                }
            } else {
                state.diff_scroll = state.diff_scroll.saturating_add(3);
            }
        } else {
            state.diff_scroll = state.diff_scroll.saturating_add(3);
        }
    } else if let MouseEventKind::ScrollUp = event.kind {
        if let Some(area) = state.last_area {
            let is_history = state.selected_section == BackupSection::History;
            let has_diff = !state.diff_lines.is_empty();
            let list_width = if has_diff {
                (area.width as f32 * 0.43) as u16
            } else {
                area.width
            };
            if event.column < area.x + list_width {
                if is_history {
                    if !state.commits.is_empty() {
                        state.selected_commit_index = state.selected_commit_index.saturating_sub(1);
                        state.load_commit_diff();
                    }
                } else if !state.selectable_files.is_empty() {
                    state.selected_index = state.selected_index.saturating_sub(1);
                    state.selected_file =
                        Some(state.selectable_files[state.selected_index].clone());
                    state.load_selected_diff();
                }
            } else {
                state.diff_scroll = state.diff_scroll.saturating_sub(3);
            }
        } else {
            state.diff_scroll = state.diff_scroll.saturating_sub(3);
        }
    }

    InputResult::None
}

fn handle_settings_mouse(state: &mut BackupState, event: MouseEvent) -> InputResult {
    let Some(area) = state.last_area else {
        return InputResult::None;
    };

    // Mirror draw_popup_frame(60, 60): popup_area → strip footer → bordered block inner
    let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);

    // Outside left-click → close settings
    if event.kind == MouseEventKind::Down(MouseButton::Left)
        && !crate::events::contains_cell(popup_area, event.column, event.row)
    {
        state.settings_open = false;
        state.input_mode = BackupInputMode::Normal;
        return InputResult::None;
    }

    if event.kind != MouseEventKind::Down(MouseButton::Left) {
        return InputResult::None;
    }

    // Reproduce the exact layout from draw_settings_popup:
    //   draw_popup_frame returns chunks[0] of [Min(1), Length(1)] split of popup_area
    let frame_chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(popup_area);
    let content = frame_chunks[0]; // what draw_popup_frame returns

    // draw_settings_popup wraps content in a bordered Block, then splits its inner
    let outer_block = ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL);
    let inner_content = outer_block.inner(content);

    // Same constraints as draw_settings_popup's Layout split of inner_content
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3), // Enabled
            Constraint::Length(3), // Backup on Save
            Constraint::Length(3), // Backup on Quit
            Constraint::Length(3), // Auto Push
            Constraint::Length(3), // Remote URL
            Constraint::Length(3), // Remote Name
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Save button
            Constraint::Min(0),
        ])
        .split(inner_content);

    let fields: &[(usize, SettingsField)] = &[
        (0, SettingsField::Enabled),
        (1, SettingsField::BackupOnSave),
        (2, SettingsField::BackupOnQuit),
        (3, SettingsField::AutoPush),
        (4, SettingsField::RemoteUrl),
        (5, SettingsField::RemoteName),
        (7, SettingsField::SaveButton),
    ];

    for &(idx, field) in fields {
        let rect = chunks[idx];
        if crate::events::contains_cell(rect, event.column, event.row) {
            state.settings.focused_field = field;
            match field {
                SettingsField::Enabled => state.settings.enabled = !state.settings.enabled,
                SettingsField::BackupOnSave => {
                    state.settings.backup_on_save = !state.settings.backup_on_save
                }
                SettingsField::BackupOnQuit => {
                    state.settings.backup_on_quit = !state.settings.backup_on_quit
                }
                SettingsField::AutoPush => state.settings.auto_push = !state.settings.auto_push,
                SettingsField::RemoteUrl | SettingsField::RemoteName => {
                    state.input_mode = BackupInputMode::EditSettingsField;
                }
                SettingsField::SaveButton => {
                    state.input_mode = BackupInputMode::Normal;
                    state.settings_open = false;
                    state.save_settings();
                    return InputResult::Refresh;
                }
            }
            return InputResult::None;
        }
    }

    InputResult::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_theme::AppThemeColors;
    use crate::config::BackupConfig;
    use crate::keybinds::Keybinds;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use std::sync::Arc;

    #[test]
    fn test_backup_esc_q_always_returns_back() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = BackupConfig::default();
        let theme = AppThemeColors::default();
        let keybinds = Keybinds::default();
        let seq_matcher = crate::keybinds::KeyMatcher::new();

        let mut state = BackupState::new(
            temp_dir.path().to_path_buf(),
            &config,
            theme,
            keybinds.clone(),
            false,
            Arc::new(parking_lot::Mutex::new(())),
            seq_matcher,
        );

        let clin_config = crate::config::ClinConfig::default();
        let _seq = clin_config.sequences_enabled();
        let _counts = clin_config.counts_enabled();

        let esc_event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let q_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

        let res1 = handle_normal_input(&mut state, esc_event, &keybinds, &clin_config);
        let res2 = handle_normal_input(&mut state, q_event, &keybinds, &clin_config);

        assert_eq!(res1, InputResult::Back);
        assert_eq!(res2, InputResult::Back);
    }

    #[test]
    fn test_backup_mouse_targeting() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = BackupConfig::default();
        let theme = AppThemeColors::default();
        let keybinds = Keybinds::default();
        let seq_matcher = crate::keybinds::KeyMatcher::new();

        let mut state = BackupState::new(
            temp_dir.path().to_path_buf(),
            &config,
            theme,
            keybinds.clone(),
            false,
            Arc::new(parking_lot::Mutex::new(())),
            seq_matcher,
        );

        state.selectable_files = vec!["file1.txt".to_string(), "file2.txt".to_string()];
        state.selected_section = BackupSection::Status;

        let area = Rect::new(0, 0, 100, 20);
        state.last_area = Some(area);

        state.status = Some(crate::backup::git_ops::GitStatus {
            branch: "main".to_string(),
            ahead: 0,
            behind: 0,
            staged: vec![],
            unstaged: vec![crate::backup::git_ops::FileStatus {
                path: "file1.txt".to_string(),
                status: crate::backup::git_ops::FileChangeType::Modified,
            }],
            untracked: vec!["file2.txt".to_string()],
        });

        let mouse_event = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 5,
            row: area.y + 5,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };

        handle_mouse(&mut state, mouse_event, crate::config::IconMode::None);

        assert_eq!(state.selected_index, 0);
        assert_eq!(state.selected_file.as_deref(), Some("file1.txt"));
    }

    #[test]
    fn test_backup_disabled_ignores_movement_and_clears_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = BackupConfig {
            enabled: true,
            ..Default::default()
        };
        let theme = AppThemeColors::default();
        let keybinds = Keybinds::default();
        let seq_matcher = crate::keybinds::KeyMatcher::new();

        let mut state = BackupState::new(
            temp_dir.path().to_path_buf(),
            &config,
            theme,
            keybinds.clone(),
            false,
            Arc::new(parking_lot::Mutex::new(())),
            seq_matcher,
        );

        state.selectable_files = vec!["file1.txt".to_string()];
        state.diff_lines = vec!["some diff".to_string()];

        state.settings.enabled = false;
        state.refresh_git_info();

        assert!(state.selectable_files.is_empty());
        assert!(state.diff_lines.is_empty());
        assert!(state.status.is_none());

        let clin_config = crate::config::ClinConfig::default();
        let j_event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let res = handle_normal_input(&mut state, j_event, &keybinds, &clin_config);
        assert_eq!(res, InputResult::None);
    }
}
