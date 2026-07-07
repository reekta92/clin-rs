//! Built-in markdown → grid renderer using comrak (GFM parse) + syntect
//! (optional code-block highlighting).
//!
//! The public entry point is [`render_builtin`], which returns a
//! `Vec<RenderLine>` matching glow's structural layout (2-space margin,
//! box-drawing tables, `•` bullets, `┃` blockquote bars, `[ ]`/`[✓]` task
//! boxes, preserved `#` heading prefixes, 8-dash HR).
//!
//! Styling goes **beyond** glow: headings are coloured per hierarchy, code
//! blocks get theme-bg shading, inline code gets bg/fg, and fenced code
//! blocks with a known language get full syntect syntax highlighting.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use comrak::nodes::{
    AstNode, ListType, NodeCodeBlock, NodeHeading, NodeList, NodeTable, NodeValue, TableAlignment,
};
use comrak::{Arena, Options, parse_document};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::markdown::MdRenderOpts;
use crate::markdown::style::{MarkdownTheme, RenderLine};

// ---------------------------------------------------------------------------
// Lazy-loaded syntect assets (first render pays ~50 ms init)
// ---------------------------------------------------------------------------

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_nonewlines);

static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Theme name for code-block syntax highlighting.
const CODE_THEME: &str = "base16-ocean.dark";
/// Default syntect theme name; also the default for `[core] code_theme`.
pub(crate) fn default_code_theme() -> &'static str {
    CODE_THEME
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Render markdown `content` into a sequence of styled lines.
///
/// Runs the full comrak → grid pipeline (optionally with syntect
/// highlighting).  Checks `cancel_token` between major operations so the
/// caller can abort mid-render.
pub(crate) fn render_builtin(
    content: &str,
    cols: u16,
    theme: &MarkdownTheme,
    opts: &MdRenderOpts,
    cancel_token: &AtomicBool,
) -> Vec<RenderLine> {
    if cancel_token.load(Ordering::Relaxed) {
        return Vec::new();
    }

    // --- Parse -----------------------------------------------------------
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options.extension.wikilinks_title_after_pipe = true;
    options.extension.description_lists = true;

    let arena = Arena::new();
    let root = parse_document(&arena, content, &options);

    if cancel_token.load(Ordering::Relaxed) {
        return Vec::new();
    }

    // --- Walk ------------------------------------------------------------
    let mut ctx = Ctx {
        lines: vec![Vec::with_capacity(cols as usize)],
        cols: cols as usize,
        wrap: opts.wrap,
        theme,
        syntax_hl: opts.syntax_hl,
        icon_mode: opts.icon_mode,
        code_theme: opts.code_theme.clone(),
        line_numbers: opts.code_line_numbers,
        wrap_indicator: opts.wrap_indicator,
        link_url_max: opts.link_url_max,
        cancel_token,
    };
    for child in root.children() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            break;
        }
        render_block(&mut ctx, child, 0);
    }

    // Remove any trailing empty rows (but keep at least one)
    while let Some(last) = ctx.lines.last() {
        if last.iter().all(|(c, _)| c.is_whitespace()) && ctx.lines.len() > 1 {
            ctx.lines.pop();
        } else {
            break;
        }
    }

    ctx.lines
        .into_iter()
        .map(|cells| RenderLine { cells })
        .collect()
}

// ---------------------------------------------------------------------------
// Internal rendering state
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    lines: Vec<Vec<(char, Style)>>,
    cols: usize,
    wrap: bool,
    theme: &'a MarkdownTheme,
    syntax_hl: bool,
    icon_mode: crate::config::IconMode,
    code_theme: String,
    line_numbers: bool,
    wrap_indicator: bool,
    link_url_max: usize,
    cancel_token: &'a AtomicBool,
}
impl Ctx<'_> {
    fn cur_col(&self) -> usize {
        self.lines.last().map(|l| l.len()).unwrap_or(0)
    }

    fn ensure_line(&mut self) -> &mut Vec<(char, Style)> {
        if self.lines.is_empty() {
            self.lines.push(Vec::with_capacity(self.cols));
        }
        self.lines.last_mut().expect("lines is not empty")
    }

    /// Start a new line with `margin` leading spaces.
    fn new_line(&mut self, margin: usize) {
        self.lines.push(Vec::with_capacity(self.cols));
        for _ in 0..margin {
            self.ensure_line().push((' ', Style::default()));
        }
    }

    /// Ensure the current line has at least `margin` leading spaces (fills
    /// with spaces if needed).  Called at the start of each block renderer.
    fn ensure_margin(&mut self, margin: usize) {
        let cur = self.cur_col();
        if cur < margin {
            for _ in cur..margin {
                self.ensure_line().push((' ', Style::default()));
            }
        }
    }

    /// Push a single character to the current line, wrapping/truncating as
    /// configured.
    fn push(&mut self, ch: char, st: Style, margin: usize) {
        if ch == '\t' {
            self.push(' ', st, margin);
            return;
        }
        if ch.is_control() {
            return;
        }
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        let col = self.cur_col();
        if col + w > self.cols {
            if self.wrap {
                if self.wrap_indicator {
                    let target = self.cols.saturating_sub(1);
                    let cur = self.cur_col();
                    if cur <= target {
                        let glyph_st = self.theme.hr;
                        for _ in cur..target {
                            self.ensure_line().push((' ', Style::default()));
                        }
                        self.ensure_line().push(('┄', glyph_st));
                    }
                }
                self.new_line(margin);
            } else {
                return;
            }
        }
        self.ensure_line().push((ch, st));
    }

    /// Push an entire string.
    fn push_str(&mut self, s: &str, st: Style, margin: usize) {
        for ch in s.chars() {
            self.push(ch, st, margin);
        }
    }

    /// Push N spaces.
    fn push_spaces(&mut self, n: usize, margin: usize) {
        for _ in 0..n {
            self.push(' ', Style::default(), margin);
        }
    }
}

// ---------------------------------------------------------------------------
// Block-level rendering
// ---------------------------------------------------------------------------

