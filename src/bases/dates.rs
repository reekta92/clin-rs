//! Date presentation uses local time and Moment-style tokens, never browser rendering.

use super::eval::arity;
use super::value::{Value, parse_date};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Local, TimeDelta, Timelike};

pub(crate) fn method(
    date: DateTime<Local>,
    name: &str,
    values: &[Value],
    now: DateTime<Local>,
) -> Result<Value> {
    Ok(match name {
        "date" => {
            arity(values.len(), 0, 0)?;
            Value::Date(parse_date(&date.format("%Y-%m-%d").to_string())?)
        }
        "time" => {
            arity(values.len(), 0, 0)?;
            Value::text(&date.format("%H:%M:%S").to_string())
        }
        "relative" => {
            arity(values.len(), 0, 0)?;
            Value::text(&relative(date, now))
        }
        "format" => {
            arity(values.len(), 1, 1)?;
            Value::text(&format_date(date, &values[0].as_text()?.utf8()?)?)
        }
        _ => bail!("unsupported date method {name:?}"),
    })
}

fn relative(date: DateTime<Local>, now: DateTime<Local>) -> String {
    let seconds = (date.timestamp_millis() - now.timestamp_millis()) as f64 / 1000.0;
    let absolute = seconds.abs();
    let phrase = if absolute < 45.0 {
        "a few seconds".into()
    } else if absolute < 90.0 {
        "a minute".into()
    } else if absolute < 45.0 * 60.0 {
        format!("{} minutes", (absolute / 60.0).round())
    } else if absolute < 90.0 * 60.0 {
        "an hour".into()
    } else if absolute < 22.0 * 3600.0 {
        format!("{} hours", (absolute / 3600.0).round())
    } else if absolute < 36.0 * 3600.0 {
        "a day".into()
    } else if absolute < 26.0 * 86_400.0 {
        format!("{} days", (absolute / 86_400.0).round())
    } else if absolute < 45.0 * 86_400.0 {
        "a month".into()
    } else if absolute < 320.0 * 86_400.0 {
        format!("{} months", (absolute / (30.436_875 * 86_400.0)).round())
    } else if absolute < 548.0 * 86_400.0 {
        "a year".into()
    } else {
        format!("{} years", (absolute / (365.2425 * 86_400.0)).round())
    };
    if seconds > 0.0 {
        format!("in {phrase}")
    } else {
        format!("{phrase} ago")
    }
}

