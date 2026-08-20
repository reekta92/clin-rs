use super::{
    BackupAction, CanvasAction, DrawAction, EditAction, GraphAction, HelpAction, KeyCombo,
    Keybinds, ListAction, OutlineAction, SetupAction,
};
use crate::config::KeybindPreset;

fn build<A: Eq + std::hash::Hash + Copy>(
    entries: &[(A, &[&str])],
) -> std::collections::HashMap<A, Vec<KeyCombo>> {
    entries
        .iter()
        .map(|(a, keys)| {
            (
                *a,
                keys.iter()
                    .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                    .collect(),
            )
        })
        .collect()
}

const DEFAULT_LIST: &[(ListAction, &[&str])] = &[
    (ListAction::MoveUp, &["Up", "k"]),
    (ListAction::MoveDown, &["Down", "j"]),
    (ListAction::MoveLeft, &["Left", "h"]),
    (ListAction::MoveRight, &["Right", "l"]),
    (ListAction::Open, &["Enter", "o"]),
    (ListAction::Delete, &["d", "Delete"]),
    (ListAction::Quit, &["q"]),
    (ListAction::Help, &["?"]),
    (ListAction::OpenLocation, &["Ctrl+l"]),
    (ListAction::CycleFocus, &["Tab"]),
    (ListAction::ReverseCycleFocus, &["BackTab"]),
    (ListAction::Confirm, &["y", "Enter"]),
    (ListAction::Cancel, &["n", "Esc"]),
    (ListAction::ToggleExternalEditor, &["Alt+e"]),
    (ListAction::NewFromTemplate, &["t"]),
    (ListAction::CreateFolder, &["Shift+N"]),
    (ListAction::CreateNote, &["n"]),
    (ListAction::RenameFolder, &["r"]),
    (ListAction::MoveNote, &["m"]),
    (ListAction::MoveToParent, &["U"]),
    (ListAction::ManageTags, &["."]),
    (ListAction::RemoveTagsFromSelected, &["Ctrl+."]),
    (ListAction::OpenCommandPalette, &[":", "Ctrl+p"]),
    (ListAction::Rename, &["r"]),
    (ListAction::Duplicate, &["y"]),
    (ListAction::TogglePin, &["p"]),
    (ListAction::CycleSort, &["s"]),
    (ListAction::Search, &["/"]),
    (ListAction::JumpToTop, &["Home", "Ctrl+Up"]),
    (ListAction::PageUp, &["Ctrl+u", "PageUp"]),
    (ListAction::PageDown, &["Ctrl+d", "PageDown"]),
    (ListAction::JumpToBottom, &["End", "Ctrl+Down"]),
    (ListAction::OpenTrash, &["Shift+T"]),
    (ListAction::TogglePreview, &["Shift+P"]),
    (ListAction::TogglePreviewFullscreen, &["Ctrl+e"]),
    (ListAction::ToggleWrap, &["Ctrl+w"]),
    // Preview paging; inherited by all presets (none override Shift+Up/Down).
    (ListAction::PreviewPageUp, &["Shift+Up"]),
    (ListAction::PreviewPageDown, &["Shift+Down"]),
    (ListAction::ToggleCalendar, &["Shift+C"]),
    (ListAction::ToggleFoldersFirst, &["Ctrl+h"]),
    (ListAction::OpenGraph, &["Ctrl+g"]),
    (ListAction::ToggleSelectMode, &["v"]),
    (ListAction::ToggleSelectItem, &["Space"]),
    (ListAction::CollapseAll, &["c"]),
    (ListAction::ExpandAll, &["e"]),
    (ListAction::ExpandToLevel, &["Shift+E"]),
    (ListAction::RefreshNotes, &["Ctrl+r"]),
    (ListAction::ManageSubnotes, &["Alt+s"]),
    (ListAction::ShowInfo, &["i"]),
];

