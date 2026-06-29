use super::*;
use crate::list_view::*;
use crate::storage::NoteSummary;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl App {
    /// Spawns a background thread that streams note summaries in batches.
    /// Caller must drain the receiver in the main loop via merge_loaded.
    pub fn start_background_load(&self) -> mpsc::Receiver<LoadBatch> {
        let (tx, rx) = mpsc::channel();
        let storage = self.storage.clone();
        let show_hidden = self.list.show_hidden_files;
        let cancel = Arc::clone(&self.load_cancel);
        std::thread::spawn(move || {
            let ids = match storage.list_note_ids(show_hidden) {
                Ok(ids) => ids,
                Err(_) => {
                    let _ = tx.send(LoadBatch::Done(0));
                    return;
                }
            };
            let total = ids.len();
            if tx.send(LoadBatch::Started(total)).is_err() {
                return;
            }

            // Reset cancel flag at start of new load
            cancel.store(false, Ordering::Release);

            let mut loaded = 0usize;
            let mut batch: Vec<(String, NoteSummary, u64)> = Vec::new();
            let batch_size = 250usize;

            for id in &ids {
                if cancel.load(Ordering::Acquire) {
                    let _ = tx.send(LoadBatch::Done(loaded));
                    return;
                }
                let mt = storage.note_mtime_millis(id);
                if let Ok(summary) = storage.load_note_summary(id) {
                    batch.push((id.clone(), summary, mt));
                    loaded += 1;
                    if batch.len() >= batch_size || loaded == total {
                        let to_send = std::mem::take(&mut batch);
                        if tx.send(LoadBatch::Items(to_send)).is_err() {
                            return;
                        }
                    }
                }
            }
            if !batch.is_empty() {
                let _ = tx.send(LoadBatch::Items(batch));
            }
            let _ = tx.send(LoadBatch::Done(loaded));
        });

        rx
    }

    /// Merge a batch from the background loader into the app state.
    /// Returns `true` if the UI should redraw.
    pub fn merge_loaded(&mut self, batch: LoadBatch) -> bool {
        match batch {
            LoadBatch::Started(total) => {
                self.loading_total = total;
                self.status = Cow::Owned(format!("Loading notes\u{2026} 0/{total}"));
                true
            }
            LoadBatch::Items(items) => {
                for (id, summary, mtime) in items {
                    self.summary_cache.insert(id.clone(), summary.clone());
                    self.summary_mtime.entry(id.clone()).or_insert(mtime);
                    self.notes.push(summary);
                }
                self.sort_notes();
                self.refresh_visual_list();
                let total = self.loading_total.max(self.notes.len());
                self.status = Cow::Owned(format!(
                    "Loading notes\u{2026} {}/{}",
                    self.notes.len(),
                    total
                ));
                true
            }
            LoadBatch::Done(_) => {
                self.initial_load_done = true;
                self.loading_total = 0;
                self.status = Cow::Borrowed("");
                // Pre-warm graph preview so the first render doesn't block
                if self
                    .list
                    .sections
                    .contains(&crate::config::NotesSection::Graf)
                {
                    self.ensure_graph_preview();
                }
                
                true
            }
        }
    }

    pub fn refresh_visual_list(&mut self) {
        let mut visual = Vec::new();

        let mut by_folder: HashMap<&str, Vec<(usize, &NoteSummary)>> = HashMap::new();
        let mut pinned_notes: Vec<(usize, &NoteSummary)> = Vec::new();
        for (i, note) in self.notes.iter().enumerate() {
            by_folder
                .entry(note.folder.as_str())
                .or_default()
                .push((i, note));
            if note.pinned {
                pinned_notes.push((i, note));
            }
        }

        visual.push(VisualItem::Folder {
            path: VIRTUAL_PINNED_PATH.to_string(),
            name: VIRTUAL_PINNED_LABEL.to_string(),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(VIRTUAL_PINNED_PATH),
            note_count: pinned_notes.len(),
        });

        if self.list.folder_expanded.contains(VIRTUAL_PINNED_PATH) {
            for (idx, note) in &pinned_notes {
                visual.push(VisualItem::Note {
                    summary_idx: *idx,
                    depth: 1,
                    is_clin: note.id.ends_with(".clin"),
                    is_draw: note.id.ends_with(".draw"),
                    is_canvas: note.id.ends_with(".canvas"),
                    in_virtual_pinned_folder: true,
                });
            }
        }

        visual.push(VisualItem::Folder {
            path: String::new(),
            name: String::from("Vault"),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(""),
            note_count: by_folder.get("").map_or(0, |v| v.len()),
        });

        if self.list.folder_expanded.contains("") {
            if let Some(notes) = by_folder.get("") {
                for (idx, note) in notes {
                    visual.push(VisualItem::Note {
                        summary_idx: *idx,
                        depth: 1,
                        is_clin: note.id.ends_with(".clin"),
                        is_draw: note.id.ends_with(".draw"),
                        is_canvas: note.id.ends_with(".canvas"),
                        in_virtual_pinned_folder: false,
                    });
                }
            }
            visual.push(VisualItem::CreateNew {
                path: String::new(),
                depth: 1,
            });
        }

        let all_folders = if let Some(ref cache) = self.list.folder_cache {
            cache
        } else {
            let folders = self
                .storage
                .list_folders(self.list.show_hidden_files)
                .unwrap_or_default();
            self.list.folder_cache = Some(folders);
            self.list
                .folder_cache
                .as_ref()
                .expect("folder_cache populated above")
        };

        if self.list.notes_layout == crate::config::NotesLayout::Grid {
            // Discard the tree-view items (Pinned/Vault folders) built above.
            visual.clear();
            let gf = &self.list.grid_folder;
            if gf == VIRTUAL_PINNED_PATH {
                // Pinned tab: show only pinned notes, no folders, no CreateNew, no ".."
                for (idx, note) in &pinned_notes {
                    visual.push(VisualItem::Note {
                        summary_idx: *idx,
                        depth: 0,
                        is_clin: note.id.ends_with(".clin"),
                        is_draw: note.id.ends_with(".draw"),
                        is_canvas: note.id.ends_with(".canvas"),
                        in_virtual_pinned_folder: true,
                    });
                }
            } else {
                // Vault tab or a subfolder: show only the contents of this folder.
                // ".." only appears when inside a subfolder (not at Vault root "").
                if !gf.is_empty() {
                    let parent_path = if let Some(slash) = gf.rfind('/') {
                        &gf[..slash]
                    } else {
                        ""
                    };
                    visual.push(VisualItem::Folder {
                        path: parent_path.to_string(),
                        name: "..".to_string(),
                        depth: 0,
                        is_expanded: false,
                        note_count: 0,
                    });
                }

                // Direct subfolders of the current folder
                for folder in all_folders {
                    let parent_path = if let Some(slash) = folder.rfind('/') {
                        &folder[..slash]
                    } else {
                        ""
                    };
                    if parent_path == gf {
                        let parts: Vec<&str> = folder.split('/').collect();
                        let name = parts.last().unwrap_or(&"").to_string();
                        visual.push(VisualItem::Folder {
                            path: folder.clone(),
                            name,
                            depth: 0,
                            is_expanded: false,
                            note_count: by_folder.get(folder.as_str()).map_or(0, |v| v.len()),
                        });
                    }
                }

                // Direct notes of the current folder
                if let Some(notes) = by_folder.get(gf.as_str()) {
                    for (idx, note) in notes {
                        visual.push(VisualItem::Note {
                            summary_idx: *idx,
                            depth: 0,
                            is_clin: note.id.ends_with(".clin"),
                            is_draw: note.id.ends_with(".draw"),
                            is_canvas: note.id.ends_with(".canvas"),
                            in_virtual_pinned_folder: false,
                        });
                    }
                }
                visual.push(VisualItem::CreateNew {
                    path: gf.clone(),
                    depth: 0,
                });
            }

            self.list.visual_list = visual;
            self.build_display_lines();
            self.request_preview_update_immediate();
            return;
        }

        for folder in all_folders {
            let parts: Vec<&str> = folder.split('/').collect();
            let depth = parts.len();
            let name = parts.last().unwrap_or(&"").to_string();

            let parent_path = if let Some(slash) = folder.rfind('/') {
                &folder[..slash]
            } else {
                ""
            };

            let mut is_visible = true;
            let mut current_parent = parent_path;
            while !current_parent.is_empty() {
                if !self.list.folder_expanded.contains(current_parent) {
                    is_visible = false;
                    break;
                }
                if let Some(slash) = current_parent.rfind('/') {
                    current_parent = &current_parent[..slash];
                } else {
                    current_parent = "";
                }
            }

            if !self.list.folder_expanded.contains("") {
                is_visible = false;
            }

            if is_visible {
                let is_expanded = self.list.folder_expanded.contains(folder.as_str());
                visual.push(VisualItem::Folder {
                    path: folder.clone(),
                    name,
                    depth,
                    is_expanded,
                    note_count: by_folder.get(folder.as_str()).map_or(0, |v| v.len()),
                });

                if is_expanded {
                    if let Some(notes) = by_folder.get(folder.as_str()) {
                        for (idx, note) in notes {
                            visual.push(VisualItem::Note {
                                summary_idx: *idx,
                                depth: depth + 1,
                                is_clin: note.id.ends_with(".clin"),
                                is_draw: note.id.ends_with(".draw"),
                                is_canvas: note.id.ends_with(".canvas"),
                                in_virtual_pinned_folder: false,
                            });
                        }
                    }
                    visual.push(VisualItem::CreateNew {
                        path: folder.clone(),
                        depth: depth + 1,
                    });
                }
            }
        }

        self.list.visual_list = visual;
        self.build_display_lines();
        self.request_preview_update_immediate();
    }

    pub fn poll_renderers(&mut self) -> bool {
        let mut updated = false;

        if let Some(last) = self.editor.last_editor_change
            && last.elapsed() > Duration::from_millis(150)
            && self.editor.pending_editor_preview_update
        {
            self.update_editor_markdown_preview();
            self.editor.pending_editor_preview_update = false;
            self.editor.last_editor_change = None;
            updated = true;
        }

        let list_active = self.list.preview_enabled || self.preview_fullscreen;
        if list_active && self.list.preview_content_width != Some(self.desired_list_preview_width())
        {
            self.update_preview();
            updated = true;
        }
        let edit_active = self.editor.editor_preview_enabled || self.preview_fullscreen;
        if edit_active
            && self.editor.preview_content_width != Some(self.desired_editor_preview_width())
        {
            self.update_editor_markdown_preview();
            updated = true;
        }

        if let Some(PreviewContent::Markdown(renderer)) = &mut self.list.preview_content
            && renderer.poll()
        {
            if !renderer.pages_built() {
                let visible = self.list.last_preview_pane_height.saturating_sub(2).max(10);
                renderer.build_pages(visible, self.app_theme.preview_bg());
            }
            updated = true;
        }
        if let Some(renderer) = &mut self.editor.md_preview_renderer
            && renderer.poll()
        {
            if !renderer.pages_built() {
                let visible = self
                    .editor
                    .last_preview_pane_height
                    .saturating_sub(2)
                    .max(10);
                renderer.build_pages(visible, self.app_theme.preview_bg());
            }
            updated = true;
        }
        updated
    }

    pub fn request_preview_update(&mut self) {
        if !(self.list.preview_enabled || self.preview_fullscreen) {
            return;
        }

        self.update_preview();
        self.list.pending_preview_update = false;
        self.list.last_selection_change = None;
    }

    pub fn request_preview_update_immediate(&mut self) {
        if !(self.list.preview_enabled || self.preview_fullscreen) {
            return;
        }

        self.update_preview();
        self.list.pending_preview_update = false;
        self.list.last_selection_change = None;
    }

    pub fn request_editor_preview_update(&mut self) {
        if !(self.editor.editor_preview_enabled || self.preview_fullscreen) {
            return;
        }
        self.editor.last_editor_change = Some(Instant::now());
        self.editor.pending_editor_preview_update = true;
    }

    pub fn update_preview(&mut self) {
        if !(self.list.preview_enabled || self.preview_fullscreen) {
            return;
        }

        let item = self.list.visual_list.get(self.list.visual_index);
        match item {
            Some(VisualItem::Note {
                summary_idx,
                is_draw,
                is_canvas,
                ..
            }) => {
                let summary_idx = *summary_idx;
                let is_draw = *is_draw;
                let is_canvas = *is_canvas;
                let id = &self.notes[summary_idx].id;
                let is_clin = id.ends_with(".clin");
                
                if self.preview_encryption && is_clin {
                    self.list.preview_content = None;
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }

                if is_draw {
                    let path = self.storage.note_path(id);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<crate::draw::state::DrawData>(&content) {
                                Ok(data) => {
                                    let grid = crate::snapshot::render_draw_snapshot(
                                        &data,
                                        &self.app_theme,
                                    );
                                    self.list.preview_content =
                                        Some(PreviewContent::DrawGrid(grid));
                                }
                                Err(e) => {
                                    self.list.preview_content = None;
                                    self.status = Cow::Owned(format!("Failed to parse draw: {e}"));
                                }
                            }
                        }
                        Err(_) => {
                            self.list.preview_content = None;
                        }
                    }
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }

                if is_canvas {
                    let path = self.storage.note_path(id);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<crate::pinstar::data::CanvasData>(&content)
                            {
                                Ok(data) => {
                                    let grid = crate::snapshot::render_canvas_snapshot(
                                        &data,
                                        &self.app_theme,
                                    );
                                    self.list.preview_content =
                                        Some(PreviewContent::CanvasGrid(grid));
                                }
                                Err(e) => {
                                    self.list.preview_content = None;
                                    self.status =
                                        Cow::Owned(format!("Failed to parse canvas: {e}"));
                                }
                            }
                        }
                        Err(_) => {
                            self.list.preview_content = None;
                        }
                    }
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }

                if let Ok(note) = self.storage.load_note(id) {
                    let width = self.desired_list_preview_width();
                    let mut renderer = MarkdownRenderer::new(width);
                    renderer.render_with(
                        &note.content,
                        width,
                        &self.app_theme,
                        self.config.core.syntax_highlighting,
                        self.config.core.preview_wrap,
                        self.config.ui.icon_mode,
                    );
                    self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                    self.list.preview_content_width = Some(width);
                } else {
                    self.list.preview_content = None;
                }
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            Some(VisualItem::Folder { path, name, .. }) => {
                let folder_path = path.clone();
                let is_pinned = folder_path == crate::app::VIRTUAL_PINNED_PATH;

                let all_folders = if let Some(ref cache) = self.list.folder_cache {
                    cache.clone()
                } else {
                    let folders = self
                        .storage
                        .list_folders(self.list.show_hidden_files)
                        .unwrap_or_default();
                    self.list.folder_cache = Some(folders.clone());
                    folders
                };

                let mut subfolders = Vec::new();
                if !is_pinned {
                    for f in &all_folders {
                        let parent_path = if let Some(slash) = f.rfind('/') {
                            &f[..slash]
                        } else {
                            ""
                        };
                        if parent_path == folder_path {
                            let name = f.split('/').next_back().unwrap_or("").to_string();
                            subfolders.push(name);
                        }
                    }
                    subfolders.sort();
                }

                let mut notes = Vec::new();
                for note in &self.notes {
                    let matches = if is_pinned {
                        note.pinned
                    } else {
                        note.folder == folder_path
                    };
                    if matches {
                        notes.push(note.title.clone());
                    }
                }
                notes.sort();

                let display_title = if is_pinned {
                    "Pinned Notes".to_string()
                } else if name == ".." {
                    format!(
                        "Parent: {}",
                        if folder_path.is_empty() {
                            "Vault"
                        } else {
                            &folder_path
                        }
                    )
                } else if folder_path.is_empty() {
                    "Vault (Root)".to_string()
                } else {
                    name.clone()
                };

                let mut md = format!("# {display_title}\n\n");

                if !subfolders.is_empty() {
                    md.push_str("## Folders\n");
                    for sub in &subfolders {
                        md.push_str(&format!("- \u{f07b} {sub}\n"));
                    }
                    md.push('\n');
                }

                if !notes.is_empty() {
                    md.push_str("## Notes\n");
                    for note in &notes {
                        md.push_str(&format!("- \u{f15c} {note}\n"));
                    }
                } else if subfolders.is_empty() {
                    md.push_str("*This folder is empty.*\n");
                }

                let width = self.desired_list_preview_width();
                let mut renderer = MarkdownRenderer::new(width);
                renderer.render_with(
                    &md,
                    width,
                    &self.app_theme,
                    self.config.core.syntax_highlighting,
                    self.config.core.preview_wrap,
                    self.config.ui.icon_mode,
                );
                self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                self.list.preview_content_width = Some(width);
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            _ => {
                self.list.preview_content = None;
                self.list.preview_content_index = None;
            }
        }
    }

    pub fn update_editor_markdown_preview(&mut self) {
        if !(self.editor.editor_preview_enabled || self.preview_fullscreen) {
            return;
        }

        let content = self.editor.editor.lines().join("\n");
        let width = self.desired_editor_preview_width();
        let mut renderer = MarkdownRenderer::new(width);
        renderer.render_with(
            &content,
            width,
            &self.app_theme,
            self.config.core.syntax_highlighting,
            self.config.core.preview_wrap,
            self.config.ui.icon_mode,
        );
        self.editor.md_preview_renderer = Some(renderer);
        self.editor.preview_content_width = Some(width);
    }
}
