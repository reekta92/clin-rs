use super::Action;
use crate::app::App;
use anyhow::{Result, anyhow};
use std::borrow::Cow;

pub struct EncryptNoteAction;

impl Action for EncryptNoteAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("note.encrypt")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Encrypt Note")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Encrypt the selected note (.md \u{2192} .clin)")
    }
    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::Notes
    }

    fn glyph(&self) -> &'static str {
        "\u{f023}"
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

        if note_id.ends_with(".clin") {
            app.set_temporary_status_static("Note is already encrypted");
            return Ok(());
        }

        match app.storage.encrypt_note(&note_id) {
            Ok(new_id) => {
                app.list.folder_cache = None;
                let _ = app.refresh_notes();
                app.set_temporary_status(&format!("Note encrypted: {}", new_id));
            }
            Err(e) => {
                app.set_temporary_status(&format!("Failed to encrypt: {e:#}"));
            }
        }

        Ok(())
    }
}
