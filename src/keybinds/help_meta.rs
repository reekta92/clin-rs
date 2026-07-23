use crate::keybinds::types::{
    BackupAction, CanvasAction, DrawAction, EditAction, GraphAction, ListAction,
};

#[derive(Clone, Copy)]
pub struct HelpMeta {
    pub group: &'static str,
    pub description: &'static str,
}

pub fn list_group_order() -> &'static [&'static str] {
    &["Navigation", "Actions", "Display", "General"]
}
pub fn edit_group_order() -> &'static [&'static str] {
    &["Navigation", "Editing", "Preview", "Panels", "General"]
}
pub fn graph_group_order() -> &'static [&'static str] {
    &["Navigation", "Display", "System"]
}
pub fn draw_group_order() -> &'static [&'static str] {
    &["Tools", "Shape Selector", "Text Editor", "General"]
}
pub fn canvas_group_order() -> &'static [&'static str] {
    &[
        "Navigation",
        "Editing",
        "Interface",
        "Menus & Popups",
        "General",
    ]
}
pub fn backup_group_order() -> &'static [&'static str] {
    &["Navigation", "Actions", "Settings Fields", "General"]
}

pub fn list_action_meta(a: ListAction) -> HelpMeta {
    match a {
        ListAction::MoveUp => HelpMeta {
            group: "Navigation",
            description: "Move up",
        },
        ListAction::MoveDown => HelpMeta {
            group: "Navigation",
            description: "Move down",
        },
        ListAction::MoveLeft => HelpMeta {
            group: "Navigation",
            description: "Move left (grid)",
        },
        ListAction::MoveRight => HelpMeta {
            group: "Navigation",
            description: "Move right (grid)",
        },
        ListAction::JumpToTop => HelpMeta {
            group: "Navigation",
            description: "Jump to top",
        },
        ListAction::JumpToBottom => HelpMeta {
            group: "Navigation",
            description: "Jump to bottom",
        },
        ListAction::PageUp => HelpMeta {
            group: "Navigation",
            description: "Scroll up half page",
        },
        ListAction::PageDown => HelpMeta {
            group: "Navigation",
            description: "Scroll down half page",
        },
        ListAction::CollapseFolder => HelpMeta {
            group: "Navigation",
            description: "Collapse folder",
        },
        ListAction::ExpandFolder => HelpMeta {
            group: "Navigation",
            description: "Expand folder",
        },
        ListAction::Open => HelpMeta {
            group: "Actions",
            description: "Open selected item",
        },
        ListAction::CreateNote => HelpMeta {
            group: "Actions",
            description: "Create new note",
        },
        ListAction::CreateFolder => HelpMeta {
            group: "Actions",
            description: "Create new folder",
        },
        ListAction::Rename => HelpMeta {
            group: "Actions",
            description: "Rename note",
        },
        ListAction::RenameFolder => HelpMeta {
            group: "Actions",
            description: "Rename folder",
        },
        ListAction::Delete => HelpMeta {
            group: "Actions",
            description: "Delete",
        },
        ListAction::Duplicate => HelpMeta {
            group: "Actions",
            description: "Duplicate note",
        },
        ListAction::MoveNote => HelpMeta {
            group: "Actions",
            description: "Move note or folder",
        },
        ListAction::MoveToParent => HelpMeta {
            group: "Actions",
            description: "Move note to parent folder",
        },
        ListAction::ManageTags => HelpMeta {
            group: "Actions",
            description: "Manage tags",
        },
        ListAction::TogglePin => HelpMeta {
            group: "Actions",
            description: "Toggle pin",
        },
        ListAction::ToggleExternalEditor => HelpMeta {
            group: "Actions",
            description: "Toggle external editor",
        },
        ListAction::OpenLocation => HelpMeta {
            group: "Actions",
            description: "Open file location",
        },
        ListAction::CreatePinstar => HelpMeta {
            group: "Actions",
            description: "Create new pinstar drawing",
        },
        ListAction::ManageSubnotes => HelpMeta {
            group: "Actions",
            description: "Manage subnotes",
        },
        ListAction::Search => HelpMeta {
            group: "Display",
            description: "Search",
        },
        ListAction::ToggleSelectMode => HelpMeta {
            group: "Display",
            description: "Toggle select mode",
        },
        ListAction::ToggleSelectItem => HelpMeta {
            group: "Display",
            description: "Toggle select item",
        },
        ListAction::TogglePreview => HelpMeta {
            group: "Display",
            description: "Toggle preview pane",
        },
        ListAction::TogglePreviewFullscreen => HelpMeta {
            group: "Display",
            description: "Toggle preview fullscreen",
        },
        ListAction::TogglePreviewWrap => HelpMeta {
            group: "Display",
            description: "Toggle preview wrap",
        },
        ListAction::PreviewPageUp => HelpMeta {
            group: "Display",
            description: "Page preview up",
        },
        ListAction::PreviewPageDown => HelpMeta {
            group: "Display",
            description: "Page preview down",
        },
        ListAction::ToggleCalendar => HelpMeta {
            group: "Display",
            description: "Toggle calendar",
        },
        ListAction::OpenCommandPalette => HelpMeta {
            group: "Display",
            description: "Open command palette",
        },
        ListAction::OpenGraph => HelpMeta {
            group: "Display",
            description: "Open graph view",
        },
        ListAction::OpenCanvas => HelpMeta {
            group: "Display",
            description: "Open canvas view",
        },
        ListAction::CollapseAll => HelpMeta {
            group: "Display",
            description: "Collapse all folders",
        },
        ListAction::ExpandAll => HelpMeta {
            group: "Display",
            description: "Expand all folders",
        },
        ListAction::ExpandToLevel => HelpMeta {
            group: "Display",
            description: "Expand folders to level (e.g. 3E)",
        },
        ListAction::ToggleFoldersFirst => HelpMeta {
            group: "Display",
            description: "Toggle folders-first sort",
        },
        ListAction::RefreshNotes => HelpMeta {
            group: "Display",
            description: "Refresh notes (external changes)",
        },
        ListAction::Quit => HelpMeta {
            group: "General",
            description: "Quit",
        },
        ListAction::Help => HelpMeta {
            group: "General",
            description: "Help",
        },
        ListAction::CycleFocus => HelpMeta {
            group: "General",
            description: "Cycle focus between panes",
        },
        ListAction::Confirm => HelpMeta {
            group: "General",
            description: "Confirm action",
        },
        ListAction::Cancel => HelpMeta {
            group: "General",
            description: "Cancel action",
        },
        ListAction::NewFromTemplate => HelpMeta {
            group: "General",
            description: "New note from template",
        },
        ListAction::CycleSort => HelpMeta {
            group: "General",
            description: "Cycle sort order",
        },
        ListAction::OpenTrash => HelpMeta {
            group: "General",
            description: "Open trash",
        },
        ListAction::ShowInfo => HelpMeta {
            group: "General",
            description: "Show note info",
        },
    }
}

