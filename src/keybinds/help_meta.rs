use crate::keybinds::types::{
    BackupAction, CanvasAction, DrawAction, EditAction, GraphAction, ListAction,
};

#[derive(Clone, Copy)]
pub struct HelpMeta {
    pub group: &'static str,
    pub description: &'static str,
}

const fn meta(group: &'static str, description: &'static str) -> HelpMeta {
    HelpMeta { group, description }
}

pub fn list_group_order() -> &'static [&'static str] {
    &["Navigation", "Actions", "Display", "General"]
}
pub fn edit_group_order() -> &'static [&'static str] {
    &["Navigation", "Editing", "Preview", "Panels", "General"]
}
pub fn graph_group_order() -> &'static [&'static str] {
    &["Navigation", "Actions", "Display", "Menu", "System"]
}
pub fn draw_group_order() -> &'static [&'static str] {
    &[
        "Tools",
        "Shape Selector",
        "Text Editor",
        "Context Menu",
        "Editing",
        "General",
    ]
}
pub fn canvas_group_order() -> &'static [&'static str] {
    &[
        "Navigation",
        "Editing",
        "Connections",
        "Display",
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
        ListAction::MoveUp => meta("Navigation", "Move up"),
        ListAction::MoveDown => meta("Navigation", "Move down"),
        ListAction::MoveLeft => meta("Navigation", "Move left (grid)"),
        ListAction::MoveRight => meta("Navigation", "Move right (grid)"),
        ListAction::JumpToTop => meta("Navigation", "Jump to top"),
        ListAction::JumpToBottom => meta("Navigation", "Jump to bottom"),
        ListAction::PageUp => meta("Navigation", "Scroll up half page"),
        ListAction::PageDown => meta("Navigation", "Scroll down half page"),
        ListAction::CollapseFolder => meta("Navigation", "Collapse folder"),
        ListAction::ExpandFolder => meta("Navigation", "Expand folder"),
        ListAction::Open => meta("Actions", "Open selected item"),
        ListAction::CreateNote => meta("Actions", "Create new note"),
        ListAction::CreateFolder => meta("Actions", "Create new folder"),
        ListAction::Rename => meta("Actions", "Rename note"),
        ListAction::RenameFolder => meta("Actions", "Rename folder"),
        ListAction::Delete => meta("Actions", "Delete"),
        ListAction::Duplicate => meta("Actions", "Duplicate note"),
        ListAction::MoveNote => meta("Actions", "Move note or folder"),
        ListAction::MoveToParent => meta("Actions", "Move note to parent folder"),
        ListAction::ManageTags => meta("Actions", "Manage tags"),
        ListAction::RemoveTagsFromSelected => meta("Actions", "Remove tags from selected notes"),
        ListAction::TogglePin => meta("Actions", "Toggle pin"),
        ListAction::ToggleExternalEditor => meta("Actions", "Toggle external editor"),
        ListAction::OpenLocation => meta("Actions", "Open file location"),
        ListAction::ManageSubnotes => meta("Actions", "Manage subnotes"),
        ListAction::Search => meta("Display", "Search"),
        ListAction::ToggleSelectMode => meta("Display", "Toggle select mode"),
        ListAction::ToggleSelectItem => meta("Display", "Toggle select item"),
        ListAction::TogglePreview => meta("Display", "Toggle preview pane"),
        ListAction::TogglePreviewFullscreen => meta("Display", "Toggle preview fullscreen"),
        ListAction::ToggleWrap => meta("Display", "Toggle word wrap (editor and preview)"),
        ListAction::PreviewPageUp => meta("Display", "Page preview up"),
        ListAction::ToggleCalendar => meta("Display", "Toggle calendar"),
        ListAction::OpenCommandPalette => meta("Display", "Open command palette"),
        ListAction::OpenGraph => meta("Display", "Open graph view"),
        ListAction::OpenCanvas => meta("Display", "Open canvas view"),
        ListAction::CollapseAll => meta("Display", "Collapse all folders"),
        ListAction::ExpandAll => meta("Display", "Expand all folders"),
        ListAction::ExpandToLevel => meta("Display", "Expand folders to level (e.g. 3E)"),
        ListAction::ToggleFoldersFirst => meta("Display", "Toggle folders-first sort"),
        ListAction::RefreshNotes => meta("Display", "Refresh notes (external changes)"),
        ListAction::Quit => meta("General", "Quit"),
        ListAction::Help => meta("General", "Help"),
        ListAction::CycleFocus => meta("General", "Cycle focus between panes"),
        ListAction::ReverseCycleFocus => meta("General", "Reverse cycle focus between panes"),
        ListAction::Confirm => meta("General", "Confirm action"),
        ListAction::Cancel => meta("General", "Cancel action"),
        ListAction::NewFromTemplate => meta("General", "New note from template"),
        ListAction::CycleSort => meta("General", "Cycle sort order"),
        ListAction::OpenTrash => meta("General", "Open trash"),
        ListAction::ShowInfo => meta("General", "Show note info"),
        ListAction::PreviewPageDown => meta("Display", "Page preview down"),
    }
}

