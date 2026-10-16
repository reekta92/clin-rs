use super::*;

/// Rewrite `[[old]]` targets as `[[new]]`, preserving `|alias` display
/// text and all surrounding bytes. Mirrors the `extract_wikilinks` scan
/// (same `[[` / `]]` pairing, skips targets containing `]`).
/// Returns the rewritten content and the number of links retargeted.
pub(crate) fn retarget_wikilinks(
    content: &str,
    old_title: &str,
    old_id: &str,
    new_title: &str,
) -> (String, usize) {
    let old_lower = old_title.to_lowercase();
    let mut out = String::with_capacity(content.len());
    let mut count = 0;
    let mut cursor = 0;
    while let Some(start_offset) = content[cursor..].find("[[") {
        let absolute_start = cursor + start_offset;
        let inner_start = absolute_start + 2;
        let Some(end_offset) = content[inner_start..].find("]]") else {
            break;
        };
        let absolute_end = inner_start + end_offset;
        let inner = &content[inner_start..absolute_end];
        let (target, alias) = match inner.find('|') {
            Some(pipe) => (&inner[..pipe], Some(&inner[pipe..])),
            None => (inner, None),
        };
        let hit = !target.is_empty()
            && !target.contains(']')
            && (target.trim().to_lowercase() == old_lower || target == old_id);
        out.push_str(&content[cursor..inner_start]);
        if hit {
            count += 1;
            out.push_str(new_title);
            if let Some(alias) = alias {
                out.push_str(alias);
            }
        } else {
            out.push_str(inner);
        }
        cursor = absolute_end;
    }
    out.push_str(&content[cursor..]);
    (out, count)
}

impl App {
    /// `(id, title)` of every note whose links point at `old_title`
    /// (case-insensitive) or `old_id` (exact) — i.e. the notes a rename
    /// would orphan. Includes the renamed note itself (self-links stale too).
    pub fn notes_linking_to(&self, old_title: &str, old_id: &str) -> Vec<(String, String)> {
        let old_lower = old_title.to_lowercase();
        self.notes
            .iter()
            .filter(|n| {
                n.links
                    .iter()
                    .any(|l| l.to_lowercase() == old_lower || l == old_id)
            })
            .map(|n| (n.id.clone(), n.title.clone()))
            .collect()
    }

    /// Offer the rename follow-up: update all `[[old]]` links to `[[new]]`.
    /// Shows a confirm popup when at least one note links here, otherwise a
    /// plain status. Call right after `storage.rename_note` (summaries are
    /// still pre-rename; reconcile is async).
    pub fn offer_backlink_update(
        &mut self,
        old_title: String,
        old_id: String,
        new_title: String,
        new_id: String,
    ) {
        // `new_id` is unused: retargeted links point at the new *title*.
        let _ = new_id;
        let sources = self.notes_linking_to(&old_title, &old_id);
        if sources.is_empty() {
            self.set_temporary_status_static("Note renamed");
            return;
        }
        let note_ids = sources.iter().map(|(id, _)| id.clone()).collect();
        self.show_confirm(crate::popups::ConfirmAction::UpdateBacklinks {
            old_title,
            old_id,
            new_title,
            note_ids,
        });
    }

    /// Confirmed `ConfirmAction::UpdateBacklinks`: rewrite the link target
    /// in every affected note, then reconcile and report.
    pub fn confirm_update_backlinks(
        &mut self,
        old_title: String,
        old_id: String,
        new_title: String,
        note_ids: Vec<String>,
    ) {
        let mut updated_notes = 0;
        let mut updated_links = 0;
        let mut failed = 0;
        for id in &note_ids {
            // The open note must be patched in its live buffer: a disk
            // rewrite would be clobbered by the next autosave (or leave
            // the editor showing stale text). Autosave persists it after.
            if Some(id) == self.editor.editing_id.as_ref() {
                let cursor = self.editor.body.cursor();
                let text = self.editor.body.text();
                let (new_text, n) = retarget_wikilinks(&text, &old_title, &old_id, &new_title);
                if n == 0 {
                    continue;
                }
                self.editor.body = crate::editor_document::EditorDocument::from_text(&new_text);
                let max_row = self.editor.body.line_count().saturating_sub(1);
                let row = cursor.row.min(max_row);
                let max_col = self
                    .editor
                    .body
                    .lines()
                    .get(row)
                    .map(|l| l.chars().count())
                    .unwrap_or(0);
                self.editor
                    .body
                    .move_cursor(ratatui_textarea::CursorMove::Jump(
                        row as u16,
                        cursor.col.min(max_col) as u16,
                    ));
                self.editor.link_complete = None;
                self.request_editor_preview_update();
                self.write_draft();
                updated_notes += 1;
                updated_links += n;
                continue;
            }
            match self.storage.load_note(id) {
                Ok(mut note) => {
                    let (content, n) =
                        retarget_wikilinks(&note.content, &old_title, &old_id, &new_title);
                    if n == 0 {
                        continue;
                    }
                    note.content = content;
                    note.updated_at = crate::ui::now_unix_secs();
                    match self.storage.save_note(id, &note) {
                        Ok(_) => {
                            updated_notes += 1;
                            updated_links += n;
                        }
                        Err(_) => failed += 1,
                    }
                }
                Err(_) => failed += 1,
            }
        }
        self.request_notes_reconcile();
        if failed > 0 {
            self.set_temporary_status(&format!(
                "Updated {updated_links} link(s) in {updated_notes} note(s); {failed} failed"
            ));
        } else {
            self.set_temporary_status(&format!(
                "Updated {updated_links} link(s) in {updated_notes} note(s)"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retarget_basic_and_alias() {
        let (out, n) = retarget_wikilinks("see [[dorime]] and [[dorime|D]]!", "dorime", "x", "new");
        assert_eq!(out, "see [[new]] and [[new|D]]!");
        assert_eq!(n, 2);
    }

    #[test]
    fn retarget_case_insensitive_target() {
        let (out, n) = retarget_wikilinks("[[Dorime]]", "dorime", "x", "new name");
        assert_eq!(out, "[[new name]]");
        assert_eq!(n, 1);
    }

    #[test]
    fn retarget_id_links_and_skips_unrelated() {
        let (out, n) = retarget_wikilinks(
            "[[folder/dorime]] plus [[other]]",
            "dorime",
            "folder/dorime",
            "n",
        );
        assert_eq!(out, "[[n]] plus [[other]]");
        assert_eq!(n, 1);
    }

    #[test]
    fn retarget_ignores_unclosed_and_bracket_targets() {
        let (out, n) = retarget_wikilinks("[[dorime and [[a]b]]", "dorime", "x", "n");
        assert_eq!(out, "[[dorime and [[a]b]]");
        assert_eq!(n, 0);
    }
}
