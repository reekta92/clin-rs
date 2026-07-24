pub mod decrypt;
pub mod encrypt;
pub mod import;
pub mod info;
pub mod insert_date;
pub mod ocr;
pub mod outline;
pub mod rasterize;

pub mod settings;

use crate::app::App;
use anyhow::Result;
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
    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;

    fn name_dynamic(&self, _app: &App) -> String {
        self.name().to_string()
    }
    fn description_dynamic(&self, _app: &App) -> String {
        self.description().to_string()
    }
}

/// Action that delegates to a single `app.$method()` call, no dynamic name.
#[macro_export]
macro_rules! simple_action {
    ($name:ident, $id:literal, $label:literal, $desc:literal,
     $cat:expr, $glyph_nerd:literal, $glyph_unicode:literal, $method:ident) => {
        pub struct $name;
        impl $crate::actions::Action for $name {
            fn id(&self) -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($id)
            }
            fn name(&self) -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($label)
            }
            fn description(&self) -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($desc)
            }
            fn category(&self) -> $crate::actions::ActionCategory {
                $cat
            }
            fn glyph(&self) -> (&'static str, &'static str) {
                ($glyph_nerd, $glyph_unicode)
            }
            fn execute(&self, app: &mut $crate::app::App, _: Option<&str>) -> ::anyhow::Result<()> {
                app.$method();
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! toggle_action {
    ($name:ident, $id:literal, $label:literal, $desc:literal,
     $cat:expr, $glyph_nerd:literal, $glyph_unicode:literal, $method:ident, $state_var:ident, $state_expr:expr) => {
        pub struct $name;
        impl $crate::actions::Action for $name {
            fn id(&self) -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($id)
            }
            fn name(&self) -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($label)
            }
            fn description(&self) -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($desc)
            }
            fn category(&self) -> $crate::actions::ActionCategory {
                $cat
            }
            fn glyph(&self) -> (&'static str, &'static str) {
                ($glyph_nerd, $glyph_unicode)
            }
            fn execute(&self, app: &mut $crate::app::App, _: Option<&str>) -> ::anyhow::Result<()> {
                app.$method();
                Ok(())
            }
            fn name_dynamic(&self, $state_var: &$crate::app::App) -> String {
                let _ = $state_var;
                format!("{} [{}]", $label, $state_expr)
            }
        }
    };
}
simple_action!(
    OpenBackupAction,
    "backup.open",
    "Open Backup Dashboard",
    "View git backup status, commit history, and push to remote",
    ActionCategory::Views,
    "\u{f1d3}",
    "\u{1f4be}",
    open_backup_view
);
simple_action!(
    CreateDrawAction,
    "draw.create",
    "Create Drawing",
    "Create a new drawing file",
    ActionCategory::Views,
    "\u{f1fc}",
    "\u{270f}",
    begin_create_draw
);
simple_action!(
    OpenGraphAction,
    "graph.open",
    "Open Graph View",
    "Visualize note connections as a force-directed graph",
    ActionCategory::Views,
    "\u{f0e8}",
    "\u{1f5fa}",
    open_graph_view
);
simple_action!(
    CreateCanvasAction,
    "create_canvas",
    "Create Canvas Map",
    "Create a new .canvas map file (Obsidian-compatible)",
    ActionCategory::Views,
    "\u{f005}",
    "\u{1f58c}",
    begin_create_canvas
);
toggle_action!(
    ToggleExternalEditorAction,
    "external_editor.toggle",
    "Toggle External Editor Mode",
    "Switch between the built-in editor and your $EDITOR for opening notes",
    ActionCategory::Settings,
    "\u{f120}",
    "\u{2328}",
    toggle_external_editor_mode,
    app,
    if app.editor.external_editor_enabled {
        "On"
    } else {
        "Off"
    }
);
toggle_action!(
    ToggleLayoutAction,
    "toggle_notes_layout",
    "Toggle Notes Layout",
    "Switch between Tree and Grid layout for the notes view",
    ActionCategory::Settings,
    "\u{f0c9}",
    "\u{1f4cb}",
    toggle_notes_layout,
    app,
    match app.list.notes_layout {
        crate::config::NotesLayout::Tree => "Tree",
        crate::config::NotesLayout::Grid => "Grid",
    }
);
toggle_action!(
    SwitchThemeAction,
    "switch_theme",
    "Switch Theme",
    "Select from available color themes",
    ActionCategory::Settings,
    "\u{f042}",
    "\u{1f3a8}",
    begin_theme_selection,
    app,
    crate::config::ClinConfig::load()
        .map(|c| c.ui.theme.clone())
        .unwrap_or_else(|_| "default".to_string())
);
simple_action!(
    SwitchKeybindPresetAction,
    "keybind.preset",
    "Switch Keybind Preset",
    "Choose a keybind preset (default, helix, vim, emacs)",
    ActionCategory::Settings,
    "\u{f11c}",
    "\u{2328}",
    begin_keybind_preset_selection
);
simple_action!(
    OpenSetupWizardAction,
    "setup_wizard",
    "Run Setup Wizard",
    "Re-run the first-run setup to choose theme, keybinds, backup, and more",
    ActionCategory::Settings,
    "\u{f0a9}",
    "\u{2699}",
    open_setup_view
);
simple_action!(
    ManageSubnotesList,
    "manage_subnotes_list",
    "Manage Sub-notes",
    "Open the sub-notes manager for the selected note.",
    ActionCategory::Notes,
    "\u{f022}",
    "\u{1f4dd}",
    open_subnotes_popup
);

