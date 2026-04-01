pub mod ocr;

use crate::app::App;
use anyhow::Result;
use once_cell::sync::Lazy;
use std::borrow::Cow;

pub trait Action: Send + Sync {
    fn id(&self) -> Cow<'static, str>;
    fn name(&self) -> Cow<'static, str>;
    fn description(&self) -> Cow<'static, str>;
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;
}

pub struct ActionInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

pub static ACTIONS: Lazy<Vec<Box<dyn Action>>> = Lazy::new(|| vec![Box::new(ocr::OcrPasteAction)]);

pub static ACTION_INFOS: Lazy<Vec<ActionInfo>> = Lazy::new(|| {
    ACTIONS
        .iter()
        .map(|a| ActionInfo {
            id: a.id().into_owned(),
            name: a.name().into_owned(),
            description: a.description().into_owned(),
        })
        .collect()
});

pub fn get_all_actions() -> &'static [Box<dyn Action>] {
    &ACTIONS
}

pub fn get_cached_action_infos() -> &'static [ActionInfo] {
    &ACTION_INFOS
}

pub fn execute_action(action_id: &str, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
    for action in get_all_actions() {
        if action.id() == action_id {
            return action.execute(app, context_note_id);
        }
    }
    anyhow::bail!("Action not found: {}", action_id)
}
