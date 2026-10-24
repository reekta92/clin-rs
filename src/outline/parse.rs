#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Header { level: u8, title: String },
    ListItem { text: String },
    Paragraph { preview: String, full_text: String },
    CodeBlock { lang: String, code: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub kind: NodeKind,
    pub depth: usize, // 0 = note-title root
    pub line: usize,  // 1-based source line for jump
    pub has_children: bool,
}

impl TreeNode {
    pub fn full_text(&self) -> &str {
        match &self.kind {
            NodeKind::Header { title, .. } => title,
            NodeKind::ListItem { text } => text,
            NodeKind::Paragraph { full_text, .. } => full_text,
            NodeKind::CodeBlock { code, .. } => code,
        }
    }
}

/// `title` = note title (becomes the depth-0 root).
/// `content` = note body (already frontmatter-stripped by `Storage::load_note`).
pub fn parse_outline(title: &str, content: &str) -> Vec<TreeNode> {
    use comrak::{Arena, arena_tree::NodeEdge, nodes::NodeValue, parse_document};
    let arena = Arena::new();
    let root = parse_document(&arena, content, &crate::markdown::structure::options());
    let mut nodes = vec![TreeNode {
        kind: NodeKind::Header {
            level: 0,
            title: title.to_owned(),
        },
        depth: 0,
        line: 1,
        has_children: false,
    }];
    let mut headings = Vec::new();
    // Headings inside a list item or quote cannot change its parent's sections.
    let mut heading_scopes = Vec::new();
    let mut source_lines = None;
    for edge in root.traverse() {
        let node = match edge {
            NodeEdge::Start(node) => node,
            NodeEdge::End(node) => {
                if matches!(
                    node.data.borrow().value,
                    NodeValue::Item(_) | NodeValue::TaskItem(_) | NodeValue::BlockQuote
                ) && let Some(start) = heading_scopes.pop()
                {
                    headings.truncate(start);
                }
                continue;
            }
        };
        let data = node.data.borrow();
        if matches!(
            data.value,
            NodeValue::Item(_) | NodeValue::TaskItem(_) | NodeValue::BlockQuote
        ) {
            heading_scopes.push(headings.len());
        }
        if !matches!(
            data.value,
            NodeValue::Heading(_)
                | NodeValue::Item(_)
                | NodeValue::TaskItem(_)
                | NodeValue::Paragraph
                | NodeValue::Table(_)
                | NodeValue::HtmlBlock(_)
                | NodeValue::CodeBlock(_)
        ) {
            continue;
        }
        let list_depth = node
            .ancestors()
            .skip(1)
            .filter(|parent| {
                matches!(
                    parent.data.borrow().value,
                    NodeValue::Item(_) | NodeValue::TaskItem(_)
                )
            })
            .count();
        let mut depth = headings.len() + list_depth + 1;
        let kind = match &data.value {
            NodeValue::Heading(heading) => {
                let scope_start = heading_scopes.last().copied().unwrap_or(0);
                while headings.len() > scope_start
                    && headings.last().is_some_and(|level| *level >= heading.level)
                {
                    headings.pop();
                }
                depth = headings.len() + list_depth + 1;
                headings.push(heading.level);
                NodeKind::Header {
                    level: heading.level,
                    title: inline_text(node),
                }
            }
            NodeValue::Item(_) | NodeValue::TaskItem(_) => {
                let mut text = node
                    .first_child()
                    .filter(|child| {
                        matches!(
                            child.data.borrow().value,
                            NodeValue::Paragraph | NodeValue::Heading(_)
                        )
                    })
                    .map(inline_text)
                    .unwrap_or_default();
                if let NodeValue::TaskItem(task) = data.value {
                    text = format!("[{}] {text}", task.symbol.unwrap_or(' '));
                }
                NodeKind::ListItem { text }
            }
            NodeValue::Paragraph | NodeValue::Table(_) | NodeValue::HtmlBlock(_) => {
                if matches!(data.value, NodeValue::Paragraph)
                    && node.previous_sibling().is_none()
                    && node.parent().is_some_and(|parent| {
                        matches!(
                            parent.data.borrow().value,
                            NodeValue::Item(_) | NodeValue::TaskItem(_)
                        )
                    })
                {
                    continue;
                }
                let full_text = match &data.value {
                    NodeValue::Table(_) => {
                        let lines =
                            source_lines.get_or_insert_with(|| content.lines().collect::<Vec<_>>());
                        lines
                            .get(
                                data.sourcepos.start.line.saturating_sub(1)
                                    ..data.sourcepos.end.line,
                            )
                            .unwrap_or_default()
                            .join("\n")
                    }
                    NodeValue::HtmlBlock(block) => block.literal.trim_end_matches('\n').to_owned(),
                    _ => inline_text(node),
                };
                let preview =
                    crate::fsutil::truncate_ellipsis(full_text.lines().next().unwrap_or(""), 60);
                NodeKind::Paragraph { preview, full_text }
            }
            NodeValue::CodeBlock(code) => NodeKind::CodeBlock {
                lang: code.info.split_whitespace().next().unwrap_or("").to_owned(),
                code: code.literal.trim_end_matches('\n').to_owned(),
            },
            _ => continue,
        };
        nodes.push(TreeNode {
            kind,
            depth,
            line: data.sourcepos.start.line,
            has_children: false,
        });
    }
    for i in 0..nodes.len().saturating_sub(1) {
        nodes[i].has_children = nodes[i + 1].depth > nodes[i].depth;
    }
    nodes
}