pub fn edit_action_meta(a: EditAction) -> HelpMeta {
    match a {
        EditAction::CycleFocus => meta("Navigation", "Cycle focus (Title, Content)"),
        EditAction::InsertTab => meta("Editing", "Insert tab character"),
        EditAction::Back => meta("Navigation", "Return to notes (auto-saves)"),
        EditAction::Copy => meta("Editing", "Copy"),
        EditAction::Cut => meta("Editing", "Cut"),
        EditAction::Paste => meta("Editing", "Paste"),
        EditAction::SelectAll => meta("Editing", "Select all"),
        EditAction::Undo => meta("Editing", "Undo"),
        EditAction::Redo => meta("Editing", "Redo"),
        EditAction::DeleteWord => meta("Editing", "Delete previous word"),
        EditAction::DeleteNextWord => meta("Editing", "Delete next word"),
        EditAction::MoveToTop => meta("Editing", "Move cursor to top"),
        EditAction::MoveToBottom => meta("Editing", "Move cursor to bottom"),
        EditAction::ManageSubnotes => meta("Editing", "Manage subnotes"),
        EditAction::ToggleMarkdownPreview => meta("Preview", "Toggle markdown preview"),
        EditAction::TogglePreviewFullscreen => meta("Preview", "Toggle preview fullscreen"),
        EditAction::ToggleWrap => meta("Editing", "Toggle word wrap (editor and preview)"),
        EditAction::PreviewPageUp => meta("Preview", "Page preview up"),
        EditAction::PreviewPageDown => meta("Preview", "Page preview down"),
        EditAction::ToggleOutline => meta("Panels", "Toggle outline pane"),
        EditAction::ToggleLinks => meta("Panels", "Toggle links pane"),
        EditAction::PreviewLink => meta("Panels", "Preview linked note under cursor"),
        EditAction::GoToLine => meta("Editor", "Go to line number"),
        EditAction::PasteImage => meta("Editing", "Paste image from clipboard"),
        EditAction::InsertImageFromFile => meta("Editing", "Insert image from file"),
        EditAction::Find => meta("Editing", "Find in document"),
        EditAction::InsertDate => meta("Editing", "Insert date/time"),
        EditAction::Save => meta("General", "Save"),
    }
}

pub fn graph_action_meta(a: GraphAction) -> HelpMeta {
    match a {
        GraphAction::PanUp => meta("Navigation", "Pan up"),
        GraphAction::PanDown => meta("Navigation", "Pan down"),
        GraphAction::PanLeft => meta("Navigation", "Pan left"),
        GraphAction::PanRight => meta("Navigation", "Pan right"),
        GraphAction::ZoomIn => meta("Navigation", "Zoom in"),
        GraphAction::ZoomOut => meta("Navigation", "Zoom out"),
        GraphAction::OpenNote => meta("Navigation", "Open selected note"),
        GraphAction::AutoFit => meta("Navigation", "Auto-fit graph to viewport"),
        GraphAction::ToggleSearch => meta("Navigation", "Search nodes"),
        GraphAction::ToggleMinimap => meta("Display", "Toggle minimap"),
        GraphAction::ToggleLegend => meta("Display", "Toggle legend"),
        GraphAction::ToggleGrid => meta("Display", "Toggle background grid"),
        GraphAction::ToggleStatus => meta("Display", "Toggle status bar"),
        GraphAction::TogglePreview => meta("Display", "Toggle preview"),
        GraphAction::OpenContextMenu => meta("Menu", "Open context menu"),
        GraphAction::CreateConnection => meta("Actions", "Create connection"),
        GraphAction::DeleteConnection => meta("Actions", "Delete connection"),
        GraphAction::LocalGraph => meta("Actions", "Local graph"),
        GraphAction::ShowGroup => meta("Actions", "Show group"),
        GraphAction::DeleteNode => meta("Actions", "Delete node"),
        GraphAction::MenuClose => meta("Menu", "Close menu"),
        GraphAction::MenuUp => meta("Menu", "Menu up"),
        GraphAction::MenuDown => meta("Menu", "Menu down"),
        GraphAction::MenuSelect => meta("Menu", "Select menu item"),
        GraphAction::LookingGlass => meta("Display", "Toggle looking glass"),
        GraphAction::Refresh => meta("System", "Refresh physics"),
        GraphAction::Help => meta("System", "Help"),
        GraphAction::Quit => meta("System", "Quit graph view"),
    }
}

