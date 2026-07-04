mod builtin;
mod style;

use style::MarkdownTheme;
pub(crate) use style::RenderLine;

use ratatui::style::{Color, Style};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

/// Renders markdown into a paged grid of `(char, Style)` cells.
///
/// Public API mirrors the previous `vt100` / glow-based implementation exactly,
/// so all three consumers (list preview, editor split, graf preview) are untouched.
///
/// Internally uses **comrak** for GFM parsing and **syntect** for optional
/// code-block syntax highlighting, all in a cancelable background thread.
pub struct MarkdownRenderer {
    pending: Option<mpsc::Receiver<Vec<RenderLine>>>,
    lines: Vec<RenderLine>,
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
            true,
            true,
            crate::config::IconMode::default(),
        );
    }

    /// Render markdown content in a background thread.
    ///
    /// - `content` — raw markdown string
    /// - `cols` — terminal column width
    /// - `theme` — app colour palette (all render styles derive from it)
    /// - `syntax_hl` — whether to syntax-highlight fenced code blocks with syntect
    /// - `wrap` — hard-wrap lines at `cols` when true, truncate when false
    pub fn render_with(
        &mut self,
        content: &str,
        cols: u16,
        theme: &crate::app_theme::AppThemeColors,
        syntax_hl: bool,
        wrap: bool,
        icon_mode: crate::config::IconMode,
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
        let (tx, rx) = mpsc::channel();

        self.pending = Some(rx);

        std::thread::spawn(move || {
            if cancel_token.load(Ordering::Relaxed) {
                return;
            }

            let lines = builtin::render_builtin(
                &content_owned,
                cols,
                &md_theme,
                wrap,
                syntax_hl,
                icon_mode,
                &cancel_token,
            );

            let _ = tx.send(lines);
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
            Ok(lines) => {
                self.lines = lines;
                self.pending = None;
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                true
            }
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

        self.total_pages = self.pages.len().max(1);
        self.current_page = 0;
    }

    pub fn current_page_grid(&self) -> Option<&Vec<Vec<(char, Style)>>> {
        self.pages.get(self.current_page)
    }

    pub fn current_page(&self) -> usize {
        self.current_page
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
        renderer.render_with(
            content,
            80,
            &theme,
            false,
            true,
            crate::config::IconMode::default(),
        );
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
        renderer.render_with(
            "",
            80,
            &theme,
            false,
            true,
            crate::config::IconMode::default(),
        );
        assert!(renderer.is_content_empty());
        assert!(!renderer.is_pending());
    }
}
