use super::*;
use crate::list_view::*;
use crate::popups::*;
use std::borrow::Cow;

impl App {
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
            self.return_mode = Some(self.mode);
        }
        self.mode = ViewMode::Help;
        self.help_tab = tab;
        self.help_page = 0;
        self.help_info_active = 0;
        self.help_tab_page.insert(tab, 0);
        self.status = Cow::Borrowed("");
        self.status_until = None;
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.reroll_help_suggestions();
    }

    pub fn close_help_page(&mut self) {
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        self.help_page = 0;
        self.help_tab_page.clear();
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.set_default_status();
    }

    pub fn open_setup_view(&mut self) {
        if self.mode != ViewMode::Setup {
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
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            self.list.grid_folder.clone()
        } else {
            self.get_current_folder_context()
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
        self.popups.active = Some(crate::popups::ActivePopup::CreateNote(
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
        if let Some(worker) = &mut self.bases_worker {
            worker.invalidate();
        }
        self.bases_refresh_requested = true;
    }

    pub fn poll_bases(&mut self) -> bool {
        let mut dirty = false;
        if self.bases_refresh_requested
            && let Some(mut state) = self.bases_state.take()
        {
            state.refresh(self, false);
            self.bases_state = Some(state);
            dirty = true;
        }
        if let Some(loaded) = self.bases_worker.as_mut().and_then(|worker| worker.poll()) {
            if let Some(state) = &mut self.bases_state {
                state.install(loaded);
            }
            dirty = true;
        }
        if let Some(mut state) = self.bases_state.take() {
            dirty |= state.poll_write(self);
            self.bases_state = Some(state);
        }
        dirty
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
        let note_id = self.get_selected_note_id();
        let state = crate::draw::app::DrawAppState::new(
            self.storage.clone(),
            note_id,
            self.app_theme.clone(),
            self.keybinds.clone(),
            self.seq_matcher.clone(),
        );
        self.draw_state = Some(state);
        if self.mode != ViewMode::Draw {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Draw;
        }
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

            match crate::pinstar_adapter::PinstarPlugin::new(
                &path,
                &self.config,
                self.keybinds.clone(),
                self.seq_matcher.clone(),
                self.image_picker.clone(),
                &self.storage.data_dir,
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
        self.help_page = self.help_tab_page.get(&tab).copied().unwrap_or(0);
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.reroll_help_suggestions();
    }

    pub fn reroll_help_suggestions(&mut self) {
        let rolled = crate::ui::roll_suggestions(self.help_tab, 4);
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
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
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
        self.popups.active = Some(crate::popups::ActivePopup::CreateNote(
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
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
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
        self.popups.active = Some(crate::popups::ActivePopup::CreateNote(
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
