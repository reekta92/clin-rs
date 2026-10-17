use super::*;
use crate::editor_document::EditorDocument;
use crate::fsutil::SecretTempFile;
use crate::list_view::*;
use crate::popups::*;
use crate::storage::Note;
use crate::templates::Template;
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::collections::HashSet;

impl App {
    pub fn request_notes_reconcile(&mut self) {
        self.invalidate_bases();
        let generation_num = self.catalog_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let cmd = crate::app::catalog::CatalogCommand::Reconcile {
            generation: generation_num,
            show_hidden: self.list.show_hidden_files,
            show_all: self.list.show_all_files,
        };
        self.send_catalog_cmd(cmd);
    }

    pub(crate) fn send_catalog_cmd(&mut self, cmd: crate::app::catalog::CatalogCommand) {
        let _ = self.catalog_cmd_tx.try_send(cmd).inspect_err(|e| {
            if matches!(e, std::sync::mpsc::TrySendError::Disconnected(_)) {
                self.messages.push(
                    "Catalog worker disconnected; note list will not refresh".to_string(),
                    crate::app::messages::MessageSeverity::Warning,
                );
            }
        });
    }

    pub fn send_catalog_paths(&mut self, changes: Vec<crate::app::catalog::PathChange>) {
        if !changes.is_empty() {
            self.base_embeds.invalidate();
            if let Some(worker) = &mut self.bases_worker {
                worker.invalidate_paths(changes.iter().map(|change| match change {
                    crate::app::catalog::PathChange::Upsert(id)
                    | crate::app::catalog::PathChange::Remove(id) => id.clone(),
                }));
            }
            self.bases_refresh_requested = true;
        }
        let generation_num = self.catalog_generation.load(Ordering::SeqCst);
        let cmd = crate::app::catalog::CatalogCommand::Paths {
            generation: generation_num,
            changes,
        };
        self.send_catalog_cmd(cmd);
    }

