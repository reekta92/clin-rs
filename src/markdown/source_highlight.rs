//! Source-preserving markdown highlighter for EDIT mode.
//!
//! Assigns one [`Style`] per character of each source line, matching the colors
//! that READ mode / the preview pane derive from the same [`MarkdownTheme`].
//! Code blocks inside fences are syntax-highlighted via syntect (shared cache).
//!
//! The highlighter is designed to be called per frame — the scan+cache path
//! is O(1) when the document hash hasn't changed.

use ratatui::style::{Modifier, Style};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::markdown::style::{MarkdownTheme, faint_background};

#[derive(Debug, Clone, PartialEq, Eq)]
enum LineTag {
    Inline,
    CodeBlock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineRecord {
    hash: u64,
    starts_in_fence: bool,
    tag: LineTag,
}

/// Source-preserving markdown highlighter.
///
/// Fence metadata is incrementally propagated from an exact document change.
/// Styles remain a visible-line concern of the renderer.
pub(crate) struct SourceHighlighter {
    theme: MarkdownTheme,
    ghost_syntax_enabled: bool,
    extended_features: bool,
    lines: Vec<LineRecord>,
}
impl SourceHighlighter {
    pub fn new(
        theme: &crate::app_theme::AppThemeColors,
        ghost_syntax_enabled: bool,
        extended_features: bool,
    ) -> Self {
        Self {
            theme: MarkdownTheme::from_app_theme(theme),
            ghost_syntax_enabled,
            extended_features,
            lines: Vec::new(),
        }
    }

    /// Rebuild metadata. Used only for initial/full synchronization.
    pub fn rescan(&mut self, full_doc: &[String]) {
        self.lines.clear();
        self.lines.reserve(full_doc.len());
        let mut in_fence = false;
        for line in full_doc {
            let record = line_record(line, in_fence);
            in_fence = ends_in_fence(line, &record);
            self.lines.push(record);
        }
    }

    /// Update fence metadata from one exact editor change. Propagation stops
    /// as soon as both line content and incoming fence state converge.
    pub fn apply_change(
        &mut self,
        document: &crate::editor_document::EditorDocument,
        change: crate::editor_document::DocumentChange,
    ) {
        let source = document.lines();
        let crate::editor_document::DocumentChange::Lines { old, new } = change else {
            self.rescan(source);
            return;
        };
        if old.end > self.lines.len() || new.end > source.len() {
            self.rescan(source);
            return;
        }
        self.lines.splice(
            old,
            source[new.clone()]
                .iter()
                .map(|line| line_record(line, false)),
        );
        let start = new.start.saturating_sub(1);
        let mut in_fence = if start == 0 {
            false
        } else {
            ends_in_fence(&source[start - 1], &self.lines[start - 1])
        };
        for row in start..source.len() {
            let record = line_record(&source[row], in_fence);
            in_fence = ends_in_fence(&source[row], &record);
            let converged = row >= new.end && self.lines[row] == record;
            self.lines[row] = record;
            if converged {
                break;
            }
        }
    }

    /// Return one [`Style`] per character of `line`, considering its role
    /// in the document (code block vs inline markdown).
    pub fn highlight_line(&mut self, line: &str, row: usize) -> Vec<Style> {
        if row < self.lines.len() && self.lines[row].tag == LineTag::CodeBlock {
            let mut style = self.theme.code_block;
            if let Some(bg) = self.theme.code_block_bg {
                style = style.bg(bg);
            }
            return vec![style; line.chars().count()];
        }
        self.highlight_inline(line)
    }

    /// Return whether `row` is inside a fenced code block.
    pub fn is_code_line(&self, row: usize) -> bool {
        matches!(
            self.lines.get(row),
            Some(LineRecord {
                tag: LineTag::CodeBlock,
                ..
            })
        )
    }

    fn highlight_inline(&self, line: &str) -> Vec<Style> {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return Vec::new();
        }

        // --- Structural checks (whole-line patterns) ---

        // Heading: `^#{1,6}\s`
        if let Some(level) = heading_level(&chars) {
            let mut heading_style = match level {
                1 => self.theme.h1_banner,
                2 => self.theme.h2,
                3 => self.theme.h3,
                4 => self.theme.h4,
                5 => self.theme.h5,
                _ => self.theme.h6,
            };
            if level > 1
                && let Some(fg) = heading_style.fg
            {
                heading_style = heading_style.bg(faint_background(fg));
            }
            let marker_end = level + 1; // # chars + space
            let ghost = self.ghost_syntax_enabled;
            let rest: String = chars.iter().skip(marker_end).collect();
            let rest_styles = self.inline_highlight_chars(&rest, heading_style);

            let mut styles = Vec::with_capacity(chars.len());
            for _ in 0..marker_end {
                let mut marker_style = if ghost {
                    self.theme.ghost_syntax
                } else {
                    heading_style
                };
                if let Some(bg) = heading_style.bg {
                    marker_style = marker_style.bg(bg);
                }
                styles.push(marker_style);
            }
            styles.extend(rest_styles);
            while styles.len() < chars.len() {
                styles.push(if ghost {
                    self.theme.ghost_syntax
                } else {
                    heading_style
                });
            }
            if let Some(bg) = heading_style.bg {
                for style in &mut styles {
                    *style = style.bg(bg);
                }
            }
            return styles;
        }
        // Setext underline `^=+$` or `^-{2,}$`
        if is_setext_underline(&chars) {
            let style = if self.ghost_syntax_enabled {
                self.theme.ghost_syntax
            } else {
                self.theme.hr
            };
            return vec![style; chars.len()];
        }

