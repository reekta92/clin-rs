mod builtin;
mod cache;
mod source_highlight;
mod style;
pub(crate) mod todotxt;
mod widget;
mod worker;

pub(crate) use builtin::default_code_theme;
pub(crate) use source_highlight::SourceHighlighter;
pub(crate) use style::{MarkdownTheme, RenderedDocument};
pub(crate) use widget::MarkdownWidget;
pub(crate) use worker::{RenderViewport, prewarm_syntax_assets};

use cache::RenderKey;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use worker::{RenderEvent, RenderJob, pack_viewport, unpack_viewport};

/// Bundled render flags threaded into `render_layout`. Replaces the prior
/// loose `(syntax_hl, wrap, icon_mode)` args.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MdRenderOpts {
    pub syntax_hl: bool,
    pub wrap: bool,
    pub icon_mode: crate::config::IconMode,
    pub code_theme: String,
    pub code_line_numbers: bool,
    pub wrap_indicator: bool,
    pub link_url_max: usize,
    pub is_todo_txt: bool,
}

impl Default for MdRenderOpts {
    fn default() -> Self {
        Self {
            syntax_hl: true,
            wrap: true,
            icon_mode: crate::config::IconMode::default(),
            code_theme: crate::markdown::default_code_theme().to_string(),
            code_line_numbers: true,
            wrap_indicator: false,
            link_url_max: 80,
            is_todo_txt: false,
        }
    }
}

impl MdRenderOpts {
    /// Build render options from the app config (used by all three preview call sites).
    pub(crate) fn from_config(config: &crate::config::ClinConfig, id: Option<&str>) -> Self {
        Self {
            syntax_hl: config.core.syntax_highlighting,
            wrap: config.core.preview_wrap,
            icon_mode: config.ui.icon_mode,
            code_theme: config.core.code_theme.clone(),
            code_line_numbers: config.core.code_line_numbers,
            wrap_indicator: config.core.preview_wrap_indicator,
            link_url_max: config.core.link_url_max_length,
            is_todo_txt: id.is_some_and(|s| s.ends_with("todo.txt")),
        }
    }
}

enum DocumentState {
    Working(RenderedDocument),
    Final(Arc<RenderedDocument>),
}

pub struct MarkdownRenderer {
    document: Option<DocumentState>,
    events: Option<mpsc::Receiver<RenderEvent>>,
    current_key: Option<RenderKey>,
    generation: u64,
    cancel: Option<Arc<AtomicBool>>,
    viewport: Arc<AtomicU64>,
    current_page: usize,
    page_height: usize,
    scroll_offset: usize,
    pending_source_anchor: Option<usize>,
    pending: bool,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for MarkdownRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarkdownRenderer")
            .field("generation", &self.generation)
            .field("pending", &self.pending)
            .field("current_page", &self.current_page)
            .field("page_height", &self.page_height)
            .field("scroll_offset", &self.scroll_offset)
            .finish()
    }
}

