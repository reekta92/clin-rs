use super::*;
use crate::list_view::*;
use crate::storage::NoteSummary;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct SmartFolderData {
    kind: SmartFolderKind,
    label: String,
    matches: Vec<usize>,
}
impl App {
    /// Spawns a background thread that streams note summaries in batches.
    /// Caller must drain the receiver in the main loop via merge_loaded.
    pub fn start_background_load(&self) -> mpsc::Receiver<LoadBatch> {
        let (tx, rx) = mpsc::channel();
        let storage = self.storage.clone();
        let show_hidden = self.list.show_hidden_files;
        let show_all = self.list.show_all_files;
        let cancel = Arc::clone(&self.load_cancel);
        std::thread::spawn(move || {
            let ids = match storage.list_note_ids(show_hidden, show_all) {
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

        let all_folders = if let Some(cache) = &self.list.folder_cache {
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

        // Build subfolders map: group each folder by parent path for recursive traversal
        let mut subfolders_map: std::collections::HashMap<&str, Vec<&String>> =
            std::collections::HashMap::new();
        for folder in all_folders {
            let parent = if let Some(slash) = folder.rfind('/') {
                &folder[..slash]
            } else {
                ""
            };
            subfolders_map.entry(parent).or_default().push(folder);
        }

        let mut recursive_count = std::collections::HashMap::new();

        fn compute_subtree<'a>(
            folder: &'a str,
            subfolders_map: &std::collections::HashMap<&'a str, Vec<&'a String>>,
            by_folder: &std::collections::HashMap<&'a str, Vec<(usize, &'a NoteSummary)>>,
            recursive_count: &mut std::collections::HashMap<&'a str, usize>,
        ) -> usize {
            let direct_count = by_folder.get(folder).map_or(0, |v| v.len());
            let mut total_count = direct_count;

            if let Some(children) = subfolders_map.get(folder) {
                for child in children {
                    total_count +=
                        compute_subtree(child.as_str(), subfolders_map, by_folder, recursive_count);
                }
            }

            recursive_count.insert(folder, total_count);
            total_count
        }

        compute_subtree("", &subfolders_map, &by_folder, &mut recursive_count);

        visual.push(VisualItem::Folder {
            path: VIRTUAL_PINNED_PATH.to_string(),
            name: VIRTUAL_PINNED_LABEL.to_string(),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(VIRTUAL_PINNED_PATH),
            note_count: pinned_notes.len(),
            recursive_count: pinned_notes.len(),
            stale: false,
            is_pinned: false,
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

        #[allow(clippy::too_many_arguments)]
        fn push_tree<'a>(
            current_folder: &'a str,
            depth: usize,
            visual: &mut Vec<VisualItem>,
            expanded_folders: &std::collections::HashSet<String>,
            subfolders_map: &std::collections::HashMap<&'a str, Vec<&'a String>>,
            by_folder: &std::collections::HashMap<&'a str, Vec<(usize, &'a NoteSummary)>>,
            folders_first: bool,
            recursive_count: &std::collections::HashMap<&'a str, usize>,
            pinned_folders: &'a std::collections::HashSet<String>,
        ) {
            let notes = by_folder.get(current_folder);
            let subfolders = subfolders_map.get(current_folder);

            if folders_first {
                if let Some(folders) = subfolders {
                    for folder in folders {
                        let parts: Vec<&str> = folder.split('/').collect();
                        let name = parts.last().unwrap_or(&"").to_string();
                        let is_expanded = expanded_folders.contains(folder.as_str());
                        let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                        let rec_count = recursive_count
                            .get(folder.as_str())
                            .copied()
                            .unwrap_or(direct);
                        let stale = rec_count == 0;
                        visual.push(VisualItem::Folder {
                            path: folder.to_string(),
                            name,
                            depth,
                            is_expanded,
                            note_count: direct,
                            recursive_count: rec_count,
                            stale,
                            is_pinned: pinned_folders.contains(folder.as_str()),
                        });
                        if is_expanded {
                            push_tree(
                                folder,
                                depth + 1,
                                visual,
                                expanded_folders,
                                subfolders_map,
                                by_folder,
                                folders_first,
                                recursive_count,
                                pinned_folders,
                            );
                        }
                    }
                }
                if let Some(notes) = notes {
                    for (idx, note) in notes {
                        visual.push(VisualItem::Note {
                            summary_idx: *idx,
                            depth,
                            is_clin: note.id.ends_with(".clin"),
                            is_draw: note.id.ends_with(".draw"),
                            is_canvas: note.id.ends_with(".canvas"),
                            in_virtual_pinned_folder: false,
                        });
                    }
                }
                visual.push(VisualItem::CreateNew {
                    path: current_folder.to_string(),
                    depth,
                });
            } else {
                if let Some(notes) = notes {
                    for (idx, note) in notes {
                        visual.push(VisualItem::Note {
                            summary_idx: *idx,
                            depth,
                            is_clin: note.id.ends_with(".clin"),
                            is_draw: note.id.ends_with(".draw"),
                            is_canvas: note.id.ends_with(".canvas"),
                            in_virtual_pinned_folder: false,
                        });
                    }
                }
                visual.push(VisualItem::CreateNew {
                    path: current_folder.to_string(),
                    depth,
                });
                if let Some(folders) = subfolders {
                    for folder in folders {
                        let parts: Vec<&str> = folder.split('/').collect();
                        let name = parts.last().unwrap_or(&"").to_string();
                        let is_expanded = expanded_folders.contains(folder.as_str());
                        let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                        let rec_count = recursive_count
                            .get(folder.as_str())
                            .copied()
                            .unwrap_or(direct);
                        let stale = rec_count == 0;
                        visual.push(VisualItem::Folder {
                            path: folder.to_string(),
                            name,
                            depth,
                            is_expanded,
                            note_count: direct,
                            recursive_count: rec_count,
                            stale,
                            is_pinned: pinned_folders.contains(folder.as_str()),
                        });
                        if is_expanded {
                            push_tree(
                                folder,
                                depth + 1,
                                visual,
                                expanded_folders,
                                subfolders_map,
                                by_folder,
                                folders_first,
                                recursive_count,
                                pinned_folders,
                            );
                        }
                    }
                }
            }
        }

        let mut sorted_pinned: Vec<String> = self
            .list
            .pinned_folders
            .iter()
            .filter(|p| !p.is_empty() && !p.starts_with('@'))
            .cloned()
            .collect();
        sorted_pinned.sort();

        for pinned_path in sorted_pinned {
            let name = if let Some(slash) = pinned_path.rfind('/') {
                pinned_path[slash + 1..].to_string()
            } else {
                pinned_path.clone()
            };

            let is_expanded = self.list.folder_expanded.contains(&pinned_path);
            let direct = by_folder.get(pinned_path.as_str()).map_or(0, |v| v.len());
            let rec_count = recursive_count
                .get(pinned_path.as_str())
                .copied()
                .unwrap_or(direct);
            let stale = rec_count == 0;

            visual.push(VisualItem::Folder {
                path: pinned_path.clone(),
                name,
                depth: 0,
                is_expanded,
                note_count: direct,
                recursive_count: rec_count,
                stale,
                is_pinned: true,
            });

            if is_expanded {
                push_tree(
                    &pinned_path,
                    1,
                    &mut visual,
                    &self.list.folder_expanded,
                    &subfolders_map,
                    &by_folder,
                    self.list.folders_first,
                    &recursive_count,
                    &self.list.pinned_folders,
                );
            }
        }
        let mut computed_smart_folders = Vec::new();
        if self.config.list.smart_folders_enabled {
            let now = crate::ui::now_unix_secs();
            let mut today_notes = Vec::new();
            let mut week_notes = Vec::new();
            let mut untagged_notes = Vec::new();
            let mut tag_to_notes: HashMap<String, Vec<usize>> = HashMap::new();

            for (idx, note) in self.notes.iter().enumerate() {
                let diff = now.saturating_sub(note.updated_at);
                if diff < 86_400 {
                    today_notes.push(idx);
                }
                if diff < 604_800 {
                    week_notes.push(idx);
                }
                if note.tags.is_empty() {
                    untagged_notes.push(idx);
                }
                for tag in &note.tags {
                    tag_to_notes.entry(tag.clone()).or_default().push(idx);
                }
            }

            if !today_notes.is_empty() {
                computed_smart_folders.push(SmartFolderData {
                    kind: SmartFolderKind::Today,
                    label: "Today".to_string(),
                    matches: today_notes,
                });
            }
            if !week_notes.is_empty() {
                computed_smart_folders.push(SmartFolderData {
                    kind: SmartFolderKind::ThisWeek,
                    label: "This Week".to_string(),
                    matches: week_notes,
                });
            }
            if !untagged_notes.is_empty() {
                computed_smart_folders.push(SmartFolderData {
                    kind: SmartFolderKind::Untagged,
                    label: "Untagged".to_string(),
                    matches: untagged_notes,
                });
            }

            for rule in &self.config.list.custom_smart_folders {
                let mut matches = Vec::new();
                for (idx, note) in self.notes.iter().enumerate() {
                    let mut ok = true;
                    for t in &rule.tags {
                        if !note.tags.contains(t) {
                            ok = false;
                            break;
                        }
                    }
                    if let Some(txt) = &rule.title_contains
                        && !note.title.to_lowercase().contains(&txt.to_lowercase())
                    {
                        ok = false;
                    }
                    if let Some(prefix) = &rule.folder_prefix
                        && !note.folder.starts_with(prefix)
                    {
                        ok = false;
                    }
                    if let Some(days) = rule.updated_within_days {
                        let diff = now.saturating_sub(note.updated_at);
                        if diff >= days * 86_400 {
                            ok = false;
                        }
                    }
                    if ok {
                        matches.push(idx);
                    }
                }
                if !matches.is_empty() {
                    computed_smart_folders.push(SmartFolderData {
                        kind: SmartFolderKind::Custom(rule.name.clone()),
                        label: rule.name.clone(),
                        matches,
                    });
                }
            }

            let mut sorted_tags: Vec<String> = tag_to_notes.keys().cloned().collect();
            sorted_tags.sort();
            for tag in sorted_tags {
                if let Some(matching) = tag_to_notes.remove(&tag) {
                    computed_smart_folders.push(SmartFolderData {
                        kind: SmartFolderKind::Tag(tag.clone()),
                        label: tag,
                        matches: matching,
                    });
                }
            }

            for data in &computed_smart_folders {
                let virtual_path = data.kind.virtual_path();
                let is_expanded = self.list.folder_expanded.contains(&virtual_path);
                visual.push(VisualItem::SmartFolder {
                    kind: data.kind.clone(),
                    label: data.label.clone(),
                    depth: 0,
                    is_expanded,
                    note_count: data.matches.len(),
                });
                if is_expanded {
                    for idx in &data.matches {
                        let note = &self.notes[*idx];
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
            }
        }
        let vault_direct = by_folder.get("").map_or(0, |v| v.len());
        let vault_recursive = recursive_count.get("").copied().unwrap_or(vault_direct);
        visual.push(VisualItem::Folder {
            path: String::new(),
            name: String::from("Vault"),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(""),
            note_count: vault_direct,
            recursive_count: vault_recursive,
            stale: false,
            is_pinned: false,
        });

        if self.list.folder_expanded.contains("") {
            push_tree(
                "",
                1,
                &mut visual,
                &self.list.folder_expanded,
                &subfolders_map,
                &by_folder,
                self.list.folders_first,
                &recursive_count,
                &self.list.pinned_folders,
            );
        }

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
            } else if gf == VIRTUAL_SMART_PATH {
                // Smart Folders tab: show all smart folders as tiles, no ".." since it's the root of the tab
                for data in &computed_smart_folders {
                    visual.push(VisualItem::SmartFolder {
                        kind: data.kind.clone(),
                        label: data.label.clone(),
                        depth: 0,
                        is_expanded: false,
                        note_count: data.matches.len(),
                    });
                }
            } else if gf.starts_with('@') {
                // User is inside a smart folder.
                // 1. Push ".." pointing back to Smart tab root
                visual.push(VisualItem::Folder {
                    path: VIRTUAL_SMART_PATH.to_string(),
                    name: "..".to_string(),
                    depth: 0,
                    is_expanded: false,
                    note_count: 0,
                    recursive_count: 0,
                    stale: false,
                    is_pinned: false,
                });
                // 2. Find the matching smart folder by virtual path and render its notes
                if let Some(folder_data) = computed_smart_folders
                    .iter()
                    .find(|d| d.kind.virtual_path() == *gf)
                {
                    for idx in &folder_data.matches {
                        let note = &self.notes[*idx];
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
                        recursive_count: 0,
                        stale: false,
                        is_pinned: false,
                    });
                }

                // Direct subfolders / notes of the current folder, respecting folders_first
                if self.list.folders_first {
                    for folder in all_folders {
                        let parent_path = if let Some(slash) = folder.rfind('/') {
                            &folder[..slash]
                        } else {
                            ""
                        };
                        if parent_path == gf {
                            let parts: Vec<&str> = folder.split('/').collect();
                            let name = parts.last().unwrap_or(&"").to_string();
                            let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                            let rec_count = recursive_count
                                .get(folder.as_str())
                                .copied()
                                .unwrap_or(direct);
                            visual.push(VisualItem::Folder {
                                path: folder.clone(),
                                name,
                                depth: 0,
                                is_expanded: false,
                                note_count: direct,
                                recursive_count: rec_count,
                                stale: false,
                                is_pinned: false,
                            });
                        }
                    }
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
                } else {
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
                    for folder in all_folders {
                        let parent_path = if let Some(slash) = folder.rfind('/') {
                            &folder[..slash]
                        } else {
                            ""
                        };
                        if parent_path == gf {
                            let parts: Vec<&str> = folder.split('/').collect();
                            let name = parts.last().unwrap_or(&"").to_string();
                            let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                            let rec_count = recursive_count
                                .get(folder.as_str())
                                .copied()
                                .unwrap_or(direct);
                            visual.push(VisualItem::Folder {
                                path: folder.clone(),
                                name,
                                depth: 0,
                                is_expanded: false,
                                note_count: direct,
                                recursive_count: rec_count,
                                stale: false,
                                is_pinned: false,
                            });
                        }
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
        if list_active
            && (self.list.preview_content_width != Some(self.desired_list_preview_width())
                || self.list.preview_content_height != Some(self.desired_list_preview_height())
                || self.list.preview_content_scale != Some(self.list.preview_scale)
                || self.list.preview_content_offset_x != Some(self.list.preview_offset_x)
                || self.list.preview_content_offset_y != Some(self.list.preview_offset_y))
        {
            self.update_preview();
            updated = true;
        }
        let edit_active = self.editor.editor_preview_enabled || self.preview_fullscreen;
        if edit_active
            && (self.editor.preview_content_width != Some(self.desired_editor_preview_width())
                || self.editor.preview_content_height != Some(self.desired_editor_preview_height()))
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

    /// Install a completed decode into the active view's image cache.
    pub fn install_image(&mut self, decoded: crate::image_render::worker::DecodedImage) {
        let picker = match self.image_picker.as_ref() {
            Some(p) => p,
            None => return,
        };

        match self.mode {
            crate::app::ViewMode::Canvas => {
                if let Some(state) = &mut self.canvas_state {
                    state.image_cache.install_decoded(decoded, picker);
                }
            }
            crate::app::ViewMode::Edit => {
                self.editor.image_cache.install_decoded(decoded, picker);
            }
            crate::app::ViewMode::List => {
                self.list.image_cache.install_decoded(decoded, picker);
            }
            _ => {}
        }
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
                    // Reuse cached DrawData for in-memory re-render (avoids disk I/O)
                    if self.list.preview_content_index == Some(self.list.visual_index)
                        && let Some(PreviewContent::DrawGrid { data, .. }) =
                            self.list.preview_content.take()
                    {
                        let width = self.desired_list_preview_width();
                        let height = self.desired_list_preview_height();
                        let scale = self.list.preview_scale;
                        let offset_x = self.list.preview_offset_x;
                        let offset_y = self.list.preview_offset_y;
                        let grid = crate::snapshot::render_draw_snapshot_with_size(
                            &data,
                            &self.app_theme,
                            self.config.ui.icon_mode,
                            width,
                            height,
                            scale,
                            offset_x,
                            offset_y,
                        );
                        self.list.preview_content = Some(PreviewContent::DrawGrid { data, grid });
                        self.list.preview_content_width = Some(width);
                        self.list.preview_content_height = Some(height);
                        self.list.preview_content_scale = Some(scale);
                        self.list.preview_content_offset_x = Some(offset_x);
                        self.list.preview_content_offset_y = Some(offset_y);
                        self.list.preview_content_index = Some(self.list.visual_index);
                        return;
                    }
                    let path = self.storage.note_path(id);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<crate::draw::state::DrawData>(&content) {
                                Ok(data) => {
                                    let width = self.desired_list_preview_width();
                                    let height = self.desired_list_preview_height();
                                    let scale = self.list.preview_scale;
                                    let offset_x = self.list.preview_offset_x;
                                    let offset_y = self.list.preview_offset_y;
                                    let grid = crate::snapshot::render_draw_snapshot_with_size(
                                        &data,
                                        &self.app_theme,
                                        self.config.ui.icon_mode,
                                        width,
                                        height,
                                        scale,
                                        offset_x,
                                        offset_y,
                                    );
                                    self.list.preview_content = Some(PreviewContent::DrawGrid {
                                        data: Box::new(data),
                                        grid,
                                    });
                                    self.list.preview_content_width = Some(width);
                                    self.list.preview_content_height = Some(height);
                                    self.list.preview_content_scale = Some(scale);
                                    self.list.preview_content_offset_x = Some(offset_x);
                                    self.list.preview_content_offset_y = Some(offset_y);
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
                    // Reuse cached CanvasData for in-memory re-render (avoids disk I/O)
                    if self.list.preview_content_index == Some(self.list.visual_index)
                        && let Some(PreviewContent::CanvasGrid { data, .. }) =
                            self.list.preview_content.take()
                    {
                        let width = self.desired_list_preview_width();
                        let height = self.desired_list_preview_height();
                        let scale = self.list.preview_scale;
                        let offset_x = self.list.preview_offset_x;
                        let offset_y = self.list.preview_offset_y;
                        let grid = crate::snapshot::render_canvas_snapshot(
                            &data,
                            &self.app_theme,
                            self.config.ui.icon_mode,
                            width,
                            height,
                            scale,
                            offset_x,
                            offset_y,
                        );
                        self.list.preview_content = Some(PreviewContent::CanvasGrid { data, grid });
                        self.list.preview_content_width = Some(width);
                        self.list.preview_content_height = Some(height);
                        self.list.preview_content_scale = Some(scale);
                        self.list.preview_content_offset_x = Some(offset_x);
                        self.list.preview_content_offset_y = Some(offset_y);
                        self.list.preview_content_index = Some(self.list.visual_index);
                        return;
                    }
                    let path = self.storage.note_path(id);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<crate::pinstar::data::CanvasData>(&content)
                            {
                                Ok(data) => {
                                    let width = self.desired_list_preview_width();
                                    let height = self.desired_list_preview_height();
                                    let scale = self.list.preview_scale;
                                    let offset_x = self.list.preview_offset_x;
                                    let offset_y = self.list.preview_offset_y;
                                    let grid = crate::snapshot::render_canvas_snapshot(
                                        &data,
                                        &self.app_theme,
                                        self.config.ui.icon_mode,
                                        width,
                                        height,
                                        scale,
                                        offset_x,
                                        offset_y,
                                    );
                                    self.list.preview_content = Some(PreviewContent::CanvasGrid {
                                        data: Box::new(data),
                                        grid,
                                    });
                                    self.list.preview_content_width = Some(width);
                                    self.list.preview_content_height = Some(height);
                                    self.list.preview_content_scale = Some(scale);
                                    self.list.preview_content_offset_x = Some(offset_x);
                                    self.list.preview_content_offset_y = Some(offset_y);
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

                let ext = std::path::Path::new(id)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if crate::storage::is_image_ext(ext) {
                    let path = self.storage.note_path(id);
                    self.list.preview_content = Some(PreviewContent::Image(path));
                    self.list.preview_content_width = Some(self.desired_list_preview_width());
                    self.list.preview_content_height = Some(self.desired_list_preview_height());
                    self.list.preview_content_scale = Some(self.list.preview_scale);
                    self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                    self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }

                if let Ok(note) = self.storage.load_note(id) {
                    let width = self.desired_list_preview_width();
                    let mut renderer = MarkdownRenderer::new(width);
                    let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
                    renderer.render_with(&note.content, width, &self.app_theme, &opts);
                    self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                    self.list.preview_content_width = Some(width);
                    self.list.preview_content_height = Some(self.desired_list_preview_height());
                    self.list.preview_content_scale = Some(self.list.preview_scale);
                    self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                    self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                } else {
                    self.list.preview_content = None;
                }
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            Some(VisualItem::Folder { path, name, .. }) => {
                let folder_path = path.clone();
                let is_pinned = folder_path == crate::app::VIRTUAL_PINNED_PATH;

                let all_folders = if let Some(cache) = &self.list.folder_cache {
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
                let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
                renderer.render_with(&md, width, &self.app_theme, &opts);
                self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                self.list.preview_content_width = Some(width);
                self.list.preview_content_height = Some(self.desired_list_preview_height());
                self.list.preview_content_scale = Some(self.list.preview_scale);
                self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
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
        let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
        renderer.render_with(&content, width, &self.app_theme, &opts);
        self.editor.md_preview_renderer = Some(renderer);
        self.editor.preview_content_width = Some(width);
        self.editor.preview_content_height = Some(self.desired_editor_preview_height());
    }
}
