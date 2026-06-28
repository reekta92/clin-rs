use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use ratatui::style::Color;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tui_term::vt100;

pub(crate) static GLOW_AVAILABLE: std::sync::LazyLock<AtomicBool> =
    std::sync::LazyLock::new(|| AtomicBool::new(which::which("glow").is_ok()));

pub fn glow_available() -> bool {
    GLOW_AVAILABLE.load(Ordering::Relaxed)
}

pub(crate) struct RenderResult {
    pub(crate) parser: vt100::Parser,
    pub(crate) content_rows: u16,
}

pub(crate) fn render_in_thread(
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
    cmd.env("PAGER", "cat");
    cmd.env("GLOW_PAGER", "cat");

    let mut child = pair.slave.spawn_command(cmd).ok()?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().ok()?;
    let _writer = pair.master.take_writer();

    let mut output = Vec::new();
    let mut buf = [0u8; 8192];

    loop {
        if cancel_token.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
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

pub(crate) fn convert_color(value: vt100::Color) -> Color {
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
