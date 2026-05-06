use super::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct CreateCanvasAction;

impl Action for CreateCanvasAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("canvas.create")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create Canvas")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create a new drawable canvas file")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.begin_create_canvas();
        Ok(())
    }
}
