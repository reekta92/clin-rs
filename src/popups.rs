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
    RemoveAllTagsFromSelected,
}

pub struct ConfirmPopup {
    pub action: ConfirmAction,
    pub message: String,
    pub detail: Option<String>,
    pub confirm_label: String,
    pub is_destructive: bool,
    pub selected_button: usize,
}

pub struct TemplatePopup {
    pub all_templates: Vec<TemplateSummary>,
    pub filtered_templates: Vec<TemplateSummary>,
    pub input: TextArea<'static>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub focus: TemplatePopupFocus,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
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
    pub is_custom: Vec<bool>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub focus: ThemePopupFocus,
    pub general_is_solid: bool,
    pub graph_is_solid: bool,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagPopupFocus {
    Input,
    AllTagsList,
}

pub struct TagPopup {
    pub note_id: String,
    pub batch_note_ids: Option<Vec<String>>,
    pub input: TextArea<'static>,
    pub all_tags: Vec<String>,
    pub suggestions: Vec<String>,
    pub suggestion_index: usize,
    pub focus: TagPopupFocus,
    pub all_tags_selected: usize,
    pub scroll_offset: usize,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
}

pub enum FolderPopupMode {
    Create { parent_path: String },
    Rename { old_path: String },
}

pub struct RemoveTagsPopup {
    pub tags: Vec<String>,
    pub selected: std::collections::HashSet<usize>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub confirm: Option<ConfirmPopup>,
    pub tag_counts: Vec<usize>,
    pub total_selected: usize,
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
    pub scroll_offset: usize,
    pub focus: FolderPickerFocus,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLineHit {
    pub line_number: usize,
    pub snippet: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchNoteHit {
    pub note_id: std::sync::Arc<str>,
    pub match_count: usize,
    pub lines: Vec<SearchLineHit>,
    pub truncated: bool,
}
pub struct SearchPopup {
    pub input: TextArea<'static>,
    pub focus: SearchFocus,

    pub title_result_ids: Vec<std::sync::Arc<str>>,
    pub title_selected: usize,

    pub grep_results: Vec<SearchNoteHit>,
    pub grep_row_offsets: Vec<usize>,
    pub grep_expanded: std::collections::HashSet<std::sync::Arc<str>>,
    pub grep_selected: usize,
    pub globally_truncated: bool,
    pub read_errors: usize,

    pub results_scroll_offset: usize,
    pub original_index: usize,
    pub original_folder_expanded: std::collections::HashSet<String>,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
}

impl SearchPopup {
    pub fn rebuild_grep_offsets(&mut self) {
        let mut offsets = Vec::with_capacity(self.grep_results.len());
        let mut current = 0usize;
        for hit in &self.grep_results {
            offsets.push(current);
            current += 1;
            if self.grep_expanded.contains(&hit.note_id) {
                current += hit.lines.len();
            }
        }
        self.grep_row_offsets = offsets;
    }

    pub fn total_grep_rows(&self) -> usize {
        let mut count = 0usize;
        for hit in &self.grep_results {
            count += 1;
            if self.grep_expanded.contains(&hit.note_id) {
                count += hit.lines.len();
            }
        }
        if self.globally_truncated {
            count += 1;
        }
        count
    }
}

pub struct SelectionPopup {
    pub selected: usize,
}
/// A single item in the info popup layout.
#[derive(Debug, Clone)]
pub enum InfoItem {
    /// Rendered as an aligned 2-column Table.
    Metrics(Vec<(String, String)>),
    /// Rendered as a wrapping Paragraph with a heading.
    Text { heading: String, body: String },
    /// Visual separation.
    Spacer,
    /// A list of tags rendered as colored chips.
    Tags(Vec<String>),
}

pub struct InfoPopup {
    pub title: String,
    pub items: Vec<InfoItem>,
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
    pub scroll_offset: usize,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubnotesFocus {
    List,
    EditTitle,
    EditContent,
}
pub struct SubnotesPopup {
    pub parent_id: String,
    pub subnotes: Vec<crate::storage::SubNote>,
    pub selected: usize,
    pub focus: SubnotesFocus,
    pub scroll_offset: usize,
    pub title_input: TextArea<'static>,
    pub content_input: TextArea<'static>,
    pub is_dirty: bool,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
}

/// The single active (non-confirm) popup. Only one is ever active at a time;
/// a `ConfirmPopup` layers separately on top via [`PopupManager::confirm`].
pub enum ActivePopup {
    Template(TemplatePopup),
    Theme(ThemePopup),
    Info(InfoPopup),
    Tag(TagPopup),
    IconMode(SelectionPopup),
    HintBarStyle(SelectionPopup),
    RemoveTags(RemoveTagsPopup),
    KeybindPreset(SelectionPopup),
    Sort(SelectionPopup),
    Folder(FolderPopup),
    FolderPicker(FolderPicker),
    NoteRename(NoteRenamePopup),
    CreateNote(NoteCreatePopup, NoteFormat),
    Import(ImportPopup),
    CreateFormat(CreateFormatPopup),
    Search(SearchPopup),
    TrashView(TrashView),
    Goals(GoalsPopup),
    Subnotes(Box<SubnotesPopup>),
}

impl ActivePopup {
    pub fn draw(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        keybinds: &crate::keybinds::Keybinds,
        mouse_pos: Option<(u16, u16)>,
    ) {
        match self {
            ActivePopup::Theme(p) => {
                crate::ui::draw_theme_popup(frame, p, area, theme, keybinds, mouse_pos)
            }
            ActivePopup::IconMode(p) => crate::ui::draw_option_list_popup(
                frame,
                area,
                "ICON MODE",
                &["Nerd Font", "Unicode", "None"],
                p.selected,
                keybinds,
                theme,
                mouse_pos,
            ),
            ActivePopup::HintBarStyle(p) => crate::ui::draw_option_list_popup(
                frame,
                area,
                "HINT BAR STYLE",
                &crate::config::HintBarStyle::ALL.map(|s| s.name()),
                p.selected,
                keybinds,
                theme,
                mouse_pos,
            ),
            ActivePopup::KeybindPreset(p) => crate::ui::draw_option_list_popup(
                frame,
                area,
                "KEYBIND PRESET",
                &[
                    "default \u{2014} Default CUA",
                    "helix \u{2014} Space leader",
                    "vim \u{2014} : commands",
                    "emacs \u{2014} Ctrl-x prefix",
                ],
                p.selected,
                keybinds,
                theme,
                mouse_pos,
            ),
            ActivePopup::Sort(p) => crate::ui::draw_option_list_popup(
                frame,
                area,
                "SORT BY",
                &[
                    "Title (A-Z)",
                    "Title (Z-A)",
                    "Modified (newest)",
                    "Modified (oldest)",
                ],
                p.selected,
                keybinds,
                theme,
                mouse_pos,
            ),
            ActivePopup::CreateFormat(p) => crate::ui::draw_option_list_popup(
                frame,
                area,
                "CREATE NEW",
                &[
                    "Markdown Note (.md)",
                    "Plain Text (.txt)",
                    "Drawing (.draw)",
                    "Canvas (.canvas)",
                ],
                p.selected,
                keybinds,
                theme,
                mouse_pos,
            ),
            ActivePopup::Subnotes(p) => crate::ui::draw_subnotes_popup(frame, p, area, theme),
            ActivePopup::Info(p) => crate::ui::draw_info_popup(frame, area, p, theme),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupTextField {
    CreateNote,
    Goals,
    NoteRename,
    Import,
    Folder,
    Tag,
    FolderPicker,
    Search,
    Template,
    SubnotesTitle,
    SubnotesContent,
}

#[derive(Default)]
pub struct PopupManager {
    /// Layered confirm dialog drawn on top of `active` when present.
    pub confirm: Option<ConfirmPopup>,
    /// The single active popup, if any.
    pub active: Option<ActivePopup>,
    pub(crate) text_selection: Option<(PopupTextField, crate::text_edit::MouseTextSelection)>,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<i32>,
}

impl PopupManager {
    pub fn has_any(&self) -> bool {
        self.confirm.is_some() || self.active.is_some()
    }

    /// True when a popup with a text input is active (and no confirm overlay
    /// is intercepting keys). Mirrors the prior text-input popup set.
    pub fn has_text_input(&self) -> bool {
        if self.confirm.is_some() {
            return false;
        }
        match &self.active {
            Some(ActivePopup::CreateNote(..))
            | Some(ActivePopup::Import(_))
            | Some(ActivePopup::Folder(_))
            | Some(ActivePopup::FolderPicker(_))
            | Some(ActivePopup::NoteRename(_))
            | Some(ActivePopup::Search(_))
            | Some(ActivePopup::Template(_))
            | Some(ActivePopup::Tag(_))
            | Some(ActivePopup::Goals(_)) => true,
            Some(ActivePopup::Subnotes(popup)) => popup.focus != SubnotesFocus::List,
            _ => false,
        }
    }

    pub fn clear_all(&mut self) {
        self.active = None;
        self.confirm = None;
        self.text_selection = None;
    }
}
