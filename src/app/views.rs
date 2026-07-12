use super::*;
use crate::list_view::*;
use crate::popups::*;
use std::borrow::Cow;

impl App {
    pub fn open_help_page(&mut self) {
        self.open_help_page_with_tab(HelpTab::Notes);
    }

    pub fn open_help_page_with_tab(&mut self, tab: HelpTab) {
        if self.mode != ViewMode::Help {
            self.return_mode = Some(self.mode);
        }
        self.mode = ViewMode::Help;
        self.help_tab = tab;
        self.help_page = 0;
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
        self.setup_state = Some(crate::setup::SetupState::from_config(
            &self.config,
            &self.app_theme,
        ));
        self.status = Cow::Borrowed("");
        self.status_until = None;
    }

    pub fn open_graph_view(&mut self) {
        if self.graph_state.is_none() {
            match crate::graf::app::GrafAppState::new(
                &self.config,
                self.storage.clone(),
                vec![],
                self.keybinds.clone(),
                self.seq_matcher.clone(),
            ) {
                Ok(state) => {
                    self.graph_state = Some(state);
                }
                Err(_) => {
                    self.set_temporary_status_static("Failed to build graph view");
                    return;
                }
            }
        }
        if self.mode != ViewMode::Graph {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Graph;
        }
    }
    pub fn open_content_tree_view(&mut self) {
        let note_id = self.get_selected_note_id();
        self.content_tree_state = if let Some(id) = note_id {
            match self.storage.load_note(&id) {
                Ok(note) => {
                    let state = crate::content_tree::state::ContentTreeState::new(
                        id.clone(),
                        &note.title,
                        &note.content,
                        self.keybinds.clone(),
                        self.seq_matcher.clone(),
                    );

                    Some(state)
                }
                Err(_) => Some(crate::content_tree::state::ContentTreeState::error(
                    id,
                    self.keybinds.clone(),
                    self.seq_matcher.clone(),
                )),
            }
        } else {
            Some(crate::content_tree::state::ContentTreeState::error(
                String::new(),
                self.keybinds.clone(),
                self.seq_matcher.clone(),
            ))
        };
        if self.mode != ViewMode::ContentTree {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::ContentTree;
        }
    }

    pub fn open_backup_view(&mut self) {
        let vault_path = crate::config::vault_path_or_dot(&self.config);
        let config = &self.config;

        self.backup_state = Some(crate::backup::state::BackupState::new(
            vault_path,
            &config.backup,
            self.app_theme.clone(),
            self.keybinds.clone(),
            config.ui.tab_icons_only,
            self.git_lock.clone(),
            self.seq_matcher.clone(),
        ));
        if self.mode != ViewMode::Backup {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Backup;
        }
    }

    pub fn open_draw_view(&mut self) {
        let note_id = self.get_selected_note_id();
        self.draw_state = Some(crate::draw::app::DrawAppState::new(
            self.storage.clone(),
            note_id,
            self.app_theme.clone(),
            self.keybinds.clone(),
            self.seq_matcher.clone(),
        ));
        if self.mode != ViewMode::Draw {
            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Draw;
        }
    }

    pub fn close_draw_view(&mut self) {
        self.editor.editing_id = None;
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);

        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        self.set_default_status();
    }

    pub fn open_canvas_view(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let path = self.storage.note_path(&self.notes[*summary_idx].id);
            if let Ok(state) = crate::pinstar::state::PinstarState::load(
                &path,
                self.keybinds.clone(),
                self.seq_matcher.clone(),
            ) {
                self.canvas_state = Some(state);
                if self.mode != ViewMode::Canvas {
                    self.return_mode = Some(self.mode);
                    self.mode = ViewMode::Canvas;
                }

                self.editor.editing_id = Some(self.notes[*summary_idx].id.clone());
                self.set_default_status();
            } else {
                self.set_temporary_status_static("Failed to load .canvas file!");
            }
        }
    }

    pub fn close_canvas_view(&mut self) {
        self.editor.editing_id = None;
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);

        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
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
        self.help_page = self.help_tab_page.get(&tab).copied().unwrap_or(0);
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.reroll_help_suggestions();
    }

    pub fn reroll_help_suggestions(&mut self) {
        let rolled = crate::ui::roll_suggestions(self.help_tab, 3);
        self.help_suggestions = rolled.into_iter().copied().collect();
    }

    pub fn begin_create_draw(&mut self) {
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            self.list.grid_folder.clone()
        } else {
            self.get_current_folder_context()
        };
        self.begin_create_draw_in_folder(folder);
    }

    pub fn begin_create_draw_in_folder(&mut self, folder: String) {
        let folder = if Self::is_virtual_pinned_path(&folder) {
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
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            self.list.grid_folder.clone()
        } else {
            self.get_current_folder_context()
        };
        self.begin_create_canvas_in_folder(folder);
    }

    pub fn begin_create_canvas_in_folder(&mut self, folder: String) {
        let folder = if Self::is_virtual_pinned_path(&folder) {
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
        match self.storage.list_trash() {
            Ok(items) => {
                if items.is_empty() {
                    self.set_temporary_status_static("Trash is empty");
                    return;
                }
                self.popups.active = Some(crate::popups::ActivePopup::TrashView(TrashView {
                    items,
                    selected: 0,
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
