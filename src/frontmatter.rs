use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Parses frontmatter from note content.
/// Returns the parsed frontmatter and the remaining content.
/// If no frontmatter is found, returns default frontmatter and the original content.
pub fn parse(content: &str) -> (Frontmatter, &str) {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return (Frontmatter::default(), content);
    }

    let end_marker = "\n---";
    if let Some(end_idx) = content[3..].find(end_marker) {
        let frontmatter_str = &content[3..3 + end_idx];

        let remaining_start = 3 + end_idx + end_marker.len();
        // Skip trailing newline after the end marker if present
        let mut content_start = remaining_start;
        if content[remaining_start..].starts_with("\r\n") {
            content_start += 2;
        } else if content[remaining_start..].starts_with('\n') {
            content_start += 1;
        }

        let remaining_content = &content[content_start..];

        // Parse YAML
        if let Ok(frontmatter) = serde_yml::from_str::<Frontmatter>(frontmatter_str) {
            return (frontmatter, remaining_content);
        }
    }

    (Frontmatter::default(), content)
}

/// Serializes frontmatter and prepends it to content.
/// If the frontmatter is empty (no tags), returns the content as is.
pub fn serialize(frontmatter: &Frontmatter, content: &str) -> String {
    if frontmatter.tags.is_empty() {
        return content.to_string();
    }

    match serde_yml::to_string(frontmatter) {
        Ok(yaml) => {
            // serde_yml typically adds `---` at the beginning, but let's be sure
            let yaml = yaml.trim();
            let yaml = yaml.to_string();

            // Format:
            // ---
            // tags:
            // - tag1
            // ---
            // <content>
            format!("---\n{}\n---\n{}", yaml, content)
        }
        Err(_) => content.to_string(), // Fallback on failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just some text";
        let (fm, remaining) = parse(content);
        assert!(fm.tags.is_empty());
        assert_eq!(remaining, "Just some text");
    }

    #[test]
    fn test_parse_with_frontmatter() {
        let content = "---\ntags:\n  - work\n  - urgent\n---\nHere is the content.";
        let (fm, remaining) = parse(content);
        assert_eq!(fm.tags, vec!["work", "urgent"]);
        assert_eq!(remaining, "Here is the content.");
    }

    #[test]
    fn test_serialize() {
        let fm = Frontmatter {
            tags: vec!["work".to_string()],
        };
        let content = "My note";
        let serialized = serialize(&fm, content);
        assert!(serialized.starts_with("---\n"));
        assert!(serialized.contains("work"));
        assert!(serialized.ends_with("\n---\nMy note"));
    }
}
