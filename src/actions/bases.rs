use super::{Action, ActionCategory};
use crate::{
    app::{App, ViewMode},
    keybinds::BasesAction,
};
use anyhow::{Result, bail};
use std::borrow::Cow;

pub(super) struct BasesCommand(pub &'static str, pub &'static str, pub BasesAction);
impl Action for BasesCommand {
    fn id(&self) -> Cow<'static, str> {
        format!("bases.{}", self.0).into()
    }
    fn name(&self) -> Cow<'static, str> {
        format!("Bases: {}", self.1).into()
    }
    fn description(&self) -> Cow<'static, str> {
        self.1.into()
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Views
    }
    fn glyph(&self) -> (&'static str, &'static str) {
        crate::ui::icons::BASES.pair()
    }
    fn execute(&self, app: &mut App, _: Option<&str>) -> Result<()> {
        if app.mode != ViewMode::Bases {
            bail!("Bases command needs active Bases view");
        }
        let Some(mut state) = app.bases_state.take() else {
            bail!("Bases view unavailable");
        };
        let result = if state.has_popup() {
            Err(anyhow::anyhow!(
                "close Bases popup before running another command"
            ))
        } else {
            state.apply_action(app, self.2, 1).map(|_| ())
        };
        if result.is_ok() {
            state.palette_preparing(app);
        }
        app.bases_state = Some(state);
        result
    }
}
pub(super) fn commands() -> Vec<Box<dyn Action>> {
    use BasesAction::{
        Clear, Copy, Edit, Export, MoveToGroup, NewNote, NextProperty, NextView, OpenCover, Paste,
        PreviousProperty, PreviousView, Redo, Refresh, Search, SelectAll, ToggleGroup,
        ToggleSelect, Undo,
    };
    [
        ("refresh", "Refresh public results", Refresh),
        ("next_view", "Next saved view", NextView),
        ("previous_view", "Previous saved view", PreviousView),
        ("edit", "Edit selected public property", Edit),
        ("clear", "Clear selected public property", Clear),
        ("copy", "Copy public property", Copy),
        ("paste", "Paste public property", Paste),
        ("select", "Toggle row selection", ToggleSelect),
        ("select_all", "Select all visible rows", SelectAll),
        ("undo", "Undo guarded write", Undo),
        ("redo", "Redo guarded write", Redo),
        ("new_note", "Create public note", NewNote),
        ("export", "Export public results", Export),
        ("move_to_group", "Move selected files to group", MoveToGroup),
        ("search", "Search public results", Search),
        (
            "toggle_group",
            "Collapse or expand selected group",
            ToggleGroup,
        ),
        ("open_cover", "Preview public cover", OpenCover),
        (
            "previous_property",
            "Previous displayed property",
            PreviousProperty,
        ),
        ("next_property", "Next displayed property", NextProperty),
    ]
    .into_iter()
    .map(|(id, label, action)| Box::new(BasesCommand(id, label, action)) as Box<dyn Action>)
    .chain(
        (0..crate::bases::palette::COMMANDS.len())
            .map(|index| Box::new(BasesConfigurationCommand(index)) as Box<dyn Action>),
    )
    .collect()
}

struct BasesConfigurationCommand(usize);

impl Action for BasesConfigurationCommand {
    fn id(&self) -> Cow<'static, str> {
        crate::bases::palette::COMMANDS[self.0].0.into()
    }
    fn name(&self) -> Cow<'static, str> {
        format!("Bases: {}", crate::bases::palette::COMMANDS[self.0].2).into()
    }
    fn description(&self) -> Cow<'static, str> {
        crate::bases::palette::COMMANDS[self.0].3.into()
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Bases
    }
    fn glyph(&self) -> (&'static str, &'static str) {
        crate::ui::icons::BASES.pair()
    }
    fn execute(&self, app: &mut App, _: Option<&str>) -> Result<()> {
        if app.mode != ViewMode::Bases {
            bail!("Bases command needs active Bases view");
        }
        let mut state = app
            .bases_state
            .take()
            .ok_or_else(|| anyhow::anyhow!("Bases view unavailable"))?;
        let result = state.run_configuration_command(app, self.0);
        app.bases_state = Some(state);
        result
    }
}
