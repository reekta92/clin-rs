//! Typed query values. Strings retain UTF-16 units until terminal/UTF-8 output.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use serde_yaml_ng::Value as Yaml;

pub(crate) const MAX_VALUE_SIZE: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Text(pub Vec<u16>);

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self(value.encode_utf16().collect())
    }
}
impl From<String> for Text {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}
impl Text {
    pub fn display(&self) -> String {
        String::from_utf16_lossy(&self.0)
    }
    pub(crate) fn utf8(&self) -> Result<String> {
        String::from_utf16(&self.0).context("string contains an unpaired UTF-16 surrogate")
    }
    pub(crate) fn contains(&self, needle: &Self) -> bool {
        needle.0.is_empty() || self.0.windows(needle.0.len()).any(|part| part == needle.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Duration {
    pub months: f64,
    pub milliseconds: f64,
}
impl Duration {
    pub fn parse(source: &str) -> Result<Self> {
        let source = source.trim();
        let split = source
            .find(|ch: char| ch.is_alphabetic())
            .context("duration requires a unit")?;
        let amount: f64 = source[..split]
            .trim()
            .parse()
            .context("invalid duration amount")?;
        if !amount.is_finite() {
            bail!("duration amount must be finite");
        }
        let (months, milliseconds) = match source[split..].trim() {
            "y" | "year" | "years" => (12.0 * amount, 0.0),
            "M" | "month" | "months" => (amount, 0.0),
            "w" | "week" | "weeks" => (0.0, 604_800_000.0 * amount),
            "d" | "day" | "days" => (0.0, 86_400_000.0 * amount),
            "h" | "hour" | "hours" => (0.0, 3_600_000.0 * amount),
            "m" | "minute" | "minutes" => (0.0, 60_000.0 * amount),
            "s" | "second" | "seconds" => (0.0, 1000.0 * amount),
            _ => bail!("unknown duration unit"),
        };
        if !months.is_finite() || !milliseconds.is_finite() {
            bail!("duration exceeds numeric range");
        }
        Ok(Self {
            months,
            milliseconds,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub target: Text,
    pub source: String,
    pub display: Option<Box<Value>>,
}
impl Link {
    pub(crate) fn new(target: Text, source: &str, display: Option<Value>) -> Self {
        let text = target.display();
        let (target, alias) = if let Some(wiki) = text
            .strip_prefix("[[")
            .and_then(|value| value.strip_suffix("]]"))
        {
            let (target, alias) = wiki
                .split_once('|')
                .map_or((wiki, None), |(target, alias)| {
                    (target, Some(Value::text(alias)))
                });
            (Text::from(target), alias)
        } else {
            (target, None)
        };
        Self {
            target,
            source: source.to_owned(),
            display: display.or(alias).map(Box::new),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Number(f64),
    String(Text),
    Date(DateTime<Local>),
    Duration(Duration),
    List(Vec<Value>),
    /// Insertion order is observable through keys()/values().
    Object(Vec<(Text, Value)>),
    File(String),
    Host(String),
    Link(Link),
    Image(Text),
    Icon(Text),
    Html(Text),
    Regex {
        pattern: String,
        flags: String,
    },
}

impl Value {
    pub fn text(value: &str) -> Self {
        Self::String(value.into())
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Date(_) => "date",
            Self::Duration(_) => "duration",
            Self::List(_) => "list",
            Self::Object(_) => "object",
            Self::File(_) | Self::Host(_) => "file",
            Self::Link(_) => "link",
            Self::Image(_) => "image",
            Self::Icon(_) => "icon",
            Self::Html(_) => "html",
            Self::Regex { .. } => "regexp",
        }
    }
    pub fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Boolean(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            Self::String(value) => !value.0.is_empty(),
            _ => true,
        }
    }
    pub fn empty(&self) -> bool {
        match self {
            Self::Null => true,
            Self::String(value) => value.0.is_empty(),
            Self::List(value) => value.is_empty(),
            Self::Object(value) => value.is_empty(),
            _ => false,
        }
    }
    pub fn to_text(&self) -> Text {
        match self {
            Self::Null => "null".into(),
            Self::Boolean(value) => value.to_string().into(),
            Self::Number(value) => ryu_js::Buffer::new().format(*value).into(),
            Self::String(value) | Self::Image(value) | Self::Icon(value) | Self::Html(value) => {
                value.clone()
            }
            Self::Date(value) => value.to_rfc3339().into(),
            Self::Duration(value) => format!("{}M {}ms", value.months, value.milliseconds).into(),
            Self::File(id) | Self::Host(id) => id.as_str().into(),
            Self::Link(value) => value.target.clone(),
            Self::Regex { pattern, flags } => format!("/{pattern}/{flags}").into(),
            Self::Object(_) => "[object Object]".into(),
            Self::List(values) => {
                let mut result = Vec::new();
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        result.push(u16::from(b','));
                    }
                    if !matches!(value, Self::Null) {
                        result.extend(value.to_text().0);
                    }
                }
                Text(result)
            }
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        let mut pending = vec![(self, 0)];
        let mut size = 0usize;
        while let Some((value, depth)) = pending.pop() {
            if depth > 64 {
                bail!("value nesting exceeds 64 levels");
            }
            size += 1;
            match value {
                Self::String(text) | Self::Image(text) | Self::Icon(text) | Self::Html(text) => {
                    size = size.saturating_add(text.0.len())
                }
                Self::List(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
                Self::Object(values) => {
                    for (key, value) in values {
                        size = size.saturating_add(key.0.len());
                        pending.push((value, depth + 1));
                    }
                }
                Self::Link(link) => {
                    size = size.saturating_add(link.target.0.len());
                    if let Some(display) = &link.display {
                        pending.push((display, depth + 1));
                    }
                }
                _ => {}
            }
            if size > MAX_VALUE_SIZE {
                bail!("value result exceeds 1 Mi units");
            }
        }
        Ok(())
    }

    pub(crate) fn as_text(&self) -> Result<&Text> {
        match self {
            Self::String(value) => Ok(value),
            _ => bail!("expected string, got {}", self.type_name()),
        }
    }
    pub(crate) fn number(&self) -> Result<f64> {
        match self {
            Self::Number(value) => Ok(*value),
            Self::Boolean(value) => Ok(f64::from(*value)),
            Self::Null => Ok(0.0),
            Self::Date(value) => Ok(value.timestamp_millis() as f64),
            Self::String(value) => {
                let value = value.utf8()?;
                let value = value.trim_matches(js_whitespace);
                if value.is_empty() {
                    return Ok(0.0);
                }
                for (prefix, radix) in [
                    ("0x", 16),
                    ("0X", 16),
                    ("0b", 2),
                    ("0B", 2),
                    ("0o", 8),
                    ("0O", 8),
                ] {
                    if let Some(digits) = value.strip_prefix(prefix) {
                        if digits.is_empty() {
                            bail!("invalid numeric string");
                        }
                        let mut result = 0.0;
                        for ch in digits.chars() {
                            result = result * f64::from(radix)
                                + f64::from(ch.to_digit(radix).context("invalid numeric string")?);
                        }
                        return Ok(result);
                    }
                }
                if value == "NaN" {
                    bail!("invalid numeric string");
                }
                value.parse().context("invalid numeric string")
            }
            Self::List(_) => Self::String(self.to_text()).number(),
            _ => bail!("cannot convert {} to number", self.type_name()),
        }
    }
    pub(crate) fn from_yaml(value: &Yaml, source: &str) -> Result<Self> {
        Ok(match value {
            Yaml::Null => Self::Null,
            Yaml::Bool(value) => Self::Boolean(*value),
            Yaml::Number(value) => {
                Self::Number(value.as_f64().context("number exceeds supported range")?)
            }
            Yaml::String(value) if value.starts_with("[[") && value.ends_with("]]") => {
                Self::Link(Link::new(value.as_str().into(), source, None))
            }
            Yaml::String(value) => Self::text(value),
            Yaml::Sequence(values) => Self::List(
                values
                    .iter()
                    .map(|value| Self::from_yaml(value, source))
                    .collect::<Result<_>>()?,
            ),
            Yaml::Mapping(values) => Self::Object(
                values
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            Self::from_yaml(key, source)?.to_text(),
                            Self::from_yaml(value, source)?,
                        ))
                    })
                    .collect::<Result<_>>()?,
            ),
            Yaml::Tagged(value) => Self::from_yaml(&value.value, source)?,
        })
    }
}

pub(crate) fn js_whitespace(ch: char) -> bool {
    matches!(ch, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

pub(crate) fn parse_date(source: &str) -> Result<DateTime<Local>> {
    if let Ok(date) = DateTime::parse_from_rfc3339(source) {
        return Ok(date.with_timezone(&Local));
    }
    for format in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(date) = NaiveDateTime::parse_from_str(source, format) {
            return Local
                .from_local_datetime(&date)
                .earliest()
                .context("date falls in a local timezone gap");
        }
    }
    let date = NaiveDate::parse_from_str(source, "%Y-%m-%d").context("invalid date")?;
    Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).context("invalid midnight")?)
        .earliest()
        .context("date falls in a local timezone gap")
}
