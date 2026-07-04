use super::parse::{BinOp, Expr};
use super::value::Value;
use crate::base::props::FileProps;
use anyhow::{Result, bail};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

pub struct EvalCtx<'a> {
    pub note_get: &'a dyn Fn(&str) -> Option<Value>,
    pub file: &'a FileProps,
    pub formulas: &'a HashMap<String, Expr>,
    pub this_file: Option<&'a FileProps>,
    pub this_val: Option<&'a Value>,
    pub resolving: &'a mut Vec<String>,
}

pub fn eval(expr: &Expr, ctx: &mut EvalCtx<'_>) -> Result<Value> {
    match expr {
        Expr::Lit(val) => Ok(val.clone()),
        Expr::Path(parts) => eval_path(parts, ctx),
        Expr::Neg(inner) => {
            let val = eval(inner, ctx)?;
            match val {
                Value::Num(n) => Ok(Value::Num(-n)),
                _ => Ok(Value::Null),
            }
        }
        Expr::Not(inner) => {
            let val = eval(inner, ctx)?;
            Ok(Value::Bool(!val.is_truthy()))
        }
        Expr::Binary(op, lhs, rhs) => {
            // Short-circuiting for logical operators
            if op == &BinOp::And {
                let l = eval(lhs, ctx)?;
                if !l.is_truthy() {
                    return Ok(l);
                }
                return eval(rhs, ctx);
            }
            if op == &BinOp::Or {
                let l = eval(lhs, ctx)?;
                if l.is_truthy() {
                    return Ok(l);
                }
                return eval(rhs, ctx);
            }

            let l = eval(lhs, ctx)?;
            let r = eval(rhs, ctx)?;

            match op {
                BinOp::Add => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a + b)),
                    (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),
                    (Value::Date(ms), Value::Str(dur)) => Ok(add_duration(*ms, dur, 1.0)),
                    (Value::Str(dur), Value::Date(ms)) => Ok(add_duration(*ms, dur, 1.0)),
                    (Value::Str(a), Value::Num(b)) => Ok(Value::Str(format!("{}{}", a, b))),
                    (Value::Num(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),
                    _ => Ok(Value::Null),
                },
                BinOp::Sub => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a - b)),
                    (Value::Date(ms), Value::Str(dur)) => Ok(add_duration(*ms, dur, -1.0)),
                    (Value::Date(ms1), Value::Date(ms2)) => Ok(Value::Num((ms1 - ms2) as f64)),
                    _ => Ok(Value::Null),
                },
                BinOp::Mul => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a * b)),
                    _ => Ok(Value::Null),
                },
                BinOp::Div => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => {
                        if *b == 0.0 {
                            Ok(Value::Null)
                        } else {
                            Ok(Value::Num(a / b))
                        }
                    }
                    _ => Ok(Value::Null),
                },
                BinOp::Mod => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a % b)),
                    _ => Ok(Value::Null),
                },
                BinOp::Eq => Ok(Value::Bool(
                    l.partial_cmp_loose(&r) == Some(Ordering::Equal),
                )),
                BinOp::Ne => Ok(Value::Bool(
                    l.partial_cmp_loose(&r) != Some(Ordering::Equal),
                )),
                BinOp::Gt => Ok(Value::Bool(
                    l.partial_cmp_loose(&r) == Some(Ordering::Greater),
                )),
                BinOp::Lt => Ok(Value::Bool(l.partial_cmp_loose(&r) == Some(Ordering::Less))),
                BinOp::Ge => Ok(Value::Bool(matches!(
                    l.partial_cmp_loose(&r),
                    Some(Ordering::Greater | Ordering::Equal)
                ))),
                BinOp::Le => Ok(Value::Bool(matches!(
                    l.partial_cmp_loose(&r),
                    Some(Ordering::Less | Ordering::Equal)
                ))),
                _ => unreachable!(),
            }
        }
        Expr::Call(name, args) => {
            if name == "if" {
                if args.len() != 3 {
                    bail!("if() expects 3 arguments");
                }
                let cond = eval(&args[0], ctx)?;
                if cond.is_truthy() {
                    eval(&args[1], ctx)
                } else {
                    eval(&args[2], ctx)
                }
            } else if name == "filter" {
                if args.len() != 2 {
                    bail!("filter() expects 2 arguments");
                }
                let list_val = eval(&args[0], ctx)?;
                match &list_val {
                    Value::List(items) => {
                        let mut kept = Vec::new();
                        for item in items {
                            let mut sub_ctx = EvalCtx {
                                note_get: ctx.note_get,
                                file: ctx.file,
                                formulas: ctx.formulas,
                                this_file: ctx.this_file,
                                this_val: Some(item),
                                resolving: ctx.resolving,
                            };
                            if eval(&args[1], &mut sub_ctx)?.is_truthy() {
                                kept.push(item.clone());
                            }
                        }
                        Ok(Value::List(kept))
                    }
                    _ => Ok(Value::Null),
                }
            } else if name == "map" {
                if args.len() != 2 {
                    bail!("map() expects 2 arguments");
                }
                let list_val = eval(&args[0], ctx)?;
                match &list_val {
                    Value::List(items) => {
                        let mut mapped = Vec::new();
                        for item in items {
                            let mut sub_ctx = EvalCtx {
                                note_get: ctx.note_get,
                                file: ctx.file,
                                formulas: ctx.formulas,
                                this_file: ctx.this_file,
                                this_val: Some(item),
                                resolving: ctx.resolving,
                            };
                            mapped.push(eval(&args[1], &mut sub_ctx)?);
                        }
                        Ok(Value::List(mapped))
                    }
                    _ => Ok(Value::Null),
                }
            } else {
                let mut evaled = Vec::new();
                for arg in args {
                    evaled.push(eval(arg, ctx)?);
                }
                super::funcs::dispatch_call(name, &evaled)
            }
        }
        Expr::Method(receiver, name, args) => {
            if name == "filter" {
                if args.len() != 1 {
                    bail!("filter() method expects 1 argument");
                }
                let list_val = eval(receiver, ctx)?;
                match &list_val {
                    Value::List(items) => {
                        let mut kept = Vec::new();
                        for item in items {
                            let mut sub_ctx = EvalCtx {
                                note_get: ctx.note_get,
                                file: ctx.file,
                                formulas: ctx.formulas,
                                this_file: ctx.this_file,
                                this_val: Some(item),
                                resolving: ctx.resolving,
                            };
                            if eval(&args[0], &mut sub_ctx)?.is_truthy() {
                                kept.push(item.clone());
                            }
                        }
                        Ok(Value::List(kept))
                    }
                    _ => Ok(Value::Null),
                }
            } else if name == "map" {
                if args.len() != 1 {
                    bail!("map() method expects 1 argument");
                }
                let list_val = eval(receiver, ctx)?;
                match &list_val {
                    Value::List(items) => {
                        let mut mapped = Vec::new();
                        for item in items {
                            let mut sub_ctx = EvalCtx {
                                note_get: ctx.note_get,
                                file: ctx.file,
                                formulas: ctx.formulas,
                                this_file: ctx.this_file,
                                this_val: Some(item),
                                resolving: ctx.resolving,
                            };
                            mapped.push(eval(&args[0], &mut sub_ctx)?);
                        }
                        Ok(Value::List(mapped))
                    }
                    _ => Ok(Value::Null),
                }
            } else {
                let rec_val = eval(receiver, ctx)?;
                let mut evaled = Vec::new();
                for arg in args {
                    evaled.push(eval(arg, ctx)?);
                }
                super::funcs::dispatch_method(name, &rec_val, &evaled)
            }
        }
    }
}

