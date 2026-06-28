use crate::markdown::style::{MarkdownTheme, RenderLine};
use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};
use ratatui::style::{Color, Modifier, Style};
use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_nonewlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);
const CODE_THEME: &str = "base16-ocean.dark";

pub(crate) fn render_builtin(
    content: &str,
    cols: u16,
    theme: &MarkdownTheme,
    wrap: bool,
    syntax_highlighting: bool,
    cancel_token: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Option<Vec<RenderLine>> {
    let arena = Arena::new();
    let mut options = Options::default();

    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options.extension.wikilinks_title_after_pipe = true;

    let root = parse_document(&arena, content, &options);

    let mut lines = Vec::new();
    render_block(
        root,
        &mut lines,
        theme,
        cols,
        wrap,
        syntax_highlighting,
        &cancel_token,
    );

    if cancel_token.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    // Trim trailing empty lines
    while let Some(last) = lines.last() {
        if last.cells.iter().all(|(c, _)| c.is_whitespace()) {
            lines.pop();
        } else {
            break;
        }
    }

    // If empty, return a single empty line
    if lines.is_empty() {
        lines.push(RenderLine { cells: Vec::new() });
    }
    Some(lines)
}

fn collect_inlines<'a>(
    node: &'a AstNode<'a>,
    theme: &MarkdownTheme,
    current_style: Style,
    out: &mut Vec<(char, Style)>,
) {
    let val = &node.data.borrow().value;
    match val {
        NodeValue::Text(s) => {
            for c in s.chars() {
                out.push((c, current_style));
            }
        }
        NodeValue::Code(c) => {
            let style = current_style.patch(theme.code_inline);
            out.push(('`', style));
            for c in c.literal.chars() {
                out.push((c, style));
            }
            out.push(('`', style));
        }
        NodeValue::Emph => {
            let style = current_style.add_modifier(Modifier::ITALIC);
            for child in node.children() {
                collect_inlines(child, theme, style, out);
            }
        }
        NodeValue::Strong => {
            let style = current_style.add_modifier(Modifier::BOLD);
            for child in node.children() {
                collect_inlines(child, theme, style, out);
            }
        }
        NodeValue::Strikethrough => {
            let style = current_style.add_modifier(Modifier::CROSSED_OUT);
            for child in node.children() {
                collect_inlines(child, theme, style, out);
            }
        }
        NodeValue::Link(l) => {
            let style = current_style.patch(theme.link);
            for child in node.children() {
                collect_inlines(child, theme, style, out);
            }
            out.push((' ', style));
            out.push(('(', style));
            for c in l.url.chars() {
                out.push((c, style));
            }
            out.push((')', style));
        }
        NodeValue::Image(l) => {
            let style = current_style.patch(theme.link);
            out.push(('!', style));
            out.push(('[', style));
            for child in node.children() {
                collect_inlines(child, theme, style, out);
            }
            out.push((']', style));
            out.push(('(', style));
            for c in l.url.chars() {
                out.push((c, style));
            }
            out.push((')', style));
        }
        NodeValue::SoftBreak | NodeValue::LineBreak => {
            out.push((' ', current_style));
        }
        NodeValue::FootnoteReference(fr) => {
            let style = current_style.patch(theme.footnote_ref);
            out.push(('[', style));
            out.push(('^', style));
            for c in fr.name.chars() {
                out.push((c, style));
            }
            out.push((']', style));
        }
        NodeValue::WikiLink(_w) => {
            let style = current_style.patch(theme.wikilink);
            out.push(('[', style));
            out.push(('[', style));
            for child in node.children() {
                collect_inlines(child, theme, style, out);
            }
            out.push((']', style));
            out.push((']', style));
        }
        _ => {
            for child in node.children() {
                collect_inlines(child, theme, current_style, out);
            }
        }
    }
}

