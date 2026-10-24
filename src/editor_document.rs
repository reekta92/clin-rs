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
    identity: uuid::Uuid,
    counts: std::cell::Cell<Option<(usize, usize)>>,
    snapshot: std::cell::OnceCell<Arc<str>>,
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
            identity: uuid::Uuid::new_v4(),
            counts: std::cell::Cell::new(None),
            snapshot: std::cell::OnceCell::from(Arc::from(normalized)),
            change: Some(DocumentChange::Full),
        }
    }

    /// Public source editing retains trailing empty lines; normal note serialization stays unchanged.
    pub(crate) fn from_source(content: &str) -> Self {
        let normalized = content.replace("\r\n", "\n");
        Self {
            textarea: TextArea::from(normalized.split('\n').map(String::from).collect::<Vec<_>>()),
            revision: 0,
            identity: uuid::Uuid::new_v4(),
            counts: std::cell::Cell::new(None),
            snapshot: std::cell::OnceCell::from(Arc::from(normalized)),
            change: Some(DocumentChange::Full),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_lines(lines: impl IntoIterator<Item = String>) -> Self {
        let textarea = TextArea::from(lines.into_iter().collect::<Vec<_>>());
        Self {
            textarea,
            revision: 0,
            identity: uuid::Uuid::new_v4(),
            counts: std::cell::Cell::new(None),
            snapshot: std::cell::OnceCell::new(),
            change: Some(DocumentChange::Full),
        }
    }

    pub(crate) fn identity(&self) -> uuid::Uuid {
        self.identity
    }
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn snapshot(&self) -> Arc<str> {
        Arc::clone(
            self.snapshot
                .get_or_init(|| Arc::from(self.textarea.lines().join("\n"))),
        )
    }

    pub(crate) fn text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub(crate) fn lines(&self) -> &[String] {
        self.textarea.lines()
    }

    pub(crate) fn counts(&self) -> (usize, usize) {
        if let Some(counts) = self.counts.get() {
            return counts;
        }
        let (words, chars) = Self::line_counts(self.lines());
        let counts = (words, chars + self.line_count().saturating_sub(1));
        self.counts.set(Some(counts));
        counts
    }

    fn line_counts(lines: &[String]) -> (usize, usize) {
        lines.iter().fold((0, 0), |(words, chars), line| {
            (
                words + crate::goals::count_words(line),
                chars + line.chars().count(),
            )
        })
    }

    pub(crate) fn line_count(&self) -> usize {
        self.textarea.lines().len()
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
        // Native control/alt commands may replay history or edit a distant region.
        let opaque = input.ctrl || input.alt;
        if opaque {
            self.counts.set(None);
        }
        let effect = self.mutate(|textarea| textarea.input(input));
        if opaque && effect.content_changed {
            self.change = Some(DocumentChange::Full);
            self.counts.set(None);
        }
        effect
    }

    pub(crate) fn insert_str(&mut self, text: impl AsRef<str>) -> EditEffect {
        self.mutate(|textarea| textarea.insert_str(text))
    }

    #[cfg(test)]
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
        self.counts.set(None);
        let effect = self.mutate(TextArea::undo);
        if effect.content_changed {
            self.change = Some(DocumentChange::Full);
            self.counts.set(None);
        }
        effect
    }

    pub(crate) fn redo(&mut self) -> EditEffect {
        self.counts.set(None);
        let effect = self.mutate(TextArea::redo);
        if effect.content_changed {
            self.change = Some(DocumentChange::Full);
            self.counts.set(None);
        }
        effect
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
        let (start, old_end) = before_selection
            .map(|(start, end)| (start.0, end.0.saturating_add(1)))
            .unwrap_or((before_cursor.0, before_cursor.0.saturating_add(1)));
        let start = start.saturating_sub(1);
        let old_end = old_end.saturating_add(1).min(before_line_count);
        let previous_counts = self
            .counts
            .get()
            .map(|total| (total, Self::line_counts(&self.lines()[start..old_end])));
        let content_changed = f(&mut self.textarea);
        let cursor_changed = self.textarea.cursor() != before_cursor;
        if content_changed {
            // Boundary deletes can merge the preceding or following source line.
            let removed_lines = old_end.saturating_sub(start);
            let preserved_lines = before_line_count.saturating_sub(removed_lines);
            let added_lines = self.line_count().saturating_sub(preserved_lines);
            self.record_change(
                if self.line_count() < preserved_lines || start > self.line_count() {
                    DocumentChange::Full
                } else {
                    DocumentChange::Lines {
                        old: start..old_end,
                        new: start..start.saturating_add(added_lines),
                    }
                },
            );
            if let Some(((words, chars), (old_words, old_chars))) = previous_counts
                && self.line_count() >= preserved_lines
                && start <= self.line_count()
            {
                let (new_words, new_chars) =
                    Self::line_counts(&self.lines()[start..start + added_lines]);
                self.counts.set(Some((
                    words - old_words + new_words,
                    chars - old_chars + new_chars + self.line_count() - before_line_count,
                )));
            }
        }
        EditEffect {
            content_changed,
            cursor_changed,
        }
    }

    fn record_change(&mut self, change: DocumentChange) {
        self.revision = self.revision.wrapping_add(1);
        self.snapshot.take();
        self.counts.set(None);
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
        // Multiple unsynchronized edits can touch disjoint ranges.
        (Some(DocumentChange::Lines { .. }), DocumentChange::Lines { .. }) => DocumentChange::Full,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_source_retains_trailing_lines_during_edits_and_undo() {
        for source in [
            "",
            "Body",
            "Body\n",
            "Body\n\n",
            "---\r\nscore: 2\r\n---\r\nBody\r\n\r\n",
        ] {
            let normalized = source.replace("\r\n", "\n");
            let mut doc = super::EditorDocument::from_source(source);
            assert_eq!(doc.text(), normalized);
            assert_eq!(doc.snapshot().as_ref(), normalized);
            doc.insert_str("New ");
            assert_eq!(doc.text(), format!("New {normalized}"));
            doc.undo();
            assert_eq!(doc.text(), normalized);
        }
    }

    use super::*;

    fn assert_counts(document: &EditorDocument) {
        let text = document.text();
        let expected = (crate::goals::count_words(&text), text.chars().count());
        assert_eq!(document.counts(), expected, "{text:?}");
        assert_eq!(document.counts(), expected, "cached: {text:?}");
    }

    #[test]
    fn counts_follow_unicode_multiline_selection_history_and_boundary_edits() {
        for content in ["", "αβγ e\u{301} 界 🚀\nsecond word\nlast", "one\n\ntwo"] {
            let mut document = EditorDocument::from_text(content);
            assert_counts(&document);
            document.insert_str("new 🚀\nwords\n");
            assert_counts(&document);
            document.move_cursor(CursorMove::Head);
            document.input(Input {
                key: ratatui_textarea::Key::Backspace,
                ..Input::default()
            });
            assert_counts(&document);
            document.move_cursor(CursorMove::End);
            document.input(Input {
                key: ratatui_textarea::Key::Delete,
                ..Input::default()
            });
            assert_counts(&document);
            document.select_all();
            document.cut();
            assert_counts(&document);
            document.paste();
            assert_counts(&document);
            document.move_cursor(CursorMove::Top);
            document.start_selection();
            document.move_cursor(CursorMove::Bottom);
            document.cut();
            assert_counts(&document);
            document.undo();
            assert_counts(&document);
            document.redo();
            assert_counts(&document);
            document.undo();
            document.input(Input {
                key: ratatui_textarea::Key::Char('z'),
                ctrl: true,
                ..Input::default()
            });
            assert_counts(&document);
        }
    }

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