pub fn edit_action_meta(a: EditAction) -> HelpMeta {
    match a {
        EditAction::CycleFocus => HelpMeta {
            group: "Navigation",
            description: "Cycle focus (Title, Content)",
        },
        EditAction::InsertTab => HelpMeta {
            group: "Editing",
            description: "Insert tab character",
        },
        EditAction::Back => HelpMeta {
            group: "Navigation",
            description: "Return to notes (auto-saves)",
        },
        EditAction::Copy => HelpMeta {
            group: "Editing",
            description: "Copy",
        },
        EditAction::Cut => HelpMeta {
            group: "Editing",
            description: "Cut",
        },
        EditAction::Paste => HelpMeta {
            group: "Editing",
            description: "Paste",
        },
        EditAction::SelectAll => HelpMeta {
            group: "Editing",
            description: "Select all",
        },
        EditAction::Undo => HelpMeta {
            group: "Editing",
            description: "Undo",
        },
        EditAction::Redo => HelpMeta {
            group: "Editing",
            description: "Redo",
        },
        EditAction::DeleteWord => HelpMeta {
            group: "Editing",
            description: "Delete previous word",
        },
        EditAction::DeleteNextWord => HelpMeta {
            group: "Editing",
            description: "Delete next word",
        },
        EditAction::MoveToTop => HelpMeta {
            group: "Editing",
            description: "Move cursor to top",
        },
        EditAction::MoveToBottom => HelpMeta {
            group: "Editing",
            description: "Move cursor to bottom",
        },
        EditAction::ManageSubnotes => HelpMeta {
            group: "Editing",
            description: "Manage subnotes",
        },
        EditAction::ToggleMarkdownPreview => HelpMeta {
            group: "Preview",
            description: "Toggle markdown preview",
        },
        EditAction::TogglePreviewFullscreen => HelpMeta {
            group: "Preview",
            description: "Toggle preview fullscreen",
        },
        EditAction::TogglePreviewWrap => HelpMeta {
            group: "Preview",
            description: "Toggle preview wrap",
        },
        EditAction::PreviewPageUp => HelpMeta {
            group: "Preview",
            description: "Page preview up",
        },
        EditAction::PreviewPageDown => HelpMeta {
            group: "Preview",
            description: "Page preview down",
        },
        EditAction::PasteImage => HelpMeta {
            group: "Editing",
            description: "Paste image from clipboard",
        },
        EditAction::InsertImageFromFile => HelpMeta {
            group: "Editing",
            description: "Insert image from file",
        },
        EditAction::Find => HelpMeta {
            group: "Editing",
            description: "Find in document",
        },
        EditAction::InsertDate => HelpMeta {
            group: "Editing",
            description: "Insert date/time",
        },
        EditAction::ToggleSoftWrap => HelpMeta {
            group: "Editing",
            description: "Toggle soft wrap",
        },
        EditAction::ToggleOutline => HelpMeta {
            group: "Panels",
            description: "Toggle outline pane",
        },
        EditAction::ToggleLinks => HelpMeta {
            group: "Panels",
            description: "Toggle links pane",
        },
        EditAction::PreviewLink => HelpMeta {
            group: "Panels",
            description: "Preview linked note under cursor",
        },
        EditAction::GoToLine => HelpMeta {
            group: "Editor",
            description: "Go to line number",
        },
    }
}

