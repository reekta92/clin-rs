use std::io::IsTerminal;
use std::sync::LazyLock;

use crossterm::style::{Attribute, Color, Stylize};

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal());

/// True if colors should be emitted: NO_COLOR unset AND stdout is a TTY.
fn enabled() -> bool {
    *ENABLED
}

// --- Color primitives ---

/// Apply foreground color if enabled, else return plain string.
fn paint(text: &str, color: Color) -> String {
    if enabled() {
        text.with(color).to_string()
    } else {
        text.to_string()
    }
}

pub fn green(s: &str) -> String {
    paint(s, Color::Green)
}
pub fn red(s: &str) -> String {
    paint(s, Color::Red)
}
pub fn yellow(s: &str) -> String {
    paint(s, Color::Yellow)
}
pub fn cyan(s: &str) -> String {
    paint(s, Color::Cyan)
}
pub fn blue(s: &str) -> String {
    paint(s, Color::Blue)
}

/// Dim/gray text for secondary info, metadata, hints.
pub fn dim(s: &str) -> String {
    paint(s, Color::DarkGrey)
}

/// Bold text. Combines with a color by calling `bold(&cyan("text"))` —
/// but `bold` itself only applies the bold attribute.
pub fn bold(s: &str) -> String {
    if enabled() {
        s.attribute(Attribute::Bold).to_string()
    } else {
        s.to_string()
    }
}

// --- Semantic helpers (with Unicode symbols) ---

/// ✓ <msg> — green checkmark prefix for success.
pub fn success(msg: &str) -> String {
    format!("{} {}", green("✓"), msg)
}

/// ✗ <msg> — red cross prefix for errors.
pub fn error(msg: &str) -> String {
    format!("{} {}", red("✗"), msg)
}

/// ! <msg> — yellow exclamation prefix for warnings.
pub fn warning(msg: &str) -> String {
    format!("{} {}", yellow("!"), msg)
}

/// • <msg> — blue bullet prefix for informational notes.
pub fn info(msg: &str) -> String {
    format!("{} {}", blue("•"), msg)
}

///   → <msg> — dim indented hint (e.g. "Run 'clin storage migrate'").
pub fn hint(msg: &str) -> String {
    format!("  {} {}", dim("→"), msg)
}

/// ─ Title ──────── — section divider with bold title.
pub fn section(title: &str) -> String {
    let padding = 50usize.saturating_sub(title.len() + 3);
    let bar = "─".repeat(padding);
    if enabled() {
        format!("{} {} {}", dim("─"), bold(title), dim(&bar))
    } else {
        format!("─ {title} {bar}")
    }
}

/// Color a filesystem path in cyan.
pub fn path(p: impl AsRef<std::path::Path>) -> String {
    cyan(&p.as_ref().display().to_string())
}

// --- Clap theme ---

use clap::builder::{Styles, styling::AnsiColor};

pub const CLAP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::BrightGreen.on_default().bold())
    .usage(AnsiColor::BrightGreen.on_default().bold())
    .literal(AnsiColor::BrightCyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::BrightRed.on_default().bold())
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::Yellow.on_default());