fn eval_path(parts: &[String], ctx: &mut EvalCtx<'_>) -> Result<Value> {
    if parts.is_empty() {
        return Ok(Value::Null);
    }
    let first = &parts[0];

    // If it's "this", check if we have a current list-element context.
    if first == "this" {
        if parts.len() == 1 {
            if let Some(tv) = ctx.this_val {
                return Ok((*tv).clone());
            } else {
                return Ok(Value::Null);
            }
        }

        if parts.get(1).map(|s| s.as_str()) == Some("file") {
            if parts.len() < 3 {
                return Ok(Value::Null);
            }
            if let Some(this_file) = ctx.this_file {
                let field = &parts[2];
                let file_val = get_file_prop(this_file, field);
                if parts.len() > 3 {
                    return Ok(resolve_sub_parts(file_val, &parts[3..]));
                } else {
                    return Ok(file_val);
                }
            } else {
                return Ok(Value::Null);
            }
        }

        // if parts.len() > 1 and parts[1] is not "file", resolve against this_val
        if let Some(tv) = ctx.this_val {
            return Ok(resolve_sub_parts((*tv).clone(), &parts[1..]));
        } else {
            return Ok(Value::Null);
        }
    }

    let val = if first == "file" {
        if parts.len() < 2 {
            return Ok(Value::Null);
        }
        let field = &parts[1];
        let file_val = get_file_prop(ctx.file, field);
        if parts.len() > 2 {
            resolve_sub_parts(file_val, &parts[2..])
        } else {
            file_val
        }
    } else if first == "formula" {
        if parts.len() < 2 {
            return Ok(Value::Null);
        }
        let name = &parts[1];
        let formula_val = if ctx.resolving.contains(name) {
            bail!("circular formula reference: {}", name);
        } else {
            ctx.resolving.push(name.clone());
            let res = if let Some(expr) = ctx.formulas.get(name) {
                eval(expr, ctx)?
            } else {
                Value::Null
            };
            ctx.resolving.pop();
            res
        };
        if parts.len() > 2 {
            resolve_sub_parts(formula_val, &parts[2..])
        } else {
            formula_val
        }
    } else if first == "note" {
        if parts.len() < 2 {
            return Ok(Value::Null);
        }
        let field = &parts[1];
        let note_val = (ctx.note_get)(field).unwrap_or(Value::Null);
        if parts.len() > 2 {
            resolve_sub_parts(note_val, &parts[2..])
        } else {
            note_val
        }
    } else {
        // Bare property: first try resolving from frontmatter (note.*), then default to Null
        let bare_val = (ctx.note_get)(first).unwrap_or(Value::Null);
        if parts.len() > 1 {
            resolve_sub_parts(bare_val, &parts[1..])
        } else {
            bare_val
        }
    };

    Ok(val)
}

