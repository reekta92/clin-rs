//! Built-in markdown layout engine using comrak (GFM parse) + syntect
//! (optional code-block highlighting).
//!
//! The public entry point is [`render_layout`], which returns a
//! `RenderedDocument` matching glow's structural layout (2-space margin,
//! box-drawing tables, `•` bullets, `┃` blockquote bars, `[ ]`/`[✓]` task
//! boxes, preserved `#` heading prefixes, 8-dash HR).
//!
//! Styling goes **beyond** glow: headings are coloured per hierarchy, code
//! blocks get theme-bg shading, inline code gets bg/fg, and fenced code
//! blocks with a known language get full syntect syntax highlighting.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};

use comrak::nodes::{
    AstNode, ListType, NodeCodeBlock, NodeHeading, NodeList, NodeTable, NodeValue, TableAlignment,
};
use comrak::{Arena, Options, parse_document};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::markdown::MdRenderOpts;
use crate::markdown::style::{
    MarkdownTheme, RenderLine, RenderedDocument, StyledSpan, faint_background,
};

pub(crate) struct PendingCodeBlock {
    pub id: u32,
    pub literal: Arc<str>,
    pub literal_fingerprint: u64,
    pub language: Arc<str>,
    pub depth: usize,
    pub first_code_source_line: usize,
    pub line_range: Range<usize>,
}

pub(crate) struct LayoutResult {
    pub document: RenderedDocument,
    pub code_blocks: Vec<PendingCodeBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HighlightedSpan {
    pub text: String,
    pub style: ratatui::style::Style,
}

pub(crate) type HighlightedBlock = Vec<Option<Vec<HighlightedSpan>>>;
// ---------------------------------------------------------------------------
// Lazy-loaded syntect assets (first render pays ~50 ms init)
// ---------------------------------------------------------------------------

pub(crate) static SYNTAX_SET: LazyLock<SyntaxSet> =
    LazyLock::new(SyntaxSet::load_defaults_nonewlines);

pub(crate) static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

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
pub(crate) fn render_layout(
    content: &str,
    cols: u16,
    theme: &MarkdownTheme,
    opts: &MdRenderOpts,
    cancel: &AtomicBool,
) -> Option<LayoutResult> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }

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

    if cancel.load(Ordering::Relaxed) {
        return None;
    }

    let mut ctx = Ctx {
        source_lines: content.split('\n').collect(),
        lines: Vec::new(),
        image_slots: Vec::new(),
        cols: cols as usize,
        theme,
        opts,
        col: 0,
        cancel_token: cancel,
        cell_clip: None,
        cell_ellipsis: false,
        current_source_line: 0,
        row_source: Vec::new(),
        code_blocks: Vec::new(),
        quote_depth: 0,
        quote_margin: None,
    };

    for child in root.children() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            return None;
        }
        render_block(&mut ctx, child, 0);
    }

    let slot_map: std::collections::HashMap<usize, String> = ctx
        .image_slots
        .iter()
        .map(|(i, url)| (*i, url.clone()))
        .collect();

    let lines: Vec<RenderLine> = ctx
        .lines
        .into_iter()
        .enumerate()
        .map(|(i, lb)| RenderLine {
            spans: lb.spans,
            visual_width: lb.visual_width,
            is_blank: false,
            image_url: slot_map.get(&i).map(|s| Arc::from(s.as_str())),
            source_line: ctx.row_source.get(i).copied().unwrap_or(0),
        })
        .collect();

    Some(LayoutResult {
        document: RenderedDocument::new(lines),
        code_blocks: ctx.code_blocks,
    })
}

pub(crate) fn code_lines(literal: &str) -> impl Iterator<Item = &str> {
    literal.split_terminator('\n')
}

fn is_closing_fence(line: &str, cb: &NodeCodeBlock) -> bool {
    let mut s = line;
    if s.ends_with('\r') {
        s = &s[..s.len() - 1];
    }
    let mut chars = s.chars().peekable();
    let mut space_count = 0;
    while let Some(&' ') = chars.peek() {
        space_count += 1;
        chars.next();
    }
    if space_count > 3 {
        return false;
    }
    let fence_char = cb.fence_char as char;
    let mut count = 0;
    while let Some(&c) = chars.peek() {
        if c == fence_char {
            count += 1;
            chars.next();
        } else {
            break;
        }
    }
    if count < cb.fence_length {
        return false;
    }
    for c in chars {
        if c != ' ' && c != '\t' {
            return false;
        }
    }
    true
}
pub(crate) fn highlight_code_block(
    language: &str,
    literal: &str,
    code_theme: &str,
    cancel: &AtomicBool,
) -> Option<Arc<HighlightedBlock>> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let syntax = SYNTAX_SET.find_syntax_by_token(language)?;
    let theme = THEME_SET.themes.get(code_theme)?;

    if cancel.load(Ordering::Relaxed) {
        return None;
    }

    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
    let mut style_cache = std::collections::HashMap::new();
    let mut block = Vec::with_capacity(code_lines(literal).count());

    for line in code_lines(literal) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }

        if let Ok(ranges) = highlighter.highlight_line(line, &SYNTAX_SET) {
            let mut line_spans: Vec<HighlightedSpan> = Vec::new();
            for (syn_style, text) in &ranges {
                let rt_style = *style_cache
                    .entry(*syn_style)
                    .or_insert_with(|| syntect_style_to_ratatui(*syn_style));
                if let Some(last_span) = line_spans.last_mut()
                    && last_span.style == rt_style
                {
                    last_span.text.push_str(text);
                    continue;
                }
                line_spans.push(HighlightedSpan {
                    text: text.to_string(),
                    style: rt_style,
                });
            }
            block.push(Some(line_spans));
        } else {
            block.push(None);
        }
    }

    if block.is_empty() {
        block.push(None);
    }

    Some(Arc::new(block))
}

pub(crate) fn render_code_patch(
    block: &PendingCodeBlock,
    highlighted: &HighlightedBlock,
    cols: u16,
    theme: &MarkdownTheme,
    opts: &MdRenderOpts,
    cancel: &AtomicBool,
) -> Option<Vec<RenderLine>> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }

    let mut ctx = Ctx {
        source_lines: Vec::new(),
        lines: Vec::new(),
        image_slots: Vec::new(),
        cols: cols as usize,
        theme,
        opts,
        col: 0,
        cancel_token: cancel,
        cell_clip: None,
        cell_ellipsis: false,
        current_source_line: block.first_code_source_line,
        row_source: Vec::new(),
        code_blocks: Vec::new(),
        quote_depth: 0,
        quote_margin: None,
    };

    let _range = render_code_rows(
        &mut ctx,
        block.depth,
        block.first_code_source_line,
        &block.literal,
        Some(highlighted),
    );

    if cancel.load(Ordering::Relaxed) {
        return None;
    }

    if ctx.lines.len() != block.line_range.len() {
        return None;
    }

    let lines: Vec<RenderLine> = ctx
        .lines
        .into_iter()
        .enumerate()
        .map(|(i, lb)| RenderLine {
            spans: lb.spans,
            visual_width: lb.visual_width,
            is_blank: false,
            image_url: None,
            source_line: ctx
                .row_source
                .get(i)
                .copied()
                .unwrap_or(block.first_code_source_line),
        })
        .collect();

    Some(lines)
}

pub(crate) fn load_syntax_assets(code_theme: &str) {
    let _ = &*SYNTAX_SET;
    let _ = THEME_SET.themes.get(code_theme);
}

struct LineBuilder {
    spans: Vec<StyledSpan>,
    visual_width: usize,
}

struct Ctx<'src, 'render> {
    source_lines: Vec<&'src str>,
    lines: Vec<LineBuilder>,
    image_slots: Vec<(usize, String)>,
    cols: usize,
    theme: &'render MarkdownTheme,
    opts: &'render MdRenderOpts,
    col: usize,
    cancel_token: &'render AtomicBool,
    cell_clip: Option<usize>,
    cell_ellipsis: bool,
    current_source_line: usize,
    row_source: Vec<usize>,
    code_blocks: Vec<PendingCodeBlock>,
    quote_depth: usize,
    quote_margin: Option<usize>,
}
fn block_margin(depth: usize) -> usize {
    1 + depth * 2
}

