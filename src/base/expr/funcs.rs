use super::value::Value;
use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;

pub fn dispatch_call(name: &str, args: &[Value]) -> Result<Value> {
    match name {
        "now" => {
            if !args.is_empty() {
                bail!("now() expects 0 arguments");
            }
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            Ok(Value::Date(ms))
        }
        "today" => {
            if !args.is_empty() {
                bail!("today() expects 0 arguments");
            }
            let now = Utc::now();
            let midnight = now.date_naive().and_hms_opt(0, 0, 0).expect("valid time");
            Ok(Value::Date(midnight.and_utc().timestamp_millis()))
        }
        "date" => {
            if args.len() != 1 {
                bail!("date() expects 1 argument");
            }
            match &args[0] {
                Value::Date(ms) => Ok(Value::Date(*ms)),
                Value::Num(n) => Ok(Value::Date(*n as i64)),
                Value::Str(s) => {
                    if let Some(ms) = parse_date_str(s) {
                        Ok(Value::Date(ms))
                    } else {
                        Ok(Value::Null)
                    }
                }
                _ => Ok(Value::Null),
            }
        }
        "link" => {
            if args.len() != 1 {
                bail!("link() expects 1 argument");
            }
            Ok(Value::Str(args[0].to_string()))
        }
        "max" => {
            if args.len() < 2 {
                bail!("max() expects at least 2 arguments");
            }
            let mut current_max = args[0].clone();
            for item in &args[1..] {
                if let Some(Ordering::Less) = current_max.partial_cmp_loose(item) {
                    current_max = item.clone();
                }
            }
            Ok(current_max)
        }
        "min" => {
            if args.len() < 2 {
                bail!("min() expects at least 2 arguments");
            }
            let mut current_min = args[0].clone();
            for item in &args[1..] {
                if let Some(Ordering::Greater) = current_min.partial_cmp_loose(item) {
                    current_min = item.clone();
                }
            }
            Ok(current_min)
        }
        "contains" => {
            if args.len() != 2 {
                bail!("contains() expects 2 arguments");
            }
            let hay = args[0].to_string();
            let needle = args[1].to_string();
            Ok(Value::Bool(hay.contains(&needle)))
        }
        "replace" => {
            if args.len() != 3 {
                bail!("replace() expects 3 arguments");
            }
            let s = args[0].to_string();
            let from = args[1].to_string();
            let to = args[2].to_string();
            Ok(Value::Str(s.replace(&from, &to)))
        }
        "split" => {
            if args.len() != 2 {
                bail!("split() expects 2 arguments");
            }
            let s = args[0].to_string();
            let sep = args[1].to_string();
            let parts: Vec<Value> = s.split(&sep).map(|p| Value::Str(p.to_string())).collect();
            Ok(Value::List(parts))
        }
        "lower" => {
            if args.len() != 1 {
                bail!("lower() expects 1 argument");
            }
            Ok(Value::Str(args[0].to_string().to_lowercase()))
        }
        "upper" => {
            if args.len() != 1 {
                bail!("upper() expects 1 argument");
            }
            Ok(Value::Str(args[0].to_string().to_uppercase()))
        }
        "title" => {
            if args.len() != 1 {
                bail!("title() expects 1 argument");
            }
            let s = args[0].to_string();
            let title_cased = s
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<String>>()
                .join(" ");
            Ok(Value::Str(title_cased))
        }
        "round" => {
            if args.len() != 1 {
                bail!("round() expects 1 argument");
            }
            match args[0] {
                Value::Num(n) => Ok(Value::Num(n.round())),
                _ => Ok(Value::Null),
            }
        }
        "ceil" => {
            if args.len() != 1 {
                bail!("ceil() expects 1 argument");
            }
            match args[0] {
                Value::Num(n) => Ok(Value::Num(n.ceil())),
                _ => Ok(Value::Null),
            }
        }
        "floor" => {
            if args.len() != 1 {
                bail!("floor() expects 1 argument");
            }
            match args[0] {
                Value::Num(n) => Ok(Value::Num(n.floor())),
                _ => Ok(Value::Null),
            }
        }
        "abs" => {
            if args.len() != 1 {
                bail!("abs() expects 1 argument");
            }
            match args[0] {
                Value::Num(n) => Ok(Value::Num(n.abs())),
                _ => Ok(Value::Null),
            }
        }
        "toFixed" => {
            if args.len() != 2 {
                bail!("toFixed() expects 2 arguments");
            }
            let n = match args[0] {
                Value::Num(num) => num,
                _ => return Ok(Value::Null),
            };
            let digits = match args[1] {
                Value::Num(num) => num as usize,
                _ => return Ok(Value::Null),
            };
            Ok(Value::Str(format!("{:.1$}", n, digits)))
        }
        "format" => {
            if args.len() != 2 {
                bail!("format() expects 2 arguments");
            }
            let ms = match args[0] {
                Value::Date(ms) => ms,
                _ => return Ok(Value::Null),
            };
            let fmt = args[1].to_string();
            if let Some(dt) = DateTime::from_timestamp_millis(ms) {
                Ok(Value::Str(dt.format(&fmt).to_string()))
            } else {
                Ok(Value::Null)
            }
        }
        "relative" => {
            if args.len() != 1 {
                bail!("relative() expects 1 argument");
            }
            let ms = match args[0] {
                Value::Date(ms) => ms,
                Value::Num(n) => n as i64,
                _ => return Ok(Value::Null),
            };
            let formatted = crate::ui::format_relative_time((ms / 1000) as u64).to_string();
            Ok(Value::Str(formatted))
        }
        "time" => {
            if args.len() != 1 {
                bail!("time() expects 1 argument");
            }
            let ms = match args[0] {
                Value::Date(ms) => ms,
                _ => return Ok(Value::Null),
            };
            if let Some(dt) = DateTime::from_timestamp_millis(ms) {
                Ok(Value::Str(dt.format("%H:%M:%S").to_string()))
            } else {
                Ok(Value::Null)
            }
        }
        "sort" => {
            if args.len() != 1 {
                bail!("sort() expects 1 argument");
            }
            match &args[0] {
                Value::List(l) => {
                    let mut sorted = l.clone();
                    sorted.sort_by(|a, b| a.partial_cmp_loose(b).unwrap_or(Ordering::Equal));
                    Ok(Value::List(sorted))
                }
                _ => Ok(Value::Null),
            }
        }
        "join" => {
            if args.is_empty() || args.len() > 2 {
                bail!("join() expects 1 or 2 arguments");
            }
            let sep = if args.len() == 2 {
                args[1].to_string()
            } else {
                ", ".to_string()
            };
            match &args[0] {
                Value::List(l) => {
                    let s: Vec<String> = l.iter().map(|item| item.to_string()).collect();
                    Ok(Value::Str(s.join(&sep)))
                }
                _ => Ok(Value::Null),
            }
        }
        "unique" => {
            if args.len() != 1 {
                bail!("unique() expects 1 argument");
            }
            match &args[0] {
                Value::List(l) => {
                    let mut unique = Vec::new();
                    for item in l {
                        if !unique.contains(item) {
                            unique.push(item.clone());
                        }
                    }
                    Ok(Value::List(unique))
                }
                _ => Ok(Value::Null),
            }
        }
        "mean" => {
            if args.len() != 1 {
                bail!("mean() expects 1 argument");
            }
            match &args[0] {
                Value::List(l) => {
                    let mut sum = 0.0;
                    let mut count = 0;
                    for item in l {
                        if let Value::Num(n) = item {
                            sum += *n;
                            count += 1;
                        }
                    }
                    if count == 0 {
                        Ok(Value::Null)
                    } else {
                        Ok(Value::Num(sum / count as f64))
                    }
                }
                _ => Ok(Value::Null),
            }
        }
        "length" => {
            if args.len() != 1 {
                bail!("length() expects 1 argument");
            }
            match &args[0] {
                Value::List(l) => Ok(Value::Num(l.len() as f64)),
                Value::Str(s) => Ok(Value::Num(s.chars().count() as f64)),
                _ => Ok(Value::Num(0.0)),
            }
        }
        _ => bail!("Unknown function: {}", name),
    }
}

pub fn dispatch_method(name: &str, receiver: &Value, args: &[Value]) -> Result<Value> {
    // If it's a method, we prepend receiver to args and dispatch to global function!
    // Except for properties/methods like .length which is already handled in path resolution/eval.
    let mut all_args = vec![receiver.clone()];
    all_args.extend_from_slice(args);
    dispatch_call(name, &all_args)
}

fn parse_date_str(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
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
