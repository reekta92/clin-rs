#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TodoTxtItem<'a> {
    pub completed: bool,
    pub priority: Option<char>,
    pub completion_date: Option<&'a str>,
    pub creation_date: Option<&'a str>,
    pub spans: Vec<TodoTxtSpan<'a>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum TodoTxtSpan<'a> {
    Text(&'a str),
    Project(&'a str),
    Context(&'a str),
    Tag(&'a str, &'a str), // key, value
}

pub(crate) fn parse_todotxt_line(line: &str) -> TodoTxtItem<'_> {
    let mut current = line.trim_start();
    let mut completed = false;
    let mut priority = None;
    let mut completion_date = None;
    let mut creation_date = None;

    // 1. Completion Status
    if current.starts_with("x ") {
        completed = true;
        current = &current[2..];
    }

    // 2. Priority
    if current.len() >= 4
        && current.starts_with('(')
        && &current[2..=3] == ") "
        && let Some(c) = current[1..=1].chars().next()
        && c.is_ascii_uppercase()
    {
        priority = Some(c);
        current = &current[4..];
    }

    // Date parsing helper
    fn parse_date(s: &str) -> Option<(&str, &str)> {
        if s.len() >= 10 && s[..10].chars().all(|c| c.is_ascii_digit() || c == '-') {
            // Very loose check for YYYY-MM-DD
            if &s[4..5] == "-" && &s[7..8] == "-" && (s.len() == 10 || s[10..].starts_with(' ')) {
                return Some((&s[..10], s[10..].trim_start()));
            }
        }
        None
    }

    // 3. Dates
    if let Some((d1, rest1)) = parse_date(current) {
        if let Some((d2, rest2)) = parse_date(rest1) {
            completion_date = Some(d1);
            creation_date = Some(d2);
            current = rest2;
        } else {
            creation_date = Some(d1);
            current = rest1;
        }
    }

    // 4. Description (Projects, Contexts, Tags)
    let mut spans = Vec::new();
    for word in current.split_whitespace() {
        if word.len() > 1 && word.starts_with('+') {
            spans.push(TodoTxtSpan::Project(word));
        } else if word.len() > 1 && word.starts_with('@') {
            spans.push(TodoTxtSpan::Context(word));
        } else if let Some(idx) = word.find(':') {
            if idx > 0 && idx < word.len() - 1 {
                spans.push(TodoTxtSpan::Tag(&word[..idx], &word[idx + 1..]));
            } else {
                spans.push(TodoTxtSpan::Text(word));
            }
        } else {
            spans.push(TodoTxtSpan::Text(word));
        }
    }

    TodoTxtItem {
        completed,
        priority,
        completion_date,
        creation_date,
        spans,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_todotxt() {
        let item = parse_todotxt_line(
            "x (A) 2024-05-01 2024-04-01 measure space for +chapelShelving @chapel due:2024-05-30",
        );
        assert!(item.completed);
        assert_eq!(item.priority, Some('A'));
        assert_eq!(item.completion_date, Some("2024-05-01"));
        assert_eq!(item.creation_date, Some("2024-04-01"));
        assert_eq!(
            item.spans,
            vec![
                TodoTxtSpan::Text("measure"),
                TodoTxtSpan::Text("space"),
                TodoTxtSpan::Text("for"),
                TodoTxtSpan::Project("+chapelShelving"),
                TodoTxtSpan::Context("@chapel"),
                TodoTxtSpan::Tag("due", "2024-05-30"),
            ]
        );

        let item2 = parse_todotxt_line("(B) 2024-05-01 Simple task");
        assert!(!item2.completed);
        assert_eq!(item2.priority, Some('B'));
        assert_eq!(item2.completion_date, None);
        assert_eq!(item2.creation_date, Some("2024-05-01"));
        assert_eq!(
            item2.spans,
            vec![TodoTxtSpan::Text("Simple"), TodoTxtSpan::Text("task"),]
        );
    }
}
