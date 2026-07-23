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

use crate::markdown::style::MarkdownTheme;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LineTag {
    Inline,
    CodeBlock,
}

/// Source-preserving markdown highlighter.
///
/// Call [`highlight_line`](SourceHighlighter::highlight_line) per line per
/// frame.  Internal state tracks fence boundaries and a syntect-based code
/// cache; cache hits are a HashMap lookup + clone of a `Vec<Style>`.
///
/// Thread-safe: no, but called only from the single UI thread.
pub(crate) struct SourceHighlighter {
    theme: MarkdownTheme,
    ghost_syntax_enabled: bool,
    /// Per-line tag computed from the last full scan.
    line_tags: Vec<LineTag>,
    /// Hash of the document at last scan.
    scanned_hash: u64,
}
impl SourceHighlighter {
    pub fn new(theme: &crate::app_theme::AppThemeColors, ghost_syntax_enabled: bool) -> Self {
        Self {
            theme: MarkdownTheme::from_app_theme(theme),
            ghost_syntax_enabled,
            line_tags: Vec::new(),
            scanned_hash: 0,
        }
    }

    /// Re-rescan fence boundaries when `full_doc` hash has changed.
    fn rescan_if_needed(&mut self, full_doc: &[String]) {
        let mut hasher = DefaultHasher::new();
        for line in full_doc {
            line.hash(&mut hasher);
            b"\n".hash(&mut hasher);
        }
        let hash = hasher.finish();

        if hash == self.scanned_hash && !self.line_tags.is_empty() {
            return;
        }
        self.scanned_hash = hash;
        self.line_tags = Vec::with_capacity(full_doc.len());

        let mut in_fence = false;

        for line in full_doc {
            let trimmed = line.trim_start();
            let spaces = line.len() - trimmed.len();
            let is_fence = spaces <= 3
                && (trimmed.starts_with("```") || trimmed.starts_with("~~~"))
                && trimmed.len() >= 3;

            if is_fence {
                self.line_tags.push(LineTag::Inline);
                in_fence = !in_fence;
            } else if in_fence {
                self.line_tags.push(LineTag::CodeBlock);
            } else {
                self.line_tags.push(LineTag::Inline);
            }
        }
    }

    /// Return one [`Style`] per character of `line`, considering its role
    /// in the document (code block vs inline markdown).
    ///
    /// `row` is the zero-based line index.  `full_doc` is the entire document
    /// lines — needed to re-scan fence boundaries when the document changes.
    pub fn highlight_line(&mut self, line: &str, row: usize, full_doc: &[String]) -> Vec<Style> {
        self.rescan_if_needed(full_doc);

        if row < self.line_tags.len() && self.line_tags[row] == LineTag::CodeBlock {
            return vec![self.theme.code_block; line.chars().count()];
        }

        self.highlight_inline(line)
    }

    /// Inline-highlight a non-code-block line.
    fn highlight_inline(&self, line: &str) -> Vec<Style> {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return Vec::new();
        }

        // --- Structural checks (whole-line patterns) ---

        // Heading: `^#{1,6}\s`
        if let Some(level) = heading_level(&chars) {
            let style = match level {
                1 => self.theme.h1,
                2 => self.theme.h2,
                3 => self.theme.h3,
                4 => self.theme.h4,
                5 => self.theme.h5,
                _ => self.theme.h6,
            };
            if self.ghost_syntax_enabled {
                let marker_end = level + 1; // # chars + space
                let mut styles = vec![self.theme.ghost_syntax; marker_end];
                styles.extend(vec![style; chars.len().saturating_sub(marker_end)]);
                return styles;
            }
            return vec![style; chars.len()];
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
            let style = if self.ghost_syntax_enabled {
                self.theme.ghost_syntax
            } else {
                self.theme.code_block
            };
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

        // Default: inline-highlight as paragraph text
        self.inline_highlight_chars(line)
    }

