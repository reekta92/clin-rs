use crate::markdown::MarkdownRenderer;
use ratatui::style::Style;
use ratatui::widgets::{ListItem, ListState};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMode {
    Normal,
    Select,
}

/// A child node in the FolderGraph preview — either a subfolder or a note.
#[derive(Debug, Clone)]
pub(crate) struct FolderGraphNode {
    pub key: String,
    pub label: String,
    pub is_note: bool,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortField {
    Title,
    Modified,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SmartFolderKind {
    Today,
    ThisWeek,
    Untagged,
    Tag(String),
    Custom(String),
}

impl SmartFolderKind {
    /// Inverse of `virtual_path()`: parse `@today`, `@week`, `@untagged`, `@tag:NAME`, `@custom:NAME`.
    pub fn from_virtual_path(path: &str) -> Option<SmartFolderKind> {
        match path {
            "@today" => Some(Self::Today),
            "@week" => Some(Self::ThisWeek),
            "@untagged" => Some(Self::Untagged),
            s if s.starts_with("@tag:") => Some(Self::Tag(s[5..].to_string())),
            s if s.starts_with("@custom:") => Some(Self::Custom(s[8..].to_string())),
            _ => None,
        }
    }
    pub fn virtual_path(&self) -> String {
        match self {
            Self::Today => "@today".into(),
            Self::ThisWeek => "@week".into(),
            Self::Untagged => "@untagged".into(),
            Self::Tag(t) => format!("@tag:{t}"),
            Self::Custom(name) => format!("@custom:{name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum VisualItem {
    Folder {
        path: String,
        name: String,
        depth: usize,
        is_expanded: bool,
        note_count: usize,
        recursive_count: usize,
        stale: bool,
        is_pinned: bool,
    },
    SmartFolder {
        kind: SmartFolderKind,
        label: String,
        depth: usize,
        is_expanded: bool,
        note_count: usize,
    },
    Note {
        summary_idx: usize,
        depth: usize,
        is_clin: bool,
        is_draw: bool,
        is_canvas: bool,
        in_virtual_pinned_folder: bool,
    },
    CreateNew {
        path: String,
        depth: usize,
    },
    /// A subnote listed under its parent in the Subnotes view.
    /// `parent_id` is the parent note id; `subnote_idx` indexes into the
    /// parent's Vec<SubNote> in App::subnotes_view_cache.
    Subnote {
        parent_id: String,
        subnote_idx: usize,
        depth: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct GridTile {
    pub visual_index: usize,
    pub rect: ratatui::layout::Rect,
}

pub struct ListView {
    pub display_items: Vec<ListItem<'static>>,
    pub visual_list: Vec<VisualItem>,
    pub visual_index: usize,
    pub list_state: ListState,
    pub grid_scroll: usize,
    pub grid_tiles: Vec<GridTile>,
    pub folder_expanded: HashSet<String>,
    pub folder_cache: Option<Vec<String>>,
    pub preview_enabled: bool,
    pub preview_content: Option<PreviewContent>,
    pub preview_content_index: Option<usize>,
    pub snapshot_scroll_offset: u16,
    pub sort_field: SortField,
    pub sort_order: SortOrder,
    pub pending_preview_update: bool,
    pub last_selection_change: Option<std::time::Instant>,
    pub image_cache: crate::image_render::cache::ImageCache,
    pub page_size: usize,
    pub list_mode: ListMode,
    pub selected_indices: HashSet<usize>,
    pub help_text_cache: Option<Vec<crate::ui::HelpRow>>,
    pub tag_to_assign: Option<String>,
    pub grid_folder: String,
    pub grid_columns: usize,
    pub notes_layout: crate::config::NotesLayout,
    pub list_density: crate::config::ListDensity,
    pub inline_info: bool,
    pub show_file_size: bool,
    pub folders_first: bool,
    pub show_hidden_files: bool,
    pub show_all_files: bool,
    pub last_preview_pane_width: u16,
    pub last_preview_pane_height: u16,
    pub preview_content_width: Option<u16>,
    pub preview_content_height: Option<u16>,
    pub preview_scale: f64,
    pub preview_content_scale: Option<f64>,
    pub preview_offset_x: f64,
    pub preview_offset_y: f64,
    pub preview_content_offset_x: Option<f64>,
    pub preview_content_offset_y: Option<f64>,
    pub calendar_enabled: bool,
    pub preview_width_ratio: f32,
    pub calendar_height: u16,
    pub calendar_position: crate::config::CalendarPosition,
    pub sections: Vec<crate::config::NotesSection>,
    pub pinned_folders: HashSet<String>,
    pub note_drag: Option<usize>,
    pub preview_drag_last_pos: Option<(u16, u16)>,
    /// Canvas viewport zoom for the SubnoteGraph preview (1.0 = fit whole graph).
    pub subnote_graph_zoom: f64,
    /// Canvas viewport pan offset in world coords.
    pub subnote_graph_pan_x: f64,
    pub subnote_graph_pan_y: f64,
    /// Canvas viewport zoom for the FolderGraph preview (1.0 = fit whole graph).
    pub(crate) folder_graph_zoom: f64,
    /// Canvas viewport pan offset in world coords.
    pub(crate) folder_graph_pan_x: f64,
    pub(crate) folder_graph_pan_y: f64,
    /// Cache of last-rendered FolderGraph child nodes (label + world position +
    /// kind) so the scroll handler can decide zoom-into-child transitions without
    /// recomputing layout. Written by render_folder_graph_static each frame.
    pub(crate) folder_graph_nodes: Vec<FolderGraphNode>,
    /// Currently zoom-focused note id inside a FolderGraph, when a note child is
    /// zoomed in past the content-card threshold. None = graph view.
    pub(crate) folder_graph_focused_note: Option<String>,
    /// Line scroll offset into the zoomed note's content inside the FolderGraph
    /// content card. Reset to 0 whenever `folder_graph_focused_note` transitions
    /// None→Some. Ignored when no note is focused.
    pub(crate) folder_graph_note_scroll: usize,
    pub drag_hover: Option<usize>,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<crate::ui::scrollbar::ScrollDrag>,
}

impl Default for ListView {
    fn default() -> Self {
        Self {
            visual_list: Vec::new(),
            visual_index: 0,
            display_items: Vec::new(),
            list_state: ListState::default(),
            grid_scroll: 0,
            grid_tiles: Vec::new(),
            folder_expanded: HashSet::new(),
            folder_cache: None,
            preview_enabled: false,
            preview_content: None,
            preview_content_index: None,
            snapshot_scroll_offset: 0,
            sort_field: SortField::Modified,
            sort_order: SortOrder::Descending,
            image_cache: crate::image_render::cache::ImageCache::new(32),
            pending_preview_update: false,
            last_selection_change: None,
            page_size: 30,
            list_mode: ListMode::Normal,
            selected_indices: HashSet::new(),
            help_text_cache: None,
            tag_to_assign: None,
            notes_layout: crate::config::NotesLayout::default(),
            inline_info: true,
            show_file_size: false,
            folders_first: true,
            list_density: crate::config::ListDensity::Compact,
            show_all_files: false,
            show_hidden_files: false,
            calendar_enabled: true,
            grid_folder: String::new(),
            grid_columns: 4,
            last_preview_pane_width: 0,
            last_preview_pane_height: 34,
            preview_content_width: None,
            preview_scale: 1.0,
            preview_content_scale: None,
            preview_offset_x: 0.0,
            preview_offset_y: 0.0,
            preview_content_offset_x: None,
            preview_content_offset_y: None,
            preview_content_height: None,
            preview_width_ratio: 0.43,
            calendar_height: 9,
            sections: crate::config::defaults::default_sections(),
            calendar_position: crate::config::CalendarPosition::default(),
            pinned_folders: HashSet::new(),
            note_drag: None,
            drag_hover: None,
            preview_drag_last_pos: None,
            subnote_graph_zoom: 1.0,
            subnote_graph_pan_x: 0.0,
            subnote_graph_pan_y: 0.0,
            folder_graph_zoom: 1.0,
            folder_graph_pan_x: 0.0,
            folder_graph_pan_y: 0.0,
            folder_graph_nodes: Vec::new(),
            folder_graph_focused_note: None,
            folder_graph_note_scroll: 0,
            last_scroll: None,
            scroll_drag: None,
        }
    }
}

impl ListView {
    pub fn new() -> Self {
        Self::default()
    }
}

pub enum PreviewContent {
    Markdown(Box<MarkdownRenderer>),
    CanvasGrid {
        data: Box<crate::pinstar::data::CanvasData>,
        grid: Vec<Vec<(char, Style)>>,
    },
    DrawGrid {
        data: Box<crate::draw::state::DrawData>,
        grid: Vec<Vec<(char, Style)>>,
    },
    Image(std::path::PathBuf),
    /// Local graph: one parent note + its subnotes, all connected to parent.
    /// Carries only the parent_id — rendered statically in draw_list_view.
    SubnoteGraph {
        parent_id: String,
    },
    /// Hierarchical folder graph: parent folder node at center, direct children
    /// (subfolders + notes) on an orbit. Zoom into a subfolder child re-focuses
    /// to that subfolder; zoom into a note child shows the note's content card.
    /// `root_path` is the originally selected folder/virtual path (stable);
    /// `focused_path` is the currently focused descendant (defaults to root).
    FolderGraph {
        root_path: String,
        focused_path: String,
    },
}

impl std::fmt::Debug for PreviewContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown(_) => f.debug_tuple("Markdown").finish(),
            Self::CanvasGrid { grid, .. } => {
                f.debug_tuple("CanvasGrid").field(&grid.len()).finish()
            }
            Self::SubnoteGraph { .. } => f.debug_tuple("SubnoteGraph").finish(),
            Self::FolderGraph { .. } => f.debug_tuple("FolderGraph").finish(),
            Self::DrawGrid { grid, .. } => f.debug_tuple("DrawGrid").field(&grid.len()).finish(),
            Self::Image(_) => f.debug_tuple("Image").finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_folder_kind_from_virtual_path_roundtrip() {
        let kinds = vec![
            SmartFolderKind::Today,
            SmartFolderKind::ThisWeek,
            SmartFolderKind::Untagged,
            SmartFolderKind::Tag("rust".to_string()),
            SmartFolderKind::Tag("testing".to_string()),
            SmartFolderKind::Custom("my-folder".to_string()),
        ];
        for kind in kinds {
            let path = kind.virtual_path();
            let parsed = SmartFolderKind::from_virtual_path(&path);
            assert_eq!(parsed, Some(kind), "roundtrip failed for {path:?}");
        }
    }

    #[test]
    fn from_virtual_path_invalid() {
        assert_eq!(SmartFolderKind::from_virtual_path(""), None);
        assert_eq!(SmartFolderKind::from_virtual_path("not_a_smart"), None);
        assert_eq!(SmartFolderKind::from_virtual_path("@invalid"), None);
    }
}
