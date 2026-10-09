use crate::config::TextAlignment;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::str::FromStr;
use yaml_edit::{AsYaml, YamlFile, YamlNode};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrontmatterEdit {
    pub key_yaml: String,
    pub value_yaml: Option<String>,
    /// Explicit source key for a rename; rename does not replace property value.
    #[serde(default)]
    pub rename_from: Option<String>,
}

/// Split framing without decoding the body (which may be ciphertext).
pub fn split_header(bytes: &[u8]) -> Result<(Option<&str>, &[u8])> {
    let opening = if bytes.starts_with(b"---\r\n") {
        5
    } else if bytes.starts_with(b"---\n") {
        4
    } else if bytes == b"---" {
        bail!("Unclosed frontmatter header");
    } else {
        return Ok((None, bytes));
    };
    let mut start = opening;
    while start < bytes.len() {
        let end = bytes[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |n| start + n);
        let line = bytes[start..end]
            .strip_suffix(b"\r")
            .unwrap_or(&bytes[start..end]);
        if line == b"---" {
            let boundary = if end < bytes.len() { end + 1 } else { end };
            return Ok((
                Some(
                    std::str::from_utf8(&bytes[..boundary])
                        .context("Frontmatter header is not UTF-8")?,
                ),
                &bytes[boundary..],
            ));
        }
        start = end.saturating_add(1);
    }
    bail!("Unclosed frontmatter header")
}

fn framing(header: &str) -> Result<(&str, &str, &str)> {
    let (found, payload) = split_header(header.as_bytes())?;
    ensure!(
        found.is_some() && payload.is_empty(),
        "Expected one complete frontmatter header"
    );
    let opening = if header.starts_with("---\r\n") { 5 } else { 4 };
    let closing_end = header.strip_suffix('\n').unwrap_or(header);
    let closing_end = closing_end.strip_suffix('\r').unwrap_or(closing_end);
    let closing_start = closing_end.len() - 3;
    Ok((
        &header[..opening],
        &header[opening..closing_start],
        &header[closing_start..],
    ))
}

pub fn is_managed_key(key: &Value) -> bool {
    matches!(
        key.as_str(),
        Some("title" | "updated_at" | "tags" | "pinned" | "links" | "original_ext" | "text_align")
    )
}

fn semantic_document(yaml: &str) -> Result<Value> {
    let mut documents = serde_yaml_ng::Deserializer::from_str(yaml);
    let value = match documents.next() {
        Some(document) => Value::deserialize(document)?,
        None => Value::Null,
    };
    ensure!(
        documents.next().is_none(),
        "Expected exactly one YAML document"
    );
    Ok(value)
}

