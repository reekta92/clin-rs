use super::expr::{self, EvalCtx, Value};
use super::model::*;
use super::props::FileProps;
use crate::frontmatter::Frontmatter;
use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct BaseRow {
    pub id: String,
    pub file: FileProps,
    pub values: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub key: String,
    pub display: String,
}

#[derive(Debug, Clone)]
pub struct GroupRows {
    pub label: Option<String>,
    pub rows: Vec<BaseRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SummaryValue {
    Num(f64),
    Str(String),
    None,
}

#[derive(Debug, Clone)]
pub struct EvalResult {
    pub groups: Vec<GroupRows>,
    pub columns: Vec<ColumnDef>,
    pub summaries: BTreeMap<String, SummaryValue>,
}

#[allow(clippy::collapsible_if, clippy::manual_is_multiple_of)]
pub fn evaluate(
    base: &BaseFile,
    view: &BaseView,
    files: Vec<(String, FileProps, Frontmatter)>,
) -> Result<EvalResult> {
    // 1. Parse all formulas
    let mut formulas = HashMap::new();
    for (name, expr_str) in &base.formulas {
        let parsed =
            expr::parse(expr_str).with_context(|| format!("Failed to parse formula '{}'", name))?;
        formulas.insert(name.clone(), parsed);
    }

    // 2. Parser cache for filters/column keys
    let mut parse_cache = HashMap::new();

    // 3. Evaluate filters and select matching files
    let mut matched_rows = Vec::new();
    let mut seen_note_keys = HashSet::new();

    for (id, file, fm) in files {
        let note_get = |key: &str| match key {
            "title" => fm.title.clone().map(Value::Str),
            "updated_at" => fm.updated_at.map(|u| Value::Num(u as f64)),
            "tags" => Some(Value::List(
                fm.tags.iter().map(|s| Value::Str(s.clone())).collect(),
            )),
            "pinned" => Some(Value::Bool(fm.pinned)),
            "links" => fm
                .links
                .clone()
                .map(|l| Value::List(l.into_iter().map(Value::Str).collect())),
            "original_ext" => fm.original_ext.clone().map(Value::Str),
            _ => fm.get(key).map(yaml_to_value),
        };

        // Track seen keys for default columns
        for k in fm.extra.keys() {
            seen_note_keys.insert(k.clone());
        }

        let mut resolving = Vec::new();
        let mut eval_ctx = EvalCtx {
            note_get: &note_get,
            file: &file,
            formulas: &formulas,
            this_file: None,
            this_val: None,
            resolving: &mut resolving,
        };

        // Global filters
        let mut pass = true;
        if let Some(global_filters) = &base.filters {
            pass = expr::eval_filter(global_filters, &mut eval_ctx, &mut parse_cache)?;
        }

        // View filters
        if pass {
            if let Some(view_filters) = &view.filters {
                pass = expr::eval_filter(view_filters, &mut eval_ctx, &mut parse_cache)?;
            }
        }
        if pass {
            matched_rows.push((id, file, fm));
        }
    }

    // 4. Determine columns
    let mut col_keys = Vec::new();
    if !view.order.is_empty() {
        col_keys = view.order.clone();
    } else {
        col_keys.push("file.name".to_string());
        // formulas
        for k in base.formulas.keys() {
            col_keys.push(format!("formula.{}", k));
        }
        // note properties
        let mut sorted_seen: Vec<String> = seen_note_keys.into_iter().collect();
        sorted_seen.sort();
        col_keys.extend(sorted_seen);
    }

    // Build ColumnDef list
    let mut columns = Vec::new();
    for key in &col_keys {
        let display = if let Some(prop_display) = base.properties.get(key) {
            prop_display
                .display_name
                .clone()
                .unwrap_or_else(|| key.clone())
        } else {
            key.clone()
        };
        columns.push(ColumnDef {
            key: key.clone(),
            display,
        });
    }

    // Parse column expressions
    let mut col_exprs = Vec::new();
    for key in &col_keys {
        let parsed = expr::parse(key)
            .with_context(|| format!("Failed to parse column key '{}' as expression", key))?;
        col_exprs.push((key.clone(), parsed));
    }

    // Grouping property expression
    let group_by_expr = if let Some(gb) = &view.group_by {
        let parsed = expr::parse(&gb.property)
            .with_context(|| format!("Failed to parse group property '{}'", gb.property))?;
        Some((gb.property.clone(), parsed))
    } else {
        None
    };

    // Layout-extra properties materialized beyond `order` columns so layout renders can read them.
    let view_extra_keys: Vec<&'static str> = {
        let mut keys: Vec<&'static str> = Vec::new();
        match view.r#type {
            ViewType::Map => keys.extend([
                "coordinates",
                "coords",
                "location",
                "marker_color",
                "color",
                "marker_icon",
                "icon",
            ]),
            ViewType::Cards => keys.extend(["color", "cover"]),
            _ => {}
        }
        keys.into_iter()
            .filter(|c| !col_keys.iter().any(|k| k.as_str() == *c))
            .collect()
    };
    let view_extra_exprs: Vec<(&'static str, expr::Expr)> = view_extra_keys
        .iter()
        .filter_map(|c| expr::parse(c).ok().map(|e| (*c, e)))
        .collect();

    // 5. Evaluate all row values
    let mut rows = Vec::new();
    for (id, file, fm) in matched_rows {
        let note_get = |key: &str| match key {
            "title" => fm.title.clone().map(Value::Str),
            "updated_at" => fm.updated_at.map(|u| Value::Num(u as f64)),
            "tags" => Some(Value::List(
                fm.tags.iter().map(|s| Value::Str(s.clone())).collect(),
            )),
            "pinned" => Some(Value::Bool(fm.pinned)),
            "links" => fm
                .links
                .clone()
                .map(|l| Value::List(l.into_iter().map(Value::Str).collect())),
            "original_ext" => fm.original_ext.clone().map(Value::Str),
            _ => fm.get(key).map(yaml_to_value),
        };

        let mut resolving = Vec::new();
        let mut eval_ctx = EvalCtx {
            note_get: &note_get,
            file: &file,
            formulas: &formulas,
            this_file: None,
            this_val: None,
            resolving: &mut resolving,
        };

        let mut values = BTreeMap::new();
        // Evaluate columns
        for (key, expr) in &col_exprs {
            let val = expr::eval(expr, &mut eval_ctx).unwrap_or(Value::Null);
            values.insert(key.clone(), val);
        }

        // Evaluate group by property if not already in columns
        if let Some((ref key, ref expr)) = group_by_expr {
            if !values.contains_key(key) {
                let val = expr::eval(expr, &mut eval_ctx).unwrap_or(Value::Null);
                values.insert(key.clone(), val);
            }
        }

        // Layout views: materialize extra properties not in columns
        for (key, e) in &view_extra_exprs {
            if !values.contains_key(*key) {
                let val = expr::eval(e, &mut eval_ctx).unwrap_or(Value::Null);
                values.insert(key.to_string(), val);
            }
        }

        rows.push(BaseRow { id, file, values });
    }

    // 6. Sort rows
    if let Some(gb) = &view.group_by {
        rows.sort_by(|a, b| {
            let val_a = a.values.get(&gb.property).unwrap_or(&Value::Null);
            let val_b = b.values.get(&gb.property).unwrap_or(&Value::Null);
            let mut ord = val_a.partial_cmp_loose(val_b).unwrap_or(Ordering::Equal);
            if gb.direction == SortDirection::Desc {
                ord = ord.reverse();
            }
            if ord == Ordering::Equal {
                a.file.path.cmp(&b.file.path)
            } else {
                ord
            }
        });
    } else {
        rows.sort_by(|a, b| a.file.path.cmp(&b.file.path));
    }

    // 7. Partition into groups
    let mut groups = Vec::new();
    if let Some(gb) = &view.group_by {
        let mut current_label: Option<Value> = None;
        let mut current_rows = Vec::new();

        for row in rows {
            let val = row.values.get(&gb.property).cloned().unwrap_or(Value::Null);
            if current_label.is_none() || current_label.as_ref() != Some(&val) {
                if !current_rows.is_empty() {
                    let label_str = match &current_label {
                        Some(Value::Null) => "null".to_string(),
                        Some(v) => v.to_string(),
                        None => "null".to_string(),
                    };
                    groups.push(GroupRows {
                        label: Some(label_str),
                        rows: current_rows,
                    });
                    current_rows = Vec::new();
                }
                current_label = Some(val);
            }
            current_rows.push(row);
        }

        if !current_rows.is_empty() {
            let label_str = match &current_label {
                Some(Value::Null) => "null".to_string(),
                Some(v) => v.to_string(),
                None => "null".to_string(),
            };
            groups.push(GroupRows {
                label: Some(label_str),
                rows: current_rows,
            });
        }
    } else {
        groups.push(GroupRows { label: None, rows });
    }

    // 8. Apply Limit
    if let Some(limit) = view.limit {
        let mut count = 0;
        let mut limited_groups = Vec::new();
        for mut g in groups {
            if count >= limit {
                break;
            }
            let remaining = limit - count;
            if g.rows.len() > remaining {
                g.rows.truncate(remaining);
            }
            count += g.rows.len();
            limited_groups.push(g);
        }
        groups = limited_groups;
    }

    // 9. Compute Summaries
    let mut summaries = BTreeMap::new();
    if let Some(view_summaries) = &view.summaries {
        for (col_key, summary_name) in view_summaries {
            // Collect non-null column values
            let col_values: Vec<Value> = groups
                .iter()
                .flat_map(|g| {
                    g.rows
                        .iter()
                        .map(|r| r.values.get(col_key).cloned().unwrap_or(Value::Null))
                })
                .collect();

            let summary_val = compute_summary(summary_name, &col_values, base)?;
            summaries.insert(col_key.clone(), summary_val);
        }
    }

    Ok(EvalResult {
        groups,
        columns,
        summaries,
    })
}

