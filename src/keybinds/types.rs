use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Open,
    Delete,
    Quit,
    Help,
    OpenLocation,
    CycleFocus,
    Confirm,
    Cancel,
    ToggleExternalEditor,
    NewFromTemplate,
    CreateFolder,
    CreateNote,
    RenameFolder,
    MoveNote,
    ManageTags,
    CollapseFolder,
    ExpandFolder,
    OpenCommandPalette,

    Rename,
    Duplicate,
    TogglePin,
    CycleSort,
    Search,
    JumpToTop,
    JumpToBottom,
    PageUp,
    PageDown,
    OpenTrash,
    TogglePreview,
    TogglePreviewFullscreen,
    TogglePreviewWrap,
    ToggleCalendar,
    OpenGraph,
    OpenCanvas,
    CreatePinstar,
    ToggleSelectMode,
    ToggleSelectItem,
    CollapseAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditAction {
    Quit,
    Back,
    CycleFocus,

    SelectAll,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    DeleteWord,
    DeleteNextWord,
    MoveToTop,
    MoveToBottom,
    ToggleMarkdownPreview,
    TogglePreviewFullscreen,
    TogglePreviewWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpAction {
    Close,
    ScrollUp,
    ScrollDown,
    NextTab,
    PrevTab,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphAction {
    Quit,
    PanUp,
    PanDown,
    PanLeft,
    PanRight,
    ZoomIn,
    ZoomOut,
    OpenNote,
    AutoFit,
    Help,
    ToggleSearch,
    ToggleMinimap,
    ToggleLegend,
    ToggleGrid,
    ToggleStatus,
    Refresh,
    ReloadConfig,
    TogglePreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawAction {
    Quit,
    Help,
    SelectDrawTool,
    ToggleShapeSelector,
    SelectTextTool,
    SelectEraseTool,
    ShapeSelectorUp,
    ShapeSelectorDown,
    ShapeSelectorConfirm,
    ShapeSelectorCancel,
    TextEditorConfirm,
    TextEditorCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasAction {
    Quit,
    Save,
    ZoomFineIn,
    ZoomFineOut,
    ZoomIn,
    ZoomOut,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    EditOrConnect,
    OpenContextMenu,
    ToggleGrid,
    ToggleEditorPane,
    CycleFocus,
    Help,
    RenameConfirm,
    RenameCancel,
    MenuClose,
    MenuUp,
    MenuDown,
    MenuSelect,
    CloseEditor,
    CloseEditorAlt,
    ConfirmResize,
    CancelResize,
    EditorUnfocus,
    EditorSyncRaw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupAction {
    Back,
    MoveDown,
    MoveUp,
    ScrollDiffDown,
    ScrollDiffUp,
    Refresh,
    EnterCommit,
    Push,
    OpenSettings,
    CycleSection,
    CancelCommit,
    ConfirmCommit,
    CloseSettings,
    ToggleFileSelect,
    NextField,
    PrevField,
    ActivateField,
    CancelEditField,
    ConfirmEditField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTreeAction {
    MoveUp,
    MoveDown,
    ToggleCollapse,
    ExpandAll,
    CollapseAll,
    Open,
    Back,
    Help,
}