    pub fn handle_catalog_event(&mut self, event: crate::app::catalog::CatalogEvent) {
        use crate::app::catalog::CatalogEvent;
        let cur_gen = self.catalog_generation.load(Ordering::SeqCst);
        match event {
            CatalogEvent::Started { generation, total } => {
                if generation == cur_gen {
                    self.catalog_status = Some(format!("Validating notes… 0/{total}"));
                    self.set_default_status();
                }
            }
            CatalogEvent::Delta {
                generation,
                upserts,
                removed,
                folders,
                processed,
                total,
            } => {
                if generation == cur_gen {
                    self.catalog_status = Some(format!("Validating notes… {processed}/{total}"));

                    let mut data_changed = false;
                    if let Some(f) = folders
                        && self.catalog_folders != f
                    {
                        self.catalog_folders = f;
                        data_changed = true;
                    }

                    if !upserts.is_empty() || !removed.is_empty() {
                        data_changed = true;
                        let removed_set: HashSet<&str> =
                            removed.iter().map(|s| s.as_str()).collect();
                        self.notes.retain(|n| !removed_set.contains(n.id.as_str()));
                        for r in &removed {
                            self.note_stamps.remove(r);
                        }

                        let upsert_map: HashMap<String, (NoteSummary, crate::storage::FileStamp)> =
                            upserts
                                .into_iter()
                                .map(|(s, st)| (s.id.clone(), (s, st)))
                                .collect();

                        for (id, (summary, stamp)) in upsert_map {
                            if let Some(pos) = self.notes.iter().position(|n| n.id == id) {
                                self.notes[pos] = summary;
                            } else {
                                self.notes.push(summary);
                            }
                            self.note_stamps.insert(id, stamp);
                        }
                    }

                    if data_changed {
                        self.sort_notes();
                        self.refresh_visual_list();
                        self.refresh_subnotes_view_cache();
                        self.notes_revision += 1;
                        self.rebuild_note_index();
                    }

                    self.set_default_status();
                }
            }
            CatalogEvent::Finished {
                generation,
                complete,
                warnings,
            } => {
                if generation == cur_gen {
                    if complete {
                        self.initial_load_done = true;
                        self.catalog_status = None;
                        if self
                            .list
                            .sections
                            .contains(&crate::config::NotesSection::Graf)
                        {
                            self.ensure_graph_preview();
                        }
                        if !warnings.is_empty() {
                            self.set_temporary_status(&format!(
                                "Notes loaded with {} warning(s)",
                                warnings.len()
                            ));
                            let push_count = warnings.len().min(10);
                            for w in warnings.iter().take(push_count) {
                                self.messages.push(
                                    w.clone(),
                                    crate::app::messages::MessageSeverity::Warning,
                                );
                            }
                            if warnings.len() > 10 {
                                self.messages.push(
                                    format!("…and {} more scan warning(s)", warnings.len() - 10),
                                    crate::app::messages::MessageSeverity::Warning,
                                );
                            }
                        } else {
                            self.set_default_status();
                        }
                    } else {
                        self.initial_load_done = false;
                        self.catalog_status =
                            Some("Notes validation incomplete; Refresh to retry".to_string());
                        if let Some(w) = warnings.first() {
                            self.set_temporary_status(w);
                        } else {
                            self.set_default_status();
                        }
                    }
                }
            }
            CatalogEvent::Failed {
                generation,
                message,
            } => {
                if generation == cur_gen {
                    self.catalog_status = Some(format!("Notes validation failed: {message}"));
                    self.messages.push(
                        format!("Notes validation failed: {message}"),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                    self.set_default_status();
                }
            }
        }
    }

    pub fn refresh_subnotes_view_cache(&mut self) {
        self.subnotes_view_cache = self.storage.get_all_subnotes().unwrap_or_default();
        self.subnotes_view_cache_sig = self.notes.len() * 31
            + self
                .subnotes_view_cache
                .iter()
                .map(|(_, v)| v.len())
                .sum::<usize>();
    }

    pub fn refresh_note_single(&mut self, prev_id: Option<&str>, id: &str) {
        if let Some(old) = prev_id
            && old != id
        {
            self.notes.retain(|n| n.id != old);
            self.note_stamps.remove(old);
        }

        let note_path = self.storage.note_path(id);
        if let Ok(meta) = std::fs::metadata(&note_path) {
            let modified_nanos = meta.modified().ok().and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_nanos())
            });
            let stamp = crate::storage::FileStamp {
                modified_nanos,
                len: meta.len(),
            };
            let entry = crate::storage::NoteFileEntry {
                id: id.to_string(),
                stamp,
            };
            match self.storage.load_note_summary_from_entry(&entry) {
                Ok(summary) => {
                    self.notes.retain(|n| n.id != id);
                    self.notes.push(summary.clone());
                    self.note_stamps.insert(id.to_string(), stamp);
                    self.sort_notes();
                    self.notes_revision += 1;

                    let generation_num = self.catalog_generation.load(Ordering::SeqCst);
                    self.send_catalog_cmd(crate::app::catalog::CatalogCommand::PutKnown {
                        generation: generation_num,
                        summary,
                        stamp,
                        old_id: prev_id.map(|s| s.to_string()),
                    });
                }
                Err(e) => {
                    self.messages.push(
                        format!("Failed to read note '{id}': {e}. Removed from list."),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                    self.notes.retain(|n| n.id != id);
                    self.note_stamps.remove(id);
                    self.sort_notes();
                    self.notes_revision += 1;

                    let generation_num = self.catalog_generation.load(Ordering::SeqCst);
                    self.send_catalog_cmd(crate::app::catalog::CatalogCommand::RemoveKnown {
                        generation: generation_num,
                        id: id.to_string(),
                    });
                }
            }
        } else {
            self.notes.retain(|n| n.id != id);
            self.note_stamps.remove(id);
            self.sort_notes();
            self.notes_revision += 1;

            let generation_num = self.catalog_generation.load(Ordering::SeqCst);
            self.send_catalog_cmd(crate::app::catalog::CatalogCommand::RemoveKnown {
                generation: generation_num,
                id: id.to_string(),
            });
        }

