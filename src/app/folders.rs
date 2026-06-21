use super::*;
use crate::list_view::*;
use crate::popups::*;
use ratatui_textarea::TextArea;

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
                } else if self.list.visual_index + 1 < self.list.visual_list.len() {
                    self.list.visual_index += 1;
                }
            }
            VisualItem::Note { .. } | VisualItem::CreateNew { .. } => {
                self.open_selected();
            }
        }
    }

    pub fn confirm_folder_popup(&mut self) {
        if let Some(popup) = self.popups.folder.take() {
            let text = popup.input.lines().join("");
            let text = text.trim();
            if text.is_empty() {
                self.set_temporary_status_static("Folder name cannot be empty");
                return;
            }

            match &popup.mode {
                FolderPopupMode::Create { parent_path } => {
                    if Self::is_virtual_pinned_path(parent_path) {
                        self.set_temporary_status_static(
                            "Cannot create folder inside virtual Pinned",
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
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Folder created");
                    }
                }
                FolderPopupMode::Rename { old_path } => {
                    if Self::is_virtual_pinned_path(old_path) {
                        self.set_temporary_status_static("Cannot rename virtual Pinned folder");
                        return;
                    }
                    if let Err(e) = self.storage.rename_folder(old_path, text) {
                        self.set_temporary_status(&format!("Failed to rename folder: {e}"));
                    } else {
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
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
            if let Ok(folders) = self.storage.list_folders(self.list.show_hidden_files) {
                let mut all_folders = vec!["".to_string()];
                all_folders.extend(folders);
                let mut input = TextArea::default();
                input.set_cursor_line_style(ratatui::style::Style::default());
                input.set_placeholder_text("Search folders...");
                self.popups.folder_picker = Some(FolderPicker {
                    mode: FolderPickerMode::MoveNote {
                        note_id: note.id.clone(),
                    },
                    filtered_folders: all_folders.clone(),
                    all_folders,
                    selected: 0,
                    input,
                    focus: FolderPickerFocus::Search,
                });
            } else {
                self.set_temporary_status_static("Failed to list folders");
            }
        } else {
            self.set_temporary_status_static("Select a note to move");
        }
    }

    pub fn begin_move_folder(&mut self) {
        if let Some(VisualItem::Folder { path, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            if Self::is_virtual_pinned_path(path) {
                self.set_temporary_status_static("Cannot move virtual Pinned folder");
                return;
            }
            let folder_path = path.clone();
            if let Ok(folders) = self.storage.list_folders(self.list.show_hidden_files) {
                let mut all_folders = vec!["".to_string()];
                all_folders.extend(
                    folders.into_iter().filter(|f| {
                        f != &folder_path && !f.starts_with(&format!("{folder_path}/"))
                    }),
                );

                let mut input = TextArea::default();
                input.set_cursor_line_style(ratatui::style::Style::default());
                input.set_placeholder_text("Search folders...");
                self.popups.folder_picker = Some(FolderPicker {
                    mode: FolderPickerMode::MoveFolder { folder_path },
                    filtered_folders: all_folders.clone(),
                    all_folders,
                    selected: 0,
                    input,
                    focus: FolderPickerFocus::Search,
                });
            } else {
                self.set_temporary_status_static("Failed to list folders");
            }
        } else {
            self.set_temporary_status_static("Select a folder to move");
        }
    }

    pub fn begin_move(&mut self) {
        if !self.list.selected_indices.is_empty() {
            let mut note_ids = Vec::new();
            for &idx in &self.list.selected_indices {
                if let Some(VisualItem::Note { summary_idx, .. }) = self.list.visual_list.get(idx) {
                    note_ids.push(self.notes[*summary_idx].id.clone());
                }
            }

            if !note_ids.is_empty()
                && let Ok(folders) = self.storage.list_folders(self.list.show_hidden_files)
            {
                let mut all_folders = vec!["".to_string()];
                all_folders.extend(folders);
                let mut input = TextArea::default();
                input.set_cursor_line_style(ratatui::style::Style::default());
                input.set_placeholder_text("Search folders...");
                self.popups.folder_picker = Some(FolderPicker {
                    mode: FolderPickerMode::BulkMoveNotes { note_ids },
                    filtered_folders: all_folders.clone(),
                    all_folders,
                    selected: 0,
                    input,
                    focus: FolderPickerFocus::Search,
                });
                return;
            }
        }

        match self.list.visual_list.get(self.list.visual_index) {
            Some(VisualItem::Note { .. }) => self.begin_move_note(),
            Some(VisualItem::Folder { .. }) => self.begin_move_folder(),
            _ => self.set_temporary_status_static("Nothing selected"),
        }
    }

    pub fn confirm_move(&mut self) {
        if let Some(picker) = self.popups.folder_picker.take()
            && let Some(target_folder) = picker.filtered_folders.get(picker.selected)
        {
            match picker.mode {
                FolderPickerMode::MoveNote { note_id } => {
                    if let Err(e) = self.storage.move_note(&note_id, target_folder) {
                        self.set_temporary_status(&format!("Failed to move note: {e}"));
                    } else {
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Note moved");
                    }
                }
                FolderPickerMode::CopyNote { note_id } => {
                    if let Err(e) = self.storage.duplicate_note(&note_id, target_folder) {
                        self.set_temporary_status(&format!("Failed to copy note: {e}"));
                    } else {
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Note copied");
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
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
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

                    self.list.folder_cache = None;
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
                    self.list.selected_indices.clear();
                    self.list.list_mode = ListMode::Normal;

                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to move {failed} note(s)"));
                    } else {
                        self.set_temporary_status_static("Selected notes moved");
                    }
                }
            }
        }
    }

    pub fn update_folder_picker_filter(&mut self) {
        if let Some(picker) = &mut self.popups.folder_picker {
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
        self.refresh_visual_list();
        self.request_preview_update();
    }}
