use std::io::{Read, Write};
use std::sync::mpsc;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Widget},
};

use once_cell::sync::Lazy;
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
    parser: vt100::Parser,
    scroll_offset: u16,
    content_rows: u16,
}

impl MarkdownRenderer {
    pub fn new(cols: u16) -> Self {
        let rows = 200;
        Self {
            state: RendererState::Idle,
            parser: vt100::Parser::new(rows, cols, 0),
            scroll_offset: 0,
            content_rows: rows,
        }
    }

    pub fn render(&mut self, content: &str, cols: u16) {
        let estimated_rows = if content.is_empty() {
            1
        } else {
            (content.lines().count() as u16).max(50).min(2000)
        };

        self.scroll_offset = 0;
        self.content_rows = estimated_rows;

        if content.is_empty() {
            self.parser = vt100::Parser::new(1, cols, 0);
            self.state = RendererState::Ready;
            return;
        }

        let content_owned = content.to_owned();
        let (tx, rx) = mpsc::channel();
        self.state = RendererState::Pending(rx);

        std::thread::spawn(move || {
            let result = render_in_thread(&content_owned, cols, estimated_rows);
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
                self.parser = result.parser;
                self.content_rows = result.content_rows;
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

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }

    pub fn content_rows(&self) -> u16 {
        self.content_rows
    }

    pub fn scroll_up(&mut self, n: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: u16, visible_rows: u16) {
        let max = self.content_rows.saturating_sub(visible_rows);
        self.scroll_offset = (self.scroll_offset + n).min(max);
    }
}

fn render_in_thread(content: &str, cols: u16, estimated_rows: u16) -> Option<RenderResult> {
    let mut parser = vt100::Parser::new(estimated_rows, cols, 0);

    if !glow_available() {
        process_fallback(&mut parser, content, estimated_rows);
        return Some(RenderResult {
            parser,
            content_rows: estimated_rows,
        });
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

    let mut child = pair.slave.spawn_command(cmd).ok()?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().ok()?;
    let _writer = pair.master.take_writer();

    let mut output = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
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

pub struct ScrollablePseudoTerminal<'a> {
    screen: &'a vt100::Screen,
    scroll_offset: u16,
    block: Option<Block<'a>>,
}

impl<'a> ScrollablePseudoTerminal<'a> {
    pub fn new(screen: &'a vt100::Screen) -> Self {
        Self {
            screen,
            scroll_offset: 0,
            block: None,
        }
    }

    pub fn scroll_offset(mut self, offset: u16) -> Self {
        self.scroll_offset = offset;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl Widget for ScrollablePseudoTerminal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = match &self.block {
            Some(b) => {
                let inner = b.inner(area);
                b.clone().render(area, buf);
                inner
            }
            None => area,
        };

        let cols = inner.width;
        let rows = inner.height;
        let col_start = inner.x;
        let row_start = inner.y;

        for row in 0..rows {
            let screen_row = row + self.scroll_offset;
            for col in 0..cols {
                let buf_col = col + col_start;
                let buf_row = row + row_start;
                if let Some(screen_cell) = self.screen.cell(screen_row, col) {
                    if screen_cell.has_contents() {
                        let cell = &mut buf[(buf_col, buf_row)];
                        cell.set_symbol(&screen_cell.contents());
                    }
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
                    let bg = convert_color(screen_cell.bgcolor());
                    style = style.fg(fg).bg(bg);

                    let cell = &mut buf[(buf_col, buf_row)];
                    cell.set_style(style);
                }
            }
        }
    }
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
