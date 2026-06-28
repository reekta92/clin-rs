use crate::debug_log;
use super::*;
use crate::list_view::*;
use crate::popups::*;
use crate::storage::Note;
use crate::templates::Template;
use anyhow::{Context, Result};
use std::collections::HashSet;
use crate::fsutil::SecretTempFile;
use std::borrow::Cow;
use ratatui_textarea::TextArea;

impl App {


    pub fn refresh_notes(&mut self) -> Result<()> {
        self.load_cancel.store(true, Ordering::Release);

        let ids = self.storage.list_note_ids(self.list.show_hidden_files)?;
        let mut summaries = Vec::new();
        let mut cached = 0usize;

        for id in &ids {
            let mt = self.storage.note_mtime_millis(id);
            if self.summary_mtime.get(id) == Some(&mt)
                && let Some(s) = self.summary_cache.get(id)
            {
                cached += 1;
                summaries.push(s.clone());
                continue;
            }
            if let Ok(summary) = self.storage.load_note_summary(id) {
                self.summary_cache.insert(id.clone(), summary.clone());
                self.summary_mtime.insert(id.clone(), mt);
                summaries.push(summary);
            }
        }

        let id_set: HashSet<&String> = ids.iter().collect();
        self.summary_cache.retain(|k, _| id_set.contains(k));
        self.summary_mtime.retain(|k, _| id_set.contains(k));

        self.notes = summaries;
        let len = self.notes.len();
        self.sort_notes();
        self.refresh_visual_list();
        debug_log!(self, Debug, "storage", "Notes refreshed: {len} total ({cached} cached hits)");
        Ok(())
    }

    /// Update only one note's summary after an in-place edit, reusing the existing
    /// summary_cache for every other note. Avoids the full per-note stat loop in
    /// refresh_notes. `prev_id` is the id before the edit; it may differ from `id`
    /// when the note was renamed because of a title change (save_note renames).
    pub fn refresh_note_single(&mut self, prev_id: Option<&str>, id: &str) {
        // 1. Handle rename: drop the old id from every view.
        if let Some(old) = prev_id {
            if old != id {
                self.summary_cache.remove(old);
                self.summary_mtime.remove(old);
                self.notes.retain(|n| n.id != old);
            }
        }
        // 2. Reload this one note's summary + mtime, replace in notes.
        if let Ok(summary) = self.storage.load_note_summary(id) {
            let mt = self.storage.note_mtime_millis(id);
            self.summary_cache.insert(id.to_string(), summary.clone());
            self.summary_mtime.insert(id.to_string(), mt);
            self.notes.retain(|n| n.id != id);
            self.notes.push(summary);
        } else {
            // Note vanished (e.g. deleted out of band): ensure it is gone.
            self.notes.retain(|n| n.id != id);
            self.summary_cache.remove(id);
            self.summary_mtime.remove(id);
        }
        // 3. Folders are unchanged for a same-folder title rename, so keep
        //    folder_cache as-is (list_folders rescan is the only other FS cost in
        //    refresh_visual_list; skipping it is the point).
        self.sort_notes();
        self.refresh_visual_list();
        debug_log!(self, Debug, "storage", "Incremental note refresh: prev={prev_id:?} id={id}");
    }

    pub(crate) fn sort_notes(&mut self) {
        self.notes.sort_by(|a, b| {
            if self.pinned_on_top {
                let pin_cmp = b.pinned.cmp(&a.pinned);
                if pin_cmp != std::cmp::Ordering::Equal {
                    return pin_cmp;
                }
            }

            let a_clin = a.id.ends_with(".clin");
            let b_clin = b.id.ends_with(".clin");
            let clin_cmp = b_clin.cmp(&a_clin);
            if clin_cmp != std::cmp::Ordering::Equal {
                return clin_cmp;
            }

            match self.list.sort_field {
                SortField::Modified => match self.list.sort_order {
                    SortOrder::Descending => b.updated_at.cmp(&a.updated_at),
                    SortOrder::Ascending => a.updated_at.cmp(&b.updated_at),
                },
                SortField::Title => match self.list.sort_order {
                    SortOrder::Ascending => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                    SortOrder::Descending => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
                },
            }
        });
    }

