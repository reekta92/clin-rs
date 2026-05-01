use super::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct OpenGraphAction;

impl Action for OpenGraphAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("graph.open")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Open Graph View")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Visualize note connections as a force-directed graph")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.open_graph_view();
        Ok(())
    }
}