const DEFAULT_EDIT: &[(EditAction, &[&str])] = &[
    (EditAction::Back, &["Esc"]),
    (EditAction::Save, &["Ctrl+s"]),
    (EditAction::CycleFocus, &["Ctrl+t"]),
    (EditAction::InsertTab, &["Tab"]),
    (EditAction::SelectAll, &["Ctrl+a", "Ctrl+Shift+a"]),
    (EditAction::Copy, &["Ctrl+Shift+c", "Ctrl+Insert", "Ctrl+c"]),
    (EditAction::Cut, &["Ctrl+Shift+x", "Shift+Delete", "Ctrl+x"]),
    (
        EditAction::Paste,
        &["Ctrl+Shift+v", "Shift+Insert", "Ctrl+v"],
    ),
    (EditAction::Undo, &["Ctrl+z"]),
    (EditAction::Redo, &["Ctrl+y", "Ctrl+Shift+z"]),
    (EditAction::DeleteWord, &["Ctrl+Backspace"]),
    (EditAction::DeleteNextWord, &["Ctrl+Delete"]),
    (EditAction::MoveToTop, &["Ctrl+Home"]),
    (EditAction::MoveToBottom, &["Ctrl+End"]),
    (EditAction::ToggleMarkdownPreview, &["Ctrl+p"]),
    (EditAction::TogglePreviewFullscreen, &["F11"]),
    (EditAction::ToggleWrap, &["F10"]),
    // Preview paging; inherited by all presets (none override PageUp/Down).
    (EditAction::PreviewPageUp, &["PageUp"]),
    (EditAction::PreviewPageDown, &["PageDown"]),
    (EditAction::ManageSubnotes, &["Alt+s"]),
    (EditAction::PasteImage, &["Ctrl+g Ctrl+i"]),
    (EditAction::InsertImageFromFile, &["Ctrl+g Ctrl+f"]),
    (EditAction::Find, &["Ctrl+f"]),
    (EditAction::GoToLine, &["Ctrl+g"]),
    (EditAction::InsertDate, &["Ctrl+;"]),
    (EditAction::ToggleOutline, &["Ctrl+o"]),
    (EditAction::ToggleLinks, &["Ctrl+b"]),
    (EditAction::PreviewLink, &["Alt+l"]),
];

const DEFAULT_HELP: &[(HelpAction, &[&str])] = &[
    (HelpAction::Close, &["Esc", "q", "?", "F1"]),
    (HelpAction::NextTab, &["Right", "l", "Tab"]),
    (HelpAction::PrevTab, &["Left", "h", "BackTab"]),
    (HelpAction::ScrollUp, &["Up", "k"]),
    (HelpAction::ScrollDown, &["Down", "j"]),
    (HelpAction::Search, &["/", "Ctrl+f"]),
    (HelpAction::Reroll, &["r"]),
];

const DEFAULT_GRAPH: &[(GraphAction, &[&str])] = &[
    (GraphAction::Quit, &["Esc", "q"]),
    (GraphAction::PanUp, &["Up", "k"]),
    (GraphAction::PanDown, &["Down", "j"]),
    (GraphAction::PanLeft, &["Left", "h"]),
    (GraphAction::PanRight, &["Right", "l"]),
    (GraphAction::ZoomIn, &["+", "="]),
    (GraphAction::ZoomOut, &["-", "_"]),
    (GraphAction::OpenNote, &["Enter", "o"]),
    (GraphAction::AutoFit, &["a"]),
    (GraphAction::Help, &["?"]),
    (GraphAction::ToggleSearch, &["/"]),
    (GraphAction::ToggleMinimap, &["Shift+M"]),
    (GraphAction::ToggleLegend, &["Shift+L"]),
    (GraphAction::ToggleGrid, &["Shift+G"]),
    (GraphAction::ToggleStatus, &["Shift+S"]),
    (GraphAction::Refresh, &["r"]),
    (GraphAction::ReloadConfig, &["Ctrl+r"]),
    (GraphAction::TogglePreview, &["Shift+P"]),
    (GraphAction::CreateConnection, &["c"]),
    (GraphAction::DeleteConnection, &["d"]),
    (GraphAction::LocalGraph, &["l"]),
    (GraphAction::ShowGroup, &["g"]),
    (GraphAction::DeleteNode, &["x"]),
    (GraphAction::MenuClose, &["Esc"]),
    (GraphAction::MenuUp, &["Up", "k"]),
    (GraphAction::MenuDown, &["Down", "j"]),
    (GraphAction::MenuSelect, &["Enter"]),
    (GraphAction::LookingGlass, &["Shift+O"]),
];

