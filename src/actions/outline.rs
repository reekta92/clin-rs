use super::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct OpenOutlineAction;

impl Action for OpenOutlineAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("outline.open")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Outline")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Show the selected note's headers and content as a navigable tree")
    }
    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f1bb}", "\u{1f333}")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        if app.get_selected_note_id().is_none() {
            app.set_temporary_status_static("Select a note first");
            return Ok(());
        }
        app.open_outline_view();
        Ok(())
    }
}
