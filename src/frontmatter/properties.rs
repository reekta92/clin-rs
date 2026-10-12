//! Generic, bounded property parsing and source-preserving property mutation.
//! The clin `Frontmatter` projection is deliberately not this data model.

use anyhow::{Context, Result, bail};
use serde::de::{DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
use serde_yaml_ng::{Mapping, Value};
use std::fmt;
use std::ops::Range;
use std::str::FromStr;

const MAX_YAML_BYTES: usize = 1024 * 1024;
const MAX_YAML_NODES: usize = 100_000;
const MAX_YAML_DEPTH: usize = 64;

pub struct PropertyDocument<'a> {
    pub properties: Mapping,
    pub body: &'a str,
    yaml_range: Option<Range<usize>>,
    bom_len: usize,
    newline: &'static str,
}

impl<'a> PropertyDocument<'a> {
    pub fn parse(source: &'a str) -> Result<Self> {
        let bom_len = usize::from(source.starts_with('\u{feff}')) * '\u{feff}'.len_utf8();
        let after_bom = &source[bom_len..];
        let first = after_bom.split_inclusive('\n').next().unwrap_or("");
        let newline = if first.ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        if first.trim_end() != "---" {
            return Ok(Self {
                properties: Mapping::new(),
                body: after_bom,
                yaml_range: None,
                bom_len,
                newline,
            });
        }
        let start = bom_len + first.len();
        let mut end = start;
        for line in source[start..].split_inclusive('\n') {
            if matches!(line.trim_end(), "---" | "...") {
                return Ok(Self {
                    properties: parse_mapping(&source[start..end])?,
                    body: &source[end + line.len()..],
                    yaml_range: Some(start..end),
                    bom_len,
                    newline,
                });
            }
            end += line.len();
        }
        bail!("frontmatter opening delimiter has no closing delimiter");
    }
}

/// Shared by note metadata and `.base` configuration. Bounds alias expansion as
/// well as source size; limiting input bytes alone cannot stop an alias bomb.
pub(crate) fn parse_mapping(source: &str) -> Result<Mapping> {
    if source.len() > MAX_YAML_BYTES {
        bail!("YAML exceeds 1 MiB parsing limit");
    }
    if source.trim().is_empty() {
        return Ok(Mapping::new());
    }
    let mut remaining = MAX_YAML_NODES;
    let mut documents = serde_yaml_ng::Deserializer::from_str(source);
    let deserializer = documents.next().context("missing YAML document")?;
    let value = BoundedValue {
        remaining: &mut remaining,
        depth: 0,
    }
    .deserialize(deserializer)?;
    if documents.next().is_some() {
        bail!("expected one YAML document");
    }
    match value {
        Value::Mapping(mapping) => Ok(mapping),
        Value::Null => Ok(Mapping::new()),
        _ => bail!("properties must be a YAML mapping"),
    }
}

struct BoundedValue<'a> {
    remaining: &'a mut usize,
    depth: usize,
}

impl BoundedValue<'_> {
    fn child(&mut self) -> BoundedValue<'_> {
        BoundedValue {
            remaining: self.remaining,
            depth: self.depth + 1,
        }
    }
}

impl<'de> DeserializeSeed<'de> for BoundedValue<'_> {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Value, D::Error> {
        if *self.remaining == 0 || self.depth > MAX_YAML_DEPTH {
            return Err(serde::de::Error::custom(
                "YAML exceeds node/depth safety limit",
            ));
        }
        *self.remaining -= 1;
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for BoundedValue<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded YAML value")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E>(self, value: i64) -> std::result::Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E>(self, value: u64) -> std::result::Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E>(self, value: f64) -> std::result::Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_str<E>(self, value: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(value.into()))
    }
    fn visit_string<E>(self, value: String) -> std::result::Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_none<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<S: SeqAccess<'de>>(
        mut self,
        mut sequence: S,
    ) -> std::result::Result<Value, S::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(self.child())? {
            values.push(value);
        }
        Ok(Value::Sequence(values))
    }

    fn visit_map<M: MapAccess<'de>>(
        mut self,
        mut mapping: M,
    ) -> std::result::Result<Value, M::Error> {
        let mut values = Mapping::new();
        while let Some(key) = mapping.next_key_seed(self.child())? {
            let value = mapping.next_value_seed(self.child())?;
            if values.insert(key, value).is_some() {
                return Err(serde::de::Error::custom("duplicate YAML property key"));
            }
        }
        Ok(Value::Mapping(values))
    }

    fn visit_enum<A: EnumAccess<'de>>(mut self, data: A) -> std::result::Result<Value, A::Error> {
        let (tag, variant) = data.variant::<String>()?;
        let value = variant.newtype_variant_seed(self.child())?;
        Ok(Value::Tagged(Box::new(serde_yaml_ng::value::TaggedValue {
            tag: serde_yaml_ng::value::Tag::new(tag),
            value,
        })))
    }
}