const DEFAULT_DRAW: &[(DrawAction, &[&str])] = &[
    (DrawAction::Quit, &["Esc", "q"]),
    (DrawAction::Help, &["?"]),
    (DrawAction::SelectDrawTool, &["d"]),
    (DrawAction::SelectCursorTool, &["a"]),
    (DrawAction::ToggleShapeSelector, &["s"]),
    (DrawAction::SelectTextTool, &["t"]),
    (DrawAction::SelectEraseTool, &["e"]),
    (DrawAction::ShapeSelectorUp, &["Up", "k"]),
    (DrawAction::ShapeSelectorDown, &["Down", "j"]),
    (DrawAction::ShapeSelectorConfirm, &["Enter"]),
    (DrawAction::ShapeSelectorCancel, &["Esc", "q"]),
    (DrawAction::ToggleColorSelector, &["c"]),
    (DrawAction::ColorSelectorUp, &["Up", "k"]),
    (DrawAction::ColorSelectorDown, &["Down", "j"]),
    (DrawAction::ColorSelectorConfirm, &["Enter"]),
    (DrawAction::ColorSelectorCancel, &["Esc", "q"]),
    (DrawAction::TextEditorConfirm, &["Enter"]),
    (DrawAction::TextEditorCancel, &["Esc"]),
    (DrawAction::MenuClose, &["Esc"]),
    (DrawAction::MenuUp, &["Up", "k"]),
    (DrawAction::MenuDown, &["Down", "j"]),
    (DrawAction::MenuSelect, &["Enter"]),
    (DrawAction::Copy, &["c"]),
    (DrawAction::Paste, &["v"]),
    (DrawAction::Undo, &["Ctrl+z"]),
    (DrawAction::Redo, &["Ctrl+y", "Ctrl+Shift+z"]),
    (DrawAction::ToggleGrid, &["Shift+G"]),
];

const DEFAULT_CANVAS: &[(CanvasAction, &[&str])] = &[
    (CanvasAction::Quit, &["Esc", "q"]),
    (CanvasAction::Undo, &["Ctrl+z"]),
    (CanvasAction::Redo, &["Ctrl+y", "Ctrl+Shift+z"]),
    (CanvasAction::Save, &["Ctrl+s"]),
    (CanvasAction::ZoomFineIn, &[">", "]"]),
    (CanvasAction::ZoomFineOut, &["<", "["]),
    (CanvasAction::ZoomIn, &["+", "="]),
    (CanvasAction::ZoomOut, &["-", "_"]),
    (CanvasAction::MoveLeft, &["Left", "h"]),
    (CanvasAction::MoveRight, &["Right", "l"]),
    (CanvasAction::MoveUp, &["Up", "k"]),
    (CanvasAction::MoveDown, &["Down", "j"]),
    (CanvasAction::EditOrConnect, &["i", "Enter"]),
    (CanvasAction::OpenContextMenu, &["a"]),
    (CanvasAction::CreateConnection, &["c"]),
    (CanvasAction::DeleteConnection, &["d"]),
    (CanvasAction::RenameNode, &["r"]),
    (CanvasAction::ResizeMode, &["s"]),
    (CanvasAction::SetColor, &["o"]),
    (CanvasAction::DeleteNode, &["x"]),
    (CanvasAction::DeleteAllConnections, &["b"]),
    (CanvasAction::AddTextNode, &["t"]),
    (CanvasAction::AddGroup, &["g"]),
    (CanvasAction::AddImageNode, &["m"]),
    (CanvasAction::ToggleGrid, &["Shift+G"]),
    (CanvasAction::ToggleOrthogonal, &["Ctrl+o"]),
    (CanvasAction::ToggleEditorPane, &["Ctrl+e"]),
    (CanvasAction::CycleFocus, &["Tab", "BackTab"]),
    (CanvasAction::Help, &["?"]),
    (CanvasAction::RenameConfirm, &["Enter"]),
    (CanvasAction::RenameCancel, &["Esc"]),
    (CanvasAction::MenuClose, &["Esc"]),
    (CanvasAction::MenuUp, &["Up"]),
    (CanvasAction::MenuDown, &["Down"]),
    (CanvasAction::MenuSelect, &["Enter"]),
    (CanvasAction::CloseEditor, &["Esc"]),
    (CanvasAction::CloseEditorAlt, &["Ctrl+Enter"]),
    (CanvasAction::ConfirmResize, &["Enter"]),
    (CanvasAction::CancelResize, &["Esc"]),
    (CanvasAction::EditorUnfocus, &["Esc"]),
];

const DEFAULT_BACKUP: &[(BackupAction, &[&str])] = &[
    (BackupAction::Back, &["Esc", "q"]),
    (BackupAction::MoveDown, &["j", "Down"]),
    (BackupAction::MoveUp, &["k", "Up"]),
    (BackupAction::ScrollDiffDown, &["Ctrl+d", "PageDown"]),
    (BackupAction::ScrollDiffUp, &["Ctrl+u", "PageUp"]),
    (BackupAction::Refresh, &["r"]),
    (BackupAction::EnterCommit, &["c"]),
    (BackupAction::Push, &["p"]),
    (BackupAction::OpenSettings, &[","]),
    (BackupAction::CycleSection, &["Tab", "BackTab"]),
    (BackupAction::Help, &["?"]),
    (BackupAction::Pull, &["Shift+P"]),
    (BackupAction::StageFile, &["Space", "s"]),
    (BackupAction::UnstageFile, &["u"]),
    (BackupAction::StageAll, &["Shift+S"]),
    (BackupAction::CancelCommit, &["Esc"]),
    (BackupAction::ConfirmCommit, &["Enter"]),
    (BackupAction::CloseSettings, &["Esc", "q"]),
    (BackupAction::NextField, &["j", "Down"]),
    (BackupAction::PrevField, &["k", "Up"]),
    (BackupAction::ActivateField, &["Enter"]),
    (BackupAction::CancelEditField, &["Esc"]),
    (BackupAction::ConfirmEditField, &["Enter"]),
];