impl Ctx<'_, '_> {
    fn cur_col(&self) -> usize {
        self.col
    }
    fn source_line(&self, one_based: usize) -> Option<&str> {
        one_based
            .checked_sub(1)
            .and_then(|i| self.source_lines.get(i).copied())
    }

    fn emit_quote_rails(&mut self) {
        for _ in 0..self.quote_depth {
            self.push_raw('│', self.theme.blockquote_bar);
            self.push_raw(' ', self.theme.blockquote_bar);
        }
    }

    fn begin_inline_continuation(&mut self, source_line: usize, content_margin: usize) {
        let q_margin = self.quote_margin.unwrap_or(0);
        self.push_line(source_line, q_margin);
        self.emit_quote_rails();
        self.ensure_margin(content_margin);
    }

    fn ensure_line(&mut self) -> &mut LineBuilder {
        if self.lines.is_empty() {
            self.lines.push(LineBuilder {
                spans: Vec::with_capacity(4),
                visual_width: 0,
            });
        }
        self.lines.last_mut().expect("lines is not empty")
    }

    // ---- new source-line helpers ----

    fn push_line(&mut self, source_line: usize, margin: usize) {
        self.lines.push(LineBuilder {
            spans: Vec::with_capacity(4),
            visual_width: 0,
        });
        self.row_source.push(source_line);
        self.current_source_line = source_line;
        self.col = 0;
        if self.quote_depth > 0 {
            let q_margin = self.quote_margin.unwrap_or(0);
            for _ in 0..q_margin {
                self.push_raw(' ', Style::default());
            }
            self.emit_quote_rails();
            self.ensure_margin(margin);
        } else {
            for _ in 0..margin {
                self.push_raw(' ', Style::default());
            }
        }
    }

    fn ensure_source_line(&mut self, source_line: usize, margin: usize) {
        if self.lines.is_empty() {
            self.push_line(source_line, margin);
            return;
        }
        if source_line <= self.current_source_line {
            self.ensure_margin(margin);
            return;
        }
        // Fill blank rows for skipped physical lines
        let mut next = self.current_source_line + 1;
        while next < source_line {
            self.push_line(next, 0);
            next += 1;
        }
        self.push_line(source_line, margin);
    }

    fn wrap_line(&mut self, margin: usize) {
        if self.quote_depth > 0 {
            self.begin_inline_continuation(self.current_source_line, margin);
        } else {
            self.lines.push(LineBuilder {
                spans: Vec::with_capacity(4),
                visual_width: 0,
            });
            self.row_source.push(self.current_source_line);
            self.col = 0;
            for _ in 0..margin {
                self.push_raw(' ', Style::default());
            }
        }
    }

    // ---- original methods (updated) ----

    fn push_raw(&mut self, ch: char, st: Style) {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        let line = self.ensure_line();
        if let Some(last_span) = line.spans.last_mut()
            && last_span.style == st
        {
            last_span.text.push(ch);
            line.visual_width += w;
            self.col += w;
            return;
        }
        let mut text = String::with_capacity(8);
        text.push(ch);
        line.spans.push(StyledSpan { text, style: st });
        line.visual_width += w;
        self.col += w;
    }

    fn ensure_margin(&mut self, margin: usize) {
        let cur = self.cur_col();
        if cur < margin {
            for _ in cur..margin {
                self.push_raw(' ', Style::default());
            }
        }
    }

    fn push(&mut self, ch: char, st: Style, margin: usize) {
        if ch == '\t' {
            self.push(' ', st, margin);
            return;
        }
        if ch.is_control() {
            return;
        }
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            return;
        }
        let col = self.col;
        if let Some(clip) = self.cell_clip {
            if col + w > clip {
                if !self.cell_ellipsis && col < clip {
                    self.push_raw('…', st);
                    self.cell_ellipsis = true;
                }
                return;
            }
        } else if col + w > self.cols {
            if self.opts.wrap {
                if self.opts.wrap_indicator {
                    let target = self.cols.saturating_sub(1);
                    let cur = self.cur_col();
                    if cur <= target {
                        let glyph_st = self.theme.hr;
                        for _ in cur..target {
                            self.push_raw(' ', Style::default());
                        }
                        self.push_raw('┄', glyph_st);
                    }
                }
                self.wrap_line(margin);
            } else {
                return;
            }
        }
        self.push_raw(ch, st);
    }

    fn push_str_raw(&mut self, s: &str, st: Style) {
        if s.is_empty() {
            return;
        }
        let w = s.len();
        let line = self.ensure_line();
        if let Some(last_span) = line.spans.last_mut()
            && last_span.style == st
        {
            last_span.text.push_str(s);
            line.visual_width += w;
            self.col += w;
            return;
        }
        line.spans.push(StyledSpan {
            text: s.to_owned(),
            style: st,
        });
        line.visual_width += w;
        self.col += w;
    }

    fn push_str(&mut self, s: &str, st: Style, margin: usize) {
        let bytes = s.as_bytes();
        let mut idx = 0;
        let mut bytes_since_cancel_check = 0;

        while idx < bytes.len() {
            if bytes_since_cancel_check >= 256 {
                if self.cancel_token.load(Ordering::Relaxed) {
                    return;
                }
                bytes_since_cancel_check = 0;
            }

            let mut run_len = 0;
            while idx + run_len < bytes.len() {
                let b = bytes[idx + run_len];
                if (0x20..=0x7e).contains(&b) {
                    run_len += 1;
                } else {
                    break;
                }
            }

            if run_len > 0 {
                let run_str = unsafe { std::str::from_utf8_unchecked(&bytes[idx..idx + run_len]) };
                idx += run_len;
                bytes_since_cancel_check += run_len;

                let mut remaining_run = run_str;
                while !remaining_run.is_empty() {
                    if let Some(clip) = self.cell_clip {
                        let available = clip.saturating_sub(self.col);
                        if available == 0 {
                            if !self.cell_ellipsis {
                                self.push_raw('…', st);
                                self.cell_ellipsis = true;
                            }
                            return;
                        }
                        let chunk_len = remaining_run.len().min(available);
                        self.push_str_raw(&remaining_run[..chunk_len], st);
                        remaining_run = &remaining_run[chunk_len..];
                    } else {
                        let available = self.cols.saturating_sub(self.col);
                        if available == 0 {
                            if self.opts.wrap {
                                if self.opts.wrap_indicator {
                                    let target = self.cols.saturating_sub(1);
                                    let cur = self.cur_col();
                                    if cur <= target {
                                        let glyph_st = self.theme.hr;
                                        for _ in cur..target {
                                            self.push_raw(' ', Style::default());
                                        }
                                        self.push_raw('┄', glyph_st);
                                    }
                                }
                                self.wrap_line(margin);
                            } else {
                                return;
                            }
                        } else {
                            let chunk_len = remaining_run.len().min(available);
                            self.push_str_raw(&remaining_run[..chunk_len], st);
                            remaining_run = &remaining_run[chunk_len..];
                        }
                    }
                }
            } else {
                let rest_str = unsafe { std::str::from_utf8_unchecked(&bytes[idx..]) };
                if let Some(ch) = rest_str.chars().next() {
                    let ch_len = ch.len_utf8();
                    idx += ch_len;
                    bytes_since_cancel_check += ch_len;
                    self.push(ch, st, margin);
                } else {
                    break;
                }
            }
        }
    }

    fn push_spaces(&mut self, n: usize, margin: usize) {
        for _ in 0..n {
            self.push(' ', Style::default(), margin);
        }
    }
}

// ---------------------------------------------------------------------------
// Block-level rendering
// ---------------------------------------------------------------------------

