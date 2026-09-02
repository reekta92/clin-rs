use super::*;
use crate::list_view::*;
use crate::popups::*;

impl App {
    pub fn collapse_selected_folder(&mut self) {
        if self.list.visual_list.is_empty() {
            return;
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::Folder {
                path, is_expanded, ..
            } => {
                if *is_expanded {
                    self.list.folder_expanded.remove(path);
                    self.refresh_visual_list();
                    self.persist_folder_state();
                } else if !path.is_empty() {
                    let parent_path = if let Some(slash) = path.rfind('/') {
                        &path[..slash]
                    } else {
                        ""
                    };

                    if let Some(idx) = self.list.visual_list.iter().position(|v| {
                        if let VisualItem::Folder { path: p, .. } = v {
                            p == parent_path
                        } else {
                            false
                        }
                    }) {
                        self.list.visual_index = idx;
                    }
                }
            }
            VisualItem::SmartFolder {
                kind, is_expanded, ..
            } => {
                if *is_expanded {
                    self.list.folder_expanded.remove(&kind.virtual_path());
                    self.refresh_visual_list();
                    self.persist_folder_state();
                }
            }
            VisualItem::Note { .. } | VisualItem::CreateNew { .. } => {
                let item_path = match &self.list.visual_list[self.list.visual_index] {
                    VisualItem::Note { summary_idx, .. } => &self.notes[*summary_idx].folder,
                    VisualItem::CreateNew { path, .. } => path,
                    _ => unreachable!(),
                };

                if let Some(idx) = self.list.visual_list.iter().position(|v| {
                    if let VisualItem::Folder { path: p, .. } = v {
                        p == item_path
                    } else {
                        false
                    }
                }) {
                    self.list.visual_index = idx;
                }
            }
            VisualItem::Subnote { .. } => {}
        }
    }

    pub fn expand_selected_folder(&mut self) {
        if self.list.visual_list.is_empty() {
            return;
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::Folder {
                path, is_expanded, ..
            } => {
                if !is_expanded {
                    self.list.folder_expanded.insert(path.clone());
                    self.refresh_visual_list();
                    self.persist_folder_state();
                } else if self.list.visual_index + 1 < self.list.visual_list.len() {
                    self.list.visual_index += 1;
                }
            }
            VisualItem::SmartFolder {
                kind, is_expanded, ..
            } => {
                if !is_expanded {
                    self.list.folder_expanded.insert(kind.virtual_path());
                    self.refresh_visual_list();
                    self.persist_folder_state();
                } else if self.list.visual_index + 1 < self.list.visual_list.len() {
                    self.list.visual_index += 1;
                }
            }
            VisualItem::Note { .. } | VisualItem::CreateNew { .. } => {
                self.open_selected();
            }
            VisualItem::Subnote { .. } => {}
        }
    }

    pub(crate) fn open_folder_picker(&mut self, mode: FolderPickerMode, hide_paths: &[String]) {
        let Ok(mut folders) = self.storage.list_folders(self.list.show_hidden_files) else {
            self.set_temporary_status_static("Failed to list folders");
            self.messages.push(
                "Failed to list folders".to_string(),
                crate::app::messages::MessageSeverity::Warning,
            );
            return;
        };
        // Protection layer: a selected source folder and ALL its descendants are
        // removed from the picker so they can never be chosen as a destination.
        let hide: Vec<&String> = hide_paths.iter().filter(|p| !p.is_empty()).collect();
        folders.retain(|f| {
            !hide
                .iter()
                .any(|h| f == *h || f.starts_with(&format!("{}/", h)))
        });
        let mut all_folders = vec![String::new()]; // "" = vault root, always present and selectable
        all_folders.extend(folders);
        let input = crate::ui::make_popup_textarea(&self.app_theme, "Search folders...");
        self.popups.active = Some(crate::popups::ActivePopup::FolderPicker(FolderPicker {
            mode,
            filtered_folders: all_folders.clone(),
            all_folders,
            selected: 0,
            input,
            focus: FolderPickerFocus::Search,
            scroll_offset: 0,
            last_scroll: None,
        }));
    }

