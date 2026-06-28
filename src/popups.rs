use crate::templates::TemplateSummary;
use ratatui_textarea::TextArea;

pub enum ConfirmAction {
    DeleteNote {
        note_id: String,
        title: String,
    },
    DeleteFolder {
        path: String,
    },
    DeleteTag {
        tag: String,
    },
    DeleteTemplate {
        filename: String,
        name: String,
    },
    DeleteFromTrash {
        item: trash::TrashItem,
    },
    EmptyTrash {
        items: Vec<trash::TrashItem>,
    },
    BulkDeleteItems {
        note_ids: Vec<String>,
        folder_paths: Vec<String>,
    },
    QuitApp,
}

pub struct ConfirmPopup {
    pub action: ConfirmAction,
    pub message: String,
    pub detail: Option<String>,
    pub confirm_label: String,
    pub is_destructive: bool,
    pub selected_button: usize,
}

pub struct ContextMenu {
    pub x: u16,
    pub y: u16,
    pub selected: usize,
}

pub struct TemplatePopup {
    pub all_templates: Vec<TemplateSummary>,
    pub filtered_templates: Vec<TemplateSummary>,
    pub input: TextArea<'static>,
    pub selected: usize,
    pub focus: TemplatePopupFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplatePopupFocus {
    Search,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePopupFocus {
    ThemeList,
    GeneralBg,
    GraphBg,
}

pub struct ThemePopup {
    pub themes: Vec<String>,
    pub selected: usize,
    pub focus: ThemePopupFocus,
    pub general_is_solid: bool,
    pub graph_is_solid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagPopupFocus {
    Input,
    AllTagsList,
}

pub struct TagPopup {
    pub note_id: String,
    pub input: TextArea<'static>,
    pub all_tags: Vec<String>,
    pub suggestions: Vec<String>,
    pub suggestion_index: usize,
    pub focus: TagPopupFocus,
    pub all_tags_selected: usize,
}

pub enum FolderPopupMode {
    Create { parent_path: String },
    Rename { old_path: String },
}

pub struct FolderPopup {
    pub mode: FolderPopupMode,
    pub input: TextArea<'static>,
}

pub enum FolderPickerMode {
    MoveNote {
        note_id: String,
    },
    CopyNote {
        note_id: String,
    },
    MoveFolder {
        folder_path: String,
    },
    BulkMoveNotes {
        note_ids: Vec<String>,
    },
    BulkCopyNotes {
        note_ids: Vec<String>,
    },
    BulkMoveFolders {
        folder_paths: Vec<String>,
    },
    BulkCopyFolders {
        folder_paths: Vec<String>,
    },
    BulkMoveMixed {
        note_ids: Vec<String>,
        folder_paths: Vec<String>,
    },
    BulkCopyMixed {
        note_ids: Vec<String>,
        folder_paths: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderPickerFocus {
    Search,
    Results,
}

pub struct FolderPicker {
    pub mode: FolderPickerMode,
    pub all_folders: Vec<String>,
    pub filtered_folders: Vec<String>,
    pub selected: usize,
    pub input: TextArea<'static>,
    pub focus: FolderPickerFocus,
}

pub struct NoteRenamePopup {
    pub note_id: String,
    pub input: TextArea<'static>,
}

pub struct NoteCreatePopup {
    pub folder: String,
    pub input: TextArea<'static>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSource {
    File,
    Csv,
    Json,
    Url,
    Clipboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportTarget {
    NewNote,
    AppendCurrent,
}

pub struct ImportPopup {
    pub source: ImportSource,
    pub target: ImportTarget,
    pub note_id: Option<String>,
    pub input: TextArea<'static>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFocus {
    Input,
    Results,
}

#[derive(Debug)]
pub struct SearchPopup {
    pub input: TextArea<'static>,
    pub focus: SearchFocus,

    pub title_results: Vec<String>,
    pub title_result_indices: Vec<usize>,
    pub title_selected: usize,

    pub grep_results: Vec<String>,
    pub grep_result_indices: Vec<usize>,
    pub grep_is_header: Vec<bool>,
    pub grep_expanded: std::collections::HashSet<usize>,

    pub grep_selected: usize,
    pub original_index: usize,
    pub original_folder_expanded: std::collections::HashSet<String>,
}
pub struct SortPopup {
    pub selected: usize,
}

pub struct IconModePopup {
    pub selected: usize,
}

pub struct HintBarStylePopup {
    pub selected: usize,
}

pub struct KeybindPresetPopup {
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteFormat {
    Markdown,
    Draw,
    Canvas,
    PlainText,
}

pub struct CreateFormatPopup {
    pub folder: String,
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalsPopupMode {
    WordGoal,
    NoteGoal,
}

pub struct GoalsPopup {
    pub mode: GoalsPopupMode,
    pub input: TextArea<'static>,
}

pub struct TrashView {
    pub items: Vec<trash::TrashItem>,
    pub selected: usize,
}

/// The single active (non-confirm) popup. Only one is ever active at a time;
/// a `ConfirmPopup` layers separately on top via [`PopupManager::confirm`].
pub enum ActivePopup {
    Template(TemplatePopup),
    Theme(ThemePopup),
    Tag(TagPopup),
    IconMode(IconModePopup),
    HintBarStyle(HintBarStylePopup),
    KeybindPreset(KeybindPresetPopup),
    Sort(SortPopup),
    Folder(FolderPopup),
    FolderPicker(FolderPicker),
    NoteRename(NoteRenamePopup),
    CreateNote(NoteCreatePopup, NoteFormat),
    Import(ImportPopup),
    CreateFormat(CreateFormatPopup),
    Search(SearchPopup),
    ContextMenu(ContextMenu),
    TrashView(TrashView),
    Goals(GoalsPopup),
}

#[derive(Default)]
pub struct PopupManager {
    /// Layered confirm dialog drawn on top of `active` when present.
    pub confirm: Option<ConfirmPopup>,
    /// The single active popup, if any.
    pub active: Option<ActivePopup>,
}

impl PopupManager {
    pub fn has_any(&self) -> bool {
        self.confirm.is_some() || self.active.is_some()
    }

    /// True when a popup with a text input is active (and no confirm overlay
    /// is intercepting keys). Mirrors the prior text-input popup set.
    pub fn has_text_input(&self) -> bool {
        self.confirm.is_none()
            && matches!(
                self.active,
                Some(ActivePopup::CreateNote(..))
                    | Some(ActivePopup::Import(_))
                    | Some(ActivePopup::Folder(_))
                    | Some(ActivePopup::FolderPicker(_))
                    | Some(ActivePopup::NoteRename(_))
                    | Some(ActivePopup::Search(_))
                    | Some(ActivePopup::Template(_))
                    | Some(ActivePopup::Tag(_))
                    | Some(ActivePopup::Goals(_))
            )
    }

    pub fn clear_all(&mut self) {
        self.active = None;
        self.confirm = None;
    }
}