#[allow(clippy::manual_is_multiple_of)]
fn compute_summary(name: &str, values: &[Value], base: &BaseFile) -> Result<SummaryValue> {
    let lower_name = name.to_lowercase();
    match lower_name.as_str() {
        "average" => {
            let nums: Vec<f64> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Num(n) => Some(*n),
                    _ => None,
                })
                .collect();
            if nums.is_empty() {
                Ok(SummaryValue::None)
            } else {
                let sum: f64 = nums.iter().sum();
                Ok(SummaryValue::Num(sum / nums.len() as f64))
            }
        }
        "sum" => {
            let nums: Vec<f64> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Num(n) => Some(*n),
                    _ => None,
                })
                .collect();
            let sum: f64 = nums.iter().sum();
            Ok(SummaryValue::Num(sum))
        }
        "min" => {
            let valid: Vec<&Value> = values.iter().filter(|v| **v != Value::Null).collect();
            if valid.is_empty() {
                Ok(SummaryValue::None)
            } else {
                let mut current_min = valid[0];
                for item in &valid[1..] {
                    if let Some(Ordering::Greater) = current_min.partial_cmp_loose(item) {
                        current_min = item;
                    }
                }
                match current_min {
                    Value::Num(n) => Ok(SummaryValue::Num(*n)),
                    _ => Ok(SummaryValue::Str(current_min.to_string())),
                }
            }
        }
        "max" => {
            let valid: Vec<&Value> = values.iter().filter(|v| **v != Value::Null).collect();
            if valid.is_empty() {
                Ok(SummaryValue::None)
            } else {
                let mut current_max = valid[0];
                for item in &valid[1..] {
                    if let Some(Ordering::Less) = current_max.partial_cmp_loose(item) {
                        current_max = item;
                    }
                }
                match current_max {
                    Value::Num(n) => Ok(SummaryValue::Num(*n)),
                    _ => Ok(SummaryValue::Str(current_max.to_string())),
                }
            }
        }
        "range" => {
            let nums: Vec<f64> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Num(n) => Some(*n),
                    _ => None,
                })
                .collect();
            if nums.is_empty() {
                Ok(SummaryValue::None)
            } else {
                let min = nums.iter().copied().fold(f64::NAN, f64::min);
                let max = nums.iter().copied().fold(f64::NAN, f64::max);
                Ok(SummaryValue::Num(max - min))
            }
        }
        "median" => {
            let mut nums: Vec<f64> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Num(n) => Some(*n),
                    _ => None,
                })
                .collect();
            if nums.is_empty() {
                Ok(SummaryValue::None)
            } else {
                nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let mid = nums.len() / 2;
                if nums.len() % 2 == 0 {
                    Ok(SummaryValue::Num((nums[mid - 1] + nums[mid]) / 2.0))
                } else {
                    Ok(SummaryValue::Num(nums[mid]))
                }
            }
        }
        "stddev" => {
            let nums: Vec<f64> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Num(n) => Some(*n),
                    _ => None,
                })
                .collect();
            if nums.len() < 2 {
                Ok(SummaryValue::None)
            } else {
                let mean = nums.iter().sum::<f64>() / nums.len() as f64;
                let variance =
                    nums.iter().map(|n| (n - mean).powi(2)).sum::<f64>() / (nums.len() - 1) as f64;
                Ok(SummaryValue::Num(variance.sqrt()))
            }
        }
        "earliest" => {
            let dates: Vec<i64> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Date(ms) => Some(*ms),
                    _ => None,
                })
                .collect();
            if dates.is_empty() {
                Ok(SummaryValue::None)
            } else {
                let min = dates.iter().min().expect("non-empty dates");
                Ok(SummaryValue::Str(Value::Date(*min).to_string()))
            }
        }
        "latest" => {
            let dates: Vec<i64> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Date(ms) => Some(*ms),
                    _ => None,
                })
                .collect();
            if dates.is_empty() {
                Ok(SummaryValue::None)
            } else {
                let max = dates.iter().max().expect("non-empty dates");
                Ok(SummaryValue::Str(Value::Date(*max).to_string()))
            }
        }
        "checked" => {
            let count = values
                .iter()
                .filter(|v| matches!(v, Value::Bool(true)))
                .count();
            Ok(SummaryValue::Num(count as f64))
        }
        "unchecked" => {
            let count = values
                .iter()
                .filter(|v| matches!(v, Value::Bool(false)))
                .count();
            Ok(SummaryValue::Num(count as f64))
        }
        "empty" => {
            let count = values
                .iter()
                .filter(|v| **v == Value::Null || matches!(v, Value::Str(s) if s.is_empty()))
                .count();
            Ok(SummaryValue::Num(count as f64))
        }
        "filled" => {
            let count = values
                .iter()
                .filter(|v| **v != Value::Null && !matches!(v, Value::Str(s) if s.is_empty()))
                .count();
            Ok(SummaryValue::Num(count as f64))
        }
        "unique" => {
            let mut unique = Vec::new();
            for val in values {
                if !unique.contains(val) {
                    unique.push(val.clone());
                }
            }
            Ok(SummaryValue::Num(unique.len() as f64))
        }
        _ => {
            // Check custom summary
            if let Some(expr_str) = base.summaries.get(name) {
                let parsed = expr::parse(expr_str)
                    .with_context(|| format!("Failed to parse custom summary '{}'", name))?;
                let mut resolving = Vec::new();
                let dummy_file = FileProps::default();
                let formulas = HashMap::new();
                let summary_note_get = |key: &str| {
                    if key == "values" {
                        Some(Value::List(values.to_vec()))
                    } else {
                        None
                    }
                };
                let mut eval_ctx = EvalCtx {
                    note_get: &summary_note_get,
                    file: &dummy_file,
                    formulas: &formulas,
                    this_file: None,
                    this_val: None,
                    resolving: &mut resolving,
                };
                let res_val = expr::eval(&parsed, &mut eval_ctx)?;
                match res_val {
                    Value::Num(n) => Ok(SummaryValue::Num(n)),
                    Value::Null => Ok(SummaryValue::None),
                    v => Ok(SummaryValue::Str(v.to_string())),
                }
            } else {
                Ok(SummaryValue::None)
            }
        }
    }
}

