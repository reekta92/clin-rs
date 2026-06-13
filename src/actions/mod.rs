pub mod backup;
pub mod content_tree;
pub mod decrypt;
pub mod draw;
pub mod encrypt;
pub mod graph;
pub mod import;
pub mod layout;
pub mod ocr;
pub mod pinstar;
pub mod theme;

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

pub static ACTIONS: Lazy<Vec<Box<dyn Action>>> = Lazy::new(|| {
    vec![
        Box::new(encrypt::EncryptNoteAction),
        Box::new(decrypt::DecryptNoteAction),
        Box::new(graph::OpenGraphAction),
        Box::new(content_tree::OpenContentTreeAction),
        Box::new(backup::OpenBackupAction),
        Box::new(draw::CreateDrawAction),
        Box::new(pinstar::CreateCanvasAction),
        Box::new(ocr::OcrPasteAction),
        Box::new(theme::SwitchThemeAction),
        Box::new(layout::ToggleLayoutAction),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::File,
            target: crate::popups::ImportTarget::NewNote,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::File,
            target: crate::popups::ImportTarget::AppendCurrent,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Csv,
            target: crate::popups::ImportTarget::NewNote,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Csv,
            target: crate::popups::ImportTarget::AppendCurrent,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Json,
            target: crate::popups::ImportTarget::NewNote,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Json,
            target: crate::popups::ImportTarget::AppendCurrent,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Url,
            target: crate::popups::ImportTarget::NewNote,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Url,
            target: crate::popups::ImportTarget::AppendCurrent,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Clipboard,
            target: crate::popups::ImportTarget::NewNote,
        }),
        Box::new(import::ImportAction {
            source: crate::popups::ImportSource::Clipboard,
            target: crate::popups::ImportTarget::AppendCurrent,
        }),
    ]
});

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