pub struct ActionInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ActionCategory,
    pub glyph: String,
}

pub static ACTIONS: std::sync::LazyLock<Vec<Box<dyn Action>>> = std::sync::LazyLock::new(|| {
    vec![
        Box::new(encrypt::EncryptNoteAction),
        Box::new(decrypt::DecryptNoteAction),
        Box::new(rasterize::RasterizeNoteAction),
        Box::new(ManageSubnotesList),
        Box::new(insert_date::InsertDateAction),
        Box::new(OpenGraphAction),
        Box::new(outline::OpenOutlineAction),
        Box::new(OpenBackupAction),
        Box::new(CreateDrawAction),
        Box::new(CreateCanvasAction),
        Box::new(ocr::OcrPasteAction),
        Box::new(ocr::PasteImageAction),
        Box::new(ocr::InsertImageFromFileAction),
        Box::new(SwitchThemeAction),
        Box::new(OpenSetupWizardAction),
        Box::new(SwitchKeybindPresetAction),
        Box::new(ToggleExternalEditorAction),
        Box::new(ToggleLayoutAction),
        Box::new(settings::ToggleLayoutEditModeAction),
        Box::new(settings::TogglePreviewPaneAction),
        Box::new(settings::ToggleWrapAction),
        Box::new(settings::ToggleCalendarAction),
        Box::new(settings::ToggleLineNumbersAction),
        Box::new(settings::ToggleConfirmDeleteAction),
        Box::new(settings::TogglePinnedOnTopAction),
        Box::new(settings::ToggleConfirmQuitAction),
        Box::new(settings::TogglePreviewEncryptionAction),
        Box::new(settings::CycleSortAction),
        Box::new(settings::ToggleShowHiddenFilesAction),
        Box::new(settings::ToggleShowAllFilesAction),
        Box::new(settings::ToggleTabIconsOnlyAction),
        Box::new(settings::SetWordGoalAction),
        Box::new(settings::ToggleFoldersFirstAction),
        Box::new(settings::ToggleInlineInfoAction),
        Box::new(settings::ToggleSmartFoldersAction),
        Box::new(settings::ConfigureSmartFoldersAction),
        Box::new(settings::SetNoteGoalAction),
        Box::new(settings::CycleIconModeAction),
        Box::new(settings::CycleHintBarStyleAction),
        Box::new(settings::ToggleEditModeHighlightAction),
        Box::new(settings::ToggleGhostSyntaxAction),
        Box::new(settings::ToggleExtendedMarkdownAction),
        Box::new(settings::ToggleScrollbarsAction),
        Box::new(settings::ToggleSyntaxHighlightingAction),
        Box::new(settings::ToggleCodeLineNumbersAction),
        Box::new(settings::ToggleShowFileSizeAction),
        Box::new(settings::CycleListDensityAction),
        Box::new(settings::CycleWeekStartAction),
        Box::new(settings::ToggleGoalsAction),
        Box::new(settings::ToggleGraphPreviewAction),
        Box::new(settings::ToggleGraphShowLegendAction),
        Box::new(settings::ToggleGraphShowGridAction),
        Box::new(settings::ToggleGraphShowMinimapAction),
        Box::new(settings::ToggleGraphShowOrphanAction),
        Box::new(info::ShowInfoAction),
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
    let icon_mode = app.config.ui.icon_mode;
    ACTIONS
        .iter()
        .map(|a| {
            let (nerd, unicode) = a.glyph();
            ActionInfo {
                id: a.id().to_string(),
                name: a.name_dynamic(app),
                description: a.description_dynamic(app),
                category: a.category(),
                glyph: crate::ui::get_icon(nerd, unicode, icon_mode).to_string(),
            }
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
    anyhow::bail!("Action not found: {action_id}")
}