fn wrap_cells(cells: &[(char, Style)], max_cols: u16, wrap: bool) -> Vec<Vec<(char, Style)>> {
    if max_cols == 0 {
        return vec![cells.to_vec()];
    }
    if !wrap {
        let mut truncated = Vec::new();
        let mut width = 0;
        for &cell in cells {
            let w = unicode_width::UnicodeWidthChar::width(cell.0).unwrap_or(1);
            if width + w > max_cols as usize {
                break;
            }
            truncated.push(cell);
            width += w;
        }
        return vec![truncated];
    }

    let mut result = Vec::new();
    let mut current_line = Vec::new();
    let mut current_width = 0;

    let mut i = 0;
    while i < cells.len() {
        let mut word_len = 0;
        let mut word_width = 0;
        while i + word_len < cells.len() && !cells[i + word_len].0.is_whitespace() {
            let c = cells[i + word_len].0;
            let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
            word_width += w;
            word_len += 1;
        }

        if word_len > 0 {
            if current_width + word_width <= max_cols as usize {
                for idx in 0..word_len {
                    current_line.push(cells[i + idx]);
                }
                current_width += word_width;
                i += word_len;
            } else if word_width > max_cols as usize {
                if current_width > 0 {
                    result.push(std::mem::take(&mut current_line));
                    current_width = 0;
                }
                while i < cells.len() && !cells[i].0.is_whitespace() {
                    let c = cells[i].0;
                    let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
                    if current_width + w > max_cols as usize {
                        result.push(std::mem::take(&mut current_line));
                        current_width = 0;
                    }
                    current_line.push(cells[i]);
                    current_width += w;
                    i += 1;
                }
            } else {
                result.push(std::mem::take(&mut current_line));
                current_width = 0;
                for idx in 0..word_len {
                    current_line.push(cells[i + idx]);
                }
                current_width += word_width;
                i += word_len;
            }
        } else {
            let c = cells[i].0;
            let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
            if current_width + w > max_cols as usize {
                result.push(std::mem::take(&mut current_line));
                current_width = 0;
            }
            if current_width > 0 || c != ' ' {
                current_line.push(cells[i]);
                current_width += w;
            }
            i += 1;
        }
    }

    if !current_line.is_empty() || result.is_empty() {
        result.push(current_line);
    }

    result
}

fn syntect_style_to_ratatui(style: SyntectStyle, theme: &MarkdownTheme) -> Style {
    let mut r_style = Style::reset();

    let fg = style.foreground;
    r_style = r_style.fg(Color::Rgb(fg.r, fg.g, fg.b));

    let bg = style.background;
    if bg.a > 0 {
        r_style = r_style.bg(Color::Rgb(bg.r, bg.g, bg.b));
    } else {
        r_style = r_style.bg(theme.code_block.bg.unwrap_or(Color::Reset));
    }

    let font_style = style.font_style;
    if font_style.contains(FontStyle::BOLD) {
        r_style = r_style.add_modifier(Modifier::BOLD);
    }
    if font_style.contains(FontStyle::ITALIC) {
        r_style = r_style.add_modifier(Modifier::ITALIC);
    }
    if font_style.contains(FontStyle::UNDERLINE) {
        r_style = r_style.add_modifier(Modifier::UNDERLINED);
    }

    r_style
}