fn render_block<'a>(ctx: &mut Ctx<'_, '_>, node: &'a AstNode<'a>, depth: usize) {
    let val_type = &node.data.borrow().value;
    if matches!(val_type, NodeValue::BlockQuote | NodeValue::Heading(_)) {
        match val_type {
            NodeValue::BlockQuote => render_blockquote(ctx, node, depth),
            NodeValue::Heading(h) => render_heading(ctx, node, h, depth),
            _ => unreachable!(),
        }
        return;
    }

    // Container-only nodes — just recurse children, no own source row
    let is_container = {
        let data = node.data.borrow();
        matches!(
            &data.value,
            NodeValue::Item(_) | NodeValue::DescriptionItem(_) | NodeValue::DescriptionList
        )
    };
    if is_container {
        for child in node.children() {
            render_block(ctx, child, depth);
        }
        return;
    }

    // Source-line advancement for content-producing nodes
    let (src_line, end_line, is_hidden, is_ti) = {
        let data = node.data.borrow();
        let src = data.sourcepos.start.line;
        let end = data.sourcepos.end.line;
        let hidden = matches!(
            &data.value,
            NodeValue::HtmlBlock(_) | NodeValue::FrontMatter(_)
        );
        let ti = matches!(&data.value, NodeValue::TaskItem(_));
        (src, end, hidden, ti)
    };

    let margin = block_margin(depth);

    if is_hidden {
        // HtmlBlock/FrontMatter: emit blank rows for every source line, no spans
        ctx.ensure_source_line(src_line, margin);
        while ctx.current_source_line < end_line {
            ctx.ensure_source_line(ctx.current_source_line + 1, 0);
        }
        return;
    }

    ctx.ensure_source_line(src_line, margin);

    if is_ti {
        // TaskItem handled here: checkbox prefix on its source line, then recurse children
        let checked = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::TaskItem(c) => *c,
                _ => None,
            }
        };
        if checked.is_some() {
            ctx.push_str("[✓] ", ctx.theme.task_checked, margin);
        } else {
            ctx.push_str("[ ] ", ctx.theme.task_unchecked, margin);
        }
        for child in node.children() {
            render_block(ctx, child, depth);
        }
        return;
    }

    let data = node.data.borrow();
    match &data.value {
        NodeValue::Heading(h) => render_heading(ctx, node, h, depth),
        NodeValue::Paragraph => render_paragraph(ctx, node, depth),
        NodeValue::List(list) => render_list(ctx, node, list, depth),
        NodeValue::CodeBlock(cb) => render_code_block(ctx, cb, depth, src_line, end_line),
        NodeValue::BlockQuote => render_blockquote(ctx, node, depth),
        NodeValue::Table(tbl) => render_table(ctx, node, tbl, depth),
        NodeValue::ThematicBreak => render_hr(ctx, depth),
        NodeValue::DescriptionTerm => {
            let style = ctx.theme.table_header;
            for child in node.children() {
                render_inline(ctx, child, style, margin);
            }
        }
        NodeValue::DescriptionDetails => {
            let inner_margin = margin + 4;
            let detail_line = node.data.borrow().sourcepos.start.line;
            let detail_src = if detail_line <= ctx.current_source_line {
                ctx.current_source_line + 1
            } else {
                detail_line
            };
            ctx.ensure_source_line(detail_src, inner_margin);

            for child in node.children() {
                let data_val = child.data.borrow();
                if matches!(&data_val.value, NodeValue::Paragraph) {
                    let style = ctx.theme.paragraph;
                    drop(data_val);
                    for inline in child.children() {
                        render_inline(ctx, inline, style, inner_margin);
                    }
                } else {
                    drop(data_val);
                    render_block(ctx, child, depth + 1);
                }
            }
        }
        NodeValue::FootnoteDefinition(_fd) => render_footnote_def(ctx, node, depth),
        _ => {
            drop(data);
            for child in node.children() {
                render_block(ctx, child, depth);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Heading
// ---------------------------------------------------------------------------

fn heading_style(ctx: &Ctx<'_, '_>, level: u8) -> Style {
    let base = match level {
        1 => ctx.theme.h1,
        2 => ctx.theme.h2,
        3 => ctx.theme.h3,
        4 => ctx.theme.h4,
        5 => ctx.theme.h5,
        _ => ctx.theme.h6,
    };
    if level == 1 {
        base
    } else {
        let mut st = Style::default().add_modifier(Modifier::BOLD);
        if let Some(fg) = base.fg {
            st = st.fg(fg);
            st = st.bg(faint_background(fg));
        }
        st
    }
}

fn heading_marker(level: u8, mode: crate::config::IconMode) -> &'static str {
    match mode {
        crate::config::IconMode::Nerd => match level {
            1 => "\u{f0ca1}",
            2 => "\u{f0ca3}",
            3 => "\u{f0ca5}",
            4 => "\u{f0ca7}",
            5 => "\u{f0ca9}",
            6 => "\u{f0cab}",
            _ => unreachable!("heading level must be 1-6"),
        },
        crate::config::IconMode::Unicode => match level {
            1 => "①",
            2 => "②",
            3 => "③",
            4 => "④",
            5 => "⑤",
            6 => "⑥",
            _ => unreachable!("heading level must be 1-6"),
        },
        crate::config::IconMode::None => match level {
            1 => "I",
            2 => "II",
            3 => "III",
            4 => "IV",
            5 => "V",
            6 => "VI",
            _ => unreachable!("heading level must be 1-6"),
        },
    }
}
fn heading_margin(depth: usize, level: u8) -> usize {
    depth + usize::from(level.saturating_sub(1))
}

fn render_heading<'a>(ctx: &mut Ctx<'_, '_>, node: &'a AstNode<'a>, h: &NodeHeading, depth: usize) {
    let _margin = block_margin(depth);
    let style = heading_style(ctx, h.level);

    let bg_style = if h.level == 1 {
        ctx.theme.h1_banner
    } else {
        style
    };

    let start_line = node.data.borrow().sourcepos.start.line;
    ctx.ensure_source_line(start_line, 0);

    let h_margin = heading_margin(depth, h.level);
    ctx.push(' ', bg_style, 0);

    for _ in 0..h_margin {
        ctx.push(' ', bg_style, 0);
    }

    ctx.push_str(heading_marker(h.level, ctx.opts.icon_mode), bg_style, 0);
    ctx.push(' ', bg_style, 0);
    let text_start_col = ctx.cur_col();
    for child in node.children() {
        render_inline(ctx, child, bg_style, 0);
    }

    let title_width = ctx.cur_col().saturating_sub(text_start_col);

    let cur = ctx.cur_col();
    if cur < ctx.cols {
        for _ in cur..ctx.cols {
            ctx.push(' ', bg_style, 0);
        }
    }

    if h.setext {
        let end_line = node.data.borrow().sourcepos.end.line;
        ctx.ensure_source_line(end_line, 0);
        ctx.ensure_margin(text_start_col);
        for _ in 0..title_width.max(1) {
            ctx.push('─', ctx.theme.hr, 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Paragraph
// ---------------------------------------------------------------------------

fn render_paragraph<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize) {
    let margin = block_margin(depth);
    let style = ctx.theme.paragraph;

    for child in node.children() {
        render_inline(ctx, child, style, margin);
    }
    // No trailing new_line — source-gap management by ensure_source_line
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
    let margin = block_margin(depth);
    let is_ordered = list.list_type == ListType::Ordered;
    let mut item_num = list.start;

    for child in node.children() {
        let child_data = child.data.borrow();
        if ctx.cancel_token.load(Ordering::Relaxed) {
            break;
        }

        let src_line = child_data.sourcepos.start.line;
        ctx.ensure_source_line(src_line, margin);

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

                // No trailing new_line — next item or block ensures its own source line
            }
            NodeValue::TaskItem(checked) => {
                if checked.is_some() {
                    ctx.push_str("[✓] ", ctx.theme.task_checked, margin);
                } else {
                    ctx.push_str("[ ] ", ctx.theme.task_unchecked, margin);
                }
                for grandchild in child.children() {
                    render_list_child(ctx, grandchild, depth + 1, margin);
                }
            }
            _ => {
                render_block(ctx, child, depth);
            }
        }
    }
    // No trailing blank lines — post-list gap driven by source
}

// ---------------------------------------------------------------------------
// Code blocks
// ---------------------------------------------------------------------------

/// Visual display width of a string (sum of per-char Unicode widths).
#[allow(dead_code)]
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

fn render_code_rows(
    ctx: &mut Ctx<'_, '_>,
    depth: usize,
    first_code_source_line: usize,
    literal: &str,
    highlighted: Option<&HighlightedBlock>,
) -> Range<usize> {
    let start_line = ctx.lines.len();
    let border_st = ctx.theme.table_border;
    let margin = block_margin(depth);
    let code_indent = margin + 2; // "│ " rail

    for (i, line) in code_lines(literal).enumerate() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            break;
        }
        ctx.ensure_source_line(first_code_source_line + i, margin);
        ctx.push('│', border_st, margin);
        ctx.push(' ', Style::default(), margin);

        let spans_opt = highlighted
            .and_then(|h| h.get(i))
            .and_then(|opt| opt.as_ref());
        if let Some(spans) = spans_opt {
            for span in spans {
                ctx.push_str(&span.text, span.style, code_indent);
            }
        } else {
            ctx.push_str(line, ctx.theme.paragraph, code_indent);
        }
    }
    start_line..ctx.lines.len()
}

