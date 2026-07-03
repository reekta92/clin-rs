use std::path::PathBuf;

/// Expand a leading `~` and `$VAR`/`${VAR}` tokens. Falls back to the raw
/// input on any lookup failure so callers never panic.
pub fn expand_path(input: &str) -> PathBuf {
    let mut s = input.to_string();

    // Leading tilde → home dir.
    if let Some(rest) = s.strip_prefix('~') {
        let home = directories::UserDirs::new()
            .and_then(|u| u.home_dir().to_str().map(str::to_string))
            .or_else(|| std::env::var("HOME").ok());
        if let Some(h) = home {
            s = format!("{h}{rest}");
        }
    }

    // `$VAR` and `${VAR}` tokens. Slice-based so non-ASCII path bytes pass
    // through intact — never reconstruct via `byte as char` (corrupts UTF-8).
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let (var_name, consumed, braced) = parse_var(&s[i..]);
            if !var_name.is_empty() {
                match std::env::var(&var_name) {
                    Ok(val) => {
                        out.push_str(&val);
                        i += consumed;
                        continue;
                    }
                    Err(_) => {
                        // Unresolved: emit literally (incl. `${` ... `}`).
                        if braced {
                            out.push_str(&format!("${{{var_name}}}"));
                        } else {
                            out.push('$');
                            out.push_str(&var_name);
                        }
                        i += consumed;
                        continue;
                    }
                }
            }
        }
        // Copy one UTF-8 char (1–4 bytes) verbatim.
        let ch_len = utf8_len_leading_byte(bytes[i]);
        let end = (i + ch_len).min(bytes.len());
        if let Ok(slice) = std::str::from_utf8(&bytes[i..end]) {
            out.push_str(slice);
        }
        i = end;
    }
    PathBuf::from(out)
}

/// Length of the UTF-8 sequence whose lead byte is `b`. Falls back to 1 for invalid lead bytes.
fn utf8_len_leading_byte(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

/// Parse `$VAR` or `${VAR}` at the start of `s`. Returns (name, bytes_consumed, was_braced).
fn parse_var(s: &str) -> (String, usize, bool) {
    let bytes = s.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'$' {
        return (String::new(), 0, false);
    }
    if bytes[1] == b'{' {
        if let Some(end) = s[2..].find('}') {
            let name = s[2..2 + end].to_string();
            return (name, 2 + end + 1, true);
        }
        return (String::new(), 0, false); // malformed `${`
    }
    let mut end = 1;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    (s[1..end].to_string(), end, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path_tilde() {
        let home = std::env::var("HOME").expect("HOME must be set");
        let result = expand_path("~/foo");
        let s = result.to_string_lossy().into_owned();
        assert!(s.starts_with(&home), "expected {s} to start with {home}");
        assert!(s.ends_with("/foo"), "expected {s} to end with /foo");
    }

    #[test]
    fn test_expand_path_dollar_home() {
        let home = std::env::var("HOME").expect("HOME must be set");
        let result = expand_path("$HOME/foo");
        let s = result.to_string_lossy().into_owned();
        assert!(s.starts_with(&home), "expected {s} to start with {home}");
        assert!(s.ends_with("/foo"), "expected {s} to end with /foo");
    }

    #[test]
    fn test_expand_path_braced_home() {
        let home = std::env::var("HOME").expect("HOME must be set");
        let result = expand_path("${HOME}/foo");
        let s = result.to_string_lossy().into_owned();
        assert!(s.starts_with(&home), "expected {s} to start with {home}");
        assert!(s.ends_with("/foo"), "expected {s} to end with /foo");
    }

    #[test]
    fn test_expand_path_plain_absolute() {
        let result = expand_path("/usr/local/foo");
        assert_eq!(result.to_string_lossy(), "/usr/local/foo");
    }

    #[test]
    fn test_expand_path_unresolved_var() {
        // Ensure set_var before any test that sets it
        let name = "__CLIN_TEST_UNRESOLVED_XYZ";
        // Remove if somehow set
        unsafe { std::env::remove_var(name) };
        let result = expand_path(&format!("${name}/foo"));
        let s = result.to_string_lossy().into_owned();
        assert_eq!(s, format!("${name}/foo"));
    }

    #[test]
    fn test_expand_path_unresolved_braced_var() {
        let name = "__CLIN_TEST_UNSET_YET";
        unsafe { std::env::remove_var(name) };
        let result = expand_path(&format!("${{{name}}}/x"));
        let s = result.to_string_lossy().into_owned();
        assert_eq!(s, format!("${{{name}}}/x"));
    }
}
