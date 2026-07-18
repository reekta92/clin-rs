mod builtin;
mod source_highlight;
mod style;

pub(crate) use builtin::{default_code_theme, render_builtin};
pub(crate) use source_highlight::SourceHighlighter;
pub(crate) use style::{MarkdownTheme, RenderLine};

use ratatui::style::{Color, Style};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

/// Bundled render flags threaded into `render_builtin`. Replaces the prior
/// loose `(syntax_hl, wrap, icon_mode)` args.
#[derive(Debug, Clone)]
pub(crate) struct MdRenderOpts {
    pub syntax_hl: bool,
    pub wrap: bool,
    pub icon_mode: crate::config::IconMode,
    pub code_theme: String,
    pub code_line_numbers: bool,
    pub wrap_indicator: bool,
    pub link_url_max: usize,
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
        }
    }
}

impl MdRenderOpts {
    /// Build render options from the app config (used by all three preview call sites).
    pub(crate) fn from_config(config: &crate::config::ClinConfig) -> Self {
        Self {
            syntax_hl: config.core.syntax_highlighting,
            wrap: config.core.preview_wrap,
            icon_mode: config.ui.icon_mode,
            code_theme: config.core.code_theme.clone(),
            code_line_numbers: config.core.code_line_numbers,
            wrap_indicator: config.core.preview_wrap_indicator,
            link_url_max: config.core.link_url_max_length,
        }
    }
}

/// Synchronous render path: renders content inline (no background thread).
/// Used by READ mode to produce the rendered grid on demand.
pub(crate) fn render_builtin_sync(
    content: &str,
    cols: u16,
    theme: &crate::app_theme::AppThemeColors,
    opts: &MdRenderOpts,
) -> Vec<RenderLine> {
    let md_theme = style::MarkdownTheme::from_app_theme(theme);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    builtin::render_builtin(content, cols, &md_theme, opts, &cancel).0
}

/// Renders markdown into a paged grid of `(char, Style)` cells.
///
/// Public API mirrors the previous `vt100` / glow-based implementation exactly,
/// so all three consumers (list preview, editor split, graf preview) are untouched.
///
/// Internally uses **comrak** for GFM parsing and **syntect** for optional
/// code-block syntax highlighting, all in a cancelable background thread.
#[allow(clippy::type_complexity)]
pub struct MarkdownRenderer {
    pending: Option<mpsc::Receiver<(Vec<RenderLine>, Vec<(usize, String)>)>>,
    lines: Vec<RenderLine>,
    /// Image slots: Vec<(rendered_line_idx, url)> from the background render.
    image_slots: Vec<(usize, String)>,
    /// Image slots per-page, derived from image_slots by build_pages.
    page_image_slots: Vec<Vec<(usize, String)>>,
    cancel_token: Arc<AtomicBool>,
    pages: Vec<Vec<Vec<(char, Style)>>>,
    current_page: usize,
    total_pages: usize,
    content_empty: bool,
}

impl std::fmt::Debug for MarkdownRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarkdownRenderer")
            .field("current_page", &self.current_page)
            .field("total_pages", &self.total_pages)
            .field("content_empty", &self.content_empty)
            .finish_non_exhaustive()
    }
}

impl Drop for MarkdownRenderer {
    fn drop(&mut self) {
        self.cancel_token.store(true, Ordering::Relaxed);
    }
}

impl MarkdownRenderer {
    pub fn new(_cols: u16) -> Self {
        Self {
            pending: None,
            lines: Vec::new(),
            image_slots: Vec::new(),
            page_image_slots: Vec::new(),
            cancel_token: Arc::new(AtomicBool::new(false)),
            pages: Vec::new(),
            current_page: 0,
            total_pages: 0,
            content_empty: true,
        }
    }

    /// Minimal shim that renders with default theme + syntax highlighting +
    /// wrapping enabled.  Kept for backwards compatibility and in-module tests.
    pub fn render(&mut self, content: &str, cols: u16) {
        self.render_with(
            content,
            cols,
            &crate::app_theme::AppThemeColors::default(),
            &MdRenderOpts::default(),
        );
    }

    /// Render markdown content in a background thread.
    ///
    /// - `content` — raw markdown string
    /// - `cols` — terminal column width
    /// - `theme` — app colour palette (all render styles derive from it)
    /// - `opts` — render options bundle (`MdRenderOpts`)
    pub(crate) fn render_with(
        &mut self,
        content: &str,
        cols: u16,
        theme: &crate::app_theme::AppThemeColors,
        opts: &MdRenderOpts,
    ) {
        // Reset state
        self.pages.clear();
        self.current_page = 0;
        self.total_pages = 0;
        self.lines.clear();
        self.content_empty = content.is_empty();

        if content.is_empty() {
            self.pending = None;
            return;
        }

        let cancel_token = Arc::clone(&self.cancel_token);
        // Reset so a new render cancels an in-flight one
        cancel_token.store(false, Ordering::Relaxed);

        let md_theme = MarkdownTheme::from_app_theme(theme);
        let content_owned = content.to_owned();
        let opts = opts.clone();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);

