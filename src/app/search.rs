use std::time::{Duration, Instant};
use super::*;
use crate::list_view::*;
use crate::popups::*;

impl App {
    /// In grid layout, cycle between Vault, Pinned, and Smart tabs.
    pub fn cycle_grid_tab(&mut self) {
        if self.list.notes_layout != crate::config::NotesLayout::Grid {
            return;
        }
        self.list.grid_folder = if self.config.list.smart_folders_enabled {
            if self.list.grid_folder == VIRTUAL_PINNED_PATH {
                VIRTUAL_SMART_PATH.to_string()
            } else if self.list.grid_folder == VIRTUAL_SMART_PATH
                || self.list.grid_folder.starts_with('@')
            {
                VIRTUAL_SUBNOTES_PATH.to_string()
            } else if self.list.grid_folder == VIRTUAL_SUBNOTES_PATH
                || Self::is_subnotes_parent_grid_path(&self.list.grid_folder)
            {
                String::new()
            } else {
                VIRTUAL_PINNED_PATH.to_string()
            }
        } else {
            if self.list.grid_folder == VIRTUAL_PINNED_PATH {
                VIRTUAL_SUBNOTES_PATH.to_string()
            } else if self.list.grid_folder == VIRTUAL_SUBNOTES_PATH
                || Self::is_subnotes_parent_grid_path(&self.list.grid_folder)
            {
                String::new()
            } else {
                VIRTUAL_PINNED_PATH.to_string()
            }
        };
        self.list.visual_index = 0;
        self.refresh_visual_list();
    }

    pub fn begin_search(&mut self) {
        let input = crate::ui::make_popup_textarea(&self.app_theme, "Search notes...");

        self.popups.active = Some(crate::popups::ActivePopup::Search(SearchPopup {
            input,
            focus: crate::popups::SearchFocus::Input,
            title_result_ids: Vec::new(),
            title_selected: 0,
            grep_results: Vec::new(),
            grep_row_offsets: Vec::new(),
            grep_expanded: std::collections::HashSet::new(),
            grep_selected: 0,
            globally_truncated: false,
            read_errors: 0,
            results_scroll_offset: 0,
            original_index: self.list.visual_index,
            original_folder_expanded: self.list.folder_expanded.clone(),
            last_scroll: None,
        }));
    }

    fn jump_to_note_index(&mut self, note_idx: usize) {
        if let Some(note) = self.notes.get(note_idx)
            && !note.folder.is_empty()
        {
            let mut path = String::new();
            for part in note.folder.split('/') {
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(part);
                self.list.folder_expanded.insert(path.clone());
            }
        }

        self.refresh_visual_list();

        for (idx, item) in self.list.visual_list.iter().enumerate() {
            if let VisualItem::Note { summary_idx, .. } = item
                && *summary_idx == note_idx
            {
                self.list.visual_index = idx;
                self.request_preview_update();
                return;
            }
        }
    }

