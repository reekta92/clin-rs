use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui_textarea::{CursorMove, Input, TextArea, WrapMode};
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TextPosition {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DocumentChange {
    Lines {
        old: Range<usize>,
        new: Range<usize>,
    },
    Full,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EditEffect {
    pub content_changed: bool,
    pub cursor_changed: bool,
}

/// Sole owner of note-body editing state.
///
/// `TextArea` remains implementation detail until measured buffer cutover.  The
/// wrapper gives callers revision/change semantics instead of backend access.
pub(crate) struct EditorDocument {
    textarea: TextArea<'static>,
    revision: u64,
    snapshot: Option<Arc<str>>,
    change: Option<DocumentChange>,
}

impl Default for EditorDocument {
    fn default() -> Self {
        Self::from_text("")
    }
}

impl EditorDocument {
    pub(crate) fn from_text(content: &str) -> Self {
        let normalized = normalize_content(content);
        Self {
            textarea: TextArea::from(normalized.lines().map(String::from).collect::<Vec<_>>()),
            revision: 0,
            snapshot: Some(Arc::from(normalized)),
            change: Some(DocumentChange::Full),
        }
    }
    #[allow(dead_code)]
    pub(crate) fn from_lines(lines: impl IntoIterator<Item = String>) -> Self {
        let textarea = TextArea::from(lines.into_iter().collect::<Vec<_>>());
        Self {
            textarea,
            revision: 0,
            snapshot: None,
            change: Some(DocumentChange::Full),
        }
    }
    #[allow(dead_code)]
    pub(crate) fn replace_text(&mut self, content: &str) -> EditEffect {
        let old_len = self.line_count();
        let normalized = normalize_content(content);
        if self.text() == normalized {
            return EditEffect::default();
        }
        self.textarea = TextArea::from(normalized.lines().map(String::from).collect::<Vec<_>>());
        self.record_change(DocumentChange::Lines {
            old: 0..old_len,
            new: 0..self.line_count(),
        });
        EditEffect {
            content_changed: true,
            cursor_changed: true,
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn snapshot(&mut self) -> Arc<str> {
        if let Some(snapshot) = &self.snapshot {
            return Arc::clone(snapshot);
        }
        let snapshot: Arc<str> = Arc::from(self.text());
        self.snapshot = Some(Arc::clone(&snapshot));
        snapshot
    }

    pub(crate) fn text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub(crate) fn lines(&self) -> &[String] {
        self.textarea.lines()
    }

    pub(crate) fn line_count(&self) -> usize {
        self.textarea.lines().len()
    }
    #[allow(dead_code)]
    pub(crate) fn line(&self, row: usize) -> Option<&str> {
        self.textarea.lines().get(row).map(String::as_str)
    }

    pub(crate) fn cursor(&self) -> TextPosition {
        let cursor = self.textarea.cursor();
        TextPosition {
            row: cursor.0,
            col: cursor.1,
        }
    }

    pub(crate) fn selection_range(&self) -> Option<(TextPosition, TextPosition)> {
        self.textarea.selection_range().map(|(start, end)| {
            (
                TextPosition {
                    row: start.0,
                    col: start.1,
                },
                TextPosition {
                    row: end.0,
                    col: end.1,
                },
            )
        })
    }

    pub(crate) fn input(&mut self, input: Input) -> EditEffect {
        self.mutate(|textarea| textarea.input(input))
    }
    #[allow(dead_code)]
    pub(crate) fn input_key(&mut self, key: KeyEvent) -> EditEffect {
        self.input(Input::from(key))
    }

    pub(crate) fn insert_str(&mut self, text: impl AsRef<str>) -> EditEffect {
        self.mutate(|textarea| textarea.insert_str(text))
    }
    #[allow(dead_code)]
    pub(crate) fn delete_str(&mut self, chars: usize) -> EditEffect {
        self.mutate(|textarea| textarea.delete_str(chars))
    }

    pub(crate) fn cut(&mut self) -> EditEffect {
        self.mutate(TextArea::cut)
    }

    pub(crate) fn paste(&mut self) -> EditEffect {
        self.mutate(TextArea::paste)
    }

    pub(crate) fn undo(&mut self) -> EditEffect {
        self.mutate(TextArea::undo)
    }

    pub(crate) fn redo(&mut self) -> EditEffect {
        self.mutate(TextArea::redo)
    }

    pub(crate) fn move_cursor(&mut self, movement: CursorMove) -> EditEffect {
        let before = self.textarea.cursor();
        self.textarea.move_cursor(movement);
        EditEffect {
            content_changed: false,
            cursor_changed: self.textarea.cursor() != before,
        }
    }

    pub(crate) fn scroll(&mut self, scrolling: (i16, i16)) -> EditEffect {
        let before = self.textarea.cursor();
        self.textarea.scroll(scrolling);
        EditEffect {
            content_changed: false,
            cursor_changed: self.textarea.cursor() != before,
        }
    }

    pub(crate) fn delete_word(&mut self) -> EditEffect {
        self.mutate(TextArea::delete_word)
    }

    pub(crate) fn delete_next_word(&mut self) -> EditEffect {
        self.mutate(TextArea::delete_next_word)
    }

    pub(crate) fn start_selection(&mut self) {
        self.textarea.start_selection();
    }

    pub(crate) fn cancel_selection(&mut self) {
        self.textarea.cancel_selection();
    }

    pub(crate) fn select_all(&mut self) {
        self.textarea.select_all();
    }

    pub(crate) fn copy(&mut self) {
        self.textarea.copy();
    }

    pub(crate) fn yank_text(&self) -> String {
        self.textarea.yank_text()
    }

    pub(crate) fn set_wrap_mode(&mut self, mode: WrapMode) {
        self.textarea.set_wrap_mode(mode);
    }

    pub(crate) fn take_change(&mut self) -> Option<DocumentChange> {
        self.change.take()
    }
    #[allow(dead_code)]
    pub(crate) fn set_yank_text(&mut self, text: impl Into<String>) {
        self.textarea.set_yank_text(text);
    }

    pub(crate) fn textarea(&self) -> &TextArea<'static> {
        &self.textarea
    }

    pub(crate) fn inner_rect(&self, area: Rect) -> Rect {
        self.textarea
            .block()
            .map(|block| block.inner(area))
            .unwrap_or(area)
    }

    pub(crate) fn textarea_mut(&mut self) -> &mut TextArea<'static> {
        &mut self.textarea
    }

    pub(crate) fn hit_test_cursor(
        &mut self,
        area: Rect,
        column: u16,
        row: u16,
        scroll_row: u16,
        scroll_col: u16,
    ) -> EditEffect {
        let before = self.textarea.cursor();
        crate::events::move_textarea_cursor_to_mouse(
            &mut self.textarea,
            area,
            column,
            row,
            usize::from(scroll_row),
            usize::from(scroll_col),
        );
        EditEffect {
            content_changed: false,
            cursor_changed: self.textarea.cursor() != before,
        }
    }

    fn mutate(&mut self, f: impl FnOnce(&mut TextArea<'static>) -> bool) -> EditEffect {
        let before_cursor = self.textarea.cursor();
        let before_selection = self.textarea.selection_range();
        let before_line_count = self.line_count();
        let content_changed = f(&mut self.textarea);
        let cursor_changed = self.textarea.cursor() != before_cursor;
        if content_changed {
            let (start, old_end) = before_selection
                .map(|(start, end)| (start.0, end.0.saturating_add(1)))
                .unwrap_or((before_cursor.0, before_cursor.0.saturating_add(1)));
            let removed_lines = old_end.saturating_sub(start);
            let preserved_lines = before_line_count.saturating_sub(removed_lines);
            let added_lines = self.line_count().saturating_sub(preserved_lines);
            self.record_change(DocumentChange::Lines {
                old: start..old_end,
                new: start..start.saturating_add(added_lines),
            });
        }
        EditEffect {
            content_changed,
            cursor_changed,
        }
    }

    fn record_change(&mut self, change: DocumentChange) {
        self.revision = self.revision.wrapping_add(1);
        self.snapshot = None;
        self.change = Some(compose_change(self.change.take(), change));
    }
}

fn normalize_content(content: &str) -> String {
    content
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .to_string()
}

fn compose_change(previous: Option<DocumentChange>, next: DocumentChange) -> DocumentChange {
    match (previous, next) {
        (None, next) => next,
        (Some(DocumentChange::Full), _) | (_, DocumentChange::Full) => DocumentChange::Full,
        (Some(DocumentChange::Lines { old, .. }), DocumentChange::Lines { new, .. }) => {
            DocumentChange::Lines { old, new }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_parity(document: &EditorDocument, textarea: &TextArea<'static>) {
        assert_eq!(document.text(), textarea.lines().join("\n"));
        let expected = textarea.cursor();
        assert_eq!(
            document.cursor(),
            TextPosition {
                row: expected.0,
                col: expected.1,
            }
        );
        assert_eq!(
            document.selection_range().is_some(),
            textarea.selection_range().is_some()
        );
    }

    #[test]
    fn editor_document_textarea_parity() {
        let content = "first\tαβγ e\u{301} 🚀\r\nmiddle line\r\nlast";
        let normalized = normalize_content(content);
        let mut document = EditorDocument::from_text(content);
        let mut textarea = TextArea::from(normalized.lines().map(String::from).collect::<Vec<_>>());
        assert_parity(&document, &textarea);

        for movement in [
            CursorMove::Top,
            CursorMove::End,
            CursorMove::Jump(1, 3),
            CursorMove::Bottom,
        ] {
            document.move_cursor(movement);
            textarea.move_cursor(movement);
            assert_parity(&document, &textarea);
        }

        document.insert_str(" paste\n行");
        textarea.insert_str(" paste\n行");
        assert_parity(&document, &textarea);

        document.start_selection();
        textarea.start_selection();
        document.move_cursor(CursorMove::WordForward);
        textarea.move_cursor(CursorMove::WordForward);
        document.cut();
        textarea.cut();
        assert_parity(&document, &textarea);

        document.undo();
        textarea.undo();
        assert_parity(&document, &textarea);
        document.redo();
        textarea.redo();
        assert_parity(&document, &textarea);

        for index in 0..50 {
            let text = index.to_string();
            document.insert_str(&text);
            textarea.insert_str(text);
            assert_parity(&document, &textarea);
        }
    }
}