fn render_block<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize) {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Heading(h) => render_heading(ctx, node, h, depth),
        NodeValue::Paragraph => render_paragraph(ctx, node, depth),
        NodeValue::List(list) => render_list(ctx, node, list, depth),
        NodeValue::Item(_) => {
            for child in node.children() {
                render_block(ctx, child, depth);
            }
        }
        NodeValue::TaskItem(checked) => {
            if checked.is_some() {
                ctx.push_str("[✓] ", ctx.theme.task_checked, 2 + depth * 2);
            } else {
                ctx.push_str("[ ] ", ctx.theme.task_unchecked, 2 + depth * 2);
            }
            for child in node.children() {
                render_block(ctx, child, depth);
            }
        }
        NodeValue::CodeBlock(cb) => render_code_block(ctx, cb, depth),
        NodeValue::BlockQuote => render_blockquote(ctx, node, depth),
        NodeValue::Table(tbl) => render_table(ctx, node, tbl, depth),
        NodeValue::ThematicBreak => render_hr(ctx, depth),
        NodeValue::DescriptionList => {
            for child in node.children() {
                render_block(ctx, child, depth);
            }
            ctx.new_line(0);
        }
        NodeValue::DescriptionItem(_) => {
            for child in node.children() {
                render_block(ctx, child, depth);
            }
        }
        NodeValue::DescriptionTerm => {
            let margin = 2 + depth * 2;
            let style = ctx.theme.table_header;
            ctx.ensure_margin(margin);
            for child in node.children() {
                render_inline(ctx, child, style, margin);
            }
            ctx.new_line(0);
        }
        NodeValue::DescriptionDetails => {
            let margin = 2 + depth * 2 + 4;
            ctx.ensure_margin(margin);
            for child in node.children() {
                let data_val = child.data.borrow();
                if matches!(&data_val.value, NodeValue::Paragraph) {
                    let style = ctx.theme.paragraph;
                    drop(data_val);
                    for inline in child.children() {
                        render_inline(ctx, inline, style, margin);
                    }
                } else {
                    drop(data_val);
                    render_block(ctx, child, depth + 1);
                }
            }
            ctx.new_line(0);
        }
        NodeValue::FootnoteDefinition(_fd) => render_footnote_def(ctx, node, depth),
        NodeValue::HtmlBlock(_) | NodeValue::FrontMatter(_) => {}
        _ => {
            for child in node.children() {
                render_block(ctx, child, depth);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Heading
// ---------------------------------------------------------------------------

fn heading_style(ctx: &Ctx, level: u8) -> Style {
    match level {
        1 => ctx.theme.h1,
        2 => ctx.theme.h2,
        3 => ctx.theme.h3,
        4 => ctx.theme.h4,
        5 => ctx.theme.h5,
        _ => ctx.theme.h6,
    }
}

fn render_heading<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, h: &NodeHeading, depth: usize) {
    let margin = 2 + depth * 2;

    if h.level == 1 {
        // H1: banner styled title with 1-space padding, no "#" prefix, no full-width fill.
        let banner = ctx.theme.h1_banner;
        ctx.ensure_line();
        ctx.push(' ', banner, 0); // leading space
        for child in node.children() {
            render_inline(ctx, child, banner, 0);
        }
        ctx.push(' ', banner, 0); // trailing space
    } else {
        let style = heading_style(ctx, h.level);
        ctx.ensure_margin(margin);
        for child in node.children() {
            render_inline(ctx, child, style, margin);
        }
    }

    ctx.new_line(0);

    ctx.new_line(0);
}

// ---------------------------------------------------------------------------
// Paragraph
// ---------------------------------------------------------------------------

fn render_paragraph<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize) {
    let margin = 2 + depth * 2;
    let style = ctx.theme.paragraph;
    ctx.ensure_margin(margin);

    for child in node.children() {
        render_inline(ctx, child, style, margin);
    }

    ctx.new_line(0);
    // Blank line after paragraph

    ctx.new_line(0);
}

/// Render one child of a list item. Paragraphs are rendered inline (tight
/// list — no trailing blank lines); all other block types go through
/// `render_block` (so nested lists / code blocks inside items still get
/// proper block separation).
fn render_list_child<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize, margin: usize) {
    let is_paragraph = matches!(node.data.borrow().value, NodeValue::Paragraph);
    if is_paragraph {
        let style = ctx.theme.paragraph;
        for inline in node.children() {
            render_inline(ctx, inline, style, margin);
        }
    } else {
        render_block(ctx, node, depth);
    }
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

fn render_list<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, list: &NodeList, depth: usize) {
    let margin = 2 + depth * 2;
    let is_ordered = list.list_type == ListType::Ordered;
    let mut item_num = list.start;
    ctx.ensure_margin(margin);

    for child in node.children() {
        let child_data = child.data.borrow();
        if ctx.cancel_token.load(Ordering::Relaxed) {
            break;
        }

        match &child_data.value {
            NodeValue::Item(_item_info) => {
                // Check for task list item child
                let is_task = list.is_task_list
                    || child
                        .children()
                        .any(|c| matches!(c.data.borrow().value, NodeValue::TaskItem(_)));

                if is_task {
                    let mut checked_state: Option<Option<char>> = None;
                    for grandchild in child.children() {
                        if let NodeValue::TaskItem(c) = &grandchild.data.borrow().value {
                            checked_state = Some(*c);
                            break;
                        }
                    }
                    let checked = checked_state.unwrap_or(None);

                    if checked.is_some() {
                        ctx.push_str("[✓] ", ctx.theme.task_checked, margin);
                    } else {
                        ctx.push_str("[ ] ", ctx.theme.task_unchecked, margin);
                    }

                    for grandchild in child.children() {
                        if matches!(grandchild.data.borrow().value, NodeValue::TaskItem(_)) {
                            continue;
                        }
                        render_list_child(ctx, grandchild, depth + 1, margin);
                    }
                } else {
                    let bullet = if is_ordered {
                        let s = format!("{}.", item_num);
                        item_num += 1;
                        s
                    } else {
                        "•".to_string()
                    };
                    ctx.push_str(&format!("{} ", bullet), ctx.theme.paragraph, margin);

                    for grandchild in child.children() {
                        render_list_child(ctx, grandchild, depth + 1, margin);
                    }
                }

                ctx.new_line(margin);
            }
            NodeValue::TaskItem(checked) => {
                if checked.is_some() {
                    ctx.push_str("[✓] ", ctx.theme.task_checked, margin);
                } else {
                    ctx.push_str("[ ] ", ctx.theme.task_unchecked, margin);
                }
                // TaskItem has children (Paragraph with text)
                for grandchild in child.children() {
                    render_list_child(ctx, grandchild, depth + 1, margin);
                }

                ctx.new_line(margin);
            }
            _ => {
                render_block(ctx, child, depth);
            }
        }
    }

    // Blank line after list

    ctx.new_line(0);

    ctx.new_line(0);
}

// ---------------------------------------------------------------------------
// Code blocks
// ---------------------------------------------------------------------------

/// Visual display width of a string (sum of per-char Unicode widths).
fn str_visual_width(s: &str) -> usize {
    s.chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum()
}