fn render_code_block(
    ctx: &mut Ctx<'_, '_>,
    cb: &NodeCodeBlock,
    depth: usize,
    src_line: usize,
    end_line: usize,
) {
    let saved_q_depth = ctx.quote_depth;
    ctx.quote_depth = 0;
    let margin = block_margin(depth);
    let border_st = ctx.theme.table_border;
    let is_fenced = cb.fenced;
    let has_label = is_fenced && !cb.info.is_empty();
    let lang = if has_label {
        cb.info.split_whitespace().next().unwrap_or("").to_string()
    } else {
        String::new()
    };

    // ---- Fenced opening ----
    if is_fenced {
        ctx.ensure_source_line(src_line, margin);
        ctx.push('┌', border_st, margin);
        if has_label && !lang.is_empty() {
            if !matches!(ctx.opts.icon_mode, crate::config::IconMode::None)
                && let Some((nerd, uni)) = lang_icon(&lang)
            {
                let g = crate::ui::get_icon(nerd, uni, ctx.opts.icon_mode);
                ctx.push(' ', Style::default(), margin);
                ctx.push_str(g, ctx.theme.blockquote, margin);
            }
            ctx.push(' ', Style::default(), margin);
            ctx.push_str(&lang, ctx.theme.code_inline, margin);
        }
    }

    let line_count = code_lines(&cb.literal).count();
    let first_code_src = if is_fenced { src_line + 1 } else { src_line };

    let range = render_code_rows(ctx, depth, first_code_src, &cb.literal, None);
    let start_line = range.start;
    let patch_end = range.end;

    // ---- Fenced closing ----
    let closing_line = cb
        .fenced
        .then(|| ctx.source_line(end_line))
        .flatten()
        .filter(|line| is_closing_fence(line, cb))
        .map(|_| end_line);

    if let Some(cl) = closing_line {
        ctx.ensure_source_line(cl, margin);
        ctx.push('└', border_st, margin);
    }

    // ---- Enqueue async syntax highlighting ----
    if ctx.opts.syntax_hl && is_fenced && !lang.is_empty() && line_count > 0 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        cb.literal.hash(&mut hasher);
        let literal_fingerprint = hasher.finish();

        let id = ctx.code_blocks.len() as u32;
        ctx.code_blocks.push(PendingCodeBlock {
            id,
            literal: Arc::from(cb.literal.as_str()),
            literal_fingerprint,
            language: Arc::from(lang.as_str()),
            depth,
            first_code_source_line: first_code_src,
            line_range: start_line..patch_end,
        });
    }
    ctx.quote_depth = saved_q_depth;
}

fn render_blockquote<'a>(ctx: &mut Ctx<'_, '_>, node: &'a AstNode<'a>, depth: usize) {
    let saved_depth = ctx.quote_depth;
    let saved_margin = ctx.quote_margin;

    ctx.quote_depth += 1;
    if ctx.quote_margin.is_none() {
        ctx.quote_margin = Some(block_margin(depth));
    }

    for child in node.children() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            break;
        }
        render_block(ctx, child, depth);
    }

    ctx.quote_depth = saved_depth;
    ctx.quote_margin = saved_margin;
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/// Captured table row data — holds references into the AST arena.
#[allow(dead_code)]
struct Row<'a> {
    is_header: bool,
    cells: Vec<Vec<&'a AstNode<'a>>>,
    source_line: usize,
}

fn cell_leading_pad(align: TableAlignment, col_width: usize, content_w: usize) -> usize {
    let slack = col_width.saturating_sub(content_w);
    match align {
        TableAlignment::Right => slack,
        TableAlignment::Center => slack / 2,
        _ => 0,
    }
}

/// Scale `col_widths` to fit within `available` visual columns,
/// distributing space proportionally to natural widths with min 1 per column.
fn scale_col_widths(col_widths: &mut Vec<usize>, available: usize) {
    let num = col_widths.len();
    if num == 0 {
        return;
    }
    let total: usize = col_widths.iter().sum();
    if total <= available || available < num {
        return;
    }
    let mut scaled = vec![1usize; num];
    let remaining = available - num;
    let weights: Vec<usize> = col_widths.iter().map(|w| (*w).saturating_sub(1)).collect();
    let weight_total: usize = weights.iter().sum();
    if weight_total == 0 {
        *col_widths = scaled;
        return;
    }
    let mut allocated: usize = 0;
    for (i, w) in weights.iter().enumerate() {
        let add = w * remaining / weight_total;
        scaled[i] += add;
        allocated += add;
    }
    let mut leftover = remaining - allocated;
    let mut order: Vec<usize> = (0..num).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(weights[i]));
    let mut k = 0;
    while leftover > 0 {
        scaled[order[k % num]] += 1;
        leftover -= 1;
        k += 1;
    }
    *col_widths = scaled;
}
fn render_table<'a>(ctx: &mut Ctx<'_, '_>, node: &'a AstNode<'a>, tbl: &NodeTable, depth: usize) {
    let margin = block_margin(depth);

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
            let source_line = data.sourcepos.start.line;
            rows.push(Row {
                is_header: *is_header,
                cells,
                source_line,
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

    // Borderless layout: margin + " │ " separators
    let sep_w: usize = 3; // " │ " visual width
    let available = ctx
        .cols
        .saturating_sub(margin * 2 + num_cols.saturating_sub(1) * sep_w);
    scale_col_widths(&mut col_widths, available);

    let mut col_offsets = vec![0usize; num_cols];
    let mut offset = margin;
    for ci in 0..num_cols {
        col_offsets[ci] = offset;
        offset += col_widths[ci];
        if ci + 1 < num_cols {
            offset += sep_w;
        }
    }
    let border_st = ctx.theme.table_border;

    // Header row → table start source line
    let table_start_line = node.data.borrow().sourcepos.start.line;
    ctx.ensure_source_line(table_start_line, margin);

    if let Some(first) = rows.first() {
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
            ctx.cell_clip = Some(col_offsets[ci] + col_widths[ci]);
            ctx.cell_ellipsis = false;
            ctx.push_spaces(cell_leading_pad(align, col_widths[ci], content_w), margin);
            let header_style = ctx.theme.table_header;
            for inline in cell_inlines {
                render_inline(ctx, inline, header_style, margin);
            }
            ctx.cell_clip = None;
            let cur = ctx.cur_col();
            let target = col_offsets[ci] + col_widths[ci];
            if cur < target {
                ctx.push_spaces(target - cur, margin);
            }
            if ci + 1 < num_cols {
                ctx.push_str(" │ ", border_st, margin);
            }
        }
    }

    // Delimiter row (source delimiter line: ─ across cells, ┼ at boundaries)
    ctx.ensure_source_line(table_start_line + 1, margin);
    for (ci, w) in col_widths.iter().enumerate() {
        for _ in 0..*w {
            ctx.push('─', border_st, margin);
        }
        if ci + 1 < num_cols {
            ctx.push('┼', border_st, margin);
        }
    }

    // Body rows
    for row in rows.iter().skip(1) {
        let body_src = row.source_line;
        ctx.ensure_source_line(body_src, margin);

        for (ci, cell_inlines) in row.cells.iter().enumerate() {
            if ci >= num_cols {
                break;
            }
            let align = tbl
                .alignments
                .get(ci)
                .copied()
                .unwrap_or(TableAlignment::None);
            ctx.cell_clip = Some(col_offsets[ci] + col_widths[ci]);
            ctx.cell_ellipsis = false;
            let content_w: usize = cell_inlines.iter().map(|n| inline_text_len(n)).sum();
            ctx.push_spaces(cell_leading_pad(align, col_widths[ci], content_w), margin);
            let cell_style = ctx.theme.table_cell;
            for inline in cell_inlines {
                render_inline(ctx, inline, cell_style, margin);
            }
            ctx.cell_clip = None;
            let cur = ctx.cur_col();
            let target = col_offsets[ci] + col_widths[ci];
            if cur < target {
                ctx.push_spaces(target - cur, margin);
            }
            if ci + 1 < num_cols {
                ctx.push_str(" │ ", border_st, margin);
            }
        }
    }
    // No trailing blank lines — gap driven by source
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
    let margin = block_margin(depth);
    let width = ctx.cols.saturating_sub(margin * 2).max(4);
    for _ in 0..width {
        ctx.push('─', ctx.theme.hr, margin);
    }
    // No trailing new_line — next block fills gaps
}

