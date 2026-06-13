use std::io::{Read, Write};
use std::sync::mpsc;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use ratatui::style::{Color, Modifier, Style};

use once_cell::sync::Lazy;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tui_term::vt100;

static GLOW_AVAILABLE: Lazy<AtomicBool> =
    Lazy::new(|| AtomicBool::new(which::which("glow").is_ok()));

pub fn glow_available() -> bool {
    GLOW_AVAILABLE.load(Ordering::Relaxed)
}

struct RenderResult {
    parser: vt100::Parser,
    content_rows: u16,
}

enum RendererState {
    Idle,
    Pending(mpsc::Receiver<Option<RenderResult>>),
    Ready,
}

pub struct MarkdownRenderer {
    state: RendererState,
    parser: Option<vt100::Parser>,
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

impl MarkdownRenderer {
    pub fn new(cols: u16) -> Self {
        let rows = 200;
        Self {
            state: RendererState::Idle,
            parser: Some(vt100::Parser::new(rows, cols, 0)),
            content_rows: rows,
            cancel_token: Arc::new(AtomicBool::new(false)),
            pages: Vec::new(),
            current_page: 0,
            total_pages: 0,
            content_empty: true,
            theme_bg: None,
        }
    }

    pub fn render(&mut self, content: &str, cols: u16) {
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
            self.parser = Some(vt100::Parser::new(1, cols, 0));
            self.state = RendererState::Ready;
            return;
        }

        let content_owned = content.to_owned();
        let (tx, rx) = mpsc::channel();
        self.state = RendererState::Pending(rx);

        let cancel_token = Arc::clone(&self.cancel_token);
        std::thread::spawn(move || {
            let result = render_in_thread(&content_owned, cols, estimated_rows, cancel_token);
            let _ = tx.send(result);
        });
    }

