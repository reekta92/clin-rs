pub fn cursor_to_offset(text: &str, row: usize, col: usize) -> usize {
    let mut offset = 0usize;
    let mut lines = text.split('\n').peekable();

    for current_row in 0..row {
        let Some(line) = lines.next() else {
            return text.len();
        };
        offset += line.len();
        if lines.peek().is_some() || current_row < row {
            offset = (offset + 1).min(text.len());
        }
    }

    let line = text.split('\n').nth(row).unwrap_or("");
    let char_count = line.chars().count();
    let target_chars = col.min(char_count);
    let byte_in_line = line
        .char_indices()
        .nth(target_chars)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    (offset + byte_in_line).min(text.len())
}

pub fn offset_to_cursor(lines: &[String], offset: usize) -> (usize, usize) {
    let mut remaining = offset;
    for (row, line) in lines.iter().enumerate() {
        let line_len = line.len();
        if remaining <= line_len {
            let col = line[..remaining].chars().count();
            return (row, col);
        }
        remaining = remaining.saturating_sub(line_len + 1);
    }

    let row = lines.len().saturating_sub(1);
    let col = lines.get(row).map(|line| line.chars().count()).unwrap_or(0);
    (row, col)
}

pub fn inner_word_range(text: &str, cursor: usize) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    let mut pos = cursor.min(text.len());
    let bytes = text.as_bytes();

    while pos < text.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    if pos >= text.len() {
        return None;
    }

    let mut start = pos;
    while start > 0 && !bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }

    let mut end = pos;
    while end < text.len() && !bytes[end].is_ascii_whitespace() {
        end += 1;
    }

    Some((start, end))
}

pub fn inner_pair_range(
    text: &str,
    cursor: usize,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    let pos = cursor.min(text.len());
    let search_left = &text[..pos];
    let mut depth = 0usize;
    let mut open_idx = None;

    for (i, c) in search_left.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                open_idx = Some(i);
                break;
            }
            depth -= 1;
        }
    }

    let open_idx = open_idx?;
    let mut depth = 0usize;
    let mut close_idx = None;

    for (i, c) in text[open_idx..].char_indices() {
        let abs = open_idx + i;
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                close_idx = Some(abs);
                break;
            }
        }
    }

    let close_idx = close_idx?;
    let start = open_idx + open.len_utf8();
    let end = close_idx;
    if start <= end {
        Some((start, end))
    } else {
        None
    }
}

pub fn inner_quote_range(text: &str, cursor: usize, quote: char) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    let pos = cursor.min(text.len());
    let mut left = None;
    for (i, c) in text[..pos].char_indices().rev() {
        if c == quote {
            left = Some(i);
            break;
        }
    }

    let left = left?;
    let mut right = None;
    let start = left + quote.len_utf8();
    for (i, c) in text[start..].char_indices() {
        if c == quote {
            right = Some(start + i);
            break;
        }
    }

    right.map(|r| (start, r))
}
