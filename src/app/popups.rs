use crate::debug_log;
use super::*;
use crate::list_view::*;
use crate::popups::*;
use crate::templates::Template;
use ratatui_textarea::TextArea;

impl App {


    pub fn open_template_popup(&mut self) {
        let template_manager = self.storage.template_manager();
        match template_manager.list() {
            Ok(templates) => {
                debug_log!(self, Debug, "view", "Template popup opened ({} templates)", templates.len());
                let mut input = TextArea::default();
                input.set_style(self.app_theme.bg_style());
                input.set_cursor_line_style(Style::default());
                input.set_placeholder_text("Search templates...");
                self.popups.template = Some(TemplatePopup {
                    all_templates: templates.clone(),
                    filtered_templates: templates,
                    input,
                    selected: 0,
                    focus: crate::popups::TemplatePopupFocus::Search,
                });
            }
            Err(_) => {
                debug_log!(self, Warn, "templates", "Failed to load templates");
                self.set_temporary_status_static("Failed to load templates");
            }
        }
    }

    pub fn close_template_popup(&mut self) {
        self.popups.template = None;
    }

    pub fn select_template(&mut self) {
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            if Self::is_virtual_pinned_path(&self.list.grid_folder) {
                String::new()
            } else {
                self.list.grid_folder.clone()
            }
        } else {
            self.get_current_folder_context()
        };
        if let Some(popup) = self.popups.template.take()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            let template_manager = self.storage.template_manager();
            if let Ok(template) = template_manager.load(&summary.filename) {
                self.start_note_from_template(&template, folder);
            } else {
                debug_log!(self, Warn, "templates", "Failed to load selected template");
                self.set_temporary_status_static("Failed to load selected template");
            }
        }
    }

    pub fn edit_selected_template_from_popup(&mut self) {
        let path = if let Some(popup) = self.popups.template.as_ref()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            self.storage
                .template_manager()
                .template_path(&summary.filename)
        } else {
            self.set_temporary_status_static("No template selected");
            return;
        };

        self.popups.template = None;
        self.open_template_path_in_editor(&path);
    }

    fn open_template_path_in_editor(&mut self, path: &std::path::Path) {
        if self.editor.external_editor_enabled {
            self.open_path_in_external_editor(path);
            self.sync_template_filename(path);
            self.refresh_template_popup();
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                debug_log!(self, Error, "templates", "Failed to load template: {e}");
                self.set_temporary_status(&format!("Failed to load template: {e}"));
                return;
            }
        };

        self.mode = ViewMode::Edit;
        self.editor.editing_id = None;
        self.editor.template_edit_path = Some(path.to_path_buf());
        self.editor.title_editor = make_title_editor(
            &format!(
                "Template: {}",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("template")
            ),
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = text_area_from_content(&content);
        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());
        self.set_temporary_status_static("Editing template (Esc to save and return)");
    }

    fn sync_template_filename(&mut self, path: &std::path::Path) -> std::path::PathBuf {
        let template = match Template::load(path) {
            Ok(t) => t,
            Err(_) => return path.to_path_buf(),
        };

        let new_path = self
            .storage
            .template_manager()
            .template_path(&template.name);

        if new_path == path {
            return path.to_path_buf();
        }

        if new_path.exists() {
            return path.to_path_buf();
        }

        if let Err(e) = std::fs::rename(path, &new_path) {
            self.set_temporary_status(&format!("Failed to rename template: {e}"));
            return path.to_path_buf();
        }

        new_path
    }

    pub fn update_template_popup_filter(&mut self) {
        if let Some(popup) = &mut self.popups.template {
            let query = popup.input.lines()[0].trim().to_lowercase();
            if query.is_empty() {
                popup.filtered_templates = popup.all_templates.clone();
            } else {
                popup.filtered_templates = popup
                    .all_templates
                    .iter()
                    .filter(|t| {
                        t.name.to_lowercase().contains(&query)
                            || t.filename.to_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect();
            }
            if popup.selected >= popup.filtered_templates.len() {
                popup.selected = popup.filtered_templates.len().saturating_sub(1);
            }
        }
    }

    pub fn begin_delete_selected_template_from_popup(&mut self) {
        let (filename, name) = if let Some(popup) = self.popups.template.as_ref()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            (summary.filename.clone(), summary.name.clone())
        } else {
            self.set_temporary_status_static("No template selected");
            return;
        };

        self.show_confirm(ConfirmAction::DeleteTemplate { filename, name });
    }

    pub fn refresh_template_popup(&mut self) {
        let template_manager = self.storage.template_manager();
        if let Some(popup) = &mut self.popups.template {
            let selected = popup.selected;
            let focus = popup.focus;
            match template_manager.list() {
                Ok(all_templates) => {
                    popup.all_templates = all_templates;
                    popup.focus = focus;
                    self.update_template_popup_filter();
                    if let Some(popup) = &mut self.popups.template {
                        if popup.filtered_templates.is_empty() {
                            popup.selected = 0;
                        } else {
                            popup.selected = selected.min(popup.filtered_templates.len() - 1);
                        }
                    }
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to refresh templates: {e}"));
                }
            }
        }
    }

    pub fn confirm_delete_template(&mut self, filename: String) {
        let template_manager = self.storage.template_manager();
        match template_manager.delete(&filename) {
            Ok(()) => {
                self.refresh_template_popup();
                self.set_temporary_status_static("Template deleted");
            }
            Err(e) => {
                debug_log!(self, Error, "templates", "Failed to delete template: {e}");
                self.set_temporary_status(&format!("Failed to delete template: {e}"));
            }
        }
    }

    pub fn create_template_from_popup(&mut self) {
        let template_manager = self.storage.template_manager();
        if let Err(e) = template_manager.ensure_dir() {
            self.set_temporary_status(&format!("Failed to prepare templates dir: {e}"));
            return;
        }

        let mut idx = 1usize;
        let filename = loop {
            let candidate = if idx == 1 {
                "new_template".to_string()
            } else {
                format!("new_template_{idx}")
            };
            let path = template_manager.template_path(&candidate);
            if !path.exists() {
                break candidate;
            }
            idx += 1;
        };

        let path = template_manager.template_path(&filename);
        let skeleton = r#"name = "New Template"

[title]
template = "Note - {date}"

[content]
template = """
"""
"#;

        if let Err(e) = std::fs::write(&path, skeleton) {
            self.set_temporary_status(&format!("Failed to create template: {e}"));
            return;
        }

        self.refresh_template_popup();
        self.open_template_path_in_editor(&path);
    }

    pub fn show_confirm(&mut self, action: ConfirmAction) {
        let (message, detail, confirm_label, is_destructive) = match &action {
            ConfirmAction::DeleteNote { title, .. } => (
                format!("Move \"{title}\" to trash?"),
                Some("Use Shift+T to view/restore trashed notes.".into()),
                "Trash".into(),
                false,
            ),
            ConfirmAction::DeleteFolder { path } => (
                format!("Move folder \"{path}\" and all contents to trash?"),
                Some("Use Shift+T to view/restore trashed notes.".into()),
                "Trash".into(),
                false,
            ),
            ConfirmAction::DeleteTag { tag } => (
                format!("Delete tag \"{tag}\"?"),
                Some("This will remove the tag from all notes.".into()),
                "Delete".into(),
                true,
            ),
            ConfirmAction::DeleteTemplate { name, .. } => (
                format!("Delete template \"{name}\"?"),
                Some("This removes template file permanently.".into()),
                "Delete".into(),
                true,
            ),
            ConfirmAction::DeleteFromTrash { item } => (
                format!("Permanently delete \"{}\"?", item.name.to_string_lossy()),
                Some("This action cannot be undone.".into()),
                "Delete Forever".into(),
                true,
            ),
            ConfirmAction::EmptyTrash { items } => (
                format!("Permanently delete {} note(s)?", items.len()),
                Some("This action cannot be undone.".into()),
                "Empty Trash".into(),
                true,
            ),
            ConfirmAction::BulkDeleteItems { note_ids, folder_paths } => {
                let mut parts = Vec::new();
                if !note_ids.is_empty()     { parts.push(format!("{} note(s)", note_ids.len())); }
                if !folder_paths.is_empty() { parts.push(format!("{} folder(s)", folder_paths.len())); }
                (
                    format!("Move {} to trash?", parts.join(" and ")),
                    Some("Use Shift+T to view/restore trashed notes.".into()),
                    "Trash".into(),
                    false,
                )
            }
            ConfirmAction::QuitApp => (
                "Are you sure you want to quit?".into(),
                None,
                "Quit".into(),
                false,
            ),
        };

        self.popups.confirm = Some(ConfirmPopup {
            action,
            message,
            detail,
            confirm_label,
            is_destructive,
            selected_button: 1,
        });
    }

    pub fn confirm_action(&mut self) {
        if let Some(popup) = self.popups.confirm.take() {
            match popup.action {
                ConfirmAction::DeleteNote { note_id, .. } => {
                    self.confirm_delete_selected(note_id);
                }
                ConfirmAction::DeleteFolder { path } => {
                    self.confirm_delete_folder(path);
                }
                ConfirmAction::DeleteTag { tag } => {
                    self.confirm_delete_tag(tag);
                }
                ConfirmAction::DeleteTemplate { filename, .. } => {
                    self.confirm_delete_template(filename);
                }
                ConfirmAction::DeleteFromTrash { item } => {
                    self.confirm_delete_from_trash(item);
                }
                ConfirmAction::EmptyTrash { items } => {
                    self.confirm_empty_trash(items);
                }
                ConfirmAction::BulkDeleteItems { note_ids, folder_paths } => {
                    self.confirm_bulk_delete(note_ids, folder_paths);
                }
                ConfirmAction::QuitApp => {
                    self.should_quit = true;
                }
            }
        }
    }

    pub fn cancel_confirm(&mut self) {
        self.popups.confirm = None;
    }

    pub fn confirm_popup_select_confirm(&mut self) {
        if let Some(popup) = &mut self.popups.confirm {
            popup.selected_button = 0;
        }
    }

    pub fn confirm_popup_select_cancel(&mut self) {
        if let Some(popup) = &mut self.popups.confirm {
            popup.selected_button = 1;
        }
    }

    pub fn confirm_popup_toggle_button(&mut self) {
        if let Some(popup) = &mut self.popups.confirm {
            popup.selected_button = (popup.selected_button + 1) % 2;
        }
    }

    pub fn confirm_popup_activate(&mut self) {
        let is_confirm = self
            .popups
            .confirm
            .as_ref()
            .map(|p| p.selected_button == 0)
            .unwrap_or(false);
        if is_confirm {
            self.confirm_action();
        } else {
            self.cancel_confirm();
        }
    }

    pub fn confirm_bulk_delete(&mut self, note_ids: Vec<String>, folder_paths: Vec<String>) {
        let mut failed = 0;
        for id in &note_ids {
            if self.storage.trash_note(id).is_err() { failed += 1; }
        }
        for path in &folder_paths {
            if self.storage.trash_folder(path).is_err() { failed += 1; }
        }
        // Drop expanded state for every trashed folder + its descendants.
        for path in &folder_paths {
            self.list.folder_expanded.remove(path);
            self.list.folder_expanded.retain(|p| !p.starts_with(&format!("{path}/")));
        }
        self.list.folder_cache = None;
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        self.clamp_visual_index();
        self.list.selected_indices.clear();
        self.list.list_mode = ListMode::Normal;

        let total = note_ids.len() + folder_paths.len();
        if failed > 0 {
            self.set_temporary_status(&format!("Failed to trash {failed} item(s)"));
        } else {
            self.set_temporary_status(&format!("Moved {total} item(s) to trash"));
        }
        debug_log!(self, Info, "storage", "Bulk trash: {} succeeded, {failed} failed", total - failed);
    }

    pub fn close_create_format_popup(&mut self) {
        self.popups.create_format = None;
    }

    pub fn cycle_sort(&mut self) {
        match (self.list.sort_field, self.list.sort_order) {
            (SortField::Modified, SortOrder::Descending) => {
                self.list.sort_field = SortField::Modified;
                self.list.sort_order = SortOrder::Ascending;
            }
            (SortField::Modified, SortOrder::Ascending) => {
                self.list.sort_field = SortField::Title;
                self.list.sort_order = SortOrder::Ascending;
            }
            (SortField::Title, SortOrder::Ascending) => {
                self.list.sort_field = SortField::Title;
                self.list.sort_order = SortOrder::Descending;
            }
            (SortField::Title, SortOrder::Descending) => {
                self.list.sort_field = SortField::Modified;
                self.list.sort_order = SortOrder::Descending;
            }
        }
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        let sort_desc = match (self.list.sort_field, self.list.sort_order) {
            (SortField::Modified, SortOrder::Descending) => "Sort: Modified (newest)",
            (SortField::Modified, SortOrder::Ascending) => "Sort: Modified (oldest)",
            (SortField::Title, SortOrder::Ascending) => "Sort: Title (A-Z)",
            (SortField::Title, SortOrder::Descending) => "Sort: Title (Z-A)",
        };
        debug_log!(self, Debug, "config", "Sort changed to {sort_desc}");
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.default_sort_field = Some(self.list.sort_field);
            config.list.default_sort_order = Some(self.list.sort_order);
            if let Err(e) = config.save() {
                debug_log!(self, Error, "config", "Config save failed (sort): {e}");
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn begin_theme_selection(&mut self) {
        let themes = vec![
            "default".to_string(),
            "tokyo_night".to_string(),
            "catppuccin_mocha".to_string(),
            "onedark".to_string(),
            "gruvbox".to_string(),
            "dracula".to_string(),
            "nord".to_string(),
            "rose_pine".to_string(),
            "everforest".to_string(),
            "kanagawa".to_string(),
            "solarized".to_string(),
        ];

        let config = crate::config::ClinConfig::load().unwrap_or_default();
        let current = config.ui.theme.to_string();
        debug_log!(self, Debug, "view", "Theme selection opened (current={current})");
        let selected = themes.iter().position(|t| t == &current).unwrap_or(0);
        let general_is_solid = matches!(config.ui.background, crate::config::Background::Solid);
        let graph_is_solid = matches!(
            config.graf.visual.graph_background,
            crate::config::Background::Solid
        );

        self.popups.theme = Some(ThemePopup {
            themes,
            selected,
            focus: ThemePopupFocus::ThemeList,
            general_is_solid,
            graph_is_solid,
        });
    }

    pub fn begin_sort_selection(&mut self) {
        use crate::list_view::{SortField, SortOrder};
        let current_idx = match (self.list.sort_field, self.list.sort_order) {
            (SortField::Title, SortOrder::Ascending) => 0,
            (SortField::Title, SortOrder::Descending) => 1,
            (SortField::Modified, SortOrder::Descending) => 2,
            (SortField::Modified, SortOrder::Ascending) => 3,
        };
        self.popups.sort = Some(crate::popups::SortPopup {
            selected: current_idx,
        });
    }

    pub fn select_sort(&mut self) {
        if let Some(popup) = self.popups.sort.take() {
            use crate::list_view::{SortField, SortOrder};
            match popup.selected {
                0 => {
                    self.list.sort_field = SortField::Title;
                    self.list.sort_order = SortOrder::Ascending;
                }
                1 => {
                    self.list.sort_field = SortField::Title;
                    self.list.sort_order = SortOrder::Descending;
                }
                2 => {
                    self.list.sort_field = SortField::Modified;
                    self.list.sort_order = SortOrder::Descending;
                }
                3 => {
                    self.list.sort_field = SortField::Modified;
                    self.list.sort_order = SortOrder::Ascending;
                }
                _ => {}
            }
            if let Err(e) = self.refresh_notes() {
                self.set_temporary_status(&format!("Refresh failed: {e}"));
            }
            debug_log!(self, Debug, "view", "Sort: {:?}", (self.list.sort_field, self.list.sort_order));
        }
    }

    pub fn close_sort_popup(&mut self) {
        self.popups.sort = None;
    }

    pub fn begin_icon_mode_selection(&mut self) {
        let current_idx = match self.config.ui.icon_mode {
            crate::config::IconMode::Nerd => 0,
            crate::config::IconMode::Unicode => 1,
            crate::config::IconMode::None => 2,
        };
        self.popups.icon_mode = Some(crate::popups::IconModePopup {
            selected: current_idx,
        });
    }

    pub fn select_icon_mode(&mut self) {
        if let Some(popup) = self.popups.icon_mode.take() {
            let mode = match popup.selected {
                0 => crate::config::IconMode::Nerd,
                1 => crate::config::IconMode::Unicode,
                _ => crate::config::IconMode::None,
            };
            self.config.ui.icon_mode = mode;
            let status = match mode {
                crate::config::IconMode::Nerd => "Icon mode: Nerd Font",
                crate::config::IconMode::Unicode => "Icon mode: Unicode",
                crate::config::IconMode::None => "Icon mode: None",
            };
            self.set_temporary_status_static(status);
            if let Ok(mut config) = crate::config::ClinConfig::load() {
                config.ui.icon_mode = mode;
                if let Err(e) = config.save() {
                    debug_log!(self, Error, "config", "Failed to save config (icon mode): {e}");
                    self.set_temporary_status(&format!("Failed to save config: {e}"));
                }
            }
        }
    }

    pub fn close_icon_mode_popup(&mut self) {
        self.popups.icon_mode = None;
    }

    pub fn begin_hint_bar_style_selection(&mut self) {
        let current_idx = match self.config.ui.hint_bar_style {
            crate::config::HintBarStyle::Classic => 0,
            crate::config::HintBarStyle::Accent => 1,
            crate::config::HintBarStyle::PowerlineSharp => 2,
            crate::config::HintBarStyle::PowerlineRounded => 3,
            crate::config::HintBarStyle::PowerlineSlanted => 4,
        };
        self.popups.hint_bar_style = Some(crate::popups::HintBarStylePopup {
            selected: current_idx,
        });
    }

    pub fn select_hint_bar_style(&mut self) {
        if let Some(popup) = self.popups.hint_bar_style.take() {
            let style = match popup.selected {
                0 => crate::config::HintBarStyle::Classic,
                1 => crate::config::HintBarStyle::Accent,
                2 => crate::config::HintBarStyle::PowerlineSharp,
                3 => crate::config::HintBarStyle::PowerlineRounded,
                _ => crate::config::HintBarStyle::PowerlineSlanted,
            };
            self.config.ui.hint_bar_style = style;
            self.app_theme.hint_bar_style = style;
            let status = match style {
                crate::config::HintBarStyle::Classic => "Hint bar style: Classic",
                crate::config::HintBarStyle::Accent => "Hint bar style: Accent",
                crate::config::HintBarStyle::PowerlineSharp => "Hint bar style: Powerline Sharp",
                crate::config::HintBarStyle::PowerlineRounded => "Hint bar style: Powerline Rounded",
                crate::config::HintBarStyle::PowerlineSlanted => "Hint bar style: Powerline Slanted",
            };
            self.set_temporary_status_static(status);
            if let Ok(mut config) = crate::config::ClinConfig::load() {
                config.ui.hint_bar_style = style;
                if let Err(e) = config.save() {
                    debug_log!(self, Error, "config", "Failed to save config (hint bar style): {e}");
                    self.set_temporary_status(&format!("Failed to save config: {e}"));
                }
            }
            self.popups.hint_bar_style = Some(popup);
        }
    }

    pub fn close_hint_bar_style_popup(&mut self) {
        self.popups.hint_bar_style = None;
    }

    pub fn begin_keybind_preset_selection(&mut self) {
        let selected = match self.config.core.keybind_preset {
            crate::config::KeybindPreset::Default => 0,
            crate::config::KeybindPreset::Helix => 1,
            crate::config::KeybindPreset::Vim => 2,
            crate::config::KeybindPreset::Emacs => 3,
        };
        self.popups.keybind_preset = Some(crate::popups::KeybindPresetPopup { selected });
    }

    pub fn select_keybind_preset(&mut self) {
        if let Some(popup) = self.popups.keybind_preset.take() {
            let new = match popup.selected {
                0 => crate::config::KeybindPreset::Default,
                1 => crate::config::KeybindPreset::Helix,
                2 => crate::config::KeybindPreset::Vim,
                _ => crate::config::KeybindPreset::Emacs,
            };
            self.config.core.keybind_preset = new;
            self.apply_keybind_preset(new);
            self.set_temporary_status(&format!("Keybind preset: {new}"));
            if let Ok(mut c) = crate::config::ClinConfig::load() {
                c.core.keybind_preset = new;
                let _ = c.save();
            }
            self.popups.keybind_preset = Some(popup);
        }
    }

    pub fn close_keybind_preset_popup(&mut self) {
        self.popups.keybind_preset = None;
    }

    pub fn apply_keybind_preset(&mut self, preset: crate::config::KeybindPreset) {
        self.keybinds = self.storage.load_keybinds_with_preset(preset);
        self.seq_matcher.clear();
    }

    pub fn select_theme(&mut self) {
        if let Some(mut popup) = self.popups.theme.take() {
            match popup.focus {
                ThemePopupFocus::ThemeList => {
                    let next_theme = popup.themes[popup.selected].clone();
                    let mut config = crate::config::ClinConfig::load().unwrap_or_default();
                    config.ui.theme = next_theme.parse().unwrap_or_default();
                    if let Err(e) = config.save() {
                        self.set_temporary_status(&format!("Failed to save theme: {e}"));
                        return;
                    }
                    self.reload_theme();
                    debug_log!(self, Info, "view", "Theme changed to {next_theme}");
                    self.set_temporary_status(&format!("Theme set to: {next_theme}"));
                    self.popups.theme = Some(popup);
                }
                ThemePopupFocus::GeneralBg => {
                    popup.general_is_solid = !popup.general_is_solid;
                    let mut config = crate::config::ClinConfig::load().unwrap_or_default();
                    config.ui.background = if popup.general_is_solid {
                        crate::config::Background::Solid
                    } else {
                        crate::config::Background::Transparent
                    };
                    if let Err(e) = config.save() {
                        self.set_temporary_status(&format!("Failed to save bg: {e}"));
                    }
                    self.reload_theme();
                    self.popups.theme = Some(popup);
                }
                ThemePopupFocus::GraphBg => {
                    popup.graph_is_solid = !popup.graph_is_solid;
                    let mut config = crate::config::ClinConfig::load().unwrap_or_default();
                    config.graf.visual.graph_background = if popup.graph_is_solid {
                        crate::config::Background::Solid
                    } else {
                        crate::config::Background::Transparent
                    };
                    if let Err(e) = config.save() {
                        self.set_temporary_status(&format!("Failed to save graph bg: {e}"));
                    }
                    self.popups.theme = Some(popup);
                }
            }
        }
    }

    pub fn close_theme_popup(&mut self) {
        self.popups.theme = None;
    }

    pub fn begin_set_word_goal(&mut self) {
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_placeholder_text("Enter daily word goal (e.g. 500)");
        if self.config.goals.word_goal > 0 {
            input.insert_str(&self.config.goals.word_goal.to_string());
        }
        self.popups.goals = Some(crate::popups::GoalsPopup {
            mode: crate::popups::GoalsPopupMode::WordGoal,
            input,
        });
    }

    pub fn begin_set_note_goal(&mut self) {
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_placeholder_text("Enter daily note goal (e.g. 3)");
        if self.config.goals.note_goal > 0 {
            input.insert_str(&self.config.goals.note_goal.to_string());
        }
        self.popups.goals = Some(crate::popups::GoalsPopup {
            mode: crate::popups::GoalsPopupMode::NoteGoal,
            input,
        });
    }

    pub fn confirm_goals_popup(&mut self) {
        if let Some(popup) = self.popups.goals.take() {
            let val_str = popup.input.lines().join("");
            match val_str.trim().parse::<usize>() {
                Ok(val) => {
                    let mut config = crate::config::ClinConfig::load().unwrap_or_default();
                    match popup.mode {
                        crate::popups::GoalsPopupMode::WordGoal => {
                            config.goals.word_goal = val;
                            self.config.goals.word_goal = val;
                            self.set_temporary_status(&format!("Daily word goal set to {val}"));
                        }
                        crate::popups::GoalsPopupMode::NoteGoal => {
                            config.goals.note_goal = val;
                            self.config.goals.note_goal = val;
                            self.set_temporary_status(&format!("Daily note goal set to {val}"));
                        }
                    }
                    let _ = config.save();
                }
                Err(_) => {
                    self.set_temporary_status_static("Please enter a valid positive number.");
                }
            }
        }
    }}