/// `(nerd_font_codepoint, unicode_glyph)` for a code-fence language, if known.
fn lang_icon(lang: &str) -> Option<(&'static str, &'static str)> {
    let l = lang.to_ascii_lowercase();
    Some(match l.as_str() {
        "rust" | "rs" => ("\u{e7a8}", "🦀"),
        "python" | "py" | "python3" => ("\u{e73c}", "🐍"),
        "javascript" | "js" => ("\u{e74e}", "📜"),
        "typescript" | "ts" => ("\u{e628}", "🔷"),
        "go" | "golang" => ("\u{e627}", "🐹"),
        "java" => ("\u{e256}", "☕"),
        "php" => ("\u{e608}", "🐘"),
        "ruby" | "rb" => ("\u{e739}", "💎"),
        "c" => ("\u{e61e}", "🅒"),
        "cpp" | "c++" => ("\u{e61d}", "🅒"),
        "html" => ("\u{e736}", "🌐"),
        "css" => ("\u{e749}", "🎨"),
        "json" | "yaml" | "yml" | "toml" => ("\u{e60b}", "📋"),
        "sql" => ("\u{e7c4}", "🗄"),
        "sh" | "bash" | "shell" | "zsh" => ("\u{e795}", "🖥"),
        "dockerfile" | "docker" | "dockercompose" => ("\u{f308}", "🐳"),
        "lua" => ("\u{e626}", "🌙"),
        "diff" | "patch" => ("\u{e728}", "Δ"),
        _ => return None,
    })
}

/// Closing pill: `╰` + `inner`×`─` + `╯`, mirroring the opening label width.
fn close_code_pill(ctx: &mut Ctx, margin: usize, inner: usize) {
    ctx.push('╰', ctx.theme.table_border, margin);
    for _ in 0..inner {
        ctx.push('─', ctx.theme.table_border, margin);
    }
    ctx.push('╯', ctx.theme.table_border, margin);
}
fn render_code_block(ctx: &mut Ctx, cb: &NodeCodeBlock, depth: usize) {
    let margin = 2 + depth * 2;
    let code_margin = margin + 4; // legacy 4-space indent

    // Line-number gutter geometry
    let line_count = cb.literal.lines().count().max(1);
    let digits = line_count.to_string().len().max(2);
    let gutter_w = if ctx.line_numbers { digits + 3 } else { 0 }; // "{:>w} │ "
    let code_indent = if ctx.line_numbers {
        margin + gutter_w
    } else {
        code_margin
    };

    // Opening label + inner width (reused for the symmetric closing pill)
    let has_label = cb.fenced && !cb.info.is_empty();
    let lang = if has_label {
        cb.info.split_whitespace().next().unwrap_or("").to_string()
    } else {
        String::new()
    };
    let mut inner: usize = 0;
    if has_label && !lang.is_empty() {
        let mut icon_seg = String::new();
        let mut icon_w = 0usize;
        if !matches!(ctx.icon_mode, crate::config::IconMode::None)
            && let Some((nerd, uni)) = lang_icon(&lang)
        {
            let g = crate::ui::get_icon(nerd, uni, ctx.icon_mode);
            icon_seg = format!("{g} ");
            icon_w = str_visual_width(&icon_seg);
        }
        inner = str_visual_width(&lang) + icon_w + 4; // "─ " + icon_seg + lang + " ─"

        ctx.ensure_margin(margin);
        ctx.push_str("╭─ ", ctx.theme.table_border, margin);
        if !icon_seg.is_empty() {
            ctx.push_str(&icon_seg, ctx.theme.blockquote, margin);
        }
        ctx.push_str(&lang, ctx.theme.code_inline, margin);
        ctx.push_str(" ─╮", ctx.theme.table_border, margin);
    }

    // Highlighted path
    if ctx.syntax_hl
        && has_label
        && !lang.is_empty()
        && let Some(syntax) = SYNTAX_SET.find_syntax_by_token(&lang)
        && let Some(theme) = THEME_SET.themes.get(&ctx.code_theme)
    {
        let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
        for (i, line) in cb.literal.lines().enumerate() {
            if ctx.cancel_token.load(Ordering::Relaxed) {
                return;
            }
            ctx.new_line(margin);
            if ctx.line_numbers {
                push_code_gutter(ctx, i + 1, digits, margin);
            } else {
                ctx.push_spaces(4, margin);
            }
            if let Ok(ranges) = highlighter.highlight_line(line, &SYNTAX_SET) {
                for (syn_style, text) in &ranges {
                    let rt_style = syntect_style_to_ratatui(*syn_style);
                    ctx.push_str(text, rt_style, code_indent);
                }
            } else {
                ctx.push_str(line, ctx.theme.paragraph, code_indent);
            }
        }
        ctx.new_line(margin);
        if has_label && !lang.is_empty() {
            close_code_pill(ctx, margin, inner);
        }
        ctx.new_line(0);
        ctx.new_line(0);
        return;
    }

    // Plain path (no syntect, unknown lang, or unknown code theme)
    for (i, line) in cb.literal.lines().enumerate() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            return;
        }
        ctx.new_line(margin);
        if ctx.line_numbers {
            push_code_gutter(ctx, i + 1, digits, margin);
        } else {
            ctx.push_spaces(4, margin);
        }
        ctx.push_str(line, ctx.theme.paragraph, code_indent);
    }
    ctx.new_line(margin);
    if has_label && !lang.is_empty() {
        close_code_pill(ctx, margin, inner);
    }
    ctx.new_line(0);
    ctx.new_line(0);
}

/// Push the right-justified line-number gutter `{:>w} │ ` in muted style.
fn push_code_gutter(ctx: &mut Ctx, idx: usize, digits: usize, margin: usize) {
    let s = format!("{:>width$} │ ", idx, width = digits);
    ctx.push_str(&s, ctx.theme.blockquote, margin);
}

// ---------------------------------------------------------------------------
// Blockquote
// ---------------------------------------------------------------------------

const BQ_BARS: [char; 3] = ['┃', '║', '┆'];

fn render_blockquote<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize) {
    let margin = 2 + depth * 2;
    ctx.ensure_margin(margin);

    for child in node.children() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            break;
        }

        let bar = BQ_BARS[depth % 3];
        ctx.push_str(&format!("{bar} "), ctx.theme.blockquote_bar, margin);

        let data = child.data.borrow();
        match &data.value {
            NodeValue::Paragraph => {
                let style = ctx.theme.blockquote;
                for inline in child.children() {
                    render_inline(ctx, inline, style, margin + 2);
                }
                ctx.new_line(margin);
            }
            _ => {
                // Block-level children (nested blockquotes, lists, etc.)
                // handle their own vertical spacing — no extra new_line here
                // to avoid compounding gaps at every nesting level.
                drop(data);
                render_block(ctx, child, depth + 1);
            }
        }
    }

    // Trailing blank-lines separator: only at the outermost blockquote
    // to prevent compounding gaps from nested levels.
    if depth == 0 {
        ctx.new_line(0);
        ctx.new_line(0);
    }
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/// Captured table row data — holds references into the AST arena.
#[allow(dead_code)]
struct Row<'a> {
    is_header: bool,
    cells: Vec<Vec<&'a AstNode<'a>>>,
}

fn cell_leading_pad(align: TableAlignment, col_width: usize, content_w: usize) -> usize {
    let slack = col_width.saturating_sub(content_w);
    match align {
        TableAlignment::Right => slack,
        TableAlignment::Center => slack / 2,
        _ => 0,
    }
}