impl Drop for MarkdownRenderer {
    fn drop(&mut self) {
        if let Some(token) = &self.cancel {
            token.store(true, Ordering::Relaxed);
        }
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            document: None,
            events: None,
            current_key: None,
            generation: 0,
            cancel: None,
            viewport: Arc::new(AtomicU64::new(0)),
            current_page: 0,
            page_height: 0,
            scroll_offset: 0,
            pending_source_anchor: None,
            pending: false,
        }
    }

    pub(crate) fn render_with(
        &mut self,
        content: &str,
        cols: u16,
        theme: &crate::app_theme::AppThemeColors,
        opts: &MdRenderOpts,
        viewport: RenderViewport,
    ) {
        if content.is_empty() {
            if let Some(token) = self.cancel.take() {
                token.store(true, Ordering::Relaxed);
            }
            self.events = None;
            let empty_doc = RenderedDocument::new(Vec::new());
            self.document = Some(DocumentState::Final(Arc::new(empty_doc)));
            self.current_key = None;
            self.pending = false;
            return;
        }

        let next_key = RenderKey::new(
            Arc::from(content),
            cols,
            MarkdownTheme::from_app_theme(theme),
            opts.clone(),
        );

        if let Some(ref cur) = self.current_key
            && cur == &next_key
        {
            self.set_viewport(viewport.start, viewport.height);
            return;
        }

        if let Some(token) = self.cancel.take() {
            token.store(true, Ordering::Relaxed);
        }
        self.events = None;
        self.generation = self.generation.wrapping_add(1);

        let content_unchanged = self
            .current_key
            .as_ref()
            .map(|k| k.content == next_key.content)
            .unwrap_or(false);

        if content_unchanged {
            let top_visible = if self.page_height > 0 {
                self.current_page * self.page_height
            } else {
                self.scroll_offset
            };
            let source_line = self.rendered_to_source_line(top_visible);
            self.pending_source_anchor = Some(source_line);
        } else {
            self.document = None;
            self.current_page = 0;
            self.scroll_offset = 0;
            self.pending_source_anchor = None;
        }

        self.current_key = Some(next_key.clone());
        self.set_viewport(viewport.start, viewport.height);

        if let Some(cached) = cache::get_document(&next_key) {
            self.document = Some(DocumentState::Final(cached));

            #[allow(clippy::manual_checked_ops)]
            if let Some(src_line) = self.pending_source_anchor.take() {
                let new_rendered = self.source_to_rendered_line(src_line);
                if self.page_height > 0 {
                    self.current_page = new_rendered / self.page_height;
                } else {
                    self.scroll_offset = new_rendered;
                }
                self.clamp_current_page();
            }

            self.pending = false;
            return;
        }

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(Arc::clone(&cancel));
        self.pending = true;

        let (tx, rx) = mpsc::sync_channel(16);
        self.events = Some(rx);

        let job = RenderJob {
            generation: self.generation,
            key: next_key,
            viewport: Arc::clone(&self.viewport),
            cancel,
            tx,
        };

        worker::submit(job);
    }

    pub fn set_viewport(&mut self, start: usize, height: usize) {
        self.viewport
            .store(pack_viewport(start, height), Ordering::Relaxed);
    }

    pub(crate) fn document(&self) -> Option<&RenderedDocument> {
        match &self.document {
            Some(DocumentState::Working(doc)) => Some(doc),
            Some(DocumentState::Final(doc)) => Some(&**doc),
            None => None,
        }
    }

    pub fn is_pending(&self) -> bool {
        self.pending
    }

    pub fn is_content_empty(&self) -> bool {
        self.document()
            .map(|d| d.is_content_empty())
            .unwrap_or(true)
    }

    pub(crate) fn is_changed(
        &self,
        content: &str,
        theme: &crate::app_theme::AppThemeColors,
        opts: &MdRenderOpts,
    ) -> bool {
        self.current_key
            .as_ref()
            .map(|k| {
                k.content.as_ref() != content
                    || k.theme != MarkdownTheme::from_app_theme(theme)
                    || &k.opts != opts
            })
            .unwrap_or(true)
    }

    pub fn set_page_height(&mut self, rows: usize) {
        self.page_height = rows;
        self.clamp_current_page();
    }

    pub fn current_page_range(&self) -> Range<usize> {
        let len = self.document().map(|d| d.line_count()).unwrap_or(0);
        if self.page_height == 0 {
            return 0..len;
        }
        let pageable_len = self.pageable_length();
        let page_h = self.page_height;
        let start = self.current_page * page_h;
        let end = (start + page_h).min(pageable_len);
        start..end
    }

    pub fn visible_range(&self, height: usize) -> Range<usize> {
        let len = self.document().map(|d| d.line_count()).unwrap_or(0);
        let start = self.scroll_offset.min(len);
        let end = (start + height).min(len);
        start..end
    }

    pub fn visible_start(&self) -> usize {
        if self.page_height > 0 {
            self.current_page * self.page_height
        } else {
            self.scroll_offset
        }
    }

    pub fn total_pages(&self) -> usize {
        if self.page_height == 0 {
            return 1;
        }
        let pageable_len = self.pageable_length();
        if pageable_len == 0 {
            return 1;
        }
        let page_h = self.page_height;
        pageable_len.div_ceil(page_h)
    }

    pub fn current_page(&self) -> usize {
        self.current_page
    }

    pub fn next_page(&mut self) {
        let tp = self.total_pages();
        if tp > 0 && self.current_page < tp - 1 {
            self.current_page += 1;
            self.update_viewport_for_current_page();
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.update_viewport_for_current_page();
        }
    }

    pub fn next_page_wrap(&mut self) {
        let tp = self.total_pages();
        if tp <= 1 {
            return;
        }
        if self.current_page < tp - 1 {
            self.current_page += 1;
        } else {
            self.current_page = 0;
        }
        self.update_viewport_for_current_page();
    }

    pub fn prev_page_wrap(&mut self) {
        let tp = self.total_pages();
        if tp <= 1 {
            return;
        }
        if self.current_page > 0 {
            self.current_page -= 1;
        } else {
            self.current_page = tp - 1;
        }
        self.update_viewport_for_current_page();
    }
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set the scroll offset directly, clamping to valid range.
    pub fn set_scroll_offset(&mut self, offset: usize, visible_height: usize) {
        let len = self.document().map(|d| d.line_count()).unwrap_or(0);
        let max = len.saturating_sub(visible_height);
        self.scroll_offset = offset.min(max);
        self.update_viewport_for_scroll();
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.update_viewport_for_scroll();
    }

    pub fn scroll_down(&mut self, lines: usize, visible_height: usize) {
        let len = self.document().map(|d| d.line_count()).unwrap_or(0);
        let max = len.saturating_sub(visible_height);
        self.scroll_offset = (self.scroll_offset + lines).min(max);
        self.update_viewport_for_scroll();
    }

    pub fn page_up(&mut self, visible_height: usize) {
        self.scroll_up(visible_height);
    }

    pub fn page_down(&mut self, visible_height: usize) {
        self.scroll_down(visible_height, visible_height);
    }

    pub fn scroll_top(&mut self) {
        self.scroll_offset = 0;
        self.update_viewport_for_scroll();
    }

    pub fn scroll_bottom(&mut self, visible_height: usize) {
        let len = self.document().map(|d| d.line_count()).unwrap_or(0);
        self.scroll_offset = len.saturating_sub(visible_height);
        self.update_viewport_for_scroll();
    }

    pub fn source_to_rendered_line(&self, source_line_0_based: usize) -> usize {
        let comrak_line = source_line_0_based + 1;
        let document = match self.document() {
            Some(doc) => doc,
            None => return 0,
        };
        document
            .lines()
            .iter()
            .position(|l| l.source_line >= comrak_line)
            .unwrap_or(0)
    }

    pub fn rendered_to_source_line(&self, rendered_line: usize) -> usize {
        let document = match self.document() {
            Some(doc) => doc,
            None => return 0,
        };
        document
            .lines()
            .get(rendered_line)
            .map(|l| l.source_line)
            .unwrap_or(1)
            .saturating_sub(1)
    }

    fn pageable_length(&self) -> usize {
        self.document()
            .and_then(|doc| doc.last_non_blank_line().map(|idx| idx + 1))
            .unwrap_or(0)
    }

    fn clamp_current_page(&mut self) {
        let tp = self.total_pages();
        if self.current_page >= tp {
            self.current_page = tp.saturating_sub(1);
        }
    }

    fn update_viewport_for_current_page(&mut self) {
        let range = self.current_page_range();
        let start = range.start;
        let height = self.page_height;
        self.set_viewport(start, height);
    }

    fn update_viewport_for_scroll(&mut self) {
        self.set_viewport(self.scroll_offset, 0);
    }

    pub fn poll(&mut self) -> bool {
        let events: Vec<RenderEvent> = {
            let Some(rx) = &self.events else {
                return false;
            };
            let mut evts = Vec::new();
            while let Ok(event) = rx.try_recv() {
                evts.push(event);
            }
            evts
        };

        if events.is_empty() {
            return false;
        }

        let mut redraw = false;
        let mut completed = false;

        for event in events {
            match event {
                RenderEvent::LayoutReady {
                    generation,
                    document,
                } => {
                    if generation != self.generation {
                        continue;
                    }

                    #[allow(clippy::manual_checked_ops)]
                    if let Some(src_line) = self.pending_source_anchor.take() {
                        self.document = Some(DocumentState::Working(document));
                        let new_rendered = self.source_to_rendered_line(src_line);
                        if self.page_height > 0 {
                            self.current_page = new_rendered / self.page_height;
                        } else {
                            self.scroll_offset = new_rendered;
                        }
                        self.clamp_current_page();
                    } else {
                        self.document = Some(DocumentState::Working(document));
                        self.clamp_current_page();
                    }

                    redraw = true;
                }
                RenderEvent::CodeBlockReady {
                    generation,
                    line_range,
                    lines,
                } => {
                    if generation != self.generation {
                        continue;
                    }

                    if let Some(DocumentState::Working(ref mut doc)) = self.document {
                        if line_range.start < doc.line_count()
                            && line_range.end <= doc.line_count()
                            && line_range.len() == lines.len()
                        {
                            let mut first_failure = None;
                            for (idx, new_line) in lines.iter().enumerate() {
                                let orig_line = &doc.lines()[line_range.start + idx];
                                let orig_text: String =
                                    orig_line.spans.iter().map(|s| s.text.as_str()).collect();
                                let new_text: String =
                                    new_line.spans.iter().map(|s| s.text.as_str()).collect();
                                if orig_text != new_text
                                    || orig_line.visual_width != new_line.visual_width
                                    || orig_line.source_line != new_line.source_line
                                    || new_line.image_url.is_some()
                                {
                                    first_failure = Some(idx);
                                    break;
                                }
                            }
                            if first_failure.is_some() {
                                continue;
                            }

                            let doc_lines = doc.lines_mut();
                            for (idx, new_line) in lines.into_iter().enumerate() {
                                doc_lines[line_range.start + idx] = new_line;
                            }

                            let (vp_start, vp_height) =
                                unpack_viewport(self.viewport.load(Ordering::Relaxed));
                            let vp_end = vp_start.saturating_add(vp_height);
                            let intersects = line_range.start < vp_end && line_range.end > vp_start;
                            if intersects {
                                redraw = true;
                            }
                        } else {
                            continue;
                        }
                    }
                }
                RenderEvent::Complete { generation } => {
                    if generation != self.generation {
                        continue;
                    }

                    if let Some(DocumentState::Working(doc)) = self.document.take() {
                        let shared = Arc::new(doc);
                        if let Some(key) = &self.current_key {
                            cache::insert_document(key.clone(), Arc::clone(&shared));
                        }
                        self.document = Some(DocumentState::Final(shared));
                    }
                    completed = true;
                    self.pending = false;
                    redraw = true;
                }
            }
        }

        if completed {
            self.events = None;
        }

        redraw
    }
}

