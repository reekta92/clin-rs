use crate::markdown::MarkdownRenderer;
use ratatui_textarea::TextArea;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFocus {
    Title,
    Body,
    Sidebar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    #[default]
    Read,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditSidebar {
    #[default]
    None,
    Outline,
    Links,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkItem {
    pub id: String,
    pub title: String,
    pub is_backlink: bool,
}

pub struct NoteEditor {
    pub editing_id: Option<String>,
    pub initial_word_count: usize,
    pub template_edit_path: Option<PathBuf>,
    pub title_editor: TextArea<'static>,
    pub editor: TextArea<'static>,
    pub external_editor_enabled: bool,
    pub external_editor: Option<String>,
    pub editor_preview_enabled: bool,
    pub md_preview_renderer: Option<MarkdownRenderer>,
    pub show_line_numbers: bool,
    pub find_popup: Option<crate::ui::quick_search::QuickSearch<(usize, String)>>,
    pub go_to_line_input: Option<String>,
    pub pending_editor_preview_update: bool,
    pub last_editor_change: Option<Instant>,
    pub last_preview_pane_width: u16,
    pub last_preview_pane_height: u16,
    pub preview_content_width: Option<u16>,
    pub preview_content_height: Option<u16>,
    pub image_cache: crate::image_render::cache::ImageCache,
    pub preview_scale: f64,
    pub preview_content_scale: Option<f64>,
    pub preview_offset_x: f64,
    pub preview_offset_y: f64,
    pub preview_content_offset_x: Option<f64>,
    pub preview_content_offset_y: Option<f64>,
    pub image_picker: Option<ratatui_image::picker::Picker>,
    pub image_decode_tx: Option<std::sync::mpsc::Sender<crate::image_render::worker::ImageJob>>,
    pub sidebar: EditSidebar,
    pub sidebar_scroll_offset: usize,
    pub sidebar_list_rect: ratatui::layout::Rect,
    pub sidebar_selected: usize,
    pub outline_nodes: Vec<crate::content_tree::parse::TreeNode>,
    pub preview_drag_last_pos: Option<(u16, u16)>,
    pub links: Vec<LinkItem>,
    pub link_preview: bool,
    pub link_preview_renderer: Option<MarkdownRenderer>,
    pub link_preview_target: Option<String>,
    pub link_preview_error: Option<String>,
    pub last_sidebar_click: Option<(u16, u16, Instant)>,
    pub header_title_rect: ratatui::layout::Rect,
    pub body_viewport_row: u16,
    pub body_viewport_col: u16,
    pub title_viewport_row: u16,
    pub title_viewport_col: u16,
    pub edit_mode: EditMode,
    /// Cells extracted from `render_builtin` for READ mode
    pub read_grid: Vec<Vec<(char, ratatui::style::Style)>>,
    pub read_offset: usize,
    /// Per-line source line from markdown rendering, used for READ↔EDIT scroll sync.
    pub read_row_source: Vec<usize>,
    /// When set, the next READ-mode render will scroll to the grid row
    /// corresponding to this 0-based logical line.
    pub pending_read_sync_from_line: Option<usize>,
    /// Content width used to produce `read_grid`
    pub read_cols: u16,
    /// Set true whenever editor text changes or width changes
    pub read_dirty: bool,
    pub last_body_width: u16,
    pub last_body_height: u16,
    pub read_gg_pending: bool,
    pub(crate) source_highlighter: Option<crate::markdown::SourceHighlighter>,
    /// Cache of per-line highlight styles, rebuilt only when the document changes.
    pub md_highlight_cache: Vec<Vec<ratatui::style::Style>>,
    /// `Instant` of the last editor change when cache was built.
    pub md_highlight_change: Option<std::time::Instant>,
    /// Number of lines in the document when cache was built.
    pub md_highlight_lines: usize,
    /// READ-mode mouse selection state
    pub read_selecting: bool,
    pub read_sel_anchor: Option<(usize, usize)>,
    pub read_sel_end: Option<(usize, usize)>,
}

impl Default for NoteEditor {
    fn default() -> Self {
        Self {
            editing_id: None,
            initial_word_count: 0,
            template_edit_path: None,
            title_editor: TextArea::default(),
            editor: TextArea::default(),
            external_editor_enabled: false,
            external_editor: None,
            editor_preview_enabled: false,
            md_preview_renderer: None,
            show_line_numbers: false,
            find_popup: None,
            go_to_line_input: None,
            pending_editor_preview_update: false,
            preview_scale: 1.0,
            preview_content_scale: None,
            preview_offset_x: 0.0,
            preview_offset_y: 0.0,
            preview_content_offset_x: None,
            preview_content_offset_y: None,
            last_editor_change: None,
            last_preview_pane_width: 0,
            last_preview_pane_height: 36,
            preview_content_width: None,
            preview_content_height: None,
            image_cache: crate::image_render::cache::ImageCache::new(32),
            image_picker: None,
            image_decode_tx: None,
            sidebar: EditSidebar::None,
            sidebar_scroll_offset: 0,
            sidebar_list_rect: ratatui::layout::Rect::default(),
            sidebar_selected: 0,
            outline_nodes: Vec::new(),
            links: Vec::new(),
            link_preview: false,
            link_preview_renderer: None,
            link_preview_target: None,
            link_preview_error: None,
            last_sidebar_click: None,
            preview_drag_last_pos: None,
            edit_mode: EditMode::Read,
            read_grid: Vec::new(),
            read_offset: 0usize,
            read_row_source: Vec::new(),
            pending_read_sync_from_line: None,
            read_cols: 0u16,
            read_dirty: true,
            last_body_width: 0u16,
            last_body_height: 0u16,
            read_gg_pending: false,
            body_viewport_row: 0,
            body_viewport_col: 0,
            title_viewport_row: 0,
            title_viewport_col: 0,
            md_highlight_cache: Vec::new(),
            md_highlight_change: None,
            md_highlight_lines: 0,
            read_selecting: false,
            read_sel_anchor: None,
            read_sel_end: None,
            source_highlighter: None,
            header_title_rect: ratatui::layout::Rect::default(),
        }
    }
}

impl NoteEditor {
    pub fn new() -> Self {
        Self::default()
    }
}
