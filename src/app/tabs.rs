use super::App;
use crate::editor::OpenTab;
use crate::events::get_title_text;

/// Row pitch of the reused notes-tree rows: Comfortable density renders a
/// blank line after every item, so one visual row costs two terminal rows.
pub fn tree_row_pitch(app: &App) -> u16 {
    if app.list.list_density == crate::config::ListDensity::Comfortable {
        2
    } else {
        1
    }
}

impl App {
    fn current_tab_title(&self) -> String {
        get_title_text(&self.editor.title_editor).into_owned()
    }

    /// Persist a still-unsaved new tab (editing_id None) before switching
    /// away. Duplicate titles are allowed: notes are identified by id, and
    /// title-based lookups (wikilinks, graph) resolve to the last match.
    pub fn save_new_tab_if_needed(&mut self) -> Result<(), String> {
        if self.editor.editing_id.is_some() {
            return self.autosave();
        }
        let title = self.current_tab_title();
        let content = self.editor.body.lines().join("\n");
        if title.trim().is_empty() && content.trim().is_empty() {
            return Ok(());
        }
        let final_title = if title.trim().is_empty() {
            "Untitled note".to_string()
        } else {
            title.trim().to_string()
        };
        let folder = self.get_current_folder_context();
        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() && !Self::is_virtual_path(&folder) {
            new_id = format!("{folder}/{new_id}");
        }
        let note = crate::storage::Note {
            title: final_title.clone(),
            content,
            updated_at: crate::ui::now_unix_secs(),
            tags: Vec::new(),
        };
        match self.storage.save_note(&new_id, &note) {
            Ok(saved_id) => {
                self.editor.editing_id = Some(saved_id.clone());
                self.refresh_note_single(None, &saved_id);
                if let Some(tab) = self.editor.tabs.get_mut(self.editor.active_tab) {
                    tab.id = Some(saved_id);
                    tab.title_snapshot = final_title;
                }
                Ok(())
            }
            Err(e) => {
                let err = format!("Failed to save new tab: {e}");
                self.set_temporary_status(&err);
                Err(err)
            }
        }
    }

    pub fn sync_active_tab_title(&mut self) {
        let title = self.current_tab_title();
        let id = self.editor.editing_id.clone();
        if let Some(tab) = self.editor.tabs.get_mut(self.editor.active_tab) {
            tab.title_snapshot = if title.trim().is_empty() {
                "Untitled".to_string()
            } else {
                title
            };
            if tab.id.is_none() {
                tab.id = id;
            }
        }
    }

    pub fn init_tabs_for_note(&mut self, id: &str, title: &str) {
        if self.editor.tabs.is_empty() {
            self.editor
                .tabs
                .push(OpenTab::new(Some(id.to_string()), title));
            self.editor.active_tab = 0;
            self.editor.file_tree_visible = true;
        } else if let Some(pos) = self
            .editor
            .tabs
            .iter()
            .position(|t| t.id.as_deref() == Some(id))
        {
            self.editor.active_tab = pos;
            self.editor.tabs[pos].title_snapshot = title.to_string();
        } else {
            self.editor
                .tabs
                .push(OpenTab::new(Some(id.to_string()), title));
            self.editor.active_tab = self.editor.tabs.len() - 1;
        }
        self.sync_file_tree_to_open_note();
    }

    pub fn open_note_in_tab(&mut self, note_id: &str, line_number: Option<usize>) {
        if self.editor.tabs.is_empty() {
            self.load_and_open_note(note_id, line_number);
            return;
        }
        if let Some(pos) = self
            .editor
            .tabs
            .iter()
            .position(|t| t.id.as_deref() == Some(note_id))
        {
            if pos != self.editor.active_tab {
                if self.save_new_tab_if_needed().is_err() {
                    return;
                }
                self.sync_active_tab_title();
                self.editor.active_tab = pos;
                self.load_note_into_editor(note_id, line_number);
            }
            return;
        }
        if self.save_new_tab_if_needed().is_err() {
            return;
        }
        self.sync_active_tab_title();
        let title = self
            .notes
            .iter()
            .find(|n| n.id == note_id)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        self.editor
            .tabs
            .push(OpenTab::new(Some(note_id.to_string()), title));
        self.editor.active_tab = self.editor.tabs.len() - 1;
        self.load_note_into_editor(note_id, line_number);
    }

