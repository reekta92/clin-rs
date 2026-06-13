use super::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct CreateDrawAction;

impl Action for CreateDrawAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("draw.create")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create Drawing")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create a new drawing file")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.begin_create_draw();
        Ok(())
    }
}