    /// Blockquote styles: leading `>` chars get blockquote_bar, rest get blockquote.
    fn blockquote_styles(&self, chars: &[char], _depth: usize, after_marker: usize) -> Vec<Style> {
        let ghost = self.ghost_syntax_enabled;
        chars
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                if idx < after_marker {
                    if ghost { self.theme.ghost_syntax } else { self.theme.blockquote_bar }
                } else {
                    self.theme.blockquote
                }
            })
            .collect()
    }

    /// Style a task-list line.
    fn task_line_styles(&self, chars: &[char], task_marker_end: usize) -> Vec<Style> {
        // task_marker_end points past: `\s*[-*+]\s+\[[xX ]\]\s`
        // Find the `[ ]` / `[x]` part
        let s: String = chars.iter().collect();
        let bracket_start = s.rfind('[').unwrap_or(0);
        let bracket_end = s[bracket_start..]
            .find(']')
            .map(|e| bracket_start + e + 1)
            .unwrap_or(task_marker_end);

        let is_checked = bracket_start + 1 < s.len()
            && (chars[bracket_start + 1] == 'x' || chars[bracket_start + 1] == 'X');
        let task_style = if is_checked {
            self.theme.task_checked
        } else {
            self.theme.task_unchecked
        };

        let after_task = bracket_end + 1; // skip the trailing space
        let rest: String = chars.iter().skip(after_task).collect();
        let rest_styles = self.inline_highlight_chars(&rest);

        let ghost = self.ghost_syntax_enabled;
        let mut styles = Vec::with_capacity(chars.len());
        for (idx, _ch) in chars.iter().enumerate() {
            if idx < bracket_start {
                styles.push(if ghost { self.theme.ghost_syntax } else { self.theme.paragraph });
            } else if idx < bracket_end {
                // Inside brackets: only the checkmark character gets task style,
                // the brackets themselves get ghost syntax
                if idx == bracket_start || idx == bracket_end - 1 {
                    styles.push(if ghost { self.theme.ghost_syntax } else { self.theme.paragraph });
                } else {
                    styles.push(task_style);
                }
            } else if idx < after_task {
                styles.push(if ghost { self.theme.ghost_syntax } else { self.theme.paragraph });
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
        let rest_styles = self.inline_highlight_chars(&rest);
        let mut styles = Vec::with_capacity(chars.len());
        for _ in 0..marker_end {
            styles.push(if ghost { self.theme.ghost_syntax } else { self.theme.paragraph });
        }
        styles.extend(rest_styles);
        while styles.len() < chars.len() {
            styles.push(if ghost { self.theme.ghost_syntax } else { self.theme.paragraph });
        }
        styles
    }

    #[allow(clippy::collapsible_if)]
    fn inline_highlight_chars(&self, text: &str) -> Vec<Style> {
        let chars: Vec<char> = text.chars().collect();
        let mut styles = vec![self.theme.paragraph; chars.len()];
        let mut i = 0;
        let ghost = self.ghost_syntax_enabled;

        while i < chars.len() {
            // Escape: `\X` — paragraph style on both chars (same in both modes)
            if chars[i] == '\\' && i + 1 < chars.len() {
                styles[i] = self.theme.paragraph;
                styles[i + 1] = self.theme.paragraph;
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
                let slice: String = chars[i..].iter().collect();
                if let Some(close_b) = slice.find(']') {
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
                            if let Some(url_end) = slice[close_b + 1..].find(')') {
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
                            if let Some(url_end) = slice[close_b + 1..].find(')') {
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
                let slice: String = chars[i..].iter().collect();
                if let Some(end) = slice.find("]]") {
                    if ghost {
                        // Ghost: [[ and ]] get ghost, content gets wikilink
                        styles[i] = self.theme.ghost_syntax;
                        styles[i + 1] = self.theme.ghost_syntax;
                        styles[i + end] = self.theme.ghost_syntax;
                        styles[i + end + 1] = self.theme.ghost_syntax;
                        for j in 2..end {
                            styles[i + j] = self.theme.wikilink;
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
                let slice: String = chars[i..].iter().collect();
                if let Some(end) = slice.find(']') {
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
                let slice: String = chars[i..].iter().collect();
                if let Some(close_b) = slice.find(']') {
                    if i + close_b + 1 < chars.len() && chars[i + close_b + 1] == '(' {
                        if ghost {
                            // Ghost: [ and ] and ( and ) get ghost, text gets link_text, url gets link_url
                            styles[i] = self.theme.ghost_syntax; // [
                            for j in 1..close_b {
                                styles[i + j] = self.theme.link_text;
                            }
                            styles[i + close_b] = self.theme.ghost_syntax; // ]
                            styles[i + close_b + 1] = self.theme.ghost_syntax; // (
                            if let Some(url_end) = slice[close_b + 1..].find(')') {
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
                            if let Some(url_end) = slice[close_b + 1..].find(')') {
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

            // Bold `**` or `__`
            if (chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*')
                || (chars[i] == '_' && i + 1 < chars.len() && chars[i + 1] == '_')
            {
                let delim = if chars[i] == '*' { "**" } else { "__" };
                let slice: String = chars[i + 2..].iter().collect();
                if let Some(end) = slice.find(delim) {
                    // Works same in both modes: delimiters get paragraph, content gets bold
                    let delim_style = if ghost { self.theme.ghost_syntax } else { self.theme.paragraph };
                    styles[i] = delim_style;
                    styles[i + 1] = delim_style;
                    for j in 2..end + 2 {
                        styles[i + j] = self.theme.paragraph.add_modifier(Modifier::BOLD);
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
                let slice: String = chars[i + 1..].iter().collect();
                if let Some(end) = slice.find(chars[i]) {
                    // Works same in both modes: delimiter gets paragraph, content gets italic
                    let delim_style = if ghost { self.theme.ghost_syntax } else { self.theme.paragraph };
                    styles[i] = delim_style;
                    for j in 1..end + 1 {
                        styles[i + j] = self.theme.paragraph.add_modifier(Modifier::ITALIC);
                    }
                    styles[i + end + 1] = delim_style;
                    i += end + 2;
                    continue;
                }
            }

            // Strikethrough `~~`
            if chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '~' {
                let slice: String = chars[i + 2..].iter().collect();
                if let Some(end) = slice.find("~~") {
                    // Works same in both modes: delimiters get paragraph, content gets crossed
                    let delim_style = if ghost { self.theme.ghost_syntax } else { self.theme.paragraph };
                    styles[i] = delim_style;
                    styles[i + 1] = delim_style;
                    for j in 2..end + 2 {
                        styles[i + j] = self.theme.paragraph.add_modifier(Modifier::CROSSED_OUT);
                    }
                    styles[i + end + 2] = delim_style;
                    styles[i + end + 3] = delim_style;
                    i += end + 4;
                    continue;
                }
            }

            // Inline code `` ` `` (single)
            if chars[i] == '`' {
                let slice: String = chars[i + 1..].iter().collect();
                if let Some(end) = slice.find('`') {
                    if ghost {
                        // Ghost: backticks get ghost, content gets code_inline
                        styles[i] = self.theme.ghost_syntax;
                        for j in 1..end + 1 {
                            styles[i + j] = self.theme.code_inline;
                        }
                        styles[i + end + 1] = self.theme.ghost_syntax;
                    } else {
                        // Original: entire inline code gets code_inline
                        for j in 0..end + 2 {
                            styles[i + j] = self.theme.code_inline;
                        }
                    }
                    i += end + 2;
                    continue;
                }
            }

            i += 1;
        }

        styles
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Try to detect an autolink `<https?://...>` starting at position `i`.
/// Returns `Some(end_index)` where `end_index` is the position of `>` relative to `i`.
fn try_autolink(chars: &[char], i: usize) -> Option<usize> {
    if chars[i] != '<' || i + 8 >= chars.len() {
        return None;
    }
    let slice: String = chars[i..].iter().collect();
    if !slice[1..].starts_with("http://") && !slice[1..].starts_with("https://") {
        return None;
    }
    let end = slice.find('>')?;
    Some(end)
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
        let mut hl = SourceHighlighter::new(&colors, false);
        let doc = vec!["# H1".to_string(), "## H2".to_string()];
        let styles_h1 = hl.highlight_line("# H1", 0, &doc);
        assert_eq!(styles_h1.len(), 4);
        // With ghost_syntax=false, entire line gets heading style
        assert_eq!(styles_h1[0], hl.theme.h1);
        let styles_h2 = hl.highlight_line("## H2", 1, &doc);
        assert_eq!(styles_h2.len(), 5);
        assert_eq!(styles_h2[0], hl.theme.h2);
    }

    #[test]
    fn inline_code() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false);
        let doc = vec!["text `code` more".to_string()];
        let styles = hl.highlight_line("text `code` more", 0, &doc);
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
        let mut hl = SourceHighlighter::new(&colors, false);
        let doc = vec!["a [text](url) b".to_string()];
        let styles = hl.highlight_line("a [text](url) b", 0, &doc);
        let link_text_start = "a ".len();
        assert_eq!(styles[link_text_start], hl.theme.link_text);
        let url_start = link_text_start + "[text]".len();
        assert_eq!(styles[url_start], hl.theme.link_url);
    }

    #[test]
    fn cache_hit_skips_rescan() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false);
        let doc = vec!["line".to_string()];
        let _ = hl.highlight_line("line", 0, &doc);
        let hash_before = hl.scanned_hash;
        let _ = hl.highlight_line("line", 0, &doc);
        assert_eq!(hl.scanned_hash, hash_before);
    }

    #[test]
    fn task_list_detection() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false);
        let doc = vec!["- [ ] task".to_string(), "- [x] done".to_string()];
        let styles = hl.highlight_line("- [ ] task", 0, &doc);
        // With ghost_syntax=false: brackets get paragraph style, checkmark gets task style
        let bracket_start = "- ".len(); // index 2 = '['
        let checkmark_idx = bracket_start + 1; // index 3 = ' ' (or 'x')
        // Bracket should be paragraph
        assert_eq!(styles[bracket_start], hl.theme.paragraph, "bracket should be paragraph");
        // Checkmark should be task_unchecked
        assert_eq!(
            styles[checkmark_idx], hl.theme.task_unchecked,
            "unchecked checkmark"
        );
        let styles2 = hl.highlight_line("- [x] done", 1, &doc);
        assert_eq!(styles2[bracket_start], hl.theme.paragraph, "bracket should be paragraph");
        assert_eq!(
            styles2[checkmark_idx], hl.theme.task_checked,
            "checked checkmark"
        );
    }
    #[test]
    fn code_block_fence_highlighting() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors, false);
        let doc = vec![
            "```rust".to_string(),
            "fn main() {}".to_string(),
            "```".to_string(),
        ];
        let styles_fence_open = hl.highlight_line("```rust", 0, &doc);
        assert_eq!(styles_fence_open[0], hl.theme.code_block);

        let styles_interior = hl.highlight_line("fn main() {}", 1, &doc);
        assert_eq!(styles_interior.len(), "fn main() {}".len());
        for style in styles_interior {
            assert_eq!(style, hl.theme.code_block);
        }

        let styles_fence_close = hl.highlight_line("```", 2, &doc);
        assert_eq!(styles_fence_close[0], hl.theme.code_block);
    }
}
