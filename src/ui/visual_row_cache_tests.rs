use super::*;
use crate::editor::NoteEditor;
use crate::editor_document::EditorDocument;
use ratatui_textarea::{CursorMove, Input};

fn assert_cache(editor: &mut NoteEditor, width: u16, numbers: bool) {
    let mode = editor.body.textarea().wrap_mode();
    let tabs = editor.body.textarea().tab_length();
    ensure_editor_visual_rows(editor, width, numbers);
    assert_eq!(
        editor.visual_row_cache.rows,
        editor_visual_rows(editor.body.lines(), mode, width, numbers, tabs)
    );
}

#[test]
fn incremental_wrapped_rows_match_full_layout_after_unicode_edits_and_history() {
    for mode in [WrapMode::Glyph, WrapMode::Word, WrapMode::WordOrGlyph] {
        for width in [5, 12, 30] {
            for numbers in [false, true] {
                let mut editor = NoteEditor {
                    body: EditorDocument::from_text("first 🙂 line\nsecond\t世界 line\nthird line"),
                    ..NoteEditor::default()
                };
                editor.body.set_wrap_mode(mode);
                editor.body.take_change();
                assert_cache(&mut editor, width, numbers);
                for text in ["prefix ", "🙂\nไทย\t", " more words"] {
                    editor.body.insert_str(text);
                    let change = editor.body.take_change().unwrap();
                    apply_editor_visual_change(&mut editor, &change);
                    assert_eq!(
                        editor.visual_row_cache.key.unwrap().0,
                        editor.body.revision()
                    );
                    assert_cache(&mut editor, width, numbers);
                }
                editor.body.undo();
                let change = editor.body.take_change().unwrap();
                assert!(matches!(
                    change,
                    crate::editor_document::DocumentChange::Full
                ));
                apply_editor_visual_change(&mut editor, &change);
                assert_cache(&mut editor, width, numbers);
                editor.body.redo();
                let change = editor.body.take_change().unwrap();
                apply_editor_visual_change(&mut editor, &change);
                assert_cache(&mut editor, width, numbers);
            }
        }
    }
}

#[test]
fn line_joins_and_gutter_digit_changes_keep_layout_correct() {
    let mut editor = NoteEditor {
        body: EditorDocument::from_text("12345\nabcde\nthird"),
        ..NoteEditor::default()
    };
    editor.body.set_wrap_mode(WrapMode::WordOrGlyph);
    editor.body.take_change();
    assert_cache(&mut editor, 10, true);
    editor.body.move_cursor(CursorMove::Jump(1, 0));
    editor
        .body
        .input(Input::from(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Backspace,
            crossterm::event::KeyModifiers::NONE,
        )));
    let change = editor.body.take_change().unwrap();
    apply_editor_visual_change(&mut editor, &change);
    assert_cache(&mut editor, 10, true);

    editor.body = EditorDocument::from_text(&["long words"; 9].join("\n"));
    editor.body.set_wrap_mode(WrapMode::WordOrGlyph);
    let change = editor.body.take_change().unwrap();
    apply_editor_visual_change(&mut editor, &change);
    assert_cache(&mut editor, 10, true);
    editor.body.insert_str("\n");
    let change = editor.body.take_change().unwrap();
    apply_editor_visual_change(&mut editor, &change);
    assert!(
        editor.visual_row_cache.key.is_none(),
        "gutter width changed at ten lines"
    );
    assert_cache(&mut editor, 10, true);
}