        // Horizontal rule `^---$` or `^***$` or `^___$`
        if is_horizontal_rule(&chars) {
            let style = if self.ghost_syntax_enabled {
                self.theme.ghost_syntax
            } else {
                self.theme.hr
            };
            return vec![style; chars.len()];
        }

        // Blockquote `^\s*>+`
        if let Some((depth, after_marker)) = blockquote_depth(&chars) {
            return self.blockquote_styles(&chars, depth, after_marker);
        }

        // Fence open/close line (``` or ~~~)
        if is_fence_marker(&chars) {
            let mut style = if self.ghost_syntax_enabled {
                self.theme.ghost_syntax
            } else {
                self.theme.code_block
            };
            if let Some(bg) = self.theme.code_block_bg {
                style = style.bg(bg);
            }
            return vec![style; chars.len()];
        }

        // Task list line
        if let Some(task_marker_end) = find_task_marker(&chars) {
            return self.task_line_styles(&chars, task_marker_end);
        }

        // List marker
        if let Some(marker_end) = find_list_marker(&chars) {
            return self.list_line_styles(&chars, marker_end);
        }

        // Description list marker
        if self.extended_features
            && let Some(marker_end) = find_description_marker(&chars)
        {
            return self.description_line_styles(&chars, marker_end);
        }

        // Footnote definition marker
        if self.extended_features
            && let Some(marker_end) = find_footnote_def_marker(&chars)
        {
            return self.footnote_def_line_styles(&chars, marker_end);
        }

        // Default: inline-highlight as paragraph text
        self.inline_highlight_chars(line, self.theme.paragraph)
    }

    /// Blockquote styles: leading `>` chars get blockquote_bar, rest get blockquote.
    fn blockquote_styles(&self, chars: &[char], _depth: usize, after_marker: usize) -> Vec<Style> {
        let ghost = self.ghost_syntax_enabled;
        let rest: String = chars.iter().skip(after_marker).collect();
        let rest_styles = self.inline_highlight_chars(&rest, self.theme.blockquote);
        let mut styles = Vec::with_capacity(chars.len());
        for _ in 0..after_marker {
            styles.push(if ghost {
                self.theme.ghost_syntax
            } else {
                self.theme.blockquote_bar
            });
        }
        styles.extend(rest_styles);
        while styles.len() < chars.len() {
            styles.push(if ghost {
                self.theme.ghost_syntax
            } else {
                self.theme.blockquote
            });
        }
        styles
    }

    /// Style a task-list line.
    fn task_line_styles(&self, chars: &[char], task_marker_end: usize) -> Vec<Style> {
        // task_marker_end points past: `\s*[-*+]\s+\[[xX ]\]\s`
        // Find the `[ ]` / `[x]` part using character indices.
        let bracket_start = chars.iter().rposition(|&ch| ch == '[').unwrap_or(0);
        let bracket_end = find_char(&chars[bracket_start..], ']')
            .map(|offset| bracket_start + offset + 1)
            .unwrap_or(task_marker_end);

        let is_checked = bracket_start + 1 < chars.len()
            && (chars[bracket_start + 1] == 'x' || chars[bracket_start + 1] == 'X');
        let task_style = if is_checked {
            self.theme.task_checked
        } else {
            self.theme.task_unchecked
        };

        let after_task = bracket_end + 1; // skip the trailing space
        let rest: String = chars.iter().skip(after_task).collect();
        let rest_styles = self.inline_highlight_chars(&rest, self.theme.paragraph);

        let ghost = self.ghost_syntax_enabled;
        let mut styles = Vec::with_capacity(chars.len());
        for (idx, _ch) in chars.iter().enumerate() {
            if idx < bracket_start {
                styles.push(if ghost {
                    self.theme.ghost_syntax
                } else {
                    self.theme.paragraph
                });
            } else if idx < bracket_end {
                // Inside brackets: only the checkmark character gets task style,
                // the brackets themselves get ghost syntax
                if idx == bracket_start || idx == bracket_end - 1 {
                    styles.push(if ghost {
                        self.theme.ghost_syntax
                    } else {
                        self.theme.paragraph
                    });
                } else {
                    styles.push(task_style);
                }
            } else if idx < after_task {
                styles.push(if ghost {
                    self.theme.ghost_syntax
                } else {
                    self.theme.paragraph
                });
            } else {
                let ri = idx - after_task;
                styles.push(rest_styles.get(ri).copied().unwrap_or(self.theme.paragraph));
            }
        }
        styles
    }

    /// Style a list line.
    fn list_line_styles(&self, chars: &[char], marker_end: usize) -> Vec<Style> {
        let ghost = self.ghost_syntax_enabled;
        let rest: String = chars.iter().skip(marker_end).collect();
        let rest_styles = self.inline_highlight_chars(&rest, self.theme.paragraph);
        let mut styles = Vec::with_capacity(chars.len());
        for _ in 0..marker_end {
            styles.push(if ghost {
                self.theme.ghost_syntax
            } else {
                self.theme.h3
            });
        }
        styles.extend(rest_styles);
        while styles.len() < chars.len() {
            styles.push(if ghost {
                self.theme.ghost_syntax
            } else {
                self.theme.paragraph
            });
        }
        styles
    }

