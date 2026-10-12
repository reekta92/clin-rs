//! Bounded native regular expressions and UTF-16 string replacement/splitting.

use super::eval::{append, arity};
use super::value::{Text, Value};
use anyhow::{Context, Result, bail};
use fancy_regex::{Captures, Regex, RegexBuilder};
use lru::LruCache;
use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::sync::Arc;

thread_local! {
    static CACHE: RefCell<LruCache<(String,String),Arc<Regex>>> = RefCell::new(LruCache::new(NonZeroUsize::new(16).expect("nonzero regex cache")));
}

fn compiled(pattern: &str, flags: &str) -> Result<Arc<Regex>> {
    let key = (pattern.to_owned(), flags.to_owned());
    if let Some(regex) = CACHE.with(|cache| cache.borrow_mut().get(&key).cloned()) {
        return Ok(regex);
    }
    // shortcut: native syntax/case folding is not full ECMAScript; close gaps before parity release.
    if flags.contains('v') {
        bail!("Unicode-set regular expressions are not supported; source retained");
    }
    let modifiers: String = flags.chars().filter(|flag| "ims".contains(*flag)).collect();
    let pattern = translate(pattern, flags.contains('u'))?;
    let pattern = format!("(?{modifiers}:{pattern})");
    let regex = Arc::new(
        RegexBuilder::new(&pattern)
            .backtrack_limit(100_000)
            .delegate_size_limit(1024 * 1024)
            .build()
            .context("unsupported or invalid regular expression")?,
    );
    CACHE.with(|cache| cache.borrow_mut().put(key, regex.clone()));
    Ok(regex)
}

fn unit_char(unit: u16) -> char {
    char::from_u32(u32::from(unit)).unwrap_or_else(|| {
        char::from_u32(0xf_0000 + u32::from(unit - 0xd800))
            .expect("surrogate mapping is a Unicode scalar")
    })
}
fn encoded(text: &Text, unicode: bool) -> Result<String> {
    if unicode {
        text.utf8()
    } else {
        Ok(text.0.iter().map(|unit| unit_char(*unit)).collect())
    }
}
fn decoded(text: &str, unicode: bool) -> Text {
    if unicode {
        return Text::from(text);
    }
    let mut units = Vec::new();
    for ch in text.chars() {
        if (0xf_0000..=0xf_07ff).contains(&u32::from(ch)) {
            units.push((u32::from(ch) - 0xf_0000 + 0xd800) as u16);
        } else {
            units.extend(ch.encode_utf16(&mut [0; 2]).iter().copied());
        }
    }
    Text(units)
}

fn translate(pattern: &str, unicode: bool) -> Result<String> {
    let mut output = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            if unicode {
                output.push(ch);
            } else {
                for unit in ch.encode_utf16(&mut [0; 2]) {
                    output.push(unit_char(*unit));
                }
            }
            continue;
        }
        let escape = chars.next().context("unterminated regexp escape")?;
        match escape {
            '/' => output.push('/'),
            'u' | 'x' => {
                if escape == 'u' && chars.peek() == Some(&'{') {
                    if !unicode {
                        bail!("Unicode scalar escapes require the u flag");
                    }
                    chars.next();
                    let mut hex = String::new();
                    while chars.peek().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                        hex.push(chars.next().expect("peeked hex digit"));
                    }
                    if chars.next() != Some('}') {
                        bail!("unterminated Unicode regexp escape");
                    }
                    let scalar =
                        u32::from_str_radix(&hex, 16).context("invalid Unicode regexp escape")?;
                    let ch = char::from_u32(scalar).context("invalid Unicode regexp scalar")?;
                    output.push_str(&format!("\\x{{{:x}}}", u32::from(ch)));
                } else {
                    let mut unit = 0;
                    for _ in 0..if escape == 'u' { 4 } else { 2 } {
                        unit = unit * 16
                            + chars
                                .next()
                                .and_then(|ch| ch.to_digit(16))
                                .context("invalid regexp hex escape")?
                                as u16;
                    }
                    if unicode && (0xd800..=0xdfff).contains(&unit) {
                        bail!(
                            "surrogate regexp escapes with the u flag require an ECMAScript backend"
                        );
                    }
                    output.push_str(&format!("\\x{{{:x}}}", u32::from(unit_char(unit))));
                }
            }
            'd' => output.push_str("[0-9]"),
            'D' => output.push_str("[^0-9]"),
            'w' => output.push_str("[A-Za-z0-9_]"),
            'W' => output.push_str("[^A-Za-z0-9_]"),
            'c' => {
                let letter = chars
                    .next()
                    .filter(char::is_ascii_alphabetic)
                    .context("invalid control escape")?;
                output.push_str(&format!("\\x{{{:x}}}", u32::from(letter) & 31));
            }
            'p' | 'P' if !unicode => bail!("Unicode property escapes require the u flag"),
            other => {
                output.push('\\');
                output.push(other);
            }
        }
    }
    Ok(output)
}

pub(crate) fn method(pattern: &str, flags: &str, name: &str, values: &[Value]) -> Result<Value> {
    if name != "matches" {
        bail!("unsupported regexp method {name:?}");
    }
    arity(values.len(), 1, 1)?;
    let input = encoded(values[0].as_text()?, flags.contains('u'))?;
    let found = compiled(pattern, flags)?.find(&input)?;
    Ok(Value::Boolean(found.is_some_and(|found| {
        !flags.contains('y') || found.start() == 0
    })))
}

