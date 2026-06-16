use crate::templates::TemplateSummary;
use ratatui_textarea::TextArea;

pub enum ConfirmAction {
    DeleteNote { note_id: String, title: String },
    DeleteFolder { path: String },
    DeleteTag { tag: String },
    DeleteTemplate { filename: String, name: String },
    DeleteFromTrash { item: trash::TrashItem },
    EmptyTrash { items: Vec<trash::TrashItem> },
    BulkDeleteNotes { note_ids: Vec<String> },
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
    MoveNote { note_id: String },
    CopyNote { note_id: String },
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
    pub sort: Option<SortPopup>,
    pub folder: Option<FolderPopup>,
    pub folder_picker: Option<FolderPicker>,
    pub note_rename: Option<NoteRenamePopup>,
    pub create_note: Option<(NoteCreatePopup, NoteFormat)>,
    pub import: Option<ImportPopup>,
    pub create_format: Option<CreateFormatPopup>,

    pub search: Option<SearchPopup>,
    pub context_menu: Option<ContextMenu>,
    pub trash_view: Option<TrashView>,
}

impl PopupManager {
    pub fn has_any(&self) -> bool {
        self.confirm.is_some()
            || self.template.is_some()
            || self.theme.is_some()
            || self.tag.is_some()
            || self.sort.is_some()
            || self.folder.is_some()
            || self.folder_picker.is_some()
            || self.note_rename.is_some()
            || self.create_note.is_some()
            || self.import.is_some()
            || self.create_format.is_some()
            || self.search.is_some()
            || self.context_menu.is_some()
            || self.trash_view.is_some()
    }

    pub fn has_text_input(&self) -> bool {
        self.create_note.is_some()
            || self.import.is_some()
            || self.folder.is_some()
            || self.folder_picker.is_some()
            || self.note_rename.is_some()
            || self.search.is_some()
            || self.template.is_some()
            || self.tag.is_some()
    }

    pub fn clear_all(&mut self) {
        self.confirm = None;
        self.template = None;
        self.theme = None;
        self.tag = None;
        self.sort = None;
        self.folder = None;
        self.folder_picker = None;
        self.note_rename = None;
        self.create_note = None;
        self.import = None;
        self.create_format = None;
        self.search = None;
        self.context_menu = None;
        self.trash_view = None;
    }
}