fn render_block<'a>(
    node: &'a AstNode<'a>,
    lines: &mut Vec<RenderLine>,
    theme: &MarkdownTheme,
    cols: u16,
    wrap: bool,
    syntax_highlighting: bool,
    cancel_token: &std::sync::atomic::AtomicBool,
) {
    if cancel_token.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let val = &node.data.borrow().value;
    match val {
        NodeValue::Document => {
            for child in node.children() {
                render_block(
                    child,
                    lines,
                    theme,
                    cols,
                    wrap,
                    syntax_highlighting,
                    cancel_token,
                );
            }
        }
        NodeValue::Paragraph => {
            let mut cells = Vec::new();
            for child in node.children() {
                collect_inlines(child, theme, theme.paragraph, &mut cells);
            }
            let wrapped = wrap_cells(&cells, cols, wrap);
            for w_line in wrapped {
                lines.push(RenderLine { cells: w_line });
            }
            lines.push(RenderLine { cells: Vec::new() });
        }
        NodeValue::Heading(h) => {
            let heading_style = match h.level {
                1 => theme.heading_1,
                2 => theme.heading_2,
                3 => theme.heading_3,
                4 => theme.heading_4,
                5 => theme.heading_5,
                _ => theme.heading_6,
            };
            let mut cells = Vec::new();
            for child in node.children() {
                collect_inlines(child, theme, heading_style, &mut cells);
            }
            let wrapped = wrap_cells(&cells, cols, wrap);
            for w_line in wrapped {
                lines.push(RenderLine { cells: w_line });
            }
            lines.push(RenderLine { cells: Vec::new() });
        }
        NodeValue::BlockQuote => {
            let border_style = theme.blockquote;
            let border_prefix = vec![('│', border_style), (' ', border_style)];
            let indent = border_prefix.len();
            let inner_cols = cols.saturating_sub(indent as u16).max(10);

            let mut inner_lines = Vec::new();
            for child in node.children() {
                render_block(
                    child,
                    &mut inner_lines,
                    theme,
                    inner_cols,
                    wrap,
                    syntax_highlighting,
                    cancel_token,
                );
            }

            while let Some(last) = inner_lines.last() {
                if last.cells.iter().all(|(c, _)| c.is_whitespace()) {
                    inner_lines.pop();
                } else {
                    break;
                }
            }

            for line in inner_lines {
                let mut new_cells = border_prefix.clone();
                new_cells.extend(line.cells);
                lines.push(RenderLine { cells: new_cells });
            }
            lines.push(RenderLine { cells: Vec::new() });
        }
        NodeValue::List(l) => {
            let mut item_index = l.start;
            for child in node.children() {
                render_list_item(
                    child,
                    lines,
                    theme,
                    cols,
                    wrap,
                    syntax_highlighting,
                    cancel_token,
                    l,
                    item_index,
                );
                item_index += 1;
            }
            lines.push(RenderLine { cells: Vec::new() });
        }
        NodeValue::CodeBlock(c) => {
            if !c.info.is_empty() {
                let header = format!("[{}]", c.info);
                let mut header_cells = Vec::new();
                for ch in header.chars() {
                    header_cells.push((ch, theme.blockquote));
                }
                lines.push(RenderLine {
                    cells: header_cells,
                });
            }

            let mut highlighted = false;
            if syntax_highlighting && !c.info.is_empty() {
                if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(&c.info) {
                    if let Some(theme_ref) = THEME_SET.themes.get(CODE_THEME) {
                        let mut h = HighlightLines::new(syntax, theme_ref);
                        for line in c.literal.lines() {
                            if cancel_token.load(std::sync::atomic::Ordering::Relaxed) {
                                return; // render_block returns early; render_builtin's cancel check returns None
                            }
                            let mut cells = Vec::new();
                            let ranges = h.highlight_line(line, &SYNTAX_SET).unwrap_or(Vec::new());
                            for (style, text) in ranges {
                                let cell_style = syntect_style_to_ratatui(style, theme);
                                for ch in text.chars() {
                                    if ch != '\n' && ch != '\r' {
                                        cells.push((ch, cell_style));
                                    }
                                }
                            }
                            let wrapped = wrap_cells(&cells, cols, wrap);
                            for w_line in wrapped {
                                lines.push(RenderLine { cells: w_line });
                            }
                        }
                        highlighted = true;
                    }
                }
            }

            if !highlighted {
                let code_style = theme.code_block;
                for line in c.literal.lines() {
                    let mut cells = Vec::new();
                    for ch in line.chars() {
                        cells.push((ch, code_style));
                    }
                    let wrapped = wrap_cells(&cells, cols, wrap);
                    for w_line in wrapped {
                        lines.push(RenderLine { cells: w_line });
                    }
                }
            }
            lines.push(RenderLine { cells: Vec::new() });
        }
        NodeValue::ThematicBreak => {
            let mut cells = Vec::new();
            for _ in 0..cols {
                cells.push(('─', theme.hr));
            }
            lines.push(RenderLine { cells });
            lines.push(RenderLine { cells: Vec::new() });
        }
        NodeValue::Table(t) => {
            // Collect rows
            let mut rows = Vec::new();
            for r_node in node.children() {
                if let NodeValue::TableRow(_) = &r_node.data.borrow().value {
                    let mut cells = Vec::new();
                    for c_node in r_node.children() {
                        if let NodeValue::TableCell = &c_node.data.borrow().value {
                            let mut cell_cells = Vec::new();
                            for child in c_node.children() {
                                collect_inlines(child, theme, theme.paragraph, &mut cell_cells);
                            }
                            cells.push(cell_cells);
                        }
                    }
                    rows.push((r_node.data.borrow().value.clone(), cells));
                }
            }

            if rows.is_empty() {
                return;
            }

            let num_cols = rows.iter().map(|(_, cells)| cells.len()).max().unwrap_or(0);
            if num_cols == 0 {
                return;
            }

            let mut col_widths = vec![0; num_cols];
            for (_, row_cells) in &rows {
                for (c_idx, cell) in row_cells.iter().enumerate() {
                    let cell_str: String = cell.iter().map(|(ch, _)| *ch).collect();
                    let cell_w = unicode_width::UnicodeWidthStr::width(cell_str.as_str());
                    col_widths[c_idx] = col_widths[c_idx].max(cell_w);
                }
            }

            // Add 2 padding
            for w in &mut col_widths {
                *w += 2;
            }

            // Top border
            let mut top = Vec::new();
            top.push(('┌', theme.table_border));
            for (c_idx, &w) in col_widths.iter().enumerate() {
                for _ in 0..w {
                    top.push(('─', theme.table_border));
                }
                if c_idx < num_cols - 1 {
                    top.push(('┬', theme.table_border));
                }
            }
            top.push(('┐', theme.table_border));
            lines.push(RenderLine { cells: top });

            // Render rows
            for (_, row_cells) in rows.iter() {
                let mut row_line = Vec::new();
                row_line.push(('│', theme.table_border));

                for c_idx in 0..num_cols {
                    let cell_data = row_cells.get(c_idx);
                    let mut cell_w = 0usize;
                    let mut cell_cells = Vec::new();
                    if let Some(c_data) = cell_data {
                        cell_cells = c_data.clone();
                        let cell_str: String = cell_cells.iter().map(|(ch, _)| *ch).collect();
                        cell_w = unicode_width::UnicodeWidthStr::width(cell_str.as_str());
                    }

                    let col_w = col_widths[c_idx];
                    let alignment = t
                        .alignments
                        .get(c_idx)
                        .copied()
                        .unwrap_or(TableAlignment::None);

                    let (pad_left, pad_right) = match alignment {
                        TableAlignment::Left | TableAlignment::None => {
                            let pl = 1;
                            let pr = col_w.saturating_sub(cell_w).saturating_sub(1);
                            (pl, pr)
                        }
                        TableAlignment::Right => {
                            let pl = col_w.saturating_sub(cell_w).saturating_sub(1);
                            let pr = 1;
                            (pl, pr)
                        }
                        TableAlignment::Center => {
                            let total_pad = col_w.saturating_sub(cell_w);
                            let pl = total_pad / 2;
                            let pr = total_pad.saturating_sub(pl);
                            (pl, pr)
                        }
                    };

                    for _ in 0..pad_left {
                        row_line.push((' ', theme.paragraph));
                    }
                    for cell_char in cell_cells {
                        row_line.push(cell_char);
                    }
                    for _ in 0..pad_right {
                        row_line.push((' ', theme.paragraph));
                    }

                    row_line.push(('│', theme.table_border));
                }
                lines.push(RenderLine { cells: row_line });
            }

            // Bottom border
            let mut bot = Vec::new();
            bot.push(('└', theme.table_border));
            for (c_idx, &w) in col_widths.iter().enumerate() {
                for _ in 0..w {
                    bot.push(('─', theme.table_border));
                }
                if c_idx < num_cols - 1 {
                    bot.push(('┴', theme.table_border));
                }
            }
            bot.push(('┘', theme.table_border));
            lines.push(RenderLine { cells: bot });
            lines.push(RenderLine { cells: Vec::new() });
        }
        NodeValue::FootnoteDefinition(fd) => {
            let prefix = format!("[^{}]: ", fd.name);
            let mut prefix_cells = Vec::new();
            for ch in prefix.chars() {
                prefix_cells.push((ch, theme.footnote_ref));
            }
            let indent = prefix_cells.len();
            let inner_cols = cols.saturating_sub(indent as u16).max(10);

            let mut inner_lines = Vec::new();
            for child in node.children() {
                render_block(
                    child,
                    &mut inner_lines,
                    theme,
                    inner_cols,
                    wrap,
                    syntax_highlighting,
                    cancel_token,
                );
            }

            while let Some(last) = inner_lines.last() {
                if last.cells.iter().all(|(c, _)| c.is_whitespace()) {
                    inner_lines.pop();
                } else {
                    break;
                }
            }

            if inner_lines.is_empty() {
                lines.push(RenderLine {
                    cells: prefix_cells,
                });
            } else {
                for (idx, line) in inner_lines.into_iter().enumerate() {
                    let mut new_cells = Vec::new();
                    if idx == 0 {
                        new_cells.extend(prefix_cells.clone());
                    } else {
                        for _ in 0..indent {
                            new_cells.push((' ', theme.paragraph));
                        }
                    }
                    new_cells.extend(line.cells);
                    lines.push(RenderLine { cells: new_cells });
                }
            }
            lines.push(RenderLine { cells: Vec::new() });
        }
        _ => {
            for child in node.children() {
                render_block(
                    child,
                    lines,
                    theme,
                    cols,
                    wrap,
                    syntax_highlighting,
                    cancel_token,
                );
            }
        }
    }
}

