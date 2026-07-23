use super::*;
use crate::editor::{EditFocus, EditSidebar, LinkItem};

impl App {
    /// Length of the active sidebar list (for bounds clamping).
    pub fn sidebar_len(&self) -> usize {
        match self.editor.sidebar {
            EditSidebar::None => 0,
            EditSidebar::Outline => self.editor.outline_nodes.len(),
            EditSidebar::Links => self.editor.links.len(),
        }
    }

    /// Common sidebar toggle: if `target` is already active, disable it;
    /// otherwise enable it, disable preview, rebuild data, and reset selection.
    fn set_sidebar(&mut self, target: EditSidebar) {
        if self.editor.sidebar == target {
            self.editor.sidebar = EditSidebar::None;
            match target {
                EditSidebar::Outline => self.set_temporary_status_static("Outline pane disabled"),
                EditSidebar::Links => self.set_temporary_status_static("Links pane disabled"),
                EditSidebar::None => {}
            }
        } else {
            self.editor.sidebar = target;
            self.editor.editor_preview_enabled = false;
            self.editor.md_preview_renderer = None;
            match target {
                EditSidebar::Outline => {
                    self.rebuild_outline();
                    self.set_temporary_status_static("Outline pane enabled");
                }
                EditSidebar::Links => {
                    self.editor.links = self.compute_links();
                    self.set_temporary_status_static("Links pane enabled");
                }
                EditSidebar::None => {}
            }
            self.editor.sidebar_selected = 0;
            self.editor.sidebar_scroll_offset = 0;
        }
    }

    pub fn toggle_outline_pane(&mut self) {
        self.set_sidebar(EditSidebar::Outline);
    }

    pub fn toggle_links_pane(&mut self) {
        self.set_sidebar(EditSidebar::Links);
    }

    /// Reparse the current note into header-only outline_nodes.
    pub fn rebuild_outline(&mut self) {
        let title = crate::events::get_title_text(&self.editor.title_editor);
        let content = self.editor.editor.lines().join("\n");
        let all = crate::outline::parse::parse_outline(&title, &content);
        self.editor.outline_nodes = all
            .into_iter()
            .filter(|n| matches!(n.kind, crate::outline::parse::NodeKind::Header { level, .. } if level >= 1))
            .collect();
        // Clamp selection into range.
        let len = self.editor.outline_nodes.len();
        if self.editor.sidebar_selected >= len {
            self.editor.sidebar_selected = len.saturating_sub(1);
        }
    }

    /// Outgoing/Forward links AND Incoming/Backlinks.
    pub fn compute_links(&self) -> Vec<LinkItem> {
        let mut items = Vec::new();

        // 1. Outgoing/Forward links
        let content = self.editor.editor.lines().join("\n");
        let forward_targets = crate::storage::extract_wikilinks(&content);

        let mut forward_notes = Vec::new();
        for target in forward_targets {
            if let Some(id) = self.resolve_wikilink_target(&target)
                && let Some(n) = self.notes.iter().find(|n| n.id == id)
                && !forward_notes.iter().any(|(f_id, _)| f_id == &n.id)
            {
                forward_notes.push((n.id.clone(), n.title.clone()));
            }
        }

        // 2. Incoming/Backlinks
        let cur_id = match &self.editor.editing_id {
            Some(id) => id.clone(),
            None => return items,
        };
        let cur_title = crate::events::get_title_text(&self.editor.title_editor).to_lowercase();
        let mut incoming_notes = Vec::new();
        for n in &self.notes {
            if n.id == cur_id {
                continue;
            }
            let title_hit =
                !cur_title.is_empty() && n.links.iter().any(|l| l.to_lowercase() == cur_title);
            let id_hit = n.links.iter().any(|l| l == &cur_id);
            if (title_hit || id_hit) && !incoming_notes.iter().any(|(i_id, _)| i_id == &n.id) {
                incoming_notes.push((n.id.clone(), n.title.clone()));
            }
        }

        // 3. Combine them
        for (id, title) in forward_notes {
            items.push(LinkItem {
                id,
                title,
                is_backlink: false,
            });
        }
        for (id, title) in incoming_notes {
            items.push(LinkItem {
                id,
                title,
                is_backlink: true,
            });
        }

        items
    }

    /// Move sidebar selection by `delta` (-1 up, +1 down), saturating clamped.
    pub fn sidebar_move(&mut self, delta: i32) {
        let len = self.sidebar_len();
        if len == 0 {
            return;
        }
        let cur = self.editor.sidebar_selected as i32;
        let next = (cur + delta).clamp(0, (len - 1) as i32) as usize;
        self.editor.sidebar_selected = next;
    }

