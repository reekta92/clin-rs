use std::collections::HashSet;

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
    pub items: Vec<&'static str>,
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
    pub results_scroll_offset: usize,
    pub original_index: usize,
    pub original_folder_expanded: std::collections::HashSet<String>,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
}

pub struct SortPopup {
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
}

pub struct InfoPopup {
    pub title: String,
    pub items: Vec<InfoItem>,
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
/// 0-based flat index of the `vis_pos`-th visible grep item.
/// Children of collapsed headers are skipped. None if out of range.
#[allow(clippy::implicit_hasher)]
pub fn grep_visible_to_flat(
    is_header: &[bool],
    expanded: &HashSet<usize>,
    vis_pos: usize,
) -> Option<usize> {
    let mut count = 0;
    let mut i = 0;
    while i < is_header.len() {
        let is_collapsed = is_header[i] && !expanded.contains(&i);
        if count == vis_pos {
            return Some(i);
        }
        count += 1;
        i += 1;
        if is_collapsed {
            while i < is_header.len() && !is_header[i] {
                i += 1;
            }
        }
    }
    None
}

/// 0-based visible position of flat index `flat`; None if hidden under a collapsed header.
#[allow(clippy::implicit_hasher)]
pub fn grep_flat_to_visible(
    is_header: &[bool],
    expanded: &HashSet<usize>,
    flat: usize,
) -> Option<usize> {
    let mut vis_pos = 0;
    let mut i = 0;
    while i < is_header.len() && i <= flat {
        let is_collapsed = is_header[i] && !expanded.contains(&i);
        if i == flat {
            return Some(vis_pos);
        }
        vis_pos += 1;
        i += 1;
        if is_collapsed {
            while i < is_header.len() && !is_header[i] {
                if i == flat {
                    return None;
                }
                i += 1;
            }
        }
    }
    None
}

#[allow(clippy::implicit_hasher)]
/// Previous visible flat index from `cur`; returns `cur` if none.
pub fn grep_prev_visible(is_header: &[bool], expanded: &HashSet<usize>, cur: usize) -> usize {
    if cur == 0 {
        return 0;
    }
    let mut i = cur - 1;
    loop {
        if is_header[i] {
            return i;
        }
        let mut parent = i;
        while parent > 0 && !is_header[parent] {
            parent -= 1;
        }
        if expanded.contains(&parent) {
            return i;
        }
        if i == 0 {
            return 0;
        }
        i -= 1;
    }
}
#[allow(clippy::implicit_hasher)]
/// Next visible flat index from `cur`; returns `cur` if none.
pub fn grep_next_visible(is_header: &[bool], expanded: &HashSet<usize>, cur: usize) -> usize {
    let mut i = cur + 1;
    while i < is_header.len() {
        if is_header[i] {
            return i;
        }
        let mut parent = i;
        while parent > 0 && !is_header[parent] {
            parent -= 1;
        }
        if expanded.contains(&parent) {
            return i;
        }
        i += 1;
    }
    cur
}

/// The single active (non-confirm) popup. Only one is ever active at a time;
/// a `ConfirmPopup` layers separately on top via [`PopupManager::confirm`].
pub enum ActivePopup {
    Template(TemplatePopup),
    Theme(ThemePopup),
    Info(InfoPopup),
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
            ActivePopup::IconMode(p) => {
                crate::ui::draw_icon_mode_popup(frame, p, area, theme, keybinds, mouse_pos)
            }
            ActivePopup::HintBarStyle(p) => {
                crate::ui::draw_hint_bar_style_popup(frame, p, area, theme, keybinds, mouse_pos)
            }
            ActivePopup::KeybindPreset(p) => {
                crate::ui::draw_keybind_preset_popup(frame, p, area, theme, keybinds, mouse_pos)
            }
            ActivePopup::Sort(p) => {
                crate::ui::draw_sort_popup(frame, p, area, theme, keybinds, mouse_pos)
            }
            ActivePopup::CreateFormat(p) => {
                crate::ui::draw_create_format_popup(frame, p, area, theme, keybinds, mouse_pos)
            }
            ActivePopup::Subnotes(p) => crate::ui::draw_subnotes_popup(frame, p, area, theme),
            ActivePopup::Info(p) => crate::ui::draw_info_popup(frame, area, p, theme),
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct PopupManager {
    /// Layered confirm dialog drawn on top of `active` when present.
    pub confirm: Option<ConfirmPopup>,
    /// The single active popup, if any.
    pub active: Option<ActivePopup>,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<crate::ui::scrollbar::ScrollDrag>,
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
    }
}
