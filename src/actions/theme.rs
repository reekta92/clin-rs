use crate::actions::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct SwitchThemeAction;

impl Action for SwitchThemeAction {
    fn id(&self) -> Cow<'static, str> {
        "switch_theme".into()
    }

    fn name(&self) -> Cow<'static, str> {
        "Switch Theme".into()
    }

    fn description(&self) -> Cow<'static, str> {
        "Cycle through available color themes".into()
    }

    fn execute(&self, app: &mut App, _context: Option<&str>) -> Result<()> {
        app.begin_theme_selection();
        Ok(())
    }
}