pub fn graph_action_meta(a: GraphAction) -> HelpMeta {
    match a {
        GraphAction::PanUp => HelpMeta {
            group: "Navigation",
            description: "Pan up",
        },
        GraphAction::PanDown => HelpMeta {
            group: "Navigation",
            description: "Pan down",
        },
        GraphAction::PanLeft => HelpMeta {
            group: "Navigation",
            description: "Pan left",
        },
        GraphAction::PanRight => HelpMeta {
            group: "Navigation",
            description: "Pan right",
        },
        GraphAction::ZoomIn => HelpMeta {
            group: "Navigation",
            description: "Zoom in",
        },
        GraphAction::ZoomOut => HelpMeta {
            group: "Navigation",
            description: "Zoom out",
        },
        GraphAction::OpenNote => HelpMeta {
            group: "Navigation",
            description: "Open selected note",
        },
        GraphAction::AutoFit => HelpMeta {
            group: "Navigation",
            description: "Auto-fit graph to viewport",
        },
        GraphAction::ToggleSearch => HelpMeta {
            group: "Navigation",
            description: "Search nodes",
        },
        GraphAction::ToggleMinimap => HelpMeta {
            group: "Display",
            description: "Toggle minimap",
        },
        GraphAction::ToggleLegend => HelpMeta {
            group: "Display",
            description: "Toggle legend",
        },
        GraphAction::ToggleGrid => HelpMeta {
            group: "Display",
            description: "Toggle background grid",
        },
        GraphAction::ToggleStatus => HelpMeta {
            group: "Display",
            description: "Toggle status bar",
        },
        GraphAction::TogglePreview => HelpMeta {
            group: "Display",
            description: "Toggle preview",
        },
        GraphAction::Refresh => HelpMeta {
            group: "System",
            description: "Refresh physics",
        },
        GraphAction::ReloadConfig => HelpMeta {
            group: "System",
            description: "Reload config",
        },
        GraphAction::Help => HelpMeta {
            group: "System",
            description: "Help",
        },
        GraphAction::Quit => HelpMeta {
            group: "System",
            description: "Quit graph view",
        },
    }
}