    pub fn get_current_folder_context(&self) -> String {
        let current = match self.list.visual_list.get(self.list.visual_index) {
            Some(VisualItem::Folder { path, .. }) => path.clone(),
            Some(VisualItem::Note { summary_idx, .. }) => self
                .notes
                .get(*summary_idx)
                .map(|n| n.folder.clone())
                .unwrap_or_default(),
            Some(VisualItem::CreateNew { path, .. }) => path.clone(),
            None => String::new(),
        };

        let current = if Self::is_virtual_pinned_path(&current) {
            String::new()
        } else {
            current
        };

        if current.is_empty() {
            self.default_folder.clone().unwrap_or_default()
        } else {
            current
        }
    }

    pub fn open_note_at_line(&mut self, note_id: &str, line_number: Option<usize>) {
        if note_id.ends_with(".draw") {
            self.open_draw_view();
            return;
        }
        if note_id.ends_with(".canvas") {
            self.open_canvas_view();
            return;
        }
        if note_id.ends_with(".clin") {
            self.status =
                Cow::Borrowed("Note is encrypted. Use command palette (Ctrl+P) to decrypt.");
            return;
        }
        if self.editor.external_editor_enabled {
            self.open_note_in_external_editor(note_id, line_number);
        } else {
            self.load_and_open_note(note_id, line_number);
        }
    }

    pub fn open_selected(&mut self) {
        if self.list.visual_list.is_empty() {
            return;
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::CreateNew { path, .. } => {
                self.begin_create_select_format_in_folder(path.clone());
            }
            VisualItem::Folder { path, .. } => {
                let p = path.clone();
                if self.list.notes_layout == crate::config::NotesLayout::Grid {
                    self.list.grid_folder = p;
                    self.list.visual_index = 0;
                } else if self.list.folder_expanded.contains(&p) {
                    self.list.folder_expanded.remove(&p);
                } else {
                    self.list.folder_expanded.insert(p);
                }
                self.refresh_visual_list();
            }
            VisualItem::Note { summary_idx, .. } => {
                let note_id = self.notes.get(*summary_idx).map(|s| s.id.clone());
                if let Some(id) = note_id {
                    self.open_note_at_line(&id, None);
                }
            }
        }
    }

    pub fn load_and_open_note(&mut self, note_id: &str, line_number: Option<usize>) {
        if let Ok(note) = self.storage.load_note(note_id) {
            self.editor.editing_id = Some(note_id.to_string());
            self.editor.initial_word_count = crate::goals::count_words(&note.content);
            self.editor.title_editor = make_title_editor(
                &note.title,
                self.app_theme.highlight_fg,
                self.app_theme.highlight_bg,
            );
            let mut editor = text_area_from_content(&note.content);
            if let Some(l) = line_number {
                editor.move_cursor(ratatui_textarea::CursorMove::Jump(
                    l.saturating_sub(1) as u16,
                    0,
                ));
            }
            self.editor.editor = editor;
            self.mode = ViewMode::Edit;
            debug_log!(self, Info, "view", "View: List → Edit (note={note_id})");
            if self.editor.editor_preview_enabled {
                self.update_editor_markdown_preview();
            } else {
                self.editor.md_preview_renderer = None;
            }
            self.status = Cow::Borrowed("");
            debug_log!(self, Info, "storage", "Note opened: {note_id} (line={line_number:?})");
        } else {
            self.status = Cow::Borrowed("Failed to load note!");
            debug_log!(self, Warn, "storage", "Failed to open note: {note_id}");
        }
    }

    pub fn open_note_in_external_editor(&mut self, note_id: &str, line_number: Option<usize>) {
        debug_log!(self, Info, "ext-editor", "Opening {note_id} in external editor");
        if let Ok(note) = self.storage.load_note(note_id) {
            let temp_dir = std::env::temp_dir();
            let temp_id = uuid::Uuid::new_v4().to_string();
            let temp_file_path = temp_dir.join(format!("clin_{temp_id}.md"));

            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&temp_file_path);

                match file {
                    Ok(mut f) => {
                        use std::io::Write;
                        if let Err(e) = f.write_all(note.content.as_bytes()) {
                            self.set_temporary_status(&format!("Failed to write temp file: {e}"));
                            return;
                        }
                    }
                    Err(e) => {
                        self.set_temporary_status(&format!("Failed to create temp file: {e}"));
                        return;
                    }
                }
            }

