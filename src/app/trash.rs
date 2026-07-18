use super::*;
use crate::list_view::*;
use crate::popups::*;

impl App {
    pub fn begin_delete_selected(&mut self) {
        if !self.list.selected_indices.is_empty() {
            let (note_ids, folder_paths) = self.collect_selected_notes_and_folders();
            if !note_ids.is_empty() || !folder_paths.is_empty() {
                if self.confirm_on_delete {
                    self.show_confirm(ConfirmAction::BulkDeleteItems {
                        note_ids,
                        folder_paths,
                    });
                } else {
                    self.confirm_bulk_delete(note_ids, folder_paths);
                }
                return;
            }
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.set_temporary_status_static("No item selected to delete");
            return;
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::Note { summary_idx, .. } => {
                if let Some(note) = self.notes.get(*summary_idx) {
                    let note_id = note.id.clone();
                    let title = note.title.clone();
                    if self.confirm_on_delete {
                        self.show_confirm(ConfirmAction::DeleteNote { note_id, title });
                    } else {
                        self.confirm_delete_selected(note_id);
                    }
                }
            }
            VisualItem::Folder { path, .. } => {
                if path.is_empty() {
                    self.set_temporary_status_static("Cannot delete Vault root");
                    return;
                }
                if Self::is_virtual_pinned_path(path) {
                    self.set_temporary_status_static("Cannot delete virtual Pinned folder");
                    return;
                }
                if Self::is_virtual_subnotes_path(path)
                    || Self::is_subnotes_parent_grid_path(path)
                {
                    self.set_temporary_status_static("Cannot delete virtual Subnotes folder");
                    return;
                }
                let path = path.clone();
                if self.confirm_on_delete {
                    self.show_confirm(ConfirmAction::DeleteFolder { path });
                } else {
                    self.confirm_delete_folder(path);
                }
            }
            _ => {
                self.set_temporary_status_static("Cannot delete this item");
            }
        }
    }

    pub fn confirm_delete_selected(&mut self, id: String) {
        match self.storage.trash_note(&id) {
            Ok(()) => {
                // Drop from in-memory caches without a full filesystem rescan.
                self.summary_cache.remove(&id);
                self.summary_mtime.remove(&id);
                self.notes.retain(|n| n.id != id);
                self.notes_with_subnotes =
                    self.storage.get_notes_with_subnotes().unwrap_or_default();

                self.sort_notes();
                self.refresh_visual_list();

                self.clamp_visual_index();
                self.set_temporary_status_static("Note moved to trash");
            }
            Err(err) => {
                self.set_temporary_status(&format!("Move to trash failed: {err:#}"));
            }
        }
    }

    pub fn confirm_delete_folder(&mut self, path: String) {
        match self.storage.trash_folder(&path) {
            Ok(()) => {
                self.list.folder_cache = None;
                self.list
                    .folder_expanded
                    .retain(|p| p != &path && !p.starts_with(&format!("{path}/")));
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.clamp_visual_index();
                self.set_temporary_status_static("Folder moved to trash");
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to trash folder: {e}"));
            }
        }
    }

    pub fn restore_from_trash(&mut self) {
        let item = if let Some(crate::popups::ActivePopup::TrashView(trash)) = &self.popups.active {
            trash.items.get(trash.selected).cloned()
        } else {
            None
        };

        let Some(item) = item else { return };

        match self.storage.restore_trash_items(vec![item]) {
            Ok(_) => {
                if let Ok(items) = self.storage.list_trash() {
                    if items.is_empty() {
                        self.popups.active = None;
                        self.set_temporary_status_static("Note restored, trash is now empty");
                    } else if let Some(crate::popups::ActivePopup::TrashView(trash)) =
                        &mut self.popups.active
                    {
                        trash.items = items;
                        trash.selected = trash.selected.min(trash.items.len().saturating_sub(1));
                        self.set_temporary_status_static("Note restored");
                    }
                }
                self.list.folder_cache = None;
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to restore: {e}"));
            }
        }
    }

    pub fn begin_delete_from_trash(&mut self) {
        if let Some(crate::popups::ActivePopup::TrashView(trash)) = &self.popups.active
            && let Some(item) = trash.items.get(trash.selected).cloned()
        {
            self.show_confirm(ConfirmAction::DeleteFromTrash { item });
        }
    }

    pub fn confirm_delete_from_trash(&mut self, item: ::trash::TrashItem) {
        match self.storage.purge_trash_items(vec![item]) {
            Ok(()) => {
                if let Some(crate::popups::ActivePopup::TrashView(trash)) = &mut self.popups.active
                    && let Ok(items) = self.storage.list_trash()
                {
                    if items.is_empty() {
                        self.popups.active = None;
                        self.set_temporary_status_static("Note deleted, trash is now empty");
                    } else {
                        trash.items = items;
                        trash.selected = trash.selected.min(trash.items.len().saturating_sub(1));
                        self.set_temporary_status_static("Note permanently deleted");
                    }
                }
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to delete: {e}"));
            }
        }
    }

    pub fn begin_empty_trash(&mut self) {
        if let Some(crate::popups::ActivePopup::TrashView(trash)) = &self.popups.active {
            if trash.items.is_empty() {
                self.set_temporary_status_static("Trash is already empty");
            } else {
                self.show_confirm(ConfirmAction::EmptyTrash {
                    items: trash.items.clone(),
                });
            }
        }
    }

    pub fn confirm_empty_trash(&mut self, items: Vec<::trash::TrashItem>) {
        let count = items.len();
        match self.storage.purge_trash_items(items) {
            Ok(()) => {
                self.popups.active = None;
                self.set_temporary_status(&format!("Deleted {count} notes from trash"));
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to empty trash: {e}"));
            }
        }
    }
}