    pub fn update_search(&mut self) {
        let Some(crate::popups::ActivePopup::Search(popup)) = &self.popups.active else {
            return;
        };
        let query_text = popup.input.lines().join("");
        let parsed = parse_search_query(&query_text);
        let title_query = parsed.text.trim().to_lowercase();
        let grep_query = parsed.grep_text.trim().to_lowercase();

        let no_filters = title_query.is_empty()
            && grep_query.is_empty()
            && parsed.folder_filter.is_none()
            && !parsed.pinned_only
            && parsed.tag_filter.is_none();
        if no_filters {
            if let Some(crate::popups::ActivePopup::Search(popup)) = &mut self.popups.active {
                popup.title_result_ids.clear();
                popup.title_selected = 0;
                popup.grep_results.clear();
                popup.grep_row_offsets.clear();
                popup.grep_expanded.clear();
                popup.grep_selected = 0;
                popup.globally_truncated = false;
                popup.read_errors = 0;
            }
            self.search_status = None;
            return;
        }

        let mut candidate_ids: Vec<Arc<str>> = Vec::new();
        let mut title_result_ids: Vec<Arc<str>> = Vec::new();

        for note in &self.notes {
            if parsed.pinned_only && !note.pinned {
                continue;
            }

            if let Some(ref folder) = parsed.folder_filter {
                let matches_folder = if folder.is_empty() {
                    note.folder.is_empty()
                } else {
                    note.folder == *folder || note.folder.starts_with(&format!("{folder}/"))
                };
                if !matches_folder {
                    continue;
                }
            }

            if let Some(ref tags) = parsed.tag_filter
                && !tags.is_empty()
            {
                let note_tags: Vec<String> = note.tags.iter().map(|t| t.to_lowercase()).collect();
                let matches_tag = tags.iter().any(|t| note_tags.contains(t));
                if !matches_tag {
                    continue;
                }
            }

            let matched_title = title_query.is_empty()
                || note.title.to_lowercase().contains(&title_query)
                || note.id.to_lowercase().contains(&title_query);

            let note_id_arc: Arc<str> = Arc::from(note.id.as_str());

            if matched_title {
                title_result_ids.push(note_id_arc.clone());
            }

            if !grep_query.is_empty() && matched_title {
                candidate_ids.push(note_id_arc);
            }
        }

        if let Some(crate::popups::ActivePopup::Search(popup)) = &mut self.popups.active {
            popup.title_result_ids = title_result_ids;
            if popup.title_selected >= popup.title_result_ids.len() {
                popup.title_selected = popup.title_result_ids.len().saturating_sub(1);
            }
        }

        if !grep_query.is_empty() {
            let gen_num = self.search_query_generation.fetch_add(1, Ordering::SeqCst) + 1;
            self.search_status = Some("Searching…".to_string());
            self.search_debounce_deadline = Some(Instant::now() + Duration::from_millis(150));
            self.unsent_search_request = Some(crate::app::search_worker::SearchRequest {
                generation: gen_num,
                query: grep_query,
                candidate_ids: candidate_ids.into_boxed_slice(),
            });
            if let Some(crate::popups::ActivePopup::Search(popup)) = &mut self.popups.active {
                popup.grep_results.clear();
                popup.grep_row_offsets.clear();
                popup.grep_expanded.clear();
                popup.grep_selected = 0;
                popup.globally_truncated = false;
                popup.read_errors = 0;
            }
            self.search_status = None;
            self.search_debounce_deadline = None;
            self.unsent_search_request = None;
        }
    }

    pub fn confirm_search(&mut self) {
        self.popups.active = None;
    }

    pub fn jump_to_selected_result(&mut self) {
        if let Some(crate::popups::ActivePopup::Search(popup)) = &self.popups.active {
            let mut target_line = None;
            let mut target_id = None;

            if popup.focus == crate::popups::SearchFocus::Results {
                let has_grep = !popup.grep_results.is_empty();
                if has_grep {
                    if popup.grep_selected < popup.total_grep_rows() {
                        let hit_idx = match popup.grep_row_offsets.binary_search(&popup.grep_selected) {
                            Ok(i) => i,
                            Err(i) => i.saturating_sub(1),
                        };
                        let base = popup.grep_row_offsets.get(hit_idx).copied().unwrap_or(0);
                        if let Some(hit) = popup.grep_results.get(hit_idx) {
                            target_id = Some(hit.note_id.to_string());
                            if popup.grep_selected > base {
                                let line_idx = popup.grep_selected - base - 1;
                                if let Some(line_hit) = hit.lines.get(line_idx) {
                                    target_line = Some(line_hit.line_number);
                                }
                            }
                        }
                    }
                } else if !popup.title_result_ids.is_empty() {
                    target_id = popup.title_result_ids.get(popup.title_selected).map(|id| id.to_string());
                }
            }

            if let Some(id) = target_id {
                if let Some(idx) = self.notes.iter().position(|n| n.id == id) {
                    self.jump_to_note_index(idx);
                    self.open_note_at_line(&id, target_line);
                }
            }
        }
    }

