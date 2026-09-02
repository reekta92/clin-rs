use crate::app_theme::AppThemeColors;
use crate::backup::git_ops::{CommitInfo, GitOps, GitStatus};
use crate::config::{BackupConfig, ClinConfig};
use crate::keybinds::Keybinds;
use ratatui_textarea::TextArea;
use std::path::PathBuf;
use std::sync::Arc;

pub struct BackupState {
    pub status: Option<GitStatus>,
    pub commits: Vec<CommitInfo>,
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
    pub theme: AppThemeColors,
    pub selected_file: Option<String>,
    pub diff_lines: Vec<String>,
    pub last_area: Option<ratatui::layout::Rect>,
    pub keybinds: Keybinds,
    pub tab_icons_only: bool,
    pub git_lock: Arc<parking_lot::Mutex<()>>,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub list_state: ratatui::widgets::ListState,
    pub history_list_state: ratatui::widgets::ListState,
    pub selected_commit_index: usize,
    pub mouse_pos: Option<(u16, u16)>,
    pub last_content_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<i32>,
    pub last_diff_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub diff_scroll_drag: Option<i32>,
    pub last_diff_area: Option<ratatui::layout::Rect>,
    pub scrollbars_enabled: bool,
    pub(crate) mouse_selection: Option<(BackupTextField, crate::text_edit::MouseTextSelection)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupTextField {
    CommitMessage,
    RemoteUrl,
    RemoteName,
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
    #[must_use]
    pub fn next(self) -> Self {
        Self::ORDER
            .iter()
            .cycle()
            .skip_while(|&&f| f != self)
            .nth(1)
            .copied()
            .unwrap_or(self)
    }
    #[must_use]
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
        tab_icons_only: bool,
        git_lock: Arc<parking_lot::Mutex<()>>,
        seq_matcher: crate::keybinds::KeyMatcher,
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
            settings,
            theme,
            keybinds,
            tab_icons_only,
            git_lock,
            seq_matcher,
            list_state: ratatui::widgets::ListState::default(),
            history_list_state: ratatui::widgets::ListState::default(),
            selected_commit_index: 0,
            mouse_pos: None,
            last_content_scroll: None,
            scroll_drag: None,
            last_diff_scroll: None,
            diff_scroll_drag: None,
            last_diff_area: None,
            scrollbars_enabled: false,
            mouse_selection: None,
        };

        state.refresh_git_info();
        state
    }

    pub(crate) fn with_git<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&GitOps) -> R,
    {
        let _guard = self.git_lock.lock();
        GitOps::init(&self.vault_path).as_ref().ok().map(f)
    }

    pub fn load_selected_diff(&mut self) {
        if let Some(path) = self.selected_file.clone() {
            if let Some(diff) = self.with_git(|g| g.get_file_diff(&path).unwrap_or_default()) {
                self.diff_lines = diff;
                self.diff_scroll = 0;
            }
        } else {
            self.diff_lines.clear();
        }
    }

    pub fn load_commit_diff(&mut self) {
        if let Some(commit) = self.commits.get(self.selected_commit_index) {
            let commit_id = commit.id.clone();
            if let Some(diff) = self.with_git(|g| g.get_commit_diff(&commit_id).unwrap_or_default())
            {
                self.diff_lines = diff;
                self.diff_scroll = 0;
            }
        } else {
            self.diff_lines.clear();
        }
    }

    pub fn refresh_git_info(&mut self) {
        if !self.settings.enabled {
            self.status = None;
            self.commits.clear();
            self.selectable_files.clear();
            self.selected_file = None;
            self.diff_lines.clear();
            return;
        }
        let mut need_diff = false;
        {
            let _g = self.git_lock.lock();
            if let Ok(git_ops) = GitOps::init(&self.vault_path) {
                self.status = git_ops.status().ok();
                self.commits = git_ops.log(50).unwrap_or_default();
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

                if let Some(file) = &self.selected_file {
                    if let Some(pos) = self.selectable_files.iter().position(|f| f == file) {
                        self.selected_index = pos;
                    } else {
                        if !self.selectable_files.is_empty() {
                            self.selected_file = Some(self.selectable_files[0].clone());
                            self.selected_index = 0;
                        } else {
                            self.selected_file = None;
                            self.selected_index = 0;
                        }
                        need_diff = true;
                    }
                } else if !self.selectable_files.is_empty() {
                    self.selected_file = Some(self.selectable_files[0].clone());
                    self.selected_index = 0;
                    need_diff = true;
                }
            }
        }
        if need_diff {
            self.load_selected_diff();
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

    pub fn rendered_index_for_file(&self, file_idx: usize) -> usize {
        let status = match &self.status {
            Some(s) => s,
            None => return 0,
        };
        let s_len = status.staged.len();
        if file_idx < s_len {
            1 + file_idx
        } else if s_len == 0 {
            4 + file_idx
        } else {
            3 + file_idx
        }
    }
    pub fn do_commit(&mut self, message: &str) {
        if let Some(msg) = self.with_git(|git_ops| match git_ops.commit(message) {
            Ok(_) => "Commit successful".to_string(),
            Err(e) => format!("Error: {e}"),
        }) {
            self.status_message = Some(msg);
        }
    }

    pub fn push_to_remote(&mut self) {
        let remote_name = self
            .settings
            .remote_name
            .lines()
            .join("")
            .trim()
            .to_string();
        self.status_message = Some(format!("Pushing to {remote_name}..."));

        if let Some(msg) = self.with_git(|git_ops| match git_ops.push(&remote_name) {
            Ok(_) => "Push complete".to_string(),
            Err(e) => format!("Push failed: {e}"),
        }) {
            self.status_message = Some(msg);
        }
    }

    pub fn pull_from_remote(&mut self) {
        let remote_name = self
            .settings
            .remote_name
            .lines()
            .join("")
            .trim()
            .to_string();
        self.status_message = Some(format!("Pulling from {remote_name}..."));

        if let Some(msg) = self.with_git(|git_ops| match git_ops.pull(&remote_name) {
            Ok(_) => "Pull complete".to_string(),
            Err(e) => format!("Pull failed: {e}"),
        }) {
            self.status_message = Some(msg);
        }
    }

    pub fn stage_file(&mut self, path: &str) {
        if let Some(msg) = self.with_git(|git_ops| match git_ops.add_paths(&[path.to_string()]) {
            Ok(_) => format!("Staged: {path}"),
            Err(e) => format!("Stage failed: {e}"),
        }) {
            self.status_message = Some(msg);
        }
    }

    pub fn unstage_file(&mut self, path: &str) {
        if let Some(msg) =
            self.with_git(|git_ops| match git_ops.unstage_paths(&[path.to_string()]) {
                Ok(_) => format!("Unstaged: {path}"),
                Err(e) => format!("Unstage failed: {e}"),
            })
        {
            self.status_message = Some(msg);
        }
    }

    pub fn stage_all(&mut self) {
        if let Some(msg) = self.with_git(|git_ops| match git_ops.add_all() {
            Ok(_) => "All changes staged".to_string(),
            Err(e) => format!("Stage all failed: {e}"),
        }) {
            self.status_message = Some(msg);
        }
    }

    pub fn save_settings(&mut self) {
        let mut config = ClinConfig::load().0.unwrap_or_default();

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
            let _g = self.git_lock.lock();
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