    pub fn poll(&mut self) -> bool {
        let rx = match &self.state {
            RendererState::Pending(rx) => rx,
            _ => return false,
        };

        match rx.try_recv() {
            Ok(Some(result)) => {
                self.content_rows = result.content_rows;
                self.content_empty = result.parser.screen().contents().trim().is_empty();
                self.parser = Some(result.parser);
                self.state = RendererState::Ready;
                true
            }
            Ok(None) => {
                self.state = RendererState::Ready;
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.state = RendererState::Ready;
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
        let Some(parser) = &self.parser else {
            return;
        };
        let screen = parser.screen();
        let cols = screen.size().1;

        let mut all_rows: Vec<Vec<(char, Style)>> = Vec::new();
        let mut last_non_empty_row = 0usize;

        let mut effective_rows = 0u16;
        'scan: for r in (0..self.content_rows).rev() {
            for c in 0..cols {
                if let Some(screen_cell) = screen.cell(r, c)
                    && screen_cell.has_contents()
                    && !screen_cell.contents().trim().is_empty()
                {
                    effective_rows = r + 1;
                    break 'scan;
                }
            }
        }

        if effective_rows == 0 && self.content_rows > 0 {
            effective_rows = 1;
        }

        for row_idx in 0..effective_rows {
            let mut row_data: Vec<(char, Style)> = Vec::with_capacity(cols as usize);
            let mut has_content = false;
            for col in 0..cols {
                if let Some(screen_cell) = screen.cell(row_idx, col) {
                    let ch = if screen_cell.has_contents() {
                        let contents = screen_cell.contents();
                        has_content = has_content || !contents.trim().is_empty();
                        contents.chars().next().unwrap_or(' ')
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

                    let fg = convert_color(screen_cell.fgcolor());
                    let bg = match screen_cell.bgcolor() {
                        vt100::Color::Default => theme_bg.unwrap_or(Color::Reset),
                        other => convert_color(other),
                    };
                    style = style.fg(fg).bg(bg);

                    row_data.push((ch, style));
                } else {
                    row_data.push((' ', Style::default()));
                }
            }
            if has_content {
                last_non_empty_row = all_rows.len();
            }
            all_rows.push(row_data);
        }

        let trimmed_rows = &all_rows[..=last_non_empty_row.min(all_rows.len().saturating_sub(1))];

        let page_height = (visible_rows as usize).max(1);
        self.pages.clear();
        for chunk in trimmed_rows.chunks(page_height) {
            self.pages.push(chunk.to_vec());
        }

        self.total_pages = self.pages.len().max(1);
        self.current_page = 0;

        self.parser = None;
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

fn render_in_thread(
    content: &str,
    cols: u16,
    estimated_rows: u16,
    cancel_token: Arc<AtomicBool>,
) -> Option<RenderResult> {
    let mut parser = vt100::Parser::new(estimated_rows, cols, 0);

    if !glow_available() {
        process_fallback(&mut parser, content, estimated_rows);
        return Some(RenderResult {
            parser,
            content_rows: estimated_rows,
        });
    }

    if cancel_token.load(Ordering::Relaxed) {
        return None;
    }

    let mut temp_file = tempfile::Builder::new()
        .suffix(".md")
        .prefix("clin_md_")
        .tempfile()
        .ok()?;

    temp_file.write_all(content.as_bytes()).ok()?;
    temp_file.flush().ok()?;

    let temp_path = temp_file.path().to_owned();

    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: estimated_rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .ok()?;

    let mut cmd = CommandBuilder::new("glow");
    cmd.arg("-w");
    cmd.arg(cols.to_string());
    cmd.arg("-s");
    cmd.arg("dark");
    cmd.arg(&temp_path);
    cmd.env("TERM", "dumb");

    let mut child = pair.slave.spawn_command(cmd).ok()?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().ok()?;
    let _writer = pair.master.take_writer();

    let mut output = Vec::new();
    let mut buf = [0u8; 8192];

    loop {
        if cancel_token.load(Ordering::Relaxed) {
            let _ = child.kill();
            return None;
        }

        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => output.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }

    let exit_ok = child.wait().map(|s| s.success()).unwrap_or(false);

    drop(_writer);
    drop(reader);
    drop(pair.master);
    drop(temp_file);

    if !output.is_empty() && exit_ok {
        parser.process(&output);
    } else {
        process_fallback(&mut parser, content, estimated_rows);
    }

    Some(RenderResult {
        parser,
        content_rows: estimated_rows,
    })
}

fn process_fallback(parser: &mut vt100::Parser, content: &str, estimated_rows: u16) {
    let mut fallback_output = Vec::new();
    for line in content.lines().take((estimated_rows - 3) as usize) {
        fallback_output.extend_from_slice(line.as_bytes());
        fallback_output.push(b'\n');
    }
    fallback_output
        .extend_from_slice(b"\n\x1b[38;5;242mInstall 'glow' for markdown rendering\x1b[0m\n");
    parser.process(&fallback_output);
}

fn convert_color(value: vt100::Color) -> Color {
    match value {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(0) => Color::Black,
        vt100::Color::Idx(1) => Color::Red,
        vt100::Color::Idx(2) => Color::Green,
        vt100::Color::Idx(3) => Color::Yellow,
        vt100::Color::Idx(4) => Color::Blue,
        vt100::Color::Idx(5) => Color::Magenta,
        vt100::Color::Idx(6) => Color::Cyan,
        vt100::Color::Idx(7) => Color::Gray,
        vt100::Color::Idx(8) => Color::DarkGray,
        vt100::Color::Idx(9) => Color::LightRed,
        vt100::Color::Idx(10) => Color::LightGreen,
        vt100::Color::Idx(11) => Color::LightYellow,
        vt100::Color::Idx(12) => Color::LightBlue,
        vt100::Color::Idx(13) => Color::LightMagenta,
        vt100::Color::Idx(14) => Color::LightCyan,
        vt100::Color::Idx(15) => Color::White,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