fn render_list_item<'a>(
    node: &'a AstNode<'a>,
    lines: &mut Vec<RenderLine>,
    theme: &MarkdownTheme,
    cols: u16,
    wrap: bool,
    syntax_highlighting: bool,
    cancel_token: &std::sync::atomic::AtomicBool,
    list_data: &comrak::nodes::NodeList,
    item_index: usize,
) {
    let val = &node.data.borrow().value;
    let mut prefix_cells = Vec::new();

    match val {
        NodeValue::Item(_) => match list_data.list_type {
            ListType::Bullet => {
                let bullet = (list_data.bullet_char as char).to_string() + " ";
                for c in bullet.chars() {
                    prefix_cells.push((c, theme.paragraph));
                }
            }
            ListType::Ordered => {
                let prefix = format!("{}. ", item_index);
                for c in prefix.chars() {
                    prefix_cells.push((c, theme.paragraph));
                }
            }
        },
        NodeValue::TaskItem(checked_status) => {
            let checkbox = match checked_status {
                Some('x') | Some('X') => ("- [x] ", theme.task_checkbox_checked),
                _ => ("- [ ] ", theme.task_checkbox_unchecked),
            };
            for c in checkbox.0.chars() {
                prefix_cells.push((c, checkbox.1));
            }
        }
        _ => return,
    }

    let indent = prefix_cells.len();
    let inner_cols = cols.saturating_sub(indent as u16).max(10);
    let mut inner_lines = Vec::new();
    for child in node.children() {
        render_block(
            child,
            &mut inner_lines,
            theme,
            inner_cols,
            wrap,
            syntax_highlighting,
            cancel_token,
        );
    }

    while let Some(last) = inner_lines.last() {
        if last.cells.iter().all(|(c, _)| c.is_whitespace()) {
            inner_lines.pop();
        } else {
            break;
        }
    }

    if inner_lines.is_empty() {
        lines.push(RenderLine {
            cells: prefix_cells,
        });
    } else {
        for (idx, line) in inner_lines.into_iter().enumerate() {
            let mut new_cells = Vec::new();
            if idx == 0 {
                new_cells.extend(prefix_cells.clone());
            } else {
                for _ in 0..indent {
                    new_cells.push((' ', theme.paragraph));
                }
            }
            new_cells.extend(line.cells);
            lines.push(RenderLine { cells: new_cells });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_theme::AppThemeColors;
    use ratatui::style::{Color, Modifier};

    fn get_test_theme() -> MarkdownTheme {
        let colors = AppThemeColors::default();
        MarkdownTheme::from_app_theme(&colors)
    }

    #[test]
    fn test_renders_heading_bold_colored() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "# Title\n## Subtitle",
            80,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        let first_cell = lines[0].cells[0];
        assert_eq!(first_cell.0, 'T');
        assert!(first_cell.1.add_modifier.contains(Modifier::BOLD));
        assert_eq!(first_cell.1.fg, Some(Color::Yellow));

        let second_cell = lines[2].cells[0];
        assert_eq!(second_cell.0, 'S');
        assert!(second_cell.1.add_modifier.contains(Modifier::BOLD));
        assert_eq!(second_cell.1.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_renders_task_list_checkboxes() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "- [ ] task1\n- [x] task2",
            80,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        let line0_str: String = lines[0].cells.iter().map(|(c, _)| *c).collect();
        assert!(line0_str.starts_with("- [ ] "));
        assert_eq!(lines[0].cells[2].1.fg, Some(Color::Yellow));

        let line1_str: String = lines[1].cells.iter().map(|(c, _)| *c).collect();
        assert!(line1_str.starts_with("- [x] "));
        assert_eq!(lines[1].cells[2].1.fg, Some(Color::Green));
    }

    #[test]
    fn test_renders_blockquote_with_border() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "> hello",
            80,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        let line_str: String = lines[0].cells.iter().map(|(c, _)| *c).collect();
        assert!(line_str.starts_with("│ hello"));
        assert_eq!(lines[0].cells[0].1.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_renders_inline_code_bg() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "`code`",
            80,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        let line_str: String = lines[0].cells.iter().map(|(c, _)| *c).collect();
        assert_eq!(line_str, "`code`");
        println!("Cells: {:?}", lines[0].cells);
        assert_eq!(lines[0].cells[1].1.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_renders_footnote_ref() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "ref[^1]\n\n[^1]: desc",
            80,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        let line0_str: String = lines[0].cells.iter().map(|(c, _)| *c).collect();
        assert!(line0_str.contains("[^1]"));
        let line2_str: String = lines[2].cells.iter().map(|(c, _)| *c).collect();
        assert!(line2_str.starts_with("[^1]: desc"));
    }

    #[test]
    fn test_renders_wikilink_accent() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "[[target]]",
            80,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        let line_str: String = lines[0].cells.iter().map(|(c, _)| *c).collect();
        assert_eq!(line_str, "[[target]]");
        println!("Wikilink Cells: {:?}", lines[0].cells);
        assert_eq!(lines[0].cells[2].1.fg, Some(Color::Cyan));
        assert!(
            lines[0].cells[2]
                .1
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn test_empty_input_returns_empty() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "",
            80,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].cells.is_empty());
    }

    #[test]
    fn test_wraps_long_lines_at_cols() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "hello world this is a test",
            10,
            &theme,
            true,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        assert!(lines.len() >= 3);
        for line in &lines {
            assert!(line.cells.len() <= 10);
        }
    }

    #[test]
    fn test_nowrap_truncates() {
        let theme = get_test_theme();
        let lines = render_builtin(
            "hello world this is a test",
            10,
            &theme,
            false,
            false,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].cells.len(), 10);
        let line_str: String = lines[0].cells.iter().map(|(c, _)| *c).collect();
        assert_eq!(line_str, "hello worl");
    }

    #[test]
    fn test_crash_on_c_code_block() {
        let theme = get_test_theme();
        let content1 = r#"```c
int number = 10;
int number2 = 10;

do 
{
	printf("This will run before the condition");
}
while(number == number2)
//in this type of loop, code block will run before the condition is checked
```"#;
        let lines1 = render_builtin(
            content1,
            80,
            &theme,
            true,
            true,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(lines1.is_some());

        let content2 = r#"Structure is a collection of different data items

```c
struct computer
{
	char proccessor[5];
	int generation;
	char memory [4];
};
```"#;
        let lines2 = render_builtin(
            content2,
            80,
            &theme,
            true,
            true,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(lines2.is_some());
    }
}
