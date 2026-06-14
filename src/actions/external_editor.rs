use crate::actions::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct ToggleExternalEditorAction;

impl Action for ToggleExternalEditorAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("external_editor.toggle")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle External Editor Mode")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Switch between the built-in editor and your $EDITOR for opening notes")
    }
    fn category(&self) -> crate::actions::ActionCategory {
        crate::actions::ActionCategory::Settings
    }

    fn glyph(&self) -> &'static str {
        "\u{f120}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.toggle_external_editor_mode();
        Ok(())
    }
}
