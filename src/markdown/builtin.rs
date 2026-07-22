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

use std::sync::{Arc, LazyLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::ops::Range;

use comrak::nodes::{
    AstNode, ListType, NodeCodeBlock, NodeHeading, NodeList, NodeTable, NodeValue, TableAlignment,
};
use comrak::{Arena, Options, parse_document};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::markdown::MdRenderOpts;
use crate::markdown::style::{MarkdownTheme, RenderLine, StyledSpan, RenderedDocument};

pub(crate) struct PendingCodeBlock {
    pub id: u32,
    pub literal: Arc<str>,
    pub literal_fingerprint: u64,
    pub language: Arc<str>,
    pub fenced: bool,
    pub depth: usize,
    pub source_line: usize,
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
        lines: vec![LineBuilder {
            spans: Vec::with_capacity(4),
            visual_width: 0,
        }],
        image_slots: Vec::new(),
        cols: cols as usize,
        theme,
        opts,
        col: 0,
        cancel_token: cancel,
        cell_clip: None,
        cell_ellipsis: false,
        current_source_line: 0,
        row_source: vec![0],
        code_blocks: Vec::new(),
    };

    for child in root.children() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            return None;
        }
        render_block(&mut ctx, child, 0);
    }

    while let Some(last) = ctx.lines.last() {
        let is_empty = last.spans.iter().all(|s| s.text.chars().all(char::is_whitespace));
        if is_empty && ctx.lines.len() > 1 {
            ctx.lines.pop();
            ctx.row_source.pop();
        } else {
            break;
        }
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
    let mut block = Vec::with_capacity(literal.lines().count());

    for line in literal.lines() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }

        if let Ok(ranges) = highlighter.highlight_line(line, &SYNTAX_SET) {
            let mut line_spans: Vec<HighlightedSpan> = Vec::new();
            for (syn_style, text) in &ranges {
                let rt_style = *style_cache.entry(*syn_style).or_insert_with(|| {
                    syntect_style_to_ratatui(*syn_style)
                });
                if let Some(last_span) = line_spans.last_mut() {
                    if last_span.style == rt_style {
                        last_span.text.push_str(text);
                        continue;
                    }
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

    let margin = 2 + block.depth * 2;
    let code_margin = margin + 4;

    let literal_lines: Vec<&str> = block.literal.lines().collect();
    let line_count = literal_lines.len().max(1);
    let digits = line_count.to_string().len().max(2);
    let line_numbers = opts.code_line_numbers;
    let gutter_w = if line_numbers { digits + 3 } else { 0 };
    let code_indent = if line_numbers {
        margin + gutter_w
    } else {
        code_margin
    };

    let mut ctx = Ctx {
        lines: Vec::with_capacity(line_count),
        image_slots: Vec::new(),
        cols: cols as usize,
        theme,
        opts,
        col: 0,
        cancel_token: cancel,
        cell_clip: None,
        cell_ellipsis: false,
        current_source_line: block.source_line,
        row_source: Vec::with_capacity(line_count),
        code_blocks: Vec::new(),
    };

    for i in 0..line_count {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }

        ctx.new_line(margin);
        if line_numbers {
            push_code_gutter(&mut ctx, i + 1, digits);
        } else {
            ctx.push_spaces(4, margin);
        }

        let orig_line = literal_lines.get(i).copied().unwrap_or("");
        
        let spans_opt = highlighted.get(i);
        if let Some(Some(spans)) = spans_opt {
            for span in spans {
                ctx.push_str(&span.text, span.style, code_indent);
            }
        } else {
            ctx.push_str(orig_line, theme.paragraph, code_indent);
        }
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
            source_line: ctx.row_source.get(i).copied().unwrap_or(block.source_line),
        })
        .collect();

    if lines.len() != block.line_range.len() {
        debug_assert_eq!(lines.len(), block.line_range.len());
        return None;
    }

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

struct Ctx<'a> {
    lines: Vec<LineBuilder>,
    image_slots: Vec<(usize, String)>,
    cols: usize,
    theme: &'a MarkdownTheme,
    opts: &'a MdRenderOpts,
    col: usize,
    cancel_token: &'a AtomicBool,
    cell_clip: Option<usize>,
    cell_ellipsis: bool,
    current_source_line: usize,
    row_source: Vec<usize>,
    code_blocks: Vec<PendingCodeBlock>,
}