    fn load_note_into_editor(&mut self, note_id: &str, line_number: Option<usize>) {
        self.editor.link_complete = None;
        if let Ok(note) = self.storage.load_note(note_id) {
            self.editor.editing_id = Some(note_id.to_string());
            self.editor.initial_word_count = crate::goals::count_words(&note.content);
            self.editor.title_editor = crate::events::make_title_editor(
                &note.title,
                self.app_theme.highlight_fg,
                self.app_theme.highlight_bg,
            );
            let mut body = crate::editor_document::EditorDocument::from_text(&note.content);
            if let Some(l) = line_number {
                body.move_cursor(ratatui_textarea::CursorMove::Jump(
                    l.saturating_sub(1) as u16,
                    0,
                ));
            }
            self.editor.body = body;
            let (fm, _) = crate::frontmatter::parse(&note.content);
            self.editor.text_align = fm.text_align.unwrap_or(self.config.editor.text_align);
            self.apply_editor_prefs();
            self.rebuild_outline();
            self.editor.links = self.compute_links();
            self.editor.image_picker = self.image_picker.clone();
            self.editor.image_decode_tx = self.image_decode_tx.clone();
            self.mode = crate::app::ViewMode::Edit;
            if self.editor.editor_preview_enabled {
                self.update_editor_markdown_preview();
            } else {
                self.editor.md_preview_renderer = None;
            }
            self.sync_file_tree_to_open_note();
        } else {
            self.set_temporary_status_static("Failed to load note!");
        }
    }

    /// Open the new-file format selector (Markdown / Text / Draw / Canvas)
    /// in the current note's folder. The created note opens as a new tab via
    /// the normal creation flow.
    pub fn new_editor_tab(&mut self) {
        if self.save_new_tab_if_needed().is_err() {
            return;
        }
        self.sync_active_tab_title();
        // Persist a possible file rename from autosave into the tab entry.
        let renamed = self.editor.editing_id.clone();
        let cur = self.editor.active_tab;
        if let Some(tab) = self.editor.tabs.get_mut(cur)
            && (tab.id.is_none() || renamed.is_some())
        {
            tab.id = renamed;
        }
        let folder = self.editing_folder_context();
        self.begin_create_select_format_in_folder(folder);
    }

    pub fn close_editor_tab(&mut self, index: usize) {
        if self.editor.tabs.is_empty() {
            return;
        }
        let index = index.min(self.editor.tabs.len() - 1);
        let closing_active = index == self.editor.active_tab;
        if closing_active && self.save_new_tab_if_needed().is_err() {
            return;
        }
        if closing_active {
            // Persist a possible file rename from autosave into the tab entry.
            let renamed = self.editor.editing_id.clone();
            if let Some(tab) = self.editor.tabs.get_mut(index) {
                if tab.id.is_none() {
                    tab.id = renamed;
                } else if let Some(new_id) = renamed {
                    tab.id = Some(new_id);
                }
            }
        }
        self.editor.tabs.remove(index);
        if self.editor.tabs.is_empty() {
            let prev = self.editor.editing_id.clone();
            let new = self.editor.editing_id.clone();
            self.back_to_list(prev.as_deref(), new.as_deref());
            return;
        }
        if self.editor.active_tab >= self.editor.tabs.len() {
            self.editor.active_tab = self.editor.tabs.len() - 1;
        } else if index < self.editor.active_tab {
            self.editor.active_tab -= 1;
        }
        if closing_active {
            let target = self.editor.tabs[self.editor.active_tab].id.clone();
            if let Some(id) = target {
                self.load_note_into_editor(&id, None);
            } else {
                self.editor.editing_id = None;
                self.editor.title_editor = crate::events::make_title_editor(
                    "",
                    self.app_theme.highlight_fg,
                    self.app_theme.highlight_bg,
                );
                self.editor.body = crate::editor_document::EditorDocument::default();
                self.apply_editor_prefs();
                self.editor.outline_nodes.clear();
                self.editor.links.clear();
            }
        }
        self.sync_file_tree_to_open_note();
    }

