use super::*;
use crate::constants::*;
use crate::list_view::*;
use crate::popups::*;
use std::borrow::Cow;
use ratatui_textarea::TextArea;

impl App {


    pub fn open_help_page(&mut self) {
        self.open_help_page_with_tab(HelpTab::Notes);
    }

    pub fn open_help_page_with_tab(&mut self, tab: HelpTab) {
        self.mode = ViewMode::Help;
        self.help_tab = tab;
        self.help_scroll = 0;
        self.help_tab_scroll.insert(tab, 0);
        self.status = Cow::Borrowed(HELP_PAGE_HINTS);
        self.status_until = None;
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
    }

    pub fn close_help_page(&mut self) {
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        self.help_scroll = 0;
        self.help_tab = HelpTab::Notes;
        self.help_tab_scroll.clear();
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
        self.set_default_status();
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
                    let node_count = state.graph_state.as_ref()
                        .and_then(|g| g.read().ok())
                        .map(|g| g.simulation.get_graph().node_count())
                        .unwrap_or(0);
                    debug_log!(self, Info, "graf", "Graph view initialized ({node_count} nodes)");
                    self.graph_state = Some(state);
                }
                Err(_) => {
                    self.set_temporary_status_static("Failed to build graph view");
                    return;
                }
            }
        }
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::Graph;
        debug_log!(self, Info, "view", "View: {:?} → Graph (opened)", self.return_mode.unwrap_or(ViewMode::List));
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
                    debug_log!(self, Debug, "content-tree", "Content tree parsed: {} nodes from {id}", state.nodes.len());
                    Some(state)
                }
                Err(e) => {
                    debug_log!(self, Warn, "content-tree", "Content tree parse failed for {id}: {e}");
                    Some(crate::content_tree::state::ContentTreeState::error(
                        id,
                        self.keybinds.clone(),
                        self.seq_matcher.clone(),
                    ))
                }
            }
        } else {
            Some(crate::content_tree::state::ContentTreeState::error(
                String::new(),
                self.keybinds.clone(),
                self.seq_matcher.clone(),
            ))
        };
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::ContentTree;
        debug_log!(self, Info, "view", "View: {:?} → ContentTree (opened)", self.return_mode.as_ref().unwrap_or(&ViewMode::List));
    }

    pub fn open_backup_view(&mut self) {
        let vault_path = self.config.effective_storage_path().unwrap_or_else(|_| {
            std::path::PathBuf::from(".")
        });
        let config = &self.config;
        debug_log!(self, Debug, "backup-dashboard", "Backup dashboard opened");
        self.backup_state = Some(crate::backup::state::BackupState::new(
            vault_path,
            &config.backup,
            self.app_theme.clone(),
            self.keybinds.clone(),
            config.ui.tab_icons_only,
            self.git_lock.clone(),
            self.seq_matcher.clone(),
        ));
        // Set footer hint
        if let Some(backup) = &mut self.backup_state {
            backup.footer_hint = format!(
                "{}: commit · {}: push · {}: refresh · {}: settings · {}: ←",
                self.keybinds.backup_keys_display(crate::keybinds::BackupAction::EnterCommit),
                self.keybinds.backup_keys_display(crate::keybinds::BackupAction::Push),
                self.keybinds.backup_keys_display(crate::keybinds::BackupAction::Refresh),
                self.keybinds.backup_keys_display(crate::keybinds::BackupAction::OpenSettings),
                self.keybinds.backup_keys_display(crate::keybinds::BackupAction::Back),
            );
        }
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::Backup;
        debug_log!(self, Info, "view", "View: {:?} → Backup (opened)", self.return_mode.as_ref().unwrap_or(&ViewMode::List));
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
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::Draw;
        debug_log!(self, Info, "view", "View: {:?} → Draw (opened)", self.return_mode.as_ref().unwrap_or(&ViewMode::List));
    }

    pub fn close_draw_view(&mut self) {
        self.editor.editing_id = None;
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        debug_log!(self, Info, "view", "View: Draw → {:?}", self.mode);
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
                self.return_mode = Some(self.mode);
                self.mode = ViewMode::Canvas;
                debug_log!(self, Info, "view", "View: {:?} → Canvas (opened)", self.return_mode.as_ref().unwrap_or(&ViewMode::List));
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
        debug_log!(self, Info, "view", "View: Canvas → {:?}", self.mode);
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        self.set_default_status();
    }

    pub fn switch_help_tab(&mut self, tab: HelpTab) {
        if tab == self.help_tab {
            return;
        }
        let current_scroll = self.help_scroll;
        self.help_tab_scroll.insert(self.help_tab, current_scroll);
        self.help_tab = tab;
        self.help_scroll = self.help_tab_scroll.get(&tab).copied().unwrap_or(0);
        self.list.help_text_cache = None;
        self.help_search = crate::app::HelpSearchState::default();
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
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Drawing Name - Esc to cancel, Enter to create"),
        );
        self.popups.create_note = Some((
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
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Canvas Name - Esc to cancel, Enter to create"),
        );
        self.popups.create_note = Some((
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
                self.popups.trash_view = Some(TrashView { items, selected: 0 });
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to open trash: {e}"));
            }
        }
    }

    pub fn close_trash_view(&mut self) {
        self.popups.trash_view = None;
    }}
