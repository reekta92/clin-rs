use ratatui_textarea::TextArea;
use crate::templates::TemplateSummary;

pub enum ConfirmAction {
    DeleteNote { note_id: String, title: String },
    DeleteFolder { path: String },
    DeleteTag { tag: String },
    DeleteFromTrash { item: trash::TrashItem },
    EmptyTrash { items: Vec<trash::TrashItem> },
    BulkDeleteNotes { note_ids: Vec<String> },
}

pub struct ConfirmPopup {
    pub action: ConfirmAction,
    pub title: String,
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
    pub templates: Vec<TemplateSummary>,
    pub selected: usize,
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

pub struct TagPopup {
    pub note_id: String,
    pub input: TextArea<'static>,
    pub all_tags: Vec<String>,
    pub suggestions: Vec<String>,
    pub suggestion_index: usize,
}

pub struct FilterTagPopup {
    pub input: TextArea<'static>,
    pub all_tags: Vec<String>,
    pub suggestions: Vec<String>,
    pub suggestion_index: usize,
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
    MoveNote { note_id: String },
    MoveFolder { folder_path: String },
    BulkMoveNotes { note_ids: Vec<String> },
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
    pub query: String,
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
pub enum SearchPopupFocus {
    Notes,
    Grep,
    GrepResults,
}

pub struct SearchPopup {
    pub note_input: TextArea<'static>,
    pub grep_input: TextArea<'static>,
    pub grep_results: Vec<String>,
    pub grep_result_note_indices: Vec<usize>,
    pub grep_selected: usize,
    pub focus: SearchPopupFocus,
    pub original_index: usize,
    pub original_folder_expanded: std::collections::HashSet<String>,
}

pub struct TrashView {
    pub items: Vec<trash::TrashItem>,
    pub selected: usize,
}

#[derive(Default)]
pub struct PopupManager {
    pub confirm: Option<ConfirmPopup>,
    pub template: Option<TemplatePopup>,
    pub theme: Option<ThemePopup>,
    pub tag: Option<TagPopup>,
    pub filter_tag: Option<FilterTagPopup>,
    pub folder: Option<FolderPopup>,
    pub folder_picker: Option<FolderPicker>,
    pub note_rename: Option<NoteRenamePopup>,
    pub note_create: Option<NoteCreatePopup>,
    pub draw_create: Option<NoteCreatePopup>,
    pub canvas_create: Option<NoteCreatePopup>,

    pub search: Option<SearchPopup>,
    pub context_menu: Option<ContextMenu>,
    pub trash_view: Option<TrashView>,
}