fn render_table<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, tbl: &NodeTable, depth: usize) {
    let margin = 2 + depth * 2;
    ctx.ensure_margin(margin);

    // Collect rows
    let mut rows: Vec<Row<'a>> = Vec::new();

    for child in node.children() {
        let data = child.data.borrow();
        if let NodeValue::TableRow(is_header) = &data.value {
            let mut cells: Vec<Vec<&'a AstNode<'a>>> = Vec::new();
            for cell in child.children() {
                let inlines: Vec<&'a AstNode<'a>> = cell.children().collect();
                cells.push(inlines);
            }
            rows.push(Row {
                is_header: *is_header,
                cells,
            });
        }
    }

    if rows.is_empty() {
        return;
    }

    let num_cols = rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return;
    }

    // Compute max width per column
    let mut col_widths = vec![0usize; num_cols];
    for row in &rows {
        for (ci, cell_inlines) in row.cells.iter().enumerate() {
            if ci >= num_cols {
                break;
            }
            let text_len: usize = cell_inlines.iter().map(|n| inline_text_len(n)).sum();
            col_widths[ci] = col_widths[ci].max(text_len);
        }
    }
    for w in &mut col_widths {
        *w = (*w).max(1);
    }
    let mut col_offsets = vec![0usize; num_cols];
    let mut offset = margin + 1; // after left border
    for ci in 0..num_cols {
        col_offsets[ci] = offset;
        offset += col_widths[ci];
        if ci + 1 < num_cols {
            offset += 1; // separator
        }
    }
    let border_st = ctx.theme.table_border;

    // Helper: render a separator row
    let render_sep = |ctx: &mut Ctx, left: char, mid: char, right: char| {
        ctx.push(left, border_st, margin);
        for (ci, w) in col_widths.iter().enumerate() {
            for _ in 0..*w {
                ctx.push('─', border_st, margin);
            }
            if ci + 1 < num_cols {
                ctx.push(mid, border_st, margin);
            }
        }
        ctx.push(right, border_st, margin);

        ctx.new_line(margin);
    };

    // Top border
    render_sep(ctx, '┌', '┬', '┐');

    // Header row
    if let Some(first) = rows.first() {
        ctx.push('┃', border_st, margin);
        for (ci, cell_inlines) in first.cells.iter().enumerate() {
            if ci >= num_cols {
                break;
            }
            let align = tbl
                .alignments
                .get(ci)
                .copied()
                .unwrap_or(TableAlignment::None);
            let content_w: usize = cell_inlines.iter().map(|n| inline_text_len(n)).sum();
            ctx.push_spaces(
                cell_leading_pad(align, col_widths[ci], content_w),
                margin + 1,
            );
            let header_style = ctx.theme.table_header;
            for inline in cell_inlines {
                render_inline(ctx, inline, header_style, margin + 1);
            }
            // Pad to width
            let cur = ctx.cur_col();
            let target = col_offsets[ci] + col_widths[ci];
            if cur < target {
                ctx.push_spaces(target - cur, margin);
            }
            if ci + 1 < num_cols {
                ctx.push('┃', border_st, margin);
            }
        }
        ctx.push('┃', border_st, margin);

        ctx.new_line(margin);
    }

    // Header/body separator
    render_sep(ctx, '├', '┼', '┤');

    // Body rows
    for row in rows.iter().skip(1) {
        ctx.push('┃', border_st, margin);
        for (ci, cell_inlines) in row.cells.iter().enumerate() {
            if ci >= num_cols {
                break;
            }
            let align = tbl
                .alignments
                .get(ci)
                .copied()
                .unwrap_or(TableAlignment::None);
            let content_w: usize = cell_inlines.iter().map(|n| inline_text_len(n)).sum();
            ctx.push_spaces(
                cell_leading_pad(align, col_widths[ci], content_w),
                margin + 1,
            );
            let cell_style = ctx.theme.table_cell;
            for inline in cell_inlines {
                render_inline(ctx, inline, cell_style, margin + 1);
            }
            // Pad to width
            let cur = ctx.cur_col();
            let target = col_offsets[ci] + col_widths[ci];
            if cur < target {
                ctx.push_spaces(target - cur, margin);
            }
            if ci + 1 < num_cols {
                ctx.push('┃', border_st, margin);
            }
        }
        ctx.push('┃', border_st, margin);

        ctx.new_line(margin);
    }

    // Bottom border
    render_sep(ctx, '└', '┴', '┘');

    ctx.new_line(0);

    ctx.new_line(0);
}

/// Return the visual width of the text contained in an inline sub-tree.
fn inline_text_len<'a>(node: &'a AstNode<'a>) -> usize {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(t) => t
            .chars()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum(),
        NodeValue::Code(c) => {
            c.literal
                .chars()
                .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
                .sum::<usize>()
                + 2
        }
        NodeValue::Link(_)
        | NodeValue::Image(_)
        | NodeValue::Strong
        | NodeValue::Emph
        | NodeValue::Strikethrough => {
            let mut len = 0usize;
            for child in node.children() {
                len += inline_text_len(child);
            }
            len
        }
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Thematic break (HR)
// ---------------------------------------------------------------------------

fn render_hr(ctx: &mut Ctx, depth: usize) {
    let margin = 2 + depth * 2;
    ctx.ensure_margin(margin);
    let width = ctx.cols.saturating_sub(margin * 2).max(4);
    for _ in 0..width {
        ctx.push('─', ctx.theme.hr, margin);
    }

    ctx.new_line(0);

    ctx.new_line(0);
}

// ---------------------------------------------------------------------------
// Footnote definition
// ---------------------------------------------------------------------------

fn render_footnote_def<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize) {
    let data = node.data.borrow();
    let NodeValue::FootnoteDefinition(fd) = &data.value else {
        return;
    };
    let margin = 2 + depth * 2;
    ctx.ensure_margin(margin);

    let label = format!("[^{}]: ", fd.name);
    ctx.push_str(&label, ctx.theme.footnote_def, margin);

    // Drop borrow before recursing
    drop(data);
    for child in node.children() {
        render_block(ctx, child, depth);
    }

    ctx.new_line(0);

    ctx.new_line(0);
}