#[cfg(test)]
mod perf_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::style::{RenderLine, RenderedDocument, StyledSpan};
    use crate::markdown::worker::RenderEvent;
    use ratatui::style::{Modifier, Style};
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use std::sync::mpsc;

    #[test]
    fn test_markdown_renderer_poll_code_block_ready_incompatible() {
        let (tx, rx) = mpsc::sync_channel(1);

        let base_line = RenderLine {
            spans: vec![StyledSpan {
                text: "original code".to_string(),
                style: Style::default(),
            }],
            visual_width: 13,
            is_blank: false,
            image_url: None,
            source_line: 42,
        };

        let mut renderer = MarkdownRenderer {
            document: Some(DocumentState::Working(RenderedDocument::new(vec![
                base_line,
            ]))),
            events: Some(rx),
            current_key: None,
            generation: 1,
            cancel: None,
            viewport: Arc::new(AtomicU64::new(0)),
            current_page: 0,
            page_height: 0,
            scroll_offset: 0,
            pending_source_anchor: None,
            pending: true,
        };

        let incompatible_line = RenderLine {
            spans: vec![StyledSpan {
                text: "mismatched code".to_string(),
                style: Style::default(),
            }],
            visual_width: 15,
            is_blank: false,
            image_url: None,
            source_line: 42,
        };

        tx.send(RenderEvent::CodeBlockReady {
            generation: 1,
            line_range: 0..1,
            lines: vec![incompatible_line],
        })
        .unwrap();

        let redraw = renderer.poll();
        assert!(!redraw);

        if let Some(DocumentState::Working(doc)) = &renderer.document {
            assert_eq!(doc.line_count(), 1);
            let spans = &doc.lines()[0].spans;
            assert_eq!(spans[0].text, "original code");
        } else {
            panic!("Expected working document state");
        }
    }

    #[test]
    fn test_markdown_renderer_poll_code_block_ready_compatible() {
        let (tx, rx) = mpsc::sync_channel(1);

        let base_line = RenderLine {
            spans: vec![StyledSpan {
                text: "original code".to_string(),
                style: Style::default(),
            }],
            visual_width: 13,
            is_blank: false,
            image_url: None,
            source_line: 42,
        };

        let mut renderer = MarkdownRenderer {
            document: Some(DocumentState::Working(RenderedDocument::new(vec![
                base_line,
            ]))),
            events: Some(rx),
            current_key: None,
            generation: 1,
            cancel: None,
            viewport: Arc::new(AtomicU64::new(pack_viewport(0, 10))),
            current_page: 0,
            page_height: 10,
            scroll_offset: 0,
            pending_source_anchor: None,
            pending: true,
        };

        let compatible_line = RenderLine {
            spans: vec![StyledSpan {
                text: "original code".to_string(),
                style: Style::default().add_modifier(Modifier::BOLD),
            }],
            visual_width: 13,
            is_blank: false,
            image_url: None,
            source_line: 42,
        };

        tx.send(RenderEvent::CodeBlockReady {
            generation: 1,
            line_range: 0..1,
            lines: vec![compatible_line],
        })
        .unwrap();

        let redraw = renderer.poll();
        assert!(redraw);

        if let Some(DocumentState::Working(doc)) = &renderer.document {
            assert_eq!(doc.line_count(), 1);
            let spans = &doc.lines()[0].spans;
            assert_eq!(spans[0].text, "original code");
            assert_ne!(spans[0].style, Style::default());
        } else {
            panic!("Expected working document state");
        }
    }
}