fn expansion(
    output: &mut Vec<u16>,
    replacement: &Text,
    original: &Text,
    start: usize,
    end: usize,
    groups: &[Option<Text>],
) -> Result<()> {
    let mut index = 0;
    while index < replacement.0.len() {
        if replacement.0[index] != u16::from(b'$') || index + 1 == replacement.0.len() {
            append(output, &replacement.0[index..index + 1])?;
            index += 1;
            continue;
        }
        let next = replacement.0[index + 1];
        match next {
            36 => append(output, &[36])?,
            38 => append(output, &original.0[start..end])?,
            96 => append(output, &original.0[..start])?,
            39 => append(output, &original.0[end..])?,
            49..=57 => {
                let mut group = usize::from(next - 48);
                let mut consumed = 2;
                if let Some(digit @ 48..=57) = replacement.0.get(index + 2) {
                    let candidate = group * 10 + usize::from(*digit - 48);
                    if candidate < groups.len() {
                        group = candidate;
                        consumed = 3;
                    }
                }
                if group < groups.len() {
                    if let Some(group) = &groups[group] {
                        append(output, &group.0)?;
                    }
                    index += consumed;
                    continue;
                }
                append(output, &[36])?;
                index += 1;
                continue;
            }
            _ => {
                append(output, &[36])?;
                index += 1;
                continue;
            }
        }
        index += 2;
    }
    Ok(())
}
fn groups(captures: &Captures<'_>, unicode: bool) -> Vec<Option<Text>> {
    (0..captures.len())
        .map(|index| {
            captures
                .get(index)
                .map(|group| decoded(group.as_str(), unicode))
        })
        .collect()
}

pub(crate) fn replace(text: &Text, pattern: &Value, replacement: &Text) -> Result<Text> {
    let mut output = Vec::new();
    if let Value::String(needle) = pattern {
        let mut copied = 0;
        let mut index = 0;
        while index <= text.0.len() {
            if text.0[index..].starts_with(&needle.0) {
                append(&mut output, &text.0[copied..index])?;
                expansion(
                    &mut output,
                    replacement,
                    text,
                    index,
                    index + needle.0.len(),
                    &[],
                )?;
                copied = index + needle.0.len();
                index = if needle.0.is_empty() {
                    index + 1
                } else {
                    copied
                };
            } else {
                index += 1;
            }
        }
        append(&mut output, &text.0[copied..])?;
        return Ok(Text(output));
    }
    let Value::Regex { pattern, flags } = pattern else {
        bail!("replace pattern must be string or regexp");
    };
    let unicode = flags.contains('u');
    let input = encoded(text, unicode)?;
    let regex = compiled(pattern, flags)?;
    let mut copied = 0;
    let mut position = 0;
    while position <= input.len() {
        let Some(captures) = regex.captures_from_pos(&input, position)? else {
            break;
        };
        let found = captures.get(0).context("regexp match is absent")?;
        if flags.contains('y') && found.start() != position {
            break;
        }
        append(
            &mut output,
            &decoded(&input[copied..found.start()], unicode).0,
        )?;
        let start = decoded(&input[..found.start()], unicode).0.len();
        let end = decoded(&input[..found.end()], unicode).0.len();
        expansion(
            &mut output,
            replacement,
            text,
            start,
            end,
            &groups(&captures, unicode),
        )?;
        copied = found.end();
        if !flags.contains('g') {
            break;
        }
        position = if found.start() == found.end() {
            found.end()
                + input[found.end()..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8)
        } else {
            found.end()
        };
    }
    append(&mut output, &decoded(&input[copied..], unicode).0)?;
    Ok(Text(output))
}

pub(crate) fn split(text: &Text, separator: &Value, limit: usize) -> Result<Vec<Value>> {
    let mut output = Vec::new();
    if limit == 0 {
        return Ok(output);
    }
    if let Value::String(separator) = separator {
        if separator.0.is_empty() {
            return Ok(text
                .0
                .iter()
                .take(limit)
                .map(|unit| Value::String(Text(vec![*unit])))
                .collect());
        }
        let mut copied = 0;
        let mut index = 0;
        while index + separator.0.len() <= text.0.len() {
            if text.0[index..].starts_with(&separator.0) {
                output.push(Value::String(Text(text.0[copied..index].to_vec())));
                if output.len() == limit {
                    return Ok(output);
                }
                index += separator.0.len();
                copied = index;
            } else {
                index += 1;
            }
        }
        output.push(Value::String(Text(text.0[copied..].to_vec())));
        return Ok(output);
    }
    let Value::Regex { pattern, flags } = separator else {
        bail!("split separator must be string or regexp");
    };
    let unicode = flags.contains('u');
    let input = encoded(text, unicode)?;
    let regex = compiled(pattern, flags)?;
    let mut copied = 0;
    for captures in regex.captures_iter(&input) {
        let captures = captures?;
        let found = captures.get(0).context("regexp match is absent")?;
        if found.start() == found.end() && (found.start() == copied || found.end() == input.len()) {
            continue;
        }
        output.push(Value::String(decoded(
            &input[copied..found.start()],
            unicode,
        )));
        if output.len() == limit {
            return Ok(output);
        }
        for group in groups(&captures, unicode).into_iter().skip(1) {
            output.push(group.map_or(Value::Null, Value::String));
            if output.len() == limit {
                return Ok(output);
            }
        }
        copied = found.end();
    }
    output.push(Value::String(decoded(&input[copied..], unicode)));
    Ok(output)
}