            #[cfg(not(unix))]
            {
                use std::io::Write;
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temp_file_path)
                {
                    Ok(mut f) => {
                        if let Err(e) = f.write_all(note.content.as_bytes()) {
                            self.set_temporary_status(&format!("Failed to write temp file: {}", e));
                            return;
                        }
                    }
                    Err(e) => {
                        self.set_temporary_status(&format!("Failed to create temp file: {}", e));
                        return;
                    }
                }
            }

            let _secret = SecretTempFile::new(temp_file_path.clone());
            debug_log!(self, Debug, "ext-editor", "External editor temp file: {}", temp_file_path.display());

            let mut args: Vec<String> = Vec::new();
            if let Some(l) = line_number {
                args.push(format!("+{l}"));
            }
            args.push(temp_file_path.to_string_lossy().into_owned());
            let (result, editor_prog) = self.run_in_external_editor(&args);
            debug_log!(self, Info, "ext-editor", "External editor launched: {editor_prog} for {note_id}");

            match result {
                Ok(status) if status.success() => {
                    if let Ok(new_content) = std::fs::read_to_string(&temp_file_path) {
                        if new_content != note.content {
                            let before_words = crate::goals::count_words(&note.content);
                            let after_words = crate::goals::count_words(&new_content);
                            let mut diff = 0;
                            if after_words > before_words {
                                diff = after_words - before_words;
                            }
                            debug_log!(self, Info, "ext-editor", "External editor changes saved: {note_id} ({diff} words)");

                            let updated_note = Note {
                                title: note.title,
                                content: new_content,
                                updated_at: now_unix_secs(),
                                tags: note.tags,
                            };
                            if let Err(e) = self.storage.save_note(note_id, &updated_note) {
                                debug_log!(self, Error, "storage", "Write failed for {note_id}: {e}");
                                self.set_temporary_status(&format!("Failed to save note: {e}"));
                            } else {
                                self.enqueue_backup(format!("auto: {}", &updated_note.title));
                                self.set_temporary_status_static("Note saved");
                                self.list.folder_cache = None;
                                if let Err(e) = self.refresh_notes() {
                                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                                }

                                let progress = self.get_current_goals_progress();
                                progress.words_written += diff;
                                progress.notes_modified.insert(note_id.to_string());
                                let progress_clone = progress.clone();
                                self.save_goals_progress(&progress_clone);
                            }
                        } else {
                            self.set_temporary_status_static("No changes made in external editor.");
                        }
                        debug_log!(self, Debug, "ext-editor", "External editor: no changes for {note_id}");
                    } else {
                        self.set_temporary_status_static("Failed to read from temp file.");
                    }
                }
                Ok(status) => {
                    debug_log!(self, Warn, "ext-editor", "External editor {editor_prog} exited with status {status}");
                    self.set_temporary_status(&format!(
                        "Editor '{editor_prog}' exited with status: {status}"
                    ));
                }
                Err(e) => {
                    debug_log!(self, Error, "ext-editor", "Failed to launch external editor {editor_prog}: {e}");
                    self.set_temporary_status(&format!(
                        "Failed to launch editor '{editor_prog}': {e}"
                    ));
                }
            }

        } else {
            self.set_temporary_status_static("Failed to load note for external editor!");
        }
    }

    pub fn open_note_by_title(&mut self, title: &str) -> bool {
        let query = title.trim();
        if query.is_empty() {
            return false;
        }

        if let Some(index) = self
            .notes
            .iter()
            .position(|note| note.title.eq_ignore_ascii_case(query))
            && let Some(v_idx) = self.list.visual_list.iter().position(|v| match v {
                VisualItem::Note { summary_idx, .. } => *summary_idx == index,
                _ => false,
            })
        {
            self.list.visual_index = v_idx;
            self.open_selected();
            return true;
        }

        false
    }

    pub fn start_new_note_with_title(&mut self, folder: String, title: String) {
        let template_manager = self.storage.template_manager();
        if let Some(default_template) = template_manager.load_default() {
            self.start_note_from_template_with_title(&default_template, folder, title);
        } else {
            self.start_blank_note_with_title(folder, title);
        }
    }

    pub fn start_blank_note_with_title(&mut self, folder: String, title: String) {
        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() && !Self::is_virtual_pinned_path(&folder) {
            new_id = format!("{folder}/{new_id}");
        }
        debug_log!(self, Info, "storage", "New note created: {new_id} (format=md)");
        self.enter_edit_mode(new_id, title, String::new());
    }

    fn enter_edit_mode(&mut self, id: String, title: String, content: String) {
        if self.editor.external_editor_enabled {
            let new_note = Note {
                title,
                content,
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if let Ok(saved_id) = self.storage.save_note(&id, &new_note) {
                self.enqueue_backup(format!("auto: {}", &new_note.title));
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&saved_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        debug_log!(self, Info, "view", "View: List → Edit (new note={id})");
        self.editor.editing_id = Some(id);
        self.editor.initial_word_count = crate::goals::count_words(&content);
        self.editor.title_editor = make_title_editor(
            &title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = TextArea::from(content.lines());
        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_selection_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());
        self.set_default_status();
    }

    pub fn start_note_from_template(&mut self, template: &Template, folder: String) {
        let rendered = template.render();

        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() && !Self::is_virtual_pinned_path(&folder) {
            new_id = format!("{folder}/{new_id}");
        }

        if self.editor.external_editor_enabled {
            let new_note = Note {
                title: rendered
                    .title
                    .clone()
                    .unwrap_or_else(|| String::from("Untitled note")),
                content: rendered.content.clone(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if let Ok(saved_id) = self.storage.save_note(&new_id, &new_note) {
                self.enqueue_backup(format!("auto: {}", &new_note.title));
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&saved_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.editing_id = Some(new_id);
        self.editor.initial_word_count = crate::goals::count_words(&rendered.content);

        self.editor.title_editor = make_title_editor(
            rendered.title.as_deref().unwrap_or(""),
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = text_area_from_content(&rendered.content);

        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_selection_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());

        self.set_default_status();
    }

    pub fn start_note_from_template_with_title(
        &mut self,
        template: &Template,
        folder: String,
        title: String,
    ) {
        let rendered = template.render();

        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() && !Self::is_virtual_pinned_path(&folder) {
            new_id = format!("{folder}/{new_id}");
        }
        debug_log!(self, Info, "storage", "New note created: {new_id} (from template)");

        if self.editor.external_editor_enabled {
            let new_note = Note {
                title,
                content: rendered.content.clone(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if let Ok(saved_id) = self.storage.save_note(&new_id, &new_note) {
                self.enqueue_backup(format!("auto: {}", &new_note.title));
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&saved_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.editing_id = Some(new_id);
        self.editor.initial_word_count = crate::goals::count_words(&rendered.content);

        self.editor.title_editor = make_title_editor(
            &title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = text_area_from_content(&rendered.content);

        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_selection_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());

        self.set_default_status();
    }

    pub fn back_to_list(&mut self, prev_id: Option<&str>, new_id: Option<&str>) {
        if let Some(return_to) = self.return_mode.take() {
            self.editor.editing_id = None;
            if self.editor.template_edit_path.is_some() {
                self.refresh_template_popup();
            }
            self.editor.template_edit_path = None;
            self.editor.title_editor =
                make_title_editor("", self.app_theme.highlight_fg, self.app_theme.highlight_bg);
            self.editor.editor = TextArea::default();
            self.popups.confirm = None;
            self.editor.md_preview_renderer = None;
            if return_to == ViewMode::Graph && self.graph_state.is_none() {
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
                    Err(e) => {
                        self.set_temporary_status(&format!("Failed to rebuild graph: {e}"));
                        self.mode = ViewMode::List;
                        return;
                    }
                }
            }
            self.mode = return_to;
            self.set_default_status();
            debug_log!(self, Debug, "view", "View: Edit → {:?} (return_mode)", self.mode);
            return;
        }
        self.mode = ViewMode::List;
        self.editor.editing_id = None;
        if self.editor.template_edit_path.is_some() {
            self.refresh_template_popup();
        }
        self.editor.template_edit_path = None;
        self.editor.title_editor =
            make_title_editor("", self.app_theme.highlight_fg, self.app_theme.highlight_bg);
        self.editor.editor = TextArea::default();
        self.popups.confirm = None;
        self.editor.md_preview_renderer = None;
        if let Some(id) = new_id {
            self.refresh_note_single(prev_id, id);
        } else if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        debug_log!(self, Debug, "view", "View: Edit → List");
        self.set_default_status();
    }

    pub fn begin_create_folder(&mut self) {
        let parent_path = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            self.list.grid_folder.clone()
        } else {
            self.get_current_folder_context()
        };
        if Self::is_virtual_pinned_path(&parent_path) {
            self.set_temporary_status_static("Cannot create folder inside virtual Pinned");
            return;
        }
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        let title = if parent_path.is_empty() {
            "Create Folder - Esc to cancel, Enter to save".to_string()
        } else {
            format!("Create Folder in '{parent_path}' - Esc to cancel, Enter to save")
        };
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title(title),
        );
        self.popups.active = Some(crate::popups::ActivePopup::Folder(FolderPopup {
            mode: FolderPopupMode::Create { parent_path },
            input,
        }));
    }

    pub fn begin_rename_folder(&mut self) {
        if let Some(VisualItem::Folder { path, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            if path.is_empty() {
                self.set_temporary_status_static("Cannot rename Vault root");
                return;
            }
            if Self::is_virtual_pinned_path(path) {
                self.set_temporary_status_static("Cannot rename virtual Pinned folder");
                return;
            }
            let mut input = TextArea::default();
            input.set_cursor_line_style(ratatui::style::Style::default());
            input.insert_str(path);
            input.set_style(self.app_theme.bg_style());
            input.set_block(
                ratatui::widgets::Block::default()
                    .style(self.app_theme.bg_style())
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Rename Folder - Esc to cancel, Enter to save"),
            );
            self.popups.active = Some(crate::popups::ActivePopup::Folder(FolderPopup {
                mode: FolderPopupMode::Rename {
                    old_path: path.clone(),
                },
                input,
            }));
        } else {
            self.set_temporary_status_static("Select a folder to rename");
        }
    }

    pub fn open_selected_note_location(&mut self) {
        if self.list.visual_index >= self.list.visual_list.len() {
            self.set_temporary_status_static("No note selected for location");
            return;
        }

        let summary_idx = match &self.list.visual_list[self.list.visual_index] {
            VisualItem::Note { summary_idx, .. } => *summary_idx,
            _ => {
                self.set_temporary_status_static("Selected item is not a note");
                return;
            }
        };

        let Some(note) = self.notes.get(summary_idx) else {
            self.set_temporary_status_static("No note selected for location");
            return;
        };

        let note_path = self.storage.note_path(&note.id);
        let Some(parent) = note_path.parent() else {
            self.set_temporary_status_static("Could not determine note directory");
            return;
        };

        match open_in_file_manager(parent) {
            Ok(()) => self.set_temporary_status_static("Opened note file location"),
            Err(err) => self.set_temporary_status(&format!("Open location failed: {err:#}")),
        }
    }

    pub fn get_selected_note_id(&self) -> Option<String> {
        if let Some(id) = &self.editor.editing_id {
            return Some(id.clone());
        }
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            Some(self.notes[*summary_idx].id.clone())
        } else {
            None
        }
    }

    pub fn open_note_from_graph(&mut self, note_id: &str) {
        if note_id.ends_with(".clin") {
            self.set_temporary_status_static("Cannot open encrypted notes. Decrypt first.");
            return;
        }
        self.return_mode = Some(ViewMode::Graph);
        if self.editor.external_editor_enabled {
            self.open_note_in_external_editor(note_id, None);
            // graph_state was destroyed when note was opened; rebuild it
            self.return_mode.take(); // discard return_mode (was set to Graph above)
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
                        self.set_temporary_status_static("Failed to rebuild graph view");
                    }
                }
            }
            self.mode = ViewMode::Graph;
        } else {
            self.load_and_open_note(note_id, None);
        }
    }

    pub fn begin_create_select_format(&mut self) {
        let folder = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            if Self::is_virtual_pinned_path(&self.list.grid_folder) {
                String::new()
            } else {
                self.list.grid_folder.clone()
            }
        } else {
            self.get_current_folder_context()
        };
        self.begin_create_select_format_in_folder(folder);
    }
    pub fn begin_create_select_format_in_folder(&mut self, folder: String) {
        self.popups.active = Some(crate::popups::ActivePopup::CreateFormat(CreateFormatPopup {
            folder,
            selected: 0,
        }));
    }
    pub fn confirm_create_format(&mut self) {
        if let Some(crate::popups::ActivePopup::CreateFormat(popup)) = self.popups.active.take() {
            match popup.selected {
                0 => self.begin_create_note_in_folder(popup.folder), // .md
                1 => self.begin_create_text_in_folder(popup.folder), // .txt (new)
                2 => self.begin_create_draw_in_folder(popup.folder), // .draw
                3 => self.begin_create_canvas_in_folder(popup.folder), // .canvas
                _ => {}
            }
        }
    }

    pub fn begin_create_text_in_folder(&mut self, folder: String) {
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
                .title("New Text File Name - Esc to cancel, Enter to create"),
        );
        self.popups.active = Some(crate::popups::ActivePopup::CreateNote(
            NoteCreatePopup { folder, input },
            NoteFormat::PlainText,
        ));
    }

    pub fn begin_create_note_in_folder(&mut self, folder: String) {
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
                .title("New Note Name - Esc to cancel, Enter to create"),
        );
        self.popups.active = Some(crate::popups::ActivePopup::CreateNote(
            NoteCreatePopup { folder, input },
            crate::popups::NoteFormat::Markdown,
        ));
    }

    pub fn confirm_create_note(&mut self) {
        if let Some(crate::popups::ActivePopup::CreateNote(popup, format)) = self.popups.active.take() {
            let mut title = popup.input.lines().join("");
            title = title.trim().to_string();
            match format {
                crate::popups::NoteFormat::Markdown => {
                    if title.is_empty() {
                        title = String::from("Untitled note");
                    }
                    self.start_new_note_with_title(popup.folder, title);
                }
                crate::popups::NoteFormat::PlainText => {
                    if title.is_empty() {
                        title = String::from("Untitled text");
                    }
                    let mut id = self.storage.new_note_id();
                    id.push_str(".txt");
                    let full_id = if popup.folder.is_empty() {
                        id
                    } else {
                        format!("{}/{}", popup.folder, id)
                    };
                    debug_log!(self, Info, "storage", "New file created: {full_id} (format=txt)");
                    self.enter_edit_mode(full_id, title, String::new());
                }
                crate::popups::NoteFormat::Draw => {
                    if title.is_empty() {
                        title = String::from("Untitled drawing");
                    }
                    let canvas_id = if popup.folder.is_empty() {
                        format!("{title}.draw")
                    } else {
                        format!("{}/{}.draw", popup.folder, title)
                    };
                    self.return_mode = Some(self.mode);
                    debug_log!(self, Info, "storage", "New file created: {canvas_id} (format=draw)");
                    self.mode = ViewMode::Draw;
                    self.editor.editing_id = Some(canvas_id);
                }
                crate::popups::NoteFormat::Canvas => {
                    if title.is_empty() {
                        title = String::from("Untitled canvas");
                    }
                    let canvas_id = if popup.folder.is_empty() {
                        format!("{title}.canvas")
                    } else {
                        format!("{}/{}.canvas", popup.folder, title)
                    };
                    let path = self.storage.note_path(&canvas_id);
                    if !path.exists() {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let data = crate::pinstar::data::CanvasData {
                            nodes: vec![],
                            edges: vec![],
                        };
                        if let Ok(content) = serde_json::to_string_pretty(&data)
                            && let Err(e) = crate::fsutil::atomic_write_str(&path, &content)
                        {
                            self.set_temporary_status(&format!("Failed to write canvas file: {e}"));
                            return;
                        }
                    }
                    self.return_mode = Some(self.mode);
                    debug_log!(self, Info, "storage", "New file created: {canvas_id} (format=canvas)");
                    self.mode = ViewMode::Canvas;
                    self.editor.editing_id = Some(canvas_id);
                    if let Ok(state) = crate::pinstar::state::PinstarState::load(
                        &path,
                        self.keybinds.clone(),
                        self.seq_matcher.clone(),
                    ) {
                        self.canvas_state = Some(state);
                    }
                    self.set_default_status();
                }
            }
        }
    }

    pub fn insert_content(
        &mut self,
        target: ImportTarget,
        note_id: Option<&str>,
        title: String,
        content: String,
    ) -> Result<()> {
        debug_log!(self, Info, "storage", "Content imported: {title} ({} bytes)", content.len());
        match target {
            ImportTarget::NewNote => {
                let folder = self.get_current_folder_context();
                self.start_note_with_content(folder, title, content);
            }
            ImportTarget::AppendCurrent => {
                let id = note_id.context("No note selected")?;
                let mut note = self.storage.load_note(id)?;
                note.content.push_str("\n\n");
                note.content.push_str(&content);
                note.updated_at = now_unix_secs();
                self.storage.save_note(id, &note)?;
                self.enqueue_backup(format!("auto: {}", note.title));
                self.refresh_notes()?;
                self.set_temporary_status_static("Content appended");
            }
        }
        Ok(())
    }

    pub fn start_note_with_content(&mut self, folder: String, title: String, content: String) {
        let folder = if Self::is_virtual_pinned_path(&folder) {
            String::new()
        } else {
            folder
        };

        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{folder}/{new_id}");
        }

        if self.editor.external_editor_enabled {
            let note = Note {
                title,
                content,
                updated_at: now_unix_secs(),
                tags: vec![],
            };
            if let Ok(saved_id) = self.storage.save_note(&new_id, &note) {
                self.enqueue_backup(format!("auto: {}", &note.title));
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&saved_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.editing_id = Some(new_id);
        self.editor.initial_word_count = crate::goals::count_words(&content);
        self.editor.title_editor = make_title_editor(
            &title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = text_area_from_content(&content);
        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_selection_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());
        self.set_default_status();
    }
    pub fn begin_rename_note(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let summary_idx = *summary_idx;
            let id = self.notes[summary_idx].id.clone();
            let note = &self.notes[summary_idx];
            let mut input = TextArea::default();
            input.set_cursor_line_style(ratatui::style::Style::default());
            input.insert_str(&note.title);
            input.set_style(self.app_theme.bg_style());
            input.set_block(
                ratatui::widgets::Block::default()
                    .style(self.app_theme.bg_style())
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Rename Note - Esc to cancel, Enter to save"),
            );
            self.popups.active = Some(crate::popups::ActivePopup::NoteRename(NoteRenamePopup { note_id: id, input }));
        } else {
            self.set_temporary_status_static("Select a note to rename");
        }
    }

    pub fn confirm_rename_note(&mut self) {
        if let Some(crate::popups::ActivePopup::NoteRename(popup)) = self.popups.active.take() {
            let new_title = popup.input.lines().join("");
            let new_title = new_title.trim();
            if new_title.is_empty() {
                self.set_temporary_status_static("Title cannot be empty");
                return;
            }
            match self.storage.rename_note(&popup.note_id, new_title) {
                Ok(_) => {
                    debug_log!(self, Info, "storage", "Note renamed: {} → {new_title}", popup.note_id);
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
                    self.set_temporary_status_static("Note renamed");
                }
                Err(e) => {
                    debug_log!(self, Error, "storage", "Note rename failed for {}: {e}", popup.note_id);
                    self.set_temporary_status(&format!("Failed to rename: {e}"));
                }
            }
        }
    }

    pub fn duplicate_note(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let id = self.notes[*summary_idx].id.clone();
            self.open_folder_picker(FolderPickerMode::CopyNote { note_id: id }, &[]);
        } else {
            self.set_temporary_status_static("Select a note to duplicate");
        }
    }
}