fn inline_text<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    use comrak::nodes::NodeValue;
    let mut text = String::new();
    for child in node.descendants().skip(1) {
        match &child.data.borrow().value {
            NodeValue::Text(value) => text.push_str(value),
            NodeValue::Code(code) => text.push_str(&code.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => text.push('\n'),
            _ => {}
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_and_html_blocks_remain_navigable() {
        for source in [
            "| A | B |\n|---|---|\n| x | y |\n",
            "<div>\nUseful HTML content\n</div>\n",
        ] {
            let nodes = parse_outline("Note", &format!("# Section\n\n{source}"));
            assert_eq!(nodes.len(), 3);
            assert_eq!(nodes[2].line, 3);
            assert_eq!(nodes[2].depth, 2);
            assert_eq!(nodes[2].full_text(), source.trim_end_matches('\n'));
        }
    }

    #[test]
    fn container_headings_do_not_capture_following_blocks() {
        for source in [
            "# Outer\n\n- ## Inside\n\nOutside paragraph.\n",
            "# Outer\n\n> # Inside\n>\n> Inside paragraph.\n\nOutside paragraph.\n",
            "# Outer\n\n- ## Inside\n  - ### Nested\n\nOutside paragraph.\n",
        ] {
            let nodes = parse_outline("Note", source);
            let outside = nodes.last().unwrap();
            assert_eq!(outside.full_text(), "Outside paragraph.");
            assert_eq!(outside.depth, 2);
            assert!(nodes.iter().all(|node| !node.full_text().is_empty()));
            for (index, node) in nodes.iter().enumerate() {
                if matches!(node.kind, NodeKind::ListItem { .. }) {
                    assert!(nodes[index + 1].depth > node.depth);
                    assert!(node.has_children);
                }
            }
        }
    }

    #[test]
    fn commonmark_fences_headings_and_nested_lists_keep_source_lines() {
        let source = "Heading\n=======\n\n````rust\n~~~\n# code, not a heading\n````\n\n- outer\n  - inner\n\n```text\nunclosed\n";
        let nodes = parse_outline("Note", source);
        assert_eq!(
            nodes.iter().map(|node| node.line).collect::<Vec<_>>(),
            [1, 1, 4, 9, 10, 12]
        );
        assert!(
            matches!(&nodes[1].kind, NodeKind::Header { level: 1, title } if title == "Heading")
        );
        assert!(
            matches!(&nodes[2].kind, NodeKind::CodeBlock { lang, code } if lang == "rust" && code == "~~~\n# code, not a heading")
        );
        assert_eq!(nodes[4].depth, nodes[3].depth + 1);
        assert!(nodes[3].has_children);
        assert!(matches!(&nodes[5].kind, NodeKind::CodeBlock { code, .. } if code == "unclosed"));
    }

    #[test]
    fn test_parse_outline() {
        let content = "
# Project
Intro paragraph line.

## Tasks
- Fix bug
- Add tests

### Stretch
Some notes here.

```rust
fn x() {}
```
";
        let nodes = parse_outline("Title", content);
        assert_eq!(
            nodes[0].kind,
            NodeKind::Header {
                level: 0,
                title: "Title".to_string()
            }
        );
        assert_eq!(nodes[0].depth, 0);

        assert_eq!(
            nodes[1].kind,
            NodeKind::Header {
                level: 1,
                title: "Project".to_string()
            }
        );
        assert_eq!(nodes[1].depth, 1);
        assert!(nodes[1].has_children);

        assert_eq!(
            nodes[2].kind,
            NodeKind::Paragraph {
                preview: "Intro paragraph line.".to_string(),
                full_text: "Intro paragraph line.".to_string(),
            }
        );
        assert_eq!(nodes[2].depth, 2);

        assert_eq!(
            nodes[3].kind,
            NodeKind::Header {
                level: 2,
                title: "Tasks".to_string()
            }
        );
        assert_eq!(nodes[3].depth, 2);
        assert!(nodes[3].has_children);

        assert_eq!(
            nodes[4].kind,
            NodeKind::ListItem {
                text: "Fix bug".to_string()
            }
        );
        assert_eq!(nodes[4].depth, 3);
        assert_eq!(
            nodes[5].kind,
            NodeKind::ListItem {
                text: "Add tests".to_string()
            }
        );
        assert_eq!(nodes[5].depth, 3);

        assert_eq!(
            nodes[6].kind,
            NodeKind::Header {
                level: 3,
                title: "Stretch".to_string()
            }
        );
        assert_eq!(nodes[6].depth, 3);
        assert!(nodes[6].has_children);

        assert_eq!(
            nodes[7].kind,
            NodeKind::Paragraph {
                preview: "Some notes here.".to_string(),
                full_text: "Some notes here.".to_string(),
            }
        );
        assert_eq!(nodes[7].depth, 4);

        assert_eq!(
            nodes[8].kind,
            NodeKind::CodeBlock {
                lang: "rust".to_string(),
                code: "fn x() {}".to_string(),
            }
        );
        assert_eq!(nodes[8].depth, 4);
    }
}
