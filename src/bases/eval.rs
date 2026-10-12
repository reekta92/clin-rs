//! Pure evaluation over public metadata. Every call shares cancellation and work limits.

use super::expr::{Expression, Kind, Literal, Node, Operator};
use super::value::{Duration, Link, MAX_VALUE_SIZE, Text, Value, js_whitespace, parse_date};
use crate::storage::metadata::VaultMetadata;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Local, Months, TimeDelta, Timelike};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Default)]
pub struct Formulas {
    expressions: BTreeMap<String, Expression>,
}
impl Formulas {
    pub fn compile(definitions: &BTreeMap<String, String>) -> Result<Self> {
        let expressions = definitions
            .iter()
            .map(|(name, source)| {
                Ok((
                    name.clone(),
                    Expression::parse(source).with_context(|| format!("formula {name:?}"))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut colors: HashMap<&str, u8> = HashMap::new();
        for name in expressions.keys() {
            let mut pending = vec![(name.as_str(), false, 0)];
            while let Some((name, leaving, depth)) = pending.pop() {
                if leaving {
                    colors.insert(name, 2);
                    continue;
                }
                match colors.get(name) {
                    Some(2) => continue,
                    Some(1) => bail!("circular formula reference involving {name:?}"),
                    _ => {}
                }
                if depth >= 64 {
                    bail!("formula dependency chain exceeds 64 levels");
                }
                let expression = expressions
                    .get(name)
                    .with_context(|| format!("unknown formula {name:?}"))?;
                colors.insert(name, 1);
                pending.push((name, true, depth));
                pending.extend(
                    expression
                        .dependencies()
                        .iter()
                        .map(|name| (name.as_str(), false, depth + 1)),
                );
            }
        }
        Ok(Self { expressions })
    }
}

#[derive(Clone, Debug)]
pub struct Environment {
    pub host: Option<String>,
    pub now: DateTime<Local>,
    /// Retain this seed during live refresh; replace when loading another view.
    pub random_seed: u64,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            host: None,
            now: Local::now(),
            random_seed: rand::random(),
        }
    }
}

pub(crate) struct Budget {
    remaining: usize,
}
impl Budget {
    pub(crate) fn new() -> Self {
        Self {
            remaining: 20_000_000,
        }
    }
    pub(crate) fn step(&mut self, cancelled: &AtomicBool) -> Result<()> {
        if self.remaining == 0 {
            bail!("Bases query work limit exceeded");
        }
        if self.remaining.is_multiple_of(256) && cancelled.load(Ordering::Relaxed) {
            bail!("Bases query cancelled");
        }
        self.remaining -= 1;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Identity {
    Null,
    Boolean(bool),
    Number(u64),
    String(Text),
    Date(i64),
    Duration(u64, u64),
    List(Vec<Self>),
    Object(Vec<(Text, Self)>),
    File(String),
    BrokenLink(Text),
    Decoration(&'static str, Text),
    Regex(String, String),
}

fn number_identity(value: f64) -> u64 {
    if value == 0.0 {
        0
    } else if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

struct Iteration {
    value: Value,
    index: usize,
    accumulator: Value,
}
pub(crate) struct Evaluator<'a> {
    metadata: &'a VaultMetadata,
    formulas: &'a Formulas,
    row: &'a str,
    environment: &'a Environment,
    cancelled: &'a AtomicBool,
    budget: &'a mut Budget,
    cache: HashMap<String, Value>,
    stack: Vec<String>,
    iteration: Vec<Iteration>,
    pub(crate) values: Option<Vec<Value>>,
    random_calls: u64,
    depth: usize,
}

impl<'a> Evaluator<'a> {
    pub(crate) fn new(
        metadata: &'a VaultMetadata,
        formulas: &'a Formulas,
        row: &'a str,
        environment: &'a Environment,
        cancelled: &'a AtomicBool,
        budget: &'a mut Budget,
    ) -> Self {
        Self {
            metadata,
            formulas,
            row,
            environment,
            cancelled,
            budget,
            cache: HashMap::new(),
            stack: Vec::new(),
            iteration: Vec::new(),
            values: None,
            random_calls: 0,
            depth: 0,
        }
    }
    pub(crate) fn expression(&mut self, expression: &Expression) -> Result<Value> {
        let value = self.node(&expression.root)?;
        value.validate()?;
        Ok(value)
    }
    pub(crate) fn step(&mut self) -> Result<()> {
        self.budget.step(self.cancelled)
    }

    pub(crate) fn property(&mut self, property: &str) -> Result<Value> {
        if let Some(name) = property.strip_prefix("formula.") {
            self.formula(name)
        } else if let Some(name) = property.strip_prefix("file.") {
            self.member(Value::File(self.row.to_owned()), &Value::text(name))
        } else {
            self.note_property(self.row, property.strip_prefix("note.").unwrap_or(property))
        }
    }
    fn formula(&mut self, name: &str) -> Result<Value> {
        if let Some(value) = self.cache.get(name) {
            return Ok(value.clone());
        }
        if self.stack.len() >= 64 || self.stack.iter().any(|entry| entry == name) {
            bail!("circular or excessive formula reference involving {name:?}");
        }
        let formulas = self.formulas;
        let expression = formulas
            .expressions
            .get(name)
            .with_context(|| format!("unknown formula {name:?}"))?;
        self.stack.push(name.to_owned());
        // Formula properties belong to the row, not the caller's list iteration scope.
        let iteration = std::mem::take(&mut self.iteration);
        let result = self.expression(expression);
        self.iteration = iteration;
        self.stack.pop();
        let value = result.with_context(|| format!("formula {name:?}"))?;
        self.cache.insert(name.to_owned(), value.clone());
        Ok(value)
    }
    fn note_property(&self, id: &str, key: &str) -> Result<Value> {
        let Some(file) = self.metadata.file(id) else {
            return Ok(Value::Null);
        };
        let Some(value) = file.properties()?.get(key) else {
            return Ok(Value::Null);
        };
        if matches!(
            self.metadata.property_types.declared_type(key),
            Some("date" | "datetime")
        ) && !value.is_null()
        {
            return Ok(Value::Date(parse_date(
                value.as_str().context("date property must be a string")?,
            )?));
        }
        Value::from_yaml(value, id)
    }
    fn all_properties(&self, id: &str) -> Result<Value> {
        let Some(file) = self.metadata.file(id) else {
            return Ok(Value::Null);
        };
        let mut values = Vec::new();
        for key in file.properties()?.keys() {
            let key = key
                .as_str()
                .context("note property name must be a string")?;
            values.push((Text::from(key), self.note_property(id, key)?));
        }
        Ok(Value::Object(values))
    }
    fn node(&mut self, node: &Node) -> Result<Value> {
        self.budget.step(self.cancelled)?;
        if self.depth >= 128 {
            bail!("expression evaluation nesting exceeds 128 levels");
        }
        self.depth += 1;
        let result = self.node_inner(node);
        self.depth -= 1;
        result
    }

    fn node_inner(&mut self, node: &Node) -> Result<Value> {
        let result = match &node.kind {
            Kind::Literal(value) => Ok(match value {
                Literal::Null => Value::Null,
                Literal::Boolean(value) => Value::Boolean(*value),
                Literal::Number(value) => Value::Number(*value),
                Literal::String(value) => Value::String(Text(value.clone())),
                Literal::Regex { pattern, flags } => Value::Regex {
                    pattern: pattern.clone(),
                    flags: flags.clone(),
                },
            }),
            Kind::Name(name) => self.name(name),
            Kind::Unary(operator, child) => {
                let value = self.node(child)?;
                match operator {
                    Operator::Not => Ok(Value::Boolean(!value.truthy())),
                    Operator::Add => Ok(Value::Number(value.number()?)),
                    Operator::Subtract => Ok(Value::Number(-value.number()?)),
                    _ => bail!("invalid unary operator"),
                }
            }
            Kind::Binary(Operator::And, left, right) => Ok(Value::Boolean(
                self.node(left)?.truthy() && self.node(right)?.truthy(),
            )),
            Kind::Binary(Operator::Or, left, right) => Ok(Value::Boolean(
                self.node(left)?.truthy() || self.node(right)?.truthy(),
            )),
            Kind::Binary(operator, left, right) => {
                let left = self.node(left)?;
                let right = self.node(right)?;
                self.binary(*operator, left, right)
            }
            Kind::Member(receiver, key) => {
                let key = self.node(key)?;
                match &receiver.kind {
                    Kind::Name(name) if name == "note" => {
                        self.note_property(self.row, &key.to_text().utf8()?)
                    }
                    Kind::Name(name) if name == "formula" => self.formula(&key.to_text().utf8()?),
                    _ => {
                        let receiver = self.node(receiver)?;
                        self.member(receiver, &key)
                    }
                }
            }
            Kind::Call(target, arguments) => self.call(target, arguments),
            Kind::List(nodes) => Ok(Value::List(
                nodes
                    .iter()
                    .map(|node| self.node(node))
                    .collect::<Result<_>>()?,
            )),
            Kind::Object(entries) => {
                let mut values: Vec<(Text, Value)> = Vec::new();
                for (key, node) in entries {
                    let value = self.node(node)?;
                    let key = Text(key.clone());
                    if let Some((_, existing)) =
                        values.iter_mut().find(|(existing, _)| existing == &key)
                    {
                        *existing = value;
                    } else {
                        values.push((key, value));
                    }
                }
                Ok(Value::Object(values))
            }
        };
        result.with_context(|| format!("expression byte {}", node.offset))
    }
    fn name(&mut self, name: &str) -> Result<Value> {
        match name {
            "file" => Ok(self
                .metadata
                .file(self.row)
                .map_or(Value::Null, |file| Value::File(file.id.clone()))),
            "this" => Ok(self
                .environment
                .host
                .as_deref()
                .and_then(|id| self.metadata.file(id))
                .map_or(Value::Null, |file| Value::Host(file.id.clone()))),
            "note" => self.all_properties(self.row),
            "formula" => {
                let names: Vec<_> = self.formulas.expressions.keys().cloned().collect();
                Ok(Value::Object(
                    names
                        .iter()
                        .map(|name| Ok((Text::from(name.as_str()), self.formula(name)?)))
                        .collect::<Result<_>>()?,
                ))
            }
            "values" if self.values.is_some() => Ok(Value::List(
                self.values.clone().expect("checked summary values"),
            )),
            "value" if !self.iteration.is_empty() => {
                Ok(self.iteration.last().expect("checked scope").value.clone())
            }
            "index" if !self.iteration.is_empty() => Ok(Value::Number(
                self.iteration.last().expect("checked scope").index as f64,
            )),
            "acc" if !self.iteration.is_empty() => Ok(self
                .iteration
                .last()
                .expect("checked scope")
                .accumulator
                .clone()),
            _ => self.note_property(self.row, name),
        }
    }
    fn member(&self, receiver: Value, key: &Value) -> Result<Value> {
        let key_text = key.to_text();
        let key = key_text.utf8()?;
        Ok(match receiver {
            Value::Null => Value::Null,
            Value::Object(values) => values
                .into_iter()
                .find(|(name, _)| name == &key_text)
                .map_or(Value::Null, |(_, value)| value),
            Value::List(values) => {
                if key == "length" {
                    Value::Number(values.len() as f64)
                } else {
                    list_index(&key, values.len())
                        .map_or(Value::Null, |index| values[index].clone())
                }
            }
            Value::String(value) => {
                if key == "length" {
                    Value::Number(value.0.len() as f64)
                } else {
                    list_index(&key, value.0.len()).map_or(Value::Null, |index| {
                        Value::String(Text(vec![value.0[index]]))
                    })
                }
            }
            Value::Host(id) => match key.as_str() {
                "file" => Value::File(id),
                "note" => self.all_properties(&id)?,
                _ => self.note_property(&id, &key)?,
            },
            Value::File(id) => {
                let Some(file) = self.metadata.file(&id) else {
                    return Ok(Value::Null);
                };
                match key.as_str() {
                    "name" => Value::text(&file.name),
                    "basename" => Value::text(&file.basename),
                    "path" => Value::text(&file.id),
                    "folder" => Value::text(&file.folder),
                    "ext" => Value::text(&file.extension),
                    "size" => Value::Number(file.size as f64),
                    "file" => Value::File(id.clone()),
                    "properties" => self.all_properties(&id)?,
                    "ctime" => file
                        .created
                        .map_or(Value::Null, |time| Value::Date(time.into())),
                    "mtime" => file
                        .modified
                        .map_or(Value::Null, |time| Value::Date(time.into())),
                    "tags" => Value::List(file.tags.iter().map(|tag| Value::text(tag)).collect()),
                    "links" | "embeds" => Value::List(
                        if key == "links" {
                            &file.links
                        } else {
                            &file.embeds
                        }
                        .iter()
                        .map(|target| Value::Link(Link::new(target.as_str().into(), &id, None)))
                        .collect(),
                    ),
                    "backlinks" => Value::List(
                        file.backlinks
                            .iter()
                            .map(|id| Value::File(id.clone()))
                            .collect(),
                    ),
                    _ => bail!("unsupported file property {key:?}"),
                }
            }
            Value::Date(date) => Value::Number(match key.as_str() {
                "year" => f64::from(date.year()),
                "month" => f64::from(date.month()),
                "day" => f64::from(date.day()),
                "hour" => f64::from(date.hour()),
                "minute" => f64::from(date.minute()),
                "second" => f64::from(date.second()),
                "millisecond" => f64::from(date.timestamp_subsec_millis()),
                _ => bail!("unsupported date property {key:?}"),
            }),
            value => bail!("{} has no property {key:?}", value.type_name()),
        })
    }
    pub(crate) fn file_id<'b>(&'b self, value: &'b Value) -> Option<&'b str> {
        match value {
            Value::File(id) | Value::Host(id) => {
                self.metadata.file(id).map(|file| file.id.as_str())
            }
            Value::Link(link) => self
                .metadata
                .resolve_link(&link.source, &link.target.utf8().ok()?)
                .map(|file| file.id.as_str()),
            Value::String(value) => self
                .metadata
                .resolve_link(self.row, &value.utf8().ok()?)
                .map(|file| file.id.as_str()),
            _ => None,
        }
    }
    pub(crate) fn equal(&self, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (
                Value::File(_) | Value::Host(_) | Value::Link(_),
                Value::File(_) | Value::Host(_) | Value::Link(_),
            ) => match (self.file_id(left), self.file_id(right)) {
                (Some(left), Some(right)) => left == right,
                (None, None) => {
                    matches!((left,right), (Value::Link(left),Value::Link(right)) if left.target == right.target)
                }
                _ => false,
            },
            (Value::List(left), Value::List(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| self.equal(left, right))
            }
            (Value::Object(left), Value::Object(right)) => {
                left.len() == right.len()
                    && left.iter().all(|(key, left)| {
                        right
                            .iter()
                            .any(|(other, right)| key == other && self.equal(left, right))
                    })
            }
            _ => left == right,
        }
    }
    pub(crate) fn identity(&self, value: &Value) -> Identity {
        match value {
            Value::Null => Identity::Null,
            Value::Boolean(value) => Identity::Boolean(*value),
            Value::Number(value) => Identity::Number(number_identity(*value)),
            Value::String(value) => Identity::String(value.clone()),
            Value::Date(value) => Identity::Date(value.timestamp_millis()),
            Value::Duration(value) => Identity::Duration(
                number_identity(value.months),
                number_identity(value.milliseconds),
            ),
            Value::File(_) | Value::Host(_) => {
                Identity::File(self.file_id(value).unwrap_or("").to_owned())
            }
            Value::Link(link) => self.file_id(value).map_or_else(
                || Identity::BrokenLink(link.target.clone()),
                |id| Identity::File(id.to_owned()),
            ),
            Value::List(values) => {
                Identity::List(values.iter().map(|value| self.identity(value)).collect())
            }
            Value::Object(values) => {
                let mut values: Vec<_> = values
                    .iter()
                    .map(|(key, value)| (key.clone(), self.identity(value)))
                    .collect();
                values.sort_by(|(left, _), (right, _)| left.cmp(right));
                Identity::Object(values)
            }
            Value::Image(text) | Value::Html(text) | Value::Icon(text) => {
                Identity::Decoration(value.type_name(), text.clone())
            }
            Value::Regex { pattern, flags } => Identity::Regex(pattern.clone(), flags.clone()),
        }
    }

    pub(crate) fn compare(&self, left: &Value, right: &Value) -> std::cmp::Ordering {
        match (left, right) {
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,
            (Value::Number(left), Value::Number(right)) => left.total_cmp(right),
            (Value::Date(left), Value::Date(right)) => left.cmp(right),
            (Value::Boolean(left), Value::Boolean(right)) => left.cmp(right),
            (Value::String(left), Value::String(right)) => left.cmp(right),
            _ => left
                .type_name()
                .cmp(right.type_name())
                .then_with(|| left.to_text().cmp(&right.to_text())),
        }
    }
    fn binary(&self, operator: Operator, left: Value, right: Value) -> Result<Value> {
        if matches!(operator, Operator::Equal | Operator::NotEqual) {
            return Ok(Value::Boolean(
                self.equal(&left, &right) == (operator == Operator::Equal),
            ));
        }
        if matches!(
            operator,
            Operator::Less | Operator::LessEqual | Operator::Greater | Operator::GreaterEqual
        ) {
            if matches!(left, Value::Null) || matches!(right, Value::Null) {
                return Ok(Value::Boolean(false));
            }
            if matches!((&left,&right), (Value::Number(left),Value::Number(right)) if left.is_nan() || right.is_nan())
            {
                return Ok(Value::Boolean(false));
            }
            let order = self.compare(&left, &right);
            return Ok(Value::Boolean(match operator {
                Operator::Less => order.is_lt(),
                Operator::LessEqual => !order.is_gt(),
                Operator::Greater => order.is_gt(),
                Operator::GreaterEqual => !order.is_lt(),
                _ => false,
            }));
        }
        if let Value::Date(date) = &left {
            if let (Operator::Subtract, Value::Date(other)) = (operator, &right) {
                return Ok(Value::Number(
                    (date.timestamp_millis() - other.timestamp_millis()) as f64,
                ));
            }
            let duration = match &right {
                Value::Duration(duration) => *duration,
                Value::String(duration) => Duration::parse(&duration.utf8()?)?,
                _ => bail!("date arithmetic requires a duration"),
            };
            let sign = match operator {
                Operator::Add => 1.0,
                Operator::Subtract => -1.0,
                _ => bail!("unsupported date arithmetic"),
            };
            let months = (duration.months * sign).round();
            let milliseconds = duration.milliseconds * sign;
            if !months.is_finite()
                || months.abs() > f64::from(u32::MAX)
                || !milliseconds.is_finite()
                || milliseconds.abs() >= i64::MAX as f64
            {
                bail!("date offset exceeds supported range");
            }
            let date = if months >= 0.0 {
                date.checked_add_months(Months::new(months as u32))
            } else {
                date.checked_sub_months(Months::new((-months) as u32))
            }
            .context("calendar date offset is invalid")?;
            return Ok(Value::Date(
                date.checked_add_signed(
                    TimeDelta::try_milliseconds(milliseconds.round() as i64)
                        .context("date offset exceeds supported range")?,
                )
                .context("date exceeds supported range")?,
            ));
        }
        if let Value::Duration(duration) = &left {
            let duration = match (&right, operator) {
                (Value::Number(scalar), Operator::Multiply | Operator::Divide) => {
                    let scalar = if operator == Operator::Divide {
                        1.0 / scalar
                    } else {
                        *scalar
                    };
                    Duration {
                        months: duration.months * scalar,
                        milliseconds: duration.milliseconds * scalar,
                    }
                }
                (Value::Duration(other), Operator::Add | Operator::Subtract) => {
                    let sign = if operator == Operator::Subtract {
                        -1.0
                    } else {
                        1.0
                    };
                    Duration {
                        months: duration.months + sign * other.months,
                        milliseconds: duration.milliseconds + sign * other.milliseconds,
                    }
                }
                _ => bail!("duration arithmetic requires a right-hand scalar or duration"),
            };
            if !duration.months.is_finite() || !duration.milliseconds.is_finite() {
                bail!("duration exceeds supported range");
            }
            return Ok(Value::Duration(duration));
        }
        if operator == Operator::Add
            && (matches!(left, Value::String(_)) || matches!(right, Value::String(_)))
        {
            let mut left = left.to_text().0;
            let right = right.to_text().0;
            if left.len().saturating_add(right.len()) > MAX_VALUE_SIZE {
                bail!("string result exceeds 1 Mi units");
            }
            left.extend(right);
            return Ok(Value::String(Text(left)));
        }
        if matches!(left, Value::Null) || matches!(right, Value::Null) {
            return Ok(Value::Null);
        }
        let (Value::Number(left), Value::Number(right)) = (left, right) else {
            bail!("arithmetic requires numbers");
        };
        Ok(Value::Number(match operator {
            Operator::Add => left + right,
            Operator::Subtract => left - right,
            Operator::Multiply => left * right,
            Operator::Divide => left / right,
            Operator::Modulo => left % right,
            _ => bail!("unsupported arithmetic operator"),
        }))
    }
    fn call(&mut self, target: &Node, arguments: &[Node]) -> Result<Value> {
        if let Kind::Name(name) = &target.kind {
            if name == "if" {
                arity(arguments.len(), 2, 3)?;
                return if self.node(&arguments[0])?.truthy() {
                    self.node(&arguments[1])
                } else {
                    arguments
                        .get(2)
                        .map_or(Ok(Value::Null), |node| self.node(node))
                };
            }
            let values = arguments
                .iter()
                .map(|node| self.node(node))
                .collect::<Result<Vec<_>>>()?;
            return self.global(name, &values);
        }
        let Kind::Member(receiver, key) = &target.kind else {
            bail!("invalid function target");
        };
        let name = key
            .string()
            .context("computed function calls are unsupported")?;
        let receiver = self.node(receiver)?;
        if matches!(name.as_str(), "map" | "filter" | "reduce") {
            return self.iterate(receiver, &name, arguments);
        }
        let values = arguments
            .iter()
            .map(|node| self.node(node))
            .collect::<Result<Vec<_>>>()?;
        self.method(receiver, &name, &values)
    }
    fn global(&mut self, name: &str, values: &[Value]) -> Result<Value> {
        match name {
            "now" | "today" | "random" => {
                arity(values.len(), 0, 0)?;
                if name == "now" {
                    return Ok(Value::Date(self.environment.now));
                }
                if name == "today" {
                    return Ok(Value::Date(parse_date(
                        &self.environment.now.format("%Y-%m-%d").to_string(),
                    )?));
                }
                let mut hash = DefaultHasher::new();
                (self.environment.random_seed, self.row, self.random_calls).hash(&mut hash);
                self.random_calls += 1;
                Ok(Value::Number(
                    (hash.finish() >> 11) as f64 / ((1u64 << 53) as f64),
                ))
            }
            "min" | "max" => {
                let mut result = if name == "min" {
                    f64::INFINITY
                } else {
                    f64::NEG_INFINITY
                };
                for value in values {
                    let value = want_number(value)?;
                    if value.is_nan() {
                        return Ok(Value::Number(f64::NAN));
                    }
                    result = if name == "min" {
                        result.min(value)
                    } else {
                        result.max(value)
                    };
                }
                Ok(Value::Number(result))
            }
            "link" => {
                arity(values.len(), 1, 2)?;
                let target = self.path(&values[0])?;
                Ok(Value::Link(Link::new(
                    target,
                    self.row,
                    values.get(1).cloned(),
                )))
            }
            _ => {
                arity(values.len(), 1, 1)?;
                let value = &values[0];
                Ok(match name {
                    "number" => Value::Number(value.number()?),
                    "list" => {
                        if matches!(value, Value::List(_)) {
                            value.clone()
                        } else {
                            Value::List(vec![value.clone()])
                        }
                    }
                    "date" => Value::Date(parse_date(&value.as_text()?.utf8()?)?),
                    "duration" => Value::Duration(Duration::parse(&value.as_text()?.utf8()?)?),
                    "file" => self
                        .file_id(value)
                        .map_or(Value::Null, |id| Value::File(id.to_owned())),
                    "image" => Value::Image(self.path(value)?),
                    "icon" => Value::Icon(value.as_text()?.clone()),
                    "html" => Value::Html(value.as_text()?.clone()),
                    "escapeHTML" => {
                        let text = value.as_text()?;
                        let mut result = Vec::new();
                        for unit in &text.0 {
                            match *unit {
                                38 => result.extend("&amp;".encode_utf16()),
                                60 => result.extend("&lt;".encode_utf16()),
                                62 => result.extend("&gt;".encode_utf16()),
                                34 => result.extend("&quot;".encode_utf16()),
                                39 => result.extend("&#39;".encode_utf16()),
                                _ => result.push(*unit),
                            }
                            if result.len() > MAX_VALUE_SIZE {
                                bail!("string result exceeds 1 Mi units");
                            }
                        }
                        Value::String(Text(result))
                    }
                    _ => bail!("unsupported global function {name:?}"),
                })
            }
        }
    }
    fn path(&self, value: &Value) -> Result<Text> {
        match value {
            Value::String(value) => Ok(value.clone()),
            Value::Link(link) => Ok(link.target.clone()),
            Value::File(_) | Value::Host(_) => self
                .file_id(value)
                .map(Text::from)
                .context("file is absent from public metadata"),
            _ => bail!("expected string, link, or public file"),
        }
    }
    fn iterate(&mut self, receiver: Value, name: &str, arguments: &[Node]) -> Result<Value> {
        let Value::List(values) = receiver else {
            bail!("{name} requires a list");
        };
        arity(
            arguments.len(),
            if name == "reduce" { 2 } else { 1 },
            if name == "reduce" { 2 } else { 1 },
        )?;
        let mut accumulator = if name == "reduce" {
            self.node(&arguments[1])?
        } else {
            Value::Null
        };
        let mut output = Vec::new();
        for (index, value) in values.into_iter().enumerate() {
            self.budget.step(self.cancelled)?;
            self.iteration.push(Iteration {
                value: value.clone(),
                index,
                accumulator: accumulator.clone(),
            });
            let result = self.node(&arguments[0]);
            self.iteration.pop();
            let result = result?;
            match name {
                "map" => output.push(result),
                "filter" if result.truthy() => output.push(value),
                "reduce" => accumulator = result,
                _ => {}
            }
        }
        Ok(if name == "reduce" {
            accumulator
        } else {
            Value::List(output)
        })
    }
    fn method(&mut self, receiver: Value, name: &str, values: &[Value]) -> Result<Value> {
        match name {
            "isTruthy" | "toString" | "isEmpty" => {
                arity(values.len(), 0, 0)?;
                return Ok(match name {
                    "isTruthy" => Value::Boolean(receiver.truthy()),
                    "isEmpty" => Value::Boolean(receiver.empty()),
                    _ => Value::String(receiver.to_text()),
                });
            }
            "isType" => {
                arity(values.len(), 1, 1)?;
                return Ok(Value::Boolean(
                    receiver.type_name() == values[0].as_text()?.utf8()?,
                ));
            }
            _ => {}
        }
        match receiver {
            Value::String(text) => self.string_method(text, name, values),
            Value::Number(number) => number_method(number, name, values),
            Value::Date(date) => super::dates::method(date, name, values, self.environment.now),
            Value::List(list) => self.list_method(list, name, values),
            Value::Object(object) => {
                arity(values.len(), 0, 0)?;
                Ok(Value::List(match name {
                    "keys" => object
                        .into_iter()
                        .map(|(key, _)| Value::String(key))
                        .collect(),
                    "values" => object.into_iter().map(|(_, value)| value).collect(),
                    _ => bail!("unsupported object method {name:?}"),
                }))
            }
            Value::File(id) | Value::Host(id) => self.file_method(&id, name, values),
            Value::Link(link) => match name {
                "asFile" => {
                    arity(values.len(), 0, 0)?;
                    Ok(self
                        .file_id(&Value::Link(link))
                        .map_or(Value::Null, |id| Value::File(id.to_owned())))
                }
                "linksTo" => {
                    arity(values.len(), 1, 1)?;
                    let source = self.file_id(&Value::Link(link)).map(str::to_owned);
                    Ok(Value::Boolean(
                        source.is_some_and(|id| self.has_link(&id, &values[0])),
                    ))
                }
                _ => bail!("unsupported link method {name:?}"),
            },
            Value::Regex { pattern, flags } => {
                super::regexp::method(&pattern, &flags, name, values)
            }
            value => bail!("unsupported {} method {name:?}", value.type_name()),
        }
    }
    fn has_link(&self, source: &str, target: &Value) -> bool {
        let Some(target) = self.file_id(target) else {
            return false;
        };
        self.metadata.file(source).is_some_and(|file| {
            file.links.iter().any(|link| {
                self.metadata
                    .resolve_link(source, link)
                    .is_some_and(|file| file.id == target)
            })
        })
    }
    fn file_method(&self, id: &str, name: &str, values: &[Value]) -> Result<Value> {
        let Some(file) = self.metadata.file(id) else {
            return Ok(Value::Null);
        };
        Ok(match name {
            "asLink" => {
                arity(values.len(), 0, 1)?;
                Value::Link(Link::new(id.into(), id, values.first().cloned()))
            }
            "hasLink" => {
                arity(values.len(), 1, 1)?;
                Value::Boolean(self.has_link(id, &values[0]))
            }
            "hasProperty" => {
                arity(values.len(), 1, 1)?;
                Value::Boolean(
                    file.properties()?
                        .contains_key(values[0].as_text()?.utf8()?),
                )
            }
            "hasTag" => {
                let mut found = false;
                for value in values {
                    let tag = value.as_text()?.utf8()?;
                    let tag = tag.trim_start_matches('#');
                    found |= file.tags.iter().any(|existing| {
                        existing == tag
                            || existing
                                .strip_prefix(tag)
                                .is_some_and(|tail| tail.starts_with('/'))
                    });
                }
                Value::Boolean(found)
            }
            "inFolder" => {
                arity(values.len(), 1, 1)?;
                let folder = values[0].as_text()?.utf8()?.replace('\\', "/");
                let folder = folder.trim_matches('/');
                Value::Boolean(
                    folder.is_empty()
                        || file.folder == folder
                        || file
                            .folder
                            .strip_prefix(folder)
                            .is_some_and(|tail| tail.starts_with('/')),
                )
            }
            _ => bail!("unsupported file method {name:?}"),
        })
    }
    fn list_method(&mut self, mut list: Vec<Value>, name: &str, values: &[Value]) -> Result<Value> {
        Ok(match name {
            "contains" | "containsAll" | "containsAny" => {
                if name == "contains" {
                    arity(values.len(), 1, 1)?;
                }
                let contains = |value| list.iter().any(|existing| self.equal(existing, value));
                Value::Boolean(if name == "containsAny" {
                    values.iter().any(contains)
                } else {
                    values.iter().all(contains)
                })
            }
            "reverse" | "sort" | "unique" | "flat" | "mean" => {
                arity(values.len(), 0, 0)?;
                match name {
                    "reverse" => list.reverse(),
                    "sort" => list.sort_by(|left, right| self.compare(left, right)),
                    "unique" => {
                        let mut unique = Vec::new();
                        let mut seen = HashSet::new();
                        for value in list {
                            self.budget.step(self.cancelled)?;
                            if seen.insert(self.identity(&value)) {
                                unique.push(value);
                            }
                        }
                        list = unique;
                    }
                    "flat" => {
                        list = list
                            .into_iter()
                            .flat_map(|value| {
                                if let Value::List(values) = value {
                                    values
                                } else {
                                    vec![value]
                                }
                            })
                            .collect();
                    }
                    "mean" => {
                        let numbers: Vec<_> = list
                            .iter()
                            .filter_map(|value| {
                                if let Value::Number(value) = value {
                                    Some(*value)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        return Ok(if numbers.is_empty() {
                            Value::Null
                        } else {
                            Value::Number(numbers.iter().sum::<f64>() / numbers.len() as f64)
                        });
                    }
                    _ => {}
                }
                Value::List(list)
            }
            "slice" => {
                arity(values.len(), 1, 2)?;
                let start = slice_index(want_number(&values[0])?, list.len());
                let end = values.get(1).map_or(Ok(list.len()), |value| {
                    Ok::<_, anyhow::Error>(slice_index(want_number(value)?, list.len()))
                })?;
                Value::List(list[start..end.max(start)].to_vec())
            }
            "join" => {
                arity(values.len(), 1, 1)?;
                let separator = values[0].as_text()?;
                let mut output = Vec::new();
                for (index, value) in list.iter().enumerate() {
                    if index > 0 {
                        append(&mut output, &separator.0)?;
                    }
                    if !matches!(value, Value::Null) {
                        append(&mut output, &value.to_text().0)?;
                    }
                }
                Value::String(Text(output))
            }
            _ => bail!("unsupported list method {name:?}"),
        })
    }
    fn string_method(&self, mut text: Text, name: &str, values: &[Value]) -> Result<Value> {
        Ok(match name {
            "contains" | "containsAll" | "containsAny" | "startsWith" | "endsWith" => {
                if matches!(name, "contains" | "startsWith" | "endsWith") {
                    arity(values.len(), 1, 1)?;
                }
                let needles = values
                    .iter()
                    .map(Value::as_text)
                    .collect::<Result<Vec<_>>>()?;
                Value::Boolean(match name {
                    "containsAny" => needles.iter().any(|needle| text.contains(needle)),
                    "startsWith" => text.0.starts_with(&needles[0].0),
                    "endsWith" => text.0.ends_with(&needles[0].0),
                    _ => needles.iter().all(|needle| text.contains(needle)),
                })
            }
            "slice" => {
                arity(values.len(), 1, 2)?;
                let start = slice_index(want_number(&values[0])?, text.0.len());
                let end = values.get(1).map_or(Ok(text.0.len()), |value| {
                    Ok::<_, anyhow::Error>(slice_index(want_number(value)?, text.0.len()))
                })?;
                Value::String(Text(text.0[start..end.max(start)].to_vec()))
            }
            "repeat" => {
                arity(values.len(), 1, 1)?;
                let count = integer(want_number(&values[0])?);
                if !count.is_finite() || count < 0.0 {
                    bail!("invalid repetition count");
                }
                if !text.0.is_empty() && count * text.0.len() as f64 > MAX_VALUE_SIZE as f64 {
                    bail!("string result exceeds 1 Mi units");
                }
                Value::String(Text(if text.0.is_empty() {
                    Vec::new()
                } else {
                    text.0.repeat(count as usize)
                }))
            }
            "reverse" => {
                arity(values.len(), 0, 0)?;
                text.0.reverse();
                Value::String(text)
            }
            "lower" | "title" | "trim" => {
                arity(values.len(), 0, 0)?;
                Value::String(transform_text(text, name))
            }
            "replace" => {
                arity(values.len(), 2, 2)?;
                Value::String(super::regexp::replace(
                    &text,
                    &values[0],
                    values[1].as_text()?,
                )?)
            }
            "split" => {
                arity(values.len(), 1, 2)?;
                let limit = values.get(1).map_or(Ok(u32::MAX as usize), |value| {
                    Ok::<_, anyhow::Error>(uint32(want_number(value)?) as usize)
                })?;
                Value::List(super::regexp::split(&text, &values[0], limit)?)
            }
            _ => bail!("unsupported string method {name:?}"),
        })
    }
}

pub(crate) fn arity(actual: usize, minimum: usize, maximum: usize) -> Result<()> {
    if actual < minimum || actual > maximum {
        bail!("expected {minimum}..={maximum} arguments, got {actual}");
    }
    Ok(())
}
pub(crate) fn want_number(value: &Value) -> Result<f64> {
    match value {
        Value::Number(value) => Ok(*value),
        _ => bail!("expected number, got {}", value.type_name()),
    }
}
pub(crate) fn append(output: &mut Vec<u16>, part: &[u16]) -> Result<()> {
    if output.len().saturating_add(part.len()) > MAX_VALUE_SIZE {
        bail!("string result exceeds 1 Mi units");
    }
    output.extend(part);
    Ok(())
}
fn integer(value: f64) -> f64 {
    if value.is_nan() { 0.0 } else { value.trunc() }
}
fn uint32(value: f64) -> u32 {
    if !value.is_finite() {
        0
    } else {
        value.trunc().rem_euclid(4_294_967_296.0) as u32
    }
}

fn list_index(key: &str, len: usize) -> Option<usize> {
    let index: usize = key.parse().ok()?;
    (index < len && index.to_string() == key).then_some(index)
}
fn slice_index(value: f64, len: usize) -> usize {
    let value = if value.is_nan() { 0.0 } else { value.trunc() };
    if value < 0.0 {
        (len as f64 + value).clamp(0.0, len as f64) as usize
    } else {
        value.min(len as f64) as usize
    }
}
fn transform_text(text: Text, name: &str) -> Text {
    if name == "trim" {
        let whitespace = |unit: &u16| char::from_u32(u32::from(*unit)).is_some_and(js_whitespace);
        let start = text
            .0
            .iter()
            .position(|unit| !whitespace(unit))
            .unwrap_or(text.0.len());
        let end = text
            .0
            .iter()
            .rposition(|unit| !whitespace(unit))
            .map_or(start, |index| index + 1);
        return Text(text.0[start..end].to_vec());
    }
    let mut output = Vec::new();
    let mut run = String::new();
    let flush = |run: &mut String, output: &mut Vec<u16>| {
        let result = match name {
            "lower" => run.to_lowercase(),
            "trim" => run.trim_matches(js_whitespace).to_owned(),
            _ => run
                .split_inclusive(|ch: char| !ch.is_alphanumeric())
                .map(|word| {
                    let mut chars = word.chars();
                    let first = chars
                        .next()
                        .map_or_else(String::new, |ch| ch.to_uppercase().collect());
                    first + &chars.as_str().to_lowercase()
                })
                .collect(),
        };
        output.extend(result.encode_utf16());
        run.clear();
    };
    // Preserve isolated surrogate units rather than corrupting formula string values.
    for ch in char::decode_utf16(text.0) {
        match ch {
            Ok(ch) => run.push(ch),
            Err(error) => {
                flush(&mut run, &mut output);
                output.push(error.unpaired_surrogate());
            }
        }
    }
    flush(&mut run, &mut output);
    Text(output)
}
fn number_method(number: f64, name: &str, values: &[Value]) -> Result<Value> {
    Ok(match name {
        "abs" | "ceil" | "floor" => {
            arity(values.len(), 0, 0)?;
            Value::Number(match name {
                "abs" => number.abs(),
                "ceil" => number.ceil(),
                _ => number.floor(),
            })
        }
        "round" => {
            arity(values.len(), 0, 1)?;
            let digits = values.first().map_or(Ok(0.0), want_number)?;
            if !digits.is_finite() || digits.abs() > 100.0 {
                bail!("round digits exceed supported range");
            }
            let factor = 10f64.powi(digits.trunc() as i32);
            let scaled = number * factor;
            let floor = scaled.floor();
            let mut rounded = if scaled - floor >= 0.5 {
                floor + 1.0
            } else {
                floor
            };
            if rounded == 0.0 && scaled.is_sign_negative() {
                rounded = -0.0;
            }
            Value::Number(rounded / factor)
        }
        "toFixed" => {
            arity(values.len(), 1, 1)?;
            let digits = integer(want_number(&values[0])?);
            if !(0.0..=100.0).contains(&digits) {
                bail!("toFixed precision must be 0..=100");
            }
            Value::text(ryu_js::Buffer::new().format_to_fixed(number, digits as u8))
        }
        _ => bail!("unsupported number method {name:?}"),
    })
}
