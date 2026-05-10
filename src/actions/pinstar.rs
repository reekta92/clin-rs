use crate::actions::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct CreatePinstarAction;

impl Action for CreatePinstarAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("create_pinstar")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create Pinstar Map")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Create a new text-driven Pinstar map (Obsidian JSON)")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.begin_create_pinstar();
        Ok(())
    }
}

pub struct ImportCanvasAction;

impl Action for ImportCanvasAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("import_canvas")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Import Obsidian Canvas")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Import an existing .canvas file as a Pinstar map")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.begin_import_canvas();
        Ok(())
    }
}
