//! Markdown syntax shared by rendering, navigation, and metadata.
use comrak::{Arena, Options, nodes::NodeValue, parse_document};
use std::collections::HashSet;
use std::sync::LazyLock;

pub(crate) fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options.extension.wikilinks_title_after_pipe = true;
    options.extension.description_lists = true;
    options
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Counts {
    pub headings: usize,
    pub tasks: usize,
}

pub(crate) fn counts(source: &str) -> Counts {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options());
    let mut counts = Counts::default();
    for node in root.descendants() {
        match node.data.borrow().value {
            NodeValue::Heading(_) => counts.headings += 1,
            NodeValue::TaskItem(_) => counts.tasks += 1,
            _ => {}
        }
    }
    counts
}

pub(crate) fn wikilinks(source: &str) -> Vec<String> {
    metadata(source).wikilinks
}

#[derive(Default)]
pub(crate) struct Metadata {
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub embeds: Vec<String>,
    pub wikilinks: Vec<String>,
}

pub(crate) fn metadata(content: &str) -> Metadata {
    let arena = Arena::new();
    let options = options();
    let root = parse_document(&arena, content, &options);
    let mut line_starts = vec![0];
    line_starts.extend(content.match_indices('\n').map(|(offset, _)| offset + 1));
    let mut metadata = Metadata::default();
    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::WikiLink(link) => {
                metadata.wikilinks.push(link.url.trim().to_owned());
                let embedded = line_starts
                    .get(data.sourcepos.start.line.saturating_sub(1))
                    .and_then(|line| {
                        line.checked_add(data.sourcepos.start.column.saturating_sub(1))
                    })
                    .and_then(|start| start.checked_sub(1))
                    .is_some_and(|bang| {
                        content.as_bytes().get(bang) == Some(&b'!') && !is_escaped(content, bang)
                    });
                if embedded {
                    metadata.embeds.push(link.url.clone());
                } else {
                    metadata.links.push(link.url.clone());
                }
            }
            NodeValue::Link(link) if !is_external(&link.url) => {
                metadata.links.push(link.url.clone())
            }
            NodeValue::Image(link) if !is_external(&link.url) => {
                metadata.embeds.push(link.url.clone())
            }
            NodeValue::Text(text) => {
                let in_link = node.ancestors().skip(1).any(|parent| {
                    matches!(
                        &parent.data.borrow().value,
                        NodeValue::Link(_) | NodeValue::WikiLink(_) | NodeValue::Image(_)
                    )
                });
                if !in_link {
                    collect_inline_tags(text, &mut metadata.tags);
                    let start = line_starts
                        .get(data.sourcepos.start.line.saturating_sub(1))
                        .and_then(|line| {
                            line.checked_add(data.sourcepos.start.column.saturating_sub(1))
                        });
                    let end = line_starts
                        .get(data.sourcepos.end.line.saturating_sub(1))
                        .and_then(|line| line.checked_add(data.sourcepos.end.column));
                    if let Some(raw) = start
                        .zip(end)
                        .and_then(|(start, end)| content.get(start..end))
                    {
                        let before = metadata.embeds.len();
                        collect_text_embeds(raw, &mut metadata.embeds);
                        metadata.wikilinks.extend(
                            metadata.embeds[before..]
                                .iter()
                                .map(|target| target.trim().to_owned()),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    for values in [
        &mut metadata.links,
        &mut metadata.embeds,
        &mut metadata.wikilinks,
    ] {
        let mut seen = HashSet::new();
        values.retain(|value| !value.is_empty() && seen.insert(value.clone()));
    }
    metadata
}

fn is_escaped(source: &str, offset: usize) -> bool {
    source.as_bytes()[..offset]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        != 0
}

fn collect_text_embeds(mut source: &str, embeds: &mut Vec<String>) {
    // Comrak leaves Obsidian bang-wikilinks in text; AST source ranges exclude code and labels.
    while let Some(start) = source.find("![[") {
        let target_start = start + 3;
        let Some(end) = source[target_start..]
            .find("]]")
            .map(|end| target_start + end)
        else {
            break;
        };
        if !is_escaped(source, start) {
            let target = source[target_start..end].split('|').next().unwrap_or("");
            if !target.is_empty() && !target.contains(['\n', '\r']) {
                embeds.push(target.to_owned());
            }
        }
        source = &source[end + 2..];
    }
}

pub(crate) fn is_external(target: &str) -> bool {
    target.contains(':') || target.starts_with("//")
}

fn collect_inline_tags(text: &str, tags: &mut Vec<String>) {
    static TAG: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"#[\p{L}\p{N}_/-]+").expect("static tag expression"));
    for tag in TAG.find_iter(text) {
        let preceded_by_word = text[..tag.start()]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '#' | '_' | '/' | '-'));
        let value = &tag.as_str()[1..];
        if !preceded_by_word && !value.chars().all(char::is_numeric) {
            tags.push(value.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_ignores_code_and_recognizes_commonmark_nodes() {
        let source = "Heading\n=======\n\nordinary paragraph\n\n+ [X] done\n1. [ ] pending\n\n[[real|alias]] ![[image]] [[real]]\n\n`[[inline]]` \\[[escaped]]\n\n````md\n~~~\n# fake\n- [ ] fake\n[[fenced]]\n````\n";
        let counts = counts(source);
        assert_eq!(counts.headings, 1);
        assert_eq!(counts.tasks, 2);
        assert_eq!(wikilinks(source), ["real", "image"]);
    }
}
