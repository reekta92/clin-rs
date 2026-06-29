use std::borrow::Cow;

pub fn sanitize_for_terminal(s: &str) -> Cow<'_, str> {
    let needs_sanitization = s.chars().any(char::is_control);
    if needs_sanitization {
        Cow::Owned(s.chars().filter(|c| !c.is_control()).collect())
    } else {
        Cow::Borrowed(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_newlines_and_tabs() {
        assert_eq!(sanitize_for_terminal("a\nb\tc\x07d"), "abcd");
    }

    #[test]
    fn keeps_unicode() {
        assert_eq!(sanitize_for_terminal("café 日本語"), "café 日本語");
    }
}