    #[allow(clippy::collapsible_if)]
    fn inline_highlight_chars(&self, text: &str, base_style: Style) -> Vec<Style> {
        let chars: Vec<char> = text.chars().collect();
        let mut styles = vec![base_style; chars.len()];
        let mut i = 0;
        let ghost = self.ghost_syntax_enabled;

        while i < chars.len() {
            // Escape: `\X` — paragraph style on both chars (same in both modes)
            if chars[i] == '\\' && i + 1 < chars.len() {
                if self.extended_features {
                    styles[i] = self.theme.ghost_syntax;
                    styles[i + 1] = base_style;
                } else {
                    styles[i] = base_style;
                    styles[i + 1] = base_style;
                }
                i += 2;
                continue;
            }

            // Autolink `<https?://...>`
            if let Some(end) = try_autolink(&chars, i) {
                if ghost {
                    // Ghost mode: < and > get ghost, URL gets link_url
                    styles[i] = self.theme.ghost_syntax;
                    styles[i + end] = self.theme.ghost_syntax;
                    for j in 1..end {
                        styles[i + j] = self.theme.link_url;
                    }
                } else {
                    // Original: entire autolink gets link_url
                    for j in 0..=end {
                        styles[i + j] = self.theme.link_url;
                    }
                }
                i += end + 1;
                continue;
            }

            // Image `![alt](url)`
            if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
                if let Some(close_b) = find_char(&chars[i..], ']') {
                    if i + close_b + 1 < chars.len() && chars[i + close_b + 1] == '(' {
                        if ghost {
                            // Ghost: ![ and ]( and ) get ghost, alt gets link_text, url gets link_url
                            styles[i] = self.theme.ghost_syntax; // !
                            styles[i + 1] = self.theme.ghost_syntax; // [
                            for j in 2..close_b {
                                styles[i + j] = self.theme.link_text;
                            }
                            styles[i + close_b] = self.theme.ghost_syntax; // ]
                            styles[i + close_b + 1] = self.theme.ghost_syntax; // (
                            if let Some(url_end) = find_char(&chars[i + close_b + 1..], ')') {
                                let url_total = close_b + 1 + url_end;
                                for j in close_b + 2..url_total {
                                    styles[i + j] = self.theme.link_url;
                                }
                                styles[i + url_total] = self.theme.ghost_syntax; // )
                                i += url_total + 1;
                                continue;
                            }
                        } else {
                            // Original: ![alt] gets link_text, ](url) gets link_url
                            for j in 0..=close_b {
                                styles[i + j] = self.theme.link_text;
                            }
                            if let Some(url_end) = find_char(&chars[i + close_b + 1..], ')') {
                                let url_total = close_b + 1 + url_end;
                                for j in close_b + 1..=url_total {
                                    styles[i + j] = self.theme.link_url;
                                }
                                i += url_total + 1;
                                continue;
                            }
                        }
                    }
                }
            }

            // Wikilink `[[...]]`
            if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
                if let Some(end) = find_subsequence(&chars[i..], &[']', ']']) {
                    if ghost {
                        // Ghost: [[ and ]] get ghost, content gets wikilink
                        styles[i] = self.theme.ghost_syntax;
                        styles[i + 1] = self.theme.ghost_syntax;
                        styles[i + end] = self.theme.ghost_syntax;
                        styles[i + end + 1] = self.theme.ghost_syntax;

                        let mut pipe_idx = None;
                        if self.extended_features {
                            for j in 2..end {
                                if chars[i + j] == '|' {
                                    pipe_idx = Some(j);
                                    break;
                                }
                            }
                        }

                        if let Some(p) = pipe_idx {
                            for j in 2..=p {
                                styles[i + j] = self.theme.ghost_syntax;
                            }
                            for j in p + 1..end {
                                styles[i + j] = self.theme.wikilink;
                            }
                        } else {
                            for j in 2..end {
                                styles[i + j] = self.theme.wikilink;
                            }
                        }
                    } else {
                        // Original: entire wikilink gets wikilink
                        for j in 0..end + 2 {
                            styles[i + j] = self.theme.wikilink;
                        }
                    }
                    i += end + 2;
                    continue;
                }
            }

            // Footnote ref `[^...]`
            if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '^' {
                if let Some(end) = find_char(&chars[i..], ']') {
                    if ghost {
                        // Ghost: [^ and ] get ghost, content gets footnote_ref
                        styles[i] = self.theme.ghost_syntax;
                        styles[i + 1] = self.theme.ghost_syntax;
                        styles[i + end] = self.theme.ghost_syntax;
                        for j in 2..end {
                            styles[i + j] = self.theme.footnote_ref;
                        }
                    } else {
                        // Original: entire footnote ref gets footnote_ref
                        for j in 0..=end {
                            styles[i + j] = self.theme.footnote_ref;
                        }
                    }
                    i += end + 1;
                    continue;
                }
            }

            // Link `[text](url)`
            if chars[i] == '[' {
                if let Some(close_b) = find_char(&chars[i..], ']') {
                    if i + close_b + 1 < chars.len() && chars[i + close_b + 1] == '(' {
                        if ghost {
                            // Ghost: [ and ] and ( and ) get ghost, text gets link_text, url gets link_url
                            styles[i] = self.theme.ghost_syntax; // [
                            for j in 1..close_b {
                                styles[i + j] = self.theme.link_text;
                            }
                            styles[i + close_b] = self.theme.ghost_syntax; // ]
                            styles[i + close_b + 1] = self.theme.ghost_syntax; // (
                            if let Some(url_end) = find_char(&chars[i + close_b + 1..], ')') {
                                let url_total = close_b + 1 + url_end;
                                for j in close_b + 2..url_total {
                                    styles[i + j] = self.theme.link_url;
                                }
                                styles[i + url_total] = self.theme.ghost_syntax; // )
                                i += url_total + 1;
                                continue;
                            }
                        } else {
                            // Original: [text] gets link_text, ](url) gets link_url
                            for j in 0..=close_b {
                                styles[i + j] = self.theme.link_text;
                            }
                            if let Some(url_end) = find_char(&chars[i + close_b + 1..], ')') {
                                let url_total = close_b + 1 + url_end;
                                for j in close_b + 1..=url_total {
                                    styles[i + j] = self.theme.link_url;
                                }
                                i += url_total + 1;
                                continue;
                            }
                        }
                    }
                }
            }

