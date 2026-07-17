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

use crate::markdown::builtin;
use crate::markdown::style::MarkdownTheme;
#[derive(Debug, Clone, PartialEq, Eq)]
enum LineTag {
    Inline,
    CodeBlock {
        block_start: usize,
        local: usize,
        lang: String,
        block_hash: u64,
    },
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
    syntax_set: &'static syntect::parsing::SyntaxSet,
    theme_set: &'static syntect::highlighting::ThemeSet,
    code_theme: String,
    /// Global syntect cache keyed by (lang, content-hash).
    code_cache: std::collections::HashMap<(String, u64), Vec<Vec<Style>>>,
    /// Per-line tag computed from the last full scan.
    line_tags: Vec<LineTag>,
    /// Hash of the document at last scan.
    scanned_hash: u64,
}

impl SourceHighlighter {
    pub fn new(theme: &crate::app_theme::AppThemeColors) -> Self {
        let syntax_set: &'static syntect::parsing::SyntaxSet = &builtin::SYNTAX_SET;
        let theme_set: &'static syntect::highlighting::ThemeSet = &builtin::THEME_SET;
        Self {
            theme: MarkdownTheme::from_app_theme(theme),
            syntax_set,
            theme_set,
            code_theme: builtin::default_code_theme().to_string(),
            code_cache: std::collections::HashMap::new(),
            line_tags: Vec::new(),
            scanned_hash: 0,
        }
    }

    /// Re-rescan fence boundaries when `full_doc` hash has changed.
    fn rescan_if_needed(&mut self, full_doc: &[String]) {
        let mut hasher = DefaultHasher::new();
        full_doc.join("\n").hash(&mut hasher);
        let hash = hasher.finish();

        if hash == self.scanned_hash && !self.line_tags.is_empty() {
            return;
        }
        self.scanned_hash = hash;
        self.line_tags = Vec::with_capacity(full_doc.len());

        // Walk lines tracking ``` / ~~~ fence state.
        let mut in_fence: Option<(usize, String)> = None;
        let mut fence_content: Vec<String> = Vec::new();

        for (i, line) in full_doc.iter().enumerate() {
            if let Some((start_line, ref lang)) = in_fence {
                let trimmed = line.trim();
                if is_fence_line(trimmed) {
                    // Close fence — cache the block content
                    let content_hash = {
                        let mut h = DefaultHasher::new();
                        fence_content.join("\n").hash(&mut h);
                        h.finish()
                    };
                    let key = (lang.clone(), content_hash);
                    if !self.code_cache.contains_key(&key) {
                        self.highlight_code_block(lang, &fence_content, &key);
                    }
                    // Tag fence-open line as inline
                    self.line_tags.push(LineTag::Inline);
                    // Tag interior lines
                    for (local, _) in fence_content.iter().enumerate() {
                        self.line_tags.push(LineTag::CodeBlock {
                            block_start: start_line,
                            local,
                            lang: lang.clone(),
                            block_hash: content_hash,
                        });
                    }
                    // Tag close-fence line as inline
                    self.line_tags.push(LineTag::Inline);
                    in_fence = None;
                    fence_content.clear();
                } else {
                    fence_content.push(line.clone());
                }
            } else {
                let trimmed = line.trim_start();
                let spaces = line.len() - trimmed.len();
                if spaces <= 3
                    && let Some(rest) = trimmed
                        .strip_prefix("```")
                        .or_else(|| trimmed.strip_prefix("~~~"))
                {
                    let lang = rest.trim().to_string();
                    in_fence = Some((i, lang));
                    fence_content = Vec::new();
                    continue;
                }
                self.line_tags.push(LineTag::Inline);
            }
        }

        // If still inside an unclosed fence, tag buffered content as inline
        if in_fence.is_some() {
            // The opening fence line was tagged inline (via `continue` above)
            // Content lines were never tagged — already skipped in the fence branch
            // without pushing. We need to tag them now as Inline.
            for _ in 0..fence_content.len() {
                self.line_tags.push(LineTag::Inline);
            }
        }
    }