const DEFAULT_OUTLINE: &[(OutlineAction, &[&str])] = &[
    (OutlineAction::MoveUp, &["k", "Up"]),
    (OutlineAction::MoveDown, &["j", "Down"]),
    (
        OutlineAction::ToggleCollapse,
        &["Tab", "Left", "Right", "h", "l"],
    ),
    (OutlineAction::ExpandAll, &["e"]),
    (OutlineAction::CollapseAll, &["c"]),
    (OutlineAction::Open, &["Enter", "o"]),
    (OutlineAction::Back, &["Esc", "q"]),
    (OutlineAction::Help, &["?"]),
];

const DEFAULT_SETUP: &[(SetupAction, &[&str])] = &[
    (SetupAction::Up, &["Up", "k"]),
    (SetupAction::Down, &["Down", "j"]),
    (SetupAction::CycleNext, &["Right", "l", "Space"]),
    (SetupAction::CyclePrev, &["Left", "h"]),
    (SetupAction::Activate, &["Enter"]),
    (SetupAction::Finish, &["Esc"]),
];

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            list: build(DEFAULT_LIST),
            edit: build(DEFAULT_EDIT),
            help: build(DEFAULT_HELP),
            graph: build(DEFAULT_GRAPH),
            draw: build(DEFAULT_DRAW),
            canvas: build(DEFAULT_CANVAS),
            backup: build(DEFAULT_BACKUP),
            outline: build(DEFAULT_OUTLINE),
            setup: build(DEFAULT_SETUP),
        }
    }
}

const HELIX_LIST: &[(ListAction, &[&str])] = &[
    // ── List view ──
    (ListAction::MoveUp, &["k", "Up"]),
    (ListAction::MoveDown, &["j", "Down"]),
    (ListAction::MoveLeft, &["h", "Left"]),
    (ListAction::MoveRight, &["l", "Right"]),
    (ListAction::Open, &["Enter", "o"]),
    (ListAction::Quit, &["q"]),
    (ListAction::Search, &["/"]),
    (ListAction::Help, &["?"]),
    (ListAction::JumpToTop, &["g g", "Shift+G"]),
    (ListAction::JumpToBottom, &["g e", "Shift+G"]),
    (ListAction::PageUp, &["Ctrl+b"]),
    (ListAction::PageDown, &["Ctrl+f"]),
    (ListAction::Delete, &["Space d"]),
    (ListAction::OpenCommandPalette, &["Space Space"]),
    (ListAction::NewFromTemplate, &["Space t"]),
    (ListAction::CreateNote, &["Space n"]),
    (ListAction::CreateFolder, &["Space N"]),
    (ListAction::TogglePin, &["Space p"]),
    (ListAction::MoveToParent, &["g u"]),
    (ListAction::OpenGraph, &["Space g"]),
    (ListAction::TogglePreview, &["Space P"]),
    (ListAction::OpenTrash, &["Space T"]),
    (ListAction::CycleSort, &["Space s"]),
    (ListAction::ManageTags, &["Space ."]),
    (ListAction::RemoveTagsFromSelected, &["Ctrl+."]),
    (ListAction::CollapseAll, &["c"]),
];

const HELIX_GRAPH: &[(GraphAction, &[&str])] = &[
    // ── Graph view ──
    (GraphAction::PanUp, &["k", "Up"]),
    (GraphAction::PanDown, &["j", "Down"]),
    (GraphAction::PanLeft, &["h", "Left"]),
    (GraphAction::PanRight, &["l", "Right"]),
    (GraphAction::Quit, &["q"]),
    (GraphAction::ToggleSearch, &["/"]),
    (GraphAction::ZoomIn, &["="]),
    (GraphAction::ZoomOut, &["-"]),
    (GraphAction::OpenNote, &["Enter", "o"]),
    (GraphAction::AutoFit, &["Space a"]),
    (GraphAction::Refresh, &["Space r"]),
    (GraphAction::ToggleMinimap, &["Space m"]),
    (GraphAction::ToggleGrid, &["Space g"]),
    (GraphAction::Help, &["?"]),
];