pub fn draw_action_meta(a: DrawAction) -> HelpMeta {
    match a {
        DrawAction::SelectDrawTool => HelpMeta {
            group: "Tools",
            description: "Draw freehand strokes",
        },
        DrawAction::ToggleShapeSelector => HelpMeta {
            group: "Tools",
            description: "Shape tool (opens picker)",
        },
        DrawAction::SelectTextTool => HelpMeta {
            group: "Tools",
            description: "Place text label",
        },
        DrawAction::SelectEraseTool => HelpMeta {
            group: "Tools",
            description: "Erase elements",
        },
        DrawAction::ShapeSelectorUp => HelpMeta {
            group: "Shape Selector",
            description: "Select previous shape",
        },
        DrawAction::ShapeSelectorDown => HelpMeta {
            group: "Shape Selector",
            description: "Select next shape",
        },
        DrawAction::ShapeSelectorConfirm => HelpMeta {
            group: "Shape Selector",
            description: "Confirm shape selection",
        },
        DrawAction::ShapeSelectorCancel => HelpMeta {
            group: "Shape Selector",
            description: "Cancel shape selection",
        },
        DrawAction::TextEditorConfirm => HelpMeta {
            group: "Text Editor",
            description: "Confirm text edit",
        },
        DrawAction::TextEditorCancel => HelpMeta {
            group: "Text Editor",
            description: "Cancel text edit",
        },
        DrawAction::ToggleGrid => HelpMeta {
            group: "Tools",
            description: "Toggle grid overlay",
        },
        DrawAction::Help => HelpMeta {
            group: "General",
            description: "Help",
        },
        DrawAction::Quit => HelpMeta {
            group: "General",
            description: "Exit canvas view",
        },
    }
}

pub fn canvas_action_meta(a: CanvasAction) -> HelpMeta {
    match a {
        CanvasAction::MoveUp => HelpMeta {
            group: "Navigation",
            description: "Move up",
        },
        CanvasAction::MoveDown => HelpMeta {
            group: "Navigation",
            description: "Move down",
        },
        CanvasAction::MoveLeft => HelpMeta {
            group: "Navigation",
            description: "Move left",
        },
        CanvasAction::MoveRight => HelpMeta {
            group: "Navigation",
            description: "Move right",
        },
        CanvasAction::ZoomIn => HelpMeta {
            group: "Navigation",
            description: "Zoom in",
        },
        CanvasAction::ZoomOut => HelpMeta {
            group: "Navigation",
            description: "Zoom out",
        },
        CanvasAction::ZoomFineIn => HelpMeta {
            group: "Navigation",
            description: "Zoom in (fine)",
        },
        CanvasAction::ZoomFineOut => HelpMeta {
            group: "Navigation",
            description: "Zoom out (fine)",
        },
        CanvasAction::EditOrConnect => HelpMeta {
            group: "Editing",
            description: "Open / edit / connect",
        },
        CanvasAction::OpenContextMenu => HelpMeta {
            group: "Editing",
            description: "Context menu",
        },
        CanvasAction::Save => HelpMeta {
            group: "Editing",
            description: "Save canvas file",
        },
        CanvasAction::RenameConfirm => HelpMeta {
            group: "Editing",
            description: "Rename confirm",
        },
        CanvasAction::RenameCancel => HelpMeta {
            group: "Editing",
            description: "Rename cancel",
        },
        CanvasAction::ToggleGrid => HelpMeta {
            group: "Interface",
            description: "Toggle grid",
        },
        CanvasAction::ToggleEditorPane => HelpMeta {
            group: "Interface",
            description: "Toggle editor pane",
        },
        CanvasAction::CycleFocus => HelpMeta {
            group: "Interface",
            description: "Cycle focus",
        },
        CanvasAction::EditorUnfocus => HelpMeta {
            group: "Interface",
            description: "Exit editor focus",
        },
        CanvasAction::EditorSyncRaw => HelpMeta {
            group: "Interface",
            description: "Save raw editor changes",
        },
        CanvasAction::MenuClose => HelpMeta {
            group: "Menus & Popups",
            description: "Close context menu",
        },
        CanvasAction::MenuUp => HelpMeta {
            group: "Menus & Popups",
            description: "Menu select up",
        },
        CanvasAction::MenuDown => HelpMeta {
            group: "Menus & Popups",
            description: "Menu select down",
        },
        CanvasAction::MenuSelect => HelpMeta {
            group: "Menus & Popups",
            description: "Menu confirm",
        },
        CanvasAction::CloseEditor => HelpMeta {
            group: "Menus & Popups",
            description: "Close editor",
        },
        CanvasAction::CloseEditorAlt => HelpMeta {
            group: "Menus & Popups",
            description: "Close editor (alt)",
        },
        CanvasAction::ConfirmResize => HelpMeta {
            group: "Menus & Popups",
            description: "Resize confirm",
        },
        CanvasAction::CancelResize => HelpMeta {
            group: "Menus & Popups",
            description: "Resize cancel",
        },
        CanvasAction::Help => HelpMeta {
            group: "General",
            description: "Help",
        },
        CanvasAction::Quit => HelpMeta {
            group: "General",
            description: "Quit canvas view",
        },
    }
}

