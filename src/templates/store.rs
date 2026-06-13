pub fn sanitize_filename(name: &str) -> String {
    let mut result = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            result.push(c.to_ascii_lowercase());
        } else if c == ' ' {
            result.push('_');
        }
    }
    if result.is_empty() {
        result = "template".to_string();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Meeting Notes"), "meeting_notes");
        assert_eq!(sanitize_filename("todo-list"), "todo-list");
        assert_eq!(sanitize_filename("My Template!"), "my_template");
        assert_eq!(sanitize_filename(""), "template");
    }
}
