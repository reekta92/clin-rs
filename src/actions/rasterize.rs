use super::Action;
use crate::app::App;
use anyhow::{anyhow, Result};
use std::borrow::Cow;

pub struct RasterizeNoteAction;

impl Action for RasterizeNoteAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("note.rasterize")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Rasterize Note Spacing")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Remove redundant empty lines without changing note content")
    }

    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f03a}", "\u{2637}")
    }

    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
        let note_id = context_note_id
            .map(str::to_owned)
            .or_else(|| app.get_selected_note_id())
            .ok_or_else(|| anyhow!("No note selected"))?;

        if note_id.ends_with(".clin") {
            app.set_temporary_status_static("Cannot rasterize encrypted notes. Decrypt first.");
            return Ok(());
        }

        let mut note = app.storage.load_note(&note_id)?;
        let editing_selected_note = app.editor.editing_id.as_deref() == Some(note_id.as_str());
        let source_content = if editing_selected_note {
            app.editor.editor.lines().join("\n")
        } else {
            note.content.clone()
        };
        let content = rasterize_spacing(&source_content);
        if content == source_content {
            app.set_temporary_status_static("Note spacing already rasterized");
            return Ok(());
        }

        note.content = content.clone();
        note.updated_at = crate::ui::now_unix_secs();
        app.storage.save_note(&note_id, &note)?;

        if editing_selected_note {
            app.editor.editor = ratatui_textarea::TextArea::from(content.lines());
            app.rebuild_outline();
            app.editor.links = app.compute_links();
        }

        app.refresh_note_single(None, &note_id);
        app.enqueue_backup(format!("auto: {}", note.title));
        app.set_temporary_status_static("Note spacing rasterized");
        Ok(())
    }
}

pub fn rasterize_spacing(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut previous_blank = true;
    let mut in_fenced_code = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let is_fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        let line = if in_fenced_code {
            line
        } else {
            line.trim_end()
        };
        let blank = line.trim().is_empty();

        if blank && !in_fenced_code {
            if previous_blank {
                continue;
            }
        }

        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(line);

        if is_fence {
            in_fenced_code = !in_fenced_code;
        }
        previous_blank = blank && !in_fenced_code;
    }

    while output.ends_with('\n') {
        output.pop();
    }

    output
}

#[cfg(test)]
mod tests {
    use super::rasterize_spacing;

    #[test]
    fn removes_redundant_empty_lines_and_edge_spacing() {
        assert_eq!(
            rasterize_spacing("\n\n# Title\n\n\nBody\n\n"),
            "# Title\n\nBody"
        );
    }

    #[test]
    fn preserves_non_empty_content_and_single_separators() {
        let content = "# Title\n\nParagraph\n\n- one\n- two";
        assert_eq!(rasterize_spacing(content), content);
    }

    #[test]
    fn preserves_empty_lines_inside_fenced_code() {
        let content = "```text\none\n\n\n two\n```\n\n\nAfter";
        assert_eq!(
            rasterize_spacing(content),
            "```text\none\n\n\n two\n```\n\nAfter"
        );
    }

    #[test]
    fn removes_trailing_whitespace_outside_fenced_code() {
        let content = "export TEST_ROOT=\"$(mktemp -d)\"                                                                                                                          ";
        assert_eq!(rasterize_spacing(content), "export TEST_ROOT=\"$(mktemp -d)\"");
    }
}
