use crate::property_model::{
    PropertyDefinitions, PropertyKind, PropertyMap, PropertyValue, compare_values, toml_to_yaml,
    validate_key,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyOperator {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Contains,
    ContainsAny,
    ContainsAll,
    Exists,
    Missing,
    IsNull,
    IsEmpty,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PropertyPredicate {
    pub property: String,
    op: PropertyOperator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<toml::Value>,
    #[serde(skip)]
    operand: Option<PropertyValue>,
}
impl<'de> Deserialize<'de> for PropertyPredicate {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Input {
            property: String,
            op: PropertyOperator,
            value: Option<toml::Value>,
        }
        let input = Input::deserialize(deserializer)?;
        Self::new(input.property, input.op, input.value).map_err(serde::de::Error::custom)
    }
}
impl PropertyPredicate {
    pub fn new(property: String, op: PropertyOperator, value: Option<toml::Value>) -> Result<Self> {
        let operand = value
            .as_ref()
            .map(toml_to_yaml)
            .transpose()?
            .as_ref()
            .map(PropertyValue::from_yaml);
        let predicate = Self {
            property,
            op,
            value,
            operand,
        };
        predicate.validate()?;
        Ok(predicate)
    }
    pub fn validate(&self) -> Result<()> {
        validate_key(&self.property)?;
        let unary = matches!(
            self.op,
            PropertyOperator::Exists
                | PropertyOperator::Missing
                | PropertyOperator::IsNull
                | PropertyOperator::IsEmpty
        );
        ensure!(
            unary == self.value.is_none(),
            "Property {}: operator {:?} {} value",
            self.property,
            self.op,
            if unary {
                "does not accept a"
            } else {
                "requires a"
            }
        );
        if let Some(value) = &self.operand {
            match self.op {
                PropertyOperator::ContainsAny | PropertyOperator::ContainsAll => ensure!(
                    matches!(value, PropertyValue::List(_)),
                    "List operand required for {}",
                    self.property
                ),
                PropertyOperator::Lt
                | PropertyOperator::Lte
                | PropertyOperator::Gt
                | PropertyOperator::Gte => ensure!(
                    matches!(
                        value,
                        PropertyValue::String(_)
                            | PropertyValue::Integer(_)
                            | PropertyValue::Unsigned(_)
                            | PropertyValue::Number(_)
                    ),
                    "Ordered scalar operand required for {}",
                    self.property
                ),
                _ => ensure!(
                    *value != PropertyValue::Unsupported,
                    "Unsupported property operand"
                ),
            }
        }
        Ok(())
    }
    pub fn matches(&self, map: &PropertyMap, definitions: &PropertyDefinitions) -> bool {
        let actual = map.get(&self.property);
        match self.op {
            PropertyOperator::Exists => return actual.is_some(),
            PropertyOperator::Missing => return actual.is_none(),
            PropertyOperator::IsNull => return actual == Some(&PropertyValue::Null),
            PropertyOperator::IsEmpty => {
                return actual.is_some_and(|value| match value {
                    PropertyValue::String(v) => v.is_empty(),
                    PropertyValue::List(v) => v.is_empty(),
                    _ => false,
                });
            }
            _ => {}
        }
        let Some(actual) = actual else {
            return false;
        };
        let definition = definitions.properties.get(&self.property);
        if definition.is_some_and(|definition| !definition.validate_indexed(actual)) {
            return false;
        }
        let Some(expected) = &self.operand else {
            return false;
        };
        let kind = definition.map(|d| d.kind);
        let comparison = compare_values(actual, expected, kind);
        match self.op {
            PropertyOperator::Eq => comparison == Some(Ordering::Equal),
            PropertyOperator::Ne => comparison.is_some_and(|order| order != Ordering::Equal),
            PropertyOperator::Lt => comparison == Some(Ordering::Less),
            PropertyOperator::Lte => comparison.is_some_and(|order| order != Ordering::Greater),
            PropertyOperator::Gt => comparison == Some(Ordering::Greater),
            PropertyOperator::Gte => comparison.is_some_and(|order| order != Ordering::Less),
            PropertyOperator::Contains => match (actual, expected) {
                (PropertyValue::String(a), PropertyValue::String(b)) => a.contains(b),
                (PropertyValue::List(a), b) => a
                    .iter()
                    .any(|a| compare_values(a, b, None) == Some(Ordering::Equal)),
                _ => false,
            },
            PropertyOperator::ContainsAny | PropertyOperator::ContainsAll => {
                match (actual, expected) {
                    (PropertyValue::List(actual), PropertyValue::List(expected)) => {
                        let found = |value: &PropertyValue| {
                            actual.iter().any(|candidate| {
                                compare_values(candidate, value, None) == Some(Ordering::Equal)
                            })
                        };
                        if self.op == PropertyOperator::ContainsAll {
                            expected.iter().all(found)
                        } else {
                            expected.iter().any(found)
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
    pub fn description(&self) -> String {
        format!(
            "{} {:?}{}",
            self.property,
            self.op,
            self.value
                .as_ref()
                .map_or_else(String::new, |v| format!(" {v}"))
        )
    }
}

pub fn custom_folder_matches(
    rule: &crate::config::CustomSmartFolder,
    note: &crate::storage::NoteSummary,
    definitions: &PropertyDefinitions,
    now: u64,
) -> bool {
    rule.tags.iter().all(|tag| note.tags.contains(tag))
        && rule
            .title_contains
            .as_ref()
            .is_none_or(|value| note.title.to_lowercase().contains(&value.to_lowercase()))
        && rule
            .folder_prefix
            .as_ref()
            .is_none_or(|value| note.folder.starts_with(value))
        && rule
            .updated_within_days
            .is_none_or(|days| now.saturating_sub(note.updated_at) < days.saturating_mul(86400))
        && rule
            .all
            .iter()
            .all(|predicate| predicate.matches(&note.properties, definitions))
        && rule.any.as_ref().is_none_or(|predicates| {
            predicates
                .iter()
                .any(|predicate| predicate.matches(&note.properties, definitions))
        })
}

fn tokens(input: &str) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escape = false;
    let mut brackets = 0usize;
    for character in input.chars() {
        if escape {
            token.push(character);
            escape = false;
            continue;
        }
        if character == '\\' && quote == Some('"') {
            token.push(character);
            escape = true;
            continue;
        }
        if let Some(current) = quote {
            token.push(character);
            if character == current {
                quote = None;
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
            token.push(character);
        } else if character == '[' {
            brackets += 1;
            token.push(character);
        } else if character == ']' {
            brackets = brackets.saturating_sub(1);
            token.push(character);
        } else if character.is_whitespace() && brackets == 0 {
            if !token.is_empty() {
                result.push(std::mem::take(&mut token));
            }
        } else {
            token.push(character);
        }
    }
    ensure!(
        quote.is_none() && !escape && brackets == 0,
        "Unclosed property query quote/list"
    );
    if !token.is_empty() {
        result.push(token);
    }
    Ok(result)
}
fn key(input: &str) -> Result<String> {
    let value = if input.starts_with(['\'', '"']) {
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(input)?
            .as_str()
            .context("Expected property key string")?
            .to_owned()
    } else {
        input.to_owned()
    };
    validate_key(&value)?;
    Ok(value)
}
fn operand(input: &str) -> Result<toml::Value> {
    let yaml: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(input).context("Invalid property query operand")?;
    // TOML has no null; explicit null uses null:key instead.
    ensure!(!yaml.is_null(), "Use null:KEY for explicit null values");
    let json = serde_json::to_value(&yaml)?;
    let result: toml::Value =
        serde_json::from_value(json).context("Unsupported property query operand")?;
    Ok(result)
}
pub fn extract_property_filters(input: &str) -> Result<(String, Vec<PropertyPredicate>)> {
    // Preserve the old parser for ordinary searches, including unmatched quotes.
    if !input.split_whitespace().any(|part| {
        ["prop:", "has:", "missing:", "null:", "empty:"]
            .iter()
            .any(|prefix| part.starts_with(prefix))
    }) {
        return Ok((input.into(), Vec::new()));
    }
    if let Some(index) = input.find("\\e\\") {
        let (before, after) = input.split_at(index);
        let (remaining, predicates) = extract_property_filters(before)?;
        if predicates.is_empty() {
            return Ok((input.into(), predicates));
        }
        let gap = if before.ends_with(char::is_whitespace) && !remaining.is_empty() {
            " "
        } else {
            ""
        };
        return Ok((format!("{remaining}{gap}{after}"), predicates));
    }
    let mut remaining = Vec::new();
    let mut predicates = Vec::new();
    for token in tokens(input)? {
        let unary = [
            ("has:", PropertyOperator::Exists),
            ("missing:", PropertyOperator::Missing),
            ("null:", PropertyOperator::IsNull),
            ("empty:", PropertyOperator::IsEmpty),
        ];
        if let Some((prefix, op)) = unary.iter().find(|(prefix, _)| token.starts_with(prefix)) {
            predicates.push(PropertyPredicate::new(
                key(&token[prefix.len()..])?,
                *op,
                None,
            )?);
            continue;
        }
        let Some(expression) = token.strip_prefix("prop:") else {
            remaining.push(token);
            continue;
        };
        let mut quote = None;
        let mut escaped = false;
        let mut operator_at = None;
        for (index, character) in expression.char_indices() {
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if delimiter == '"' && character == '\\' {
                    escaped = true;
                } else if character == delimiter {
                    quote = None;
                }
                continue;
            }
            if matches!(character, '"' | '\'') {
                quote = Some(character);
                continue;
            }
            if matches!(character, '=' | '!' | '<' | '>' | '~') {
                operator_at = Some(index);
                break;
            }
        }
        let index =
            operator_at.context("Property filter requires =, !=, <, <=, >, >= or ~ (contains)")?;
        let tail = &expression[index..];
        let (operator, op) = [
            (">=", PropertyOperator::Gte),
            ("<=", PropertyOperator::Lte),
            ("!=", PropertyOperator::Ne),
            ("=", PropertyOperator::Eq),
            (">", PropertyOperator::Gt),
            ("<", PropertyOperator::Lt),
            ("~", PropertyOperator::Contains),
        ]
        .into_iter()
        .find(|(operator, _)| tail.starts_with(operator))
        .context("Invalid property query operator")?;
        predicates.push(PropertyPredicate::new(
            key(&expression[..index])?,
            op,
            Some(operand(&tail[operator.len()..])?),
        )?);
    }
    Ok((remaining.join(" "), predicates))
}

pub fn validate_date_binding(key: &str, definitions: &PropertyDefinitions) -> Result<PropertyKind> {
    let kind = definitions
        .properties
        .get(key)
        .context("Calendar property needs date/date_time definition")?
        .kind;
    ensure!(
        matches!(kind, PropertyKind::Date | PropertyKind::DateTime),
        "Calendar property needs date/date_time definition"
    );
    Ok(kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_distinguish_types_presence_and_quoted_operands() {
        let (text, rules) = extract_property_filters(
            "f:work prop:history_related=true prop:\"review status\"=\"needs work\"",
        )
        .unwrap();
        assert_eq!(text, "f:work");
        let map = PropertyMap::from([
            ("history_related".into(), PropertyValue::Bool(true)),
            (
                "review status".into(),
                PropertyValue::String("needs work".into()),
            ),
        ]);
        assert!(
            rules
                .iter()
                .all(|rule| rule.matches(&map, &PropertyDefinitions::default()))
        );
        let (_, rules) =
            extract_property_filters("prop:history_related=\"true\" missing:absent null:absent")
                .unwrap();
        assert!(!rules[0].matches(&map, &PropertyDefinitions::default()));
        assert!(rules[1].matches(&map, &PropertyDefinitions::default()));
        assert!(!rules[2].matches(&map, &PropertyDefinitions::default()));
        let (_, rules) = extract_property_filters("prop:absent!=false").unwrap();
        assert!(!rules[0].matches(&map, &PropertyDefinitions::default()));
        assert!(extract_property_filters("prop:score>=oops").is_ok());
    }
}
