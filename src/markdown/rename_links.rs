//! Lossless edits to link destinations found by the Markdown parser.
use super::structure::{is_escaped, options};
use comrak::{Arena, nodes::NodeValue, parse_document};
use std::ops::Range;

#[derive(Clone, Copy)]
pub(crate) enum Style {
    Wiki,
    Markdown,
}

pub(crate) fn rewrite(
    source: &str,
    mut replacement: impl FnMut(&str, Style) -> Option<String>,
) -> String {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options());
    let mut lines = vec![0];
    lines.extend(source.match_indices('\n').map(|(offset, _)| offset + 1));
    let mut edits = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        let pos = data.sourcepos;
        let Some(start) = lines
            .get(pos.start.line.saturating_sub(1))
            .map(|line| line + pos.start.column.saturating_sub(1))
        else {
            continue;
        };
        let Some(end) = lines
            .get(pos.end.line.saturating_sub(1))
            .map(|line| line + pos.end.column)
        else {
            continue;
        };
        let Some(raw) = source.get(start..end) else {
            continue;
        };
        match &data.value {
            NodeValue::WikiLink(link) => {
                if let Some(inner) = raw.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
                    replace_wiki(
                        inner,
                        start + 2,
                        &mut |_, style| replacement(&link.url, style),
                        &mut edits,
                    );
                }
            }
            NodeValue::Link(link) | NodeValue::Image(link) => {
                let label_hint = node.last_child().and_then(|child| {
                    let pos = child.data.borrow().sourcepos.end;
                    let end = lines.get(pos.line.checked_sub(1)?)? + pos.column;
                    end.checked_sub(start)
                });
                if let Some(target) = replacement(&link.url, Style::Markdown)
                    && let Some((range, text)) = markdown_edit(raw, label_hint, target, &link.title)
                {
                    edits.push((start + range.start..start + range.end, text));
                }
            }
            NodeValue::Text(_)
                if !node.ancestors().skip(1).any(|parent| {
                    matches!(
                        parent.data.borrow().value,
                        NodeValue::Link(_) | NodeValue::Image(_) | NodeValue::WikiLink(_)
                    )
                }) =>
            {
                let mut from = 0;
                while let Some(open) = raw[from..].find("![[").map(|index| from + index) {
                    let inner_start = open + 3;
                    let Some(close) = raw[inner_start..]
                        .find("]]")
                        .map(|index| inner_start + index)
                    else {
                        break;
                    };
                    let inner = &raw[inner_start..close];
                    if !is_escaped(source, start + open) && !inner.contains(['\n', '\r']) {
                        replace_wiki(inner, start + inner_start, &mut replacement, &mut edits);
                    }
                    from = close + 2;
                }
            }
            _ => {}
        }
    }
    if edits.is_empty() {
        return source.to_owned();
    }
    edits.sort_unstable_by_key(|(range, _)| (range.start, range.end));
    edits.dedup_by(|(a, _), (b, _)| a == b);
    let mut result = String::with_capacity(source.len());
    let mut next = 0;
    for (range, text) in edits {
        if range.start >= next {
            result.push_str(&source[next..range.start]);
            result.push_str(&text);
            next = range.end;
        }
    }
    result.push_str(&source[next..]);
    result
}

fn replace_wiki(
    inner: &str,
    offset: usize,
    replacement: &mut impl FnMut(&str, Style) -> Option<String>,
    edits: &mut Vec<(Range<usize>, String)>,
) {
    let path = inner.split(['|', '#']).next().unwrap_or("");
    let trimmed = path.trim();
    if !trimmed.is_empty()
        && let Some(target) = replacement(trimmed, Style::Wiki)
    {
        let start = offset + path.len() - path.trim_start().len();
        edits.push((start..start + trimmed.len(), target));
    }
}

fn markdown_edit(
    raw: &str,
    label_hint: Option<usize>,
    target: String,
    title: &str,
) -> Option<(Range<usize>, String)> {
    let label = raw.strip_prefix('!').unwrap_or(raw);
    if !label.starts_with('[') {
        return None;
    }
    let label_end = label_hint
        .and_then(|hint| {
            let tail = raw.get(hint..)?.trim_start_matches('`').trim_start();
            tail.starts_with(']').then_some(raw.len() - tail.len() + 1)
        })
        .or_else(|| {
            let mut depth = 0usize;
            raw.bytes().enumerate().find_map(|(index, byte)| {
                if !matches!(byte, b'[' | b']') || is_escaped(raw, index) {
                    return None;
                }
                if byte == b'[' {
                    depth += 1;
                } else {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                None
            })
        })?;
    let rest = &raw[label_end..];
    if let Some(destination) = rest.strip_prefix('(') {
        let trimmed = destination.trim_start();
        let start = label_end + 1 + destination.len() - trimmed.len();
        if let Some(angled) = trimmed.strip_prefix('<') {
            let end = angled
                .match_indices('>')
                .find(|(index, _)| !is_escaped(angled, *index))?
                .0;
            return Some((start + 1..start + 1 + end, target));
        }
        let mut nesting = 0;
        let end = trimmed.bytes().enumerate().find_map(|(index, byte)| {
            if !matches!(byte, b'(' | b')') && !byte.is_ascii_whitespace()
                || is_escaped(trimmed, index)
            {
                return None;
            }
            match byte {
                b'(' => nesting += 1,
                b')' if nesting > 0 => nesting -= 1,
                b')' => return Some(index),
                byte if byte.is_ascii_whitespace() => return Some(index),
                _ => {}
            }
            None
        })?;
        return Some((start..start + end, target));
    }
    if !(rest.is_empty() || rest.starts_with('[') && rest.ends_with(']')) {
        return None;
    }
    let title = if title.is_empty() {
        String::new()
    } else {
        format!(" \"{}\"", title.replace('\\', "\\\\").replace('"', "\\\""))
    };
    Some((label_end..raw.len(), format!("({target}{title})")))
}
