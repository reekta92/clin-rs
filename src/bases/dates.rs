//! Date presentation uses local time and Moment-style tokens, never browser rendering.

use super::eval::arity;
use super::value::{Value, parse_date};
use anyhow::{Context, Result, bail};
use jiff::{ToSpan, Zoned, civil::Date};

pub(crate) fn method(date: &Zoned, name: &str, values: &[Value], now: &Zoned) -> Result<Value> {
    Ok(match name {
        "date" => {
            arity(values.len(), 0, 0)?;
            Value::Date(parse_date(&date.strftime("%Y-%m-%d").to_string())?)
        }
        "time" => {
            arity(values.len(), 0, 0)?;
            Value::text(&date.strftime("%H:%M:%S").to_string())
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

fn relative(date: &Zoned, now: &Zoned) -> String {
    let seconds =
        (date.timestamp().as_millisecond() - now.timestamp().as_millisecond()) as f64 / 1000.0;
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

pub(crate) fn format_date(date: &Zoned, source: &str) -> Result<String> {
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
            output.push_str(&date.strftime(format).to_string());
        } else {
            let value = match *token {
                "Q" => ((date.month() - 1) / 3 + 1).to_string(),
                "Qo" => ordinal((date.month() - 1) / 3 + 1),
                "Do" => ordinal(date.day()),
                "Mo" => ordinal(date.month()),
                "S" => ((date.subsec_nanosecond() / 1_000_000) / 100).to_string(),
                "SS" => format!("{:02}", (date.subsec_nanosecond() / 1_000_000) / 10),
                "SSS" => format!("{:03}", (date.subsec_nanosecond() / 1_000_000)),
                "k" => (if date.hour() == 0 { 24 } else { date.hour() }).to_string(),
                "kk" => format!("{:02}", if date.hour() == 0 { 24 } else { date.hour() }),
                "d" | "e" => date.weekday().to_sunday_zero_offset().to_string(),
                "E" => date.weekday().to_monday_one_offset().to_string(),
                "dd" => date.strftime("%a").to_string()[..2].to_owned(),
                "x" => date.timestamp().as_millisecond().to_string(),
                "X" => (date.timestamp().as_millisecond() as f64 / 1000.0).to_string(),
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
                "L" => date.strftime("%m/%d/%Y").to_string(),
                "l" => date.strftime("%-m/%-d/%Y").to_string(),
                "LT" => date.strftime("%-I:%M %p").to_string(),
                "LTS" => date.strftime("%-I:%M:%S %p").to_string(),
                "LL" => date.strftime("%B %-d, %Y").to_string(),
                "ll" => date.strftime("%b %-d, %Y").to_string(),
                "LLL" => date.strftime("%B %-d, %Y %-I:%M %p").to_string(),
                "lll" => date.strftime("%b %-d, %Y %-I:%M %p").to_string(),
                "LLLL" => date.strftime("%A, %B %-d, %Y %-I:%M %p").to_string(),
                "llll" => date.strftime("%a, %b %-d, %Y %-I:%M %p").to_string(),
                _ => bail!("unsupported date format token {token:?}"),
            };
            output.push_str(&value);
        }
        remaining = &remaining[token.len()..];
    }
    Ok(output)
}
fn ordinal(value: i8) -> String {
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
fn locale_week(date: &Zoned) -> Result<(i16, i64)> {
    let day = date.date();
    let year = date.year();
    let first = Date::new(year, 1, 1).expect("valid current year");
    let start = first
        .checked_sub(i64::from(first.weekday().to_sunday_zero_offset()).days())
        .context("week date exceeds supported range")?;
    if let Ok(next) = Date::new(year + 1, 1, 1) {
        let next_start = next
            .checked_sub(i64::from(next.weekday().to_sunday_zero_offset()).days())
            .context("week date exceeds supported range")?;
        if day >= next_start {
            return Ok((year + 1, 1));
        }
    }
    Ok((year, i64::from(day.since(start)?.get_days()) / 7 + 1))
}
