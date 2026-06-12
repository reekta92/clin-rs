use ratatui::layout::{Constraint, Layout, Rect};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::backup::state::{BackupInputMode, BackupSection, BackupState, SettingsField};
use ratatui_textarea::TextArea;
use crate::backup::git_ops::GitOps;
use crate::config::ClinConfig;

pub enum InputResult {
    None,
    Back,
    Refresh,
}

pub fn handle_input(state: &mut BackupState, event: KeyEvent) -> InputResult {
    if state.input_mode == BackupInputMode::Normal {
        state.status_message = None;
    }

    match state.input_mode {
        BackupInputMode::Normal => handle_normal_input(state, event),
        BackupInputMode::EditCommitMessage => handle_commit_input(state, event),
        BackupInputMode::EditSettings => handle_settings_input(state, event),
        BackupInputMode::EditSettingsField => handle_settings_field_input(state, event),
    }
}
fn handle_normal_input(state: &mut BackupState, event: KeyEvent) -> InputResult {
    match event.code {
        KeyCode::Esc => return InputResult::Back,
        KeyCode::Char('j') | KeyCode::Down => {
            if !state.selectable_files.is_empty() {
                state.selected_index = (state.selected_index + 1) % state.selectable_files.len();
                state.selected_file = Some(state.selectable_files[state.selected_index].clone());
                state.load_selected_diff();
                state.adjust_scroll_to_selection();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if !state.selectable_files.is_empty() {
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
        KeyCode::PageDown => {
            state.diff_scroll = state.diff_scroll.saturating_add(10);
        }
        KeyCode::PageUp => {
            state.diff_scroll = state.diff_scroll.saturating_sub(10);
        }
        KeyCode::Char('r') => return InputResult::Refresh,
        KeyCode::Char('s') => {
            if state.status.is_some() {
                state.input_mode = BackupInputMode::EditCommitMessage;
                state.commit_textarea = TextArea::default();
            }
        }
        KeyCode::Char('p') => {
            state.push_to_remote();
        }
        KeyCode::Char('/') => {
            state.settings_open = true;
            state.input_mode = BackupInputMode::EditSettings;
        }
        KeyCode::Tab => {
            state.selected_section = match state.selected_section {
                BackupSection::Status => BackupSection::Changes,
                BackupSection::Changes => BackupSection::History,
                BackupSection::History => BackupSection::Status,
            };
        }
        _ => {}
    }
    InputResult::None
}
fn handle_commit_input(state: &mut BackupState, event: KeyEvent) -> InputResult {
    match event.code {
        KeyCode::Esc => {
            state.input_mode = BackupInputMode::Normal;
        }
        KeyCode::Enter if !event.modifiers.contains(KeyModifiers::CONTROL) => {
            let msg = state.commit_textarea.lines().join("\n").trim().to_string();
            if !msg.is_empty() {
                state.do_commit(&msg);
                state.input_mode = BackupInputMode::Normal;
                state.commit_textarea = TextArea::default();
                return InputResult::Refresh;
            }
        }
        _ => {
            state.commit_textarea.input(event);
        }
    }
    InputResult::None
}

fn handle_settings_input(state: &mut BackupState, event: KeyEvent) -> InputResult {
    match event.code {
        KeyCode::Esc => {
            state.settings_open = false;
            state.input_mode = BackupInputMode::Normal;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.settings.focused_field = state.settings.focused_field.next();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.settings.focused_field = state.settings.focused_field.prev();
        }
        KeyCode::Enter => {
            match state.settings.focused_field {
                SettingsField::Enabled => state.settings.enabled = !state.settings.enabled,
                SettingsField::BackupOnSave => state.settings.backup_on_save = !state.settings.backup_on_save,
                SettingsField::BackupOnQuit => state.settings.backup_on_quit = !state.settings.backup_on_quit,
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

fn handle_settings_field_input(state: &mut BackupState, event: KeyEvent) -> InputResult {
    match event.code {
        KeyCode::Esc => {
            state.input_mode = BackupInputMode::EditSettings;
        }
        KeyCode::Enter if !event.modifiers.contains(KeyModifiers::CONTROL) && !event.modifiers.contains(KeyModifiers::SHIFT) => {
            state.input_mode = BackupInputMode::EditSettings;
        }
        _ => {
            match state.settings.focused_field {
                SettingsField::RemoteUrl => { state.settings.remote_url.input(event); }
                SettingsField::RemoteName => { state.settings.remote_name.input(event); }
                _ => {}
            }
        }
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
            let list_width = (area.width as f32 * 0.4) as u16;
            if x >= area.x && x < area.x + list_width && y >= area.y + 1 && y < area.y + area.height - 1 {
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
            let popup_area = crate::ui::centered_rect(50, 65, area);
            let content_area = Rect {
                x: popup_area.x,
                y: popup_area.y + 1,
                width: popup_area.width,
                height: popup_area.height.saturating_sub(1),
            };

            for (rect, field) in settings_field_rects(content_area) {
                if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
                    state.settings.focused_field = field;
                    match field {
                        SettingsField::Enabled => state.settings.enabled = !state.settings.enabled,
                        SettingsField::BackupOnSave => state.settings.backup_on_save = !state.settings.backup_on_save,
                        SettingsField::BackupOnQuit => state.settings.backup_on_quit = !state.settings.backup_on_quit,
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
                Err(e) => self.status_message = Some(format!("Error: {}", e)),
            }
        }
    }

    fn push_to_remote(&mut self) {
        let remote_name = self.settings.remote_name.lines().join("").trim().to_string();
        self.status_message = Some(format!("Pushing to {}...", remote_name));
        
        if let Ok(git_ops) = GitOps::init(&self.vault_path) {
            match git_ops.push(&remote_name) {
                Ok(_) => self.status_message = Some("Push complete".to_string()),
                Err(e) => self.status_message = Some(format!("Push failed: {}", e)),
            }
        }
    }

    fn save_settings(&mut self) {
        let mut config = match ClinConfig::load() {
            Ok(c) => c,
            Err(_) => ClinConfig::default(),
        };

        config.backup.enabled = self.settings.enabled;
        config.backup.backup_on_save = self.settings.backup_on_save;
        config.backup.backup_on_quit = self.settings.backup_on_quit;
        config.backup.auto_push = self.settings.auto_push;
        let url_text = self.settings.remote_url.lines().join("").trim().to_string();
        let name_text = self.settings.remote_name.lines().join("").trim().to_string();
        config.backup.remote_url = if url_text.is_empty() { None } else { Some(url_text) };
        config.backup.remote_name = if name_text.is_empty() { Some("origin".to_string()) } else { Some(name_text.clone()) };

        if let Err(e) = config.save() {
            self.status_message = Some(format!("Config save failed: {}", e));
        } else {
            self.status_message = Some("Settings saved".to_string());
            
            // Re-init git if enabled and not initialized
            if config.backup.enabled && !GitOps::is_initialized(&self.vault_path) {
                if let Ok(git_ops) = GitOps::init(&self.vault_path) {
                    if let Some(url) = &config.backup.remote_url {
                        let _ = git_ops.set_remote(&name_text, url);
                    }
                    // Initial commit
                    let _ = git_ops.add_all().and_then(|_| git_ops.commit("Initial backup"));
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