pub fn backup_action_meta(a: BackupAction) -> HelpMeta {
    match a {
        BackupAction::MoveUp => HelpMeta {
            group: "Navigation",
            description: "Move up",
        },
        BackupAction::MoveDown => HelpMeta {
            group: "Navigation",
            description: "Move down",
        },
        BackupAction::ScrollDiffUp => HelpMeta {
            group: "Navigation",
            description: "Scroll diff up",
        },
        BackupAction::ScrollDiffDown => HelpMeta {
            group: "Navigation",
            description: "Scroll diff down",
        },
        BackupAction::CycleSection => HelpMeta {
            group: "Navigation",
            description: "Cycle sections",
        },
        BackupAction::StageFile => HelpMeta {
            group: "Actions",
            description: "Stage file",
        },
        BackupAction::UnstageFile => HelpMeta {
            group: "Actions",
            description: "Unstage file",
        },
        BackupAction::StageAll => HelpMeta {
            group: "Actions",
            description: "Stage all changes",
        },
        BackupAction::Refresh => HelpMeta {
            group: "Actions",
            description: "Refresh status",
        },
        BackupAction::EnterCommit => HelpMeta {
            group: "Actions",
            description: "Enter commit",
        },
        BackupAction::ConfirmCommit => HelpMeta {
            group: "Actions",
            description: "Confirm commit",
        },
        BackupAction::CancelCommit => HelpMeta {
            group: "Actions",
            description: "Cancel commit",
        },
        BackupAction::Push => HelpMeta {
            group: "Actions",
            description: "Push to remote",
        },
        BackupAction::Pull => HelpMeta {
            group: "Actions",
            description: "Pull from remote",
        },
        BackupAction::OpenSettings => HelpMeta {
            group: "Actions",
            description: "Open settings",
        },
        BackupAction::CloseSettings => HelpMeta {
            group: "Actions",
            description: "Close settings",
        },
        BackupAction::NextField => HelpMeta {
            group: "Settings Fields",
            description: "Next field",
        },
        BackupAction::PrevField => HelpMeta {
            group: "Settings Fields",
            description: "Previous field",
        },
        BackupAction::ActivateField => HelpMeta {
            group: "Settings Fields",
            description: "Activate field",
        },
        BackupAction::ConfirmEditField => HelpMeta {
            group: "Settings Fields",
            description: "Confirm edit field",
        },
        BackupAction::CancelEditField => HelpMeta {
            group: "Settings Fields",
            description: "Cancel edit field",
        },
        BackupAction::Help => HelpMeta {
            group: "General",
            description: "Show help",
        },
        BackupAction::Back => HelpMeta {
            group: "General",
            description: "Back to list",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    macro_rules! assert_meta_nonempty {
        ($a:ty, $f:path) => {
            for a in <$a>::iter() {
                let m = $f(a);
                assert!(!m.group.is_empty() && !m.description.is_empty(), "{:?}", a);
            }
        };
    }

    #[test]
    fn list_meta_nonempty() {
        assert_meta_nonempty!(ListAction, list_action_meta);
    }
    #[test]
    fn edit_meta_nonempty() {
        assert_meta_nonempty!(EditAction, edit_action_meta);
    }
    #[test]
    fn graph_meta_nonempty() {
        assert_meta_nonempty!(GraphAction, graph_action_meta);
    }
    #[test]
    fn draw_meta_nonempty() {
        assert_meta_nonempty!(DrawAction, draw_action_meta);
    }
    #[test]
    fn canvas_meta_nonempty() {
        assert_meta_nonempty!(CanvasAction, canvas_action_meta);
    }
    #[test]
    fn backup_meta_nonempty() {
        assert_meta_nonempty!(BackupAction, backup_action_meta);
    }
}
