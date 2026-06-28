use super::*;
use crate::debug_log;
use crate::list_view::*;
use crate::popups::*;
use ratatui_textarea::TextArea;
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

            let mut input = TextArea::default();
            input.set_cursor_line_style(ratatui::style::Style::default());
            input.set_style(self.app_theme.bg_style());
            input.set_placeholder_text("Add tags...");
            input.insert_str(current_tags.join(", "));

            self.popups.active = Some(crate::popups::ActivePopup::Tag(TagPopup {
                note_id: note.id.clone(),
                input,
                all_tags,
                suggestions: Vec::new(),
                suggestion_index: 0,
                focus: crate::popups::TagPopupFocus::Input,
                all_tags_selected: 0,
            }));
            self.update_tag_suggestions();
        } else {
            self.set_temporary_status_static("Select a note to manage tags");
        }
    }

    pub fn confirm_manage_tags(&mut self) {
        if let Some(crate::popups::ActivePopup::Tag(popup)) = self.popups.active.take() {
            let text = popup.input.lines().join("");
            let tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

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
                } else {
                    self.enqueue_backup(format!("auto: {}", &note.title));
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
                    self.set_temporary_status_static("Tags updated");
                }
            } else {
                self.set_temporary_status_static("Failed to load note to update tags");
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

    pub fn begin_delete_tag_with_name(&mut self, tag: String) {
        let count = self
            .storage
            .list_note_ids(self.list.show_hidden_files)
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
        if let Ok(note_ids) = self.storage.list_note_ids(self.list.show_hidden_files) {
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
                    }
                    count += 1;
                }
            }
        }

        debug_log!(self, Info, "storage", "Tag deleted: {tag}");
        self.set_temporary_status(&format!("Deleted '{tag}' from {count} note(s)"));
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
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

    pub fn apply_tag_to_selected(&mut self, tag: String) {
        let mut count = 0;
        let indices: Vec<usize> = self.list.selected_indices.iter().copied().collect();

        for &idx in &indices {
            if let Some(crate::list_view::VisualItem::Note { summary_idx, .. }) =
                self.list.visual_list.get(idx)
            {
                let note = &self.notes[*summary_idx];
                let note_id = note.id.clone();
                let note_title = note.title.clone();
                let ext = std::path::Path::new(&note_id)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                if ext != "md" && ext != "txt" && ext != "clin" {
                    continue;
                }

                if let Ok(mut loaded) = self.storage.load_note(&note_id) {
                    if !loaded.tags.contains(&tag) {
                        loaded.tags.push(tag.clone());
                    }
                    if self.storage.save_note(&note_id, &loaded).is_ok() {
                        self.enqueue_backup(format!("auto: {}", &note_title));
                        count += 1;
                    }
                }
            }
        }

        self.list.selected_indices.clear();
        self.list.list_mode = crate::list_view::ListMode::Normal;

        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
            return;
        }

        self.set_temporary_status(&format!("Tag '{tag}' applied to {count} note(s)"));
    }
}
