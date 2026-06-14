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
pub mod external_editor;
pub mod settings;

use crate::app::App;
use anyhow::Result;
use once_cell::sync::Lazy;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCategory {
    General,
    Notes,
    Import,
    Append,
    Views,
    Settings,
}

pub trait Action: Send + Sync {
    fn id(&self) -> Cow<'static, str>;
    fn name(&self) -> Cow<'static, str>;
    fn description(&self) -> Cow<'static, str>;
    fn category(&self) -> ActionCategory {
        ActionCategory::General
    }
    fn glyph(&self) -> &'static str {
        ""
    }
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;

    fn name_dynamic(&self, _app: &App) -> String {
        self.name().to_string()
    }
    fn description_dynamic(&self, _app: &App) -> String {
        self.description().to_string()
    }
}

pub struct ActionInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ActionCategory,
    pub glyph: String,
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
        Box::new(external_editor::ToggleExternalEditorAction),
        Box::new(layout::ToggleLayoutAction),
        Box::new(settings::TogglePreviewPaneAction),
        Box::new(settings::ToggleLineNumbersAction),
        Box::new(settings::ToggleConfirmDeleteAction),
        Box::new(settings::TogglePinnedOnTopAction),
        Box::new(settings::ToggleConfirmQuitAction),
        Box::new(settings::TogglePreviewEncryptionAction),
        Box::new(settings::CycleSortAction),
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

pub fn get_all_action_infos(app: &App) -> Vec<ActionInfo> {
    ACTIONS
        .iter()
        .map(|a| ActionInfo {
            id: a.id().to_string(),
            name: a.name_dynamic(app),
            description: a.description_dynamic(app),
            category: a.category(),
            glyph: a.glyph().to_string(),
        })
        .collect()
}


pub fn get_all_actions() -> &'static [Box<dyn Action>] {
    &ACTIONS
}
pub fn execute_action(action_id: &str, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
    for action in get_all_actions() {
        if action.id() == action_id {
            return action.execute(app, context_note_id);
        }
    }
    anyhow::bail!("Action not found: {}", action_id)
}