/// All writers validate before touching disk; reader fallback stays permissive.
pub fn validate_header(header: &str) -> Result<Mapping> {
    let (_, yaml, _) = framing(header)?;
    let mapping = match semantic_document(yaml)? {
        Value::Mapping(mapping) => mapping,
        Value::Null
            if yaml
                .lines()
                .all(|line| line.trim().is_empty() || line.trim_start().starts_with('#')) =>
        {
            Mapping::new()
        }
        _ => bail!("Frontmatter must be a YAML mapping"),
    };
    let managed: Mapping = mapping
        .iter()
        .filter(|(key, _)| is_managed_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    serde_yaml_ng::from_value::<Frontmatter>(Value::Mapping(managed))
        .context("Invalid managed frontmatter field")?;
    let file = YamlFile::from_str(yaml).context("Unsupported frontmatter syntax")?;
    ensure!(
        file.documents().count() <= 1,
        "Expected one frontmatter document"
    );
    ensure!(
        file.to_string() == yaml,
        "YAML parser cannot preserve this header"
    );
    Ok(mapping)
}

pub fn checked_parse(header: &str) -> Result<Frontmatter> {
    let mut mapping = validate_header(header)?;
    let managed: Mapping = mapping
        .iter()
        .filter(|(key, _)| is_managed_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut fm: Frontmatter = serde_yaml_ng::from_value(Value::Mapping(managed))?;
    mapping.retain(|key, _| !is_managed_key(key));
    fm.extra = mapping;
    Ok(fm)
}

fn fragment(yaml: &str) -> Result<YamlNode> {
    let file = YamlFile::from_str(yaml).context("Invalid YAML fragment")?;
    ensure!(
        file.documents().count() == 1,
        "Expected exactly one YAML value"
    );
    let document = file.document().context("Missing YAML value")?;
    document
        .as_node()
        .context("Missing YAML node")?
        .children()
        .find_map(YamlNode::from_syntax)
        .context("Missing YAML value")
}

pub fn apply_edits(header: &str, edits: &[FrontmatterEdit]) -> Result<String> {
    let mut original = validate_header(header)?;
    if edits.is_empty() {
        return Ok(header.to_owned());
    }
    let (opening, yaml, closing) = framing(header)?;
    let mut file = YamlFile::from_str(yaml)?;
    let mut touched = Vec::with_capacity(edits.len());
    for edit in edits {
        // Reparse after each mutation so key lookup sees renamed CST entries.
        file = YamlFile::from_str(&file.to_string())?;
        let before = opening.ends_with("\r\n").then(|| file.to_string());
        let document = file.ensure_document();
        let mapping = document
            .as_mapping()
            .context("Frontmatter must be a YAML mapping")?;
        let key = fragment(&edit.key_yaml)?;
        let semantic_key = semantic_document(&edit.key_yaml)?;
        if let Some(source) = &edit.rename_from {
            ensure!(
                edit.value_yaml.is_none(),
                "Rename cannot replace property value"
            );
            let source_key = fragment(source)?;
            let source_semantic = semantic_document(source)?;
            crate::property_model::validate_key(
                source_semantic
                    .as_str()
                    .context("Rename supports string keys")?,
            )?;
            crate::property_model::validate_key(
                semantic_key
                    .as_str()
                    .context("Rename supports string keys")?,
            )?;
            ensure!(mapping.contains_key(&source_key), "Property does not exist");
            if source_semantic == semantic_key {
                continue;
            }
            ensure!(!mapping.contains_key(&key), "Property already exists");
            ensure!(
                mapping.rename_key(&source_key, &key),
                "Property rename failed"
            );
            if let Some(value) = original.remove(&source_semantic) {
                original.insert(semantic_key.clone(), value);
            }
            if touched.contains(&source_semantic) {
                touched.push(semantic_key.clone());
            }
            touched.push(source_semantic);
        } else {
            match &edit.value_yaml {
                Some(value) => {
                    let value_node = fragment(value)?;
                    // Preserve presentation for semantic no-ops, including quoted scalars.
                    if let Ok(semantic_value) = semantic_document(value)
                        && original.get(&semantic_key) == Some(&semantic_value)
                        && !touched.contains(&semantic_key)
                        && (touched.is_empty()
                            || semantic_document(&file.to_string()).is_ok_and(|value| {
                                value.as_mapping().is_some_and(|values| {
                                    values.get(&semantic_key) == Some(&semantic_value)
                                })
                            }))
                    {
                        continue;
                    }
                    if let Some(entry) = mapping.find_entry_by_key(&key) {
                        entry.set_value(&value_node, mapping.is_flow_style());
                    } else {
                        mapping.set(&key, &value_node);
                    }
                }
                None => {
                    mapping.remove(&key);
                }
            }
            touched.push(semantic_key);
        }
        if let Some(before) = before {
            let after = file.to_string();
            let mut prefix = before
                .bytes()
                .zip(after.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            while !after.is_char_boundary(prefix) {
                prefix -= 1;
            }
            let mut suffix = before[prefix..]
                .bytes()
                .rev()
                .zip(after[prefix..].bytes().rev())
                .take_while(|(a, b)| a == b)
                .count();
            while !after.is_char_boundary(after.len() - suffix) {
                suffix -= 1;
            }
            let middle = after[prefix..after.len() - suffix]
                .replace("\r\n", "\n")
                .replace('\n', "\r\n");
            file = YamlFile::from_str(&format!(
                "{}{middle}{}",
                &after[..prefix],
                &after[after.len() - suffix..]
            ))?;
        }
    }
    let mut updated = file.to_string();
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push_str(if opening.ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        });
    }
    let candidate = format!("{opening}{updated}{closing}");
    validate_header(&candidate)?;
    Ok(candidate)
}

pub fn rename_key(header: &str, old: &str, new: &str) -> Result<String> {
    crate::property_model::validate_key(old)?;
    crate::property_model::validate_key(new)?;
    let semantic = validate_header(header)?;
    ensure!(
        semantic.contains_key(Value::String(old.into())),
        "Property does not exist"
    );
    ensure!(
        old == new || !semantic.contains_key(Value::String(new.into())),
        "Property already exists"
    );
    if old == new {
        return Ok(header.into());
    }
    apply_edits(
        header,
        &[FrontmatterEdit {
            key_yaml: serde_yaml_ng::to_string(&Value::String(new.into()))?,
            value_yaml: None,
            rename_from: Some(serde_yaml_ng::to_string(&Value::String(old.into()))?),
        }],
    )
}

pub fn parse(content: &str) -> (Frontmatter, &str) {
    if let Ok((Some(header), payload)) = split_header(content.as_bytes())
        && let Ok(fm) = checked_parse(header)
        && let Ok(body) = std::str::from_utf8(payload)
    {
        return (fm, body);
    }
    (Frontmatter::default(), content)
}

pub fn serialize(
    frontmatter: &Frontmatter,
    source_header: Option<&str>,
    content: &str,
) -> Result<String> {
    let empty = frontmatter.tags.is_empty()
        && !frontmatter.pinned
        && frontmatter.title.is_none()
        && frontmatter.updated_at.is_none()
        && frontmatter.links.is_none()
        && frontmatter.original_ext.is_none()
        && frontmatter.extra.is_empty()
        && frontmatter.text_align.is_none();
    if source_header.is_none() && empty {
        return Ok(content.to_owned());
    }
    let desired = serde_yaml_ng::to_value(frontmatter)?;
    let desired = desired
        .as_mapping()
        .context("Expected frontmatter mapping")?;
    let Some(header) = source_header else {
        return Ok(format!(
            "---\n{}---\n{content}",
            serde_yaml_ng::to_string(frontmatter)?
        ));
    };
    let existing = validate_header(header)?;
    let mut edits = Vec::new();
    for (key, value) in desired {
        if existing.get(key) != Some(value) {
            edits.push(FrontmatterEdit {
                key_yaml: serde_yaml_ng::to_string(key)?,
                value_yaml: Some(serde_yaml_ng::to_string(value)?),
                rename_from: None,
            });
        }
    }
    for key in existing.keys() {
        if !desired.contains_key(key) {
            edits.push(FrontmatterEdit {
                key_yaml: serde_yaml_ng::to_string(key)?,
                value_yaml: None,
                rename_from: None,
            });
        }
    }
    let mut header = apply_edits(header, &edits)?;
    if !content.is_empty() && !header.ends_with('\n') {
        header.push_str(if header.starts_with("---\r\n") {
            "\r\n"
        } else {
            "\n"
        });
    }
    Ok(format!("{header}{content}"))
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
        let serialized = serialize(&fm, None, content).unwrap();
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

        let serialized = serialize(&fm, split_header(content.as_bytes()).unwrap().0, body).unwrap();
        assert!(serialized.contains("type: knowledge_concept"));
        assert!(serialized.contains("confidence: 0.9"));
        assert!(serialized.contains("sources:"));
        assert!(
            serialized.contains("- '[[kimball-dwt]]'")
                || serialized.contains("- \"[[kimball-dwt]]\"")
                || serialized.contains("- [[kimball-dwt]]")
        );
    }

    #[test]
    fn frontmatter_properties_preservation_and_validation() {
        for newline in ["\n", "\r\n"] {
            let source = "---\n# identity\nid: \"001\" # keep\nstatus: open # state\nsources: [one, 'two']\ndetails:\n  score: 0.9\nsummary: |-\n  First\n  Second\nconfidence: high\n---\n".replace('\n', newline);
            let edits = [
                FrontmatterEdit {
                    key_yaml: "status".into(),
                    value_yaml: Some("answered".into()),
                    rename_from: None,
                },
                FrontmatterEdit {
                    key_yaml: "reviewed".into(),
                    value_yaml: Some("true".into()),
                    rename_from: None,
                },
                FrontmatterEdit {
                    key_yaml: "confidence".into(),
                    value_yaml: None,
                    rename_from: None,
                },
            ];
            let result = apply_edits(&source, &edits).unwrap();
            let expected = source.replace("status: open", "status: answered").replace(
                &format!("confidence: high{newline}"),
                &format!("reviewed: true{newline}"),
            );
            assert_eq!(result, expected);
            assert_eq!(apply_edits(&source, &[]).unwrap(), source);
            assert_eq!(
                apply_edits(
                    &source,
                    &[FrontmatterEdit {
                        key_yaml: "id".into(),
                        value_yaml: Some("'001'".into()),
                        rename_from: None,
                    }]
                )
                .unwrap(),
                source
            );
        }
        let mixed =
            "---\r\nid: '001'\nstatus: café # keep\r\nsummary: |-\n  first\n  second\r\n---\r\n";
        assert_eq!(
            apply_edits(
                mixed,
                &[FrontmatterEdit {
                    key_yaml: "status".into(),
                    value_yaml: Some("cafê".into()),
                    rename_from: None,
                }]
            )
            .unwrap(),
            mixed.replace("status: café", "status: cafê")
        );
        let repeated = [
            FrontmatterEdit {
                key_yaml: "x".into(),
                value_yaml: Some("changed".into()),
                rename_from: None,
            },
            FrontmatterEdit {
                key_yaml: "x".into(),
                value_yaml: Some("original".into()),
                rename_from: None,
            },
        ];
        assert_eq!(validate_header(&apply_edits("---\nx: original\n---\n", &repeated).unwrap()).unwrap()["x"].as_str(), Some("original"));
        for (yaml, expected) in [
            ("'true'", Value::String("true".into())),
            ("true", Value::Bool(true)),
            ("2", Value::Number(2.into())),
            ("null", Value::Null),
            ("|-\n  one\n  two\n", Value::String("one\ntwo".into())),
            (
                "{score: 2, approved: false}",
                serde_yaml_ng::from_str("{score: 2, approved: false}").unwrap(),
            ),
            (
                "!custom value",
                serde_yaml_ng::from_str("!custom value").unwrap(),
            ),
        ] {
            let result = apply_edits(
                "---\n---\n",
                &[FrontmatterEdit {
                    key_yaml: "custom".into(),
                    value_yaml: Some(yaml.into()),
                    rename_from: None,
                }],
            )
            .unwrap();
            assert_eq!(
                validate_header(&result).unwrap().get("custom"),
                Some(&expected)
            );
        }
        let nonstring = apply_edits(
            "---\n1: old\n---\n",
            &[FrontmatterEdit {
                key_yaml: "1".into(),
                value_yaml: Some("new".into()),
                rename_from: None,
            }],
        )
        .unwrap();
        assert_eq!(
            validate_header(&nonstring)
                .unwrap()
                .get(Value::Number(1.into())),
            Some(&Value::String("new".into()))
        );
        let anchors = "---\nbase: &base [one, two]\ncopy: *base\n---\n";
        let result = apply_edits(
            anchors,
            &[FrontmatterEdit {
                key_yaml: "third".into(),
                value_yaml: Some("*base".into()),
                rename_from: None,
            }],
        )
        .unwrap();
        assert_eq!(
            validate_header(&result).unwrap().get("third"),
            validate_header(anchors).unwrap().get("base")
        );
        assert!(
            apply_edits(
                anchors,
                &[FrontmatterEdit {
                    key_yaml: "base".into(),
                    value_yaml: None,
                    rename_from: None,
                }]
            )
            .is_err()
        );
        for source in [
            "---\nx: 1\nx: 2\n---\n",
            "---\n[one]\n---\n",
            "---\nx: [\n---\n",
            "---\ntags: true\n---\n",
        ] {
            assert!(apply_edits(source, &[]).is_err(), "{source}");
        }
    }

    #[test]
    fn frontmatter_complete_delimiters_and_binary_payload() {
        let source = b"---\ntext: |-\n  ---\n  ---suffix\n---suffix: keep\n---\n\xff\x00";
        let (header, payload) = split_header(source).unwrap();
        assert!(header.unwrap().contains("---suffix: keep"));
        assert_eq!(payload, b"\xff\x00");
        assert!(split_header(b"---\nx: 1\n---suffix\n").is_err());
        assert_eq!(split_header(b"---suffix\nbody").unwrap().0, None);
        assert!(split_header(b"---\n\xff\n---\n").is_err());
        let header = "---\nx: 1\n---";
        let saved = serialize(&checked_parse(header).unwrap(), Some(header), "new body").unwrap();
        assert_eq!(parse(&saved).1, "new body");
    }
}
