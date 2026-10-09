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

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f023}", "\u{1f512}")
    }

    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
        let note_id = context_note_id
            .map(str::to_owned)
            .or_else(|| app.get_selected_note_id())
            .ok_or_else(|| anyhow!("No note selected"))?;

        if note_id.ends_with(".clin") {
            app.set_temporary_status_static("Note is already encrypted");
            return Ok(());
        }

        let header = app.storage.load_frontmatter(&note_id)?;
        if header
            .as_deref()
            .map(crate::frontmatter::checked_parse)
            .transpose()?
            .is_some_and(|header| !header.extra.is_empty())
        {
            app.show_confirm(crate::popups::ConfirmAction::EncryptNote { note_id });
        } else {
            app.encrypt_note_confirmed(&note_id);
        }

        Ok(())
    }
}

impl App {
    pub(crate) fn encrypt_note_confirmed(&mut self, note_id: &str) {
        match self.convert_note_with_preview(note_id, true) {
            Ok(()) => {}
            Err(error) => {
                self.set_temporary_status(&format!("Failed to encrypt: {error:#}"));
                self.messages.push(
                    format!("Failed to encrypt: {error:#}"),
                    crate::app::messages::MessageSeverity::Warning,
                );
            }
        }
    }
}
