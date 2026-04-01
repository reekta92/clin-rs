use std::borrow::Cow;

pub fn sanitize_for_terminal(s: &str) -> Cow<'_, str> {
    let needs_sanitization = s.chars().any(|c| c.is_control() && c != '\n' && c != '\t');

    if needs_sanitization {
        Cow::Owned(
            s.chars()
                .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                .collect(),
        )
    } else {
        Cow::Borrowed(s)
    }
}