fn get_file_prop(file: &FileProps, field: &str) -> Value {
    match field {
        "name" => Value::Str(file.name.clone()),
        "basename" => Value::Str(file.basename.clone()),
        "path" => Value::Str(file.path.clone()),
        "folder" => Value::Str(file.folder.clone()),
        "ext" => Value::Str(file.ext.clone()),
        "size" => Value::Num(file.size as f64),
        "ctime" => Value::Date(file.ctime),
        "mtime" => Value::Date(file.mtime),
        "tags" => Value::List(file.tags.iter().map(|s| Value::Str(s.clone())).collect()),
        "links" => Value::List(file.links.iter().map(|s| Value::Str(s.clone())).collect()),
        "properties" => {
            let mut obj = BTreeMap::new();
            for (k, v) in &file.properties {
                obj.insert(k.clone(), yaml_to_value(v));
            }
            Value::Object(obj)
        }
        _ => Value::Null,
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

fn resolve_sub_parts(mut value: Value, parts: &[String]) -> Value {
    for part in parts {
        match &value {
            Value::Object(o) => {
                value = o.get(part).cloned().unwrap_or(Value::Null);
            }
            Value::List(l) if part == "length" => {
                value = Value::Num(l.len() as f64);
            }
            Value::Str(s) if part == "length" => {
                value = Value::Num(s.chars().count() as f64);
            }
            _ => return Value::Null,
        }
    }
    value
}

fn parse_duration(dur_str: &str) -> Option<(f64, String)> {
    let s = dur_str.trim();
    let mut num_part = String::new();
    let mut unit_part = String::new();
    let mut parsing_num = true;
    for c in s.chars() {
        if parsing_num {
            if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
                num_part.push(c);
            } else if c.is_whitespace() {
                if !num_part.is_empty() {
                    parsing_num = false;
                }
            } else {
                parsing_num = false;
                unit_part.push(c);
            }
        } else {
            unit_part.push(c);
        }
    }
    let num: f64 = num_part.parse().ok()?;
    let unit = unit_part.trim().to_string();
    Some((num, unit))
}

fn add_duration(ms: i64, dur_str: &str, sign: f64) -> Value {
    if let Some((val, unit)) = parse_duration(dur_str) {
        let mult = match unit.as_str() {
            "s" | "second" | "seconds" | "Second" | "Seconds" => 1000.0,
            "m" | "minute" | "minutes" | "Minute" | "Minutes" => 60_000.0,
            "h" | "hour" | "hours" | "Hour" | "Hours" => 3_600_000.0,
            "d" | "day" | "days" | "Day" | "Days" => 86_400_000.0,
            "w" | "week" | "weeks" | "Week" | "Weeks" => 604_800_000.0,
            "M" | "month" | "months" | "Month" | "Months" => 2_592_000_000.0,
            "y" | "year" | "years" | "Year" | "Years" => 31_536_000_000.0,
            _ => return Value::Null,
        };
        let delta = val * mult * sign;
        Value::Date(ms + delta as i64)
    } else {
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::expr::parse;

    fn test_eval_ok(
        expr_str: &str,
        file: &FileProps,
        note_get: &dyn Fn(&str) -> Option<Value>,
    ) -> Value {
        let expr = parse(expr_str).unwrap();
        let formulas = HashMap::new();
        let mut resolving = Vec::new();
        let mut ctx = EvalCtx {
            note_get,
            file,
            formulas: &formulas,
            this_file: None,
            this_val: None,
            resolving: &mut resolving,
        };
        eval(&expr, &mut ctx).unwrap()
    }

    #[test]
    fn test_operators() {
        let file = FileProps::default();
        let note_get = |_s: &str| None;

        assert_eq!(
            test_eval_ok("1 + 2 * 3 == 7", &file, &note_get),
            Value::Bool(true)
        );
        assert_eq!(
            test_eval_ok("!(false) == true", &file, &note_get),
            Value::Bool(true)
        );
        assert_eq!(
            test_eval_ok("\"a\" == \"a\"", &file, &note_get),
            Value::Bool(true)
        );
        assert_eq!(
            test_eval_ok("10 - 2 == 8", &file, &note_get),
            Value::Bool(true)
        );
        assert_eq!(test_eval_ok("10 % 3", &file, &note_get), Value::Num(1.0));
        assert_eq!(
            test_eval_ok("true && false", &file, &note_get),
            Value::Bool(false)
        );
        assert_eq!(
            test_eval_ok("false || true", &file, &note_get),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_functions() {
        let file = FileProps::default();
        let note_get = |_s: &str| None;

        assert_eq!(
            test_eval_ok("if(true, 1, 2) == 1", &file, &note_get),
            Value::Bool(true)
        );
        assert_eq!(
            test_eval_ok("if(false, 1, 2) == 2", &file, &note_get),
            Value::Bool(true)
        );

        // Date math
        // "2024-01-02" parsed as UTC is 1_704_153_600_000 ms.
        assert_eq!(
            test_eval_ok("date(\"2024-01-02\") + \"1 day\"", &file, &note_get),
            Value::Date(1_704_153_600_000 + 86_400_000)
        );
        assert_eq!(
            test_eval_ok("date(\"2024-01-02\") - \"1 week\"", &file, &note_get),
            Value::Date(1_704_153_600_000 - 7 * 86_400_000)
        );

        // String functions
        assert_eq!(
            test_eval_ok("contains(\"hello world\", \"world\")", &file, &note_get),
            Value::Bool(true)
        );
        assert_eq!(
            test_eval_ok("replace(\"foo-bar\", \"foo\", \"baz\")", &file, &note_get),
            Value::Str("baz-bar".to_string())
        );
        assert_eq!(
            test_eval_ok("lower(\"HELLO\")", &file, &note_get),
            Value::Str("hello".to_string())
        );
        assert_eq!(
            test_eval_ok("upper(\"hello\")", &file, &note_get),
            Value::Str("HELLO".to_string())
        );
        assert_eq!(
            test_eval_ok("title(\"hello world\")", &file, &note_get),
            Value::Str("Hello World".to_string())
        );
    }

    #[test]
    fn test_path_resolution() {
        let file = FileProps {
            name: "my_note.md".to_string(),
            size: 1234,
            ..Default::default()
        };
        let note_get = |s: &str| {
            if s == "status" {
                Some(Value::Str("done".to_string()))
            } else if s == "tasks" {
                Some(Value::List(vec![
                    Value::Str("a".to_string()),
                    Value::Str("b".to_string()),
                ]))
            } else {
                None
            }
        };

        assert_eq!(
            test_eval_ok("file.name", &file, &note_get),
            Value::Str("my_note.md".to_string())
        );
        assert_eq!(
            test_eval_ok("file.size == 1234", &file, &note_get),
            Value::Bool(true)
        );
        assert_eq!(
            test_eval_ok("status", &file, &note_get),
            Value::Str("done".to_string())
        );
        assert_eq!(
            test_eval_ok("tasks.length", &file, &note_get),
            Value::Num(2.0)
        );
        assert_eq!(
            test_eval_ok("status.length", &file, &note_get),
            Value::Num(4.0)
        );
        assert_eq!(test_eval_ok("nonexistent", &file, &note_get), Value::Null);
    }

    #[test]
    fn test_list_functions() {
        let file = FileProps::default();
        let note_get = |_s: &str| None;

        // length
        assert_eq!(
            test_eval_ok("length(split(\"a,b,c\", \",\"))", &file, &note_get),
            Value::Num(3.0)
        );
        // join
        assert_eq!(
            test_eval_ok("join(split(\"a,b,c\", \",\"), \"-\")", &file, &note_get),
            Value::Str("a-b-c".to_string())
        );
    }
}
