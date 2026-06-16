use crate::app_theme::AppThemeColors;
use crate::backup::git_ops::{CommitInfo, FileDiff, GitOps, GitStatus};
use crate::config::BackupConfig;
use ratatui_textarea::TextArea;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::keybinds::Keybinds;

pub struct BackupState {
    pub status: Option<GitStatus>,
    pub commits: Vec<CommitInfo>,
    pub diffs: Vec<FileDiff>,
    pub scroll: u16,
    pub history_scroll: u16,
    pub diff_scroll: u16,
    pub last_content_height: u16,
    pub last_diff_height: u16,
    pub selected_section: BackupSection,
    pub selected_index: usize,
    pub commit_textarea: TextArea<'static>,
    pub input_mode: BackupInputMode,
    pub status_message: Option<String>,
    pub vault_path: PathBuf,
    pub settings_open: bool,
    pub settings: BackupSettingsState,
    pub selectable_files: Vec<String>,
    pub selected_for_commit: HashSet<String>,
    pub theme: AppThemeColors,
    pub selected_file: Option<String>,
    pub diff_lines: Vec<String>,
    pub last_area: Option<ratatui::layout::Rect>,
    pub footer_hint: String,
    pub keybinds: Keybinds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupSection {
    Status,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupInputMode {
    Normal,
    EditCommitMessage,
    EditSettings,
    EditSettingsField,
}

pub struct BackupSettingsState {
    pub enabled: bool,
    pub backup_on_save: bool,
    pub backup_on_quit: bool,
    pub auto_push: bool,
    pub remote_url: TextArea<'static>,
    pub remote_name: TextArea<'static>,
    pub focused_field: SettingsField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    Enabled,
    BackupOnSave,
    BackupOnQuit,
    AutoPush,
    RemoteUrl,
    RemoteName,
    SaveButton,
}

impl SettingsField {
    const ORDER: [SettingsField; 7] = [
        SettingsField::Enabled,
        SettingsField::BackupOnSave,
        SettingsField::BackupOnQuit,
        SettingsField::AutoPush,
        SettingsField::RemoteUrl,
        SettingsField::RemoteName,
        SettingsField::SaveButton,
    ];
    pub fn next(self) -> Self {
        Self::ORDER
            .iter()
            .cycle()
            .skip_while(|&&f| f != self)
            .nth(1)
            .copied()
            .unwrap_or(self)
    }
    pub fn prev(self) -> Self {
        Self::ORDER
            .iter()
            .rev()
            .cycle()
            .skip_while(|&&f| f != self)
            .nth(1)
            .copied()
            .unwrap_or(self)
    }
}

impl BackupState {
    pub fn new(
        vault_path: PathBuf,
        config: &BackupConfig,
        theme: AppThemeColors,
        keybinds: Keybinds,
    ) -> Self {
        let settings = BackupSettingsState {
            enabled: config.enabled,
            backup_on_save: config.backup_on_save,
            backup_on_quit: config.backup_on_quit,
            auto_push: config.auto_push,
            remote_url: TextArea::from(vec![config.remote_url.clone().unwrap_or_default()]),
            remote_name: TextArea::from(vec![
                config
                    .remote_name
                    .clone()
                    .unwrap_or_else(|| "origin".to_string()),
            ]),
            focused_field: SettingsField::Enabled,
        };

        let mut state = Self {
            status: None,
            commits: Vec::new(),
            diffs: Vec::new(),
            scroll: 0,
            history_scroll: 0,
            diff_scroll: 0,
            selected_section: BackupSection::Status,
            selected_index: 0,
            selectable_files: Vec::new(),
            selected_file: None,
            diff_lines: Vec::new(),
            last_area: None,
            commit_textarea: TextArea::default(),
            input_mode: BackupInputMode::Normal,
            status_message: None,
            last_content_height: 0,
            last_diff_height: 0,
            vault_path: vault_path.clone(),
            settings_open: false,
            footer_hint: String::new(),
            settings,
            theme,
            selected_for_commit: HashSet::new(),
            keybinds,
        };

        state.refresh_git_info();
        state
    }

    pub fn load_selected_diff(&mut self) {
        if let Some(path) = &self.selected_file {
            if let Ok(git_ops) = GitOps::init(&self.vault_path) {
                self.diff_lines = git_ops.get_file_diff(path).unwrap_or_default();
                self.diff_scroll = 0;
            }
        } else {
            self.diff_lines.clear();
        }
    }

    pub fn refresh_git_info(&mut self) {
        if let Ok(git_ops) = GitOps::init(&self.vault_path) {
            self.status = git_ops.status().ok();
            self.commits = git_ops.log(50).unwrap_or_default();
            self.diffs = git_ops.diff_summary().unwrap_or_default();
            let mut files = Vec::new();
            if let Some(status) = &self.status {
                for s in &status.staged {
                    files.push(s.path.clone());
                }
                for s in &status.unstaged {
                    files.push(s.path.clone());
                }
                for s in &status.untracked {
                    files.push(s.clone());
                }
            }
            self.selectable_files = files;
            self.selected_for_commit = self
                .status
                .as_ref()
                .map(|s| {
                    s.unstaged
                        .iter()
                        .map(|f| f.path.clone())
                        .chain(s.untracked.iter().cloned())
                        .collect()
                })
                .unwrap_or_default();

            if self.selected_file.is_none() && !self.selectable_files.is_empty() {
                self.selected_file = Some(self.selectable_files[0].clone());
                self.load_selected_diff();
            }
        }
    }

    pub fn adjust_scroll_to_selection(&mut self) {
        let visible_lines = 20;
        if self.selected_index < self.scroll as usize {
            self.scroll = self.selected_index as u16;
        } else if self.selected_index >= self.scroll as usize + visible_lines {
            self.scroll = (self.selected_index + 1).saturating_sub(visible_lines) as u16;
        }
    }

    pub fn file_index_at_rendered_line(&self, line_idx: usize) -> Option<usize> {
        if self.selected_section != BackupSection::Status {
            return None;
        }

        let status = self.status.as_ref()?;
        let mut current_line = 0;
        let mut current_file_idx = 0;

        // Staged
        if current_line == line_idx {
            return None;
        }
        current_line += 1; // Header
        if status.staged.is_empty() {
            if current_line == line_idx {
                return None;
            }
            current_line += 1; // "No staged changes"
        } else {
            for _ in &status.staged {
                if current_line == line_idx {
                    return Some(current_file_idx);
                }
                current_line += 1;
                current_file_idx += 1;
            }
        }

        if current_line == line_idx {
            return None;
        }
        current_line += 1; // Empty line

        // Unstaged
        if current_line == line_idx {
            return None;
        }
        current_line += 1; // Header
        if status.unstaged.is_empty() && status.untracked.is_empty() {
            if current_line == line_idx {
                return None;
            }
            current_line += 1;
            let _ = current_line; // Represents "No unstaged changes" line
        } else {
            for _ in &status.unstaged {
                if current_line == line_idx {
                    return Some(current_file_idx);
                }
                current_line += 1;
                current_file_idx += 1;
            }
            for _ in &status.untracked {
                if current_line == line_idx {
                    return Some(current_file_idx);
                }
                current_line += 1;
                current_file_idx += 1;
            }
        }

        None
    }
}
