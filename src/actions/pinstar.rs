use crate::actions::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct CreateCanvasAction;

impl Action for CreateCanvasAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("create_canvas")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create Canvas Map")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create a new .canvas map file (Obsidian-compatible)")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.begin_create_canvas();
        Ok(())
    }
}
