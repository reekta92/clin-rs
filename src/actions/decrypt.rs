use super::Action;
use crate::app::App;
use anyhow::{Result, anyhow};
use std::borrow::Cow;

pub struct DecryptNoteAction;

impl Action for DecryptNoteAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("note.decrypt")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Decrypt Note")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Decrypt the selected note (.clin \u{2192} .md)")
    }
    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::Notes
    }

    fn glyph(&self) -> &'static str {
        "\u{f3c1}"
    }

    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
        let note_id = context_note_id
            .map(|s| s.to_string())
            .or_else(|| {
                app.list
                    .visual_list
                    .get(app.list.visual_index)
                    .and_then(|item| match item {
                        crate::app::VisualItem::Note { summary_idx, .. } => {
                            Some(app.notes[*summary_idx].id.clone())
                        }
                        _ => None,
                    })
            })
            .ok_or_else(|| anyhow!("No note selected"))?;

        if !note_id.ends_with(".clin") {
            app.set_temporary_status_static("Note is not encrypted");
            return Ok(());
        }

        match app.storage.decrypt_note(&note_id) {
            Ok(new_id) => {
                app.list.folder_cache = None;
                let _ = app.refresh_notes();
                app.set_temporary_status(&format!("Note decrypted: {new_id}"));
            }
            Err(e) => {
                app.set_temporary_status(&format!("Failed to decrypt: {e:#}"));
            }
        }

        Ok(())
    }
}
