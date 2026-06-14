use crate::backup::git_ops::GitOps;
use crate::backup::state::{BackupInputMode, BackupSection, BackupState, SettingsField};
use crate::config::ClinConfig;
use crate::keybinds::{BackupAction, Keybinds};
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui_textarea::TextArea;

pub enum InputResult {
    None,
    Back,
    Refresh,
}

pub fn handle_input(state: &mut BackupState, event: KeyEvent, keybinds: &Keybinds) -> InputResult {
    if state.input_mode == BackupInputMode::Normal {
        state.status_message = None;
    }

    match state.input_mode {
        BackupInputMode::Normal => handle_normal_input(state, event, keybinds),
        BackupInputMode::EditCommitMessage => handle_commit_input(state, event, keybinds),
        BackupInputMode::EditSettings => handle_settings_input(state, event, keybinds),
        BackupInputMode::EditSettingsField => handle_settings_field_input(state, event, keybinds),
    }
}

fn handle_normal_input(
    state: &mut BackupState,
    event: KeyEvent,
    keybinds: &Keybinds,
) -> InputResult {
    match event.code {
        _ if keybinds.matches_backup(BackupAction::Back, &event) => return InputResult::Back,
        _ if keybinds.matches_backup(BackupAction::MoveDown, &event) => {
            if state.selected_section == BackupSection::History {
                state.history_scroll = state.history_scroll.saturating_add(1);
            } else if !state.selectable_files.is_empty() {
                state.selected_index = (state.selected_index + 1) % state.selectable_files.len();
                state.selected_file = Some(state.selectable_files[state.selected_index].clone());
                state.load_selected_diff();
                state.adjust_scroll_to_selection();
            }
        }
        _ if keybinds.matches_backup(BackupAction::MoveUp, &event) => {
            if state.selected_section == BackupSection::History {
                state.history_scroll = state.history_scroll.saturating_sub(1);
            } else if !state.selectable_files.is_empty() {
                state.selected_index = if state.selected_index == 0 {
                    state.selectable_files.len() - 1
                } else {
                    state.selected_index - 1
                };
                state.selected_file = Some(state.selectable_files[state.selected_index].clone());
                state.load_selected_diff();
                state.adjust_scroll_to_selection();
            }
        }
        _ if keybinds.matches_backup(BackupAction::ScrollDiffDown, &event) => {
            state.diff_scroll = state.diff_scroll.saturating_add(10);
        }
        _ if keybinds.matches_backup(BackupAction::ScrollDiffUp, &event) => {
            state.diff_scroll = state.diff_scroll.saturating_sub(10);
        }
        _ if keybinds.matches_backup(BackupAction::Refresh, &event) => return InputResult::Refresh,
        _ if keybinds.matches_backup(BackupAction::EnterCommit, &event) => {
            if state.status.is_some() {
                state.input_mode = BackupInputMode::EditCommitMessage;
                state.commit_textarea = TextArea::default();
            }
        }
        _ if keybinds.matches_backup(BackupAction::Push, &event) => {
            state.push_to_remote();
        }
        _ if keybinds.matches_backup(BackupAction::OpenSettings, &event) => {
            state.settings_open = true;
            state.input_mode = BackupInputMode::EditSettings;
        }
        _ if keybinds.matches_backup(BackupAction::CycleSection, &event) => {
            state.selected_section = match state.selected_section {
                BackupSection::Status => BackupSection::History,
                BackupSection::History => BackupSection::Status,
            };
        }
        _ => {}
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

pub fn handle_mouse(state: &mut BackupState, event: MouseEvent) -> InputResult {
    if state.settings_open {
        return handle_settings_mouse(state, event);
    }

    if let MouseEventKind::Down(MouseButton::Left) = event.kind {
        let x = event.column;
        let y = event.row;

        if let Some(area) = state.last_area {
            if y == area.y {
                let tabs = [("Status", None), ("History", None)];
                if let Some(i) = crate::ui::hit_test_tabs(&tabs, area.x, area.width, x) {
                    state.selected_section = match i {
                        1 => BackupSection::History,
                        _ => BackupSection::Status,
                    };
                }
                return InputResult::None;
            }

            let list_width = (area.width as f32 * 0.4) as u16;
            if x >= area.x && x < area.x + list_width && y > area.y && y < area.y + area.height - 1
            {
                let line_index = (y - area.y - 2) as usize;
                if line_index < state.selectable_files.len() {
                    state.selected_index = line_index;
                    state.selected_file = Some(state.selectable_files[line_index].clone());
                    state.load_selected_diff();
                }
            }
        }
    } else if let MouseEventKind::ScrollDown = event.kind {
        if let Some(area) = state.last_area {
            let list_width = (area.width as f32 * 0.4) as u16;
            if state.selected_file.is_some() && event.column < area.x + list_width {
                state.scroll = state.scroll.saturating_add(3);
            } else {
                state.diff_scroll = state.diff_scroll.saturating_add(3);
            }
        } else {
            state.diff_scroll = state.diff_scroll.saturating_add(3);
        }
    } else if let MouseEventKind::ScrollUp = event.kind {
        if let Some(area) = state.last_area {
            let list_width = (area.width as f32 * 0.4) as u16;
            if state.selected_file.is_some() && event.column < area.x + list_width {
                state.scroll = state.scroll.saturating_sub(3);
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
    if let MouseEventKind::Down(MouseButton::Left) = event.kind {
        let x = event.column;
        let y = event.row;

        if let Some(area) = state.last_area {
            let popup_area = crate::ui::centered_rect_fixed(55, 17, area);
            let content_area = ratatui::layout::Rect {
                x: popup_area.x,
                y: popup_area.y,
                width: popup_area.width,
                height: popup_area.height.saturating_sub(1),
            };

            for (rect, field) in settings_field_rects(content_area) {
                if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
                {
                    state.settings.focused_field = field;
                    match field {
                        SettingsField::Enabled => state.settings.enabled = !state.settings.enabled,
                        SettingsField::BackupOnSave => {
                            state.settings.backup_on_save = !state.settings.backup_on_save
                        }
                        SettingsField::BackupOnQuit => {
                            state.settings.backup_on_quit = !state.settings.backup_on_quit
                        }
                        SettingsField::AutoPush => {
                            state.settings.auto_push = !state.settings.auto_push
                        }
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
                    break;
                }
            }
        }
    }
    InputResult::None
}

fn settings_field_rects(content_area: Rect) -> Vec<(Rect, SettingsField)> {
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(1), // General heading
            Constraint::Length(3), // Enabled
            Constraint::Length(1), // spacer
            Constraint::Length(1), // Auto-Backup heading
            Constraint::Length(3), // Backup on Save
            Constraint::Length(3), // Backup on Quit
            Constraint::Length(3), // Auto-Push
            Constraint::Length(1), // spacer
            Constraint::Length(1), // Remote heading
            Constraint::Length(3), // Remote URL
            Constraint::Length(3), // Remote Name
            Constraint::Length(1), // spacer
            Constraint::Length(3), // Save button
            Constraint::Min(0),    // filler
            Constraint::Length(1), // footer
        ])
        .split(content_area);
    vec![
        (chunks[1], SettingsField::Enabled),
        (chunks[4], SettingsField::BackupOnSave),
        (chunks[5], SettingsField::BackupOnQuit),
        (chunks[6], SettingsField::AutoPush),
        (chunks[9], SettingsField::RemoteUrl),
        (chunks[10], SettingsField::RemoteName),
        (chunks[12], SettingsField::SaveButton),
    ]
}

impl BackupState {
    fn do_commit(&mut self, message: &str) {
        if let Ok(git_ops) = GitOps::init(&self.vault_path) {
            match git_ops.add_all().and_then(|_| git_ops.commit(message)) {
                Ok(_) => self.status_message = Some("Commit successful".to_string()),
                Err(e) => self.status_message = Some(format!("Error: {e}")),
            }
        }
    }

    fn push_to_remote(&mut self) {
        let remote_name = self
            .settings
            .remote_name
            .lines()
            .join("")
            .trim()
            .to_string();
        self.status_message = Some(format!("Pushing to {remote_name}..."));

        if let Ok(git_ops) = GitOps::init(&self.vault_path) {
            match git_ops.push(&remote_name) {
                Ok(_) => self.status_message = Some("Push complete".to_string()),
                Err(e) => self.status_message = Some(format!("Push failed: {e}")),
            }
        }
    }

    fn save_settings(&mut self) {
        let mut config = ClinConfig::load().unwrap_or_default();

        config.backup.enabled = self.settings.enabled;
        config.backup.backup_on_save = self.settings.backup_on_save;
        config.backup.backup_on_quit = self.settings.backup_on_quit;
        config.backup.auto_push = self.settings.auto_push;
        let url_text = self.settings.remote_url.lines().join("").trim().to_string();
        let name_text = self
            .settings
            .remote_name
            .lines()
            .join("")
            .trim()
            .to_string();
        config.backup.remote_url = if url_text.is_empty() {
            None
        } else {
            Some(url_text)
        };
        config.backup.remote_name = if name_text.is_empty() {
            Some("origin".to_string())
        } else {
            Some(name_text.clone())
        };

        if let Err(e) = config.save() {
            self.status_message = Some(format!("Config save failed: {e}"));
        } else {
            self.status_message = Some("Settings saved".to_string());

            // Re-init git if enabled and not initialized
            if config.backup.enabled && !GitOps::is_initialized(&self.vault_path) {
                if let Ok(git_ops) = GitOps::init(&self.vault_path) {
                    if let Some(url) = &config.backup.remote_url {
                        let _ = git_ops.set_remote(&name_text, url);
                    }
                    // Initial commit
                    let _ = git_ops
                        .add_all()
                        .and_then(|_| git_ops.commit("Initial backup"));
                    if config.backup.auto_push {
                        let _ = git_ops.push(&name_text);
                    }
                }
            } else if let Ok(git_ops) = GitOps::init(&self.vault_path) {
                // Update remote if url changed
                if let Some(url) = &config.backup.remote_url {
                    let _ = git_ops.set_remote(&name_text, url);
                }
            }
        }
    }
}