            // Bare URL (extended feature)
            if self.extended_features && (chars[i] == 'h' || chars[i] == 'H') {
                if let Some(len) = try_bare_url(&chars, i) {
                    for j in 0..len {
                        styles[i + j] = self.theme.link_url;
                    }
                    i += len;
                    continue;
                }
            }

            // Bold italic `***` or `___` (extended feature)
            if self.extended_features
                && ((chars[i] == '*'
                    && i + 2 < chars.len()
                    && chars[i + 1] == '*'
                    && chars[i + 2] == '*')
                    || (chars[i] == '_'
                        && i + 2 < chars.len()
                        && chars[i + 1] == '_'
                        && chars[i + 2] == '_'))
            {
                let delimiter = [chars[i]; 3];
                if let Some(end) = find_subsequence(&chars[i + 3..], &delimiter) {
                    let delim_style = if ghost {
                        self.theme.ghost_syntax
                    } else {
                        base_style
                    };
                    styles[i] = delim_style;
                    styles[i + 1] = delim_style;
                    styles[i + 2] = delim_style;
                    for j in 3..end + 3 {
                        styles[i + j] = base_style.add_modifier(Modifier::BOLD | Modifier::ITALIC);
                    }
                    styles[i + end + 3] = delim_style;
                    styles[i + end + 4] = delim_style;
                    styles[i + end + 5] = delim_style;
                    i += end + 6;
                    continue;
                }
            }

            // Bold `**` or `__`
            if (chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*')
                || (chars[i] == '_' && i + 1 < chars.len() && chars[i + 1] == '_')
            {
                let delimiter = [chars[i]; 2];
                if let Some(end) = find_subsequence(&chars[i + 2..], &delimiter) {
                    // Works same in both modes: delimiters get paragraph, content gets bold
                    let delim_style = if ghost {
                        self.theme.ghost_syntax
                    } else {
                        base_style
                    };
                    styles[i] = delim_style;
                    styles[i + 1] = delim_style;
                    for j in 2..end + 2 {
                        styles[i + j] = base_style.add_modifier(Modifier::BOLD);
                    }
                    styles[i + end + 2] = delim_style;
                    styles[i + end + 3] = delim_style;
                    i += end + 4;
                    continue;
                }
            }

            // Italic `*` or `_` (not part of ** or __)
            if (chars[i] == '*' || chars[i] == '_')
                && !(i + 1 < chars.len()
                    && ((chars[i] == '*' && chars[i + 1] == '*')
                        || (chars[i] == '_' && chars[i + 1] == '_')))
            {
                if let Some(end) = find_char(&chars[i + 1..], chars[i]) {
                    // Works same in both modes: delimiters get paragraph, content gets italic
                    let delim_style = if ghost {
                        self.theme.ghost_syntax
                    } else {
                        base_style
                    };
                    styles[i] = delim_style;
                    for j in 1..end + 1 {
                        styles[i + j] = base_style.add_modifier(Modifier::ITALIC);
                    }
                    styles[i + end + 1] = delim_style;
                    i += end + 2;
                    continue;
                }
            }

            // Strikethrough `~~`
            if chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '~' {
                if let Some(end) = find_subsequence(&chars[i + 2..], &['~', '~']) {
                    // Works same in both modes: delimiters get paragraph, content gets crossed
                    let delim_style = if ghost {
                        self.theme.ghost_syntax
                    } else {
                        base_style
                    };
                    styles[i] = delim_style;
                    styles[i + 1] = delim_style;
                    for j in 2..end + 2 {
                        styles[i + j] = base_style.add_modifier(Modifier::CROSSED_OUT);
                    }
                    styles[i + end + 2] = delim_style;
                    styles[i + end + 3] = delim_style;
                    i += end + 4;
                    continue;
                }
            }

            // Inline code with multi-backticks
            if chars[i] == '`' {
                let mut bt_count = 0;
                while i + bt_count < chars.len() && chars[i + bt_count] == '`' {
                    bt_count += 1;
                }
                if let Some(end) = chars[i + bt_count..]
                    .windows(bt_count)
                    .position(|window| window.iter().all(|&ch| ch == '`'))
                {
                    let mut code_style = base_style;
                    if let Some(fg) = self.theme.code_inline.fg {
                        code_style = code_style.fg(fg);
                    }
                    if let Some(bg) = self.theme.code_inline.bg {
                        code_style = code_style.bg(bg);
                    }
                    if ghost {
                        let mut ghost_code_style = self.theme.ghost_syntax;
                        if let Some(bg) = self.theme.code_inline.bg {
                            ghost_code_style = ghost_code_style.bg(bg);
                        }
                        if let Some(fg) = self.theme.code_inline.fg {
                            ghost_code_style = ghost_code_style.fg(fg);
                        }
                        ghost_code_style = ghost_code_style.add_modifier(Modifier::DIM);
                        for j in 0..bt_count {
                            styles[i + j] = ghost_code_style;
                        }
                        for j in bt_count..bt_count + end {
                            styles[i + j] = code_style;
                        }
                        for j in bt_count + end..bt_count + end + bt_count {
                            styles[i + j] = ghost_code_style;
                        }
                    } else {
                        for j in 0..bt_count + end + bt_count {
                            styles[i + j] = code_style;
                        }
                    }
                    i += bt_count + end + bt_count;
                    continue;
                }
            }

            // Table Delimiters
            if chars[i] == '|' {
                styles[i] = self.theme.table_border;
                i += 1;
                continue;
            }

            i += 1;
        }