    /// Run syntect on a code block and store in cache.
    fn highlight_code_block(&mut self, lang: &str, lines: &[String], key: &(String, u64)) {
        use syntect::easy::HighlightLines;
        use syntect::highlighting::Theme;

        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        let theme: &Theme = self
            .theme_set
            .themes
            .get(&self.code_theme)
            .unwrap_or_else(|| {
                self.theme_set
                    .themes
                    .get(builtin::default_code_theme())
                    .expect("default code theme should exist")
            });
        let mut hl = HighlightLines::new(syntax, theme);
        let mut result = Vec::with_capacity(lines.len());
        for line in lines {
            let ranges = hl.highlight_line(line, self.syntax_set).unwrap_or_default();
            let styles: Vec<Style> = ranges
                .iter()
                .flat_map(|(style, text)| {
                    let s = builtin::syntect_style_to_ratatui(*style);
                    text.chars().map(move |_| s)
                })
                .collect();
            result.push(styles);
        }
        self.code_cache.insert(key.clone(), result);
    }

    /// Return one [`Style`] per character of `line`, considering its role
    /// in the document (code block vs inline markdown).
    ///
    /// `row` is the zero-based line index.  `full_doc` is the entire document
    /// lines — needed to re-scan fence boundaries when the document changes.
    pub fn highlight_line(&mut self, line: &str, row: usize, full_doc: &[String]) -> Vec<Style> {
        self.rescan_if_needed(full_doc);

        if row < self.line_tags.len()
            && let LineTag::CodeBlock {
                block_start: _,
                local,
                lang,
                block_hash,
            } = &self.line_tags[row]
            && let Some(cached) = self.code_cache.get(&(lang.clone(), *block_hash))
            && let Some(styles) = cached.get(*local)
        {
            return styles.clone();
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
            return vec![style; chars.len()];
        }

        // Setext underline `^=+$` or `^-{2,}$`
        if is_setext_underline(&chars) {
            return vec![self.theme.hr; chars.len()];
        }

        // HR line: `(\s*\*){3,}\s*` | `(\s*-){3,}\s*` | `(\s*_){3,}\s*`
        if is_horizontal_rule(&chars) {
            return vec![self.theme.hr; chars.len()];
        }

        // Blockquote `^\s*>+`
        if let Some((depth, after_marker)) = blockquote_depth(&chars) {
            return self.blockquote_styles(&chars, depth, after_marker);
        }

        // Fence open/close line (``` or ~~~)
        if is_fence_marker(&chars) {
            return vec![self.theme.code_inline; chars.len()];
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
        chars
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                if idx < after_marker {
                    self.theme.blockquote_bar
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

        let mut styles = Vec::with_capacity(chars.len());
        for (idx, _ch) in chars.iter().enumerate() {
            if idx < bracket_start {
                styles.push(self.theme.paragraph);
            } else if idx < bracket_end {
                styles.push(task_style);
            } else if idx < after_task {
                styles.push(self.theme.paragraph);
            } else {
                let ri = idx - after_task;
                styles.push(rest_styles.get(ri).copied().unwrap_or(self.theme.paragraph));
            }
        }
        styles
    }

    /// Style a list line.
    fn list_line_styles(&self, chars: &[char], marker_end: usize) -> Vec<Style> {
        let rest: String = chars.iter().skip(marker_end).collect();
        let rest_styles = self.inline_highlight_chars(&rest);
        let mut styles = Vec::with_capacity(chars.len());
        for _ in 0..marker_end {
            styles.push(self.theme.paragraph);
        }
        styles.extend(rest_styles);
        while styles.len() < chars.len() {
            styles.push(self.theme.paragraph);
        }
        styles
    }

    #[allow(clippy::collapsible_if)]
    fn inline_highlight_chars(&self, text: &str) -> Vec<Style> {
        let chars: Vec<char> = text.chars().collect();
        let mut styles = vec![self.theme.paragraph; chars.len()];
        let mut i = 0;

        while i < chars.len() {
            // Escape: `\X` — paragraph style on both chars
            if chars[i] == '\\' && i + 1 < chars.len() {
                styles[i] = self.theme.paragraph;
                styles[i + 1] = self.theme.paragraph;
                i += 2;
                continue;
            }

            // Autolink `<https?://...>`
            if let Some(end) = try_autolink(&chars, i) {
                for j in 0..=end {
                    styles[i + j] = self.theme.link_url;
                }
                i += end + 1;
                continue;
            }

            // Image `![alt](url)`
            if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
                let slice: String = chars[i..].iter().collect();
                if let Some(close_b) = slice.find(']') {
                    if i + close_b + 1 < chars.len() && chars[i + close_b + 1] == '(' {
                        for j in 0..=close_b {
                            styles[i + j] = self.theme.link_text;
                        }
                        if let Some(url_end) = slice[close_b + 1..].find(')') {
                            let img_total = close_b + 1 + url_end;
                            for j in close_b + 1..=img_total {
                                styles[i + j] = self.theme.link_url;
                            }
                            i += img_total + 1;
                            continue;
                        }
                    }
                }
            }

            // Wikilink `[[...]]`
            if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
                let slice: String = chars[i..].iter().collect();
                if let Some(end) = slice.find("]]") {
                    for j in 0..end + 2 {
                        styles[i + j] = self.theme.wikilink;
                    }
                    i += end + 2;
                    continue;
                }
            }

            // Footnote ref `[^...]`
            if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '^' {
                let slice: String = chars[i..].iter().collect();
                if let Some(end) = slice.find(']') {
                    for j in 0..=end {
                        styles[i + j] = self.theme.footnote_ref;
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

            // Bold `**` or `__`
            if (chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*')
                || (chars[i] == '_' && i + 1 < chars.len() && chars[i + 1] == '_')
            {
                let delim = if chars[i] == '*' { "**" } else { "__" };
                let slice: String = chars[i + 2..].iter().collect();
                if let Some(end) = slice.find(delim) {
                    for j in 0..end + 4 {
                        let is_content = j > 0 && j < end + 3;
                        styles[i + j] = if is_content {
                            self.theme.paragraph.add_modifier(Modifier::BOLD)
                        } else {
                            self.theme.paragraph
                        };
                    }
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
                    for j in 0..end + 2 {
                        let is_content = j > 0 && j < end + 1;
                        styles[i + j] = if is_content {
                            self.theme.paragraph.add_modifier(Modifier::ITALIC)
                        } else {
                            self.theme.paragraph
                        };
                    }
                    i += end + 2;
                    continue;
                }
            }

            // Strikethrough `~~`
            if chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '~' {
                let slice: String = chars[i + 2..].iter().collect();
                if let Some(end) = slice.find("~~") {
                    for j in 0..end + 4 {
                        let is_content = j > 0 && j < end + 3;
                        styles[i + j] = if is_content {
                            self.theme.paragraph.add_modifier(Modifier::CROSSED_OUT)
                        } else {
                            self.theme.paragraph
                        };
                    }
                    i += end + 4;
                    continue;
                }
            }

            // Inline code `` ` `` (single)
            if chars[i] == '`' {
                let slice: String = chars[i + 1..].iter().collect();
                if let Some(end) = slice.find('`') {
                    for j in 0..end + 2 {
                        styles[i + j] = self.theme.code_inline;
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

/// Check if a trimmed line is a fence close: only ```` ``` ```` / `~~~` and whitespace.
fn is_fence_line(trimmed: &str) -> bool {
    (trimmed.starts_with("```") || trimmed.starts_with("~~~"))
        && trimmed.len() >= 3
        && trimmed
            .chars()
            .all(|c| c == '`' || c == '~' || c.is_whitespace())
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
        let mut hl = SourceHighlighter::new(&colors);
        let doc = vec!["# H1".to_string(), "## H2".to_string()];
        let styles_h1 = hl.highlight_line("# H1", 0, &doc);
        assert_eq!(styles_h1.len(), 4);
        assert_eq!(styles_h1[0], hl.theme.h1);
        let styles_h2 = hl.highlight_line("## H2", 1, &doc);
        assert_eq!(styles_h2.len(), 5);
        assert_eq!(styles_h2[0], hl.theme.h2);
    }

    #[test]
    fn inline_code() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors);
        let doc = vec!["text `code` more".to_string()];
        let styles = hl.highlight_line("text `code` more", 0, &doc);
        let code_start = "text ".len();
        let code_end = code_start + "`code`".len();
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
        let mut hl = SourceHighlighter::new(&colors);
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
        let mut hl = SourceHighlighter::new(&colors);
        let doc = vec!["line".to_string()];
        let _ = hl.highlight_line("line", 0, &doc);
        let hash_before = hl.scanned_hash;
        let _ = hl.highlight_line("line", 0, &doc);
        assert_eq!(hl.scanned_hash, hash_before);
    }

    #[test]
    fn task_list_detection() {
        let colors = AppThemeColors::default();
        let mut hl = SourceHighlighter::new(&colors);
        let doc = vec!["- [ ] task".to_string(), "- [x] done".to_string()];
        let styles = hl.highlight_line("- [ ] task", 0, &doc);
        // The `[ ]` part should use task_unchecked
        let bracket_start = "- ".len();
        assert_eq!(
            styles[bracket_start], hl.theme.task_unchecked,
            "unchecked bracket"
        );
        let styles2 = hl.highlight_line("- [x] done", 1, &doc);
        assert_eq!(
            styles2[bracket_start], hl.theme.task_checked,
            "checked bracket"
        );
    }
}
