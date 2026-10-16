use super::*;
use crate::editor::LinkCompletion;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::Input;

/// Max suggestion rows shown in the inline `[[...]]` completion popup.
pub(crate) const LINK_COMPLETE_LIMIT: usize = 8;

/// If char column `col` sits inside an unclosed `[[query` span on `line`,
/// return the anchor column (just after `[[`) and the raw query text.
///
/// The span must start with `[[` on the same line with no closing `]]`
/// before the cursor. A `|` in the query means the cursor is past the
/// alias separator (display text, not a title), so no completion applies.
pub(crate) fn link_query_at_cursor(line: &str, col: usize) -> Option<(usize, String)> {
    let prefix: String = line.chars().take(col).collect();
    let open = prefix.rfind("[[")?;
    let after = &prefix[open + 2..];
    if after.contains("]]") || after.contains('|') {
        return None;
    }
    let anchor_col = prefix[..open + 2].chars().count();
    Some((anchor_col, after.to_string()))
}

/// Filter `(id, title)` candidates by substring match (case-insensitive).
/// Prefix matches rank first (stable); output is capped at `limit`.
pub(crate) fn filter_link_candidates(
    mut all: Vec<(String, String)>,
    query: &str,
    limit: usize,
) -> Vec<(String, String)> {
    let ql = query.to_lowercase();
    if !ql.is_empty() {
        all.retain(|(_, title)| title.to_lowercase().contains(&ql));
        all.sort_by(|a, b| {
            b.1.to_lowercase()
                .starts_with(&ql)
                .cmp(&a.1.to_lowercase().starts_with(&ql))
        });
    }
    all.truncate(limit);
    all
}

impl App {
    /// Refresh the inline `[[...]]` suggestion popup from the body cursor.
    /// Callers must ensure Body focus; clears the popup when the cursor is
    /// not inside an unclosed `[[query` or suggestions are disabled.
    pub fn update_link_completion(&mut self) {
        if !self.config.editor.link_suggestions
            || self.popups.confirm.is_some()
            || self.editor.find_popup.is_some()
            || self.editor.go_to_line_input.is_some()
        {
            self.editor.link_complete = None;
            return;
        }
        let cursor = self.editor.body.cursor();
        let line = match self.editor.body.lines().get(cursor.row) {
            Some(l) => l.clone(),
            None => {
                self.editor.link_complete = None;
                return;
            }
        };
        let Some((anchor_col, query)) = link_query_at_cursor(&line, cursor.col) else {
            self.editor.link_complete = None;
            return;
        };
        let all: Vec<(String, String)> = self
            .visible_notes()
            .map(|(_, n)| (n.id.clone(), n.title.clone()))
            .collect();
        let matches = filter_link_candidates(all, &query, LINK_COMPLETE_LIMIT);
        // Keep the selection when the same note is still listed.
        let prev_id = self
            .editor
            .link_complete
            .as_ref()
            .and_then(|c| c.matches.get(c.selected))
            .map(|(id, _)| id.clone());
        let selected = prev_id
            .and_then(|id| matches.iter().position(|(m_id, _)| m_id == &id))
            .unwrap_or(0);
        self.editor.link_complete = Some(LinkCompletion {
            anchor_line: cursor.row,
            anchor_col,
            query,
            matches,
            selected,
        });
    }

    /// Keep the popup in sync after a body key: content edits re-filter
    /// around the cursor, pure cursor moves dismiss it. A finished link
    /// must not pop suggestions back open, so `Enter` can follow it.
    pub fn sync_link_completion(&mut self, content_changed: bool) {
        if content_changed {
            self.update_link_completion();
        } else {
            self.editor.link_complete = None;
        }
    }

    /// Move the completion selection by `delta` (-1 up, +1 down), clamped.
    pub fn link_complete_move(&mut self, delta: i32) {
        let Some(comp) = self.editor.link_complete.as_mut() else {
            return;
        };
        if comp.matches.is_empty() {
            return;
        }
        let cur = comp.selected as i32;
        let max = comp.matches.len().saturating_sub(1) as i32;
        comp.selected = cur.saturating_add(delta).clamp(0, max) as usize;
    }

    /// Accept the selected suggestion: replace `[[query` with `[[Title]]`.
    /// Reuses an existing `]]` right after the cursor (auto-pairs leaves one
    /// there); otherwise inserts it. Returns `true` when a title was
    /// inserted (key consumed), `false` when the popup was merely dismissed.
    pub fn accept_link_completion(&mut self) -> bool {
        let Some(comp) = self.editor.link_complete.take() else {
            return false;
        };
        if comp.anchor_line != self.editor.body.cursor().row {
            return false;
        }
        let Some((_, title)) = comp.matches.get(comp.selected).cloned() else {
            return false;
        };
        let backspace = Input::from(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        for _ in 0..comp.query.chars().count() {
            self.editor.body.input(backspace.clone());
        }
        self.editor.body.insert_str(title);
        // Skip over an auto-inserted `]]` instead of doubling it.
        let after: String = self
            .editor
            .body
            .lines()
            .get(comp.anchor_line)
            .map(|line| line.chars().skip(self.editor.body.cursor().col).collect())
            .unwrap_or_default();
        if after.starts_with("]]") {
            use ratatui_textarea::CursorMove;
            self.editor.body.move_cursor(CursorMove::Forward);
            self.editor.body.move_cursor(CursorMove::Forward);
        } else {
            self.editor.body.insert_str("]]");
        }
        self.request_editor_preview_update();
        self.write_draft();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_query_basic() {
        assert_eq!(
            link_query_at_cursor("see [[rust", 10),
            Some((6, "rust".to_string()))
        );
    }

    #[test]
    fn link_query_empty_after_brackets() {
        // `[[` just typed (auto-pairs leaves `[[]]` with cursor between).
        assert_eq!(link_query_at_cursor("[[]]", 2), Some((2, String::new())));
    }

    #[test]
    fn link_query_closed_link_is_none() {
        assert_eq!(link_query_at_cursor("[[rust]]", 8), None);
        // Cursor before the opening brackets.
        assert_eq!(link_query_at_cursor("see [[rust", 4), None);
    }

    #[test]
    fn link_query_picks_last_span() {
        assert_eq!(
            link_query_at_cursor("[[a]] and [[b", 13),
            Some((12, "b".to_string()))
        );
    }

    #[test]
    fn link_query_alias_separator_dismisses() {
        assert_eq!(link_query_at_cursor("[[rust|the", 11), None);
    }

    #[test]
    fn link_query_cursor_mid_word() {
        // Cursor in the middle still completes the typed-so-far prefix.
        assert_eq!(
            link_query_at_cursor("[[rust]]", 6),
            Some((2, "rust".to_string()))
        );
    }

    #[test]
    fn link_filter_prefix_first_and_capped() {
        let all = vec![
            ("1".to_string(), "Crust".to_string()),
            ("2".to_string(), "Rust book".to_string()),
            ("3".to_string(), "Unrelated".to_string()),
        ];
        let out = filter_link_candidates(all, "rust", 8);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1, "Rust book");
        let all = (0..20)
            .map(|i| (i.to_string(), format!("note {i}")))
            .collect();
        assert_eq!(filter_link_candidates(all, "", 8).len(), 8);
    }
}
