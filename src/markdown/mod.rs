pub mod builtin;
pub mod glow;
pub mod style;
pub(crate) mod worker;

use crate::app_theme::AppThemeColors;
use ratatui::style::{Color, Modifier, Style};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use tui_term::vt100;

pub use glow::glow_available;
pub use style::{MarkdownTheme, RenderLine};

pub(crate) enum RenderResult {
    Glow {
        parser: vt100::Parser,
        content_rows: u16,
    },
    Builtin {
        lines: Vec<RenderLine>,
    },
}

enum RendererState {
    Idle,
    Pending(mpsc::Receiver<Option<RenderResult>>),
    Ready,
}

pub struct MarkdownRenderer {
    state: RendererState,
    lines: Vec<RenderLine>,
    content_rows: u16,
    cancel_token: Arc<AtomicBool>,
    pages: Vec<Vec<Vec<(char, Style)>>>,
    current_page: usize,
    total_pages: usize,
    content_empty: bool,
    theme_bg: Option<Color>,
}

impl Drop for MarkdownRenderer {
    fn drop(&mut self) {
        self.cancel_token.store(true, Ordering::Relaxed);
    }
}

/// Execute a render job synchronously. Returns the render result, or None if cancelled.
pub(crate) fn execute_render(
    content: &zeroize::Zeroizing<String>,
    cols: u16,
    estimated_rows: u16,
    theme: &MarkdownTheme,
    wrap: bool,
    syntax_hl: bool,
    renderer: crate::config::MarkdownRendererKind,
    cancel_token: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Option<RenderResult> {
    if cancel_token.load(Ordering::Relaxed) {
        return None;
    }

    let use_glow = match renderer {
        crate::config::MarkdownRendererKind::Glow => true,
        crate::config::MarkdownRendererKind::Builtin => false,
        crate::config::MarkdownRendererKind::Auto => glow::glow_available(),
    };

    if use_glow {
        let res = glow::render_in_thread(content, cols, estimated_rows, Arc::clone(&cancel_token));
        res.map(|r| RenderResult::Glow {
            parser: r.parser,
            content_rows: r.content_rows,
        })
    } else {
        let res = builtin::render_builtin(
            content,
            cols,
            theme,
            wrap,
            syntax_hl,
            Arc::clone(&cancel_token),
        );
        res.map(|lines| RenderResult::Builtin { lines })
    }
}

impl MarkdownRenderer {
    pub fn new(_cols: u16) -> Self {
        Self {
            state: RendererState::Idle,
            lines: Vec::new(),
            content_rows: 0,
            cancel_token: Arc::new(AtomicBool::new(false)),
            pages: Vec::new(),
            current_page: 0,
            total_pages: 0,
            content_empty: true,
            theme_bg: None,
        }
    }

    pub fn render(&mut self, content: &str, cols: u16) {
        let default_colors = AppThemeColors::default();
        self.render_with(
            content,
            cols,
            &default_colors,
            crate::config::MarkdownRendererKind::Auto,
            true,
        );
    }

    pub fn render_with(
        &mut self,
        content: &str,
        cols: u16,
        theme: &AppThemeColors,
        renderer: crate::config::MarkdownRendererKind,
        syntax_hl: bool,
    ) {
        let estimated_rows = if content.is_empty() {
            1
        } else {
            (((content.lines().count() as u32 * 10) + 300).min(20000) as u16).clamp(300, 20000)
        };

        self.content_rows = estimated_rows;
        self.pages.clear();
        self.current_page = 0;
        self.total_pages = 0;
        self.content_empty = content.is_empty();

        if content.is_empty() {
            self.lines = vec![RenderLine { cells: Vec::new() }];
            self.state = RendererState::Ready;
            return;
        }

        self.cancel_token.store(false, Ordering::Relaxed);
        let content_owned = zeroize::Zeroizing::new(content.to_owned());
        let theme_mapped = MarkdownTheme::from_app_theme(theme);
        let wrap = cols < 1000;

        let (tx, rx) = mpsc::channel();
        self.state = RendererState::Pending(rx);

        let job = worker::Job {
            content: content_owned,
            cols,
            estimated_rows,
            theme: theme_mapped,
            wrap,
            syntax_hl,
            renderer,
            cancel: Arc::clone(&self.cancel_token),
            result_tx: tx,
        };
        if let Err(returned) = worker::submit(job) {
            let res = execute_render(
                &returned.content,
                returned.cols,
                returned.estimated_rows,
                &returned.theme,
                returned.wrap,
                returned.syntax_hl,
                returned.renderer,
                Arc::clone(&self.cancel_token),
            );
            self.apply_result(res);
        }
    }

    /// Apply a render result, setting state to Ready.
    pub(crate) fn apply_result(&mut self, res: Option<RenderResult>) {
        match res {
            Some(RenderResult::Glow {
                parser,
                content_rows,
            }) => {
                self.content_rows = content_rows;
                let screen = parser.screen();
                let cols = screen.size().1;
                let mut all_rows = Vec::new();
                for row_idx in 0..self.content_rows {
                    let mut row_data = Vec::with_capacity(cols as usize);
                    for col in 0..cols {
                        if let Some(screen_cell) = screen.cell(row_idx, col) {
                            let ch = if screen_cell.has_contents() {
                                screen_cell.contents().chars().next().unwrap_or(' ')
                            } else {
                                ' '
                            };

                            let mut style = Style::reset();
                            if screen_cell.bold() {
                                style = style.add_modifier(Modifier::BOLD);
                            }
                            if screen_cell.italic() {
                                style = style.add_modifier(Modifier::ITALIC);
                            }
                            if screen_cell.underline() {
                                style = style.add_modifier(Modifier::UNDERLINED);
                            }
                            if screen_cell.inverse() {
                                style = style.add_modifier(Modifier::REVERSED);
                            }

                            let fg = glow::convert_color(screen_cell.fgcolor());
                            let bg = match screen_cell.bgcolor() {
                                vt100::Color::Default => self.theme_bg.unwrap_or(Color::Reset),
                                other => glow::convert_color(other),
                            };
                            style = style.fg(fg).bg(bg);

                            row_data.push((ch, style));
                        } else {
                            row_data.push((' ', Style::default()));
                        }
                    }
                    all_rows.push(RenderLine { cells: row_data });
                }
                self.lines = all_rows;
                let screen_contents = screen.contents();
                self.content_empty = screen_contents.trim().is_empty();
            }
            Some(RenderResult::Builtin { lines }) => {
                self.lines = lines;
                self.content_rows = self.lines.len() as u16;
                self.content_empty = self
                    .lines
                    .iter()
                    .all(|l| l.cells.iter().all(|(c, _)| c.is_whitespace()));
            }
            None => {}
        }
        self.state = RendererState::Ready;
    }

    pub fn poll(&mut self) -> bool {
        let rx = match &self.state {
            RendererState::Pending(rx) => rx,
            _ => return false,
        };

        match rx.try_recv() {
            Ok(Some(result)) => {
                self.apply_result(Some(result));
                true
            }
            Ok(None) => {
                self.apply_result(None);
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.apply_result(None);
                true
            }
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, RendererState::Pending(_))
    }

    pub fn is_content_empty(&self) -> bool {
        self.content_empty
    }

    pub fn build_pages(&mut self, visible_rows: u16, theme_bg: Option<Color>) {
        self.theme_bg = theme_bg;

        let trimmed_len = if self.content_empty {
            0
        } else {
            self.lines
                .iter()
                .rposition(|line| line.cells.iter().any(|(c, _)| !c.is_whitespace()))
                .map(|idx| idx + 1)
                .unwrap_or(self.lines.len())
        };

        let trimmed_lines = &self.lines[..trimmed_len];
        let page_height = (visible_rows as usize).max(1);
        self.pages.clear();
        for chunk in trimmed_lines.chunks(page_height) {
            let page: Vec<Vec<(char, Style)>> = chunk
                .iter()
                .map(|line| {
                    line.cells
                        .iter()
                        .map(|&(ch, mut style)| {
                            if style.bg == Some(Color::Reset) || style.bg == None {
                                if let Some(bg) = theme_bg {
                                    style = style.bg(bg);
                                }
                            }
                            (ch, style)
                        })
                        .collect()
                })
                .collect();
            self.pages.push(page);
        }

        self.total_pages = self.pages.len().max(1);
        self.current_page = 0;
    }

    pub fn pages_built(&self) -> bool {
        !self.pages.is_empty() || self.content_empty
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

    /// Regression test: rapid cycling of MarkdownRenderer must not pile up threads.
    /// Under the old detached-thread design, 300 sequential create+render+drop cycles
    /// would spawn 300 threads and OOM or crash. The new single-worker design bounds
    /// concurrency to 1 thread regardless of churn.
    #[test]
    fn rapid_cycling_does_not_pile_up_threads() {
        let sample_md = concat!(
            "# Hello\n",
            "This is a test paragraph with **bold** and *italic* text.\n",
            "\n",
            "- list item 1\n",
            "- list item 2\n",
            "- list item 3\n",
            "\n",
            "```rust\n",
            "fn hello() -> &'static str {\n",
            "    \"world\"\n",
            "}\n",
            "```\n",
            "\n",
            "| Col A | Col B |\n",
            "|-------|-------|\n",
            "| A1    | B1    |\n",
            "| A2    | B2    |\n",
        );

        let started = std::time::Instant::now();
        // Rapid churn: 300 create+render+drop cycles
        for _ in 0..300 {
            let mut r = MarkdownRenderer::new(78);
            r.render_with(
                sample_md,
                78,
                &AppThemeColors::default(),
                crate::config::MarkdownRendererKind::Builtin,
                true,
            );
            // Drop r without polling — simulates preview cycling where the renderer
            // is dropped before the render completes. The worker should handle this.
        }

        // After all the churn, create one final renderer, render, poll, build, verify.
        let mut r = MarkdownRenderer::new(78);
        r.render_with(
            sample_md,
            78,
            &AppThemeColors::default(),
            crate::config::MarkdownRendererKind::Builtin,
            true,
        );
        // Poll until result is ready (timeout after 8s)
        let poll_deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while !r.poll() {
            if std::time::Instant::now() > poll_deadline {
                panic!("Timed out waiting for render after rapid cycling");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        r.build_pages(34, None);
        assert!(r.pages_built(), "Pages should be built after render");
        assert!(
            r.total_pages() >= 1,
            "Should have at least 1 page, got {}",
            r.total_pages()
        );

        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs() < 8,
            "Rapid cycling test took {}.{:03}s — suspiciously slow (budget 8s)",
            elapsed.as_secs(),
            elapsed.subsec_millis(),
        );
    }
}