        self.refresh_visual_list();
        self.refresh_subnotes_view_cache();
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
            Some(VisualItem::TemplateConfig { folder, .. }) => folder.clone(),
            Some(VisualItem::SmartFolder { .. }) => String::new(),
            Some(VisualItem::Subnote { .. }) => String::new(),
            None => String::new(),
        };

        let current = if Self::is_virtual_path(&current) {
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
        if !self.prepare_editor_transition() {
            return;
        }
        if note_id.ends_with(".base") {
            self.open_bases_view(note_id, None);
            return;
        }
        if note_id.ends_with(".draw") {
            self.open_draw_view();
            return;
        }
        if note_id.ends_with(".canvas") {
            self.open_canvas_view();
            return;
        }
        if note_id.ends_with(".clin") {
            self.status = Cow::Borrowed(
                "Legacy encrypted note: use the previous release to decrypt a safe copy.",
            );
            return;
        }
        let ext = std::path::Path::new(note_id)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if ext != "md" && ext != "txt" {
            let path = self.storage.note_path(note_id);
            match crate::ui::open_with_default_application(&path) {
                Ok(()) => {
                    self.status =
                        Cow::Owned(format!("Opened in default application: {}", path.display()))
                }
                Err(e) => self.set_temporary_status(&format!("Failed to open file: {e}")),
            }
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
            VisualItem::TemplateConfig { folder, .. } => {
                let folder = folder.clone();
                self.open_folder_template_config(&folder);
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
            VisualItem::SmartFolder { kind, .. } => {
                let p = kind.virtual_path();
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
            VisualItem::Subnote {
                parent_id,
                subnote_idx,
                ..
            } => {
                let pid = parent_id.clone();
                let idx = *subnote_idx;
                self.open_subnotes_popup_for(&pid, Some(idx));
            }
        }
    }

    pub fn open_public_document(&mut self, snapshot: crate::storage::documents::DocumentSnapshot) {
        if !self.prepare_editor_transition() {
            return;
        }
        self.editor.editing_id = Some(snapshot.id().to_owned());
        self.editor.template_edit_path = None;
        self.editor.title_editor = make_title_editor(
            snapshot.id(),
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.body = EditorDocument::from_source(snapshot.source());
        self.editor.public_document = Some(snapshot);
        self.editor.public_document_body_only = false;
        self.editor.public_document_title = None;
        self.editor.note_baseline = None;
        self.editor.bases_sidebar_renderer = None;
        if self.editor.sidebar == crate::editor::EditSidebar::Bases {
            self.editor.sidebar = crate::editor::EditSidebar::None;
        }
        self.editor.editor_preview_enabled = false;
        self.editor.md_preview_renderer = None;
        self.editor.link_preview_renderer = None;
        self.editor.link_preview_target = None;
        self.editor.link_preview_error = None;
        self.editor.link_preview = false;
        self.editor.links.clear();
        self.preview_fullscreen = false;
        self.rebuild_outline();
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::Edit;
        self.set_temporary_status_static(
            "Verbatim public source editor; filename unchanged. Return to Bases for live view.",
        );
    }

    pub fn load_and_open_note(&mut self, note_id: &str, line_number: Option<usize>) {
        if !self.prepare_editor_transition() {
            return;
        }
        if let Ok(note) = self.storage.load_note(note_id) {
            self.editor.bases_sidebar_renderer = None;
            if self.editor.sidebar == crate::editor::EditSidebar::Bases {
                self.editor.sidebar = crate::editor::EditSidebar::None;
            }
            self.editor.public_document = None;
            self.editor.public_document_body_only = false;
            self.editor.public_document_title = None;
            self.editor.template_edit_path = None;
            self.editor.note_baseline = note.baseline.clone();
            self.editor.editing_id = Some(note_id.to_string());
            self.editor.initial_word_count = crate::goals::count_words(&note.content);
            self.editor.title_editor = make_title_editor(
                &note.title,
                self.app_theme.highlight_fg,
                self.app_theme.highlight_bg,
            );
            let mut body = EditorDocument::from_source(&note.content);
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
            // Clone image infrastructure into the editor so markdown images
            // can be decoded and rendered in the preview pane.
            self.editor.image_picker = self.image_picker.clone();
            self.editor.image_decode_tx = self.image_decode_tx.clone();
            self.mode = ViewMode::Edit;

            if self.editor.editor_preview_enabled {
                self.update_editor_markdown_preview();
            } else {
                self.editor.md_preview_renderer = None;
            }
            self.status = Cow::Borrowed("");
        } else {
            self.status = Cow::Borrowed("Failed to load note!");
        }
    }

    pub fn open_note_in_external_editor(&mut self, note_id: &str, line_number: Option<usize>) {
        if let Ok(note) = self.storage.load_note(note_id) {
            let temp_dir = std::env::temp_dir().join("clin");
            let _ = std::fs::create_dir_all(&temp_dir);
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

            let mut args: Vec<String> = Vec::new();
            if let Some(l) = line_number {
                args.push(format!("+{l}"));
            }
            args.push(temp_file_path.to_string_lossy().into_owned());
            let (result, editor_prog) = self.run_in_external_editor(&args);

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

                            let updated_note = Note {
                                baseline: note.baseline.clone(),
                                title: note.title,
                                content: new_content,
                                updated_at: now_unix_secs(),
                                tags: note.tags,
                            };
                            if let Err(e) = self.storage.save_note(note_id, &updated_note) {
                                self.set_temporary_status(&format!("Failed to save note: {e}"));
                                self.messages.push(
                                    format!("Failed to save note: {e}"),
                                    crate::app::messages::MessageSeverity::Warning,
                                );
                            } else {
                                self.set_temporary_status_static("Note saved");
                                self.refresh_note_single(None, note_id);

                                let vault_identity =
                                    crate::local_state::vault_identity_path(&self.storage.data_dir)
                                        .map(|p| p.to_string_lossy().into_owned())
                                        .unwrap_or_else(|_| {
                                            self.storage.data_dir.to_string_lossy().into_owned()
                                        });
                                let progress = {
                                    let progress = self.get_current_goals_progress();
                                    progress.words_written += diff;
                                    progress.notes_modified.insert(crate::goals::TrackedNote {
                                        vault: vault_identity,
                                        note_id: note_id.to_string(),
                                    });
                                    progress.clone()
                                };
                                if let Err(error) = self.save_goals_progress(&progress) {
                                    self.set_temporary_status(&format!(
                                        "Failed to save local state: {error}"
                                    ));
                                }
                            }
                        } else {
                            self.set_temporary_status_static("No changes made in external editor.");
                        }
                    } else {
                        self.set_temporary_status_static("Failed to read from temp file.");
                        self.messages.push(
                            "Failed to read from temp file.".to_string(),
                            crate::app::messages::MessageSeverity::Warning,
                        );
                    }
                }
                Ok(status) => {
                    self.set_temporary_status(&format!(
                        "Editor '{editor_prog}' exited with status: {status}"
                    ));
                }
                Err(e) => {
                    self.set_temporary_status(&format!(
                        "Failed to launch editor '{editor_prog}': {e}"
                    ));
                }
            }
        } else {
            self.set_temporary_status_static("Failed to load note for external editor!");
            self.messages.push(
                "Failed to load note for external editor!".to_string(),
                crate::app::messages::MessageSeverity::Warning,
            );
        }
    }

    pub fn open_note_by_title(&mut self, title: &str) -> bool {
        let query = title.trim();
        if query.is_empty() {
            return false;
        }

        let found = self
            .visible_notes()
            .find(|(_, note)| note.title.eq_ignore_ascii_case(query))
            .map(|(i, _)| i);
        if let Some(index) = found
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
        if self.config.features.templates.is_enabled()
            && let Some(default_template) = self.storage.load_default_template()
        {
            self.start_note_from_template_with_title(&default_template, folder, title);
        } else {
            self.start_blank_note_with_title(folder, title);
        }
    }

    pub fn start_blank_note_with_title(&mut self, folder: String, title: String) {
        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() && !Self::is_virtual_path(&folder) {
            new_id = format!("{folder}/{new_id}");
        }

        self.enter_edit_mode(new_id, title, String::new());
    }

    fn enter_edit_mode(&mut self, id: String, title: String, content: String) {
        if !self.prepare_editor_transition() {
            return;
        }
        if self.editor.external_editor_enabled {
            let new_note = Note {
                baseline: None,
                title,
                content,
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            match self.storage.save_note(&id, &new_note) {
                Ok(saved_id) => {
                    self.refresh_note_single(None, &saved_id);
                    self.open_note_in_external_editor(&saved_id, None);
                }
                Err(e) => {
                    let text = format!("Failed to save new note '{}': {e}", new_note.title);
                    self.set_temporary_status(&text);
                    self.messages
                        .push(text, crate::app::messages::MessageSeverity::Warning);
                }
            }
            return;
        }

        self.mode = ViewMode::Edit;

        self.editor.public_document = None;
        self.editor.template_edit_path = None;
        self.editor.note_baseline =
            Some(crate::storage::documents::DocumentSnapshot::new(id.clone()));
        self.editor.editing_id = Some(id);
        self.editor.initial_word_count = crate::goals::count_words(&content);
        self.editor.title_editor = make_title_editor(
            &title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.body = EditorDocument::from_text(&content);
        self.editor.text_align = self.config.editor.text_align;
        self.apply_editor_prefs();
        self.set_default_status();
    }

    pub fn start_note_from_template(&mut self, template: &Template, folder: String) {
        let Some(rendered) = self.render_template_for_creation(template, &folder) else {
            return;
        };
        let note_title = rendered
            .title
            .clone()
            .unwrap_or_else(|| String::from("Untitled note"));
        let editor_title = rendered.title.unwrap_or_default();
        self.open_new_note_from_rendered(&folder, note_title, editor_title, rendered.content);
    }

    pub fn start_note_from_template_with_title(
        &mut self,
        template: &Template,
        folder: String,
        title: String,
    ) {
        let mut template = template.clone();
        template.title.template = None;
        let Some(rendered) = self.render_template_for_creation(&template, &folder) else {
            return;
        };
        self.open_new_note_from_rendered(&folder, title.clone(), title, rendered.content);
    }

    fn render_template_for_creation(
        &mut self,
        template: &Template,
        folder: &str,
    ) -> Option<crate::templates::RenderedTemplate> {
        let folder = if Self::is_virtual_path(folder) {
            ""
        } else {
            folder
        };
        match self.storage.render_template_in_folder(template, folder) {
            Ok(rendered) => {
                if let Some(warning) = rendered.warning() {
                    self.messages
                        .push(warning, crate::app::messages::MessageSeverity::Warning);
                }
                Some(rendered)
            }
            Err(error) => {
                let text = format!("Failed to render template: {error:#}");
                self.set_temporary_status(&text);
                self.messages
                    .push(text, crate::app::messages::MessageSeverity::Warning);
                None
            }
        }
    }

    fn open_new_note_from_rendered(
        &mut self,
        folder: &str,
        note_title: String,
        editor_title: String,
        content: String,
    ) {
        if !self.prepare_editor_transition() {
            return;
        }
        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() && !Self::is_virtual_path(folder) {
            new_id = format!("{folder}/{new_id}");
        }

        if self.editor.external_editor_enabled {
            let new_note = Note {
                baseline: None,
                title: note_title,
                content,
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            match self.storage.save_note(&new_id, &new_note) {
                Ok(saved_id) => {
                    self.refresh_note_single(None, &saved_id);
                    self.open_note_in_external_editor(&saved_id, None);
                }
                Err(e) => {
                    let text = format!("Failed to save new note '{}': {e}", new_note.title);
                    self.set_temporary_status(&text);
                    self.messages
                        .push(text, crate::app::messages::MessageSeverity::Warning);
                }
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.public_document = None;
        self.editor.template_edit_path = None;
        self.editor.note_baseline = Some(crate::storage::documents::DocumentSnapshot::new(
            new_id.clone(),
        ));
        self.editor.editing_id = Some(new_id);
        self.editor.initial_word_count = crate::goals::count_words(&content);
        self.editor.title_editor = make_title_editor(
            &editor_title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.body = EditorDocument::from_text(&content);
        self.apply_editor_prefs();

        self.set_default_status();
    }

    pub fn back_to_list(&mut self, prev_id: Option<&str>, new_id: Option<&str>) {
        if let Some(return_to) = self.return_mode.take() {
            if return_to == ViewMode::Edit
                && let Some(editor) = self.embedded_host_editor.take()
            {
                self.editor = *editor;
                self.mode = ViewMode::Edit;
                self.set_default_status();
                return;
            }
            if return_to == ViewMode::Bases
                && let Some(state) = &mut self.bases_state
                && let Some(request) = &state.embedded_request
                && request.host.as_deref() == self.editor.editing_id.as_deref()
                && matches!(
                    request.definition,
                    crate::bases::embedding::Definition::Inline { .. }
                )
                && let Some(snapshot) = &self.editor.public_document
            {
                match request.after_save(snapshot.source()) {
                    Ok(request) => {
                        if let crate::bases::embedding::Definition::Inline { source, .. } =
                            &request.definition
                        {
                            state.named_view = crate::bases::format::BaseDocument::parse(source)
                                .ok()
                                .and_then(|document| {
                                    document
                                        .config
                                        .views
                                        .get(state.loaded.as_ref().map_or(0, |loaded| loaded.view))
                                        .map(|view| view.name.clone())
                                });
                        }
                        state.embedded_request = Some(request);
                    }
                    Err(error) => {
                        state.error = Some(format!(
                            "Inline source anchor changed; reopen the containing preview: {error:#}"
                        ))
                    }
                }
            }
            self.editor.editing_id = None;
            self.editor.note_baseline = None;
            self.editor.public_document = None;
            if self
                .editor
                .template_edit_path
                .as_ref()
                .is_some_and(|path| !crate::templates::is_folder_template_config(path))
            {
                self.refresh_template_popup();
            }
            self.editor.template_edit_path = None;
            self.editor.title_editor =
                make_title_editor("", self.app_theme.highlight_fg, self.app_theme.highlight_bg);
            self.editor.body = EditorDocument::default();
            self.apply_editor_prefs();
            self.popups.confirm = None;
            self.editor.md_preview_renderer = None;
            if return_to == ViewMode::Graph && self.graph_plugin.is_none() {
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
                    Err(e) => {
                        self.set_temporary_status(&format!("Failed to rebuild graph: {e}"));
                        self.messages.push(
                            format!("Failed to rebuild graph: {e}"),
                            crate::app::messages::MessageSeverity::Warning,
                        );
                        self.mode = ViewMode::List;
                        return;
                    }
                }
            }
            self.mode = return_to;
            if return_to == ViewMode::Bases
                && let Some(mut state) = self.bases_state.take()
            {
                state.refresh(self, true);
                self.bases_state = Some(state);
            }
            self.set_default_status();

            return;
        }
        self.mode = ViewMode::List;
        self.editor.editing_id = None;
        self.editor.note_baseline = None;
        self.editor.public_document = None;
        if self
            .editor
            .template_edit_path
            .as_ref()
            .is_some_and(|path| !crate::templates::is_folder_template_config(path))
        {
            self.refresh_template_popup();
        }
        self.editor.template_edit_path = None;
        self.editor.title_editor =
            make_title_editor("", self.app_theme.highlight_fg, self.app_theme.highlight_bg);
        self.editor.body = EditorDocument::default();
        self.apply_editor_prefs();
        self.popups.confirm = None;
        self.editor.md_preview_renderer = None;
        if let Some(id) = new_id {
            self.refresh_note_single(prev_id, id);
        } else {
            self.request_notes_reconcile();
        }

        self.set_default_status();
    }

    pub fn begin_create_folder(&mut self) {
        let parent_path = if self.list.notes_layout == crate::config::NotesLayout::Grid {
            self.list.grid_folder.clone()
        } else {
            self.get_current_folder_context()
        };
        if Self::is_virtual_path(&parent_path) {
            self.set_temporary_status_static("Cannot create folder inside a virtual folder");
            return;
        }
        let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
        let title = if parent_path.is_empty() {
            "Create Folder - Esc to cancel, Enter to save".to_string()
        } else {
            format!("Create Folder in '{parent_path}' - Esc to cancel, Enter to save")
        };
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
            if Self::is_virtual_path(path) {
                self.set_temporary_status_static("Cannot rename virtual folder");
                return;
            }
            let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
            input.insert_str(path);
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
        match open_in_file_manager(&self.storage.notes_dir) {
            Ok(()) => self.set_temporary_status_static("Opened vault location"),
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
            self.set_temporary_status_static(
                "Legacy encrypted notes require the previous release; data retained.",
            );
            return;
        }
        let ext = std::path::Path::new(note_id)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if ext != "md" && ext != "txt" {
            let path = self.storage.note_path(note_id);
            match crate::ui::open_with_default_application(&path) {
                Ok(()) => {
                    self.status =
                        Cow::Owned(format!("Opened in default application: {}", path.display()))
                }
                Err(e) => self.set_temporary_status(&format!("Failed to open file: {e}")),
            }
            return;
        }

        self.return_mode = Some(ViewMode::Graph);
        if self.editor.external_editor_enabled {
            self.open_note_in_external_editor(note_id, None);
            // graph_state was destroyed when note was opened; rebuild it
            self.return_mode.take(); // discard return_mode (was set to Graph above)
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
                        self.set_temporary_status_static("Failed to rebuild graph view");
                        self.messages.push(
                            "Failed to rebuild graph view".to_string(),
                            crate::app::messages::MessageSeverity::Warning,
                        );
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
            if Self::is_virtual_path(&self.list.grid_folder) {
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
        self.popups.active = Some(crate::popups::ActivePopup::CreateFormat(
            CreateFormatPopup {
                folder,
                selected: 0,
            },
        ));
    }
    pub fn confirm_create_format(&mut self) {
        if let Some(crate::popups::ActivePopup::CreateFormat(popup)) = self.popups.active.take() {
            match popup.selected {
                0 => self.begin_create_note_in_folder(popup.folder), // .md
                1 => self.begin_create_text_in_folder(popup.folder), // .txt (new)
                2 => self.begin_create_draw_in_folder(popup.folder), // .draw
                3 => self.begin_create_canvas_in_folder(popup.folder), // .canvas
                4 => self.begin_create_base_in_folder(popup.folder),
                _ => {}
            }
        }
    }

    pub fn begin_create_text_in_folder(&mut self, folder: String) {
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
                .title("New Text File Name - Esc to cancel, Enter to create"),
        );
        self.popups.active = Some(crate::popups::ActivePopup::CreateNote(
            NoteCreatePopup { folder, input },
            NoteFormat::PlainText,
        ));
    }

    pub fn begin_create_note_in_folder(&mut self, folder: String) {
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
                .title("New Note Name - Esc to cancel, Enter to create"),
        );
        self.popups.active = Some(crate::popups::ActivePopup::CreateNote(
            NoteCreatePopup { folder, input },
            crate::popups::NoteFormat::Markdown,
        ));
    }

    pub fn confirm_create_note(&mut self) {
        if let Some(crate::popups::ActivePopup::CreateNote(popup, format)) =
            self.popups.active.take()
        {
            if let Err(error) =
                self.apply_note_creation(popup.folder, format, &popup.input.lines().join(""))
            {
                self.set_temporary_status(&format!("Could not create document: {error:#}"));
            }
        }
    }

    pub(crate) fn apply_note_creation(
        &mut self,
        folder: String,
        format: crate::popups::NoteFormat,
        input: &str,
    ) -> Result<()> {
        let mut title = input.to_owned();
        title = title.trim().to_string();
        match format {
            crate::popups::NoteFormat::Base => self.create_base(folder, title),
            crate::popups::NoteFormat::Markdown => {
                if title.is_empty() {
                    title = String::from("Untitled note");
                }
                self.start_new_note_with_title(folder, title);
            }
            crate::popups::NoteFormat::PlainText => {
                if title.is_empty() {
                    title = String::from("Untitled text");
                }
                let mut id = self.storage.new_note_id();
                id.push_str(".txt");
                let full_id = if folder.is_empty() {
                    id
                } else {
                    format!("{}/{}", folder, id)
                };

                self.enter_edit_mode(full_id, title, String::new());
            }
            format @ (crate::popups::NoteFormat::Draw | crate::popups::NoteFormat::Canvas) => {
                self.create_graphic_document(&folder, format, &title)?;
            }
        }
        Ok(())
    }

    pub(crate) fn action_document_id(
        &self,
        folder: &str,
        format: crate::popups::NoteFormat,
        title: &str,
    ) -> Result<String> {
        let (extension, default) = match format {
            crate::popups::NoteFormat::Draw => ("draw", "Untitled drawing"),
            crate::popups::NoteFormat::Canvas => ("canvas", "Untitled canvas"),
            crate::popups::NoteFormat::Base => ("base", "Untitled"),
            _ => anyhow::bail!("Unsupported creation format"),
        };
        let title = if title.trim().is_empty() {
            default
        } else {
            title.trim()
        };
        anyhow::ensure!(!title.contains(['\n', '\r', '\0']), "Invalid document name");
        let name = if title.ends_with(&format!(".{extension}")) {
            title.to_owned()
        } else {
            format!("{title}.{extension}")
        };
        let id = if folder.is_empty() {
            name
        } else {
            format!("{folder}/{name}")
        };
        self.storage.document_path(&id)?;
        Ok(id)
    }

    pub(crate) fn validate_document_creation(
        &self,
        folder: &str,
        format: crate::popups::NoteFormat,
        title: &str,
    ) -> Result<()> {
        let id = self.action_document_id(folder, format, title)?;
        anyhow::ensure!(
            !self.storage.document_path(&id)?.exists(),
            "Document already exists: {id}"
        );
        Ok(())
    }

    fn create_graphic_document(
        &mut self,
        folder: &str,
        format: crate::popups::NoteFormat,
        title: &str,
    ) -> Result<()> {
        let id = self.action_document_id(folder, format, title)?;
        self.save_before_session_replace()?;
        let source = if format == crate::popups::NoteFormat::Draw {
            serde_json::to_string(&crate::draw::state::DrawData::default())?
        } else {
            serde_json::to_string_pretty(&pinstar::data::CanvasData {
                nodes: vec![],
                edges: vec![],
                orientation: pinstar::data::DiagramOrientation::default(),
            })?
        };
        let snapshot = self.storage.create_document(&id, &source)?;
        if format == crate::popups::NoteFormat::Draw {
            self.draw_state = Some(crate::draw::app::DrawAppState::new(
                self.storage.clone(),
                Some(id.clone()),
                self.app_theme.clone(),
                self.keybinds.clone(),
                self.seq_matcher.clone(),
            ));
        } else {
            self.canvas_state = Some(crate::pinstar_adapter::PinstarPlugin::new(
                &self.storage.document_path(&id)?,
                &self.config,
                self.keybinds.clone(),
                self.seq_matcher.clone(),
                self.image_picker.clone(),
                &self.storage.data_dir,
            )?);
        }
        self.return_mode = Some(self.mode);
        self.mode = if format == crate::popups::NoteFormat::Draw {
            ViewMode::Draw
        } else {
            ViewMode::Canvas
        };
        self.editor.note_baseline = Some(snapshot);
        self.editor.editing_id = Some(id);
        self.set_default_status();
        Ok(())
    }

    pub fn insert_content(
        &mut self,
        target: ImportTarget,
        note_id: Option<&str>,
        title: String,
        content: String,
    ) -> Result<()> {
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
                self.refresh_note_single(None, id);
                self.set_temporary_status_static("Content appended");
            }
        }
        Ok(())
    }

    pub fn start_note_with_content(&mut self, folder: String, title: String, content: String) {
        if !self.prepare_editor_transition() {
            return;
        }
        let folder = if Self::is_virtual_path(&folder) {
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
                baseline: None,
                title,
                content,
                updated_at: now_unix_secs(),
                tags: vec![],
            };
            match self.storage.save_note(&new_id, &note) {
                Ok(saved_id) => {
                    self.refresh_note_single(None, &saved_id);
                    self.open_note_in_external_editor(&saved_id, None);
                }
                Err(e) => {
                    let text = format!("Failed to save new note '{}': {e}", note.title);
                    self.set_temporary_status(&text);
                    self.messages
                        .push(text, crate::app::messages::MessageSeverity::Warning);
                }
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.public_document = None;
        self.editor.template_edit_path = None;
        self.editor.note_baseline = Some(crate::storage::documents::DocumentSnapshot::new(
            new_id.clone(),
        ));
        self.editor.editing_id = Some(new_id);
        self.editor.initial_word_count = crate::goals::count_words(&content);
        self.editor.title_editor = make_title_editor(
            &title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.body = EditorDocument::from_text(&content);
        self.apply_editor_prefs();
        self.set_default_status();
    }
    pub fn begin_rename_note(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let id = self.notes[*summary_idx].id.clone();
            let note = &self.notes[*summary_idx];
            let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
            input.insert_str(&note.title);
            input.set_block(
                ratatui::widgets::Block::default()
                    .style(self.app_theme.bg_style())
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Rename file: links are NOT updated - Esc cancel, Enter confirm"),
            );
            self.popups.active = Some(crate::popups::ActivePopup::NoteRename(NoteRenamePopup {
                note_id: id,
                input,
            }));
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
                    self.request_notes_reconcile();
                    self.set_temporary_status_static("Note renamed");
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to rename: {e}"));
                    self.messages.push(
                        format!("Failed to rename: {e}"),
                        crate::app::messages::MessageSeverity::Warning,
                    );
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
    pub fn open_subnotes_popup_for(&mut self, parent_id: &str, preselect: Option<usize>) {
        let subnotes = self.storage.get_subnotes(parent_id).unwrap_or_default();

        let mut title_input = crate::ui::make_popup_textarea(&self.app_theme, "");
        let mut content_input = crate::ui::make_popup_textarea(&self.app_theme, "");

        let selected = preselect.unwrap_or(0).min(subnotes.len().saturating_sub(1));
        if !subnotes.is_empty() && selected < subnotes.len() {
            title_input.insert_str(&subnotes[selected].title);
            content_input.insert_str(&subnotes[selected].content);
        }

        let popup = crate::popups::SubnotesPopup {
            parent_id: parent_id.to_string(),
            subnotes,
            selected,
            focus: crate::popups::SubnotesFocus::List,
            scroll_offset: 0,
            title_input,
            content_input,
            is_dirty: false,
            last_scroll: None,
        };

        self.present_action_popup(crate::popups::ActivePopup::Subnotes(Box::new(popup)));
    }

    pub fn open_subnotes_popup(&mut self) {
        if self.feature_disabled(
            self.config.features.subnotes.is_enabled(),
            "Subnotes",
            "subnotes",
        ) {
            return;
        }
        let parent_id = match self.get_selected_note_id() {
            Some(id) => id,
            None => {
                self.set_temporary_status_static("No note selected");
                return;
            }
        };
        self.open_subnotes_popup_for(&parent_id, None);
    }

    pub fn close_subnotes_popup(&mut self) -> Result<(), String> {
        if let Some(crate::popups::ActivePopup::Subnotes(popup)) = self.popups.active.take() {
            if let Err(error) = self.save_subnotes_state(&popup) {
                self.popups.active = Some(crate::popups::ActivePopup::Subnotes(popup));
                let message = format!("Failed to save sub-notes: {error:#}");
                self.set_temporary_status(&message);
                return Err(message);
            }
        }
        self.popups.active = None;
        Ok(())
    }
    pub(crate) fn save_subnotes_state(
        &mut self,
        popup: &crate::popups::SubnotesPopup,
    ) -> Result<()> {
        if popup.is_dirty {
            self.storage
                .set_subnotes(&popup.parent_id, &popup.subnotes)?;
        }
        self.notes_with_subnotes = self.storage.get_notes_with_subnotes()?;
        self.refresh_subnotes_view_cache();
        Ok(())
    }

    pub fn open_subnote_in_external_editor(&mut self) {
        let mut popup = match self.popups.active.take() {
            Some(crate::popups::ActivePopup::Subnotes(popup)) => popup,
            other => {
                self.popups.active = other;
                return;
            }
        };

        self.edit_subnote_external(&mut popup);
        self.popups.active = Some(crate::popups::ActivePopup::Subnotes(popup));
    }

    pub(crate) fn edit_subnote_external(&mut self, popup: &mut crate::popups::SubnotesPopup) {
        if popup.subnotes.is_empty() || popup.selected >= popup.subnotes.len() {
            return;
        }

        // Sync TUI inputs to subnote struct first
        let cur_idx = popup.selected;
        let new_title = popup.title_input.lines().join("");
        let new_content_tui = popup.content_input.lines().join("\n");
        if popup.subnotes[cur_idx].title != new_title
            || popup.subnotes[cur_idx].content != new_content_tui
        {
            popup.subnotes[cur_idx].title = new_title;
            popup.subnotes[cur_idx].content = new_content_tui;
            popup.subnotes[cur_idx].updated_at = now_unix_secs();
            popup.is_dirty = true;
        }

        let subnote = &popup.subnotes[cur_idx];
        let temp_dir = std::env::temp_dir().join("clin");
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_id = uuid::Uuid::new_v4().to_string();
        let temp_file_path = temp_dir.join(format!("clin_subnote_{temp_id}.md"));

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
                    if let Err(e) = f.write_all(subnote.content.as_bytes()) {
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
                    if let Err(e) = f.write_all(subnote.content.as_bytes()) {
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

        let _secret = SecretTempFile::new(temp_file_path.clone());

        let args: Vec<String> = vec![temp_file_path.to_string_lossy().into_owned()];
        let (result, editor_prog) = self.run_in_external_editor(&args);

        match result {
            Ok(status) if status.success() => {
                if let Ok(new_content) = std::fs::read_to_string(&temp_file_path) {
                    if new_content != subnote.content {
                        popup.subnotes[cur_idx].content = new_content.clone();
                        popup.subnotes[cur_idx].updated_at = now_unix_secs();
                        popup.is_dirty = true;

                        popup.content_input = crate::ui::make_popup_textarea(&self.app_theme, "");
                        popup.content_input.insert_str(&new_content);

                        self.set_temporary_status_static("Sub-note saved");
                    } else {
                        self.set_temporary_status_static("No changes made in external editor.");
                    }
                } else {
                    self.set_temporary_status_static("Failed to read from temp file.");
                    self.messages.push(
                        "Failed to read from temp file.".to_string(),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                }
            }
            Ok(status) => {
                self.set_temporary_status(&format!(
                    "Editor '{editor_prog}' exited with status: {status}"
                ));
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to launch editor '{editor_prog}': {e}"));
            }
        }
    }
}