fn yaml_to_value(v: &serde_yaml_ng::Value) -> Value {
    match v {
        serde_yaml_ng::Value::Null => Value::Null,
        serde_yaml_ng::Value::Bool(b) => Value::Bool(*b),
        serde_yaml_ng::Value::Number(num) => {
            if let Some(f) = num.as_f64() {
                Value::Num(f)
            } else if let Some(i) = num.as_i64() {
                Value::Num(i as f64)
            } else if let Some(u) = num.as_u64() {
                Value::Num(u as f64)
            } else {
                Value::Null
            }
        }
        serde_yaml_ng::Value::String(s) => {
            if let Some(ms) = parse_date_str(s) {
                Value::Date(ms)
            } else {
                Value::Str(s.clone())
            }
        }
        serde_yaml_ng::Value::Sequence(seq) => Value::List(seq.iter().map(yaml_to_value).collect()),
        serde_yaml_ng::Value::Mapping(map) => {
            let mut obj = BTreeMap::new();
            for (k, v) in map {
                if let Some(k_str) = k.as_str() {
                    obj.insert(k_str.to_string(), yaml_to_value(v));
                }
            }
            Value::Object(obj)
        }
        _ => Value::Null,
    }
}

fn parse_date_str(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(ndt.and_utc().timestamp_millis());
    }
    if let Ok(nd) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(
            nd.and_hms_opt(0, 0, 0)
                .expect("valid time")
                .and_utc()
                .timestamp_millis(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_formula_group_summary() {
        let mut extra1 = BTreeMap::new();
        extra1.insert(
            "status".to_string(),
            serde_yaml_ng::Value::String("done".to_string()),
        );
        extra1.insert(
            "price".to_string(),
            serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(10)),
        );
        extra1.insert(
            "qty".to_string(),
            serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(2)),
        );

        let fm1 = Frontmatter {
            title: Some("Note A".to_string()),
            extra: extra1,
            ..Default::default()
        };

        let mut extra2 = BTreeMap::new();
        extra2.insert(
            "status".to_string(),
            serde_yaml_ng::Value::String("done".to_string()),
        );
        extra2.insert(
            "price".to_string(),
            serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(15)),
        );
        extra2.insert(
            "qty".to_string(),
            serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(3)),
        );

        let fm2 = Frontmatter {
            title: Some("Note B".to_string()),
            extra: extra2,
            ..Default::default()
        };

        let mut extra3 = BTreeMap::new();
        extra3.insert(
            "status".to_string(),
            serde_yaml_ng::Value::String("todo".to_string()),
        );
        extra3.insert(
            "price".to_string(),
            serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(5)),
        );
        extra3.insert(
            "qty".to_string(),
            serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(1)),
        );

        let fm3 = Frontmatter {
            title: Some("Note C".to_string()),
            extra: extra3,
            ..Default::default()
        };

        let file1 = FileProps {
            path: "a.md".to_string(),
            name: "a.md".to_string(),
            ..Default::default()
        };
        let file2 = FileProps {
            path: "b.md".to_string(),
            name: "b.md".to_string(),
            ..Default::default()
        };
        let file3 = FileProps {
            path: "c.md".to_string(),
            name: "c.md".to_string(),
            ..Default::default()
        };

        let files = vec![
            ("a.md".to_string(), file1, fm1),
            ("b.md".to_string(), file2, fm2),
            ("c.md".to_string(), file3, fm3),
        ];

        let mut formulas = BTreeMap::new();
        formulas.insert("total".to_string(), "price * qty".to_string());
        let base = BaseFile {
            formulas,
            filters: Some(FilterNode::Expr("status == \"done\"".to_string())),
            ..Default::default()
        };

        let mut summaries = BTreeMap::new();
        summaries.insert("price".to_string(), "Sum".to_string());
        let view = BaseView {
            group_by: Some(GroupBy {
                property: "status".to_string(),
                direction: SortDirection::Asc,
            }),
            order: vec![
                "file.name".to_string(),
                "status".to_string(),
                "price".to_string(),
                "formula.total".to_string(),
            ],
            summaries: Some(summaries),
            ..Default::default()
        };

        let result = evaluate(&base, &view, files).expect("failed to evaluate pipeline");

        // Note C should be filtered out because status != "done"
        assert_eq!(result.groups.len(), 1);
        let group = &result.groups[0];
        assert_eq!(group.label.as_deref(), Some("done"));
        assert_eq!(group.rows.len(), 2);

        // Rows sorted stable by file.path (a.md, b.md)
        let row1 = &group.rows[0];
        assert_eq!(row1.id, "a.md");
        assert_eq!(row1.values.get("formula.total"), Some(&Value::Num(20.0)));

        let row2 = &group.rows[1];
        assert_eq!(row2.id, "b.md");
        assert_eq!(row2.values.get("formula.total"), Some(&Value::Num(45.0)));

        // Price summary should be 10 + 15 = 25
        assert_eq!(
            result.summaries.get("price"),
            Some(&SummaryValue::Num(25.0))
        );
    }

    #[test]
    fn map_view_materializes_coordinates() {
        let mut extra = BTreeMap::new();
        extra.insert(
            "coordinates".to_string(),
            serde_yaml_ng::Value::String("48.86, 2.35".to_string()),
        );
        let fm = Frontmatter {
            title: Some("Paris Note".to_string()),
            extra,
            ..Default::default()
        };
        let file = FileProps {
            path: "paris.md".to_string(),
            name: "paris.md".to_string(),
            ..Default::default()
        };
        let files = vec![("paris.md".to_string(), file, fm)];

        let view = BaseView {
            r#type: ViewType::Map,
            order: vec!["file.name".to_string()],
            ..Default::default()
        };
        let base = BaseFile::default();

        let result = evaluate(&base, &view, files).expect("failed to evaluate pipeline");
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].rows.len(), 1);
        let row = &result.groups[0].rows[0];
        assert_eq!(
            row.values.get("coordinates"),
            Some(&Value::Str("48.86, 2.35".to_string()))
        );
    }

    #[test]
    fn cards_view_materializes_color() {
        let mut extra = BTreeMap::new();
        extra.insert(
            "color".to_string(),
            serde_yaml_ng::Value::String("#ff0000".to_string()),
        );
        let fm = Frontmatter {
            title: Some("Red Note".to_string()),
            extra,
            ..Default::default()
        };
        let file = FileProps {
            path: "red.md".to_string(),
            name: "red.md".to_string(),
            ..Default::default()
        };
        let files = vec![("red.md".to_string(), file, fm)];

        let view = BaseView {
            r#type: ViewType::Cards,
            order: vec!["file.name".to_string()],
            ..Default::default()
        };
        let base = BaseFile::default();

        let result = evaluate(&base, &view, files).expect("failed to evaluate pipeline");
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].rows.len(), 1);
        let row = &result.groups[0].rows[0];
        assert_eq!(
            row.values.get("color"),
            Some(&Value::Str("#ff0000".to_string()))
        );
    }
}