        styles
    }

    fn description_line_styles(&self, chars: &[char], marker_end: usize) -> Vec<Style> {
        let ghost = self.ghost_syntax_enabled;
        let rest: String = chars.iter().skip(marker_end).collect();
        let rest_styles = self.inline_highlight_chars(&rest, self.theme.paragraph);
        let mut styles = Vec::with_capacity(chars.len());
        for _ in 0..marker_end {
            styles.push(if ghost {
                self.theme.ghost_syntax
            } else {
                self.theme.h5
            });
        }
        styles.extend(rest_styles);
        while styles.len() < chars.len() {
            styles.push(if ghost {
                self.theme.ghost_syntax
            } else {
                self.theme.paragraph
            });
        }
        styles
    }

    fn footnote_def_line_styles(&self, chars: &[char], marker_end: usize) -> Vec<Style> {
        let ghost = self.ghost_syntax_enabled;
        let rest: String = chars.iter().skip(marker_end).collect();
        let rest_styles = self.inline_highlight_chars(&rest, self.theme.paragraph);
        let mut styles = Vec::with_capacity(chars.len());

        if ghost {
            let mut i = 0;
            while i < marker_end && chars[i] == ' ' {
                styles.push(self.theme.ghost_syntax);
                i += 1;
            }
            if i + 1 < marker_end && chars[i] == '[' && chars[i + 1] == '^' {
                styles.push(self.theme.ghost_syntax); // [
                styles.push(self.theme.ghost_syntax); // ^
                i += 2;
                while i < marker_end && chars[i] != ']' {
                    styles.push(self.theme.footnote_ref);
                    i += 1;
                }
                if i < marker_end && chars[i] == ']' {
                    styles.push(self.theme.ghost_syntax); // ]
                    i += 1;
                }
                if i < marker_end && chars[i] == ':' {
                    styles.push(self.theme.ghost_syntax); // :
                    i += 1;
                }
                if i < marker_end && chars[i] == ' ' {
                    styles.push(self.theme.ghost_syntax); // space
                }
            }
            while styles.len() < marker_end {
                styles.push(self.theme.ghost_syntax);
            }
        } else {
            for _ in 0..marker_end {
                styles.push(self.theme.footnote_ref);
            }
        }

        styles.extend(rest_styles);
        while styles.len() < chars.len() {
            styles.push(if ghost {
                self.theme.ghost_syntax
            } else {
                self.theme.paragraph
            });
        }
        styles
    }
}

fn line_record(line: &str, starts_in_fence: bool) -> LineRecord {
    let mut hasher = DefaultHasher::new();
    line.hash(&mut hasher);
    let tag = if starts_in_fence && !is_fence_marker(&line.chars().collect::<Vec<_>>()) {
        LineTag::CodeBlock
    } else {
        LineTag::Inline
    };
    LineRecord {
        hash: hasher.finish(),
        starts_in_fence,
        tag,
    }
}