const HELIX_DRAW: &[(DrawAction, &[&str])] = &[
    // ── Draw view ──
    (DrawAction::Quit, &["q", "Esc"]),
    (DrawAction::Help, &["?"]),
    (DrawAction::SelectDrawTool, &["d"]),
    (DrawAction::ToggleShapeSelector, &["s"]),
    (DrawAction::SelectTextTool, &["t"]),
    (DrawAction::SelectEraseTool, &["e"]),
    (DrawAction::ShapeSelectorUp, &["k", "Up"]),
    (DrawAction::ShapeSelectorDown, &["j", "Down"]),
    (DrawAction::ShapeSelectorConfirm, &["Enter"]),
    (DrawAction::ShapeSelectorCancel, &["Esc"]),
    (DrawAction::TextEditorConfirm, &["Enter"]),
    (DrawAction::TextEditorCancel, &["Esc"]),
    (DrawAction::ToggleGrid, &["Space g"]),
];

const HELIX_CANVAS: &[(CanvasAction, &[&str])] = &[
    // ── Canvas view ──
    (CanvasAction::Quit, &["q", "Esc"]),
    (CanvasAction::Save, &["Ctrl+s"]),
    (CanvasAction::ZoomIn, &["=", "+"]),
    (CanvasAction::ZoomOut, &["-"]),
    (CanvasAction::ZoomFineIn, &[">"]),
    (CanvasAction::ZoomFineOut, &["<"]),
    (CanvasAction::MoveUp, &["k", "Up"]),
    (CanvasAction::MoveDown, &["j", "Down"]),
    (CanvasAction::MoveLeft, &["h", "Left"]),
    (CanvasAction::MoveRight, &["l", "Right"]),
    (CanvasAction::EditOrConnect, &["i", "Enter", "o"]),
    (CanvasAction::OpenContextMenu, &["Space m"]),
    (CanvasAction::ToggleGrid, &["Space g"]),
    (CanvasAction::Help, &["?"]),
];

const HELIX_BACKUP: &[(BackupAction, &[&str])] = &[
    // ── Backup view ──
    (BackupAction::Back, &["q", "Esc"]),
    (BackupAction::MoveDown, &["j", "Down"]),
    (BackupAction::MoveUp, &["k", "Up"]),
    (BackupAction::ScrollDiffDown, &["Ctrl+d", "PageDown"]),
    (BackupAction::ScrollDiffUp, &["Ctrl+u", "PageUp"]),
    (BackupAction::Refresh, &["r"]),
    (BackupAction::EnterCommit, &["c"]),
    (BackupAction::Push, &["p"]),
    (BackupAction::StageFile, &["Space"]),
    (BackupAction::OpenSettings, &["Space s"]),
    (BackupAction::CycleSection, &["Tab", "BackTab"]),
];

const HELIX_OUTLINE: &[(OutlineAction, &[&str])] = &[
    // ── Outline view ──
    (OutlineAction::MoveUp, &["k", "Up"]),
    (OutlineAction::MoveDown, &["j", "Down"]),
    (
        OutlineAction::ToggleCollapse,
        &["Tab", "Left", "Right", "h", "l"],
    ),
    (OutlineAction::ExpandAll, &["e"]),
    (OutlineAction::CollapseAll, &["c"]),
    (OutlineAction::Open, &["Enter", "o"]),
    (OutlineAction::Back, &["Esc", "q"]),
    (OutlineAction::Help, &["?"]),
];

const VIM_LIST: &[(ListAction, &[&str])] = &[
    // ── List view ──
    (ListAction::MoveUp, &["k", "Up"]),
    (ListAction::MoveDown, &["j", "Down"]),
    (ListAction::MoveLeft, &["h", "Left"]),
    (ListAction::MoveRight, &["l", "Right"]),
    (ListAction::Open, &["Enter", "o"]),
    (ListAction::Delete, &["d d"]),
    (ListAction::Quit, &[": q"]),
    (ListAction::Help, &["?"]),
    (ListAction::Search, &["/"]),
    (ListAction::JumpToTop, &["g g"]),
    (ListAction::JumpToBottom, &["g G", "Shift+G"]),
    (ListAction::PageUp, &["Ctrl+u", "PageUp"]),
    (ListAction::PageDown, &["Ctrl+d", "PageDown"]),
    (ListAction::OpenCommandPalette, &["Ctrl+p"]),
    (ListAction::CreateNote, &["n"]),
    (ListAction::CreateFolder, &["Shift+N"]),
    (ListAction::NewFromTemplate, &["t"]),
    (ListAction::TogglePin, &["p"]),
    (ListAction::CycleSort, &["s"]),
    (ListAction::RemoveTagsFromSelected, &["Ctrl+."]),
    (ListAction::ManageTags, &["."]),
    (ListAction::Rename, &["r"]),
    (ListAction::MoveNote, &["m"]),
    (ListAction::MoveToParent, &["g u"]),
    (ListAction::ToggleExternalEditor, &["Alt+e"]),
    (ListAction::OpenGraph, &["Ctrl+g"]),
    (ListAction::OpenTrash, &["Shift+T"]),
    (ListAction::TogglePreview, &["Shift+P"]),
    (ListAction::CollapseAll, &["c"]),
];

