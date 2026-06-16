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
    let mut nodes = Vec::new();

    // 1. Push root node
    nodes.push(TreeNode {
        kind: NodeKind::Header {
            level: 0,
            title: title.to_string(),
        },
        depth: 0,
        line: 1,
        has_children: false,
    });

    let mut stack: Vec<(u8, usize)> = vec![];
    let mut current_code_block: Option<(usize, String, String)> = None; // (start_line, lang, code)
    let mut para: Option<(usize, String)> = None; // (start_line, full_text)

    let cur_depth = |stack: &[(u8, usize)]| -> usize { stack.last().map(|(_, d)| *d).unwrap_or(0) };

    let close_paragraph =
        |para: &mut Option<(usize, String)>, nodes: &mut Vec<TreeNode>, current_depth: usize| {
            if let Some((line, full_text)) = para.take() {
                let first_line = full_text.lines().next().unwrap_or("").to_string();
                let preview = truncate(&first_line);
                nodes.push(TreeNode {
                    kind: NodeKind::Paragraph { preview, full_text },
                    depth: current_depth + 1,
                    line,
                    has_children: false,
                });
            }
        };

    for (idx, line_raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let t = line_raw.trim_start();

        // Check for code block fences
        if t.starts_with("```") || t.starts_with("~~~") {
            if current_code_block.is_none() {
                // Starting a code block
                close_paragraph(&mut para, &mut nodes, cur_depth(&stack));
                let lang = t[3..].trim().to_string();
                current_code_block = Some((line_no, lang, String::new()));
            } else {
                // Closing a code block
                if let Some((start_line, lang, mut code)) = current_code_block.take() {
                    if code.ends_with('\n') {
                        code.pop();
                    }
                    nodes.push(TreeNode {
                        kind: NodeKind::CodeBlock { lang, code },
                        depth: cur_depth(&stack) + 1,
                        line: start_line,
                        has_children: false,
                    });
                }
            }
            continue;
        }

        // If we are currently inside a code block, accumulate the line
        if let Some((_, _, ref mut code)) = current_code_block {
            code.push_str(line_raw);
            code.push('\n');
            continue;
        }

        // Check for ATX header
        let mut chars = t.chars();
        let mut hash_count = 0;
        while let Some('#') = chars.clone().next() {
            chars.next();
            hash_count += 1;
        }

        if (1..=6).contains(&hash_count) {
            let next_char = chars.next();
            if next_char.is_none_or(|c| c.is_whitespace()) {
                close_paragraph(&mut para, &mut nodes, cur_depth(&stack));
                let header_title = chars.collect::<String>().trim().to_string();

                while stack.last().map(|(lv, _)| *lv >= hash_count) == Some(true) {
                    stack.pop();
                }

                let depth = stack.len() + 1;
                nodes.push(TreeNode {
                    kind: NodeKind::Header {
                        level: hash_count,
                        title: header_title,
                    },
                    depth,
                    line: line_no,
                    has_children: false,
                });
                stack.push((hash_count, depth));
                continue;
            }
        }

        // Blank line
        if t.is_empty() {
            close_paragraph(&mut para, &mut nodes, cur_depth(&stack));
            continue;
        }

        // List item
        let is_list = if t.starts_with("- ")
            || t.starts_with("* ")
            || t.starts_with("+ ")
            || t == "-"
            || t == "*"
            || t == "+"
        {
            Some(if t.len() >= 2 {
                t[2..].trim().to_string()
            } else {
                String::new()
            })
        } else {
            let mut chs = t.chars().peekable();
            let mut digits = String::new();
            while let Some(&c) = chs.peek() {
                if c.is_ascii_digit() {
                    digits.push(c);
                    chs.next();
                } else {
                    break;
                }
            }
            if !digits.is_empty() {
                let next_char = chs.next();
                if next_char == Some('.') || next_char == Some(')') {
                    let after_char = chs.next();
                    if after_char.is_none_or(|c| c.is_whitespace()) {
                        let rest = chs.collect::<String>();
                        Some(rest.trim().to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(list_text) = is_list {
            close_paragraph(&mut para, &mut nodes, cur_depth(&stack));
            nodes.push(TreeNode {
                kind: NodeKind::ListItem { text: list_text },
                depth: cur_depth(&stack) + 1,
                line: line_no,
                has_children: false,
            });
            continue;
        }

        // Prose paragraph
        if let Some((_, ref mut full_text)) = para {
            full_text.push('\n');
            full_text.push_str(t);
        } else {
            para = Some((line_no, t.to_string()));
        }
    }

    // Close remaining paragraph
    close_paragraph(&mut para, &mut nodes, cur_depth(&stack));

    // Update has_children in a final pass
    for i in 0..nodes.len() {
        if i + 1 < nodes.len() {
            nodes[i].has_children = nodes[i + 1].depth > nodes[i].depth;
        }
    }

    nodes
}

fn truncate(s: &str) -> String {
    if s.chars().count() > 60 {
        let mut truncated: String = s.chars().take(60).collect();
        truncated.push('…');
        truncated
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
