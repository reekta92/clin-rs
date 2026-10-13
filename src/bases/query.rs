//! Deterministic filtering, formula projection, sorting, grouping, limits, and summaries.

use super::eval::{Budget, Environment, Evaluator, Formulas};
use super::expr::Expression;
use super::format::{BaseDocument, Direction, Filter, PropertySort};
use super::value::Value;
use crate::storage::metadata::VaultMetadata;
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug)]
enum CompiledFilter {
    Expression(Expression),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Vec<Self>),
}
impl CompiledFilter {
    fn compile(filter: &Filter) -> Result<Self> {
        Ok(match filter {
            Filter::Expression(source) => Self::Expression(Expression::parse(source)?),
            Filter::And(filters) => {
                Self::And(filters.iter().map(Self::compile).collect::<Result<_>>()?)
            }
            Filter::Or(filters) => {
                Self::Or(filters.iter().map(Self::compile).collect::<Result<_>>()?)
            }
            Filter::Not(filters) => {
                Self::Not(filters.iter().map(Self::compile).collect::<Result<_>>()?)
            }
        })
    }
    fn matches(&self, evaluator: &mut Evaluator<'_>) -> Result<bool> {
        match self {
            Self::Expression(expression) => Ok(evaluator.expression(expression)?.truthy()),
            Self::And(filters) => {
                for filter in filters {
                    if !filter.matches(evaluator)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Or(filters) => {
                for filter in filters {
                    if filter.matches(evaluator)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Not(filters) => {
                for filter in filters {
                    if filter.matches(evaluator)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub values: BTreeMap<String, Value>,
}
#[derive(Clone, Debug)]
pub struct Group {
    pub value: Value,
    pub rows: Vec<usize>,
    pub summaries: BTreeMap<String, Value>,
}
#[derive(Clone, Debug)]
pub struct QueryResult {
    pub rows: Vec<Row>,
    pub groups: Vec<Group>,
    pub summaries: BTreeMap<String, Value>,
    /// Count after filters, before group visibility and the explicit view limit.
    pub matched: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug)]
enum Summary {
    Builtin(String),
    Custom(Expression),
}
#[derive(Clone, Debug)]
pub struct Query {
    formulas: Formulas,
    global_filter: CompiledFilter,
    view_filter: CompiledFilter,
    pub columns: Vec<String>,
    projected: BTreeSet<String>,
    sort: Vec<PropertySort>,
    group_by: Option<PropertySort>,
    group_order: Option<Vec<serde_yaml_ng::Value>>,
    limit: Option<usize>,
    summaries: BTreeMap<String, Summary>,
    search: String,
}
impl Query {
    pub fn compile(document: &BaseDocument, view_index: usize) -> Result<Self> {
        let view = document
            .config
            .views
            .get(view_index)
            .context("base view index out of bounds")?;
        if !view.layout.supported() {
            bail!(
                "unsupported Bases view {:?}; configuration retained",
                view.layout.name()
            );
        }
        if view.group_order.is_some() && view.group_by.is_none() {
            bail!("groupOrder requires groupBy");
        }
        let formulas = Formulas::compile(&document.config.formulas)?;
        let columns = if view.order.is_empty() {
            vec!["file.name".into()]
        } else {
            view.order.clone()
        };
        let mut projected: BTreeSet<_> = columns.iter().cloned().collect();
        projected.extend(view.sort.iter().map(|sort| sort.property.clone()));
        if let Some(group) = &view.group_by {
            projected.insert(group.property.clone());
        }
        let mut summaries = BTreeMap::new();
        for (property, name) in &view.summaries {
            let summary = if let Some(source) = document.config.summaries.get(name) {
                Summary::Custom(
                    Expression::parse(source).with_context(|| format!("summary {name:?}"))?,
                )
            } else if [
                "Average",
                "Min",
                "Max",
                "Sum",
                "Range",
                "Median",
                "Stddev",
                "Earliest",
                "Latest",
                "Checked",
                "Unchecked",
                "Empty",
                "Filled",
                "Unique",
            ]
            .contains(&name.as_str())
            {
                Summary::Builtin(name.clone())
            } else {
                bail!("unsupported summary {name:?}");
            };
            summaries.insert(property.clone(), summary);
            projected.insert(property.clone());
        }
        Ok(Self {
            formulas,
            global_filter: CompiledFilter::compile(&document.config.filter)?,
            view_filter: CompiledFilter::compile(&view.filter)?,
            columns,
            projected,
            sort: view.sort.clone(),
            group_by: view.group_by.clone(),
            group_order: view.group_order.clone(),
            limit: view.limit,
            summaries,
            search: String::new(),
        })
    }

    pub(crate) fn set_search(&mut self, text: &str) -> Result<()> {
        if text.len() > 64 * 1024 {
            bail!("Bases search exceeds 64 KiB limit");
        }
        self.search = text.to_lowercase();
        Ok(())
    }

    pub fn execute(
        &self,
        metadata: &VaultMetadata,
        environment: &Environment,
        cancelled: &AtomicBool,
    ) -> Result<QueryResult> {
        if cancelled.load(Ordering::Relaxed) {
            bail!("Bases query cancelled");
        }
        let mut budget = Budget::new();
        let mut rows = Vec::new();
        // Metadata is hash-indexed; ordering never depends on hash iteration or worker scheduling.
        let mut files: Vec<_> = metadata.files().collect();
        files.sort_by(|left, right| left.id.cmp(&right.id));
        for file in files {
            if cancelled.load(Ordering::Relaxed) {
                bail!("Bases query cancelled");
            }
            let mut evaluator = Evaluator::new(
                metadata,
                &self.formulas,
                &file.id,
                environment,
                cancelled,
                &mut budget,
            );
            let evaluate = |evaluator: &mut Evaluator<'_>| -> Result<Option<Row>> {
                if !self.global_filter.matches(evaluator)?
                    || !self.view_filter.matches(evaluator)?
                {
                    return Ok(None);
                }
                let values: BTreeMap<String, Value> = self
                    .projected
                    .iter()
                    .map(|property| {
                        Ok((
                            property.clone(),
                            evaluator
                                .property(property)
                                .with_context(|| format!("property {property:?}"))?,
                        ))
                    })
                    .collect::<Result<_>>()?;
                if !self.search.is_empty() {
                    let mut matches = false;
                    for column in &self.columns {
                        if matches!(values[column], Value::Null) {
                            continue;
                        }
                        let text = values[column].to_text();
                        for _ in &text.0 {
                            evaluator.step()?;
                        }
                        if text.display().to_lowercase().contains(&self.search) {
                            matches = true;
                            break;
                        }
                    }
                    if !matches {
                        return Ok(None);
                    }
                }
                Ok(Some(Row {
                    id: file.id.clone(),
                    values,
                }))
            };
            if let Some(row) =
                evaluate(&mut evaluator).with_context(|| format!("Bases row {:?}", file.id))?
            {
                rows.push(row);
            }
        }
        let matched = rows.len();
        let mut evaluator = Evaluator::new(
            metadata,
            &self.formulas,
            "",
            environment,
            cancelled,
            &mut budget,
        );
        let mut sort_error = None;
        rows.sort_by(|left, right| {
            if let Err(error) = evaluator.step() {
                sort_error = Some(error);
                return std::cmp::Ordering::Equal;
            }
            for sort in &self.sort {
                let order =
                    evaluator.compare(&left.values[&sort.property], &right.values[&sort.property]);
                let order = if sort.direction == Direction::Descending {
                    order.reverse()
                } else {
                    order
                };
                if !order.is_eq() {
                    return order;
                }
            }
            left.id.cmp(&right.id)
        });
        if let Some(error) = sort_error {
            return Err(error);
        }
        let date_formula_groups = self.group_by.as_ref().is_some_and(|group| {
            group.property.starts_with("formula.")
                && rows
                    .iter()
                    .any(|row| matches!(row.values[&group.property], Value::Date(_)))
                && rows
                    .iter()
                    .all(|row| matches!(row.values[&group.property], Value::Date(_) | Value::Null))
        });
        let manual = self
            .group_order
            .as_ref()
            .map(|values| {
                values
                    .iter()
                    .map(|value| {
                        let property = &self
                            .group_by
                            .as_ref()
                            .context("manual order requires grouping")?
                            .property;
                        let source = environment.host.as_deref().unwrap_or("");
                        if date_formula_groups && !value.is_null() {
                            Ok(Value::Date(super::value::parse_date(
                                value.as_str().context("date group must be a string")?,
                            )?))
                        } else if property.starts_with("file.") || property.starts_with("formula.")
                        {
                            Value::from_yaml(value, source)
                        } else {
                            evaluator.property_value(
                                source,
                                property.strip_prefix("note.").unwrap_or(property),
                                value,
                            )
                        }
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let group_value = |row: &Row| {
            self.group_by.as_ref().map_or(Value::Null, |group| {
                let value = row.values[&group.property].clone();
                if value.empty() { Value::Null } else { value }
            })
        };
        if let Some(order) = &manual {
            let visible: HashSet<_> = order
                .iter()
                .map(|value| evaluator.identity(value))
                .collect();
            rows.retain(|row| visible.contains(&evaluator.identity(&group_value(row))));
        }
        if let Some(limit) = self.limit {
            rows.truncate(limit);
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut group_indices = HashMap::new();
        if let Some(order) = manual {
            for value in order {
                evaluator.step()?;
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    group_indices.entry(evaluator.identity(&value))
                {
                    entry.insert(groups.len());
                    groups.push(Group {
                        value,
                        rows: Vec::new(),
                        summaries: BTreeMap::new(),
                    });
                }
            }
        }
        for (index, row) in rows.iter().enumerate() {
            evaluator.step()?;
            let value = group_value(row);
            let group_index = *group_indices
                .entry(evaluator.identity(&value))
                .or_insert_with(|| {
                    let index = groups.len();
                    groups.push(Group {
                        value,
                        rows: Vec::new(),
                        summaries: BTreeMap::new(),
                    });
                    index
                });
            groups[group_index].rows.push(index);
        }
        if self.group_order.is_none()
            && let Some(group_by) = &self.group_by
        {
            groups.sort_by(|left, right| {
                let order = evaluator.compare(&left.value, &right.value);
                if group_by.direction == Direction::Descending {
                    order.reverse()
                } else {
                    order
                }
            });
        }
        let summaries = self.summarize(rows.iter(), &mut evaluator)?;
        for group in &mut groups {
            group.summaries =
                self.summarize(group.rows.iter().map(|index| &rows[*index]), &mut evaluator)?;
        }
        let mut diagnostics = Vec::new();
        if rows.iter().any(|row| {
            row.values
                .values()
                .any(|value| matches!(value, Value::Html(_)))
        }) {
            diagnostics.push(
                "html() output is retained as inert text; HTML rendering is unsupported".into(),
            );
        }
        Ok(QueryResult {
            rows,
            groups,
            summaries,
            matched,
            diagnostics,
        })
    }

    fn summarize<'a>(
        &self,
        rows: impl Iterator<Item = &'a Row>,
        evaluator: &mut Evaluator<'_>,
    ) -> Result<BTreeMap<String, Value>> {
        let rows: Vec<_> = rows.collect();
        let mut summaries = BTreeMap::new();
        for (property, summary) in &self.summaries {
            evaluator.step()?;
            let values: Vec<_> = rows
                .iter()
                .map(|row| row.values[property].clone())
                .collect();
            let value = match summary {
                Summary::Builtin(name) => builtin_summary(name, &values, evaluator)?,
                Summary::Custom(expression) => {
                    evaluator.values = Some(values);
                    let result = evaluator.expression(expression);
                    evaluator.values = None;
                    result?
                }
            };
            summaries.insert(property.clone(), value);
        }
        Ok(summaries)
    }
}

fn builtin_summary(name: &str, values: &[Value], evaluator: &mut Evaluator<'_>) -> Result<Value> {
    let count = |predicate: fn(&Value) -> bool| {
        Value::Number(values.iter().filter(|value| predicate(value)).count() as f64)
    };
    match name {
        "Empty" => return Ok(count(Value::empty)),
        "Filled" => return Ok(count(|value| !value.empty())),
        "Checked" => return Ok(count(|value| matches!(value, Value::Boolean(true)))),
        "Unchecked" => return Ok(count(|value| matches!(value, Value::Boolean(false)))),
        "Unique" => {
            let mut unique = HashSet::new();
            for value in values {
                evaluator.step()?;
                unique.insert(evaluator.identity(value));
            }
            return Ok(Value::Number(unique.len() as f64));
        }
        "Earliest" | "Latest" => {
            let dates = values
                .iter()
                .filter(|value| matches!(value, Value::Date(_)));
            return Ok(if name == "Earliest" {
                dates.min_by(|left, right| evaluator.compare(left, right))
            } else {
                dates.max_by(|left, right| evaluator.compare(left, right))
            }
            .cloned()
            .unwrap_or(Value::Null));
        }
        _ => {}
    }
    let mut numbers: Vec<_> = values
        .iter()
        .filter_map(|value| {
            if let Value::Number(number) = value {
                Some(*number)
            } else {
                None
            }
        })
        .collect();
    if name == "Range" && numbers.is_empty() {
        let dates: Vec<_> = values
            .iter()
            .filter_map(|value| {
                if let Value::Date(date) = value {
                    Some(date.timestamp_millis())
                } else {
                    None
                }
            })
            .collect();
        return Ok(match (dates.iter().min(), dates.iter().max()) {
            (Some(min), Some(max)) => Value::Number((max - min) as f64),
            _ => Value::Null,
        });
    }
    if numbers.is_empty() {
        return Ok(if name == "Sum" {
            Value::Number(0.0)
        } else {
            Value::Null
        });
    }
    numbers.sort_by(f64::total_cmp);
    let min = numbers[0];
    let max = numbers[numbers.len() - 1];
    let result = match name {
        "Sum" => numbers.iter().sum(),
        "Average" => numbers.iter().sum::<f64>() / numbers.len() as f64,
        "Min" => min,
        "Max" => max,
        "Range" => max - min,
        "Median" => {
            let middle = numbers.len() / 2;
            if numbers.len() % 2 == 0 {
                numbers[middle - 1] / 2.0 + numbers[middle] / 2.0
            } else {
                numbers[middle]
            }
        }
        "Stddev" => {
            let mut mean = 0.0;
            let mut squared = 0.0;
            for (index, number) in numbers.iter().enumerate() {
                let delta = number - mean;
                mean += delta / (index + 1) as f64;
                squared += delta * (number - mean);
            }
            (squared / numbers.len() as f64).sqrt()
        }
        _ => bail!("unsupported summary {name:?}"),
    };
    Ok(Value::Number(result))
}
