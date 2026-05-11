use crate::markdown::MarkdownRenderer;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::ListState;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMode {
    Normal,
    Select,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFocus {
    Notes,
    ExternalEditorToggle,
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

#[derive(Debug, Clone)]
pub enum VisualItem {
    Folder {
        path: String,
        name: String,
        depth: usize,
        is_expanded: bool,
        note_count: usize,
    },
    Note {
        id: String,
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
}

pub struct ListView {
    pub visual_list: Vec<VisualItem>,
    pub visual_index: usize,
    pub list_focus: ListFocus,
    pub list_state: ListState,
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
    pub page_size: usize,
    pub list_mode: ListMode,
    pub selected_indices: HashSet<usize>,
    pub help_text_cache: Option<Text<'static>>,
    pub tag_to_assign: Option<String>,
}

impl Default for ListView {
    fn default() -> Self {
        Self {
            visual_list: Vec::new(),
            visual_index: 0,
            list_focus: ListFocus::Notes,
            list_state: ListState::default(),
            folder_expanded: HashSet::new(),
            folder_cache: None,
            preview_enabled: false,
            preview_content: None,
            preview_content_index: None,
            snapshot_scroll_offset: 0,
            sort_field: SortField::Modified,
            sort_order: SortOrder::Descending,
            pending_preview_update: false,
            last_selection_change: None,
            page_size: 30,
            list_mode: ListMode::Normal,
            selected_indices: HashSet::new(),
            help_text_cache: None,
            tag_to_assign: None,
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
    CanvasGrid(Vec<Vec<(char, Style)>>),
    DrawGrid(Vec<Vec<(char, Style)>>),
}

impl std::fmt::Debug for PreviewContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown(_) => f.debug_tuple("Markdown").finish(),
            Self::CanvasGrid(g) => f.debug_tuple("CanvasGrid").field(&g.len()).finish(),
            Self::DrawGrid(g) => f.debug_tuple("DrawGrid").field(&g.len()).finish(),
        }
    }
}
