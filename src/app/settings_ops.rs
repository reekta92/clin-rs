use super::*;
use crate::list_view::*;

impl App {
    /// Load, patch, and save an editor config field.  Replaces the
    /// `ClinConfig::load() → set → save()` boilerplate used by toggle methods.
    fn persist_editor_config(&mut self, field_update: impl FnOnce(&mut crate::config::ClinConfig)) {
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            field_update(&mut config);
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

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
        match self.list.visual_list.get(self.list.visual_index) {
            Some(VisualItem::Note { summary_idx, .. }) => {
                let id = self.notes[*summary_idx].id.clone();
                match self.storage.toggle_pin(&id) {
                    Ok(pinned) => {
                        self.refresh_note_single(None, &id);
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
            }
            Some(VisualItem::Folder { path, .. }) => {
                self.toggle_pin_folder(path.clone());
            }
            Some(VisualItem::SmartFolder { .. })
            | Some(VisualItem::CreateNew { .. })
            | Some(VisualItem::Subnote { .. }) => {
                self.set_temporary_status_static("Cannot pin virtual folders or actions");
            }
            None => {
                self.set_temporary_status_static("Nothing selected");
            }
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

    pub fn toggle_inline_info(&mut self) {
        self.list.inline_info = !self.list.inline_info;
        let msg: &'static str = if self.list.inline_info {
            "Inline info shown"
        } else {
            "Inline info hidden"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.inline_info = self.list.inline_info;
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
        self.set_temporary_status_static(
            if matches!(self.preview_position, crate::config::PreviewPosition::Left) {
                "Preview moved to left"
            } else {
                "Preview moved to right"
            },
        );
        self.persist_list_layout();
    }

    pub fn swap_calendar_position(&mut self) {
        self.calendar_position = match self.calendar_position {
            crate::config::CalendarPosition::Top => crate::config::CalendarPosition::Bottom,
            crate::config::CalendarPosition::Bottom => crate::config::CalendarPosition::Top,
        };
        self.set_temporary_status_static(
            if matches!(self.calendar_position, crate::config::CalendarPosition::Top) {
                "Calendar moved to top"
            } else {
                "Calendar moved to bottom"
            },
        );
        self.persist_list_layout();
    }

    pub fn active_strip_sections(&self) -> Vec<crate::config::NotesSection> {
        let mut v: Vec<_> = self
            .list
            .sections
            .iter()
            .copied()
            .filter(|s| {
                !matches!(s, crate::config::NotesSection::Goals) || self.config.goals.enabled
            })
            .collect();
        if v.is_empty() {
            v.push(crate::config::NotesSection::Calendar);
        }
        v
    }

    pub fn active_strip_sections_for(&self, width: u16) -> Vec<crate::config::NotesSection> {
        let mut v = self.active_strip_sections();
        if width < 42 {
            v.retain(|s| !matches!(s, crate::config::NotesSection::Goals));
        }
        if v.is_empty() {
            v.push(crate::config::NotesSection::Calendar);
        }
        v
    }

    pub fn swap_section_order(&mut self) {
        if self.list.sections.len() >= 2 {
            self.list.sections.reverse();
            self.set_temporary_status_static("Sections swapped");
            self.persist_list_layout();
        }
    }

    pub fn cycle_section(&mut self, slot: usize) {
        if slot >= self.list.sections.len() {
            return;
        }
        self.list.sections[slot] = match self.list.sections[slot] {
            crate::config::NotesSection::Calendar => crate::config::NotesSection::Goals,
            crate::config::NotesSection::Goals => crate::config::NotesSection::Draw,
            crate::config::NotesSection::Draw => crate::config::NotesSection::Graf,
            crate::config::NotesSection::Graf => crate::config::NotesSection::Calendar,
        };
        self.set_temporary_status_static("Section changed");
        self.persist_list_layout();
    }

    pub fn toggle_section(&mut self) {
        if self.list.sections.len() < 2 {
            self.list.sections.push(crate::config::NotesSection::Goals);
            self.set_temporary_status_static("Section added");
        } else {
            self.list.sections.truncate(1);
            self.set_temporary_status_static("Section removed");
        }
        self.persist_list_layout();
    }

    pub(crate) fn persist_list_layout(&mut self) {
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.preview_width_ratio = self.list.preview_width_ratio;
            config.list.calendar_height = self.list.calendar_height;
            config.list.preview_position = self.preview_position;
            config.list.calendar_position = self.calendar_position;
            config.list.sections = self.list.sections.clone();
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save layout: {e}"));
            }
        }
    }

    pub(crate) fn persist_folder_state(&mut self) {
        // Save pinned_folders to config.toml (preference)
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.pinned_folders = self.list.pinned_folders.iter().cloned().collect();
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save folder state: {e}"));
            }
        }

        // Save expanded_folders to state.json per vault (session state)
        let vault_id = crate::local_state::vault_identity_path(&self.storage.data_dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| self.storage.data_dir.to_string_lossy().into_owned());
        let expanded: std::collections::BTreeSet<String> =
            self.list.folder_expanded.iter().cloned().collect();
        if let Ok(paths) = crate::paths::AppPaths::discover(
            crate::config::ClinConfig::config_path().unwrap_or_default(),
        ) {
            let state_path = paths.state_path();
            let _ = crate::local_state::LocalState::update(&state_path, |s| {
                let vault = s.vaults.entry(vault_id.clone()).or_default();
                vault.expanded_folders = expanded.clone();
                Ok(())
            });
        }
    }

