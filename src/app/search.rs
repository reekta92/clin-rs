use super::*;
use crate::list_view::*;
use crate::popups::*;
use ratatui_textarea::TextArea;

impl App {


    /// In grid layout, cycle between Pinned and Vault tabs.
    pub fn cycle_grid_tab(&mut self) {
        if self.list.notes_layout != crate::config::NotesLayout::Grid {
            return;
        }
        self.list.grid_folder = if self.list.grid_folder == VIRTUAL_PINNED_PATH {
            String::new()
        } else {
            VIRTUAL_PINNED_PATH.to_string()
        };
        self.list.visual_index = 0;
        self.refresh_visual_list();
    }

    pub fn begin_search(&mut self) {
        let mut input = TextArea::default();
        input.set_style(self.app_theme.bg_style());
        input.set_cursor_line_style(Style::default());
        input.set_placeholder_text("Search notes...");

        self.popups.search = Some(SearchPopup {
            input,
            focus: crate::popups::SearchFocus::Input,
            title_results: Vec::new(),
            title_result_indices: Vec::new(),
            title_selected: 0,
            grep_results: Vec::new(),
            grep_result_indices: Vec::new(),
            grep_is_header: Vec::new(),
            grep_expanded: std::collections::HashSet::new(),
            grep_selected: 0,
            original_index: self.list.visual_index,
            original_folder_expanded: self.list.folder_expanded.clone(),
        });
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
        let Some(popup) = self.popups.search.as_ref() else {
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
            if let Some(popup) = &mut self.popups.search {
                popup.title_results.clear();
                popup.title_result_indices.clear();
                popup.title_selected = 0;
                popup.grep_results.clear();
                popup.grep_result_indices.clear();
                popup.grep_is_header.clear();
                popup.grep_expanded.clear();
                popup.grep_selected = 0;
            }
            return;
        }

        let mut title_results = Vec::new();
        let mut title_result_indices = Vec::new();
        let mut grep_results = Vec::new();
        let mut grep_result_indices = Vec::new();
        let mut grep_is_header = Vec::new();

        for (note_idx, note) in self.notes.iter().enumerate() {
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

            let content_opt = if !grep_query.is_empty() {
                self.storage.load_note(&note.id).ok()
            } else {
                None
            };
            let matched_grep = grep_query.is_empty()
                || content_opt
                    .as_ref()
                    .is_some_and(|n| n.content.to_lowercase().contains(&grep_query));

            let label = if note.folder.is_empty() {
                note.title.clone()
            } else {
                format!("{}/{}", note.folder, note.title)
            };
            let lock_prefix = if note.id.ends_with(".clin") {
                "\u{f023} "
            } else {
                ""
            };
            let tags_str = if note.tags.is_empty() {
                String::new()
            } else {
                format!(
                    " [{}]",
                    note.tags
                        .iter()
                        .map(|t| t.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };

            if !title_query.is_empty() && matched_title {
                title_results.push(format!("{lock_prefix}{label}{tags_str}"));
                title_result_indices.push(note_idx);
            }

            if !grep_query.is_empty()
                && matched_grep
                && matched_title
                && let Some(note_data) =
                    content_opt.filter(|n| n.content.to_lowercase().contains(&grep_query))
            {
                let match_count = note_data
                    .content
                    .lines()
                    .filter(|l| l.to_lowercase().contains(&grep_query))
                    .count();
                grep_results.push(format!(" {lock_prefix}{label}{tags_str} ({match_count})"));
                grep_result_indices.push(note_idx);
                grep_is_header.push(true);

                for (line_no, line) in note_data
                    .content
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.to_lowercase().contains(&grep_query))
                {
                    let trimmed = line.trim();
                    let snippet: String = if trimmed.chars().count() > 56 {
                        trimmed.chars().take(56).collect::<String>() + "…"
                    } else {
                        trimmed.to_string()
                    };
                    grep_results.push(format!("  L{}: {}", line_no + 1, snippet));
                    grep_result_indices.push(note_idx);
                    grep_is_header.push(false);
                }
            }

            if title_query.is_empty()
                && grep_query.is_empty()
                && (parsed.folder_filter.is_some()
                    || parsed.pinned_only
                    || parsed.tag_filter.is_some())
            {
                let tags_str = if note.tags.is_empty() {
                    String::new()
                } else {
                    format!(
                        "  [{}]",
                        note.tags
                            .iter()
                            .map(|t| t.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                };
                title_results.push(format!("{lock_prefix}{label}{tags_str}"));
                title_result_indices.push(note_idx);
            }
        }

        if let Some(popup) = &mut self.popups.search {
            popup.title_results = title_results;
            popup.title_result_indices = title_result_indices;
            if popup.title_selected >= popup.title_results.len() {
                popup.title_selected = popup.title_results.len().saturating_sub(1);
            }
            popup.grep_results = grep_results;
            popup.grep_result_indices = grep_result_indices;
            popup.grep_is_header = grep_is_header;
            if popup.grep_selected >= popup.grep_results.len() {
                popup.grep_selected = popup.grep_results.len().saturating_sub(1);
            }
        }
    }

    pub fn confirm_search(&mut self) {
        self.popups.search = None;
    }

    pub fn jump_to_selected_result(&mut self) {
        if let Some(popup) = &self.popups.search {
            let mut target_line = None;
            let note_idx = match popup.focus {
                crate::popups::SearchFocus::Results => {
                    let has_grep = !popup.grep_results.is_empty();
                    if has_grep {
                        let is_header = popup
                            .grep_is_header
                            .get(popup.grep_selected)
                            .copied()
                            .unwrap_or(false);
                        if !is_header
                            && let Some(line_str) = popup.grep_results.get(popup.grep_selected)
                            && let Some(l_pos) = line_str.as_str().find('L')
                            && let Some(colon_pos) = line_str.as_str().find(':')
                            && colon_pos > l_pos + 1
                            && let Ok(num) = line_str[l_pos + 1..colon_pos].trim().parse::<usize>()
                        {
                            target_line = Some(num);
                        }
                        popup.grep_result_indices.get(popup.grep_selected).copied()
                    } else {
                        popup
                            .title_result_indices
                            .get(popup.title_selected)
                            .copied()
                    }
                }
                crate::popups::SearchFocus::Input => None,
            };
            if let Some(idx) = note_idx {
                self.jump_to_note_index(idx);
                if let Some(note) = self.notes.get(idx) {
                    let id = note.id.clone();
                    self.open_note_at_line(&id, target_line);
                }
            }
        }
    }

    pub fn cancel_search(&mut self) {
        if let Some(popup) = self.popups.search.take() {
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
        self.list.visual_index = 0;
        self.request_preview_update();
    }

    pub fn jump_to_bottom(&mut self) {
        self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        self.request_preview_update();
    }

    pub fn page_up(&mut self) {
        self.list.visual_index = self.list.visual_index.saturating_sub(self.list.page_size);
        self.request_preview_update();
    }

    pub fn page_down(&mut self) {
        let max_index = self.list.visual_list.len().saturating_sub(1);
        self.list.visual_index = (self.list.visual_index + self.list.page_size).min(max_index);
        self.request_preview_update();
    }}
