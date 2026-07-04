use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

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
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_yaml_ng::Value>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&serde_yaml_ng::Value> {
        self.extra.get(key)
    }
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
    fn test_parse_and_serialize_extra_keys() {
        let content = "---\ntags:\n  - work\nstatus: done\nrating: 4\n---\nBody here";
        let (fm, remaining) = parse(content);
        assert_eq!(fm.tags, vec!["work"]);
        assert_eq!(fm.get("status").and_then(|v| v.as_str()), Some("done"));
        assert_eq!(fm.get("rating").and_then(|v| v.as_u64()), Some(4));
        assert_eq!(remaining, "Body here");

        let serialized = serialize(&fm, "Body here");
        let (fm2, remaining2) = parse(&serialized);
        assert_eq!(fm2.tags, vec!["work"]);
        assert_eq!(fm2.get("status").and_then(|v| v.as_str()), Some("done"));
        assert_eq!(fm2.get("rating").and_then(|v| v.as_u64()), Some(4));
        assert_eq!(remaining2, "Body here");
    }
}
