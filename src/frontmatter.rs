use crate::config::TextAlignment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub updated_at: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub links: Option<Vec<String>>,
    #[serde(default)]
    pub original_ext: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_align: Option<TextAlignment>,
    #[serde(flatten)]
    pub extra: serde_yaml_ng::Mapping,
}

/// Raw text and the disk header it was based on. Kept in encrypted drafts too.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontmatterEdit {
    pub text: String,
    pub original: Option<String>,
    pub saved_text: String,
    pub saved_title: String,
}

impl FrontmatterEdit {
    /// The same title precedence applies to live saves and draft recovery.
    pub fn parse_with_title(&self, title: &str) -> anyhow::Result<(Frontmatter, String)> {
        let fm = parse_yaml(&self.text)?;
        let title = title.trim();
        let title = if title.is_empty() {
            "Untitled note"
        } else {
            title
        };
        let previous = parse_yaml(&self.saved_text).unwrap_or_default();
        let title = if !self.text.trim().is_empty() && fm.title != previous.title {
            let yaml_title = fm
                .title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or("Untitled note");
            anyhow::ensure!(
                title == self.saved_title || title == yaml_title,
                "Title changed in both fields; make their values agree before saving"
            );
            yaml_title.to_string()
        } else {
            title.to_string()
        };
        Ok((fm, title))
    }
}

/// Split framing even when YAML is invalid, so the editor can repair it.
pub fn split_raw(content: &str) -> (Option<&str>, &str) {
    let start = if content.starts_with("---\r\n") {
        5
    } else if content.starts_with("---\n") {
        4
    } else {
        return (None, content);
    };
    let mut offset = start;
    for line in content[start..].split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return (
                Some(content[start..offset].trim_end_matches(['\r', '\n'])),
                &content[offset + line.len()..],
            );
        }
        offset += line.len();
    }
    // Without a closing delimiter this may be a Markdown horizontal rule.
    // Preserve the existing body-only reader behavior instead of swallowing it.
    (None, content)
}

pub fn parse_yaml(text: &str) -> anyhow::Result<Frontmatter> {
    if text.trim().is_empty() {
        return Ok(Frontmatter::default());
    }
    // Mapping deserialization rejects duplicate keys and non-mapping documents.
    let mapping: serde_yaml_ng::Mapping = serde_yaml_ng::from_str(text)?;
    Ok(serde_yaml_ng::from_value(serde_yaml_ng::Value::Mapping(
        mapping,
    ))?)
}

pub fn parse(content: &str) -> (Frontmatter, &str) {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return (Frontmatter::default(), content);
    }

    let end_marker = "\n---";
    if let Some(end_idx) = content[3..].find(end_marker) {
        let frontmatter_str = &content[3..3 + end_idx];

        let remaining_start = 3 + end_idx + end_marker.len();

        let mut content_start = remaining_start;
        if content[remaining_start..].starts_with("\r\n") {
            content_start += 2;
        } else if content[remaining_start..].starts_with('\n') {
            content_start += 1;
        }

        let remaining_content = &content[content_start..];

        if let Ok(frontmatter) = serde_yaml_ng::from_str::<Frontmatter>(frontmatter_str) {
            return (frontmatter, remaining_content);
        }
    }

    (Frontmatter::default(), content)
}

pub fn serialize(frontmatter: &Frontmatter, content: &str) -> String {
    if frontmatter.tags.is_empty()
        && !frontmatter.pinned
        && frontmatter.title.is_none()
        && frontmatter.updated_at.is_none()
        && frontmatter.links.is_none()
        && frontmatter.original_ext.is_none()
        && frontmatter.extra.is_empty()
        && frontmatter.text_align.is_none()
    {
        return content.to_string();
    }

    match serde_yaml_ng::to_string(frontmatter) {
        Ok(yaml) => {
            let yaml = yaml.trim();
            let yaml = yaml.to_string();

            format!("---\n{yaml}\n---\n{content}")
        }
        Err(_) => content.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just some text";
        let (fm, remaining) = parse(content);
        assert_eq!(fm.tags.len(), 0);
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
            pinned: false,
            ..Default::default()
        };
        let content = "My note";
        let serialized = serialize(&fm, content);
        assert!(serialized.starts_with("---\n"));
        assert!(serialized.contains("work"));
        assert!(serialized.ends_with("\n---\nMy note"));
    }

    #[test]
    fn test_parse_preserves_unknown_keys() {
        let content = "---\ntitle: Known Title\ntags: [tag1]\npinned: true\ntype: knowledge_concept\ncreated: 2024-03-24\nconfidence: 0.9\nsources:\n  - \"[[kimball-dwt]]\"\n---\nBody content";
        let (fm, body) = parse(content);

        assert_eq!(fm.title.as_deref(), Some("Known Title"));
        assert_eq!(fm.tags, vec!["tag1"]);
        assert!(fm.pinned);

        assert!(fm.extra.contains_key("type"));
        assert!(fm.extra.contains_key("created"));
        assert!(fm.extra.contains_key("confidence"));
        assert!(fm.extra.contains_key("sources"));

        assert_eq!(body, "Body content");

        let serialized = serialize(&fm, body);
        assert!(serialized.contains("type: knowledge_concept"));
        assert!(serialized.contains("confidence: 0.9"));
        assert!(serialized.contains("sources:"));
        assert!(
            serialized.contains("- '[[kimball-dwt]]'")
                || serialized.contains("- \"[[kimball-dwt]]\"")
                || serialized.contains("- [[kimball-dwt]]")
        );
    }
}