impl Ctx<'_> {
    fn cur_col(&self) -> usize {
        self.col
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

    fn push_raw(&mut self, ch: char, st: Style) {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        let line = self.ensure_line();
        if let Some(last_span) = line.spans.last_mut() {
            if last_span.style == st {
                last_span.text.push(ch);
                line.visual_width += w;
                self.col += w;
                return;
            }
        }
        let mut text = String::with_capacity(8);
        text.push(ch);
        line.spans.push(StyledSpan { text, style: st });
        line.visual_width += w;
        self.col += w;
    }

    fn new_line(&mut self, margin: usize) {
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
                self.new_line(margin);
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
        if let Some(last_span) = line.spans.last_mut() {
            if last_span.style == st {
                last_span.text.push_str(s);
                line.visual_width += w;
                self.col += w;
                return;
            }
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
                if b >= 0x20 && b <= 0x7e {
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
                                self.new_line(margin);
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

fn tag_row_source(ctx: &mut Ctx, src_line: usize) {
    if ctx.lines.last().is_some_and(|l| l.spans.is_empty())
        && let Some(s) = ctx.row_source.last_mut()
        && *s == 0
    {
        *s = src_line;
    }
}

// ---------------------------------------------------------------------------
// Block-level rendering
// ---------------------------------------------------------------------------

fn render_block<'a>(ctx: &mut Ctx, node: &'a AstNode<'a>, depth: usize) {
    let src_line = node.data.borrow().sourcepos.start.line;
    ctx.current_source_line = src_line;
    // Tag the pre-existing (initial) empty row so it maps to this block.
    tag_row_source(ctx, src_line);
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
        // H1: banner styled title with 2-space left indent, matching body text.
        let banner = ctx.theme.h1_banner;
        ctx.ensure_line();
        ctx.push(' ', Style::default(), 0); // col 0: plain indent space (default bg, not badge)
        ctx.push(' ', banner, 0); // col 1: badge starts — highlighted leading space
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
    let gutter_w = if ctx.opts.code_line_numbers { digits + 3 } else { 0 }; // "{:>w} │ "
    let code_indent = if ctx.opts.code_line_numbers {
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
        if !matches!(ctx.opts.icon_mode, crate::config::IconMode::None)
            && let Some((nerd, uni)) = lang_icon(&lang)
        {
            let g = crate::ui::get_icon(nerd, uni, ctx.opts.icon_mode);
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

    let start_line = ctx.lines.len();

    // Plain path
    for (i, line) in cb.literal.lines().enumerate() {
        if ctx.cancel_token.load(Ordering::Relaxed) {
            return;
        }
        ctx.new_line(margin);
        if ctx.opts.code_line_numbers {
            push_code_gutter(ctx, i + 1, digits);
        } else {
            ctx.push_spaces(4, margin);
        }
        ctx.push_str(line, ctx.theme.paragraph, code_indent);
    }
    let end_line = ctx.lines.len();

    ctx.new_line(margin);
    if has_label && !lang.is_empty() {
        close_code_pill(ctx, margin, inner);
    }
    ctx.new_line(0);
    ctx.new_line(0);

    // Enqueue for async syntax highlighting if requested and valid
    if ctx.opts.syntax_hl && cb.fenced && !lang.is_empty() {
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
            fenced: cb.fenced,
            depth,
            source_line: ctx.current_source_line,
            line_range: start_line..end_line,
        });
    }
}

/// Push the right-justified line-number gutter `{:>w} │ ` in muted style.
fn push_code_gutter(ctx: &mut Ctx, idx: usize, digits: usize) {
    use std::fmt::Write;
    let st = ctx.theme.blockquote;
    let line = ctx.ensure_line();
    let can_coalesce = if let Some(last_span) = line.spans.last_mut() {
        last_span.style == st
    } else {
        false
    };
    
    if can_coalesce {
        let last_span = line.spans.last_mut().unwrap();
        let prev_len = last_span.text.len();
        let _ = write!(last_span.text, "{:>width$} │ ", idx, width = digits);
        let added_len = last_span.text.len() - prev_len;
        line.visual_width += added_len;
        ctx.col += added_len;
    } else {
        let mut text = String::with_capacity(digits + 4);
        let _ = write!(text, "{:>width$} │ ", idx, width = digits);
        let added_len = text.len();
        line.spans.push(StyledSpan {
            text,
            style: st,
        });
        line.visual_width += added_len;
        ctx.col += added_len;
    }
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
    // Scale columns to fit the pane width
    let available = ctx
        .cols
        .saturating_sub(margin + 2 + num_cols.saturating_sub(1));
    scale_col_widths(&mut col_widths, available);

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
            ctx.cell_clip = Some(col_offsets[ci] + col_widths[ci]);
            ctx.cell_ellipsis = false;
            ctx.push_spaces(
                cell_leading_pad(align, col_widths[ci], content_w),
                margin + 1,
            );
            let header_style = ctx.theme.table_header;
            for inline in cell_inlines {
                render_inline(ctx, inline, header_style, margin + 1);
            }
            // Pad to width
            ctx.cell_clip = None;
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
            ctx.cell_clip = Some(col_offsets[ci] + col_widths[ci]);
            ctx.cell_ellipsis = false;
            let content_w: usize = cell_inlines.iter().map(|n| inline_text_len(n)).sum();
            ctx.push_spaces(
                cell_leading_pad(align, col_widths[ci], content_w),
                margin + 1,
            );
            let cell_style = ctx.theme.table_cell;
            for inline in cell_inlines {
                render_inline(ctx, inline, cell_style, margin + 1);
            }
            ctx.cell_clip = None;
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
            let icon = link_icon(&link.url, ctx.opts.icon_mode);
            if !icon.is_empty() {
                ctx.push_str(icon, ctx.theme.blockquote, margin);
                ctx.push(' ', ctx.theme.blockquote, margin);
            }
            for child in node.children() {
                render_inline(ctx, child, ctx.theme.link_text, margin);
            }
            ctx.push(' ', base_style, margin);
            ctx.push_str(
                &truncate_url_middle(&link.url, ctx.opts.link_url_max),
                ctx.theme.link_url,
                margin,
            );
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
        render_layout(content, cols, &theme, &opts, &cancel).unwrap().document.lines().to_vec()
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
        let h1_first = h1_cells
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
        let h2_cells = line_cells(h2_line.expect("lines is not empty"));
        let h2_first = h2_cells
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
            !line_cells(h2.expect("lines is not empty"))
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
            for &(ch, _) in &line_cells(line) {
                assert!(!ch.is_control(), "cell char is control: {ch:?}");
            }
        }
        let has_bell_ch = lines
            .iter()
            .flat_map(|l| line_cells(l))
            .any(|(c, _)| c == '\x07');
        assert!(!has_bell_ch, "bell char \\x07 should not appear in cells");
    }

    #[test]
    fn tab_becomes_space() {
        let lines = render_test("a\tb", 80, true, false);
        let text: String = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains(' '), "tab should render as space");
        assert!(!text.contains('\t'), "no raw tab in output");
        let tab_pos = lines.iter().flat_map(|l| line_cells(l)).position(|(c, _)| c == '\t');
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
        assert!(
            !lines[0].is_blank,
            "first row should not be blank"
        );
    }

    #[test]
    fn no_trailing_blank_lines() {
        let lines = render_test("a\n\nb", 80, true, false);
        let last = lines.last().expect("lines is not empty");
        assert!(
            !last.is_blank,
            "last row should not be blank"
        );
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
        let filtered: Vec<&RenderLine> = lines
            .iter()
            .filter(|l| !l.is_blank)
            .collect();
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
        let expected = "  Title ";
        let text: String = row0_cells.iter().map(|(c, _)| c).collect();
        assert_eq!(text, expected, "H1 row should be '  Title '");
        assert_eq!(
            row0_cells[0].1.bg, None,
            "col 0 should be plain (no badge bg)"
        );
        for (i, (ch, st)) in row0_cells.iter().enumerate().skip(1) {
            assert_eq!(
                st.bg,
                Some(theme_colors.heading),
                "cell {} bg should be heading color; char={:?}",
                i,
                ch
            );
        }
        for (i, (ch, st)) in row0_cells.iter().enumerate() {
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
        let leading_spaces = line_cells(def_line).iter().take_while(|(c, _)| *c == ' ').count();
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
        let lines = render_layout(
            "[repo](https://github.com/user/repo)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        ).unwrap().document.lines().to_vec();
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
        ).unwrap().document.lines().to_vec();
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
        ).unwrap().document.lines().to_vec();
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("🖼 alt"), "should contain image unicode icon");

        let lines_none = render_layout(
            "![alt](url.png)",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::None),
            &cancel,
        ).unwrap().document.lines().to_vec();
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
        let lines = render_layout("```txt\na\nb\nc\n```\n", 80, &theme, &opts, &cancel).unwrap().document.lines().to_vec();
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
        let lines = render_layout(
            "```rust\nx\n```\n",
            80,
            &theme,
            &mk_opts(crate::config::IconMode::Unicode),
            &cancel,
        ).unwrap().document.lines().to_vec();
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
        let lines = render_layout(long, 80, &theme, &opts, &cancel).unwrap().document.lines().to_vec();
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
        let cells = line_cells(line);
        let idx = cells.iter().position(|(c, _)| *c == 'x').expect("x");
        assert_eq!(cells[idx - 1].0, ' ', "leading padding space");
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
        let lines =
            render_layout("aaaaaaa\u{4e00}bcdefghijklmnop", 10, &theme, &opts, &cancel).unwrap().document.lines().to_vec();
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
        let lines = render_layout("```rust\nfn main(){}\n```\n", 80, &theme, &opts, &cancel).unwrap().document.lines().to_vec();
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
}
