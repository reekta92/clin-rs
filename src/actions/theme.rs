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
        "Select from available color themes".into()
    }

    fn category(&self) -> crate::actions::ActionCategory {
        crate::actions::ActionCategory::Settings
    }

    fn glyph(&self) -> &'static str {
        "\u{f042}"
    }

    fn execute(&self, app: &mut App, _context: Option<&str>) -> Result<()> {
        app.begin_theme_selection();
        Ok(())
    }

    fn name_dynamic(&self, _app: &App) -> String {
        let current = crate::config::ClinConfig::load()
            .map(|c| c.theme.theme.to_string())
            .unwrap_or_else(|_| "default".to_string());
        format!("Switch Theme [{}]", current)
    }
}
