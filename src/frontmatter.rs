pub mod properties;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    #[default]
    Unix,
    Iso8601,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WritePolicy {
    pub title: bool,
    pub tags: bool,
    pub pinned: bool,
    pub links: bool,
    pub text_align: bool,
    pub updated_at: TimestampFormat,
    /// Compatibility only: the original extension has no automatic producer.
    pub original_ext: bool,
}
impl Default for WritePolicy {
    fn default() -> Self {
        Self {
            title: true,
            tags: true,
            pinned: true,
            links: true,
            text_align: true,
            updated_at: TimestampFormat::Unix,
            original_ext: true,
        }
    }
}

pub fn parse(content: &str) -> (Frontmatter, &str) {
    let Ok(frame) = properties::PropertyFrame::parse(content) else {
        return (Frontmatter::default(), content);
    };
    let Ok(mapping) = properties::parse_mapping(frame.yaml()) else {
        return (Frontmatter::default(), frame.body);
    };
    let mut fm = Frontmatter {
        extra: mapping.clone(),
        ..Frontmatter::default()
    };
    fm.title = mapping
        .get("title")
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_owned);
    fm.updated_at = mapping.get("updated_at").and_then(timestamp);
    fm.tags = mapping
        .get("tags")
        .and_then(|v| serde_yaml_ng::from_value(v.clone()).ok())
        .unwrap_or_default();
    fm.pinned = mapping
        .get("pinned")
        .and_then(serde_yaml_ng::Value::as_bool)
        .unwrap_or(false);
    fm.links = mapping
        .get("links")
        .and_then(|v| serde_yaml_ng::from_value(v.clone()).ok());
    fm.original_ext = mapping
        .get("original_ext")
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_owned);
    fm.text_align = mapping
        .get("text_align")
        .and_then(|v| serde_yaml_ng::from_value(v.clone()).ok());
    for key in [
        "title",
        "updated_at",
        "tags",
        "pinned",
        "links",
        "original_ext",
        "text_align",
    ] {
        fm.extra.remove(key);
    }
    (fm, frame.body)
}

pub(crate) fn timestamp(value: &serde_yaml_ng::Value) -> Option<u64> {
    if let Some(value) = value.as_u64() {
        return Some(value);
    }
    let text = value.as_str()?;
    let seconds = chrono::DateTime::parse_from_rfc3339(text)
        .map(|date| date.timestamp())
        .ok()
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)
                .map(|date| date.and_utc().timestamp())
        })?;
    u64::try_from(seconds).ok()
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