fn ends_in_fence(line: &str, record: &LineRecord) -> bool {
    record.starts_in_fence ^ is_fence_marker(&line.chars().collect::<Vec<_>>())
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn find_char(chars: &[char], target: char) -> Option<usize> {
    chars.iter().position(|&ch| ch == target)
}

fn find_subsequence(chars: &[char], target: &[char]) -> Option<usize> {
    chars
        .windows(target.len())
        .position(|window| window == target)
}

/// Try to detect an autolink `<https?://...>` starting at position `i`.
/// Returns `Some(end_index)` where `end_index` is the position of `>` relative to `i`.
fn try_autolink(chars: &[char], i: usize) -> Option<usize> {
    if chars[i] != '<' || i + 8 >= chars.len() {
        return None;
    }
    let scheme = &chars[i + 1..];
    if !scheme.starts_with(&['h', 't', 't', 'p', ':', '/', '/'])
        && !scheme.starts_with(&['h', 't', 't', 'p', 's', ':', '/', '/'])
    {
        return None;
    }
    find_char(&chars[i..], '>')
}

/// Try to detect a bare URL `http://` or `https://` starting at position `i`.
fn try_bare_url(chars: &[char], i: usize) -> Option<usize> {
    if i > 0 && chars[i - 1].is_alphanumeric() {
        return None;
    }
    if i + 7 > chars.len() {
        return None;
    }
    let slice: String = chars[i..].iter().collect();
    if !slice.starts_with("http://") && !slice.starts_with("https://") {
        return None;
    }
    let mut len = 0;
    for &ch in &chars[i..] {
        if ch.is_whitespace() || ['<', '>', '"', '\'', '`', '[', ']', '(', ')'].contains(&ch) {
            break;
        }
        len += 1;
    }
    while len > 0 {
        let last_ch = chars[i + len - 1];
        if ['.', ',', ';', ':', '!', '?'].contains(&last_ch) {
            len -= 1;
        } else {
            break;
        }
    }
    if len > 7 { Some(len) } else { None }
}

/// Find end of description marker (up to 3 leading spaces followed by `: `).
fn find_description_marker(chars: &[char]) -> Option<usize> {
    let mut i = 0;
    while i < chars.len() && chars[i] == ' ' && i < 3 {
        i += 1;
    }
    if i + 1 < chars.len() && chars[i] == ':' && chars[i + 1] == ' ' {
        Some(i + 2)
    } else {
        None
    }
}

/// Find end of footnote definition marker (optional up to 3 leading spaces followed by `[^...]:`).
fn find_footnote_def_marker(chars: &[char]) -> Option<usize> {
    if chars.len() < 5 {
        return None;
    }
    let mut i = 0;
    while i < chars.len() && chars[i] == ' ' && i < 3 {
        i += 1;
    }
    if i + 3 < chars.len() && chars[i] == '[' && chars[i + 1] == '^' {
        let start = i;
        i += 2;
        while i < chars.len() && chars[i] != ']' {
            i += 1;
        }
        if i < chars.len() && chars[i] == ']' && i > start + 2 {
            i += 1;
            if i < chars.len() && chars[i] == ':' {
                i += 1;
                if i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
                return Some(i);
            }
        }
    }
    None
}

/// Detect heading level from `#` prefix.
fn heading_level(chars: &[char]) -> Option<usize> {
    let mut count = 0usize;
    for &ch in chars {
        if ch == '#' {
            count += 1;
        } else if ch == ' ' && count > 0 {
            break;
        } else {
            return None;
        }
    }
    if (1..=6).contains(&count) && chars.get(count) == Some(&' ') {
        Some(count)
    } else {
        None
    }
}

/// Check if the line is a setext underline.
fn is_setext_underline(chars: &[char]) -> bool {
    if chars.len() < 2 {
        return false;
    }
    let first = chars[0];
    (first == '=' || first == '-') && chars.iter().all(|&c| c == first || c.is_whitespace())
}

/// Check if the line is a horizontal rule.
fn is_horizontal_rule(chars: &[char]) -> bool {
    let trimmed: Vec<&char> = chars.iter().filter(|c| !c.is_whitespace()).collect();
    if trimmed.len() < 3 {
        return false;
    }
    let first = *trimmed[0];
    (first == '*' || first == '-' || first == '_') && trimmed.iter().all(|&&c| c == first)
}

/// Find blockquote depth. Returns (depth, index_after_last_>).
fn blockquote_depth(chars: &[char]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let mut depth = 0usize;
    while i < chars.len() && chars[i] == '>' {
        depth += 1;
        i += 1;
    }
    if depth > 0 { Some((depth, i)) } else { None }
}

/// Check if line is a fence marker (``` or ~~~ at start, ignoring up to 3 spaces).
fn is_fence_marker(chars: &[char]) -> bool {
    let s: String = chars.iter().collect();
    let trimmed = s.trim_start();
    let spaces = s.len() - trimmed.len();
    if spaces > 3 {
        return false;
    }
    (trimmed.starts_with("```") || trimmed.starts_with("~~~")) && trimmed.len() >= 3
}

/// Find end of task list marker `\s*[-*+]\s+\[[ xX]\]\s`. Returns Some(end_index).
fn find_task_marker(chars: &[char]) -> Option<usize> {
    let _s: String = chars.iter().collect();
    let mut i = 0;
    // Skip leading whitespace
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    // Bullet: [-*+]
    if i < chars.len() && (chars[i] == '-' || chars[i] == '*' || chars[i] == '+') {
        i += 1;
    } else {
        return None;
    }
    // Require at least one space
    if i < chars.len() && chars[i].is_whitespace() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
    } else {
        return None;
    }
    // `[` + optional `xX ` + `]`
    if i < chars.len() && chars[i] == '[' {
        i += 1;
    } else {
        return None;
    }
    if i < chars.len() && (chars[i] == ' ' || chars[i] == 'x' || chars[i] == 'X') {
        i += 1;
    } else {
        return None;
    }
    if i < chars.len() && chars[i] == ']' {
        i += 1;
    } else {
        return None;
    }
    // Require trailing space
    if i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    } else {
        return None;
    }
    Some(i)
}