pub fn draw_action_meta(a: DrawAction) -> HelpMeta {
    match a {
        DrawAction::SelectCursorTool => meta("Tools", "Select and transform element"),
        DrawAction::SelectDrawTool => meta("Tools", "Draw freehand strokes"),
        DrawAction::ToggleShapeSelector => meta("Tools", "Shape tool (opens picker)"),
        DrawAction::SelectTextTool => meta("Tools", "Place text label"),
        DrawAction::SelectEraseTool => meta("Tools", "Erase elements"),
        DrawAction::ShapeSelectorUp => meta("Shape Selector", "Select previous shape"),
        DrawAction::ShapeSelectorDown => meta("Shape Selector", "Select next shape"),
        DrawAction::ShapeSelectorConfirm => meta("Shape Selector", "Confirm shape selection"),
        DrawAction::ShapeSelectorCancel => meta("Shape Selector", "Cancel shape selection"),
        DrawAction::ToggleColorSelector => meta("Tools", "Color picker (opens picker)"),
        DrawAction::ColorSelectorUp => meta("Color Selector", "Select previous color"),
        DrawAction::ColorSelectorDown => meta("Color Selector", "Select next color"),
        DrawAction::ColorSelectorConfirm => meta("Color Selector", "Confirm color selection"),
        DrawAction::ColorSelectorCancel => meta("Color Selector", "Cancel color selection"),
        DrawAction::TextEditorConfirm => meta("Text Editor", "Confirm text edit"),
        DrawAction::TextEditorCancel => meta("Text Editor", "Cancel text edit"),
        DrawAction::MenuClose => meta("Context Menu", "Close context menu"),
        DrawAction::MenuUp => meta("Context Menu", "Select previous menu item"),
        DrawAction::MenuDown => meta("Context Menu", "Select next menu item"),
        DrawAction::MenuSelect => meta("Context Menu", "Activate selected menu item"),
        DrawAction::Copy => meta("Editing", "Copy selected element"),
        DrawAction::Paste => meta("Editing", "Paste copied element"),
        DrawAction::Undo => meta("Editing", "Undo last draw change"),
        DrawAction::Redo => meta("Editing", "Redo last draw change"),
        DrawAction::ToggleGrid => meta("Tools", "Toggle grid overlay"),
        DrawAction::Help => meta("General", "Help"),
        DrawAction::Quit => meta("General", "Exit draw view"),
    }
}