    pub fn handle_search_events(&mut self) {
        use crate::app::search_worker::SearchEvent;
        let cur_gen = self.search_query_generation.load(Ordering::SeqCst);

        if let Some(deadline) = self.search_debounce_deadline {
            if Instant::now() >= deadline {
                self.search_debounce_deadline = None;
                if let Some(req) = self.unsent_search_request.take() {
                    if req.generation == cur_gen {
                        let _ = self.search_worker.req_tx.try_send(req);
                    }
                }
            }
        }

        while let Ok(event) = self.search_worker.event_rx.try_recv() {
            match event {
                SearchEvent::Batch {
                    generation,
                    hits,
                    finished,
                    errors,
                    globally_truncated,
                } => {
                    if generation == cur_gen {
                        if let Some(crate::popups::ActivePopup::Search(popup)) = &mut self.popups.active {
                            popup.grep_results.extend(hits);
                            popup.globally_truncated = globally_truncated;
                            popup.read_errors = errors;
                            popup.rebuild_grep_offsets();
                            if popup.grep_selected >= popup.total_grep_rows() {
                                popup.grep_selected = popup.total_grep_rows().saturating_sub(1);
                            }
                        }
                        if finished {
                            self.search_status = None;
                            if errors > 0 {
                                self.set_temporary_status(&format!("Search completed with {errors} read error(s)"));
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn cancel_search(&mut self) {
        if let Some(crate::popups::ActivePopup::Search(popup)) = self.popups.active.take() {
            self.list.visual_index = popup.original_index;
            self.list.folder_expanded = popup.original_folder_expanded;
            self.refresh_visual_list();
            if !self.list.visual_list.is_empty() {
                self.list.visual_index = self
                    .list
                    .visual_index
                    .min(self.list.visual_list.len().saturating_sub(1));
            }
            self.request_preview_update();
        }
    }

    pub fn jump_to_top(&mut self) {
        self.jump_to(None, true);
    }

    pub fn jump_to_bottom(&mut self) {
        self.jump_to(None, false);
    }

    /// Jump to list top (top=true) or bottom (top=false). With a count, jump to
    /// absolute 0-based index `count - 1` instead (vim `nG`/`ngg` parity).
    pub fn jump_to(&mut self, count: Option<u32>, top: bool) {
        let len = self.list.visual_list.len();
        let idx = match count {
            Some(c) => (c as usize).saturating_sub(1).min(len.saturating_sub(1)),
            None if top => 0,
            None => len.saturating_sub(1),
        };
        self.list.visual_index = idx;
        self.request_preview_update();
    }

    /// True when `idx` is a real note/folder item selectable in Select mode.
    /// `CreateNew` is excluded so bulk-tag/selection never lands on the
    /// "Create a new note..." sentinel row.
    pub fn is_selectable_index(&self, idx: usize) -> bool {
        !matches!(
            self.list.visual_list.get(idx),
            Some(crate::list_view::VisualItem::CreateNew { .. })
        )
    }

    /// Single-step list directional movements (grid-aware).
    pub fn move_up(&mut self) {
        let is_grid = self.list.notes_layout == crate::config::NotesLayout::Grid;
        let cols = if is_grid {
            self.list.grid_columns.max(1)
        } else {
            1
        };
        if is_grid {
            if self.list.visual_index >= cols {
                self.list.visual_index -= cols;
                if self.list.list_mode == crate::list_view::ListMode::Select
                    && !self.is_selectable_index(self.list.visual_index)
                    && self.list.visual_index >= cols
                {
                    self.list.visual_index -= cols;
                }
            }
            self.request_preview_update();
        } else if self.list.visual_index > 0 {
            self.list.visual_index -= 1;
            if self.list.list_mode == crate::list_view::ListMode::Select
                && !self.is_selectable_index(self.list.visual_index)
                && self.list.visual_index > 0
            {
                self.list.visual_index -= 1;
            }
            self.request_preview_update();
        }
    }

    pub fn move_down(&mut self) {
        let is_grid = self.list.notes_layout == crate::config::NotesLayout::Grid;
        let cols = if is_grid {
            self.list.grid_columns.max(1)
        } else {
            1
        };
        let len = self.list.visual_list.len();
        let in_select = self.list.list_mode == crate::list_view::ListMode::Select;
        if is_grid {
            let next = self.list.visual_index + cols;
            if next < len && (!in_select || self.is_selectable_index(next)) {
                self.list.visual_index = next;
            } else if self.list.visual_index / cols < (len.saturating_sub(1)) / cols {
                self.list.visual_index = len.saturating_sub(1);
            }
            self.request_preview_update();
        } else if self.list.visual_index < len.saturating_sub(1)
            && (!in_select || self.is_selectable_index(self.list.visual_index + 1))
        {
            self.list.visual_index += 1;
            self.request_preview_update();
        }
    }

    pub fn move_left(&mut self) {
        let is_grid = self.list.notes_layout == crate::config::NotesLayout::Grid;
        if is_grid {
            self.list.visual_index = self.list.visual_index.saturating_sub(1);
            self.request_preview_update();
        } else {
            self.collapse_selected_folder();
        }
    }

    pub fn move_right(&mut self) {
        let is_grid = self.list.notes_layout == crate::config::NotesLayout::Grid;
        let len = self.list.visual_list.len();
        if is_grid {
            if len > 0 {
                self.list.visual_index = self
                    .list
                    .visual_index
                    .saturating_add(1)
                    .min(len.saturating_sub(1));
            }
            self.request_preview_update();
        } else {
            self.expand_selected_folder();
        }
    }

    pub fn page_up(&mut self) {
        self.list.visual_index = self.list.visual_index.saturating_sub(self.list.page_size);
        self.request_preview_update();
    }

    pub fn page_down(&mut self) {
        let max_index = self.list.visual_list.len().saturating_sub(1);
        self.list.visual_index = (self.list.visual_index + self.list.page_size).min(max_index);
        self.request_preview_update();
    }
}