fn replacement_node(value: &Value) -> Result<yaml_edit::YamlNode> {
    match value {
        Value::Sequence(values) => {
            let sequence = yaml_edit::Sequence::new_flow();
            for value in values {
                sequence.push(replacement_node(value)?);
            }
            Ok(yaml_edit::YamlNode::Sequence(sequence))
        }
        Value::Mapping(values) => {
            let mapping = yaml_edit::Mapping::new_flow();
            for (key, value) in values {
                mapping.set(replacement_node(key)?, replacement_node(value)?);
            }
            Ok(yaml_edit::YamlNode::Mapping(mapping))
        }
        _ => {
            let source = if let Value::String(value) = value {
                format!("value: {}\n", serde_json::to_string(value)?)
            } else {
                serde_yaml_ng::to_string(&std::collections::BTreeMap::from([("value", value)]))?
            };
            yaml_edit::YamlFile::from_str(&source)?
                .document()
                .and_then(|document| document.get("value"))
                .context("missing replacement value")
        }
    }
}

/// Only requested keys change; malformed or lossy edits fail before any write.
/// `None` removes a key; `Some(Value::Null)` writes an explicit null.
pub fn edit_properties(source: &str, changes: &[(String, Option<Value>)]) -> Result<String> {
    let parsed = PropertyDocument::parse(source)?;
    let mut expected = parsed.properties.clone();
    for (key, value) in changes {
        let key = Value::String(key.clone());
        match value {
            Some(value) => {
                expected.insert(key, value.clone());
            }
            None => {
                expected.remove(&key);
            }
        }
    }
    if expected == parsed.properties {
        return Ok(source.to_owned());
    }
    let updated = if let Some(range) = parsed.yaml_range {
        let original = &source[range.clone()];
        // Empty frontmatter still has comments to preserve when adding its first key.
        let yaml = yaml_edit::YamlFile::from_str(original)
            .context("frontmatter cannot be edited losslessly")?;
        let document = yaml.ensure_document();
        if document.as_mapping().is_none() && !parsed.properties.is_empty() {
            bail!("frontmatter has no editable mapping");
        }
        for (key, value) in changes {
            match value {
                Some(value) => {
                    // New collections use flow form so block and JSON-style parents both work.
                    let node = replacement_node(value)?;
                    if !document.set(key.as_str(), node) {
                        bail!("property could not be changed losslessly");
                    }
                }
                None => {
                    document.remove(key.as_str());
                }
            }
        }
        let mut edited = yaml.to_string();
        if parsed.newline == "\r\n" && !original.replace("\r\n", "").contains('\n') {
            edited = edited.replace("\r\n", "\n").replace('\n', "\r\n");
        }
        if !edited.is_empty() && !edited.ends_with('\n') {
            edited.push_str(parsed.newline);
        }
        format!(
            "{}{}{}",
            &source[..range.start],
            edited,
            &source[range.end..]
        )
    } else {
        let yaml = serde_yaml_ng::to_string(&expected)?;
        let yaml = if parsed.newline == "\r\n" {
            yaml.replace('\n', "\r\n")
        } else {
            yaml
        };
        format!(
            "{}---{}{}---{}{}",
            &source[..parsed.bom_len],
            parsed.newline,
            yaml,
            parsed.newline,
            parsed.body
        )
    };
    let verified = PropertyDocument::parse(&updated)?;
    if verified.properties != expected || verified.body != parsed.body {
        bail!("property edit would change unrelated data; original retained");
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_properties_do_not_inherit_clin_field_types() {
        let source = "---\ntitle: 123\nupdated_at: 2024-01-01\nlinks: '[[A]]'\ntags: '#work'\nobject: {nested: true}\n---\nBody";
        let parsed = PropertyDocument::parse(source).unwrap();
        assert_eq!(parsed.properties["title"], Value::Number(123.into()));
        assert_eq!(parsed.properties["links"], Value::String("[[A]]".into()));
        assert_eq!(parsed.body, "Body");
    }

    #[test]
    fn edit_preserves_body_unrelated_comments_quotes_and_crlf() {
        for newline in ["\n", "\r\n"] {
            let source = "---\n# heading\nstatus: draft # inline\nuntouched: 'quoted' # keep\nobject: { a: 1, b: [x, y] }\n---\n\n# Body\n---body\n".replace('\n', newline);
            let edited = edit_properties(
                &source,
                &[("status".into(), Some(Value::String("ready".into())))],
            )
            .unwrap();
            assert!(edited.contains("# heading"));
            assert!(edited.contains("# inline"));
            assert!(edited.contains(&format!(
                "untouched: 'quoted' # keep{newline}object: {{ a: 1, b: [x, y] }}{newline}"
            )));
            assert_eq!(
                PropertyDocument::parse(&edited).unwrap().body,
                PropertyDocument::parse(&source).unwrap().body
            );
        }
    }

    #[test]
    fn properties_can_be_added_removed_and_cleared_without_body_changes() {
        for source in [
            "Body\n",
            "\u{feff}Body\n",
            "---\n---\nBody\n",
            "---\n# keep\n---\nBody\n",
            "---\n{\"name\": \"old\", \"untouched\": [1, 2]}\n---\nBody\n",
        ] {
            let body = PropertyDocument::parse(source).unwrap().body;
            let edited = edit_properties(
                source,
                &[
                    ("name".into(), Some(Value::String("null".into()))),
                    ("null".into(), Some(Value::Null)),
                    (
                        "list".into(),
                        Some(Value::Sequence(vec![
                            Value::Bool(true),
                            Value::Number(0.into()),
                        ])),
                    ),
                ],
            )
            .unwrap();
            let parsed = PropertyDocument::parse(&edited).unwrap();
            assert_eq!(parsed.body, body);
            assert_eq!(parsed.properties["name"], Value::String("null".into()));
            assert_eq!(parsed.properties["null"], Value::Null);
            assert_eq!(
                parsed.properties["list"],
                Value::Sequence(vec![Value::Bool(true), Value::Number(0.into())])
            );
            let removed = edit_properties(&edited, &[("name".into(), None)]).unwrap();
            assert!(
                !PropertyDocument::parse(&removed)
                    .unwrap()
                    .properties
                    .contains_key("name")
            );
        }
    }

    #[test]
    fn nested_values_and_multiline_text_work_in_flow_frontmatter() {
        let source = "---\n{\"keep\": 1}\n---\nBody";
        let value: Value = serde_yaml_ng::from_str("{nested: [true, {n: 0}], empty: []}").unwrap();
        let edited = edit_properties(
            source,
            &[
                ("object".into(), Some(value.clone())),
                ("text".into(), Some(Value::String("one\ntwo\n".into()))),
            ],
        )
        .unwrap();
        let parsed = PropertyDocument::parse(&edited).unwrap();
        assert_eq!(parsed.properties["object"], value);
        assert_eq!(
            parsed.properties["text"],
            Value::String("one\ntwo\n".into())
        );
        assert_eq!(parsed.properties["keep"], Value::Number(1.into()));
        assert_eq!(parsed.body, "Body");
    }

    #[test]
    fn new_keys_follow_existing_crlf_convention() {
        let source = "---\r\n# keep\r\nx: 1\r\n---\r\nBody\r\n";
        let edited = edit_properties(source, &[("y".into(), Some(Value::Bool(false)))]).unwrap();
        assert!(!edited.replace("\r\n", "").contains('\n'));
        assert!(edited.contains("# keep\r\nx: 1\r\n"));
    }

    #[test]
    fn malformed_frontmatter_and_duplicate_keys_never_become_empty_data() {
        for source in [
            "---\na: [\n---\nBody",
            "---\na: 1\na: 2\n---\nBody",
            "---\n- list\n---\nBody",
            "---\na: 1\n---not-delimiter\nBody",
        ] {
            assert!(PropertyDocument::parse(source).is_err(), "{source}");
            assert!(edit_properties(source, &[("x".into(), Some(Value::Bool(true)))]).is_err());
        }
    }

    #[test]
    fn noop_edits_are_byte_preserving() {
        let source = "---\n# keep\nx: 'yes'\n---\nBody";
        assert_eq!(
            edit_properties(source, &[("x".into(), Some(Value::String("yes".into())))]).unwrap(),
            source
        );
        assert_eq!(edit_properties(source, &[]).unwrap(), source);
    }

    #[test]
    fn ordinary_aliases_are_supported_but_expansion_is_bounded() {
        let parsed = parse_mapping("a: &a [1, 2]\nb: *a\n").unwrap();
        assert_eq!(parsed["a"], parsed["b"]);
        let mut bomb = "a: &a [0,0,0,0,0,0,0,0,0,0]\n".to_owned();
        for (key, prev) in [('b', 'a'), ('c', 'b'), ('d', 'c'), ('e', 'd'), ('f', 'e')] {
            bomb.push_str(&format!(
                "{key}: &{key} [{aliases}]\n",
                aliases = std::iter::repeat_n(format!("*{prev}"), 10)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        assert!(parse_mapping(&bomb).is_err());
        assert!(parse_mapping(&format!("x: {}0{}", "[".repeat(70), "]".repeat(70))).is_err());
        assert!(parse_mapping(&"x".repeat(MAX_YAML_BYTES + 1)).is_err());
    }

    #[test]
    fn tagged_and_block_values_survive_unrelated_edits() {
        let source = "---\nx: !custom hello\npoem: |\n  one\n  two\nstatus: old\n---\nBody";
        let edited = edit_properties(
            source,
            &[("status".into(), Some(Value::String("new".into())))],
        )
        .unwrap();
        assert!(edited.contains("x: !custom hello\npoem: |\n  one\n  two\n"));
    }
}