const VIM_GRAPH: &[(GraphAction, &[&str])] = &[
    // ── Graph view ──
    (GraphAction::PanUp, &["k", "Up"]),
    (GraphAction::PanDown, &["j", "Down"]),
    (GraphAction::PanLeft, &["h", "Left"]),
    (GraphAction::PanRight, &["l", "Right"]),
    (GraphAction::Quit, &[": q", "q"]),
    (GraphAction::ZoomIn, &["=", "+"]),
    (GraphAction::ZoomOut, &["-"]),
    (GraphAction::ToggleSearch, &["/"]),
    (GraphAction::OpenNote, &["Enter", "o"]),
    (GraphAction::AutoFit, &["a"]),
    (GraphAction::Refresh, &["r"]),
    (GraphAction::Help, &["?"]),
];

const VIM_DRAW: &[(DrawAction, &[&str])] = &[
    // ── Draw view ──
    (DrawAction::Quit, &["q", "Esc"]),
    (DrawAction::Help, &["?"]),
    (DrawAction::SelectDrawTool, &["d"]),
    (DrawAction::ToggleShapeSelector, &["s"]),
    (DrawAction::SelectTextTool, &["t"]),
    (DrawAction::SelectEraseTool, &["e"]),
    (DrawAction::ShapeSelectorUp, &["k", "Up"]),
    (DrawAction::ShapeSelectorDown, &["j", "Down"]),
    (DrawAction::ShapeSelectorConfirm, &["Enter"]),
    (DrawAction::ShapeSelectorCancel, &["Esc"]),
    (DrawAction::TextEditorConfirm, &["Enter"]),
    (DrawAction::TextEditorCancel, &["Esc"]),
    (DrawAction::ToggleGrid, &["Space"]),
];

const VIM_CANVAS: &[(CanvasAction, &[&str])] = &[
    // ── Canvas view ──
    (CanvasAction::Quit, &["q", "Esc"]),
    (CanvasAction::Save, &["Ctrl+s"]),
    (CanvasAction::ZoomIn, &["=", "+"]),
    (CanvasAction::ZoomOut, &["-"]),
    (CanvasAction::ZoomFineIn, &[">"]),
    (CanvasAction::ZoomFineOut, &["<"]),
    (CanvasAction::MoveUp, &["k", "Up"]),
    (CanvasAction::MoveDown, &["j", "Down"]),
    (CanvasAction::MoveLeft, &["h", "Left"]),
    (CanvasAction::MoveRight, &["l", "Right"]),
    (CanvasAction::EditOrConnect, &["i", "Enter", "o"]),
    (CanvasAction::OpenContextMenu, &["Space"]),
    (CanvasAction::ToggleGrid, &["Space"]),
    (CanvasAction::Help, &["?"]),
];

const VIM_BACKUP: &[(BackupAction, &[&str])] = &[
    // ── Backup view ──
    (BackupAction::Back, &["q", "Esc"]),
    (BackupAction::MoveDown, &["j", "Down"]),
    (BackupAction::MoveUp, &["k", "Up"]),
    (BackupAction::ScrollDiffDown, &["Ctrl+d", "PageDown"]),
    (BackupAction::ScrollDiffUp, &["Ctrl+u", "PageUp"]),
    (BackupAction::Refresh, &["r"]),
    (BackupAction::EnterCommit, &["c"]),
    (BackupAction::Push, &["p"]),
    (BackupAction::OpenSettings, &["Space"]),
    (BackupAction::CycleSection, &["Tab", "BackTab"]),
    (BackupAction::StageFile, &["Space"]),
];

