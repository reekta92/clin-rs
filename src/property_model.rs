use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Component, Path};

use crate::frontmatter::{self, FrontmatterEdit};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    #[default]
    String,
    Number,
    Boolean,
    Null,
    Yaml,
    Date,
    DateTime,
    List,
    Select,
    MultiSelect,
    NoteReference,
    NoteReferences,
}
impl PropertyKind {
    pub const ALL: [Self; 12] = [
        Self::String,
        Self::Number,
        Self::Boolean,
        Self::Null,
        Self::Yaml,
        Self::Date,
        Self::DateTime,
        Self::List,
        Self::Select,
        Self::MultiSelect,
        Self::NoteReference,
        Self::NoteReferences,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::String => "String",
            Self::Number => "Number",
            Self::Boolean => "Boolean",
            Self::Null => "Null",
            Self::Yaml => "YAML",
            Self::Date => "Date",
            Self::DateTime => "Date/time",
            Self::List => "List",
            Self::Select => "Select",
            Self::MultiSelect => "Multi-select",
            Self::NoteReference => "Note reference",
            Self::NoteReferences => "Note references",
        }
    }
    pub fn of(value: &Value) -> Self {
        match value {
            Value::String(_) => Self::String,
            Value::Number(_) => Self::Number,
            Value::Bool(_) => Self::Boolean,
            Value::Null => Self::Null,
            Value::Sequence(_) => Self::List,
            _ => Self::Yaml,
        }
    }
    pub fn encode(self, input: &str) -> Result<String> {
        if self == Self::Yaml {
            return Ok(input.into());
        }
        Ok(serde_yaml_ng::to_string(&parse_property_value(
            self, input,
        )?)?)
    }
    pub fn is_reference(self) -> bool {
        matches!(self, Self::NoteReference | Self::NoteReferences)
    }
    #[must_use]
    pub fn cycle(self, delta: isize) -> Self {
        let index = Self::ALL.iter().position(|kind| *kind == self).unwrap_or(0);
        Self::ALL[(index as isize + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Number(f64),
    String(String),
    List(Vec<Self>),
    Unsupported,
}
pub type PropertyMap = BTreeMap<String, PropertyValue>;
impl PropertyValue {
    pub fn from_yaml(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(v) => Self::Bool(*v),
            Value::String(v) => Self::String(v.clone()),
            Value::Number(v) => v
                .as_i64()
                .map(Self::Integer)
                .or_else(|| v.as_u64().map(Self::Unsigned))
                .or_else(|| v.as_f64().map(Self::Number))
                .unwrap_or(Self::Unsupported),
            Value::Sequence(v) => Self::List(v.iter().map(Self::from_yaml).collect()),
            _ => Self::Unsupported,
        }
    }
    pub fn to_yaml(&self) -> Result<Value> {
        Ok(match self {
            Self::Null => Value::Null,
            Self::Bool(v) => Value::Bool(*v),
            Self::Integer(v) => serde_yaml_ng::to_value(v)?,
            Self::Unsigned(v) => serde_yaml_ng::to_value(v)?,
            Self::Number(v) => serde_yaml_ng::to_value(v)?,
            Self::String(v) => Value::String(v.clone()),
            Self::List(v) => Value::Sequence(v.iter().map(Self::to_yaml).collect::<Result<_>>()?),
            Self::Unsupported => bail!("Complex YAML value requires YAML editor"),
        })
    }
    pub fn display(&self) -> String {
        match self {
            Self::String(v) => v.clone(),
            Self::Null => "null".into(),
            Self::Bool(v) => v.to_string(),
            Self::Integer(v) => v.to_string(),
            Self::Unsigned(v) => v.to_string(),
            Self::Number(v) => v.to_string(),
            Self::List(v) => format!(
                "[{}]",
                v.iter().map(Self::display).collect::<Vec<_>>().join(", ")
            ),
            Self::Unsupported => "(YAML)".into(),
        }
    }
    pub fn date(&self) -> Option<NaiveDate> {
        let Self::String(value) = self else {
            return None;
        };
        strict_date(value).ok().or_else(|| {
            DateTime::parse_from_rfc3339(value)
                .ok()
                .map(|date| date.with_timezone(&Local).date_naive())
        })
    }
    pub fn word_goal(&self) -> Option<usize> {
        match self {
            Self::Integer(v) => usize::try_from(*v).ok(),
            Self::Unsigned(v) => usize::try_from(*v).ok(),
            Self::Number(v)
                if v.is_finite() && *v >= 0.0 && v.fract() == 0.0 && *v < usize::MAX as f64 =>
            {
                Some(*v as usize)
            }
            _ => None,
        }
    }
}
pub fn properties_from_mapping(mapping: &Mapping) -> PropertyMap {
    mapping
        .iter()
        .filter_map(|(key, value)| {
            key.as_str()
                .filter(|_| !frontmatter::is_managed_key(key))
                .map(|key| (key.to_owned(), PropertyValue::from_yaml(value)))
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropertyDefinition {
    #[serde(rename = "type")]
    pub kind: PropertyKind,
    pub description: String,
    pub default: Option<toml::Value>,
    pub options: Vec<String>,
}
impl PropertyDefinition {
    pub fn validate(&self, value: &Value) -> Result<()> {
        validate_kind(self.kind, value)?;
        match (self.kind, value) {
            (PropertyKind::Select, Value::String(value)) => ensure!(
                self.options.contains(value),
                "Value is not a declared select option"
            ),
            (PropertyKind::MultiSelect, Value::Sequence(values)) => ensure!(
                values.iter().all(|v| v
                    .as_str()
                    .is_some_and(|v| self.options.iter().any(|option| option == v))),
                "Value is not a declared multi-select option"
            ),
            _ => {}
        }
        Ok(())
    }
    pub fn validate_indexed(&self, value: &PropertyValue) -> bool {
        match (self.kind, value) {
            (PropertyKind::Yaml, _) => true,
            (PropertyKind::Null, PropertyValue::Null) | (PropertyKind::Boolean, PropertyValue::Bool(_)) | (PropertyKind::Number, PropertyValue::Integer(_) | PropertyValue::Unsigned(_)) | (PropertyKind::String, PropertyValue::String(_)) => true,
            (PropertyKind::Number, PropertyValue::Number(value)) => value.is_finite(),
            (PropertyKind::Date, PropertyValue::String(value)) => strict_date(value).is_ok(),
            (PropertyKind::DateTime, PropertyValue::String(value)) => DateTime::parse_from_rfc3339(value).is_ok(),
            (PropertyKind::Select, PropertyValue::String(value)) => self.options.contains(value),
            (PropertyKind::List, PropertyValue::List(values)) => values.iter().all(|value| !matches!(value, PropertyValue::Unsupported | PropertyValue::List(_))),
            (PropertyKind::MultiSelect, PropertyValue::List(values)) => values.iter().all(|value| matches!(value, PropertyValue::String(value) if self.options.contains(value))),
            (PropertyKind::NoteReference, PropertyValue::String(value)) => validate_reference(value).is_ok(),
            (PropertyKind::NoteReferences, PropertyValue::List(values)) => values.iter().all(|value| matches!(value, PropertyValue::String(value) if validate_reference(value).is_ok())),
            _ => false,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropertyDefinitions {
    pub properties: BTreeMap<String, PropertyDefinition>,
}
impl PropertyDefinitions {
    pub fn path(vault: &Path) -> std::path::PathBuf {
        vault.join(".clin").join("properties.toml")
    }
    pub fn load(vault: &Path) -> Result<Self> {
        let path = Self::path(vault);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error).context("Failed to read property definitions"),
        };
        let definitions: Self = toml::from_str(&text).context("Invalid property definitions")?;
        definitions.check()?;
        Ok(definitions)
    }
    pub fn check(&self) -> Result<()> {
        for (key, definition) in &self.properties {
            validate_key(key)?;
            let mut options = std::collections::BTreeSet::new();
            ensure!(
                definition
                    .options
                    .iter()
                    .all(|option| options.insert(option)),
                "Duplicate options for property {key}"
            );
            ensure!(
                definition
                    .options
                    .iter()
                    .all(|option| !option.is_empty() && !option.contains(['\r', '\n'])),
                "Select options must be nonempty and single-line for {key}"
            );
            if matches!(
                definition.kind,
                PropertyKind::Select | PropertyKind::MultiSelect
            ) {
                ensure!(
                    !definition.options.is_empty(),
                    "Property {key} requires select options"
                );
            }
            if let Some(value) = &definition.default {
                definition
                    .validate(&toml_to_yaml(value)?)
                    .with_context(|| format!("Invalid default for {key}"))?;
            }
        }
        Ok(())
    }
    pub fn save(&self, vault: &Path) -> Result<()> {
        self.check()?;
        let path = Self::path(vault);
        let mut document = match std::fs::read_to_string(&path) {
            Ok(text) => text
                .parse::<toml_edit::DocumentMut>()
                .context("Invalid existing property definitions")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                toml_edit::DocumentMut::new()
            }
            Err(error) => return Err(error).context("Failed to read property definitions"),
        };
        let desired: toml_edit::DocumentMut = toml_edit::ser::to_string_pretty(self)?.parse()?;
        for (key, value) in desired.iter() {
            if let Some(existing) = document.get_mut(key) {
                crate::config::merge::merge_edit_item(existing, value.clone());
            } else {
                document.insert(key, value.clone());
            }
        }
        std::fs::create_dir_all(vault.join(".clin"))?;
        crate::fsutil::atomic_write(&path, document.to_string().as_bytes())
    }
    pub fn defaults(&self) -> Result<Vec<FrontmatterEdit>> {
        self.properties
            .iter()
            .filter_map(|(key, definition)| definition.default.as_ref().map(|value| (key, value)))
            .map(|(key, value)| property_edit(key, Some(&toml_to_yaml(value)?)))
            .collect()
    }
    pub fn validate(&self, key: &str, value: &Value) -> Result<()> {
        validate_key(key)?;
        if let Some(definition) = self.properties.get(key) {
            definition
                .validate(value)
                .with_context(|| format!("Invalid property {key}"))?;
        }
        Ok(())
    }
    pub fn date_value(&self, key: &str, values: &PropertyMap) -> Option<NaiveDate> {
        let definition = self.properties.get(key)?;
        let value = values.get(key)?;
        if matches!(definition.kind, PropertyKind::Date | PropertyKind::DateTime)
            && definition.validate_indexed(value)
        {
            value.date()
        } else {
            None
        }
    }
    pub fn diagnostics(&self, properties: &PropertyMap) -> Vec<String> {
        self.properties
            .iter()
            .filter_map(|(key, definition)| {
                properties
                    .get(key)
                    .filter(|value| !definition.validate_indexed(value))
                    .map(|_| format!("{key}: invalid {} value", definition.kind.label()))
            })
            .collect()
    }
}
pub(crate) fn inferred_definition<'a>(
    definitions: &PropertyDefinitions,
    notes: impl IntoIterator<Item = &'a crate::storage::NoteSummary>,
    key: &str,
    kind: PropertyKind,
    value: &Value,
    choices: &[String],
) -> Result<Option<PropertyDefinition>> {
    definitions.validate(key, value)?;
    if let Some(definition) = definitions.properties.get(key) {
        ensure!(
            kind == definition.kind || kind == PropertyKind::Yaml,
            "Property {key} is defined as {}; change type through definition manager",
            definition.kind.label()
        );
        return Ok(None);
    }
    if !matches!(
        kind,
        PropertyKind::Date
            | PropertyKind::DateTime
            | PropertyKind::Select
            | PropertyKind::MultiSelect
            | PropertyKind::NoteReference
            | PropertyKind::NoteReferences
    ) {
        return Ok(None);
    }
    let mut options = Vec::new();
    if matches!(kind, PropertyKind::Select | PropertyKind::MultiSelect) {
        options.extend_from_slice(choices);
        let values: Vec<_> = match value {
            Value::Sequence(values) => values.iter().filter_map(Value::as_str).collect(),
            _ => value.as_str().into_iter().collect(),
        };
        for value in values {
            if !options.iter().any(|option| option == value) {
                options.push(value.into());
            }
        }
    }
    let definition = PropertyDefinition {
        kind,
        options,
        ..Default::default()
    };
    definition.validate(value)?;
    ensure!(
        notes
            .into_iter()
            .filter_map(|note| note.properties.get(key))
            .all(|value| definition.validate_indexed(value)),
        "Existing notes conflict with {key} type; review conflicts in definition manager"
    );
    Ok(Some(definition))
}
pub fn validate_key(key: &str) -> Result<()> {
    ensure!(
        !key.trim().is_empty() && !key.contains(['\r', '\n']),
        "Name must be nonempty and single-line"
    );
    ensure!(
        !frontmatter::is_managed_key(&Value::String(key.into())),
        "Managed by Clin; use existing note controls"
    );
    Ok(())
}
pub fn property_edit(key: &str, value: Option<&Value>) -> Result<FrontmatterEdit> {
    validate_key(key)?;
    Ok(FrontmatterEdit {
        key_yaml: serde_yaml_ng::to_string(&Value::String(key.into()))?,
        value_yaml: value.map(serde_yaml_ng::to_string).transpose()?,
        rename_from: None,
    })
}
pub fn toml_to_yaml(value: &toml::Value) -> Result<Value> {
    match value {
        toml::Value::Datetime(value) => Ok(Value::String(value.to_string())),
        toml::Value::Array(values) => Ok(Value::Sequence(
            values.iter().map(toml_to_yaml).collect::<Result<_>>()?,
        )),
        toml::Value::Table(values) => Ok(Value::Mapping(
            values
                .iter()
                .map(|(key, value)| Ok((Value::String(key.clone()), toml_to_yaml(value)?)))
                .collect::<Result<_>>()?,
        )),
        _ => Ok(serde_yaml_ng::to_value(value)?),
    }
}
fn strict_date(value: &str) -> Result<NaiveDate> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").context("Expected YYYY-MM-DD date")?;
    ensure!(
        value.len() == 10 && value.as_bytes()[4] == b'-' && value.as_bytes()[7] == b'-',
        "Expected YYYY-MM-DD date"
    );
    Ok(date)
}
/// Normalize only explicitly declared reference fields; raw YAML is never scanned.
pub(crate) fn reference_target(value: &str) -> &str {
    let value = value
        .strip_prefix("[[")
        .and_then(|v| v.strip_suffix("]]"))
        .unwrap_or(value);
    value
        .split('|')
        .next()
        .unwrap_or(value)
        .split('#')
        .next()
        .unwrap_or(value)
        .trim()
}
pub(crate) fn reference_links(
    values: &PropertyMap,
    definitions: &PropertyDefinitions,
) -> Vec<String> {
    let mut links = std::collections::BTreeSet::new();
    for (key, definition) in &definitions.properties {
        if !definition.kind.is_reference() {
            continue;
        }
        let Some(value) = values
            .get(key)
            .filter(|value| definition.validate_indexed(value))
        else {
            continue;
        };
        match value {
            PropertyValue::String(value) => {
                links.insert(reference_target(value).to_owned());
            }
            PropertyValue::List(values) => {
                for value in values {
                    if let PropertyValue::String(value) = value {
                        links.insert(reference_target(value).to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    links.into_iter().collect()
}
pub(crate) fn refresh_reference_links(
    notes: &mut [crate::storage::NoteSummary],
    definitions: &PropertyDefinitions,
) {
    for note in notes {
        note.property_links = reference_links(&note.properties, definitions);
    }
}
pub(crate) fn resolve_reference<'a>(
    notes: &'a [crate::storage::NoteSummary],
    target: &str,
) -> Option<&'a crate::storage::NoteSummary> {
    let target = reference_target(target);
    if let Some(note) = notes.iter().find(|note| note.id == target) {
        return Some(note);
    }
    let mut matches = notes
        .iter()
        .filter(|note| note.title.eq_ignore_ascii_case(target));
    let note = matches.next()?;
    matches.next().is_none().then_some(note)
}

pub fn validate_reference(value: &str) -> Result<()> {
    let value = value
        .strip_prefix("[[")
        .and_then(|v| v.strip_suffix("]]"))
        .unwrap_or(value);
    let value = value.split('|').next().unwrap_or(value);
    ensure!(
        !value.trim().is_empty() && !value.contains(['\r', '\n', '\0', '\\']),
        "Expected vault-relative note reference"
    );
    ensure!(
        Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir)),
        "Note reference must stay inside vault"
    );
    Ok(())
}
pub fn validate_kind(kind: PropertyKind, value: &Value) -> Result<()> {
    let valid = match kind {
        PropertyKind::Yaml => true,
        PropertyKind::String | PropertyKind::Select => value.is_string(),
        PropertyKind::Null => value.is_null(),
        PropertyKind::Boolean => value.is_bool(),
        PropertyKind::Number => {
            matches!(value, Value::Number(number) if number.as_f64().is_some_and(f64::is_finite))
        }
        PropertyKind::Date => value
            .as_str()
            .is_some_and(|value| strict_date(value).is_ok()),
        PropertyKind::DateTime => value
            .as_str()
            .is_some_and(|value| DateTime::parse_from_rfc3339(value).is_ok()),
        PropertyKind::List => value.as_sequence().is_some_and(|values| {
            values.iter().all(|value| {
                !matches!(
                    value,
                    Value::Mapping(_) | Value::Sequence(_) | Value::Tagged(_)
                )
            })
        }),
        PropertyKind::MultiSelect => value
            .as_sequence()
            .is_some_and(|values| values.iter().all(Value::is_string)),
        PropertyKind::NoteReference => value
            .as_str()
            .is_some_and(|value| validate_reference(value).is_ok()),
        PropertyKind::NoteReferences => value.as_sequence().is_some_and(|values| {
            values.iter().all(|value| {
                value
                    .as_str()
                    .is_some_and(|value| validate_reference(value).is_ok())
            })
        }),
    };
    ensure!(valid, "Expected {}", kind.label());
    Ok(())
}
pub fn parse_property_value(kind: PropertyKind, input: &str) -> Result<Value> {
    let value = match kind {
        PropertyKind::String
        | PropertyKind::Select
        | PropertyKind::Date
        | PropertyKind::DateTime
        | PropertyKind::NoteReference => Value::String(input.into()),
        PropertyKind::Null => {
            ensure!(matches!(input.trim(), "" | "null" | "~"), "Expected null");
            Value::Null
        }
        PropertyKind::List | PropertyKind::MultiSelect | PropertyKind::NoteReferences
            if !input.trim_start().starts_with('[')
                || kind == PropertyKind::NoteReferences && input.trim_start().starts_with("[[") =>
        {
            Value::Sequence(if input.is_empty() {
                Vec::new()
            } else {
                input
                    .split('\n')
                    .map(|line| Value::String(line.into()))
                    .collect()
            })
        }
        _ => serde_yaml_ng::from_str(input).context("Invalid property value")?,
    };
    validate_kind(kind, &value)?;
    Ok(value)
}
fn integer(value: &PropertyValue) -> Option<i128> {
    match value {
        PropertyValue::Integer(v) => Some(i128::from(*v)),
        PropertyValue::Unsigned(v) => Some(i128::from(*v)),
        _ => None,
    }
}
fn integer_float(value: i128, float: f64) -> Option<Ordering> {
    if float.is_nan() {
        return None;
    }
    if float >= 18_446_744_073_709_551_616.0 {
        return Some(Ordering::Less);
    }
    if float < -9_223_372_036_854_775_808.0 {
        return Some(Ordering::Greater);
    }
    let whole = float as i128;
    Some(value.cmp(&whole).then_with(|| {
        0.0_f64
            .partial_cmp(&float.fract())
            .unwrap_or(Ordering::Equal)
    }))
}
pub fn compare_values(
    a: &PropertyValue,
    b: &PropertyValue,
    kind: Option<PropertyKind>,
) -> Option<Ordering> {
    if matches!(kind, Some(PropertyKind::Date | PropertyKind::DateTime)) {
        let (PropertyValue::String(a), PropertyValue::String(b)) = (a, b) else {
            return None;
        };
        return if kind == Some(PropertyKind::Date) {
            Some(strict_date(a).ok()?.cmp(&strict_date(b).ok()?))
        } else {
            Some(
                DateTime::parse_from_rfc3339(a)
                    .ok()?
                    .cmp(&DateTime::parse_from_rfc3339(b).ok()?),
            )
        };
    }
    if let (Some(a), Some(b)) = (integer(a), integer(b)) {
        return Some(a.cmp(&b));
    }
    if let (Some(a), PropertyValue::Number(b)) = (integer(a), b) {
        return integer_float(a, *b);
    }
    if let (PropertyValue::Number(a), Some(b)) = (a, integer(b)) {
        return integer_float(b, *a).map(Ordering::reverse);
    }
    match (a, b) {
        (PropertyValue::Number(a), PropertyValue::Number(b)) => a.partial_cmp(b),
        (PropertyValue::String(a), PropertyValue::String(b)) => Some(a.cmp(b)),
        (PropertyValue::Bool(a), PropertyValue::Bool(b)) => Some(a.cmp(b)),
        (PropertyValue::Null, PropertyValue::Null) => Some(Ordering::Equal),
        (PropertyValue::List(a), PropertyValue::List(b)) => {
            for (a, b) in a.iter().zip(b) {
                let order = compare_values(a, b, None).or_else(|| {
                    let rank = |value: &PropertyValue| match value {
                        PropertyValue::Null => Some(0),
                        PropertyValue::Bool(_) => Some(1),
                        PropertyValue::Integer(_)
                        | PropertyValue::Unsigned(_)
                        | PropertyValue::Number(_) => Some(2),
                        PropertyValue::String(_) => Some(3),
                        PropertyValue::List(_) => Some(4),
                        PropertyValue::Unsupported => None,
                    };
                    let (a, b) = (rank(a)?, rank(b)?);
                    (a != b).then(|| a.cmp(&b))
                })?;
                if order != Ordering::Equal {
                    return Some(order);
                }
            }
            Some(a.len().cmp(&b.len()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_values_and_definition_boundaries() {
        let high = PropertyValue::Unsigned(u64::MAX);
        assert_eq!(
            compare_values(
                &high,
                &PropertyValue::Number(18_446_744_073_709_551_616.0),
                None
            ),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare_values(
                &PropertyValue::Integer(9_007_199_254_740_993),
                &PropertyValue::Number(9_007_199_254_740_992.0),
                None
            ),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare_values(
                &PropertyValue::Bool(true),
                &PropertyValue::String("true".into()),
                None
            ),
            None
        );
        assert!(parse_property_value(PropertyKind::Date, "2026-02-30").is_err());
        assert!(parse_property_value(PropertyKind::DateTime, "2026-10-09T10:00:00").is_err());
        assert!(parse_property_value(PropertyKind::NoteReference, "../secret.md").is_err());
        let values = PropertyMap::from([
            ("flag".into(), PropertyValue::Bool(true)),
            ("large".into(), high),
            (
                "list".into(),
                PropertyValue::List(vec![
                    PropertyValue::Null,
                    PropertyValue::String("001".into()),
                ]),
            ),
        ]);
        let bytes = bincode::serde::encode_to_vec(&values, bincode::config::standard()).unwrap();
        let (decoded, used): (PropertyMap, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(decoded, values);
        assert_eq!(used, bytes.len());
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".clin")).unwrap();
        std::fs::write(
            PropertyDefinitions::path(dir.path()),
            "# keep\n[properties.flag]\ntype = \"boolean\" # type comment\ndefault = false\n",
        )
        .unwrap();
        let mut definitions = PropertyDefinitions::load(dir.path()).unwrap();
        definitions.properties.get_mut("flag").unwrap().default = Some(toml::Value::Boolean(true));
        definitions.save(dir.path()).unwrap();
        assert!(
            std::fs::read_to_string(PropertyDefinitions::path(dir.path()))
                .unwrap()
                .contains("# type comment")
        );
        assert_eq!(PropertyDefinitions::load(dir.path()).unwrap(), definitions);
    }
}
