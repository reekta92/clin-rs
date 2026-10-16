//! Auto-closing bracket/quote pairs for the editor body (VS Code style).
//!
//! Pure pair table + guards live here so they are unit-testable without an
//! `App`. The `handle_*` entry points operate on `EditorDocument` directly
//! (borrowed, never whole-`App`) and are wired in from `events::edit`.

use crate::editor_document::EditorDocument;
use ratatui_textarea::CursorMove;

/// Opening char → its closing counterpart.
pub(crate) fn matching_close(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

/// Closing char → its opening counterpart.
pub(crate) fn matching_open(close: char) -> Option<char> {
    match close {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`')
}

fn is_closing_bracket(c: char) -> bool {
    matches!(c, ')' | ']' | '}')
}

/// Chars before which it is safe to auto-close (end of line, whitespace,
/// or a closing delimiter).
fn next_allows_pair(next: Option<char>) -> bool {
    match next {
        None => true,
        Some(c) => {
            c.is_whitespace()
                || is_closing_bracket(c)
                || matches!(c, ',' | '.' | ';' | ':' | '!' | '?')
        }
    }
}

/// Quotes must not auto-close inside words (`don't` → must stay single).
/// Allow only when the previous char is absent, whitespace, or an opening
/// delimiter.
fn prev_allows_quote(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => c.is_whitespace() || matches!(c, '(' | '[' | '{' | '"' | '\'' | '`'),
    }
}

/// Handle a typed `char` in the editor body.
///
/// Returns `true` when the keystroke was fully consumed (caller must refresh
/// preview + draft and return). Returns `false` to fall through to the
/// default `body.input(...)` path.
pub(crate) fn handle_body_char(body: &mut EditorDocument, typed: char) -> bool {
    // Non-empty keyboard/mouse selection? Typing an opener wraps it.
    // (Zero-width selections must NOT wrap — yank would be stale.)
    let has_selection = body
        .selection_range()
        .is_some_and(|(start, end)| start != end);
    if matching_close(typed).is_some() && has_selection {
        // Quotes only wrap when the quote guard passes (avoids mangling words).
        if is_quote(typed) {
            let prev = body.char_before_cursor();
            if !prev_allows_quote(prev) {
                return false;
            }
        }
        let close = matching_close(typed).expect("guarded by is_some");
        // Remove the selection into the yank buffer, then insert the wrapped
        // pair at the (now collapsed) cursor. `cut` (not `copy`) guarantees
        // no duplication regardless of `insert_str` selection semantics.
        body.cut();
        let selected = body.yank_text();
        body.insert_str(format!("{typed}{selected}{close}"));
        return true;
    }

    // 2. Skip-over: typing a closer that is already under the cursor just
    // moves past it (no duplicate).
    if matching_open(typed).is_some() && body.char_after_cursor() == Some(typed) {
        // For quotes, only skip when it is plausibly our own auto-inserted
        // closer (prev char is the matching opener or we are at a boundary).
        // Otherwise typing `'` after `don` would jump over an unrelated quote.
        if is_quote(typed) {
            let prev = body.char_before_cursor();
            if prev != Some(typed) && !prev_allows_quote(prev) {
                return false;
            }
        }
        body.move_cursor(CursorMove::Forward);
        return true;
    }

    // 3. Auto-close openers.
    if let Some(close) = matching_close(typed) {
        let next = body.char_after_cursor();
        if !next_allows_pair(next) {
            return false;
        }
        if is_quote(typed) {
            let prev = body.char_before_cursor();
            if !prev_allows_quote(prev) {
                return false;
            }
            // Typing a quote directly before the same auto-inserted quote:
            // skip over instead of doubling (e.g. empty `""` → move past).
            if next == Some(typed) {
                body.move_cursor(CursorMove::Forward);
                return true;
            }
        }
        body.insert_str(format!("{typed}{close}"));
        body.move_cursor(CursorMove::Back);
        return true;
    }

    false
}

/// True when the cursor sits between the two halves of an empty pair, e.g.
/// `(|)`. The caller runs the default Backspace deletion first and then
/// removes the orphaned closing half with `delete_next_chars(1)`.
pub(crate) fn is_empty_pair_at_cursor(body: &EditorDocument) -> bool {
    matches!(
        (body.char_before_cursor(), body.char_after_cursor()),
        (Some(open), Some(close)) if matching_close(open) == Some(close)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_table_round_trips() {
        for open in ['(', '[', '{', '"', '\'', '`'] {
            let close = matching_close(open).expect("opener maps");
            assert_eq!(matching_open(close), Some(open));
        }
        assert_eq!(matching_close('x'), None);
        assert_eq!(matching_open('x'), None);
    }

    #[test]
    fn next_guard_rejects_word_chars() {
        assert!(next_allows_pair(None));
        assert!(next_allows_pair(Some(' ')));
        assert!(next_allows_pair(Some('\n')));
        assert!(next_allows_pair(Some(')')));
        assert!(!next_allows_pair(Some('a')));
        assert!(!next_allows_pair(Some('(')));
    }

    #[test]
    fn quote_guard_rejects_inside_words() {
        assert!(prev_allows_quote(None));
        assert!(prev_allows_quote(Some(' ')));
        assert!(prev_allows_quote(Some('(')));
        assert!(!prev_allows_quote(Some('n'))); // don't
        assert!(!prev_allows_quote(Some(')')));
    }
}