    /// Activate the selected sidebar item. Outline → jump cursor to that line,
    /// return focus to Body. Links → autosave current note, open the linking
    /// or linked note. Returns true if the note was switched.
    pub fn sidebar_activate(&mut self, focus: &mut EditFocus) -> bool {
        match self.editor.sidebar {
            EditSidebar::Outline => {
                if let Some(node) = self.editor.outline_nodes.get(self.editor.sidebar_selected) {
                    let line = node.line.saturating_sub(1) as u16;
                    self.editor
                        .editor
                        .move_cursor(ratatui_textarea::CursorMove::Jump(line, 0));
                    self.request_editor_preview_update();
                }
                *focus = EditFocus::Body;
                false
            }
            EditSidebar::Links => {
                if let Some(item) = self.editor.links.get(self.editor.sidebar_selected).cloned() {
                    self.autosave();
                    self.open_note_at_line(&item.id, None);
                    true
                } else {
                    false
                }
            }
            EditSidebar::None => false,
        }
    }
}

/// If the cursor at char `col` sits within a `[[...]]` span on `line`,
/// return the link target (text before `|`, trimmed). Byte-offset scan
/// mirroring storage::extract_wikilinks, with char-index boundary check.
fn wikilink_at_cursor(line: &str, col: usize) -> Option<String> {
    let mut cursor = 0;
    while let Some(rel) = line[cursor..].find("[[") {
        let open = cursor + rel; // byte offset of first '['
        let inner_start = open + 2;
        let close = match line[inner_start..].find("]]") {
            Some(r) => inner_start + r, // byte offset of first ']'
            None => break,
        };
        let span_end = close + 2; // byte offset after second ']'
        let span_start_col = line[..open].chars().count();
        let span_end_col = line[..span_end].chars().count(); // exclusive
        if col >= span_start_col && col < span_end_col {
            let inner = &line[inner_start..close];
            let target = inner.split('|').next().unwrap_or("").trim();
            return if target.is_empty() {
                None
            } else {
                Some(target.to_string())
            };
        }
        cursor = span_end;
    }
    None
}

impl App {
    /// Resolve a wikilink target string to a note id (title match, lowercased;
    /// fallback exact id). Mirrors graf/graph.rs:78-97 + the backlinks matcher.
    fn resolve_wikilink_target(&self, target: &str) -> Option<String> {
        let lower = target.to_lowercase();
        self.notes
            .iter()
            .find(|n| n.title.to_lowercase() == lower || n.id == target)
            .map(|n| n.id.clone())
    }

    /// Open (or close if already open) the linked-note preview for the wikilink
    /// under the body cursor. No-op + status message if cursor is not on a [[...]].
    pub fn open_link_preview(&mut self) {
        if self.editor.link_preview {
            self.editor.link_preview = false;
            return;
        }
        let cursor = self.editor.editor.cursor();
        let row = cursor.0;
        let col = cursor.1;
        let line = match self.editor.editor.lines().get(row) {
            Some(l) => l.clone(),
            None => return,
        };
        let target = match wikilink_at_cursor(&line, col) {
            Some(t) => t,
            None => {
                self.set_temporary_status_static("No link under cursor");
                return;
            }
        };
        // Reuse an already-rendered preview for the same target.
        if self.editor.link_preview_target.as_deref() == Some(&target)
            && self.editor.link_preview_renderer.is_some()
        {
            self.editor.link_preview = true;
            return;
        }
        let id = self.resolve_wikilink_target(&target);
        let (content, error) = match &id {
            Some(id) => match self.storage.load_note(id) {
                Ok(note) => (Some(note.content), None),
                Err(_) => (None, Some(format!("Could not load: {target}"))),
            },
            None => (None, Some(format!("Note not found: {target}"))),
        };
        if let Some(content) = content {
            // Fixed width matching PopupSize::Large inner on a ~120-col terminal
            // (60% width, max 100, minus borders/padding ≈ 76).
            let width = 76u16;
            let mut renderer = self
                .editor
                .link_preview_renderer
                .take()
                .unwrap_or_else(crate::markdown::MarkdownRenderer::new);
            let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
            let viewport = crate::markdown::RenderViewport {
                start: 0,
                height: 20,
            };
            renderer.render_with(&content, width, &self.app_theme, &opts, viewport);
            self.editor.link_preview_renderer = Some(renderer);
            self.editor.link_preview_error = None;
        } else {
            self.editor.link_preview_renderer = None;
            self.editor.link_preview_error = error;
        }
        self.editor.link_preview_target = Some(target);
        self.editor.link_preview = true;
    }

    pub fn close_link_preview(&mut self) {
        self.editor.link_preview = false;
    }
}