    pub fn switch_editor_tab(&mut self, index: usize) {
        if self.editor.tabs.is_empty() || index >= self.editor.tabs.len() {
            return;
        }
        if index == self.editor.active_tab {
            return;
        }
        if self.save_new_tab_if_needed().is_err() {
            return;
        }
        self.sync_active_tab_title();
        // Autosave may rename the file (title → filename sync); keep tab id fresh.
        let renamed = self.editor.editing_id.clone();
        let cur = self.editor.active_tab;
        if let Some(tab) = self.editor.tabs.get_mut(cur)
            && (tab.id.is_none() || renamed.is_some())
        {
            tab.id = renamed;
        }
        self.editor.active_tab = index;
        let target = self.editor.tabs[index].id.clone();
        if let Some(id) = target {
            self.load_note_into_editor(&id, None);
        } else {
            self.editor.editing_id = None;
            self.editor.title_editor = crate::events::make_title_editor(
                "",
                self.app_theme.highlight_fg,
                self.app_theme.highlight_bg,
            );
            self.editor.body = crate::editor_document::EditorDocument::default();
            self.apply_editor_prefs();
            self.editor.outline_nodes.clear();
            self.editor.links.clear();
            self.editor.md_preview_renderer = None;
        }
        self.sync_file_tree_to_open_note();
    }

    pub fn next_editor_tab(&mut self) {
        if self.editor.tabs.len() < 2 {
            return;
        }
        let next = (self.editor.active_tab + 1) % self.editor.tabs.len();
        self.switch_editor_tab(next);
    }

    pub fn prev_editor_tab(&mut self) {
        if self.editor.tabs.len() < 2 {
            return;
        }
        let prev = self
            .editor
            .active_tab
            .checked_sub(1)
            .unwrap_or(self.editor.tabs.len() - 1);
        self.switch_editor_tab(prev);
    }

    pub fn toggle_file_tree(&mut self) {
        self.editor.file_tree_visible = !self.editor.file_tree_visible;
        self.set_temporary_status_static(if self.editor.file_tree_visible {
            "File sidebar enabled"
        } else {
            "File sidebar disabled"
        });
        self.clamp_file_tree_selection();
    }

    /// Length of the reused notes-tree (`visual_list`) shown in the editor
    /// file sidebar. Rendering uses `App::format_visual_item`, so the
    /// sidebar looks and behaves exactly like the notes list tree view.
    pub fn file_tree_len(&self) -> usize {
        self.list.visual_list.len()
    }

    pub fn clamp_file_tree_selection(&mut self) {
        let len = self.file_tree_len();
        if len == 0 {
            self.editor.file_tree_selected = 0;
            self.editor.file_tree_scroll = 0;
            return;
        }
        if self.editor.file_tree_selected >= len {
            self.editor.file_tree_selected = len - 1;
        }
    }

    /// Point the sidebar selection at the note currently open in the editor.
    pub fn sync_file_tree_to_open_note(&mut self) {
        let open_id = self.editor.editing_id.clone();
        if let Some(id) = open_id
            && let Some(vi) = self.list.visual_list.iter().position(|v| match v {
                crate::list_view::VisualItem::Note { summary_idx, .. } => {
                    self.notes.get(*summary_idx).is_some_and(|s| s.id == id)
                }
                _ => false,
            })
        {
            self.editor.file_tree_selected = vi;
        }
        self.clamp_file_tree_selection();
    }