        std::thread::spawn(move || {
            if cancel_token.load(Ordering::Relaxed) {
                return;
            }

            let (lines, slots) =
                builtin::render_builtin(&content_owned, cols, &md_theme, &opts, &cancel_token);

            let _ = tx.send((lines, slots));
        });
    }

    /// Poll the background renderer.  Returns `true` if lines were received
    /// (ready for `build_pages`), `false` if still pending.
    pub fn poll(&mut self) -> bool {
        let rx = match &self.pending {
            Some(rx) => rx,
            None => return false,
        };

        match rx.try_recv() {
            Ok((lines, slots)) => {
                self.lines = lines;
                self.image_slots = slots;
                self.pending = None;
                true
            }
            Err(_) => false,
        }
    }
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn is_content_empty(&self) -> bool {
        self.content_empty
    }

    pub fn pages_built(&self) -> bool {
        !self.pages.is_empty() || self.content_empty
    }

    /// Build page chunks from the rendered lines.
    ///
    /// `theme_bg` is accepted for API compatibility but **ignored** — each cell
    /// already carries its own background colour from the markdown theme.
    pub fn build_pages(&mut self, visible_rows: u16, _theme_bg: Option<Color>) {
        // Trim trailing lines that are entirely whitespace / empty
        let last_non_empty = self
            .lines
            .iter()
            .rposition(|l| l.cells.iter().any(|(c, _)| !c.is_whitespace()))
            .unwrap_or(0);

        let page_height = (visible_rows as usize).max(1);
        self.pages = self.lines[..=last_non_empty]
            .chunks(page_height)
            .map(|chunk| chunk.iter().map(|l| l.cells.clone()).collect())
            .collect();
        // Build page image slots: map global line indices to page-local indices
        // Rebuild per-page slots from flat image_slots
        let mut page_slots: Vec<Vec<(usize, String)>> = Vec::new();
        for (global_line, url) in &self.image_slots {
            if *global_line > last_non_empty {
                continue;
            }
            let page_idx = *global_line / page_height;
            let local_line = *global_line % page_height;
            while page_slots.len() <= page_idx {
                page_slots.push(Vec::new());
            }
            page_slots[page_idx].push((local_line, url.clone()));
        }
        self.page_image_slots = page_slots;

        self.total_pages = self.pages.len().max(1);
        self.current_page = 0;
    }

    pub fn current_page(&self) -> usize {
        self.current_page
    }

    pub fn current_page_image_slots(&self) -> &[(usize, String)] {
        self.page_image_slots
            .get(self.current_page)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn current_page_grid(&self) -> Option<&[Vec<(char, Style)>]> {
        self.pages.get(self.current_page).map(|v| v.as_slice())
    }

    pub fn total_pages(&self) -> usize {
        self.total_pages
    }

    pub fn next_page(&mut self) {
        if self.total_pages > 0 && self.current_page < self.total_pages - 1 {
            self.current_page += 1;
        }
    }

    pub fn prev_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }

    /// Advance one page, wrapping to the first page when past the last.
    pub fn next_page_wrap(&mut self) {
        if self.total_pages <= 1 {
            return;
        }
        if self.current_page < self.total_pages - 1 {
            self.current_page += 1;
        } else {
            self.current_page = 0;
        }
    }

    /// Go back one page, wrapping to the last page when before the first.
    pub fn prev_page_wrap(&mut self) {
        if self.total_pages <= 1 {
            return;
        }
        if self.current_page > 0 {
            self.current_page -= 1;
        } else {
            self.current_page = self.total_pages - 1;
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_theme::AppThemeColors;

    #[test]
    fn test_render_builtin_produces_lines() {
        let content = "# Vault (Root)\n\n## Folders\n- Documents\n\n## Notes\n- hello\n";
        let mut renderer = MarkdownRenderer::new(80);
        let theme = AppThemeColors::default();
        renderer.render_with(content, 80, &theme, &MdRenderOpts::default());
        // Poll until done (thread runs synchronously here due to simple input)
        let mut tries = 0;
        let mut completed = false;
        while renderer.is_pending() && tries < 50 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if renderer.poll() {
                completed = true;
                break;
            }
            tries += 1;
        }
        assert!(
            completed || !renderer.is_pending(),
            "render should complete"
        );

        renderer.build_pages(30, theme.bg);
        assert!(renderer.pages_built(), "pages should be built");
        assert!(renderer.total_pages() >= 1, "at least one page");
        assert!(!renderer.is_content_empty(), "content not empty");
    }

    #[test]
    fn test_empty_input_returns_empty() {
        let mut renderer = MarkdownRenderer::new(80);
        let theme = AppThemeColors::default();
        renderer.render_with("", 80, &theme, &MdRenderOpts::default());
        assert!(renderer.is_content_empty());
        assert!(!renderer.is_pending());
    }
}
