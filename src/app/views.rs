use super::*;
use crate::list_view::*;
use crate::popups::*;
use std::borrow::Cow;

impl App {
    fn palette_source(&self) -> ViewMode {
        if matches!(self.mode, ViewMode::Help | ViewMode::Setup) {
            self.return_mode.unwrap_or(ViewMode::List)
        } else {
            self.mode
        }
    }

    fn guard_palette_departure(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.base_embeds.has_writes()
                && !self
                    .bases_state
                    .as_ref()
                    .is_some_and(|state| state.is_writing()),
            "Write in progress; wait before leaving Bases"
        );
        Ok(())
    }

    /// Save owners without committing their return routes or dropping workers.
    pub(crate) fn save_before_session_replace(&mut self) -> anyhow::Result<()> {
        self.guard_palette_departure()?;
        let source = self.palette_source();
        let host = if source == ViewMode::Bases {
            self.bases_state
                .as_ref()
                .map_or(source, |s| s.previous_mode)
        } else {
            source
        };
        if source == ViewMode::Bases {
            anyhow::ensure!(
                self.bases_state
                    .as_ref()
                    .is_some_and(|s| s.palette_input_available()),
                "Finish the Bases interaction before leaving"
            );
        }
        match host {
            ViewMode::Draw => {
                let state = self
                    .draw_state
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Draw view unavailable"))?;
                state.save_for_navigation()?;
            }
            ViewMode::Canvas => self
                .canvas_state
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Canvas view unavailable"))?
                .save_for_navigation()?,
            ViewMode::Graph => {
                self.config.save()?;
            }
            _ => {}
        }
        let editor_live = host == ViewMode::Edit
            || (matches!(host, ViewMode::Graph | ViewMode::Outline)
                && (self.return_mode == Some(ViewMode::Edit)
                    || self.visit_return_mode == Some(ViewMode::Edit)));
        if editor_live
            && (self.editor.editing_id.is_some() || self.editor.template_edit_path.is_some())
        {
            self.save_editor_before_navigation()?;
        }
        if let Some(editor) = self.embedded_host_editor.take() {
            let active = std::mem::replace(&mut self.editor, *editor);
            let result = self.save_editor_before_navigation();
            self.embedded_host_editor = Some(Box::new(std::mem::replace(&mut self.editor, active)));
            result?;
        }
        Ok(())
    }

    pub(crate) fn save_editor_before_navigation(&mut self) -> anyhow::Result<()> {
        if let Err(error) = self.autosave() {
            self.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
            self.editor.autosave_timer = None;
            self.write_draft();
            anyhow::bail!(error);
        }
        Ok(())
    }

    pub(crate) fn navigate_from_palette(&mut self, destination: &str) -> anyhow::Result<()> {
        use anyhow::ensure;
        match destination {
            "setup.finish" => {
                ensure!(self.mode == ViewMode::Setup, "Setup is not active");
                self.finish_setup();
                return Ok(());
            }
            "setup.cancel" => {
                ensure!(self.mode == ViewMode::Setup, "Setup is not active");
                if let Some(state) = &mut self.setup_state {
                    state.confirm_exit = true;
                }
                return Ok(());
            }
            _ => ensure!(self.mode != ViewMode::Setup, "Finish or cancel Setup first"),
        }
        self.guard_palette_departure()?;
        if destination == "help.open" {
            ensure!(self.config.features.help_view.is_enabled(), "Help disabled");
            if self.mode == ViewMode::Calendar {
                if let Some(view) = &mut self.calendar_view {
                    view.dialog = Some(crate::calendar::view::Dialog::Help);
                }
                return Ok(());
            }
            self.open_help_page_with_tab(self.mode.help_tab().unwrap_or(HelpTab::Notes));
            return Ok(());
        }
        if destination == "setup_wizard" {
            self.open_setup_view();
            return Ok(());
        }
        ensure!(
            matches!(destination, "notes.open" | "graph.open" | "calendar.open"),
            "Unknown destination"
        );
        ensure!(
            destination != "graph.open" || self.config.features.graph_view.is_enabled(),
            "Graph disabled"
        );
        self.save_before_session_replace()?;
        #[cfg(test)]
        ensure!(
            !std::mem::take(&mut self.palette_destination_failure),
            "Destination preparation failed"
        );
        let graph = if destination == "graph.open" && self.graph_plugin.is_none() {
            Some(crate::graf_adapter::GrafPlugin::new(
                &self.config,
                self.storage.clone(),
                self.notes.clone(),
                self.config_errors.clone(),
                self.keybinds.clone(),
                crate::keybinds::KeyMatcher::default(),
            )?)
        } else {
            None
        };
        let retained_graph = if destination == "graph.open" {
            self.graph_plugin.take()
        } else {
            None
        };
        if destination == "calendar.open" {
            self.calendar.ensure_loaded(&self.storage)?;
        }
        self.release_navigation_owners();
        if destination == "graph.open" {
            self.graph_plugin = graph.or(retained_graph);
            self.mode = ViewMode::Graph;
            self.return_mode = Some(ViewMode::List);
        }
        if destination == "calendar.open" {
            if self.calendar_view.is_none() {
                self.calendar_view = Some(crate::calendar::view::View::new(
                    chrono::Local::now().date_naive(),
                ));
            }
            self.mode = ViewMode::Calendar;
            self.return_mode = Some(ViewMode::List);
        }
        self.sync_retained_view_settings();
        Ok(())
    }

    /// Commit departure only after saving and preparing the destination.
    pub(crate) fn release_navigation_owners(&mut self) {
        if self.mode == ViewMode::Help {
            self.close_help_page();
        }
        if self.mode == ViewMode::Bases {
            crate::finish_overlay_event(
                self,
                crate::overlay::ViewKind::Bases,
                crate::overlay::OverlayResult::Exit,
            );
        }
        self.graph_plugin = None;
        self.draw_state = None;
        self.canvas_state = None;
        self.outline_state = None;
        self.bases_state = None;
        self.embedded_host_editor = None;
        if let Some(worker) = &mut self.bases_worker {
            worker.cancel();
        }
        self.base_embeds = crate::bases::embedding::Embeds::default();
        self.return_mode = None;
        self.visit_return_mode = None;
        self.back_to_list(None, None);
    }
    pub fn open_help_page(&mut self) {
        self.open_help_page_with_tab(HelpTab::Notes);
    }

    pub fn open_help_page_with_tab(&mut self, tab: HelpTab) {
        if self.feature_disabled(
            self.config.features.help_view.is_enabled(),
            "Help",
            "help_view",
        ) {
            return;
        }
        if self.mode != ViewMode::Help {
            self.visit_return_mode = self.return_mode;
            self.return_mode = Some(self.mode);
        }
        self.mode = ViewMode::Help;
        self.help_tab = tab;
        self.help_page = 0;
        self.help_info_active = 0;
        self.help_side_scroll = [0; 2];
        self.help_tab_page.insert(tab, 0);
        self.status = Cow::Borrowed("");
        self.status_until = None;
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.reroll_help_suggestions();
    }

    pub fn close_help_page(&mut self) {
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        self.return_mode = self.visit_return_mode.take();
        self.help_page = 0;
        self.help_tab_page.clear();
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.set_default_status();
    }

    pub fn open_setup_view(&mut self) {
        if self.mode == ViewMode::Help {
            self.close_help_page();
        }
        if self.mode != ViewMode::Setup {
            self.visit_return_mode = self.return_mode;
            self.return_mode = Some(self.mode);
        }
        self.mode = ViewMode::Setup;
        let vault_path = self
            .config
            .effective_storage_path()
            .unwrap_or_else(|_| self.storage.data_dir.clone());
        self.setup_state = Some(crate::setup::SetupState::from_config(
            &self.config,
            &self.app_theme,
            vault_path,
            crate::config::has_storage_path_override(),
        ));
        self.status = Cow::Borrowed("");
        self.status_until = None;
    }

    pub fn open_bases_view(&mut self, id: &str, named_view: Option<String>) {
        if self.feature_disabled(
            self.config.features.bases_view.is_enabled(),
            "Bases",
            "bases_view",
        ) {
            return;
        }
        if !self.ensure_bases_worker() {
            return;
        }
        let mut state = crate::bases::state::BasesState::new(id.to_owned(), named_view);
        state.previous_mode = if self.mode == ViewMode::Bases {
            self.bases_state
                .as_ref()
                .map_or(ViewMode::List, |current| current.previous_mode)
        } else {
            self.mode
        };
        state.refresh(self, false);
        self.bases_state = Some(state);
        if self.mode != ViewMode::Bases {
            self.return_mode = Some(self.mode);
        }
        self.mode = ViewMode::Bases;
        self.set_default_status();
    }

    pub(crate) fn ensure_bases_worker(&mut self) -> bool {
        if self.bases_worker.is_none() {
            match crate::bases::worker::BasesWorker::new(self.storage.clone()) {
                Ok(worker) => self.bases_worker = Some(worker),
                Err(error) => {
                    self.set_temporary_status(&format!("Failed to start Bases worker: {error}"));
                    return false;
                }
            }
        }
        true
    }
    pub fn begin_create_base(&mut self) {
        let folder = if self.mode == ViewMode::Bases {
            let Some(state) = &self.bases_state else {
                return;
            };
            std::path::Path::new(&state.id)
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy()
                .into_owned()
        } else if self.mode == ViewMode::List && self.list.notes_layout.browses_folders() {
            self.list.grid_folder.clone()
        } else if self.mode == ViewMode::List {
            self.get_current_folder_context()
        } else {
            return;
        };
        self.begin_create_base_in_folder(folder);
    }
    pub fn begin_create_base_in_folder(&mut self, folder: String) {
        if self.feature_disabled(
            self.config.features.bases_view.is_enabled(),
            "Bases",
            "bases_view",
        ) {
            return;
        }
        let folder = if Self::is_virtual_path(&folder) {
            String::new()
        } else {
            folder
        };
        let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Bases File Name - Esc to cancel, Enter to create"),
        );
        self.present_action_popup(crate::popups::ActivePopup::CreateNote(
            crate::popups::NoteCreatePopup { folder, input },
            crate::popups::NoteFormat::Base,
        ));
    }
    pub(crate) fn create_base(&mut self, folder: String, title: String) {
        if self.feature_disabled(
            self.config.features.bases_view.is_enabled(),
            "Bases",
            "bases_view",
        ) || !self.ensure_bases_worker()
        {
            return;
        }
        let name = if title.is_empty() {
            "Untitled.base".to_owned()
        } else if title.ends_with(".base") {
            title
        } else {
            format!("{title}.base")
        };
        let id = if folder.is_empty() {
            name
        } else {
            format!("{folder}/{name}")
        };
        if let Err(error) = self.storage.document_path(&id) {
            self.set_temporary_status(&format!("Cannot create Bases document: {error:#}"));
            return;
        }
        let previous_mode = if self.mode == ViewMode::Bases {
            self.bases_state
                .as_ref()
                .map_or(ViewMode::List, |state| state.previous_mode)
        } else {
            self.mode
        };
        if let Some(worker) = &mut self.bases_worker {
            worker.cancel();
        }
        let state = crate::bases::state::BasesState::for_creation(
            id,
            previous_mode,
            self.storage.clone(),
            &self.notes_worker_pool,
        );
        self.bases_state = Some(state);
        self.mode = ViewMode::Bases;
        self.set_default_status();
    }
    pub(crate) fn invalidate_bases(&mut self) {
        self.base_embeds.invalidate();
        if let Some(worker) = &mut self.bases_worker {
            worker.invalidate();
        }
        self.bases_refresh_requested = true;
    }

    pub fn poll_bases(&mut self) -> bool {
        let creation_dirty = self.poll_palette_base_creation();
        let mut dirty = false;
        if let Some(canvas) = &mut self.canvas_state {
            match canvas.poll_base_renderers() {
                Ok(changed) => dirty |= changed,
                Err(error) => {
                    self.set_temporary_status(&format!("Canvas host reload failed: {error:#}"))
                }
            }
        }
        if self.bases_refresh_requested
            && self.mode == ViewMode::Bases
            && let Some(mut state) = self.bases_state.take()
        {
            state.refresh(self, false);
            self.bases_state = Some(state);
            dirty = true;
        }
        if let Some((key, loaded)) = self
            .bases_worker
            .as_mut()
            .and_then(|worker| worker.poll_completed())
        {
            if let Some(key) = key
                && !self.bases_state.as_ref().is_some_and(|state| {
                    state
                        .embedded_request
                        .as_ref()
                        .is_some_and(|request| request.key == key)
                })
            {
                self.base_embeds.install(key, loaded);
            } else if let Some(state) = &mut self.bases_state {
                if let Ok(loaded) = &loaded {
                    self.base_embeds.observe_revision(loaded.public_revision);
                }
                state.install(loaded);
            }
            dirty = true;
        }
        if let Some(mut state) = self.bases_state.take() {
            dirty |= state.poll_media(self);
            dirty |= state.poll_write(self);
            self.bases_state = Some(state);
        }
        creation_dirty | dirty | self.poll_base_embeds()
    }

    pub fn open_graph_view(&mut self) {
        if self.feature_disabled(
            self.config.features.graph_view.is_enabled(),
            "Graph view",
            "graph_view",
        ) {
            return;
        }
        if self.graph_plugin.is_none() {
            match crate::graf_adapter::GrafPlugin::new(
                &self.config,
                self.storage.clone(),
                self.notes.clone(),
                self.config_errors.clone(),
                self.keybinds.clone(),
                self.seq_matcher.clone(),
            ) {
                Ok(state) => {
                    self.graph_plugin = Some(state);
                }
                Err(_) => {
                    self.set_temporary_status_static("Failed to build graph view");
                    self.messages.push(
                        "Failed to build graph view".to_string(),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                    return;
                }
            }
        }
        if self.mode != ViewMode::Graph {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Graph;
        }
    }
    pub fn open_outline_view(&mut self) {
        if self.feature_disabled(
            self.config.features.outline_view.is_enabled(),
            "Outline view",
            "outline_view",
        ) {
            return;
        }
        let note_id = self.get_selected_note_id();
        self.outline_state = if let Some(id) = note_id {
            match self.storage.load_note(&id) {
                Ok(note) => {
                    let state = crate::outline::state::OutlineState::new(
                        id.clone(),
                        &note.title,
                        &note.content,
                        self.keybinds.clone(),
                        self.seq_matcher.clone(),
                    );

                    Some(state)
                }
                Err(_) => Some(crate::outline::state::OutlineState::error(
                    id,
                    self.keybinds.clone(),
                    self.seq_matcher.clone(),
                )),
            }
        } else {
            Some(crate::outline::state::OutlineState::error(
                String::new(),
                self.keybinds.clone(),
                self.seq_matcher.clone(),
            ))
        };
        if self.mode != ViewMode::Outline {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Outline;
        }
    }

    pub fn open_draw_view(&mut self) {
        if self.feature_disabled(
            self.config.features.draw_view.is_enabled(),
            "Draw view",
            "draw_view",
        ) {
            return;
        }
        if let Some(id) = self
            .get_selected_note_id()
            .filter(|id| id.ends_with(".draw"))
        {
            if let Err(error) = self.open_draw_document(&id) {
                self.set_temporary_status(&format!("{error:#}"));
            }
        } else {
            self.begin_create_draw();
        }
    }

    pub(crate) fn open_draw_document(&mut self, id: &str) -> anyhow::Result<()> {
        anyhow::ensure!(self.config.features.draw_view.is_enabled(), "Draw disabled");
        if self.palette_source() == ViewMode::Draw
            && self
                .draw_state
                .as_ref()
                .is_some_and(|draw| draw.current_file == id)
        {
            if self.mode != ViewMode::Draw {
                self.save_before_session_replace()?;
                self.return_mode = Some(self.mode);
                self.mode = ViewMode::Draw;
            }
            return Ok(());
        }
        self.save_before_session_replace()?;
        let plugin = crate::draw_adapter::DrawPlugin::load(
            &self.storage,
            id,
            self.keybinds.clone(),
            self.seq_matcher.clone(),
        )?;
        self.draw_state = Some(plugin);
        if self.mode != ViewMode::Draw {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Draw;
        }
        Ok(())
    }

    pub fn close_draw_view(&mut self) {
        let editing_id = self.editor.editing_id.take();
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);

        if let Some(id) = editing_id {
            self.refresh_note_single(None, &id);
        } else {
            self.request_notes_reconcile();
        }
        self.set_default_status();
    }

    pub fn open_canvas_view(&mut self) {
        if self.feature_disabled(
            self.config.features.canvas_view.is_enabled(),
            "Canvas view",
            "canvas_view",
        ) {
            return;
        }
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let note_id = self.notes[*summary_idx].id.clone();
            let path = self.storage.note_path(&note_id);
            let prev_mode = self.mode;
            self.load_and_open_note(&note_id, None);
            if self.mode == ViewMode::Edit {
                if prev_mode != ViewMode::Canvas {
                    self.return_mode = Some(prev_mode);
                }
                self.mode = ViewMode::Canvas;
            } else {
                self.set_temporary_status_static("Failed to load .canvas file!");
                self.messages.push(
                    "Failed to load .canvas file!".to_string(),
                    crate::app::messages::MessageSeverity::Warning,
                );
            }

            match crate::pinstar_adapter::PinstarPlugin::new_in_storage(
                &path,
                &self.config,
                self.keybinds.clone(),
                self.seq_matcher.clone(),
                self.image_picker.clone(),
                &self.storage,
            ) {
                Ok(plugin) => {
                    self.canvas_state = Some(plugin);
                    self.set_default_status();
                }
                Err(_) => {
                    self.set_temporary_status_static("Failed to load .canvas file!");
                    self.messages.push(
                        "Failed to load .canvas file!".to_string(),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                }
            }
        }
    }

    pub fn close_canvas_view(&mut self) {
        let editing_id = self.editor.editing_id.take();
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);

        if let Some(id) = editing_id {
            self.refresh_note_single(None, &id);
        } else {
            self.request_notes_reconcile();
        }
        self.set_default_status();
    }

    pub fn switch_help_tab(&mut self, tab: HelpTab) {
        if tab == self.help_tab {
            return;
        }
        let current_page = self.help_page;
        self.help_tab_page.insert(self.help_tab, current_page);
        self.help_tab = tab;
        self.help_info_active = 0;
        self.help_side_scroll = [0; 2];
        self.help_page = self.help_tab_page.get(&tab).copied().unwrap_or(0);
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.reroll_help_suggestions();
    }

    pub fn reroll_help_suggestions(&mut self) {
        self.help_side_scroll[1] = 0;
        let rolled = crate::ui::roll_suggestions(self.help_tab, 3);
        self.help_suggestions = rolled.into_iter().copied().collect();
    }

    pub fn begin_create_draw(&mut self) {
        if self.feature_disabled(
            self.config.features.draw_view.is_enabled(),
            "Draw view",
            "draw_view",
        ) {
            return;
        }
        let folder = if self.list.notes_layout.browses_folders() {
            self.list.grid_folder.clone()
        } else {
            self.get_current_folder_context()
        };
        self.begin_create_draw_in_folder(folder);
    }

    pub fn begin_create_draw_in_folder(&mut self, folder: String) {
        let folder = if Self::is_virtual_path(&folder) {
            String::new()
        } else {
            folder
        };
        let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Drawing Name - Esc to cancel, Enter to create"),
        );
        self.present_action_popup(crate::popups::ActivePopup::CreateNote(
            crate::popups::NoteCreatePopup { folder, input },
            crate::popups::NoteFormat::Draw,
        ));
    }

    pub fn begin_create_canvas(&mut self) {
        if self.feature_disabled(
            self.config.features.canvas_view.is_enabled(),
            "Canvas view",
            "canvas_view",
        ) {
            return;
        }
        let folder = if self.list.notes_layout.browses_folders() {
            self.list.grid_folder.clone()
        } else {
            self.get_current_folder_context()
        };
        self.begin_create_canvas_in_folder(folder);
    }

    pub fn begin_create_canvas_in_folder(&mut self, folder: String) {
        let folder = if Self::is_virtual_path(&folder) {
            String::new()
        } else {
            folder
        };
        let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Canvas Name - Esc to cancel, Enter to create"),
        );
        self.present_action_popup(crate::popups::ActivePopup::CreateNote(
            crate::popups::NoteCreatePopup { folder, input },
            crate::popups::NoteFormat::Canvas,
        ));
    }

    pub fn open_trash_view(&mut self) {
        if self.feature_disabled(self.config.features.trash.is_enabled(), "Trash", "trash") {
            return;
        }
        match self.storage.list_trash(&self.config.features) {
            Ok(items) => {
                if items.is_empty() {
                    self.set_temporary_status_static("Trash is empty");
                    return;
                }
                self.popups.active = Some(crate::popups::ActivePopup::TrashView(TrashView {
                    items,
                    selected: 0,
                    scroll_offset: 0,
                    last_scroll: None,
                }));
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to open trash: {e}"));
            }
        }
    }

    pub fn close_trash_view(&mut self) {
        self.popups.active = None;
    }
}

#[cfg(test)]
mod tests {
    fn make_test_storage(dir: &std::path::Path) -> crate::storage::Storage {
        crate::storage::Storage {
            data_dir: dir.to_path_buf(),
            config_dir: dir.to_path_buf(),
            notes_dir: dir.to_path_buf(),
            templates_dir: dir.to_path_buf(),
            skip_dir_patterns: vec![],
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        }
    }

    #[test]
    fn disabled_graph_view_blocks_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = make_test_storage(tmp.path());
        let mut app = crate::app::App::new(storage).unwrap();
        app.config.features.graph_view = crate::config::FeatureState::Disabled;
        app.mode = crate::app::ViewMode::List;
        app.open_graph_view();
        assert_eq!(app.mode, crate::app::ViewMode::List);
        assert!(app.status.contains("disabled"));
    }
}
