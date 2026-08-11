use super::*;
use crate::editor_document::EditorDocument;
use crate::list_view::*;
use crate::popups::*;
use crate::templates::Template;

impl App {
    pub fn open_template_popup(&mut self) {
        let template_manager = self.storage.template_manager();
        match template_manager.list() {
            Ok(templates) => {
                let input = crate::ui::make_popup_textarea(&self.app_theme, "Search templates...");
                self.popups.active = Some(crate::popups::ActivePopup::Template(TemplatePopup {
                    all_templates: templates.clone(),
                    filtered_templates: templates,
                    input,
                    selected: 0,
                    scroll_offset: 0,
                    focus: crate::popups::TemplatePopupFocus::Search,
                    last_scroll: None,
                }));
            }
            Err(e) => {
                self.set_temporary_status_static("Failed to load templates");
                self.messages.push(
                    format!("Failed to load templates: {e}"),
                    crate::app::messages::MessageSeverity::Warning,
                );
            }
        }
    }

    pub fn close_template_popup(&mut self) {
        self.popups.active = None;
    }

    pub fn select_template(&mut self) {
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            if Self::is_virtual_path(&self.list.grid_folder) {
                String::new()
            } else {
                self.list.grid_folder.clone()
            }
        } else {
            self.get_current_folder_context()
        };
        if let Some(crate::popups::ActivePopup::Template(popup)) = self.popups.active.take()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            let template_manager = self.storage.template_manager();
            match template_manager.load(&summary.filename) {
                Ok(template) => {
                    self.start_note_from_template(&template, folder);
                }
                Err(e) => {
                    self.set_temporary_status_static("Failed to load selected template");
                    self.messages.push(
                        format!("Failed to load selected template: {e}"),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                }
            }
        }
    }

    pub fn edit_selected_template_from_popup(&mut self) {
        let path = if let Some(crate::popups::ActivePopup::Template(popup)) =
            self.popups.active.as_ref()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            self.storage
                .template_manager()
                .template_path(&summary.filename)
        } else {
            self.set_temporary_status_static("No template selected");
            return;
        };

        self.popups.active = None;
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
        self.editor.body = EditorDocument::from_text(&content);
        self.apply_editor_prefs();
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
        if let Some(crate::popups::ActivePopup::Template(popup)) = &mut self.popups.active {
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
        let (filename, name) = if let Some(crate::popups::ActivePopup::Template(popup)) =
            self.popups.active.as_ref()
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
        if let Some(crate::popups::ActivePopup::Template(popup)) = &mut self.popups.active {
            let selected = popup.selected;
            let focus = popup.focus;
            match template_manager.list() {
                Ok(all_templates) => {
                    popup.all_templates = all_templates;
                    popup.focus = focus;
                    self.update_template_popup_filter();
                    if let Some(crate::popups::ActivePopup::Template(popup)) =
                        &mut self.popups.active
                    {
                        if popup.filtered_templates.is_empty() {
                            popup.selected = 0;
                        } else {
                            popup.selected =
                                selected.min(popup.filtered_templates.len().saturating_sub(1));
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

        if let Err(e) = crate::fsutil::atomic_write_str(&path, skeleton) {
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
            ConfirmAction::BulkDeleteItems {
                note_ids,
                folder_paths,
            } => {
                let mut parts = Vec::new();
                if !note_ids.is_empty() {
                    parts.push(format!("{} note(s)", note_ids.len()));
                }
                if !folder_paths.is_empty() {
                    parts.push(format!("{} folder(s)", folder_paths.len()));
                }
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
            ConfirmAction::RemoveAllTagsFromSelected => (
                "Remove ALL tags from selected notes?".into(),
                Some("This cannot be undone.".into()),
                "Remove All".into(),
                true,
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
                ConfirmAction::BulkDeleteItems {
                    note_ids,
                    folder_paths,
                } => {
                    self.confirm_bulk_delete(note_ids, folder_paths);
                }
                ConfirmAction::QuitApp => {
                    self.should_quit = true;
                }
                ConfirmAction::RemoveAllTagsFromSelected => {
                    self.confirm_remove_all_tags();
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
            if self.storage.trash_note(id).is_err() {
                failed += 1;
            }
        }
        for path in &folder_paths {
            if self.storage.trash_folder(path).is_err() {
                failed += 1;
            }
        }
        // Drop expanded state for every trashed folder + its descendants.
        for path in &folder_paths {
            self.list.folder_expanded.remove(path);
            self.list
                .folder_expanded
                .retain(|p| !p.starts_with(&format!("{path}/")));
        }
        self.request_notes_reconcile();
        self.clamp_visual_index();
        self.list.selected_indices.clear();
        self.list.list_mode = ListMode::Normal;

        let total = note_ids.len() + folder_paths.len();
        if failed > 0 {
            self.set_temporary_status(&format!("Failed to trash {failed} item(s)"));
        } else {
            self.set_temporary_status(&format!("Moved {total} item(s) to trash"));
        }
    }

    pub fn close_create_format_popup(&mut self) {
        self.popups.active = None;
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
        self.sort_notes();
        self.refresh_visual_list();

        if let Ok(mut config) = crate::config::ClinConfig::load().0 {
            config.list.default_sort_field = Some(self.list.sort_field);
            config.list.default_sort_order = Some(self.list.sort_order);
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn begin_theme_selection(&mut self) {
        let builtin_count = crate::config::Theme::BUILTIN_NAMES.len();
        let mut themes: Vec<String> = crate::config::Theme::BUILTIN_NAMES
            .iter()
            .map(|s| s.to_string())
            .collect();
        themes.extend(crate::config::custom_themes::list_custom_themes());
        let is_custom: Vec<bool> = (0..themes.len()).map(|i| i >= builtin_count).collect();

        let config = crate::config::ClinConfig::load().0.unwrap_or_default();
        let current = config.ui.theme.clone();

        let selected = themes.iter().position(|t| t == &current).unwrap_or(0);
        let general_is_solid = matches!(config.ui.background, crate::config::Background::Solid);
        let graph_is_solid = matches!(
            config.graf.visual.graph_background,
            crate::config::Background::Solid
        );

        self.popups.active = Some(crate::popups::ActivePopup::Theme(ThemePopup {
            themes,
            selected,
            is_custom,
            focus: ThemePopupFocus::ThemeList,
            scroll_offset: 0,
            general_is_solid,
            graph_is_solid,
            last_scroll: None,
        }));
    }

    pub fn begin_sort_selection(&mut self) {
        use crate::list_view::{SortField, SortOrder};
        let current_idx = match (self.list.sort_field, self.list.sort_order) {
            (SortField::Title, SortOrder::Ascending) => 0,
            (SortField::Title, SortOrder::Descending) => 1,
            (SortField::Modified, SortOrder::Descending) => 2,
            (SortField::Modified, SortOrder::Ascending) => 3,
        };
        self.popups.active = Some(crate::popups::ActivePopup::Sort(crate::popups::SortPopup {
            selected: current_idx,
        }));
    }

    pub fn select_sort(&mut self) {
        if let Some(crate::popups::ActivePopup::Sort(popup)) = self.popups.active.take() {
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
            self.sort_notes();
            self.refresh_visual_list();
        }
    }

    pub fn close_sort_popup(&mut self) {
        self.popups.active = None;
    }

    pub fn begin_icon_mode_selection(&mut self) {
        let current_idx = match self.config.ui.icon_mode {
            crate::config::IconMode::Nerd => 0,
            crate::config::IconMode::Unicode => 1,
            crate::config::IconMode::None => 2,
        };
        self.popups.active = Some(crate::popups::ActivePopup::IconMode(
            crate::popups::IconModePopup {
                selected: current_idx,
            },
        ));
    }

    pub fn select_icon_mode(&mut self) {
        if let Some(crate::popups::ActivePopup::IconMode(popup)) = self.popups.active.take() {
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
            if let Ok(mut config) = crate::config::ClinConfig::load().0 {
                config.ui.icon_mode = mode;
                if let Err(e) = config.save() {
                    self.set_temporary_status(&format!("Failed to save config: {e}"));
                }
            }
        }
    }

    pub fn close_icon_mode_popup(&mut self) {
        self.popups.active = None;
    }

    pub fn begin_hint_bar_style_selection(&mut self) {
        let current_idx = self.config.ui.hint_bar_style.index();
        self.popups.active = Some(crate::popups::ActivePopup::HintBarStyle(
            crate::popups::HintBarStylePopup {
                selected: current_idx,
            },
        ));
    }

    pub fn select_hint_bar_style(&mut self) {
        if let Some(crate::popups::ActivePopup::HintBarStyle(popup)) = self.popups.active.take() {
            let style = crate::config::HintBarStyle::from_index(popup.selected);
            self.config.ui.hint_bar_style = style;
            self.app_theme.hint_bar_style = style;
            self.set_temporary_status(&format!("Hint bar style: {}", style.name()));
            if let Ok(mut config) = crate::config::ClinConfig::load().0 {
                config.ui.hint_bar_style = style;
                if let Err(e) = config.save() {
                    self.set_temporary_status(&format!("Failed to save config: {e}"));
                }
            }
            self.popups.active = Some(crate::popups::ActivePopup::HintBarStyle(popup));
        }
    }

    pub fn close_hint_bar_style_popup(&mut self) {
        self.popups.active = None;
    }

    pub fn begin_keybind_preset_selection(&mut self) {
        let selected = match self.config.core.keybind_preset {
            crate::config::KeybindPreset::Default => 0,
            crate::config::KeybindPreset::Helix => 1,
            crate::config::KeybindPreset::Vim => 2,
            crate::config::KeybindPreset::Emacs => 3,
        };
        self.popups.active = Some(crate::popups::ActivePopup::KeybindPreset(
            crate::popups::KeybindPresetPopup { selected },
        ));
    }

    pub fn select_keybind_preset(&mut self) {
        if let Some(crate::popups::ActivePopup::KeybindPreset(popup)) = self.popups.active.take() {
            let new = match popup.selected {
                0 => crate::config::KeybindPreset::Default,
                1 => crate::config::KeybindPreset::Helix,
                2 => crate::config::KeybindPreset::Vim,
                _ => crate::config::KeybindPreset::Emacs,
            };
            self.config.core.keybind_preset = new;
            self.apply_keybind_preset(new);
            self.set_temporary_status(&format!("Keybind preset: {new}"));
            if let Ok(mut c) = crate::config::ClinConfig::load().0 {
                c.core.keybind_preset = new;
                let _ = c.save();
            }
            self.popups.active = Some(crate::popups::ActivePopup::KeybindPreset(popup));
        }
    }

    pub fn close_keybind_preset_popup(&mut self) {
        self.popups.active = None;
    }

    pub fn apply_keybind_preset(&mut self, preset: crate::config::KeybindPreset) {
        let (keybinds, warnings) = self.storage.load_keybinds_with_preset(preset);
        self.keybinds = keybinds;
        for w in warnings {
            self.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        self.seq_matcher.clear();
    }

    pub fn select_theme(&mut self) {
        if let Some(crate::popups::ActivePopup::Theme(mut popup)) = self.popups.active.take() {
            match popup.focus {
                ThemePopupFocus::ThemeList => {
                    let next_theme = popup.themes[popup.selected].clone();
                    self.config.ui.theme = next_theme.clone();
                    self.refresh_theme_from_config();
                    self.set_temporary_status(&format!("Theme set to: {next_theme}"));
                    self.popups.active = Some(crate::popups::ActivePopup::Theme(popup));
                }
                ThemePopupFocus::GeneralBg => {
                    popup.general_is_solid = !popup.general_is_solid;
                    self.config.ui.background = if popup.general_is_solid {
                        crate::config::Background::Solid
                    } else {
                        crate::config::Background::Transparent
                    };
                    self.refresh_theme_from_config();
                    self.popups.active = Some(crate::popups::ActivePopup::Theme(popup));
                }
                ThemePopupFocus::GraphBg => {
                    popup.graph_is_solid = !popup.graph_is_solid;
                    self.config.graf.visual.graph_background = if popup.graph_is_solid {
                        crate::config::Background::Solid
                    } else {
                        crate::config::Background::Transparent
                    };
                    self.popups.active = Some(crate::popups::ActivePopup::Theme(popup));
                }
            }
        }
    }

    pub fn close_theme_popup(&mut self) {
        if let Err(e) = self.config.save() {
            self.set_temporary_status(&format!("Failed to save theme: {e}"));
        }
        self.popups.active = None;
    }

    pub fn begin_set_word_goal(&mut self) {
        let mut input =
            crate::ui::make_popup_textarea(&self.app_theme, "Enter daily word goal (e.g. 500)");
        if self.config.goals.word_goal > 0 {
            input.insert_str(self.config.goals.word_goal.to_string());
        }
        self.popups.active = Some(crate::popups::ActivePopup::Goals(
            crate::popups::GoalsPopup {
                mode: crate::popups::GoalsPopupMode::WordGoal,
                input,
            },
        ));
    }

    pub fn begin_set_note_goal(&mut self) {
        let mut input =
            crate::ui::make_popup_textarea(&self.app_theme, "Enter daily note goal (e.g. 3)");
        if self.config.goals.note_goal > 0 {
            input.insert_str(self.config.goals.note_goal.to_string());
        }
        self.popups.active = Some(crate::popups::ActivePopup::Goals(
            crate::popups::GoalsPopup {
                mode: crate::popups::GoalsPopupMode::NoteGoal,
                input,
            },
        ));
    }

    pub fn confirm_goals_popup(&mut self) {
        if let Some(crate::popups::ActivePopup::Goals(popup)) = self.popups.active.take() {
            let val_str = popup.input.lines().join("");
            match val_str.trim().parse::<usize>() {
                Ok(val) => {
                    let mut config = crate::config::ClinConfig::load().0.unwrap_or_default();
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
                    if let Err(e) = config.save() {
                        self.messages.push(
                            format!("Failed to save config: {e}"),
                            crate::app::messages::MessageSeverity::Warning,
                        );
                    }
                }
                Err(_) => {
                    self.set_temporary_status_static("Please enter a valid positive number.");
                }
            }
        }
    }

    pub fn begin_setup_vault_selection(&mut self) {
        let Some(state) = self.setup_state.as_ref() else {
            return;
        };
        if state.vault_cli_override {
            return;
        }
        let pending = state.vault_path.clone();
        match crate::ui::pick_directory("Select vault directory") {
            Ok(crate::ui::DirectoryPickerOutcome::Selected(path)) => {
                self.select_setup_vault(path);
            }
            Ok(crate::ui::DirectoryPickerOutcome::Cancelled) => {}
            Ok(crate::ui::DirectoryPickerOutcome::Unavailable) => {
                self.open_setup_vault_input(
                    pending,
                    Some("Directory picker unavailable; enter an absolute vault path.".to_string()),
                );
            }
            Err(error) => self
                .open_setup_vault_input(pending, Some(format!("Directory picker failed: {error}"))),
        }
    }

    pub fn open_setup_vault_input(&mut self, path: std::path::PathBuf, notice: Option<String>) {
        if let Some(state) = self.setup_state.as_mut() {
            state.vault_modal = Some(crate::setup::SetupVaultModal::PathInput {
                input: Box::new(ratatui_textarea::TextArea::from([path
                    .display()
                    .to_string()])),
                notice,
            });
            state.vault_error = None;
        }
    }

    pub fn select_setup_vault(&mut self, path: std::path::PathBuf) {
        match crate::setup::vault_requires_confirmation(&path) {
            Ok(true) => {
                if let Some(state) = self.setup_state.as_mut() {
                    state.vault_modal =
                        Some(crate::setup::SetupVaultModal::ConfirmNonEmpty { path });
                }
            }
            Ok(false) => {
                if let Some(state) = self.setup_state.as_mut() {
                    state.vault_path = path;
                    state.confirmed_nonempty_path = None;
                    state.vault_modal = None;
                    state.vault_error = None;
                }
            }
            Err(error) => {
                if let Some(state) = self.setup_state.as_mut() {
                    state.vault_error = Some(error.to_string());
                }
            }
        }
    }

    pub fn apply_setup_live(&mut self) {
        let mut visuals_changed = false;

        {
            let Some(state) = self.setup_state.as_ref() else {
                return;
            };

            // 1. Theme
            let name = state.themes[state.theme].clone();
            if self.config.ui.theme != name {
                self.config.ui.theme = name.to_string();
                visuals_changed = true;
            }

            // 2. Background
            let bg = if state.background_solid {
                crate::config::Background::Solid
            } else {
                crate::config::Background::Transparent
            };
            if self.config.ui.background != bg {
                self.config.ui.background = bg;
                visuals_changed = true;
            }

            // 3. Keybind preset — only rebuild keybinds when the preset actually changed.
            let preset = match state.keybind_preset {
                1 => crate::config::KeybindPreset::Helix,
                2 => crate::config::KeybindPreset::Vim,
                3 => crate::config::KeybindPreset::Emacs,
                _ => crate::config::KeybindPreset::Default,
            };
            if self.config.core.keybind_preset != preset {
                self.config.core.keybind_preset = preset;
                let (kb, warnings) = self.storage.load_keybinds_with_preset(preset);
                self.keybinds = kb;
                for w in warnings {
                    self.messages
                        .push(w, crate::app::messages::MessageSeverity::Warning);
                }
                self.seq_matcher.clear();
                visuals_changed = true;
            }

            // 4. Icon mode
            let im = crate::setup::icon_mode_at(state.icon_mode);
            if self.config.ui.icon_mode != im {
                self.config.ui.icon_mode = im;
                visuals_changed = true;
            }

            // 5. Hint bar style
            let hbs = crate::setup::hint_style_at(state.hint_bar_style);
            if self.config.ui.hint_bar_style != hbs {
                self.config.ui.hint_bar_style = hbs;
                visuals_changed = true;
            }
        }

        // Preview theme/background immediately (in-memory; no disk write).
        if visuals_changed {
            self.refresh_theme_from_config();
        }
    }

    /// Finish setup. A changed vault is initialized and committed for a clean
    /// in-process session rebootstrap; unchanged setup keeps normal flow.
    pub fn finish_setup(&mut self) {
        let previous_config = self.config.clone();
        self.apply_setup_live();
        let (selected_path, changed_vault, confirmed_path) = {
            let Some(state) = self.setup_state.as_ref() else {
                return;
            };
            let selected_path = state.vault_path.clone();
            (
                selected_path.clone(),
                !state.vault_cli_override && selected_path != state.initial_vault_path,
                state.confirmed_nonempty_path.clone(),
            )
        };
        if !changed_vault {
            match self.config.save() {
                Ok(()) => {
                    let _ = self.storage.template_manager().create_examples();
                    self.request_notes_reconcile();
                    self.set_temporary_status_static("Setup complete");
                    self.setup_state = None;
                    self.mode = self
                        .return_mode
                        .take()
                        .unwrap_or(crate::app::ViewMode::List);
                }
                Err(error) => {
                    self.set_temporary_status(&format!("Setup failed to save: {error}"));
                    if let Some(state) = self.setup_state.as_mut() {
                        state.confirm_exit = false;
                    }
                }
            }
            return;
        }
        match crate::setup::vault_requires_confirmation(&selected_path) {
            Ok(true) if confirmed_path.as_ref() != Some(&selected_path) => {
                if let Some(state) = self.setup_state.as_mut() {
                    state.confirm_exit = false;
                    state.vault_modal = Some(crate::setup::SetupVaultModal::ConfirmNonEmpty {
                        path: selected_path,
                    });
                }
                return;
            }
            Ok(_) => {}
            Err(error) => {
                self.set_temporary_status(&format!("Failed to initialize vault: {error}"));
                if let Some(state) = self.setup_state.as_mut() {
                    state.confirm_exit = false;
                }
                return;
            }
        }
        if let Err(error) = std::fs::create_dir_all(&selected_path) {
            self.set_temporary_status(&format!("Failed to initialize vault: {error}"));
            if let Some(state) = self.setup_state.as_mut() {
                state.confirm_exit = false;
            }
            return;
        }
        let mut candidate = self.config.clone();
        candidate.core.storage_path = Some(selected_path.clone());
        let (storage_result, warnings) = crate::storage::Storage::init_with_config(&candidate);
        let storage = match storage_result {
            Ok(storage) => storage,
            Err(error) => {
                self.set_temporary_status(&format!("Failed to initialize vault: {error}"));
                if let Some(state) = self.setup_state.as_mut() {
                    state.confirm_exit = false;
                }
                return;
            }
        };
        if let Err(error) = candidate.save() {
            self.set_temporary_status(&format!("Setup failed to save: {error}"));
            if let Some(state) = self.setup_state.as_mut() {
                state.confirm_exit = false;
            }
            return;
        }
        self.config = candidate;
        self.setup_rebootstrap = Some(crate::setup::SetupRebootstrapRequest {
            storage,
            warnings,
            previous_config,
            previous_path: self.storage.data_dir.clone(),
            selected_path,
        });
        self.should_quit = true;
    }

    /// Discard wizard mutations: reload config + keybinds from disk, close wizard.
    pub fn abort_setup(&mut self) {
        if let Ok(fresh) = crate::config::ClinConfig::load().0 {
            self.config = fresh;
        }
        // Rebuild keybinds for the (now disk-truth) preset; clear any stale
        // in-flight sequence buffered against the old binding set.
        let (kb, warnings) = self
            .storage
            .load_keybinds_with_preset(self.config.core.keybind_preset);
        self.keybinds = kb;
        for w in warnings {
            self.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        self.seq_matcher.clear();
        self.refresh_theme_from_config();
        self.set_temporary_status_static("Setup cancelled");
        self.setup_state = None;
        self.mode = self
            .return_mode
            .take()
            .unwrap_or(crate::app::ViewMode::List);
    }
}
