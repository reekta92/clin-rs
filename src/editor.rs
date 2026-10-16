pub(crate) mod frontmatter;
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
    Frontmatter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditSidebar {
    #[default]
    None,
    Outline,
    Links,
    Bases,
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
impl EditorVisualRowCache {
    /// Source line and wrapped sub-row for the editor's first visible screen row.
    pub(crate) fn source_position(&self, visual_row: usize) -> (usize, usize) {
        let Some(row) = self.rows.get(visual_row) else {
            return (
                self.rows.last().map_or(visual_row, |row| row.source_line),
                0,
            );
        };
        let first = self.rows[..visual_row]
            .iter()
            .rposition(|previous| previous.source_line != row.source_line)
            .map_or(0, |index| index + 1);
        (row.source_line, visual_row - first)
    }
}

#[cfg(test)]
mod scroll_tests {
    use super::{EditorVisualRow, EditorVisualRowCache};

    #[test]
    fn visual_rows_map_to_source_and_wrapped_offset() {
        let cache = EditorVisualRowCache {
            key: None,
            rows: [0, 0, 0, 1, 2, 2]
                .into_iter()
                .map(|source_line| EditorVisualRow {
                    source_line,
                    start_char: 0,
                    end_char: 1,
                })
                .collect(),
        };
        assert_eq!(cache.source_position(0), (0, 0));
        assert_eq!(cache.source_position(2), (0, 2));
        assert_eq!(cache.source_position(3), (1, 0));
        assert_eq!(cache.source_position(5), (2, 1));
        assert_eq!(cache.source_position(99), (2, 0));
        assert_eq!(EditorVisualRowCache::default().source_position(0), (0, 0));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutosaveStatus {
    #[default]
    Saved,
    Unsaved,
    RecentlySaved,
}

pub struct NoteEditor {
    pub(crate) frontmatter: Option<frontmatter::FrontmatterEditor>,
    pub autosave_status: AutosaveStatus,
    pub autosave_timer: Option<Instant>,
    pub last_saved_time: Option<Instant>,
    pub editing_id: Option<String>,
    pub public_document: Option<crate::storage::documents::DocumentSnapshot>,
    pub(crate) public_document_body_only: bool,
    pub(crate) public_document_title: Option<String>,
    pub note_baseline: Option<crate::storage::documents::DocumentSnapshot>,
    pub initial_word_count: usize,
    pub template_edit_path: Option<PathBuf>,
    pub(crate) template_source: Option<String>,
    pub title_editor: TextArea<'static>,
    pub(crate) body: EditorDocument,
    pub external_editor_enabled: bool,
    pub external_editor: Option<String>,
    pub editor_preview_enabled: bool,
    pub(crate) mermaid_rendering_enabled: bool,
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
    pub(crate) bases_sidebar_renderer: Option<MarkdownRenderer>,
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
    pub preview_scroll_synced_anchor: Option<(usize, usize)>,
    pub preview_scroll_overridden: bool,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<i32>,
    pub(crate) source_highlighter: Option<crate::markdown::SourceHighlighter>,
    /// Bounded visible-line styles, keyed by exact content, fence role, and todo mode.
    pub md_highlight_memo:
        lru::LruCache<(String, bool, bool), std::rc::Rc<[ratatui::style::Style]>>,
    pub(crate) visual_row_cache: EditorVisualRowCache,
    /// TTL cache for {modified} statusline token (500ms bounded).
    pub modified_status_cache: std::cell::RefCell<Option<(std::time::Instant, bool)>>,
    pub text_align: crate::config::TextAlignment,
}

impl Default for NoteEditor {
    fn default() -> Self {
        Self {
            frontmatter: None,
            autosave_status: AutosaveStatus::default(),
            autosave_timer: None,
            last_saved_time: None,
            editing_id: None,
            public_document: None,
            public_document_body_only: false,
            public_document_title: None,
            note_baseline: None,
            initial_word_count: 0,
            template_edit_path: None,
            template_source: None,
            title_editor: TextArea::default(),
            body: EditorDocument::default(),
            external_editor_enabled: false,
            external_editor: None,
            editor_preview_enabled: false,
            mermaid_rendering_enabled: true,
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
            bases_sidebar_renderer: None,
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
            preview_scroll_synced_anchor: None,
            preview_scroll_overridden: false,
            last_scroll: None,
            scroll_drag: None,
            body_viewport_row: 0,
            body_viewport_col: 0,
            title_viewport_row: 0,
            title_viewport_col: 0,
            md_highlight_memo: lru::LruCache::new(std::num::NonZeroUsize::MIN),
            visual_row_cache: EditorVisualRowCache::default(),
            modified_status_cache: std::cell::RefCell::new(None),
            text_align: crate::config::TextAlignment::default(),
            source_highlighter: None,
            header_title_rect: ratatui::layout::Rect::default(),
        }
    }
}

impl NoteEditor {
    pub fn public_document_source(&self) -> Option<String> {
        let baseline = self.public_document.as_ref()?;
        let body = self.body.text();
        if self.public_document_body_only {
            let mut source = self.note_working_source().ok()?;
            let title = crate::events::get_title_text(&self.title_editor);
            let original = self
                .frontmatter
                .as_ref()
                .map(|state| state.loaded_title.as_str())
                .or(self.public_document_title.as_deref());
            if original.is_some_and(|original| title.as_ref() != original) {
                let before =
                    crate::frontmatter::properties::PropertyDocument::parse(baseline.source())
                        .ok()?;
                let after =
                    crate::frontmatter::properties::PropertyDocument::parse(&source).ok()?;
                if before.properties.get("title") != after.properties.get("title")
                    && after
                        .properties
                        .get("title")
                        .and_then(serde_yaml_ng::Value::as_str)
                        != Some(title.as_ref())
                {
                    return None;
                }
                source = crate::frontmatter::properties::edit_properties(
                    &source,
                    &[(
                        "title".into(),
                        Some(serde_yaml_ng::Value::String(title.into_owned())),
                    )],
                )
                .ok()?;
            }
            return Some(source);
        }
        let content = body;
        let mut source = if baseline.source().replace("\r\n", "\n") == content {
            baseline.source().to_owned()
        } else if baseline.source().contains("\r\n") {
            content.replace('\n', "\r\n")
        } else {
            content
        };
        if let Some(original) = &self.public_document_title {
            let title = crate::events::get_title_text(&self.title_editor);
            if title.as_ref() != original {
                source = crate::frontmatter::properties::edit_properties(
                    &source,
                    &[(
                        "title".into(),
                        Some(serde_yaml_ng::Value::String(title.into_owned())),
                    )],
                )
                .ok()?;
            }
        }
        Some(source)
    }
    pub(crate) fn synchronize_public_snapshot(
        &mut self,
        before: &crate::storage::documents::DocumentSnapshot,
        after: &crate::storage::documents::DocumentSnapshot,
    ) -> bool {
        if self.editing_id.as_deref() != Some(after.id()) || self.public_document.is_none() {
            return false;
        }
        let Some(current) = self.public_document_source() else {
            return false;
        };
        let current_matches = current == before.source();
        let bodies = if self.public_document_body_only {
            crate::frontmatter::properties::PropertyDocument::parse(before.source())
                .ok()
                .zip(crate::frontmatter::properties::PropertyDocument::parse(after.source()).ok())
        } else {
            None
        };
        let can_rebase = !self.header_pending()
            && bodies
                .as_ref()
                .is_some_and(|(before, after)| before.body == after.body)
            && self
                .public_document
                .as_ref()
                .is_some_and(|baseline| baseline.source() == before.source());
        if !current_matches
            && bodies.as_ref().is_some_and(|(before, after)| {
                before.properties.get("title") != after.properties.get("title")
            })
            && self.public_document_title.as_deref()
                != Some(crate::events::get_title_text(&self.title_editor).as_ref())
        {
            return false;
        }
        if !current_matches && !can_rebase {
            return false;
        }
        if current_matches {
            let body = bodies
                .as_ref()
                .map_or(after.source(), |(_, after)| after.body);
            let cursor = self.body.cursor();
            if self.body.text() != body.replace("\r\n", "\n") {
                self.body.select_all();
                self.body.insert_str(body.replace("\r\n", "\n"));
                self.pending_editor_preview_update = true;
                self.preview_scheduler
                    .schedule(self.body.revision(), Instant::now());
            }
            self.body.move_cursor(ratatui_textarea::CursorMove::Jump(
                cursor.row as u16,
                cursor.col as u16,
            ));
        }
        if self.public_document_body_only
            && let Some((before, after)) = bodies
        {
            let old = before.properties.get("title");
            let new = after.properties.get("title");
            if old != new
                && self.public_document_title.as_deref()
                    == Some(crate::events::get_title_text(&self.title_editor).as_ref())
            {
                let title = new
                    .and_then(serde_yaml_ng::Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        std::path::Path::new(self.editing_id.as_deref().unwrap_or(""))
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .unwrap_or("")
                            .to_owned()
                    });
                self.title_editor.select_all();
                self.title_editor.insert_str(&title);
                self.public_document_title = Some(title);
            }
        }
        if self.adopt_frontmatter(after.source()).is_err() {
            return false;
        }
        self.public_document = Some(after.clone());
        true
    }
    pub fn new() -> Self {
        Self::default()
    }
}