pub fn canvas_action_meta(a: CanvasAction) -> HelpMeta {
    match a {
        CanvasAction::Undo => meta("Editing", "Undo last canvas edit"),
        CanvasAction::Redo => meta("Editing", "Redo canvas edit"),
        CanvasAction::MoveUp => meta("Navigation", "Move up"),
        CanvasAction::MoveDown => meta("Navigation", "Move down"),
        CanvasAction::MoveLeft => meta("Navigation", "Move left"),
        CanvasAction::MoveRight => meta("Navigation", "Move right"),
        CanvasAction::ZoomIn => meta("Navigation", "Zoom in"),
        CanvasAction::ZoomOut => meta("Navigation", "Zoom out"),
        CanvasAction::ZoomFineIn => meta("Navigation", "Zoom in (fine)"),
        CanvasAction::ZoomFineOut => meta("Navigation", "Zoom out (fine)"),
        CanvasAction::EditOrConnect => meta("Editing", "Open / edit / connect"),
        CanvasAction::OpenContextMenu => meta("Editing", "Context menu"),
        CanvasAction::Save => meta("Editing", "Save canvas file"),
        CanvasAction::RenameConfirm => meta("Editing", "Rename confirm"),
        CanvasAction::ToggleOrthogonal => meta("Display", "Toggle orthogonal edge routing"),
        CanvasAction::RenameCancel => meta("Editing", "Rename cancel"),
        CanvasAction::ToggleGrid => meta("Interface", "Toggle grid"),
        CanvasAction::ToggleEditorPane => meta("Interface", "Toggle editor pane"),
        CanvasAction::CycleFocus => meta("Interface", "Cycle focus"),
        CanvasAction::EditorUnfocus => meta("Interface", "Exit editor focus"),
        CanvasAction::MenuClose => meta("Menus & Popups", "Close context menu"),
        CanvasAction::MenuUp => meta("Menus & Popups", "Menu select up"),
        CanvasAction::MenuDown => meta("Menus & Popups", "Menu select down"),
        CanvasAction::MenuSelect => meta("Menus & Popups", "Menu confirm"),
        CanvasAction::CloseEditor => meta("Menus & Popups", "Close editor"),
        CanvasAction::CloseEditorAlt => meta("Menus & Popups", "Close editor (alt)"),
        CanvasAction::ConfirmResize => meta("Menus & Popups", "Resize confirm"),
        CanvasAction::CancelResize => meta("Menus & Popups", "Resize cancel"),
        CanvasAction::CreateConnection => {
            meta("Connections", "Create connection from selected node")
        }
        CanvasAction::DeleteConnection => {
            meta("Connections", "Delete connection from selected node")
        }
        CanvasAction::DeleteAllConnections => {
            meta("Connections", "Delete all connections on selected node")
        }
        CanvasAction::RenameNode => meta("Editing", "Rename selected node"),
        CanvasAction::ResizeMode => meta("Editing", "Enter resize mode"),
        CanvasAction::SetColor => meta("Editing", "Set color of selected node(s)"),
        CanvasAction::DeleteNode => meta("Editing", "Delete selected node(s)"),
        CanvasAction::AddTextNode => meta("Editing", "Add text node at cursor"),
        CanvasAction::AddGroup => meta("Editing", "Add group at cursor"),
        CanvasAction::AddImageNode => meta("Editing", "Add image node at cursor"),
        CanvasAction::Help => meta("General", "Help"),
        CanvasAction::Quit => meta("General", "Quit canvas view"),
    }
}

pub fn backup_action_meta(a: BackupAction) -> HelpMeta {
    match a {
        BackupAction::MoveUp => meta("Navigation", "Move up"),
        BackupAction::MoveDown => meta("Navigation", "Move down"),
        BackupAction::ScrollDiffUp => meta("Navigation", "Scroll diff up"),
        BackupAction::ScrollDiffDown => meta("Navigation", "Scroll diff down"),
        BackupAction::CycleSection => meta("Navigation", "Cycle sections"),
        BackupAction::StageFile => meta("Actions", "Stage file"),
        BackupAction::UnstageFile => meta("Actions", "Unstage file"),
        BackupAction::StageAll => meta("Actions", "Stage all changes"),
        BackupAction::Refresh => meta("Actions", "Refresh status"),
        BackupAction::EnterCommit => meta("Actions", "Enter commit"),
        BackupAction::ConfirmCommit => meta("Actions", "Confirm commit"),
        BackupAction::CancelCommit => meta("Actions", "Cancel commit"),
        BackupAction::Push => meta("Actions", "Push to remote"),
        BackupAction::Pull => meta("Actions", "Pull from remote"),
        BackupAction::OpenSettings => meta("Actions", "Open settings"),
        BackupAction::CloseSettings => meta("Actions", "Close settings"),
        BackupAction::NextField => meta("Settings Fields", "Next field"),
        BackupAction::PrevField => meta("Settings Fields", "Previous field"),
        BackupAction::ActivateField => meta("Settings Fields", "Activate field"),
        BackupAction::ConfirmEditField => meta("Settings Fields", "Confirm edit field"),
        BackupAction::CancelEditField => meta("Settings Fields", "Cancel edit field"),
        BackupAction::Help => meta("General", "Show help"),
        BackupAction::Back => meta("General", "Back to list"),
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
