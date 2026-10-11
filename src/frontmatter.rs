use crate::config::TextAlignment;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::str::FromStr;

/// Controls automatic writes, not explicit edits in the YAML field.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WritePolicy {
    pub title: bool,
    pub tags: bool,
    pub pinned: bool,
    pub links: bool,
    pub original_ext: bool,
    pub text_align: bool,
    pub updated_at: TimestampFormat,
}

impl Default for WritePolicy {
    fn default() -> Self {
        Self {
            title: true,
            tags: true,
            pinned: true,
            links: true,
            original_ext: true,
            text_align: true,
            updated_at: TimestampFormat::Unix,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    Unix,
    Iso8601,
    None,
}

impl WritePolicy {
    pub fn values(&self, fm: &Frontmatter) -> anyhow::Result<Mapping> {
        let Value::Mapping(mut values) = serde_yaml_ng::to_value(fm)? else {
            unreachable!();
        };
        for (key, enabled) in [
            ("title", self.title),
            ("tags", self.tags),
            ("pinned", self.pinned),
            ("links", self.links),
            ("original_ext", self.original_ext),
            ("text_align", self.text_align),
            ("updated_at", self.updated_at != TimestampFormat::None),
        ] {
            if !enabled {
                values.remove(Value::from(key));
            }
        }
        if self.updated_at == TimestampFormat::Iso8601
            && let Some(seconds) = fm.updated_at
        {
            let time = chrono::DateTime::from_timestamp(i64::try_from(seconds)?, 0)
                .ok_or_else(|| anyhow::anyhow!("updated_at is outside the ISO date range"))?;
            values.insert(
                Value::from("updated_at"),
                Value::from(time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            );
        }
        Ok(values)
    }
}

fn deserialize_timestamp<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("updated_at must be nonnegative")),
        Some(Value::String(text)) => {
            let seconds = chrono::DateTime::parse_from_rfc3339(&text)
                .map(|time| time.timestamp())
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d")
                        .map(|date| date.and_time(chrono::NaiveTime::MIN).and_utc().timestamp())
                })
                .map_err(serde::de::Error::custom)?;
            u64::try_from(seconds)
                .map(Some)
                .map_err(serde::de::Error::custom)
        }
        _ => Err(serde::de::Error::custom(
            "updated_at must be a Unix timestamp or ISO date",
        )),
    }
}

/// Patch changed values only. Never fall back to reserializing user YAML.
/// A semantic check catches parser disagreements and anchor side effects before I/O.
pub fn patch_yaml(text: &str, updates: Mapping) -> anyhow::Result<String> {
    let mut expected: Mapping = if text.trim().is_empty() {
        Mapping::new()
    } else {
        serde_yaml_ng::from_str(text)?
    };
    let updates: Mapping = updates
        .into_iter()
        .filter(|(key, value)| expected.get(key) != Some(value))
        .collect();
    if updates.is_empty() {
        return Ok(text.to_string());
    }
    let crlf = text.contains("\r\n");
    let normalized = text.replace("\r\n", "\n");
    if normalized.trim().is_empty() {
        return Ok(serde_yaml_ng::to_string(&updates)?
            .trim_end_matches('\n')
            .to_string());
    }
    let file = yaml_edit::YamlFile::from_str(&normalized)?;
    let doc = file
        .documents()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Frontmatter must be a YAML mapping"))?;
    let mapping = doc
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Frontmatter must be a YAML mapping"))?;
    for (key, value) in updates {
        let name = key
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid managed field name"))?;
        let single = Mapping::from_iter([(key.clone(), value.clone())]);
        let replacement = yaml_edit::Document::from_str(&serde_yaml_ng::to_string(&single)?)?;
        let node = replacement
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Cannot encode frontmatter field {name}"))?;
        mapping.set(name, node);
        expected.insert(key, value);
    }
    let mut result = file.to_string();
    // The syntax editor may add a framing newline. Keep the original convention.
    if !normalized.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    anyhow::ensure!(
        serde_yaml_ng::from_str::<Mapping>(&result)? == expected,
        "Cannot safely update this YAML; edit the affected fields explicitly"
    );
    if crlf {
        result = result.replace('\n', "\r\n");
    }
    Ok(result)
}

pub fn frame(header: Option<&str>, body: &str, newline: &str) -> String {
    match header {
        Some(header) if !header.is_empty() => {
            format!("---{newline}{header}{newline}---{newline}{body}")
        }
        _ => body.to_string(),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_timestamp")]
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
        let title = if fm.title.is_some() && fm.title != previous.title {
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
                Some(
                    content[start..offset]
                        .strip_suffix("\r\n")
                        .or_else(|| content[start..offset].strip_suffix('\n'))
                        .unwrap_or(&content[start..offset]),
                ),
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
    let (header, body) = split_raw(content);
    match header.and_then(|text| parse_yaml(text).ok()) {
        Some(fm) => (fm, body),
        None => (Frontmatter::default(), content),
    }
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