pub(crate) fn format_date(date: DateTime<Local>, source: &str) -> Result<String> {
    const TOKENS: &[&str] = &[
        "LLLL", "llll", "YYYY", "MMMM", "dddd", "DDDD", "GGGG", "gggg", "LTS", "LLL", "lll", "MMM",
        "ddd", "DDD", "SSS", "SS", "Do", "Mo", "Qo", "YY", "MM", "DD", "HH", "hh", "kk", "mm",
        "ss", "ZZ", "WW", "ww", "dd", "LL", "ll", "LT", "M", "D", "H", "h", "k", "m", "s", "S",
        "Z", "X", "x", "A", "a", "Q", "d", "E", "e", "W", "w", "L", "l",
    ];
    let mut remaining = source;
    let mut output = String::new();
    while !remaining.is_empty() {
        if let Some(literal) = remaining.strip_prefix('[') {
            let end = literal
                .find(']')
                .context("unterminated date format literal")?;
            output.push_str(&literal[..end]);
            remaining = &literal[end + 1..];
            continue;
        }
        if let Some(literal) = remaining.strip_prefix('\\') {
            let ch = literal
                .chars()
                .next()
                .context("unterminated date format escape")?;
            output.push(ch);
            remaining = &literal[ch.len_utf8()..];
            continue;
        }
        let Some(token) = TOKENS.iter().find(|token| remaining.starts_with(**token)) else {
            let ch = remaining
                .chars()
                .next()
                .context("missing date format character")?;
            output.push(ch);
            remaining = &remaining[ch.len_utf8()..];
            continue;
        };
        let format = match *token {
            "YYYY" => Some("%Y"),
            "YY" => Some("%y"),
            "M" => Some("%-m"),
            "MM" => Some("%m"),
            "MMM" => Some("%b"),
            "MMMM" => Some("%B"),
            "D" => Some("%-d"),
            "DD" => Some("%d"),
            "DDD" => Some("%-j"),
            "DDDD" => Some("%j"),
            "H" => Some("%-H"),
            "HH" => Some("%H"),
            "h" => Some("%-I"),
            "hh" => Some("%I"),
            "m" => Some("%-M"),
            "mm" => Some("%M"),
            "s" => Some("%-S"),
            "ss" => Some("%S"),
            "Z" => Some("%:z"),
            "ZZ" => Some("%z"),
            "ddd" => Some("%a"),
            "dddd" => Some("%A"),
            "GGGG" => Some("%G"),
            "WW" => Some("%V"),
            "W" => Some("%-V"),
            "A" => Some("%p"),
            "a" => Some("%P"),
            _ => None,
        };
        if let Some(format) = format {
            output.push_str(&date.format(format).to_string());
        } else {
            let value = match *token {
                "Q" => ((date.month() - 1) / 3 + 1).to_string(),
                "Qo" => ordinal((date.month() - 1) / 3 + 1),
                "Do" => ordinal(date.day()),
                "Mo" => ordinal(date.month()),
                "S" => (date.timestamp_subsec_millis() / 100).to_string(),
                "SS" => format!("{:02}", date.timestamp_subsec_millis() / 10),
                "SSS" => format!("{:03}", date.timestamp_subsec_millis()),
                "k" => (if date.hour() == 0 { 24 } else { date.hour() }).to_string(),
                "kk" => format!("{:02}", if date.hour() == 0 { 24 } else { date.hour() }),
                "d" | "e" => date.weekday().num_days_from_sunday().to_string(),
                "E" => date.weekday().number_from_monday().to_string(),
                "dd" => date.format("%a").to_string()[..2].to_owned(),
                "x" => date.timestamp_millis().to_string(),
                "X" => (date.timestamp_millis() as f64 / 1000.0).to_string(),
                "w" | "ww" | "gggg" => {
                    let week = locale_week(date)?;
                    if *token == "gggg" {
                        week.0.to_string()
                    } else if *token == "ww" {
                        format!("{:02}", week.1)
                    } else {
                        week.1.to_string()
                    }
                }
                "L" => date.format("%m/%d/%Y").to_string(),
                "l" => date.format("%-m/%-d/%Y").to_string(),
                "LT" => date.format("%-I:%M %p").to_string(),
                "LTS" => date.format("%-I:%M:%S %p").to_string(),
                "LL" => date.format("%B %-d, %Y").to_string(),
                "ll" => date.format("%b %-d, %Y").to_string(),
                "LLL" => date.format("%B %-d, %Y %-I:%M %p").to_string(),
                "lll" => date.format("%b %-d, %Y %-I:%M %p").to_string(),
                "LLLL" => date.format("%A, %B %-d, %Y %-I:%M %p").to_string(),
                "llll" => date.format("%a, %b %-d, %Y %-I:%M %p").to_string(),
                _ => bail!("unsupported date format token {token:?}"),
            };
            output.push_str(&value);
        }
        remaining = &remaining[token.len()..];
    }
    Ok(output)
}
fn ordinal(value: u32) -> String {
    let suffix = if (11..=13).contains(&(value % 100)) {
        "th"
    } else {
        match value % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{value}{suffix}")
}
fn locale_week(date: DateTime<Local>) -> Result<(i32, u32)> {
    let day = date.date_naive();
    let year = date.year();
    let first = chrono::NaiveDate::from_ymd_opt(year, 1, 1).expect("valid current year");
    let next = chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1);
    let start = first
        .checked_sub_signed(TimeDelta::days(i64::from(
            first.weekday().num_days_from_sunday(),
        )))
        .context("week date exceeds supported range")?;
    if let Some(next) = next {
        let next_start = next
            .checked_sub_signed(TimeDelta::days(i64::from(
                next.weekday().num_days_from_sunday(),
            )))
            .context("week date exceeds supported range")?;
        if day >= next_start {
            return Ok((year + 1, 1));
        }
    }
    Ok((year, ((day - start).num_days() / 7 + 1) as u32))
}