    pub fn confirm_folder_popup(&mut self) {
        if let Some(crate::popups::ActivePopup::Folder(popup)) = self.popups.active.take() {
            let text = popup.input.lines().join("");
            let text = text.trim();
            if text.is_empty() {
                self.set_temporary_status_static("Folder name cannot be empty");
                return;
            }

            match &popup.mode {
                FolderPopupMode::Create { parent_path } => {
                    if Self::is_virtual_pinned_path(parent_path)
                        || Self::is_virtual_subnotes_path(parent_path)
                        || Self::is_subnotes_parent_grid_path(parent_path)
                    {
                        self.set_temporary_status_static(
                            "Cannot create folder inside virtual Subnotes",
                        );
                        return;
                    }
                    let full_path = if parent_path.is_empty() {
                        text.to_string()
                    } else {
                        format!("{parent_path}/{text}")
                    };
                    if let Err(e) = self.storage.create_folder(&full_path) {
                        self.set_temporary_status(&format!("Failed to create folder: {e}"));
                    } else {
                        self.request_notes_reconcile();
                        self.set_temporary_status_static("Folder created");
                    }
                }
                FolderPopupMode::Rename { old_path } => {
                    if Self::is_virtual_pinned_path(old_path)
                        || Self::is_virtual_subnotes_path(old_path)
                        || Self::is_subnotes_parent_grid_path(old_path)
                    {
                        self.set_temporary_status_static("Cannot rename virtual Subnotes folder");
                        return;
                    }
                    if let Err(e) = self.storage.rename_folder(old_path, text) {
                        self.set_temporary_status(&format!("Failed to rename folder: {e}"));
                        self.messages.push(
                            format!("Failed to rename folder: {e}"),
                            crate::app::messages::MessageSeverity::Warning,
                        );
                    } else {
                        self.request_notes_reconcile();
                        self.set_temporary_status_static("Folder renamed");
                    }
                }
            }
        }
    }

