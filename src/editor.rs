use crate::editor_document::EditorDocument;
use crate::markdown::MarkdownRenderer;
use ratatui_textarea::TextArea;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFocus {
    Title,
    Body,
    Sidebar,
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

pub(crate) struct EditorPreviewScheduler {
    pending_revision: Option<u64>,
    deadline: Option<Instant>,
    layout_ewma: Duration,
}

impl Default for EditorPreviewScheduler {
    fn default() -> Self {
        Self {
            pending_revision: None,
            deadline: None,
            layout_ewma: Duration::from_millis(75),
        }
    }
}

impl EditorPreviewScheduler {
    pub(crate) fn schedule(&mut self, revision: u64, now: Instant) {
        let delay = self
            .layout_ewma
            .saturating_mul(2)
            .clamp(Duration::from_millis(150), Duration::from_millis(750));
        self.pending_revision = Some(revision);
        self.deadline = Some(now + delay);
    }

    pub(crate) fn due(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|deadline| now >= deadline)
    }

    pub(crate) fn clear(&mut self) {
        self.pending_revision = None;
        self.deadline = None;
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EditorVisualRow {
    pub(crate) source_line: usize,
    pub(crate) start_char: usize,
    pub(crate) end_char: usize,
}

#[derive(Default)]
pub(crate) struct EditorVisualRowCache {
    pub(crate) key: Option<(u64, u16, bool, ratatui_textarea::WrapMode, u8)>,
    pub(crate) rows: Vec<EditorVisualRow>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutosaveStatus {
    #[default]
    Saved,
    Unsaved,
    RecentlySaved,
}

pub struct NoteEditor {
    pub autosave_status: AutosaveStatus,
    pub autosave_timer: Option<Instant>,
    pub last_saved_time: Option<Instant>,
    pub editing_id: Option<String>,
    pub initial_word_count: usize,
    pub template_edit_path: Option<PathBuf>,
    pub title_editor: TextArea<'static>,
    pub(crate) body: EditorDocument,
    pub external_editor_enabled: bool,
    pub external_editor: Option<String>,
    pub editor_preview_enabled: bool,
    pub md_preview_renderer: Option<MarkdownRenderer>,
    pub show_line_numbers: bool,
    pub find_popup: Option<crate::ui::quick_search::QuickSearch<(usize, String)>>,
    pub go_to_line_input: Option<String>,
    pub pending_editor_preview_update: bool,
    pub last_editor_change: Option<Instant>,
    pub(crate) preview_scheduler: EditorPreviewScheduler,
    pub last_preview_pane_width: u16,
    pub last_preview_pane_height: u16,
    pub preview_content_width: Option<u16>,
    pub pending_markdown_resize: Option<(u16, std::time::Instant)>,
    pub markdown_inner_rect: Option<ratatui::layout::Rect>,
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
    pub outline_nodes: Vec<crate::outline::parse::TreeNode>,
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
    pub last_body_width: u16,
    pub last_body_height: u16,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<crate::ui::scrollbar::ScrollDrag>,
    pub(crate) source_highlighter: Option<crate::markdown::SourceHighlighter>,
    /// Cache of per-line highlight styles, one entry per source line, rebuilt when the doc changes.
    pub md_highlight_cache: Vec<std::rc::Rc<[ratatui::style::Style]>>,
    /// `Instant` of the last editor change when cache was built.
    pub md_highlight_change: Option<std::time::Instant>,
    /// Number of lines in the document when cache was built.
    pub md_highlight_lines: usize,
    /// Bounded visible-line style memo, keyed by content and fence role.
    pub md_highlight_memo: lru::LruCache<(u64, bool), std::rc::Rc<[ratatui::style::Style]>>,
    pub(crate) visual_row_cache: EditorVisualRowCache,
    /// TTL cache for {modified} statusline token (500ms bounded).
    pub modified_status_cache: std::cell::RefCell<Option<(std::time::Instant, bool)>>,
}

impl Default for NoteEditor {
    fn default() -> Self {
        Self {
            autosave_status: AutosaveStatus::default(),
            autosave_timer: None,
            last_saved_time: None,
            editing_id: None,
            initial_word_count: 0,
            template_edit_path: None,
            title_editor: TextArea::default(),
            body: EditorDocument::default(),
            external_editor_enabled: false,
            external_editor: None,
            editor_preview_enabled: false,
            md_preview_renderer: None,
            show_line_numbers: false,
            find_popup: None,
            go_to_line_input: None,
            pending_editor_preview_update: false,
            preview_scheduler: EditorPreviewScheduler::default(),
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
            pending_markdown_resize: None,
            markdown_inner_rect: None,
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
            last_body_width: 0u16,
            last_body_height: 0u16,
            last_scroll: None,
            scroll_drag: None,
            body_viewport_row: 0,
            body_viewport_col: 0,
            title_viewport_row: 0,
            title_viewport_col: 0,
            md_highlight_cache: Vec::new(),
            md_highlight_change: None,
            md_highlight_lines: 0,
            md_highlight_memo: lru::LruCache::new(std::num::NonZeroUsize::MIN),
            visual_row_cache: EditorVisualRowCache::default(),
            modified_status_cache: std::cell::RefCell::new(None),
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
