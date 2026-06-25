use super::*;
use crate::list_view::*;

impl App {


    pub fn toggle_external_editor_mode(&mut self) {
        self.editor.external_editor_enabled = !self.editor.external_editor_enabled;
        let msg = if self.editor.external_editor_enabled {
            "External editor mode enabled"
        } else {
            "External editor mode disabled"
        };
        self.set_temporary_status(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.editor.external_enabled = self.editor.external_editor_enabled;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_pin(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let id = self.notes[*summary_idx].id.clone();
            match self.storage.toggle_pin(&id) {
                Ok(pinned) => {
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
                    if pinned {
                        self.set_temporary_status_static("Note pinned");
                    } else {
                        self.set_temporary_status_static("Note unpinned");
                    }
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to toggle pin: {e}"));
                }
            }
        } else {
            self.set_temporary_status_static("Select a note to pin/unpin");
        }
    }

    pub fn toggle_preview(&mut self) {
        self.list.preview_enabled = !self.list.preview_enabled;
        if self.list.preview_enabled {
            self.update_preview();
            self.set_temporary_status_static("Preview enabled");
        } else {
            self.list.preview_content = None;
            self.set_temporary_status_static("Preview disabled");
        }
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.preview_enabled = self.list.preview_enabled;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_calendar(&mut self) {
        self.list.calendar_enabled = !self.list.calendar_enabled;
        if self.list.calendar_enabled {
            self.set_temporary_status_static("Calendar enabled");
        } else {
            self.set_temporary_status_static("Calendar disabled");
        }
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.calendar_enabled = self.list.calendar_enabled;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_layout_edit(&mut self) {
        self.layout_edit = !self.layout_edit;
        self.layout_drag = None;
        self.set_temporary_status_static(if self.layout_edit {
            "Layout edit mode: drag borders / hjkl←→↑↓ / s swap / c cal / Esc"
        } else {
            "Layout edit mode off"
        });
        if !self.layout_edit {
            self.persist_list_layout();
        }
    }

    pub fn adjust_preview_width(&mut self, delta: f32) {
        self.adjust_preview_width_to(self.list.preview_width_ratio + delta);
        self.persist_list_layout();
    }

    pub fn adjust_preview_width_to(&mut self, ratio: f32) {
        self.list.preview_width_ratio = ratio.clamp(0.2, 0.8);
    }

    pub fn adjust_calendar_height(&mut self, delta: i16) {
        self.adjust_calendar_height_to(self.list.calendar_height.saturating_add_signed(delta));
        self.persist_list_layout();
    }

    pub fn adjust_calendar_height_to(&mut self, height: u16) {
        self.list.calendar_height = height.clamp(9, 20);
    }

    pub fn swap_preview_position(&mut self) {
        self.preview_position = match self.preview_position {
            crate::config::PreviewPosition::Left => crate::config::PreviewPosition::Right,
            crate::config::PreviewPosition::Right => crate::config::PreviewPosition::Left,
        };
        self.set_temporary_status_static(if matches!(self.preview_position, crate::config::PreviewPosition::Left) {
            "Preview moved to left"
        } else {
            "Preview moved to right"
        });
        self.persist_list_layout();
    }

    pub fn swap_calendar_position(&mut self) {
        self.calendar_position = match self.calendar_position {
            crate::config::CalendarPosition::Top => crate::config::CalendarPosition::Bottom,
            crate::config::CalendarPosition::Bottom => crate::config::CalendarPosition::Top,
        };
        self.set_temporary_status_static(if matches!(self.calendar_position, crate::config::CalendarPosition::Top) {
            "Calendar moved to top"
        } else {
            "Calendar moved to bottom"
        });
        self.persist_list_layout();
    }

    pub(crate) fn persist_list_layout(&mut self) {
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.preview_width_ratio = self.list.preview_width_ratio;
            config.list.calendar_height = self.list.calendar_height;
            config.list.preview_position = self.preview_position;
            config.list.calendar_position = self.calendar_position;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save layout: {e}"));
            }
        }
    }

    pub fn toggle_markdown_preview(&mut self) {
        self.editor.editor_preview_enabled = !self.editor.editor_preview_enabled;
        if self.editor.editor_preview_enabled {
            self.update_editor_markdown_preview();
            self.set_temporary_status_static("Markdown preview enabled");
        } else {
            self.editor.md_preview_renderer = None;
            self.set_temporary_status_static("Markdown preview disabled");
        }
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.editor.preview_enabled = self.editor.editor_preview_enabled;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }
    pub fn toggle_preview_fullscreen(&mut self) {
        if matches!(
            self.config.core.preview_expand_mode,
            crate::config::PreviewExpandMode::External
        ) {
            self.open_external_preview();
            return;
        }
        self.preview_fullscreen = !self.preview_fullscreen;
        match self.mode {
            ViewMode::Edit => self.update_editor_markdown_preview(),
            _ => self.update_preview(),
        }
        if self.preview_fullscreen {
            self.set_temporary_status_static("Preview expanded");
        } else {
            self.set_temporary_status_static("Preview restored");
        }
    }

    pub fn toggle_preview_wrap(&mut self) {
        self.preview_wrap = !self.preview_wrap;
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.core.preview_wrap = self.preview_wrap;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
        match self.mode {
            ViewMode::Edit => self.update_editor_markdown_preview(),
            _ => self.update_preview(),
        }
        self.set_temporary_status_static(if self.preview_wrap {
            "Wrap on"
        } else {
            "Wrap off"
        });
    }

    pub fn toggle_show_line_numbers(&mut self) {
        self.editor.show_line_numbers = !self.editor.show_line_numbers;
        let msg: &'static str = if self.editor.show_line_numbers {
            "Line numbers enabled"
        } else {
            "Line numbers disabled"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.editor.show_line_numbers = self.editor.show_line_numbers;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_confirm_on_delete(&mut self) {
        self.confirm_on_delete = !self.confirm_on_delete;
        let msg: &'static str = if self.confirm_on_delete {
            "Delete confirmation enabled"
        } else {
            "Delete confirmation disabled"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.core.confirm_on_delete = self.confirm_on_delete;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_confirm_on_quit(&mut self) {
        self.confirm_on_quit = !self.confirm_on_quit;
        let msg: &'static str = if self.confirm_on_quit {
            "Quit confirmation enabled"
        } else {
            "Quit confirmation disabled"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.core.confirm_on_quit = self.confirm_on_quit;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_preview_encryption(&mut self) {
        self.preview_encryption = !self.preview_encryption;
        let msg: &'static str = if self.preview_encryption {
            "Encrypted note previews enabled"
        } else {
            "Encrypted note previews hidden"
        };
        self.set_temporary_status_static(msg);
        if self.list.preview_enabled {
            self.update_preview();
        }
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.preview_encryption = self.preview_encryption;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_pinned_on_top(&mut self) {
        self.pinned_on_top = !self.pinned_on_top;
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        let msg: &'static str = if self.pinned_on_top {
            "Pinned notes shown on top"
        } else {
            "Pinned notes in natural order"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.pinned_on_top = self.pinned_on_top;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_show_hidden_files(&mut self) {
        self.list.show_hidden_files = !self.list.show_hidden_files;
        self.list.folder_cache = None; // invalidate cached folder list
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        let msg: &'static str = if self.list.show_hidden_files {
            "Hidden files shown"
        } else {
            "Hidden files hidden"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.show_hidden_files = self.list.show_hidden_files;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_tab_icons_only(&mut self) {
        self.config.ui.tab_icons_only = !self.config.ui.tab_icons_only;
        let msg: &'static str = if self.config.ui.tab_icons_only {
            "Tab icons only"
        } else {
            "Tab icons + labels"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.ui.tab_icons_only = self.config.ui.tab_icons_only;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_notes_layout(&mut self) {
        self.list.notes_layout = match self.list.notes_layout {
            crate::config::NotesLayout::Tree => crate::config::NotesLayout::Grid,
            crate::config::NotesLayout::Grid => crate::config::NotesLayout::Tree,
        };
        self.list.visual_index = 0;
        // #1: entering grid always opens the Vault tab (grid_folder == "")
        self.list.grid_folder = String::new();
        self.refresh_visual_list();
        // #2: persist
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.default_view = self.list.notes_layout.clone();
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }
    pub fn load_goals_progress(&self) -> crate::goals::DailyProgress {
        let path = self.storage.config_dir.join("goals_progress.json");
        let today = chrono::Local::now().date_naive().to_string();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(progress) = serde_json::from_str::<crate::goals::DailyProgress>(&content)
                {
                    if progress.date == today {
                        return progress;
                    }
                }
            }
        }
        crate::goals::DailyProgress {
            date: today,
            words_written: 0,
            notes_modified: std::collections::HashSet::new(),
        }
    }

    pub fn save_goals_progress(&self, progress: &crate::goals::DailyProgress) {
        let path = self.storage.config_dir.join("goals_progress.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string(progress) {
            let _ = std::fs::write(&path, content);
        }
    }

    pub fn get_current_goals_progress(&mut self) -> &mut crate::goals::DailyProgress {
        self.check_and_reload_config();
        let today = chrono::Local::now().date_naive().to_string();
        if self.goals_progress.date != today {
            self.goals_progress = self.load_goals_progress();
        }
        &mut self.goals_progress
    }
}