    pub fn file_tree_move(&mut self, delta: i32) {
        let len = self.file_tree_len();
        if len == 0 {
            return;
        }
        let cur = self.editor.file_tree_selected as i32;
        self.editor.file_tree_selected = (cur + delta).clamp(0, (len - 1) as i32) as usize;
    }

    fn file_tree_set_folder(&mut self, path: &str, expanded: bool) {
        if expanded {
            self.list.folder_expanded.insert(path.to_string());
        } else {
            self.list.folder_expanded.remove(path);
        }
        self.refresh_visual_list();
        self.clamp_file_tree_selection();
    }

    pub fn file_tree_collapse(&mut self) {
        let sel = self.editor.file_tree_selected;
        let path = match self.list.visual_list.get(sel) {
            Some(crate::list_view::VisualItem::Folder { path, .. }) => Some(path.clone()),
            Some(crate::list_view::VisualItem::SmartFolder { kind, .. }) => {
                Some(kind.virtual_path())
            }
            Some(crate::list_view::VisualItem::Note { summary_idx, .. }) => self
                .notes
                .get(*summary_idx)
                .map(|s| s.folder.clone())
                .filter(|f| !f.is_empty()),
            _ => None,
        };
        if let Some(path) = path {
            self.file_tree_set_folder(&path, false);
        }
    }

    pub fn file_tree_expand(&mut self) {
        let sel = self.editor.file_tree_selected;
        let path = match self.list.visual_list.get(sel) {
            Some(crate::list_view::VisualItem::Folder { path, .. }) => Some(path.clone()),
            Some(crate::list_view::VisualItem::SmartFolder { kind, .. }) => {
                Some(kind.virtual_path())
            }
            _ => None,
        };
        if let Some(path) = path {
            self.file_tree_set_folder(&path, true);
        } else {
            self.file_tree_activate();
        }
    }

    /// Activate the selected sidebar row. Mirrors `open_selected` from the
    /// notes list, except notes open as editor tabs instead of replacing
    /// the current note.
    pub fn file_tree_activate(&mut self) {
        let sel = self.editor.file_tree_selected;
        let item = match self.list.visual_list.get(sel) {
            Some(item) => item.clone(),
            None => return,
        };
        match item {
            crate::list_view::VisualItem::CreateNew { path, .. } => {
                self.begin_create_select_format_in_folder(path);
            }
            crate::list_view::VisualItem::TemplateConfig { folder, .. } => {
                self.open_folder_template_config(&folder);
            }
            crate::list_view::VisualItem::Folder { path, .. } => {
                if self.list.notes_layout == crate::config::NotesLayout::Grid {
                    self.list.grid_folder = path;
                } else if self.list.folder_expanded.contains(&path) {
                    self.list.folder_expanded.remove(&path);
                } else {
                    self.list.folder_expanded.insert(path);
                }
                self.refresh_visual_list();
                self.clamp_file_tree_selection();
            }
            crate::list_view::VisualItem::SmartFolder { kind, .. } => {
                let path = kind.virtual_path();
                if self.list.notes_layout == crate::config::NotesLayout::Grid {
                    self.list.grid_folder = path;
                } else if self.list.folder_expanded.contains(&path) {
                    self.list.folder_expanded.remove(&path);
                } else {
                    self.list.folder_expanded.insert(path);
                }
                self.refresh_visual_list();
                self.clamp_file_tree_selection();
            }
            crate::list_view::VisualItem::Note { summary_idx, .. } => {
                if let Some(id) = self.notes.get(summary_idx).map(|s| s.id.clone()) {
                    self.open_note_in_tab(&id, None);
                }
            }
            crate::list_view::VisualItem::Subnote {
                parent_id,
                subnote_idx,
                ..
            } => {
                self.open_subnotes_popup_for(&parent_id, Some(subnote_idx));
            }
        }
    }
}