    pub fn toggle_markdown_preview(&mut self) {
        self.editor.editor_preview_enabled = !self.editor.editor_preview_enabled;
        if self.editor.editor_preview_enabled {
            self.editor.sidebar = EditSidebar::None;
            self.update_editor_markdown_preview();
            self.set_temporary_status_static("Markdown preview enabled");
        } else {
            self.editor.md_preview_renderer = None;
            self.set_temporary_status_static("Markdown preview disabled");
        }
        let val = self.editor.editor_preview_enabled;
        self.persist_editor_config(|c| c.editor.preview_enabled = val);
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
        self.config.core.preview_wrap = self.preview_wrap;
        let val = self.preview_wrap;
        self.persist_editor_config(|c| c.core.preview_wrap = val);
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

    pub fn toggle_editor_soft_wrap(&mut self) {
        self.config.editor.soft_wrap = !self.config.editor.soft_wrap;
        let mode = if self.config.editor.soft_wrap {
            ratatui_textarea::WrapMode::WordOrGlyph
        } else {
            ratatui_textarea::WrapMode::None
        };
        self.editor.editor.set_wrap_mode(mode);
        self.editor.title_editor.set_wrap_mode(mode);
        let val = self.config.editor.soft_wrap;
        self.persist_editor_config(|c| c.editor.soft_wrap = val);
        self.set_temporary_status_static(if self.config.editor.soft_wrap {
            "Editor wrap on"
        } else {
            "Editor wrap off"
        });
    }

    pub fn apply_editor_prefs(&mut self) {
        let mode = if self.config.editor.soft_wrap {
            ratatui_textarea::WrapMode::WordOrGlyph
        } else {
            ratatui_textarea::WrapMode::None
        };
        self.editor.editor.set_wrap_mode(mode);
        self.editor.title_editor.set_wrap_mode(mode);
    }

    pub fn toggle_show_line_numbers(&mut self) {
        self.editor.show_line_numbers = !self.editor.show_line_numbers;
        let msg: &'static str = if self.editor.show_line_numbers {
            "Line numbers enabled"
        } else {
            "Line numbers disabled"
        };
        self.set_temporary_status_static(msg);
        let val = self.editor.show_line_numbers;
        self.persist_editor_config(|c| c.editor.show_line_numbers = val);
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
        self.sort_notes();
        self.refresh_visual_list();
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
        self.request_notes_reconcile();
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

    pub fn toggle_show_all_files(&mut self) {
        self.list.show_all_files = !self.list.show_all_files;
        self.request_notes_reconcile();
        let msg: &'static str = if self.list.show_all_files {
            "Showing all files"
        } else {
            "Showing notes only"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.show_all_files = self.list.show_all_files;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_folders_first(&mut self) {
        self.list.folders_first = !self.list.folders_first;
        self.refresh_visual_list();
        let msg: &'static str = if self.list.folders_first {
            "Folders first in list"
        } else {
            "Files first in list"
        };
        self.set_temporary_status_static(msg);
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.folders_first = self.list.folders_first;
            if let Err(e) = config.save() {
                self.set_temporary_status(&format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn toggle_smart_folders(&mut self) {
        self.config.list.smart_folders_enabled = !self.config.list.smart_folders_enabled;
        self.refresh_visual_list();
        self.set_temporary_status_static(if self.config.list.smart_folders_enabled {
            "Smart folders enabled"
        } else {
            "Smart folders disabled"
        });
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.list.smart_folders_enabled = self.config.list.smart_folders_enabled;
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
    fn local_state_path(&self) -> anyhow::Result<std::path::PathBuf> {
        #[cfg(test)]
        {
            // Hand-built Storage fixtures intentionally do not resolve XDG
            // state; keep their state in their temporary config root.
            Ok(self.storage.config_dir.join("state.json"))
        }
        #[cfg(not(test))]
        {
            let config_path = crate::config::ClinConfig::config_path()?;
            Ok(crate::paths::AppPaths::discover(config_path)?.state_path())
        }
    }

    pub fn load_goals_progress(&self) -> crate::goals::DailyProgress {
        let today = chrono::Local::now().date_naive().to_string();
        let fresh = || crate::goals::DailyProgress {
            date: today.clone(),
            words_written: 0,
            notes_modified: std::collections::BTreeSet::new(),
        };

        let Ok(state_path) = self.local_state_path() else {
            return fresh();
        };
        let Ok(state) = crate::local_state::LocalState::load(&state_path) else {
            return fresh();
        };
        if state.goals.date == today {
            state.goals
        } else {
            fresh()
        }
    }

    pub fn save_goals_progress(
        &self,
        progress: &crate::goals::DailyProgress,
    ) -> anyhow::Result<()> {
        let state_path = self.local_state_path()?;
        let progress = progress.clone();
        crate::local_state::LocalState::update(&state_path, |state| {
            state.goals = progress;
            Ok(())
        })?;
        Ok(())
    }


    pub fn get_current_goals_progress(&mut self) -> &mut crate::goals::DailyProgress {
        self.check_and_reload_config();
        let today = chrono::Local::now().date_naive().to_string();
        if self.goals_progress.date != today {
            self.goals_progress = self.load_goals_progress();
        }
        &mut self.goals_progress
    }

    pub fn ensure_draw_preview(&mut self) {
        let target = self
            .get_selected_note_id()
            .filter(|id| id.ends_with(".draw"))
            .or_else(|| {
                self.notes
                    .iter()
                    .filter(|n| n.id.ends_with(".draw"))
                    .max_by_key(|n| n.updated_at)
                    .map(|n| n.id.clone())
            });
        let target = match target {
            Some(id) => id,
            None => {
                if !matches!(
                    self.draw_preview.as_ref().map(|(id, _)| id.as_str()),
                    Some("")
                ) {
                    self.draw_preview =
                        Some((String::new(), crate::draw::state::DrawData::default()));
                }
                return;
            }
        };
        if self
            .draw_preview
            .as_ref()
            .map(|(id, _)| id == &target)
            .unwrap_or(false)
        {
            return;
        }
        let data = std::fs::read_to_string(self.storage.note_path(&target))
            .ok()
            .and_then(|s| serde_json::from_str::<crate::draw::state::DrawData>(&s).ok())
            .unwrap_or_default();
        self.draw_preview = Some((target, data));
    }

    pub fn ensure_graph_preview(&mut self) {
        let sig = self.notes.len();
        if self.graph_preview.is_some() && self.graph_preview_sig == sig {
            return;
        }
        match crate::graf::graph::GraphState::new(&self.notes, &self.config) {
            Ok(mut gs) => {
                gs.viewport = gs
                    .viewport
                    .auto_fit_from_graph(gs.simulation.get_graph(), 1.4);
                gs.graph_bounds =
                    crate::graf::render::compute_graph_bounds(gs.simulation.get_graph());
                self.graph_preview = Some(gs);
                self.graph_preview_sig = sig;
                self.graph_preview_steps = 0;
            }
            Err(_) => {
                self.graph_preview = None;
                self.graph_preview_sig = sig;
                self.graph_preview_steps = 0;
            }
        }
    }

    /// Scroll the body editor and keep the cached viewport offset in sync.
    /// Mirrors `ratatui_textarea::Viewport::scroll` (widget.rs:68) so the
    /// mouse-to-cursor cache (`body_viewport_row/col`) matches the real viewport
    /// after explicit scrolls (mouse wheel, pinstar).
    pub fn scroll_editor(&mut self, rows: i16, cols: i16) {
        self.editor.editor.scroll((rows, cols));
        fn apply(pos: u16, d: i16) -> u16 {
            if d >= 0 {
                pos.saturating_add(d as u16)
            } else {
                pos.saturating_sub((-d) as u16)
            }
        }
        self.editor.body_viewport_row = apply(self.editor.body_viewport_row, rows);
        self.editor.body_viewport_col = apply(self.editor.body_viewport_col, cols);
    }

    pub fn refresh_read_mode(&mut self) {
        let content = self.editor.editor.lines().join("\n");
        let cols = self.editor.last_body_width;
        if cols == 0 {
            // No render yet; keep dirty so first render triggers refresh.
            self.editor.read_dirty = true;
            return;
        }
        let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
        let lines = crate::markdown::render_builtin_sync(&content, cols, &self.app_theme, &opts);
        let mut grid = Vec::with_capacity(lines.len());
        let mut src = Vec::with_capacity(lines.len());
        for l in lines {
            grid.push(l.cells);
            src.push(l.source_line);
        }
        self.editor.read_grid = grid;
        self.editor.read_row_source = src;
        self.editor.read_cols = cols;
        self.editor.read_dirty = false;
        // EDIT→READ: place read_offset at the grid row for the source line being edited.
        if let Some(edited_line) = self.editor.pending_read_sync_from_line.take() {
            let target_src = edited_line + 1; // 0-based logical → 1-based source line
            let row_source = &self.editor.read_row_source;
            let g = row_source
                .iter()
                .position(|&s| s == target_src)
                .unwrap_or_else(|| {
                    row_source
                        .iter()
                        .rposition(|&s| s != 0 && s <= target_src)
                        .unwrap_or(0)
                });
            self.editor.read_offset = g.saturating_sub(1); // 1 row of context above
        }
        let max = self.editor.read_grid.len().saturating_sub(1);
        self.editor.read_offset = self.editor.read_offset.min(max);
    }

    pub fn activate_edit_mode(&mut self) {
        // READ→EDIT: jump the cursor to the source line of the top visible READ row.
        let src_line = self
            .editor
            .read_row_source
            .get(self.editor.read_offset)
            .copied()
            .unwrap_or(1);
        let logical = src_line.saturating_sub(1);
        self.editor.read_selecting = false;
        self.editor.read_sel_anchor = None;
        self.editor.read_sel_end = None;
        self.editor.edit_mode = crate::editor::EditMode::Edit;
        let max_line = self.editor.editor.lines().len().saturating_sub(1);
        let target = logical.min(max_line);
        self.editor
            .editor
            .move_cursor(ratatui_textarea::CursorMove::Jump(target as u16, 0));
        let delta = target as i16 - self.editor.body_viewport_row as i16;
        if delta != 0 {
            self.scroll_editor(delta, 0);
        }
        self.set_temporary_status_static("EDIT");
    }

    pub fn back_to_read_mode(&mut self) {
        self.editor.read_selecting = false;
        self.editor.read_sel_anchor = None;
        self.editor.read_sel_end = None;
        self.editor.read_dirty = true;
        self.editor.pending_read_sync_from_line = Some(self.editor.editor.cursor().0);
        self.editor.edit_mode = crate::editor::EditMode::Read;
        self.set_temporary_status_static("READ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app() -> App {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&templates_dir).unwrap();

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        App::new(storage).unwrap()
    }

    #[test]
    fn test_swap_section_order_reverses() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        app.list.sections = vec![
            crate::config::NotesSection::Calendar,
            crate::config::NotesSection::Draw,
        ];
        app.swap_section_order();
        assert_eq!(app.list.sections[0], crate::config::NotesSection::Draw);
        assert_eq!(app.list.sections[1], crate::config::NotesSection::Calendar);
    }

    #[test]
    fn test_swap_section_order_noop_on_single() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        app.list.sections = vec![crate::config::NotesSection::Calendar];
        app.swap_section_order();
        assert_eq!(app.list.sections.len(), 1);
        assert_eq!(app.list.sections[0], crate::config::NotesSection::Calendar);
    }

    #[test]
    fn test_cycle_section_calendar_to_goals() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        app.list.sections = vec![crate::config::NotesSection::Calendar];
        app.cycle_section(0);
        assert_eq!(app.list.sections[0], crate::config::NotesSection::Goals);
    }

    #[test]
    fn test_cycle_section_graf_wraps_to_calendar() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        app.list.sections = vec![crate::config::NotesSection::Graf];
        app.cycle_section(0);
        assert_eq!(app.list.sections[0], crate::config::NotesSection::Calendar);
    }

    #[test]
    fn test_cycle_section_out_of_range_noop() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        app.list.sections = vec![crate::config::NotesSection::Calendar];
        app.cycle_section(5); // out of range
        assert_eq!(app.list.sections.len(), 1);
        assert_eq!(app.list.sections[0], crate::config::NotesSection::Calendar);
    }

    #[test]
    fn apply_setup_live_writes_5_fields() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        let config_file_path = app.storage.config_dir.join("config.toml");
        crate::config::set_config_path_override(config_file_path);

        app.setup_state = Some(crate::setup::SetupState {
            theme: 4, // gruvbox
            background_solid: true,
            hint_bar_style: 1, // Sharp
            icon_mode: 1,      // Unicode
            keybind_preset: 2, // Vim
            selected: 0,
            confirm_exit: false,
        });

        app.finish_setup();

        assert_eq!(app.config.ui.theme, "gruvbox");
        assert_eq!(app.config.ui.background, crate::config::Background::Solid);
        assert_eq!(
            app.config.ui.hint_bar_style,
            crate::config::HintBarStyle::Sharp
        );
        assert_eq!(app.config.ui.icon_mode, crate::config::IconMode::Unicode);
        assert_eq!(
            app.config.core.keybind_preset,
            crate::config::KeybindPreset::Vim
        );
        // finish_setup tears down the view.
        assert!(app.setup_state.is_none());
        assert_eq!(app.mode, crate::app::ViewMode::List);
    }