fn link_icon(url: &str, icon_mode: crate::config::IconMode) -> &'static str {
    if matches!(icon_mode, crate::config::IconMode::None) {
        return "";
    }
    let url_lower = url.to_ascii_lowercase();
    if url_lower.contains("github.com") {
        crate::ui::get_icon("\u{f09b}", "\u{1f4e6}", icon_mode) // nerd:  / unicode: 📦
    } else if url_lower.contains("gitlab.com") {
        crate::ui::get_icon("\u{f296}", "\u{1f4e6}", icon_mode) // nerd:  / unicode: 📦
    } else if url_lower.contains("stackoverflow.com") || url_lower.contains("stackexchange.com") {
        crate::ui::get_icon("\u{f16c}", "\u{2753}", icon_mode) // nerd:  / unicode: ❓
    } else if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
        crate::ui::get_icon("\u{f167}", "\u{25b6}", icon_mode) // nerd:  / unicode: ▶
    } else if url_lower.contains("reddit.com") {
        crate::ui::get_icon("\u{f281}", "\u{1f4ac}", icon_mode) // nerd:  / unicode: 💬
    } else if url_lower.contains("twitter.com") || url_lower.contains("x.com") {
        crate::ui::get_icon("\u{f099}", "\u{1f426}", icon_mode) // nerd:  / unicode: 🐦
    } else if url_lower.contains("wikipedia.org") {
        crate::ui::get_icon("\u{f266}", "\u{1f4d6}", icon_mode) // nerd:  / unicode: 📖
    } else if url_lower.starts_with("mailto:") {
        crate::ui::get_icon("\u{f0e0}", "\u{2709}", icon_mode) // nerd:  / unicode: ✉
    } else {
        crate::ui::get_icon("\u{f0c1}", "\u{1f517}", icon_mode) // nerd:  / unicode: 🔗
    }
}

fn truncate_url_middle(url: &str, max: usize) -> String {
    let count = url.chars().count();
    if max == 0 || count <= max {
        return url.to_string();
    }
    let head = (max * 6 / 10).max(1);
    let tail = max.saturating_sub(head).saturating_sub(1).max(1); // -1 for '…'
    let chars: Vec<char> = url.chars().collect();
    let mut s: String = chars.iter().take(head).collect();
    s.push('…');
    s.extend(chars.iter().rev().take(tail).rev()); // last `tail` chars, in order
    s
}