const VIM_OUTLINE: &[(OutlineAction, &[&str])] = &[
    // ── Outline view ──
    (OutlineAction::MoveUp, &["k", "Up"]),
    (OutlineAction::MoveDown, &["j", "Down"]),
    (
        OutlineAction::ToggleCollapse,
        &["Tab", "Left", "Right", "h", "l"],
    ),
    (OutlineAction::ExpandAll, &["e"]),
    (OutlineAction::CollapseAll, &["c"]),
    (OutlineAction::Open, &["Enter", "o"]),
    (OutlineAction::Back, &["Esc", "q"]),
    (OutlineAction::Help, &["?"]),
];

const EMACS_LIST: &[(ListAction, &[&str])] = &[
    // ── List view ──
    (ListAction::MoveUp, &["Ctrl+p", "Up"]),
    (ListAction::MoveDown, &["Ctrl+n", "Down"]),
    (ListAction::MoveLeft, &["Ctrl+b", "Left"]),
    (ListAction::MoveRight, &["Ctrl+f", "Right"]),
    (ListAction::Quit, &["Ctrl+x Ctrl+c", "q"]),
    (ListAction::Help, &["Ctrl+h"]),
    (ListAction::Search, &["Ctrl+s"]),
    (ListAction::PageDown, &["Ctrl+v", "PageDown"]),
    (ListAction::PageUp, &["Alt+v", "PageUp"]),
    (ListAction::Delete, &["Ctrl+d", "Delete"]),
    (ListAction::OpenCommandPalette, &["Ctrl+x Ctrl+p"]),
    (ListAction::CollapseAll, &["c"]),
];

const EMACS_GRAPH: &[(GraphAction, &[&str])] = &[
    // ── Graph view ──
    (GraphAction::PanUp, &["Ctrl+p", "Up"]),
    (GraphAction::PanDown, &["Ctrl+n", "Down"]),
    (GraphAction::PanLeft, &["Ctrl+b", "Left"]),
    (GraphAction::PanRight, &["Ctrl+f", "Right"]),
    (GraphAction::Quit, &["Ctrl+x Ctrl+c", "q"]),
    (GraphAction::OpenNote, &["Enter", "o"]),
    (GraphAction::AutoFit, &["a"]),
    (GraphAction::Refresh, &["r"]),
    (GraphAction::Help, &["Ctrl+h"]),
    (GraphAction::ToggleSearch, &["Ctrl+s"]),
];

const EMACS_DRAW: &[(DrawAction, &[&str])] = &[
    // ── Draw view ──
    (DrawAction::Quit, &["Ctrl+x Ctrl+c", "q"]),
    (DrawAction::ShapeSelectorUp, &["Ctrl+p", "Up"]),
    (DrawAction::ShapeSelectorDown, &["Ctrl+n", "Down"]),
    (DrawAction::ShapeSelectorConfirm, &["Enter"]),
    (DrawAction::ShapeSelectorCancel, &["Esc"]),
    (DrawAction::Help, &["Ctrl+h"]),
];

const EMACS_CANVAS: &[(CanvasAction, &[&str])] = &[
    // ── Canvas view ──
    (CanvasAction::MoveUp, &["Ctrl+p", "Up"]),
    (CanvasAction::MoveDown, &["Ctrl+n", "Down"]),
    (CanvasAction::MoveLeft, &["Ctrl+b", "Left"]),
    (CanvasAction::MoveRight, &["Ctrl+f", "Right"]),
    (CanvasAction::Quit, &["Ctrl+x Ctrl+c"]),
    (CanvasAction::Save, &["Ctrl+s"]),
    (CanvasAction::Help, &["Ctrl+h"]),
];

const EMACS_BACKUP: &[(BackupAction, &[&str])] = &[
    // ── Backup view ──
    (BackupAction::Back, &["Ctrl+x Ctrl+c", "q"]),
    (BackupAction::MoveDown, &["Ctrl+n", "Down"]),
    (BackupAction::MoveUp, &["Ctrl+p", "Up"]),
    (BackupAction::Refresh, &["r"]),
    (BackupAction::EnterCommit, &["c"]),
    (BackupAction::Push, &["p"]),
    (BackupAction::CycleSection, &["Tab", "BackTab"]),
];

const EMACS_OUTLINE: &[(OutlineAction, &[&str])] = &[
    // ── Outline view ──
    (OutlineAction::MoveUp, &["Ctrl+p", "Up"]),
    (OutlineAction::MoveDown, &["Ctrl+n", "Down"]),
    (OutlineAction::ToggleCollapse, &["Tab"]),
    (OutlineAction::Open, &["Enter", "o"]),
    (OutlineAction::Back, &["Ctrl+x Ctrl+c", "q"]),
    (OutlineAction::Help, &["Ctrl+h"]),
];