// ---------------------------------------------------------------------------
// Footnote definition
// ---------------------------------------------------------------------------
fn render_footnote_def<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize) {
    let data = node.data.borrow();
    let NodeValue::FootnoteDefinition(fd) = &data.value else {
        return;
    };
    let margin = block_margin(depth);

    let label = format!("[^{}]: ", fd.name);
    ctx.push_str(&label, ctx.theme.footnote_def, margin);

    // Drop borrow before recursing
    drop(data);
    for child in node.children() {
        render_block(ctx, child, depth);
    }
    // No trailing new_line — gap driven by source
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
fn render_inline<'a>(
    ctx: &mut Ctx<'_, '_>,
    node: &'a AstNode<'a>,
    base_style: Style,
    margin: usize,
) {
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
            // compute is_bare first (single text child equal to URL)
            let is_bare = {
                let mut child_iter = node.children();
                if let Some(first_child) = child_iter.next() {
                    if child_iter.next().is_none() {
                        let d = first_child.data.borrow();
                        matches!(&d.value, NodeValue::Text(t) if *t == link.url)
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            // write configured icon once
            let icon = link_icon(&link.url, ctx.opts.icon_mode);
            if !icon.is_empty() {
                ctx.push_str(icon, ctx.theme.blockquote, margin);
                ctx.push(' ', ctx.theme.blockquote, margin);
            }

            // render child label once in theme.link_text
            for child in node.children() {
                render_inline(ctx, child, ctx.theme.link_text, margin);
            }

            if is_bare {
                // stop
            } else if let Some(host) = compact_http_host(&link.url) {
                // append default-style space, "· " plus host in theme.link_url
                ctx.push(' ', Style::default(), margin);
                ctx.push_str("· ", ctx.theme.link_url, margin);
                ctx.push_str(host, ctx.theme.link_url, margin);
            }
        }
        NodeValue::Image(img) => {
            let icon = crate::ui::get_icon("\u{f03e}", "\u{1f5bc}", ctx.opts.icon_mode);
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
                    &truncate_url_middle(&img.url, ctx.opts.link_url_max),
                    ctx.theme.link_url,
                    margin,
                );
            }
            // Record image slot for UI overlay
            if !img.url.is_empty() && !ctx.lines.is_empty() {
                let line_idx = ctx.lines.len() - 1;
                ctx.image_slots.push((line_idx, img.url.clone()));
            }
        }
        NodeValue::WikiLink(wl) => {
            ctx.push_str(&format!("[[{}]]", wl.url), ctx.theme.wikilink, margin);
        }
        NodeValue::FootnoteReference(fr) => {
            ctx.push_str(&format!("[^{}]", fr.name), ctx.theme.footnote_ref, margin);
        }
        NodeValue::SoftBreak => {
            let src_line = ctx.current_source_line + 1;
            if ctx.quote_depth > 0 {
                ctx.begin_inline_continuation(src_line, margin);
            } else {
                ctx.ensure_source_line(src_line, 0);
            }
        }
        NodeValue::LineBreak => {
            let src_line = ctx.current_source_line + 1;
            if ctx.quote_depth > 0 {
                ctx.begin_inline_continuation(src_line, margin);
            } else {
                ctx.ensure_source_line(src_line, 0);
            }
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
pub(crate) fn syntect_style_to_ratatui(s: syntect::highlighting::Style) -> ratatui::style::Style {
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

/// Strip scheme, userinfo, and optional leading `www.` from an HTTP(S) URL.
/// Returns `None` for non-HTTP(S) or empty authority.
fn compact_http_host(url: &str) -> Option<&str> {
    let rest = if url.len() >= 7 && url[..7].eq_ignore_ascii_case("http://") {
        &url[7..]
    } else if url.len() >= 8 && url[..8].eq_ignore_ascii_case("https://") {
        &url[8..]
    } else {
        return None;
    };
    if rest.is_empty() {
        return None;
    }
    let mut auth_len = 0;
    for &b in rest.as_bytes() {
        if b == b'/' || b == b'?' || b == b'#' {
            break;
        }
        auth_len += 1;
    }
    let auth = &rest[..auth_len];
    if auth.is_empty() {
        return None;
    }
    let after_userinfo = if let Some(pos) = auth.rfind('@') {
        &auth[pos + 1..]
    } else {
        auth
    };
    let host = if after_userinfo
        .get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("www."))
    {
        &after_userinfo[4..]
    } else {
        after_userinfo
    };
    if host.is_empty() {
        return None;
    }
    Some(host)
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
        render_layout(content, cols, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec()
    }

    fn line_text(line: &RenderLine) -> String {
        line.spans.iter().map(|s| s.text.as_str()).collect()
    }

    fn line_cells(line: &RenderLine) -> Vec<(char, Style)> {
        let mut cells = Vec::new();
        for span in &line.spans {
            for c in span.text.chars() {
                cells.push((c, span.style));
            }
        }
        cells
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
        let h1_cells = line_cells(h1_line.expect("lines is not empty"));
        let h1_first = h1_cells.iter().find(|(c, _)| *c != ' ').map(|(_, s)| *s);
        assert!(h1_first.is_some(), "h1 has content");
        assert!(
            has_mod(h1_first.expect("lines is not empty"), Modifier::BOLD),
            "h1 bold"
        );

        let h2_line = lines.iter().find(|l| line_text(l).contains("Heading 2"));
        assert!(h2_line.is_some(), "h2 should appear");
        let h2_cells = line_cells(h2_line.expect("lines is not empty"));
        let h2_first = h2_cells.iter().find(|(c, _)| *c != ' ').map(|(_, s)| *s);
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
            !line_cells(h2.expect("lines is not empty"))
                .iter()
                .any(|(c, _)| *c == '#'),
            "heading should NOT contain hash prefix"
        );
    }
    #[test]
    fn renders_table_without_box_borders() {
        let lines = render_test("| A | B |\n|---|---|\n| 1 | 2 |\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains('│'), "column separator bar");
        assert!(text.contains('A'), "cell A");
        assert!(text.contains('B'), "cell B");
        assert!(text.contains('1'), "cell 1");
        assert!(text.contains('2'), "cell 2");
        assert!(!text.contains('┌'), "no top-left border");
        assert!(!text.contains('┃'), "no column separator pipe");
    }

    #[test]
    fn headings_have_one_leading_cell_and_reduced_indentation() {
        let lines = render_test(
            "# H1\n\n## H2\n\n### H3\n\n#### H4\n\n##### H5\n\n###### H6\n",
            80,
            true,
            false,
        );

        for (title, indent) in [
            ("H1", 1),
            ("H2", 2),
            ("H3", 3),
            ("H4", 4),
            ("H5", 5),
            ("H6", 6),
        ] {
            let line = lines
                .iter()
                .find(|line| line_text(line).contains(title))
                .expect("heading should render");
            let text = line_text(line);
            assert_eq!(
                text.chars().take_while(|c| *c == ' ').count(),
                indent,
                "{title} indentation"
            );
        }

        let atx = render_test("# Narrow heading wraps safely\n", 8, true, false);
        assert!(
            line_text(&atx[0]).starts_with(' '),
            "narrow ATX heading keeps leading cell"
        );

        let setext = render_test("Setext heading\n==============\n", 80, true, false);
        let title = setext
            .iter()
            .find(|line| line_text(line).contains("Setext heading"))
            .expect("setext title");
        let underline = setext
            .iter()
            .find(|line| line_text(line).contains('─'))
            .expect("setext underline");
        assert!(
            line_text(title).starts_with(' '),
            "setext title keeps leading cell"
        );
        assert_eq!(
            line_text(underline).chars().filter(|ch| *ch == '─').count(),
            "Setext heading".chars().count(),
            "setext underline preserves title width"
        );
        let narrow_setext =
            render_test("Setext wraps safely\n===================\n", 8, true, false);
        assert!(
            narrow_setext
                .iter()
                .any(|line| line_text(line).contains('─')),
            "narrow setext heading retains underline"
        );
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
            line_text(bq.expect("lines is not empty")).contains('│'),
            "blockquote rail"
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
    fn empty_input_returns_no_lines() {
        let lines = render_test("", 80, true, false);
        assert!(lines.is_empty(), "empty input should have no lines");
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
            for &(ch, _) in &line_cells(line) {
                assert!(!ch.is_control(), "cell char is control: {ch:?}");
            }
        }
        let has_bell_ch = lines.iter().flat_map(line_cells).any(|(c, _)| c == '\x07');
        assert!(!has_bell_ch, "bell char \\x07 should not appear in cells");
    }

    #[test]
    fn tab_becomes_space() {
        let lines = render_test("a\tb", 80, true, false);
        let text: String = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains(' '), "tab should render as space");
        assert!(!text.contains('\t'), "no raw tab in output");
        let tab_pos = lines
            .iter()
            .flat_map(line_cells)
            .position(|(c, _)| c == '\t');
        assert!(tab_pos.is_none(), "no tab char in any cell");
    }

    #[test]
    fn single_blank_line_between_paragraphs() {
        let lines = render_test("para one\n\npara two", 80, true, false);
        let non_blank_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.is_blank)
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
            .filter(|(_, l)| !l.is_blank)
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
        assert!(!lines[0].is_blank, "first row should not be blank");
    }

    #[test]
    fn no_trailing_blank_lines() {
        let lines = render_test("a\n\nb", 80, true, false);
        let last = lines.last().expect("lines is not empty");
        assert!(!last.is_blank, "last row should not be blank");
    }

    #[test]
    fn adjacent_blocks_dont_touch() {
        let lines = render_test("a\n\nb", 80, true, false);
        let non_blank_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.is_blank)
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
            .filter(|(_, l)| {
                let text = line_text(l);
                !text.chars().all(|c| c.is_whitespace() || c == ' ')
            })
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
        let filtered: Vec<&RenderLine> = lines.iter().filter(|l| !l.is_blank).collect();
        // The two list items should be on consecutive rows
        let docs = filtered
            .iter()
            .position(|l| line_text(l).contains('D') || line_text(l).contains('d'))
            .filter(|_| {
                filtered
                    .iter()
                    .any(|l| line_text(l).contains('N') || line_text(l).contains('n'))
            });
        assert!(docs.is_some(), "should find Documents and Notes items");
    }

    #[test]
    fn h1_renders_as_banner() {
        let theme_colors = AppThemeColors::default();
        let lines = render_test("# Title\n", 30, true, false);
        assert!(!lines.is_empty(), "should have at least one line");
        let row0 = &lines[0];
        let row0_cells = line_cells(row0);

        let expected_bg = Some(faint_background(theme_colors.accent));
        let expected_fg = Some(theme_colors.accent);

        assert_eq!(
            row0_cells.len(),
            30,
            "row length should be 30 (filled line)"
        );

        for (i, (ch, st)) in row0_cells.iter().enumerate() {
            assert_eq!(st.bg, expected_bg, "cell {} bg should be faint accent", i);
            if *ch != ' ' {
                assert_eq!(st.fg, expected_fg, "cell {} fg should be accent", i);
                assert!(
                    has_mod(*st, Modifier::BOLD),
                    "cell {} should be bold; char={:?}",
                    i,
                    ch
                );
            }
        }
        assert!(
            !row0_cells.iter().any(|(c, _)| *c == '#'),
            "H1 banner should not contain # prefix"
        );
    }
    #[test]
    fn h2_renders_without_banner() {
        let lines = render_test("## Sub\n", 80, true, false);
        let h2_line = lines.iter().find(|l| line_text(l).contains("Sub"));
        assert!(h2_line.is_some(), "H2 should contain Sub");
        let h2 = h2_line.expect("lines is not empty");
        let h2_cells = line_cells(h2);
        assert!(
            !h2_cells.iter().any(|(c, _)| *c == '#'),
            "H2 should NOT have # prefix"
        );
        let h2_first = h2_cells.iter().find(|(c, _)| *c != ' ').map(|(_, s)| *s);
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
            for (c, st) in &line_cells(line) {
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
    fn code_block_shows_compact_fence() {
        let lines = render_test("```rust\nfn main() {}\n```\n", 80, true, false);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(
            text.contains('┌') && text.contains("rust") && text.contains('└'),
            "should contain compact fence with lang name"
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
        let leading_spaces = line_cells(def_line)
            .iter()
            .take_while(|(c, _)| *c == ' ')
            .count();
        assert_eq!(
            leading_spaces,
            5,
            "definition detail should be indented by 5 spaces (block_margin+4): {:?}",
            line_text(def_line)
        );
    }

    #[test]
    fn link_shows_github_icon() {
        let theme_colors = AppThemeColors::default();
        let theme = MarkdownTheme::from_app_theme(&theme_colors);
        let cancel = AtomicBool::new(false);
        let lines = render_layout(
            "[repo](https://github.com/user/repo)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(
            text.contains("📦 repo"),
            "should contain github unicode icon"
        );

        let lines_none = render_layout(
            "[repo](https://github.com/user/repo)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::None),
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
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
        let lines = render_layout(
            "![alt](url.png)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("🖼 alt"), "should contain image unicode icon");

        let lines_none = render_layout(
            "![alt](url.png)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::None),
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
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
    fn code_block_has_compact_fence() {
        let lines = render_test("```rust\nfn main(){}\n```\n", 80, true, false);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        let open = text.iter().find(|l| l.contains('┌')).expect("open fence");
        let close = text.iter().find(|l| l.contains('└')).expect("close fence");
        assert!(open.contains("rust"));
        assert!(open.len() >= 4);
        assert_eq!(
            close.chars().count(),
            2,
            "close fence is just '└' after margin"
        );
    }

    #[test]
    fn code_block_lang_icon() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let lines = render_layout(
            "```rust\nx\n```\n",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
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
        let cell = t.split('│').nth(1).unwrap_or("");
        assert!(cell.starts_with(' '), "right-aligned cell has leading pad");
        assert!(cell.trim_end().ends_with('2'), "value flush right");
    }

    #[test]
    fn compact_host_for_labeled_links() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::None);
        opts.link_url_max = 20;
        let long = "[t](https://example.com/very/long/path/to/resource)";
        let lines = render_layout(long, 80, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let text: Vec<String> = lines.iter().map(line_text).collect();
        let joined = text.join("");
        // Labeled HTTP(S) link shows compact host, not truncated URL
        assert!(joined.contains("example.com"), "compact host shown");
        assert!(!joined.contains("/resource"), "no full URL suffix");
    }

    #[test]
    fn inline_code_has_padding() {
        let lines = render_test("a `x` b", 80, true, false);
        let line = lines
            .iter()
            .find(|l| line_text(l).contains('x'))
            .expect("code line");
        let cells = line_cells(line);
        let idx = cells.iter().position(|(c, _)| *c == 'x').expect("x");
        assert_eq!(cells[idx - 1].0, ' ', "leading padding space");
    }

    #[test]
    fn blockquote_uses_consistent_rail() {
        let lines = render_test("> > > deep", 80, true, false);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            text.iter().any(|l| l.contains("│")),
            "blockquote rail present"
        );
    }

    #[test]
    fn wrap_indicator_shown() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::default());
        opts.wrap = true;
        opts.wrap_indicator = true;
        // 8 a's (col=9 with margin=1) then CJK (w=2, wraps at col 9 → indicator)
        let res = render_layout("aaaaaaaa\u{4e00}bbbbbbbb", 10, &theme, &opts, &cancel).unwrap();
        let lines = res.document.lines().to_vec();
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            text[0].ends_with('┄'),
            "first line should end with wrap indicator"
        );
        assert!(
            text.iter().any(|l| l.ends_with('┄')),
            "some line ends with continuation glyph"
        );
    }

    #[test]
    fn code_theme_unknown_falls_back_to_plain() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::default());
        opts.syntax_hl = true;
        opts.code_theme = "does-not-exist".to_string();
        let lines = render_layout("```rust\nfn main(){}\n```\n", 80, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            text.iter().any(|l| l.contains("fn main")),
            "code still rendered (plain fallback)"
        );
    }

    #[allow(dead_code)]
    fn table_shrinks_to_fit_pane() {
        // 4 columns, each ~20 chars content -> natural width ~90. At cols=22
        // the table must scale down with truncation ellipsis (each col gets ~3-4).
        let md = "| Col AAAAAAAAAAAAA | Col BBBBBBBBBBBBBBB | Col CCCCCCCCCCCCCCCC | Col DDDDDDDDDDDDDDDD |\n\
                  |--------------------|---------------------|----------------------|-----------------------|\n\
                  | aaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbb | cccccccccccccccccccc | ddddddddddddddddddddd |\n";
        let lines = render_test(md, 22, true, false);

        let is_border = |c: char| -> bool {
            matches!(c, '┃' | '┼' | '┬' | '┴' | '┌' | '┐' | '├' | '┤' | '└' | '┘')
        };

        // Collect border character visual positions per row
        let mut border_positions: Vec<Vec<usize>> = Vec::new();
        for line in &lines {
            let s = line_text(line);
            let positions: Vec<usize> = s
                .chars()
                .scan(0usize, |col, c| {
                    let pos = *col;
                    *col += UnicodeWidthChar::width(c).unwrap_or(0);
                    Some((pos, c))
                })
                .filter(|&(_, c)| is_border(c))
                .map(|(pos, _)| pos)
                .collect();
            if !positions.is_empty() {
                border_positions.push(positions);
            }
        }
        // All border rows must have identical border positions
        if let Some(first) = border_positions.first() {
            for (i, pos) in border_positions.iter().enumerate() {
                assert_eq!(
                    pos, first,
                    "border positions differ at row {i}: {pos:?} vs {first:?}"
                );
            }
        }
        assert!(
            !border_positions.is_empty(),
            "should have at least one border row"
        );
        // At least one ellipsis from column truncation
        let all_text: String = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(
            all_text.contains('…'),
            "should contain ellipsis from truncation"
        );
        // No row exceeds the visual column limit
        for line in &lines {
            let w: usize = str_visual_width(&line_text(line));
            assert!(
                w <= 22,
                "row visual width {w} exceeds pane cols 22: {}",
                line_text(line)
            );
        }
    }

    #[test]
    fn table_cjk_cells_align() {
        let md = "| 姓名 | 城市 |\n|---|---|\n| 张三 | 北京 |\n";
        let lines = render_test(md, 40, true, false);
        let is_border = |c: char| -> bool {
            matches!(c, '┃' | '┼' | '┬' | '┴' | '┌' | '┐' | '├' | '┤' | '└' | '┘')
        };
        let mut border_positions: Vec<Vec<usize>> = Vec::new();
        for line in &lines {
            let s = line_text(line);
            let positions: Vec<usize> = s
                .chars()
                .scan(0usize, |col, c| {
                    let pos = *col;
                    *col += UnicodeWidthChar::width(c).unwrap_or(0);
                    Some((pos, c))
                })
                .filter(|&(_, c)| is_border(c))
                .map(|(pos, _)| pos)
                .collect();
            if !positions.is_empty() {
                border_positions.push(positions);
            }
        }
        // All border rows must have identical positions
        if let Some(first) = border_positions.first() {
            for (i, pos) in border_positions.iter().enumerate() {
                assert_eq!(
                    pos, first,
                    "CJK table border positions differ at row {i}: {pos:?} vs {first:?}"
                );
            }
        }
        assert!(!border_positions.is_empty(), "should have border rows");
        // Verify CJK chars are present in output
        let all_text: String = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(all_text.contains('姓'), "姓名 should be in output");
        assert!(all_text.contains('城'), "城市 should be in output");
        assert!(all_text.contains('张'), "张三 should be in output");
        assert!(all_text.contains('北'), "北京 should be in output");
    }

    #[test]
    fn wide_char_does_not_overflow_cols() {
        let lines = render_test("中文测试", 6, true, false);
        for line in &lines {
            let s = line_text(line);
            let w: usize = str_visual_width(&s);
            assert!(w <= 6, "line visual width {w} exceeds cols=6: {s:?}");
        }
        // All 4 characters must be present (wrapped across lines)
        let all: String = lines.iter().map(line_text).collect::<String>();
        assert!(all.contains('中'), "中 missing");
        assert!(all.contains('文'), "文 missing");
        assert!(all.contains('测'), "测 missing");
        assert!(all.contains('试'), "试 missing");
    }
    fn assert_patch_compatible(base: &[RenderLine], patch: &[RenderLine]) {
        assert_eq!(
            base.len(),
            patch.len(),
            "length mismatch between base and patch"
        );
        for (i, (b, p)) in base.iter().zip(patch.iter()).enumerate() {
            let b_text = line_text(b);
            let p_text = line_text(p);
            assert_eq!(b_text, p_text, "text mismatch at line {}", i);
            assert_eq!(
                b.visual_width, p.visual_width,
                "visual width mismatch at line {}",
                i
            );
            assert_eq!(
                b.source_line, p.source_line,
                "source line mismatch at line {}",
                i
            );
            assert!(
                p.image_url.is_none(),
                "patch line {} should have no image URL",
                i
            );
        }
    }

    #[test]
    fn wide_no_wrap_source_row_fixture_test() {
        let mut content = String::new();
        content.push_str("# H1\n\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n\nalpha\nbeta\n\nSetext\n------\n\n> quote one\n> quote two\n\n```rust\nlet x = 1;\n```\n\n| A | B |\n|---|--:|\n| x | 1 |\n\n<div>\nhidden\n</div>\n\n");
        content.push_str("[repo](HTTPS://User@WWW.Example.com:8443/path)\n");

        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::Unicode);
        opts.wrap = false;

        let lines = render_layout(&content, 80, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();

        let texts: Vec<String> = lines.iter().map(line_text).collect();

        let h2_line = texts
            .iter()
            .find(|t| t.contains('②'))
            .expect("H2 should contain ②");
        let h3_line = texts
            .iter()
            .find(|t| t.contains('③'))
            .expect("H3 should contain ③");
        let h4_line = texts
            .iter()
            .find(|t| t.contains('④'))
            .expect("H4 should contain ④");
        let h5_line = texts
            .iter()
            .find(|t| t.contains('⑤'))
            .expect("H5 should contain ⑤");
        let h6_line = texts
            .iter()
            .find(|t| t.contains('⑥'))
            .expect("H6 should contain ⑥");

        assert!(!h2_line.contains('▌'));

        assert!(h2_line.starts_with("  ②"));
        assert!(h3_line.starts_with("   ③"));
        assert!(h4_line.starts_with("    ④"));
        assert!(h5_line.starts_with("     ⑤"));
        assert!(h6_line.starts_with("      ⑥"));

        let setext_idx = texts
            .iter()
            .position(|t| t.contains("Setext"))
            .expect("Setext heading");
        let underline_line = &texts[setext_idx + 1];
        assert!(underline_line.starts_with("    ──"));
        assert_eq!(str_visual_width("Setext"), 6);

        let q1_idx = texts
            .iter()
            .position(|t| t.contains("quote one"))
            .expect("quote one");
        let q2_idx = texts
            .iter()
            .position(|t| t.contains("quote two"))
            .expect("quote two");
        assert!(texts[q1_idx].starts_with(" │ "));
        assert!(texts[q2_idx].starts_with(" │ "));

        let open_fence = texts.iter().find(|t| t.contains('┌')).expect("open fence");
        assert!(open_fence.contains("rust"));
        let code_line = texts
            .iter()
            .find(|t| t.contains("let x = 1;"))
            .expect("code literal");
        assert!(code_line.starts_with(" │ "));
        let close_fence = texts.iter().find(|t| t.contains('└')).expect("close fence");
        assert!(close_fence.trim().starts_with('└'));

        let t_header = texts
            .iter()
            .find(|t| t.contains(" A │ B"))
            .expect("table header");
        assert!(!t_header.contains('┌') && !t_header.contains('┐'));
        let t_delim = texts
            .iter()
            .find(|t| t.contains('┼'))
            .expect("table delimiter");
        assert!(t_delim.contains("─┼─"));
        let t_body = texts
            .iter()
            .find(|t| t.contains(" x │ 1"))
            .expect("table body row");
        assert!(!t_body.contains('└') && !t_body.contains('┘'));

        let link_line = texts
            .iter()
            .find(|t| t.contains("repo"))
            .expect("link line");
        assert!(link_line.contains("repo · Example.com:8443"));
        assert!(!link_line.contains("User@"));
    }

    #[test]
    fn link_matrix_test() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let opts = mk_opts(crate::config::IconMode::None);

        let lines1 = render_layout("<https://example.com/path>", 80, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let t1 = line_text(&lines1[0]);
        assert_eq!(t1.trim(), "https://example.com/path");

        let lines2 = render_layout("[relative](../x.md)", 80, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let t2 = line_text(&lines2[0]);
        assert_eq!(t2.trim(), "relative");

        let lines3 = render_layout("[mail](mailto:a@b.com)", 80, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let t3 = line_text(&lines3[0]);
        assert_eq!(t3.trim(), "mail");

        let lines4 = render_layout(
            "[mixed](hTtPs://WWW.Example.com:8443/p)",
            80,
            &theme,
            &opts,
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
        let t4 = line_text(&lines4[0]);
        assert_eq!(t4.trim(), "mixed · Example.com:8443");
    }

    #[test]
    fn quote_continuation_matrix_test() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);

        let md = "> a\n> b";
        let lines = render_layout(
            md,
            80,
            &theme,
            &mk_opts(crate::config::IconMode::None),
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
        let t0 = line_text(&lines[0]);
        let t1 = line_text(&lines[1]);
        assert!(t0.starts_with(" │ "));
        assert!(t1.starts_with(" │ "));

        let mut opts = mk_opts(crate::config::IconMode::None);
        opts.wrap = true;
        let long_md = "> this is a very long line that should wrap on a narrow viewport";
        let lines_wrap = render_layout(long_md, 20, &theme, &opts, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        for line in &lines_wrap {
            let t = line_text(line);
            assert!(
                t.starts_with(" │ "),
                "wrapped quote line should start with rail: {:?}",
                t
            );
        }
    }

    #[test]
    fn table_row_source_matrix_test() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let md = "\
| a | b |
|---|---|
|   | 2 |
";
        let lines = render_layout(
            md,
            80,
            &theme,
            &mk_opts(crate::config::IconMode::None),
            &cancel,
        )
        .unwrap()
        .document
        .lines()
        .to_vec();
        assert_eq!(lines[2].source_line, 3);
    }

    #[test]
    fn fence_matrix_test() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let opts = mk_opts(crate::config::IconMode::None);

        let md1 = "```rust\nlet x = 1;\n```\n";
        let res1 = render_layout(md1, 80, &theme, &opts, &cancel).unwrap();
        assert_eq!(res1.code_blocks.len(), 1);
        let block = &res1.code_blocks[0];
        assert_eq!(block.line_range, 1..2);

        let md2 = "```rust\n```\n";
        let res2 = render_layout(md2, 80, &theme, &opts, &cancel).unwrap();
        assert_eq!(res2.code_blocks.len(), 0);

        let md3 = "```bash\nlong command\nnext";
        let res3 = render_layout(md3, 80, &theme, &opts, &cancel).unwrap();
        let lines3 = res3.document.lines();
        let texts3: Vec<String> = lines3.iter().map(line_text).collect();
        for t in &texts3 {
            assert!(!t.contains('└'));
        }
    }

    #[test]
    fn narrow_code_patch_regression_test() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);
        let mut opts = mk_opts(crate::config::IconMode::None);
        opts.code_line_numbers = false;
        opts.wrap = true;

        let content = "\
```rust
let very_long_variable_name_value = 42;
```
";
        let layout_res = render_layout(content, 24, &theme, &opts, &cancel).unwrap();
        let base_lines = layout_res.document.lines().to_vec();
        assert_eq!(layout_res.code_blocks.len(), 1);
        let block = &layout_res.code_blocks[0];

        let highlighted =
            highlight_code_block(&block.language, &block.literal, &opts.code_theme, &cancel)
                .unwrap();
        let patch_lines =
            render_code_patch(block, &highlighted, 24, &theme, &opts, &cancel).unwrap();

        let base_slice = &base_lines[block.line_range.clone()];
        assert_patch_compatible(base_slice, &patch_lines);
    }
    #[test]
    fn heading_marker_modes_test() {
        let theme = MarkdownTheme::from_app_theme(&AppThemeColors::default());
        let cancel = AtomicBool::new(false);

        // Nerd mode
        let opts_nerd = mk_opts(crate::config::IconMode::Nerd);
        let lines_nerd = render_layout("## H2", 80, &theme, &opts_nerd, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let t_nerd = line_text(&lines_nerd[0]);
        assert!(t_nerd.contains("\u{f0ca3}"));

        // Unicode mode
        let opts_uni = mk_opts(crate::config::IconMode::Unicode);
        let lines_uni = render_layout("## H2", 80, &theme, &opts_uni, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let t_uni = line_text(&lines_uni[0]);
        assert!(t_uni.contains("②"));

        // None mode
        let opts_none = mk_opts(crate::config::IconMode::None);
        let lines_none = render_layout("## H2", 80, &theme, &opts_none, &cancel)
            .unwrap()
            .document
            .lines()
            .to_vec();
        let t_none = line_text(&lines_none[0]);
        assert!(t_none.contains("II "));
    }
}
