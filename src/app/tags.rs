use super::*;
use crate::popups::*;
use std::collections::HashSet;

impl App {
    pub fn collect_live_tags(&self) -> Vec<String> {
        let mut tags: HashSet<String> = HashSet::new();
        for note in &self.notes {
            for tag in &note.tags {
                tags.insert(tag.clone());
            }
        }
        let mut result: Vec<String> = tags.into_iter().collect();
        result.sort();
        result
    }

    pub fn begin_manage_tags(&mut self) {
        let in_select_mode = self.list.list_mode == crate::list_view::ListMode::Select;

        if in_select_mode && !self.list.selected_indices.is_empty() {
            // Batch mode: apply tags to all selected notes
            let batch_ids: Vec<String> = self
                .list
                .selected_indices
                .iter()
                .filter_map(|&idx| {
                    if let Some(VisualItem::Note { summary_idx, .. }) =
                        self.list.visual_list.get(idx)
                    {
                        Some(self.notes[*summary_idx].id.clone())
                    } else {
                        None
                    }
                })
                .collect();

            if batch_ids.is_empty() {
                self.set_temporary_status_static("No taggable notes selected");
                return;
            }

            // Seed input with tags from the focused note
            let current_tags = if let Some(VisualItem::Note { summary_idx, .. }) =
                self.list.visual_list.get(self.list.visual_index)
            {
                self.notes[*summary_idx].tags.clone()
            } else {
                Vec::new()
            };

            let all_tags = self.collect_live_tags();
            let mut input = crate::ui::make_popup_textarea(&self.app_theme, "Add tags...");
            input.insert_str(current_tags.join(", "));

            self.popups.active = Some(crate::popups::ActivePopup::Tag(TagPopup {
                note_id: batch_ids.first().cloned().unwrap_or_default(),
                batch_note_ids: Some(batch_ids),
                input,
                all_tags,
                suggestions: Vec::new(),
                suggestion_index: 0,
                focus: crate::popups::TagPopupFocus::Input,
                all_tags_selected: 0,
                scroll_offset: 0,
                last_scroll: None,
            }));
            self.update_tag_suggestions();
            return;
        }

        // Single-note mode (existing behavior)
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let note = &self.notes[*summary_idx];
            let ext = std::path::Path::new(&note.id)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            if ext != "md" && ext != "txt" && ext != "clin" {
                self.set_temporary_status_static(
                    "Tagging is only supported for .md, .txt, and .clin files",
                );
                return;
            }

            let current_tags = note.tags.clone();
            let all_tags = self.collect_live_tags();

            let mut input = crate::ui::make_popup_textarea(&self.app_theme, "Add tags...");
            input.insert_str(current_tags.join(", "));

            self.popups.active = Some(crate::popups::ActivePopup::Tag(TagPopup {
                note_id: note.id.clone(),
                batch_note_ids: None,
                input,
                all_tags,
                suggestions: Vec::new(),
                suggestion_index: 0,
                focus: crate::popups::TagPopupFocus::Input,
                all_tags_selected: 0,
                scroll_offset: 0,
                last_scroll: None,
            }));
            self.update_tag_suggestions();
        } else {
            self.set_temporary_status_static("Select a note to manage tags");
        }
    }

    pub fn confirm_manage_tags(&mut self) {
        if let Some(crate::popups::ActivePopup::Tag(popup)) = self.popups.active.take() {
            let text = popup.input.lines().join("");
            let mut seen = std::collections::HashSet::new();
            let tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && seen.insert(s.clone()))
                .collect();

            if let Some(batch_ids) = popup.batch_note_ids {
                // Batch mode: apply tags to all selected notes
                let mut count = 0;
                let mut failed = 0;
                for note_id in &batch_ids {
                    if let Ok(mut note) = self.storage.load_note(note_id) {
                        note.tags = tags.clone();
                        if self.storage.save_note(note_id, &note).is_ok() {
                            self.enqueue_backup(format!("auto: {}", note.title));
                            count += 1;
                        } else {
                            failed += 1;
                        }
                    }
                }
                self.list.selected_indices.clear();
                self.list.list_mode = crate::list_view::ListMode::Normal;
                self.refresh_visual_list();
                self.clamp_visual_index();
                self.request_notes_reconcile();
                self.set_temporary_status(&format!("Tags applied to {count} note(s)"));
                if failed > 0 {
                    let text = format!("Failed to update tags on {failed} note(s)");
                    self.set_temporary_status(&text);
                    self.messages
                        .push(text, crate::app::messages::MessageSeverity::Warning);
                }
                return;
            }

            // Single-note mode
            let ext = std::path::Path::new(&popup.note_id)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            if ext != "md" && ext != "txt" && ext != "clin" {
                self.set_temporary_status_static(
                    "Tagging is only supported for .md, .txt, and .clin files",
                );
                return;
            }

            if let Ok(mut note) = self.storage.load_note(&popup.note_id) {
                note.tags = tags;
                if let Err(e) = self.storage.save_note(&popup.note_id, &note) {
                    self.set_temporary_status(&format!("Failed to save tags: {e}"));
                    self.messages.push(
                        format!("Failed to save tags: {e}"),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                } else {
                    self.enqueue_backup(format!("auto: {}", note.title));
                    self.refresh_note_single(None, &popup.note_id);
                    self.set_temporary_status_static("Tags updated");
                }
            } else {
                self.set_temporary_status_static("Failed to load note to update tags");
                self.messages.push(
                    "Failed to load note to update tags".to_string(),
                    crate::app::messages::MessageSeverity::Warning,
                );
            }
        }
    }

    fn get_current_tag_word(input: &str) -> &str {
        input.rsplit(',').next().map(|s| s.trim()).unwrap_or("")
    }

    pub fn update_tag_suggestions(&mut self) {
        if let Some(crate::popups::ActivePopup::Tag(popup)) = &mut self.popups.active {
            let text = popup.input.lines().join("");
            let current_word = Self::get_current_tag_word(&text).to_lowercase();

            let entered_tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();

            if current_word.is_empty() {
                popup.suggestions.clear();
            } else {
                popup.suggestions = popup
                    .all_tags
                    .iter()
                    .filter(|tag| {
                        let tag_lower = tag.to_lowercase();
                        tag_lower.starts_with(&current_word) && !entered_tags.contains(&tag_lower)
                    })
                    .cloned()
                    .collect();
            }
            popup.suggestion_index = 0;
        }
    }

    pub fn accept_tag_suggestion(&mut self) {
        if let Some(crate::popups::ActivePopup::Tag(popup)) = &mut self.popups.active
            && let Some(suggestion) = popup.suggestions.get(popup.suggestion_index).cloned()
        {
            let text = popup.input.lines().join("");

            if let Some(last_comma) = text.rfind(',') {
                let prefix = &text[..=last_comma];
                let new_text = format!("{prefix} {suggestion}, ");

                popup.input.select_all();
                popup.input.cut();
                popup.input.insert_str(&new_text);
            } else {
                popup.input.select_all();
                popup.input.cut();
                popup.input.insert_str(format!("{suggestion}, "));
            }

            popup.suggestions.clear();
            popup.suggestion_index = 0;
        }
    }

    pub fn accept_tag_from_all_tags(&mut self) {
        if let Some(crate::popups::ActivePopup::Tag(popup)) = &mut self.popups.active
            && let Some(tag) = popup.all_tags.get(popup.all_tags_selected).cloned()
        {
            let text = popup.input.lines().join("");
            let trimmed = text.trim().trim_end_matches(',').trim();
            let new_text = if trimmed.is_empty() {
                format!("{tag}, ")
            } else {
                format!("{trimmed}, {tag}, ")
            };
            popup.input.select_all();
            popup.input.cut();
            popup.input.insert_str(&new_text);
        }
    }

    pub fn begin_delete_tag_with_name(&mut self, tag: String) {
        let count = self
            .storage
            .list_note_ids(self.list.show_hidden_files, false)
            .ok()
            .map(|ids| {
                ids.iter()
                    .filter(|id| {
                        self.storage
                            .load_note(id)
                            .ok()
                            .is_some_and(|n| n.tags.contains(&tag))
                    })
                    .count()
            })
            .unwrap_or(0);

        let detail = if count == 1 {
            "Will remove tag from 1 note.".to_string()
        } else {
            format!("Will remove tag from {count} notes.")
        };

        self.popups.confirm = Some(ConfirmPopup {
            action: ConfirmAction::DeleteTag { tag: tag.clone() },
            message: format!("Delete tag \"{tag}\"?"),
            detail: Some(detail),
            confirm_label: "Delete".into(),
            is_destructive: true,
            selected_button: 1,
        });
    }

    pub fn confirm_delete_tag(&mut self, tag: String) {
        let mut count = 0;
        let mut failed = 0;
        if let Ok(note_ids) = self
            .storage
            .list_note_ids(self.list.show_hidden_files, false)
        {
            for note_id in note_ids {
                let ext = std::path::Path::new(&note_id)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                if ext != "md" && ext != "txt" && ext != "clin" {
                    continue;
                }

                if let Ok(mut note) = self.storage.load_note(&note_id)
                    && note.tags.contains(&tag)
                {
                    note.tags.retain(|t| t != &tag);
                    if self.storage.save_note(&note_id, &note).is_ok() {
                        self.enqueue_backup(format!("auto: {}", note.title));
                    } else {
                        failed += 1;
                    }
                    count += 1;
                }
            }
        }

        self.set_temporary_status(&format!("Deleted '{tag}' from {count} note(s)"));
        if failed > 0 {
            let text = format!("Failed to update tags on {failed} note(s)");
            self.set_temporary_status(&text);
            self.messages
                .push(text, crate::app::messages::MessageSeverity::Warning);
        }
        self.request_notes_reconcile();
        let live_tags = self.collect_live_tags();

        if let Some(crate::popups::ActivePopup::Tag(popup)) = &mut self.popups.active {
            popup.all_tags = live_tags;
            let text = popup.input.lines().join("");

            let entered_tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != &tag)
                .collect();

            let new_text = if entered_tags.is_empty() {
                String::new()
            } else {
                format!("{}, ", entered_tags.join(", "))
            };

            popup.input.select_all();
            popup.input.cut();
            popup.input.insert_str(&new_text);
        }
        self.update_tag_suggestions();
    }
    pub fn begin_remove_tags_from_selected(&mut self) {
        if self.list.list_mode != crate::list_view::ListMode::Select
            || self.list.selected_indices.is_empty()
        {
            self.set_temporary_status_static("Enter Select mode and select notes first");
            return;
        }

        let mut tags_set: HashSet<String> = HashSet::new();
        for &idx in &self.list.selected_indices {
            if let Some(VisualItem::Note { summary_idx, .. }) =
                self.list.visual_list.get(idx)
            {
                for tag in &self.notes[*summary_idx].tags {
                    tags_set.insert(tag.clone());
                }
            }
        }

        let mut tags: Vec<String> = tags_set.into_iter().collect();
        tags.sort();

        if tags.is_empty() {
            self.set_temporary_status_static("Selected notes have no tags to remove");
            return;
        }

        self.popups.active = Some(crate::popups::ActivePopup::RemoveTags(RemoveTagsPopup {
            tags,
            selected: HashSet::new(),
            cursor: 0,
            scroll_offset: 0,
            last_scroll: None,
            confirm: None,
        }));
    }

    pub fn confirm_remove_tags_from_selected(&mut self) {
        let (selected_tags, note_ids) = {
            let popup = match self.popups.active.take() {
                Some(crate::popups::ActivePopup::RemoveTags(p)) => p,
                _ => return,
            };
            let selected_tags: HashSet<String> = popup
                .selected
                .iter()
                .filter_map(|&i| popup.tags.get(i).cloned())
                .collect();

            let note_ids: Vec<String> = self
                .list
                .selected_indices
                .iter()
                .filter_map(|&idx| {
                    if let Some(VisualItem::Note { summary_idx, .. }) =
                        self.list.visual_list.get(idx)
                    {
                        Some(self.notes[*summary_idx].id.clone())
                    } else {
                        None
                    }
                })
                .collect();

            (selected_tags, note_ids)
        };

        if selected_tags.is_empty() {
            self.set_temporary_status_static("No tags selected to remove");
            self.list.selected_indices.clear();
            self.list.list_mode = crate::list_view::ListMode::Normal;
            self.refresh_visual_list();
            self.clamp_visual_index();
            self.request_notes_reconcile();
            return;
        }

        let count = note_ids.len();
        let mut failed = 0;
        for note_id in &note_ids {
            if let Ok(mut note) = self.storage.load_note(note_id) {
                note.tags.retain(|t| !selected_tags.contains(t));
                if self.storage.save_note(note_id, &note).is_ok() {
                    self.enqueue_backup(format!("auto: {}", note.title));
                } else {
                    failed += 1;
                }
            }
        }

        self.list.selected_indices.clear();
        self.list.list_mode = crate::list_view::ListMode::Normal;
        self.refresh_visual_list();
        self.clamp_visual_index();
        self.request_notes_reconcile();
        self.set_temporary_status(&format!(
            "Removed tags from {}/{} note(s)",
            count - failed,
            count
        ));
        if failed > 0 {
            let text = format!("Failed to update tags on {} note(s)", failed);
            self.messages
                .push(text, crate::app::messages::MessageSeverity::Warning);
        }
    }

    pub fn begin_remove_all_tags_from_selected(&mut self) {
        if let Some(crate::popups::ActivePopup::RemoveTags(popup)) = &mut self.popups.active {
            popup.confirm = Some(ConfirmPopup {
                action: crate::popups::ConfirmAction::RemoveAllTagsFromSelected,
                message: "Remove ALL tags from selected notes?".into(),
                detail: Some("This cannot be undone.".into()),
                confirm_label: "Remove All".into(),
                is_destructive: true,
                selected_button: 1,
            });
        }
    }

    pub fn confirm_remove_all_tags(&mut self) {
        self.popups.active = None;

        let note_ids: Vec<String> = self
            .list
            .selected_indices
            .iter()
            .filter_map(|&idx| {
                if let Some(VisualItem::Note { summary_idx, .. }) =
                    self.list.visual_list.get(idx)
                {
                    Some(self.notes[*summary_idx].id.clone())
                } else {
                    None
                }
            })
            .collect();

        let count = note_ids.len();
        let mut failed = 0;
        for note_id in &note_ids {
            if let Ok(mut note) = self.storage.load_note(note_id) {
                note.tags.clear();
                if self.storage.save_note(note_id, &note).is_ok() {
                    self.enqueue_backup(format!("auto: {}", note.title));
                } else {
                    failed += 1;
                }
            }
        }

        self.list.selected_indices.clear();
        self.list.list_mode = crate::list_view::ListMode::Normal;
        self.refresh_visual_list();
        self.clamp_visual_index();
        self.request_notes_reconcile();
        self.set_temporary_status(&format!(
            "Removed all tags from {}/{} note(s)",
            count - failed,
            count
        ));
        if failed > 0 {
            let text = format!("Failed to update tags on {} note(s)", failed);
            self.messages
                .push(text, crate::app::messages::MessageSeverity::Warning);
        }
    }
}