    #[test]
    fn setup_cycle_live_applies_theme() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();

        app.setup_state = Some(crate::setup::SetupState {
            theme: 0,
            background_solid: false,
            hint_bar_style: 0,
            icon_mode: 0,
            keybind_preset: 0,
            selected: 0, // Theme row
            confirm_exit: false,
        });

        // Cycle theme forward → apply_setup_live writes it to config.
        app.setup_state.as_mut().unwrap().cycle(true);
        app.apply_setup_live();
        assert_eq!(app.config.ui.theme, "tokyo_night");

        // Flip background via row 1 → config mirrors it.
        let state = app.setup_state.as_mut().unwrap();
        state.selected = 1;
        state.cycle(true);
        app.apply_setup_live();
        assert_eq!(app.config.ui.background, crate::config::Background::Solid);
    }

    #[test]
    fn setup_esc_opens_confirm_then_y_finishes() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        let config_file_path = app.storage.config_dir.join("config.toml");
        crate::config::set_config_path_override(config_file_path);

        app.setup_state = Some(crate::setup::SetupState {
            theme: 4, // gruvbox — non-default so we can confirm it propagates
            background_solid: true,
            hint_bar_style: 0,
            icon_mode: 0,
            keybind_preset: 0,
            selected: 0,
            confirm_exit: false,
        });

        let esc = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        crate::events::handle_setup_keys(&mut app, esc);
        assert!(app.setup_state.as_ref().unwrap().confirm_exit);

        // 'n' cancels the confirm overlay.
        let n = KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        crate::events::handle_setup_keys(&mut app, n);
        assert!(!app.setup_state.as_ref().unwrap().confirm_exit);

        // Re-open and confirm with 'y' → finish_setup saves + closes.
        app.setup_state.as_mut().unwrap().confirm_exit = true;
        let y = KeyEvent {
            code: KeyCode::Char('y'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        crate::events::handle_setup_keys(&mut app, y);
        assert!(app.setup_state.is_none());
        assert_eq!(app.mode, crate::app::ViewMode::List);
        assert_eq!(app.config.ui.theme, "gruvbox");
    }

    #[test]
    fn test_toggle_smart_folders() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        let config_file_path = app.storage.config_dir.join("config.toml");
        crate::config::set_config_path_override(config_file_path);

        app.config.list.smart_folders_enabled = false;
        app.toggle_smart_folders();
        assert!(app.config.list.smart_folders_enabled);
        app.toggle_smart_folders();
        assert!(!app.config.list.smart_folders_enabled);
    }

    #[test]
    fn test_custom_smart_folders_rule_matching() {
        let mut app = make_app();
        app.config.list.smart_folders_enabled = true;
        app.list.grid_folder = crate::app::VIRTUAL_SMART_PATH.to_string();

        // Define custom rules
        app.config.list.custom_smart_folders = vec![crate::config::structs::CustomSmartFolder {
            name: "Work Projects".to_string(),
            tags: vec!["work".to_string()],
            title_contains: Some("project".to_string()),
            folder_prefix: Some("work/".to_string()),
            updated_within_days: Some(7),
        }];

        let now = crate::ui::now_unix_secs();

        // Create mock notes
        app.notes = vec![
            // Matches all criteria
            crate::storage::NoteSummary {
                id: "1.md".to_string(),
                title: "My work project".to_string(),
                updated_at: now - 3600, // 1 hour ago
                tags: vec!["work".to_string(), "active".to_string()],
                folder: "work/active".to_string(),
                pinned: false,
                links: Vec::new(),
                size_bytes: 0,
            },
            // Fails folder_prefix
            crate::storage::NoteSummary {
                id: "2.md".to_string(),
                title: "My work project".to_string(),
                updated_at: now - 3600,
                tags: vec!["work".to_string()],
                folder: "personal/".to_string(),
                pinned: false,
                links: Vec::new(),
                size_bytes: 0,
            },
            // Fails title_contains
            crate::storage::NoteSummary {
                id: "3.md".to_string(),
                title: "My work tasks".to_string(),
                updated_at: now - 3600,
                tags: vec!["work".to_string()],
                folder: "work/active".to_string(),
                pinned: false,
                links: Vec::new(),
                size_bytes: 0,
            },
            // Fails tags
            crate::storage::NoteSummary {
                id: "4.md".to_string(),
                title: "My work project".to_string(),
                updated_at: now - 3600,
                tags: vec![],
                folder: "work/active".to_string(),
                pinned: false,
                links: Vec::new(),
                size_bytes: 0,
            },
            // Fails updated_within_days (8 days ago)
            crate::storage::NoteSummary {
                id: "5.md".to_string(),
                title: "My work project".to_string(),
                updated_at: now - 8 * 86400,
                tags: vec!["work".to_string()],
                folder: "work/active".to_string(),
                pinned: false,
                links: Vec::new(),
                size_bytes: 0,
            },
        ];

        app.refresh_visual_list();

        // Find the SmartFolder in visual_list
        let smart_folder = app.list.visual_list.iter().find(|item| {
            matches!(item, VisualItem::SmartFolder { label, .. } if label == "Work Projects")
        });

        assert!(
            smart_folder.is_some(),
            "Custom smart folder should be present"
        );
        if let Some(VisualItem::SmartFolder { note_count, .. }) = smart_folder {
            assert_eq!(*note_count, 1, "Only one note should match all criteria");
        }
    }
}