    pub fn begin_move_note(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let note = &self.notes[*summary_idx];
            self.open_folder_picker(
                FolderPickerMode::MoveNote {
                    note_id: note.id.clone(),
                },
                &[],
            );
        } else {
            self.set_temporary_status_static("Select a note to move");
        }
    }

    pub fn begin_move_folder(&mut self) {
        if let Some(VisualItem::Folder { path, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            if Self::is_virtual_pinned_path(path)
                || Self::is_virtual_subnotes_path(path)
                || Self::is_subnotes_parent_grid_path(path)
            {
                self.set_temporary_status_static("Cannot move virtual Subnotes folder");
                return;
            }
            let folder_path = path.clone();
            self.open_folder_picker(
                FolderPickerMode::MoveFolder {
                    folder_path: folder_path.clone(),
                },
                &[folder_path],
            );
        } else {
            self.set_temporary_status_static("Select a folder to move");
        }
    }

    pub(crate) fn collect_selected_notes_and_folders(&self) -> (Vec<String>, Vec<String>) {
        let mut note_ids = Vec::new();
        let mut folder_paths = Vec::new();
        for &idx in &self.list.selected_indices {
            match self.list.visual_list.get(idx) {
                Some(VisualItem::Note { summary_idx, .. }) => {
                    if let Some(n) = self.notes.get(*summary_idx) {
                        note_ids.push(n.id.clone());
                    }
                }
                Some(VisualItem::Folder { path, .. }) => {
                    if path.is_empty() {
                        continue;
                    } // never move/delete vault root
                    if Self::is_virtual_pinned_path(path)
                        || Self::is_virtual_subnotes_path(path)
                        || Self::is_subnotes_parent_grid_path(path)
                    {
                        continue;
                    } // never touch virtual folders
                    folder_paths.push(path.clone());
                }
                _ => {}
            }
        }
        (note_ids, folder_paths)
    }

    pub fn begin_move(&mut self) {
        if !self.list.selected_indices.is_empty() {
            let (note_ids, folder_paths) = self.collect_selected_notes_and_folders();
            if note_ids.is_empty() && folder_paths.is_empty() {
                self.set_temporary_status_static("Nothing selected");
                return;
            }
            let mode = match (!note_ids.is_empty(), !folder_paths.is_empty()) {
                (true, false) => FolderPickerMode::BulkMoveNotes { note_ids },
                (false, true) => FolderPickerMode::BulkMoveFolders {
                    folder_paths: folder_paths.clone(),
                },
                (true, true) => FolderPickerMode::BulkMoveMixed {
                    note_ids,
                    folder_paths: folder_paths.clone(),
                },
                (false, false) => unreachable!(),
            };
            self.open_folder_picker(mode, &folder_paths);
            return;
        }
        match self.list.visual_list.get(self.list.visual_index) {
            Some(VisualItem::Note { .. }) => self.begin_move_note(),
            Some(VisualItem::Folder { .. }) => self.begin_move_folder(),
            _ => self.set_temporary_status_static("Nothing selected"),
        }
    }

    pub fn begin_duplicate(&mut self) {
        if !self.list.selected_indices.is_empty() {
            let (note_ids, folder_paths) = self.collect_selected_notes_and_folders();
            if note_ids.is_empty() && folder_paths.is_empty() {
                self.set_temporary_status_static("Nothing selected");
                return;
            }
            let mode = match (!note_ids.is_empty(), !folder_paths.is_empty()) {
                (true, false) => FolderPickerMode::BulkCopyNotes { note_ids },
                (false, true) => FolderPickerMode::BulkCopyFolders {
                    folder_paths: folder_paths.clone(),
                },
                (true, true) => FolderPickerMode::BulkCopyMixed {
                    note_ids,
                    folder_paths: folder_paths.clone(),
                },
                (false, false) => unreachable!(),
            };
            self.open_folder_picker(mode, &folder_paths);
            return;
        }
        self.duplicate_note(); // no selection: existing single-note behavior
    }

    pub(crate) fn clamp_visual_index(&mut self) {
        if self.list.visual_index >= self.list.visual_list.len()
            && !self.list.visual_list.is_empty()
        {
            self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        } else if self.list.visual_list.is_empty() {
            self.list.visual_index = 0;
        }
    }

    /// Shared post move/copy cleanup.
    fn finish_bulk_list_op(&mut self) {
        self.request_notes_reconcile();
        self.clamp_visual_index();
        self.list.selected_indices.clear();
        self.list.list_mode = ListMode::Normal;
        self.request_preview_update();
    }

    /// Move every selected folder into the single shared `target`. When `target` is
    /// itself one of the selected folders, move all OTHERS into it first, then move
    /// `target` last so the already-moved children travel with it. Returns failure count.
    fn bulk_move_folders(&mut self, folder_paths: Vec<String>, target: &str) -> usize {
        let mut failed = 0;
        let target_is_selected = folder_paths.iter().any(|f| f == target);
        // Phase 1: every folder that is not the chosen target, in selection order.
        for f in folder_paths
            .iter()
            .filter(|f| !(target_is_selected && *f == target))
        {
            if self.move_one_folder(f, target).is_err() {
                failed += 1;
            }
        }
        // Phase 2: relocate the target folder itself last (if it was co-selected).
        if target_is_selected
            && let Some(t) = folder_paths.iter().find(|f| *f == target)
            && self.move_one_folder(t, target).is_err()
        {
            failed += 1;
        }
        failed
    }

    /// Move one folder via `rename_folder`, with no-op + self/descendant guards
    /// (defense-in-depth; the picker already excludes these as destinations) and
    /// expanded-state remap on success.
    fn move_one_folder(&mut self, folder_path: &str, target: &str) -> anyhow::Result<()> {
        let base = folder_path.rsplit('/').next().unwrap_or(folder_path);
        let new_path = if target.is_empty() {
            base.to_string()
        } else {
            format!("{target}/{base}")
        };
        if folder_path == new_path {
            return Ok(()); // already at destination: no-op success
        }
        if new_path.starts_with(&format!("{folder_path}/")) {
            anyhow::bail!("Cannot move a folder into itself");
        }
        self.storage.rename_folder(folder_path, &new_path)?;
        // Remap expanded state: drop the old path and anything beneath it; if the old
        // folder was expanded, expand the new location (mirrors folders.rs:289-291).
        let was_expanded = self.list.folder_expanded.remove(folder_path);
        self.list
            .folder_expanded
            .retain(|p| !p.starts_with(&format!("{folder_path}/")));
        if was_expanded {
            self.list.folder_expanded.insert(new_path);
        }
        Ok(())
    }

    pub fn confirm_move(&mut self) {
        if let Some(crate::popups::ActivePopup::FolderPicker(picker)) = self.popups.active.take()
            && let Some(target_folder) = picker.filtered_folders.get(picker.selected)
        {
            match picker.mode {
                FolderPickerMode::MoveNote { note_id } => {
                    if let Err(e) = self.storage.move_note(&note_id, target_folder) {
                        self.set_temporary_status(&format!("Failed to move note: {e}"));
                    } else {
                        self.request_notes_reconcile();
                        self.set_temporary_status_static("Note moved");
                    }
                }
                FolderPickerMode::CopyNote { note_id } => {
                    match self.storage.duplicate_note(&note_id, target_folder) {
                        Ok(_) => {
                            self.request_notes_reconcile();
                            self.set_temporary_status_static("Note copied");
                        }
                        Err(e) => {
                            self.set_temporary_status(&format!("Failed to copy note: {e}"));
                        }
                    }
                }
                FolderPickerMode::MoveFolder { folder_path } => {
                    let folder_name = folder_path.rsplit('/').next().unwrap_or(&folder_path);
                    let new_path = if target_folder.is_empty() {
                        folder_name.to_string()
                    } else {
                        format!("{target_folder}/{folder_name}")
                    };

                    if folder_path == new_path {
                        self.set_temporary_status_static("Folder is already in this location");
                        return;
                    }

                    if let Err(e) = self.storage.rename_folder(&folder_path, &new_path) {
                        self.set_temporary_status(&format!("Failed to move folder: {e}"));
                    } else {
                        if self.list.folder_expanded.remove(&folder_path) {
                            self.list.folder_expanded.insert(new_path);
                        }
                        self.request_notes_reconcile();
                        self.set_temporary_status_static("Folder moved");
                    }
                }
                FolderPickerMode::BulkMoveNotes { note_ids } => {
                    let mut failed = 0;
                    for id in note_ids {
                        if self.storage.move_note(&id, target_folder).is_err() {
                            failed += 1;
                        }
                    }
                    self.finish_bulk_list_op();
                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to move {failed} note(s)"));
                    } else {
                        self.set_temporary_status_static("Selected notes moved");
                    }
                }
                FolderPickerMode::BulkCopyNotes { note_ids } => {
                    let mut failed = 0;
                    for id in &note_ids {
                        if self.storage.duplicate_note(id, target_folder).is_err() {
                            failed += 1;
                        }
                    }
                    self.finish_bulk_list_op();
                    let ok = note_ids.len() - failed;
                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to copy {failed} note(s)"));
                    } else {
                        self.set_temporary_status(&format!("Copied {ok} note(s)"));
                    }
                }
                FolderPickerMode::BulkCopyFolders { folder_paths } => {
                    let mut failed = 0;
                    for p in &folder_paths {
                        if self.storage.duplicate_folder(p, target_folder).is_err() {
                            failed += 1;
                        }
                    }
                    self.finish_bulk_list_op();
                    let ok = folder_paths.len() - failed;
                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to copy {failed} folder(s)"));
                    } else {
                        self.set_temporary_status(&format!("Copied {ok} folder(s)"));
                    }
                }
                FolderPickerMode::BulkCopyMixed {
                    note_ids,
                    folder_paths,
                } => {
                    let mut failed = 0;
                    for id in &note_ids {
                        if self.storage.duplicate_note(id, target_folder).is_err() {
                            failed += 1;
                        }
                    }
                    for p in &folder_paths {
                        if self.storage.duplicate_folder(p, target_folder).is_err() {
                            failed += 1;
                        }
                    }
                    self.finish_bulk_list_op();
                    let total = note_ids.len() + folder_paths.len();
                    let ok = total - failed;
                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to copy {failed} item(s)"));
                    } else {
                        self.set_temporary_status(&format!("Copied {ok} item(s)"));
                    }
                }
                FolderPickerMode::BulkMoveFolders { folder_paths } => {
                    let failed = self.bulk_move_folders(folder_paths, target_folder);
                    self.finish_bulk_list_op();
                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to move {failed} folder(s)"));
                    } else {
                        self.set_temporary_status_static("Moved folder(s)");
                    }
                }
                FolderPickerMode::BulkMoveMixed {
                    note_ids,
                    folder_paths,
                } => {
                    let total = note_ids.len() + folder_paths.len();
                    let mut failed = 0;
                    for id in &note_ids {
                        if self.storage.move_note(id, target_folder).is_err() {
                            failed += 1;
                        }
                    }
                    failed += self.bulk_move_folders(folder_paths, target_folder);
                    self.finish_bulk_list_op();
                    let ok = total - failed;
                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to move {failed} item(s)"));
                    } else {
                        self.set_temporary_status(&format!("Moved {ok} item(s)"));
                    }
                }
            }
        }
    }

    pub fn update_folder_picker_filter(&mut self) {
        if let Some(crate::popups::ActivePopup::FolderPicker(picker)) = &mut self.popups.active {
            let query = picker.input.lines().join("").trim().to_lowercase();
            if query.is_empty() {
                picker.filtered_folders = picker.all_folders.clone();
            } else {
                picker.filtered_folders = picker
                    .all_folders
                    .iter()
                    .filter(|folder| folder.to_lowercase().contains(&query))
                    .cloned()
                    .collect();
            }
            if picker.selected >= picker.filtered_folders.len() {
                picker.selected = picker.filtered_folders.len().saturating_sub(1);
            }
        }
    }

    pub fn collapse_all_folders(&mut self) {
        self.list.folder_expanded.clear();
        self.list.folder_expanded.insert(String::new());
        self.list.visual_index = 0;
        self.refresh_visual_list();
        self.request_preview_update();
        self.persist_folder_state();
    }

    pub fn expand_all_folders(&mut self) {
        self.expand_folders_to_depth(usize::MAX);
        self.list.visual_index = 0;
        self.refresh_visual_list();
        self.request_preview_update();
        self.persist_folder_state();
    }

    pub fn expand_to_level(&mut self, level: usize) {
        self.expand_folders_to_depth(level);
        self.refresh_visual_list();
        self.request_preview_update();
        self.persist_folder_state();
    }

    pub fn expand_folders_to_depth(&mut self, level: usize) {
        let folders = self.catalog_folders.clone();

        for path in folders {
            let depth = if path.is_empty() {
                0
            } else {
                path.matches('/').count() + 1
            };
            if depth < level {
                self.list.folder_expanded.insert(path);
            }
        }
        self.list.folder_expanded.insert(String::new());
        self.list
            .folder_expanded
            .insert(crate::app::VIRTUAL_PINNED_PATH.to_string());
        // Virtual folders for subnotes and smart folders
        self.list
            .folder_expanded
            .insert(crate::app::VIRTUAL_SUBNOTES_PATH.to_string());
        if self.config.list.smart_folders_enabled {
            self.list.folder_expanded.insert("@today".to_string());
            self.list.folder_expanded.insert("@week".to_string());
            self.list.folder_expanded.insert("@untagged".to_string());
            self.list.folder_expanded.insert("@tagged".to_string());
            for rule in &self.config.list.custom_smart_folders {
                self.list
                    .folder_expanded
                    .insert(format!("@custom:{}", rule.name));
            }
            let mut seen_tags: std::collections::HashSet<String> = std::collections::HashSet::new();
            for note in &self.notes {
                for tag in &note.tags {
                    if seen_tags.insert(tag.clone()) {
                        self.list.folder_expanded.insert(format!("@tag:{tag}"));
                    }
                }
            }
        }
    }
    pub fn toggle_pin_folder(&mut self, path: String) {
        if path.is_empty() || path == crate::app::VIRTUAL_PINNED_PATH {
            self.set_temporary_status_static("Cannot pin Vault root or Pinned notes");
            return;
        }
        if self.list.pinned_folders.contains(&path) {
            self.list.pinned_folders.remove(&path);
            self.set_temporary_status_static("Folder unpinned from top");
        } else {
            self.list.pinned_folders.insert(path);
            self.set_temporary_status_static("Folder pinned to top");
        }
        self.refresh_visual_list();
        self.persist_folder_state();
    }

    pub fn get_selected_folder_path(&self) -> Option<String> {
        if self.mode == ViewMode::List
            && let Some(VisualItem::Folder { path, .. }) =
                self.list.visual_list.get(self.list.visual_index)
        {
            return Some(path.clone());
        }
        None
    }
}