impl KeybindPreset {
    /// Returns true if this preset's base bindings include any multi-key sequences.
    pub fn uses_sequences(&self) -> bool {
        let kb = self.base_keybinds();
        Self::has_multi_seq(&kb.list)
            || Self::has_multi_seq(&kb.edit)
            || Self::has_multi_seq(&kb.help)
            || Self::has_multi_seq(&kb.graph)
            || Self::has_multi_seq(&kb.draw)
            || Self::has_multi_seq(&kb.canvas)
            || Self::has_multi_seq(&kb.backup)
            || Self::has_multi_seq(&kb.outline)
            || Self::has_multi_seq(&kb.setup)
    }

    fn has_multi_seq<A>(map: &std::collections::HashMap<A, Vec<super::KeyCombo>>) -> bool {
        map.values().flatten().any(|c| c.keys.len() > 1)
    }

    /// Return the base bindings for this preset.
    /// The `edit` map is always `Keybinds::default().edit` (presets never affect text editing).
    pub fn base_keybinds(&self) -> Keybinds {
        let default_kb = Keybinds::default();
        match self {
            KeybindPreset::Default => default_kb,
            KeybindPreset::Helix => {
                let mut kb = default_kb;
                for (a, keys) in HELIX_LIST {
                    kb.list.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in HELIX_GRAPH {
                    kb.graph.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in HELIX_DRAW {
                    kb.draw.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in HELIX_CANVAS {
                    kb.canvas.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in HELIX_BACKUP {
                    kb.backup.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in HELIX_OUTLINE {
                    kb.outline.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                kb.edit = Keybinds::default().edit;
                kb
            }
            KeybindPreset::Vim => {
                let mut kb = default_kb;
                for (a, keys) in VIM_LIST {
                    kb.list.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in VIM_GRAPH {
                    kb.graph.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in VIM_DRAW {
                    kb.draw.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in VIM_CANVAS {
                    kb.canvas.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in VIM_BACKUP {
                    kb.backup.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in VIM_OUTLINE {
                    kb.outline.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                kb.edit = Keybinds::default().edit;
                kb
            }
            KeybindPreset::Emacs => {
                let mut kb = default_kb;
                for (a, keys) in EMACS_LIST {
                    kb.list.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in EMACS_GRAPH {
                    kb.graph.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in EMACS_DRAW {
                    kb.draw.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in EMACS_CANVAS {
                    kb.canvas.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in EMACS_BACKUP {
                    kb.backup.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                for (a, keys) in EMACS_OUTLINE {
                    kb.outline.insert(
                        *a,
                        keys.iter()
                            .map(|k| KeyCombo::parse(k).expect("valid key combo"))
                            .collect(),
                    );
                }
                kb.edit = Keybinds::default().edit;
                kb
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use strum::IntoEnumIterator;

    #[test]
    fn ctrl_shift_a_matches_select_all() {
        let keybinds = Keybinds::default();
        let event = KeyEvent::new(
            KeyCode::Char('A'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(keybinds.matches_edit(EditAction::SelectAll, &event));
    }

    #[test]
    fn draw_actions_have_bindings_in_every_preset() {
        for preset in [
            KeybindPreset::Default,
            KeybindPreset::Helix,
            KeybindPreset::Vim,
            KeybindPreset::Emacs,
        ] {
            let keybinds = preset.base_keybinds();
            for action in DrawAction::iter() {
                assert!(
                    keybinds.draw.contains_key(&action),
                    "{preset} is missing {action:?}"
                );
            }
        }
    }

    #[test]
    fn default_draw_editing_bindings_match_contract() {
        let keybinds = Keybinds::default();
        for (action, key) in [
            (
                DrawAction::SelectCursorTool,
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            ),
            (
                DrawAction::Copy,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            ),
            (
                DrawAction::Paste,
                KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
            ),
            (
                DrawAction::Undo,
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            ),
            (
                DrawAction::Redo,
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
            ),
            (
                DrawAction::Redo,
                KeyEvent::new(
                    KeyCode::Char('z'),
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                ),
            ),
        ] {
            assert!(keybinds.matches_draw(action, &key), "{action:?}");
        }
        for (action, key) in [
            (
                DrawAction::Copy,
                KeyEvent::new(
                    KeyCode::Char('c'),
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                ),
            ),
            (
                DrawAction::Paste,
                KeyEvent::new(
                    KeyCode::Char('v'),
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                ),
            ),
        ] {
            assert!(!keybinds.matches_draw(action, &key), "{action:?}");
        }
    }
}