/// Find end of list marker `^\s*[-*+]\s` or `^\s*\d+\.\s`.
fn find_list_marker(chars: &[char]) -> Option<usize> {
    let mut i = 0;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= chars.len() {
        return None;
    }
    // Unordered: [-*+]
    if chars[i] == '-' || chars[i] == '*' || chars[i] == '+' {
        i += 1;
        if i < chars.len() && chars[i].is_whitespace() {
            // Skip spaces after marker
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            return Some(i);
        }
    }
    // Ordered: \d+.
    let mut j = i;
    while j < chars.len() && chars[j].is_ascii_digit() {
        j += 1;
    }
    if j > i && j < chars.len() && chars[j] == '.' {
        j += 1;
        if j < chars.len() && chars[j].is_whitespace() {
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            return Some(j);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_theme::AppThemeColors;

    #[test]
    fn heading_levels() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);
        let doc = vec!["# H1".to_string(), "## H2".to_string()];
        hl.rescan(&doc);
        let styles_h1 = hl.highlight_line("# H1", 0);
        assert_eq!(styles_h1.len(), "# H1".chars().count());
        // With ghost_syntax=false, entire line gets heading style
        assert_eq!(styles_h1[0], hl.theme.h1_banner);
        let styles_h2 = hl.highlight_line("## H2", 1);
        assert_eq!(styles_h2.len(), "## H2".chars().count());
        let mut expected_h2 = hl.theme.h2;
        if let Some(fg) = expected_h2.fg {
            expected_h2 = expected_h2.bg(faint_background(fg));
        }
        assert_eq!(styles_h2[0], expected_h2);
        assert!(!styles_h2[0].add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn inline_code() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);
        let doc = vec!["text `code` more".to_string()];
        hl.rescan(&doc);
        let styles = hl.highlight_line("text `code` more", 0);
        let code_start = "text ".len();
        let code_end = code_start + "`code`".len();
        #[allow(clippy::needless_range_loop)]
        for i in code_start..code_end {
            assert_eq!(
                styles[i], hl.theme.code_inline,
                "char {i} should be code_inline"
            );
        }
    }

    #[test]
    fn link_split() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);
        let doc = vec!["a [text](url) b".to_string()];
        hl.rescan(&doc);
        let styles = hl.highlight_line("a [text](url) b", 0);
        let link_text_start = "a ".len();
        assert_eq!(styles[link_text_start], hl.theme.link_text);
        let url_start = link_text_start + "[text]".len();
        assert_eq!(styles[url_start], hl.theme.link_url);
    }

    #[test]
    fn rescan_is_idempotent() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);
        let doc = vec!["line".to_string()];
        hl.rescan(&doc);
        let records = hl.lines.clone();
        hl.rescan(&doc);
        assert_eq!(hl.lines, records);
    }

    #[test]
    fn task_list_detection() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);
        let doc = vec!["- [ ] task".to_string(), "- [x] done".to_string()];
        hl.rescan(&doc);
        let styles = hl.highlight_line("- [ ] task", 0);
        // With ghost_syntax=false: brackets get paragraph style, checkmark gets task style
        let bracket_start = "- ".len(); // index 2 = '['
        let checkmark_idx = bracket_start + 1; // index 3 = ' ' (or 'x')
        // Bracket should be paragraph
        assert_eq!(
            styles[bracket_start], hl.theme.paragraph,
            "bracket should be paragraph"
        );
        // Checkmark should be task_unchecked
        assert_eq!(
            styles[checkmark_idx], hl.theme.task_unchecked,
            "unchecked checkmark"
        );
        let styles2 = hl.highlight_line("- [x] done", 1);
        assert_eq!(
            styles2[bracket_start], hl.theme.paragraph,
            "bracket should be paragraph"
        );
        assert_eq!(
            styles2[checkmark_idx], hl.theme.task_checked,
            "checked checkmark"
        );
    }
    #[test]
    fn code_block_fence_highlighting() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);
        let doc = vec![
            "```rust".to_string(),
            "fn main() {}".to_string(),
            "```".to_string(),
        ];
        hl.rescan(&doc);
        let styles_fence_open = hl.highlight_line("```rust", 0);
        let mut expected_fence_style = hl.theme.code_block;
        if let Some(bg) = hl.theme.code_block_bg {
            expected_fence_style = expected_fence_style.bg(bg);
        }
        assert_eq!(styles_fence_open[0], expected_fence_style);

        let styles_interior = hl.highlight_line("fn main() {}", 1);
        assert_eq!(styles_interior.len(), "fn main() {}".chars().count());
        for style in styles_interior {
            assert_eq!(style, expected_fence_style);
        }

        let styles_fence_close = hl.highlight_line("```", 2);
        assert_eq!(styles_fence_close[0], expected_fence_style);
    }

    #[test]
    fn nested_formatting_and_custom_features() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);

        // 1. Heading 1 styles exactly its source characters.
        let doc1 = vec!["# H1".to_string()];
        hl.rescan(&doc1);
        let styles_h1 = hl.highlight_line("# H1", 0);
        assert_eq!(styles_h1.len(), "# H1".chars().count());
        assert_eq!(styles_h1[0], hl.theme.h1_banner);

        // 2. Bold nested in Heading 1
        let doc2 = vec!["# H1 **bold**".to_string()];
        hl.rescan(&doc2);
        let styles_nested = hl.highlight_line("# H1 **bold**", 0);
        // `# H1 ` has length 5. `**` bold delimiter style is heading style (h1_banner)
        assert_eq!(styles_nested[5], hl.theme.h1_banner);
        // `bold` is indices 7..11. It should have h1_banner style + BOLD modifier
        let expected_bold = hl.theme.h1_banner.add_modifier(Modifier::BOLD);
        assert_eq!(styles_nested[7], expected_bold);

        // 3. Pipe character (table delimiter)
        let doc3 = vec!["| col |".to_string()];
        hl.rescan(&doc3);
        let styles_table = hl.highlight_line("| col |", 0);
        assert_eq!(styles_table[0], hl.theme.table_border);
        assert_eq!(styles_table[6], hl.theme.table_border);

        // 4. Blockquote nested formatting
        let doc4 = vec!["> **quote**".to_string()];
        hl.rescan(&doc4);
        let styles_bq = hl.highlight_line("> **quote**", 0);
        assert_eq!(styles_bq[0], hl.theme.blockquote_bar); // marker
        // `quote` is nested. It should have blockquote style + BOLD modifier
        let expected_bq_bold = hl.theme.blockquote.add_modifier(Modifier::BOLD);
        assert_eq!(styles_bq[4], expected_bq_bold);

        // 5. List marker styling
        let doc5 = vec!["- item".to_string()];
        hl.rescan(&doc5);
        let styles_list = hl.highlight_line("- item", 0);
        assert_eq!(styles_list[0], hl.theme.h3); // list marker
        assert_eq!(styles_list[2], hl.theme.paragraph); // text

        // 6. Heading 2 nested formatting retaining background
        let doc6 = vec!["## H2 *italic*".to_string()];
        hl.rescan(&doc6);
        let styles_h2_nested = hl.highlight_line("## H2 *italic*", 0);
        let expected_h2 = hl.theme.h2;
        let mut expected_h2_bg = expected_h2;
        if let Some(fg) = expected_h2.fg {
            expected_h2_bg = expected_h2.bg(faint_background(fg));
        }
        if let Some(bg) = expected_h2_bg.bg {
            // Verify that the delimiter * (index 6) has the background color
            assert_eq!(styles_h2_nested[6].bg, Some(bg));
            // Verify that the italic text 'italic' (index 7) has the background color
            assert_eq!(styles_h2_nested[7].bg, Some(bg));
        }
    }

    #[test]
    fn unicode_inside_bold_delimiter_uses_character_offsets() {
        let colors = AppThemeColors::default();
        let mut highlighter = SourceHighlighter::new(&colors, false, false);
        let line = "**boéld**";
        highlighter.rescan(&[line.to_string()]);

        let styles = highlighter.highlight_line(line, 0);
        assert_eq!(styles.len(), line.chars().count());
        assert_eq!(
            styles[2],
            highlighter.theme.paragraph.add_modifier(Modifier::BOLD)
        );
    }

    #[test]
    fn url_import_unicode_markdown_highlights_without_panicking() {
        let colors = AppThemeColors::default();
        let mut highlighter = SourceHighlighter::new(&colors, true, true);
        let document = [
            "**Résumé d'été**",
            "*café* and ~~façade~~",
            "[naïve](https://example.test/café)",
            "![café](https://example.test/image-é.png)",
            "[[café|Résumé]] and [^référence]",
            "`const café = 1;`",
            "<https://example.test/café>",
            "- [x] café",
        ];
        let lines = document.iter().map(ToString::to_string).collect::<Vec<_>>();
        highlighter.rescan(&lines);

        for (row, line) in document.iter().enumerate() {
            assert_eq!(
                highlighter.highlight_line(line, row).len(),
                line.chars().count(),
                "style count for {line:?}"
            );
        }
    }

    #[test]
    fn code_block_fence_highlighting_retains_bg() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false, false);
        let doc = vec![
            "```rust".to_string(),
            "fn main() {}".to_string(),
            "```".to_string(),
        ];
        hl.rescan(&doc);
        let styles_fence_open = hl.highlight_line("```rust", 0);
        let mut expected_fence_style = hl.theme.code_block;
        if let Some(bg) = hl.theme.code_block_bg {
            expected_fence_style = expected_fence_style.bg(bg);
        }
        assert_eq!(styles_fence_open[0], expected_fence_style);
    }

    #[test]
    fn extended_markdown_features() {
        let colors = AppThemeColors::default();

        // Test with extended_features = true
        let mut hl = SourceHighlighter::new(&colors, false, true);

        // 1. Multi-backtick and background fix
        let doc = vec!["``code``".to_string()];
        hl.rescan(&doc);
        let styles = hl.highlight_line("``code``", 0);
        let mut expected_code = hl.theme.code_inline;
        if let Some(bg) = hl.theme.code_inline.bg {
            expected_code = expected_code.bg(bg);
        }
        // Ghost is false: so delimiters and content all get expected_code style
        assert_eq!(styles[0], expected_code);
        assert_eq!(styles[2], expected_code);

        // Multi-backticks with ghost = true
        let mut hl_ghost = SourceHighlighter::new(&colors, true, true);
        hl_ghost.rescan(&doc);
        let styles_ghost = hl_ghost.highlight_line("``code``", 0);
        let mut expected_ghost_code = hl_ghost.theme.ghost_syntax;
        if let Some(bg) = hl_ghost.theme.code_inline.bg {
            expected_ghost_code = expected_ghost_code.bg(bg);
        }
        if let Some(fg) = hl_ghost.theme.code_inline.fg {
            expected_ghost_code = expected_ghost_code.fg(fg);
        }
        expected_ghost_code = expected_ghost_code.add_modifier(Modifier::DIM);
        assert_eq!(styles_ghost[0], expected_ghost_code);
        assert_eq!(styles_ghost[2], expected_code);

        // 2. Bare URL
        let doc_url = vec!["Visit https://google.com now".to_string()];
        hl.rescan(&doc_url);
        let styles_url = hl.highlight_line("Visit https://google.com now", 0);
        let url_start = "Visit ".len();
        assert_eq!(styles_url[url_start], hl.theme.link_url);
        assert_eq!(
            styles_url[url_start + "https://google.com".len() - 1],
            hl.theme.link_url
        );
        assert_eq!(
            styles_url[url_start + "https://google.com".len()],
            hl.theme.paragraph
        );

        // 3. Bold italic nested style
        let doc_bi = vec!["***bold italic***".to_string()];
        hl.rescan(&doc_bi);
        let styles_bi = hl.highlight_line("***bold italic***", 0);
        let expected_bi = hl
            .theme
            .paragraph
            .add_modifier(Modifier::BOLD | Modifier::ITALIC);
        assert_eq!(styles_bi[3], expected_bi);

        // 4. Dimmed escapes
        let doc_esc = vec!["\\*".to_string()];
        hl.rescan(&doc_esc);
        let styles_esc = hl.highlight_line("\\*", 0);
        // With ghost=false but extended=true: escape backslash is themed.ghost_syntax
        assert_eq!(styles_esc[0], hl.theme.ghost_syntax);
        assert_eq!(styles_esc[1], hl.theme.paragraph);

        // 5. Description list markers
        let doc_desc = vec![": definition".to_string()];
        hl.rescan(&doc_desc);
        let styles_desc = hl.highlight_line(": definition", 0);
        assert_eq!(styles_desc[0], hl.theme.h5);
        assert_eq!(styles_desc[1], hl.theme.h5);
        assert_eq!(styles_desc[2], hl.theme.paragraph);

        // 6. Wikilink syntax separation
        let doc_wiki = vec!["[[link|title]]".to_string()];
        hl_ghost.rescan(&doc_wiki);
        let styles_wiki = hl_ghost.highlight_line("[[link|title]]", 0);
        // [[ and link| get ghost syntax
        assert_eq!(styles_wiki[0], hl_ghost.theme.ghost_syntax);
        assert_eq!(styles_wiki[6], hl_ghost.theme.ghost_syntax); // '|'
        // title gets wikilink style
        assert_eq!(styles_wiki[7], hl_ghost.theme.wikilink);

        // 7. Footnote definitions
        let doc_fn = vec!["[^1]: footnote text".to_string()];
        hl.rescan(&doc_fn);
        let styles_fn = hl.highlight_line("[^1]: footnote text", 0);
        assert_eq!(styles_fn[0], hl.theme.footnote_ref);
        assert_eq!(styles_fn[5], hl.theme.footnote_ref);
        assert_eq!(styles_fn[6], hl.theme.paragraph);
    }
}