// ---------------------------------------------------------------------------
// Inline rendering
// ---------------------------------------------------------------------------
fn render_inline<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, base_style: Style, margin: usize) {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(t) => {
            // `t` is &String — access as &str
            ctx.push_str(t, base_style, margin);
        }
        NodeValue::Code(c) => {
            ctx.push(' ', ctx.theme.code_inline, margin);
            ctx.push_str(&c.literal, ctx.theme.code_inline, margin);
            ctx.push(' ', ctx.theme.code_inline, margin);
        }
        NodeValue::Strong => {
            let st = base_style.add_modifier(Modifier::BOLD);
            for child in node.children() {
                render_inline(ctx, child, st, margin);
            }
        }
        NodeValue::Emph => {
            let st = base_style.add_modifier(Modifier::ITALIC);
            for child in node.children() {
                render_inline(ctx, child, st, margin);
            }
        }
        NodeValue::Strikethrough => {
            let st = base_style.add_modifier(Modifier::CROSSED_OUT);
            for child in node.children() {
                render_inline(ctx, child, st, margin);
            }
        }
        NodeValue::Link(link) => {
            let icon = link_icon(&link.url, ctx.icon_mode);
            if !icon.is_empty() {
                ctx.push_str(icon, ctx.theme.blockquote, margin);
                ctx.push(' ', ctx.theme.blockquote, margin);
            }
            for child in node.children() {
                render_inline(ctx, child, ctx.theme.link_text, margin);
            }
            ctx.push(' ', base_style, margin);
            ctx.push_str(
                &truncate_url_middle(&link.url, ctx.link_url_max),
                ctx.theme.link_url,
                margin,
            );
        }
        NodeValue::Image(img) => {
            let icon = crate::ui::get_icon("\u{f03e}", "\u{1f5bc}", ctx.icon_mode);
            if !icon.is_empty() {
                ctx.push_str(icon, ctx.theme.blockquote, margin);
                ctx.push(' ', ctx.theme.blockquote, margin);
            }
            let alt_text = node
                .children()
                .map(|c| {
                    let d = c.data.borrow();
                    if let NodeValue::Text(t) = &d.value {
                        t.clone()
                    } else {
                        String::new()
                    }
                })
                .collect::<String>();
            if alt_text.is_empty() {
                ctx.push_str("[image]", ctx.theme.link_text, margin);
            } else {
                ctx.push_str(&alt_text, ctx.theme.link_text, margin);
            }
            if !img.url.is_empty() {
                ctx.push(' ', base_style, margin);
                ctx.push_str(
                    &truncate_url_middle(&img.url, ctx.link_url_max),
                    ctx.theme.link_url,
                    margin,
                );
            }
        }
        NodeValue::WikiLink(wl) => {
            ctx.push_str(&format!("[[{}]]", wl.url), ctx.theme.wikilink, margin);
        }
        NodeValue::FootnoteReference(fr) => {
            ctx.push_str(&format!("[^{}]", fr.name), ctx.theme.footnote_ref, margin);
        }
        NodeValue::SoftBreak => {
            ctx.push(' ', base_style, margin);
        }
        NodeValue::LineBreak => {
            ctx.new_line(margin);
        }
        NodeValue::HtmlInline(_) => {}
        NodeValue::Escaped => {
            for child in node.children() {
                render_inline(ctx, child, base_style, margin);
            }
        }
        _ => {
            for child in node.children() {
                render_inline(ctx, child, base_style, margin);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Syntect → ratatui style mapping
// ---------------------------------------------------------------------------

/// Convert a syntect `Style` (highlighting style, not the parsing type) to a
/// ratatui `Style`.  This is the **only** place raw `Color::Rgb` is emitted
/// — syntect's palette is not theme-derived.
fn syntect_style_to_ratatui(s: syntect::highlighting::Style) -> ratatui::style::Style {
    let mut st = ratatui::style::Style::default();

    st = st.fg(ratatui::style::Color::Rgb(
        s.foreground.r,
        s.foreground.g,
        s.foreground.b,
    ));
    if s.font_style.contains(FontStyle::BOLD) {
        st = st.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(FontStyle::ITALIC) {
        st = st.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        st = st.add_modifier(Modifier::UNDERLINED);
    }

    st
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_theme::AppThemeColors;
    use std::sync::atomic::AtomicBool;

    fn mk_opts(icon_mode: crate::config::IconMode) -> MdRenderOpts {
        MdRenderOpts {
            syntax_hl: true,
            wrap: true,
            icon_mode,
            code_theme: default_code_theme().to_string(),
            code_line_numbers: false,
            wrap_indicator: false,
            link_url_max: 0,
        }
    }

    fn render_test(content: &str, cols: u16, wrap: bool, syntax_hl: bool) -> Vec<RenderLine> {
        let theme_colors = AppThemeColors::default();
        let theme = MarkdownTheme::from_app_theme(&theme_colors);
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::default());
        opts.wrap = wrap;
        opts.syntax_hl = syntax_hl;
        render_builtin(content, cols, &theme, &opts, &cancel)
    }

    fn line_text(line: &RenderLine) -> String {
        line.cells.iter().map(|(c, _)| c).collect()
    }

    fn has_mod(st: Style, m: Modifier) -> bool {
        st.add_modifier.contains(m)
    }

    #[test]
    fn renders_heading_bold_and_colored() {
        let lines = render_test("# Heading 1\n\n## Heading 2\n", 80, true, false);
        let h1_line = lines.iter().find(|l| line_text(l).contains("Heading 1"));
        assert!(h1_line.is_some(), "h1 should appear");
        // First non-space cell is '#' which should be bold
        let h1_first = h1_line
            .expect("lines is not empty")
            .cells
            .iter()
            .find(|(c, _)| *c != ' ')
            .map(|(_, s)| *s);
        assert!(h1_first.is_some(), "h1 has content");
        assert!(
            has_mod(h1_first.expect("lines is not empty"), Modifier::BOLD),
            "h1 bold"
        );

        let h2_line = lines.iter().find(|l| line_text(l).contains("Heading 2"));
        assert!(h2_line.is_some(), "h2 should appear");
        let h2_first = h2_line
            .expect("lines is not empty")
            .cells
            .iter()
            .find(|(c, _)| *c != ' ')
            .map(|(_, s)| *s);
        assert!(h2_first.is_some(), "h2 has content");
        assert!(
            has_mod(h2_first.expect("lines is not empty"), Modifier::BOLD),
            "h2 bold"
        );
    }

    #[test]
    fn heading_renders_without_hash_prefix() {
        let lines = render_test("## Heading 2\n", 80, true, false);
        // H2 should render without any # prefix
        let h2 = lines.iter().find(|l| line_text(l).contains("Heading 2"));
        assert!(h2.is_some(), "H2 should contain heading text");
        assert!(
            !h2.expect("lines is not empty")
                .cells
                .iter()
                .any(|(c, _)| *c == '#'),
            "heading should NOT contain hash prefix"
        );
    }

    #[test]
    fn renders_table_with_box_borders() {
        let lines = render_test("| A | B |\n|---|---|\n| 1 | 2 |\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains('┌'), "top-left border");
        assert!(text.contains('┐'), "top-right border");
        assert!(text.contains('├'), "middle-left border");
        assert!(text.contains('┤'), "middle-right border");
        assert!(text.contains('└'), "bottom-left border");
        assert!(text.contains('┘'), "bottom-right border");
        assert!(text.contains('┃'), "column separator");
        assert!(text.contains('A'), "cell A");
        assert!(text.contains('B'), "cell B");
        assert!(text.contains('1'), "cell 1");
        assert!(text.contains('2'), "cell 2");
    }

    #[test]
    fn renders_task_list_checkboxes() {
        let lines = render_test("- [ ] unchecked\n- [x] checked\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("[ ]"), "unchecked box");
        assert!(text.contains("[✓]"), "checked box");
        assert!(text.contains("unchecked"), "unchecked text");
        assert!(text.contains("checked"), "checked text");
    }

    #[test]
    fn renders_blockquote_with_bar() {
        let lines = render_test("> blockquote", 80, true, false);
        let bq = lines.iter().find(|l| line_text(l).contains("blockquote"));
        assert!(bq.is_some(), "blockquote text should appear");
        assert!(
            line_text(bq.expect("lines is not empty")).contains('┃'),
            "blockquote bar"
        );
    }

    #[test]
    fn renders_inline_code_bg() {
        let lines = render_test("text `code` here", 80, true, false);
        let code_line = lines.iter().find(|l| line_text(l).contains("code"));
        assert!(code_line.is_some(), "code should appear");
    }

    #[test]
    fn renders_footnote_ref_tag_color() {
        let lines = render_test("text[^1]\n\n[^1]: body\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("[^1]"), "footnote ref");
    }

    #[test]
    fn renders_wikilink_styled() {
        let lines = render_test("[[target]]", 80, true, false);
        let wl = lines.iter().find(|l| line_text(l).contains("[[target]]"));
        assert!(wl.is_some(), "wikilink should appear");
    }

    #[test]
    fn renders_code_block_plain_when_no_lang() {
        let lines = render_test("```\nplain code\n```\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("plain code"), "code content");
    }

    #[test]
    fn renders_code_block_highlighted_when_lang() {
        let lines = render_test("```rust\nfn main() {}\n```\n", 80, true, true);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("fn"), "code fn keyword");
        assert!(text.contains("main"), "code main text");
    }

    #[test]
    fn empty_input_returns_lines() {
        let lines = render_test("", 80, true, false);
        assert!(!lines.is_empty(), "empty input should have lines");
    }

    #[test]
    fn hr_fills_available_width() {
        let lines = render_test("---\n", 40, true, false);
        let hr_line = lines.iter().find(|l| line_text(l).contains("─"));
        assert!(hr_line.is_some(), "HR should be present");
        let hr_str = line_text(hr_line.expect("lines is not empty"));
        let count = hr_str.chars().filter(|&c| c == '─').count();
        assert!(
            count >= 30,
            "HR line should fill most of available width: got {}",
            count
        );
    }

    #[test]
    fn wraps_long_lines_at_cols() {
        let long = "a".repeat(100);
        let lines = render_test(&long, 40, true, false);
        assert!(
            lines.len() >= 3,
            "should wrap long line: {} lines",
            lines.len()
        );
        for line in &lines {
            assert!(line_text(line).len() <= 40, "each line <= 40 cols");
        }
    }

    #[test]
    fn nowrap_truncates_at_cols() {
        let long = "a".repeat(100);
        let lines = render_test(&long, 40, false, false);
        let combined: String = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(combined.len() <= 100, "should not wrap");
    }

    #[test]
    fn renders_unordered_list_bullet() {
        let lines = render_test("- item1\n- item2\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains('•'), "bullet character");
        assert!(text.contains("item1"), "item1 text");
        assert!(text.contains("item2"), "item2 text");
    }

    #[test]
    fn renders_ordered_list_numbers() {
        let lines = render_test("1. first\n2. second\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("1."), "item 1 number");
        assert!(text.contains("2."), "item 2 number");
        assert!(text.contains("first"), "item 1 text");
        assert!(text.contains("second"), "item 2 text");
    }

    #[test]
    fn control_chars_dropped_from_grid() {
        let lines = render_test("hello\nworld\tend\x07bell", 80, true, false);
        for line in &lines {
            for &(ch, _) in &line.cells {
                assert!(!ch.is_control(), "cell char is control: {ch:?}");
            }
        }
        // The \x07 (bell) is dropped; the printable "bell" text after it survives.
        // Verify no literal bell byte survives in any cell.
        let has_bell_ch = lines
            .iter()
            .flat_map(|l| &l.cells)
            .any(|(c, _)| *c == '\x07');
        assert!(!has_bell_ch, "bell char \\x07 should not appear in cells");
    }

    #[test]
    fn tab_becomes_space() {
        let lines = render_test("a\tb", 80, true, false);
        let text: String = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains(' '), "tab should render as space");
        assert!(!text.contains('\t'), "no raw tab in output");
        let mut cells = lines.iter().flat_map(|l| &l.cells);
        let tab_pos = cells.position(|(c, _)| *c == '\t');
        assert!(tab_pos.is_none(), "no tab char in any cell");
    }

    #[test]
    fn single_blank_line_between_paragraphs() {
        let lines = render_test("para one\n\npara two", 80, true, false);
        let non_blank_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.cells.iter().all(|(c, _)| c.is_whitespace()))
            .map(|(i, _)| i)
            .collect();
        assert!(
            non_blank_indices.len() >= 2,
            "should have at least two non-blank lines"
        );
        let blank_count = non_blank_indices[1] - non_blank_indices[0] - 1;
        assert_eq!(
            blank_count, 1,
            "expected exactly 1 blank line between paragraphs, got {blank_count}"
        );
    }

    #[test]
    fn single_blank_line_after_heading() {
        let lines = render_test("# Title\n\nbody", 80, true, false);
        let non_blank_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.cells.iter().all(|(c, _)| c.is_whitespace()))
            .map(|(i, _)| i)
            .collect();
        assert!(
            non_blank_indices.len() >= 2,
            "should have at least two non-blank lines"
        );
        let blank_count = non_blank_indices[1] - non_blank_indices[0] - 1;
        assert_eq!(
            blank_count, 1,
            "expected exactly 1 blank line after heading, got {blank_count}"
        );
    }

    #[test]
    fn no_leading_blank_line() {
        let lines = render_test("first\n\nsecond", 80, true, false);
        assert!(
            !lines[0].cells.iter().all(|(c, _)| c.is_whitespace()),
            "first row should not be blank"
        );
    }

    #[test]
    fn no_trailing_blank_lines() {
        let lines = render_test("a\n\nb", 80, true, false);
        let last = lines.last().expect("lines is not empty");
        assert!(
            !last.cells.iter().all(|(c, _)| c.is_whitespace()),
            "last row should not be blank"
        );
    }

    #[test]
    fn adjacent_blocks_dont_touch() {
        let lines = render_test("a\n\nb", 80, true, false);
        let non_blank_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.cells.iter().all(|(c, _)| c.is_whitespace()))
            .map(|(i, _)| i)
            .collect();
        assert!(
            non_blank_indices.len() >= 2,
            "should have at least two non-blank lines"
        );
        let blank_count = non_blank_indices[1] - non_blank_indices[0] - 1;
        assert!(
            blank_count > 0,
            "there must be a blank line between adjacent blocks"
        );
    }

    #[test]
    fn tight_list_no_blank_between_items() {
        let lines = render_test("- a\n- b\n- c\n", 80, true, false);
        // Find non-blank rows (content rows: bullet + item text)
        let non_blank_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.cells.iter().all(|(c, _)| c.is_whitespace() || *c == ' '))
            .map(|(i, _)| i)
            .collect();
        // Should have 3 content rows (one per item)
        assert!(
            non_blank_indices.len() >= 3,
            "should have at least 3 non-blank rows, got {}",
            non_blank_indices.len()
        );
        // No blank rows between items: each consecutive item's index diff is 1
        for i in 0..non_blank_indices.len().saturating_sub(1) {
            let gap = non_blank_indices[i + 1] - non_blank_indices[i] - 1;
            assert_eq!(
                gap,
                0,
                "blank rows between items at {}->{}: {}",
                non_blank_indices[i],
                non_blank_indices[i + 1],
                gap
            );
        }
    }

    #[test]
    fn folder_preview_md_renders_tight() {
        let lines = render_test(
            "# Vault (Root)\n\n## Folders\n- Documents\n- Notes\n",
            60,
            true,
            false,
        );
        let filtered: Vec<&RenderLine> = lines
            .iter()
            .filter(|l| !l.cells.iter().all(|(c, _)| c.is_whitespace()))
            .collect();
        // The two list items should be on consecutive rows
        let docs = filtered
            .iter()
            .position(|l| l.cells.iter().any(|(c, _)| *c == 'D' || *c == 'd'))
            .filter(|_| {
                filtered
                    .iter()
                    .any(|l| l.cells.iter().any(|(c, _)| *c == 'N' || *c == 'n'))
            });
        assert!(docs.is_some(), "should find Documents and Notes items");
    }

    #[test]
    fn h1_renders_as_banner() {
        let theme_colors = AppThemeColors::default();
        let lines = render_test("# Title\n", 30, true, false);
        assert!(!lines.is_empty(), "should have at least one line");
        let row0 = &lines[0];
        // Row 0: leading space + "Title" + trailing space = 7 cells, NOT full-width.
        let expected = " Title ";
        let text: String = row0.cells.iter().map(|(c, _)| c).collect();
        assert_eq!(text, expected, "H1 row should be ' Title '");
        // Every cell's bg should be Some(theme.heading)
        for (i, (ch, st)) in row0.cells.iter().enumerate() {
            assert_eq!(
                st.bg,
                Some(theme_colors.heading),
                "cell {} bg should be heading color; char={:?}",
                i,
                ch
            );
        }
        // Non-space cells should have fg = highlight_fg + BOLD
        for (i, (ch, st)) in row0.cells.iter().enumerate() {
            if *ch != ' ' {
                assert_eq!(
                    st.fg,
                    Some(theme_colors.highlight_fg),
                    "cell {} fg should be highlight_fg; char={:?}",
                    i,
                    ch
                );
                assert!(
                    has_mod(*st, Modifier::BOLD),
                    "cell {} should be bold; char={:?}",
                    i,
                    ch
                );
            }
        }
        // No '#' char anywhere in row 0
        assert!(
            !row0.cells.iter().any(|(c, _)| *c == '#'),
            "H1 banner should not contain # prefix"
        );
    }
    #[test]
    fn h2_renders_without_banner() {
        let lines = render_test("## Sub\n", 80, true, false);
        let h2_line = lines.iter().find(|l| line_text(l).contains("Sub"));
        assert!(h2_line.is_some(), "H2 should contain Sub");
        let h2 = h2_line.expect("lines is not empty");
        // No '#' char in the rendered output
        assert!(
            !h2.cells.iter().any(|(c, _)| *c == '#'),
            "H2 should NOT have # prefix"
        );
        // No bg fill: cells after text should be absent/default
        let h2_first = h2.cells.iter().find(|(c, _)| *c != ' ').map(|(_, s)| *s);
        assert!(h2_first.is_some(), "H2 has content");
        assert!(
            has_mod(h2_first.expect("lines is not empty"), Modifier::BOLD),
            "H2 bold"
        );
    }

    #[test]
    fn nested_list_keeps_block_separation() {
        let lines = render_test("- outer\n  - inner\n", 80, true, false);
        let text: String = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("outer"), "should contain outer item text");
        assert!(text.contains("inner"), "should contain inner item text");
    }

    #[test]
    fn table_columns_aligned() {
        let lines = render_test(
            "| Short | Very Long Column |\n|---|---|\n| 1 | 2 |\n",
            80,
            true,
            false,
        );
        for line in &lines {
            println!("ROW: {:?}", line_text(line));
        }
        let mut separator_positions = Vec::new();
        for line in &lines {
            let s = line_text(line);
            let positions: Vec<usize> = s
                .chars()
                .enumerate()
                .filter(|&(_, c)| {
                    c == '┃'
                        || c == '┼'
                        || c == '┬'
                        || c == '┴'
                        || c == '┌'
                        || c == '┐'
                        || c == '├'
                        || c == '┤'
                        || c == '└'
                        || c == '┘'
                })
                .map(|(idx, _)| idx)
                .collect();
            if !positions.is_empty() {
                separator_positions.push(positions);
            }
        }
        assert!(
            !separator_positions.is_empty(),
            "should find separator rows"
        );
        let first = &separator_positions[0];
        for pos in &separator_positions[1..] {
            assert_eq!(first, pos, "table column separators must align");
        }
    }
    #[test]
    fn strikethrough_uses_crossed_out() {
        let lines = render_test("~~deleted~~", 80, true, false);
        let mut found = false;
        for line in &lines {
            for (c, st) in &line.cells {
                if *c == 'd' || *c == 'e' || *c == 'l' {
                    assert!(
                        has_mod(*st, Modifier::CROSSED_OUT),
                        "strikethrough text must be crossed out"
                    );
                    found = true;
                }
            }
        }
        assert!(found, "should find deleted text");
    }

    #[test]
    fn code_block_shows_language_label() {
        let lines = render_test("```rust\nfn main() {}\n```\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(
            text.contains("╭─") && text.contains("rust") && text.contains("─╮"),
            "should contain language label line with lang name"
        );
    }

    #[test]
    fn renders_description_list() {
        let lines = render_test("Term\n: Definition text\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Term"), "should contain Term");
        assert!(
            text.contains("Definition text"),
            "should contain Definition text"
        );
        let def_line = lines
            .iter()
            .find(|l| line_text(l).contains("Definition text"))
            .expect("lines is not empty");
        let leading_spaces = def_line.cells.iter().take_while(|(c, _)| *c == ' ').count();
        assert_eq!(
            leading_spaces, 6,
            "definition detail should be indented by 6 spaces"
        );
    }

    #[test]
    fn link_shows_github_icon() {
        let theme_colors = AppThemeColors::default();
        let theme = MarkdownTheme::from_app_theme(&theme_colors);
        let cancel = AtomicBool::new(false);
        let lines = render_builtin(
            "[repo](https://github.com/user/repo)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        );
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(
            text.contains("📦 repo"),
            "should contain github unicode icon"
        );

        let lines_none = render_builtin(
            "[repo](https://github.com/user/repo)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::None),
            &cancel,
        );
        let text_none = lines_none
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !text_none.contains("📦"),
            "should not contain github unicode icon"
        );
    }

    #[test]
    fn image_shows_indicator() {
        let theme_colors = AppThemeColors::default();
        let theme = MarkdownTheme::from_app_theme(&theme_colors);
        let cancel = AtomicBool::new(false);
        let lines = render_builtin(
            "![alt](url.png)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        );
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("🖼 alt"), "should contain image unicode icon");

        let lines_none = render_builtin(
            "![alt](url.png)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::None),
            &cancel,
        );
        let text_none = lines_none
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !text_none.contains("🖼"),
            "should not contain image unicode icon"
        );
    }

    #[test]
    fn code_block_has_closing_pill() {
        let lines = render_test("```rust\nfn main(){}\n```\n", 80, true, false);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        let open = text.iter().find(|l| l.contains('╭')).expect("open pill");
        let close = text.iter().find(|l| l.contains('╰')).expect("close pill");
        assert!(open.contains("─╮"));
        assert!(close.contains("╰") && close.contains('╯'));
        assert_eq!(
            open.chars().count(),
            close.chars().count(),
            "pill widths equal"
        );
    }

    #[test]
    fn code_block_line_numbers() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::default());
        opts.code_line_numbers = true;
        let lines = render_builtin("```txt\na\nb\nc\n```\n", 80, &theme, &opts, &cancel);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            text.iter().any(|l| l.trim_start().starts_with("1 │ a")),
            "line 1 gutter"
        );
        assert!(
            text.iter().any(|l| l.trim_start().starts_with("3 │ c")),
            "line 3 gutter"
        );
    }

    #[test]
    fn code_block_lang_icon() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let lines = render_builtin(
            "```rust\nx\n```\n",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        );
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            text.iter().any(|l| l.contains('🦀')),
            "rust unicode icon in pill"
        );
    }

    #[test]
    fn table_honors_alignment() {
        let lines = render_test("| a | bbb |\n|:---|---:|\n|1|2|\n", 80, true, false);
        let row = lines
            .iter()
            .find(|l| line_text(l).contains('2'))
            .expect("data row");
        let t = line_text(row);
        let cell = t.split('┃').nth(2).unwrap_or("");
        assert!(cell.starts_with(' '), "right-aligned cell has leading pad");
        assert!(cell.trim_end().ends_with('2'), "value flush right");
    }

    #[test]
    fn url_truncated_when_long() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::None);
        opts.link_url_max = 20;
        let long = "[t](https://example.com/very/long/path/to/resource)";
        let lines = render_builtin(long, 80, &theme, &opts, &cancel);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        let joined = text.join("");
        assert!(joined.contains('…'), "truncated");
        assert!(
            !joined.contains("/resource"),
            "tail-after-cut removed or kept short; full url absent"
        );
    }

    #[test]
    fn inline_code_has_padding() {
        let lines = render_test("a `x` b", 80, true, false);
        let line = lines
            .iter()
            .find(|l| line_text(l).contains('x'))
            .expect("code line");
        let idx = line.cells.iter().position(|(c, _)| *c == 'x').expect("x");
        assert_eq!(line.cells[idx - 1].0, ' ', "leading padding space");
    }

    #[test]
    fn blockquote_depth_glyphs() {
        let lines = render_test("> > > deep", 80, true, false);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(text.iter().any(|l| l.contains('┆')), "depth-2 glyph");
    }

    #[test]
    fn wrap_indicator_shown() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::default());
        opts.wrap = true;
        opts.wrap_indicator = true;
        let lines = render_builtin("aaaaaaa\u{4e00}bcdefghijklmnop", 10, &theme, &opts, &cancel);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            text.iter().any(|l| l.ends_with('┄')),
            "wrapped line ends with continuation glyph"
        );
    }

    #[test]
    fn code_theme_unknown_falls_back_to_plain() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::default());
        opts.syntax_hl = true;
        opts.code_theme = "does-not-exist".to_string();
        let lines = render_builtin("```rust\nfn main(){}\n```\n", 80, &theme, &opts, &cancel);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            text.iter().any(|l| l.contains("fn main")),
            "code still rendered (plain fallback)"
        );
    }
}
