pub mod ocr;

use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub trait Action {
    fn id(&self) -> Cow<'static, str>;
    fn name(&self) -> Cow<'static, str>;
    fn description(&self) -> Cow<'static, str>;
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;
}

pub fn get_all_actions() -> Vec<Box<dyn Action>> {
    vec![
        Box::new(ocr::OcrPasteAction),
    ]
}

pub fn execute_action(action_id: &str, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
    for action in get_all_actions() {
        if action.id() == action_id {
            return action.execute(app, context_note_id);
        }
    }
    anyhow::bail!("Action not found: {}", action_id)
}
