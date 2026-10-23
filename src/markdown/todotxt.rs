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
    Tag(&'a str),
}

impl<'a> TodoTxtSpan<'a> {
    pub fn text(&self) -> &'a str {
        match self {
            Self::Text(text) | Self::Project(text) | Self::Context(text) | Self::Tag(text) => text,
        }
    }
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
    let bytes = current.as_bytes();
    if bytes.len() >= 4
        && bytes[0] == b'('
        && bytes[1].is_ascii_uppercase()
        && bytes[2] == b')'
        && bytes[3].is_ascii_whitespace()
    {
        priority = Some(char::from(bytes[1]));
        current = &current[4..];
    }

    // Date parsing helper
    fn parse_date(s: &str) -> Option<(&str, &str)> {
        let date = s.get(..10)?;
        if date.as_bytes()[4] == b'-'
            && date.as_bytes()[7] == b'-'
            && date.bytes().all(|b| b.is_ascii_digit() || b == b'-')
            && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok()
            && (s.len() == 10 || s[10..].starts_with(char::is_whitespace))
        {
            return Some((date, s[10..].trim_start()));
        }
        None
    }

    // 3. Dates
    if let Some((d1, rest1)) = parse_date(current) {
        if completed {
            completion_date = Some(d1);
            current = rest1;
            if let Some((d2, rest2)) = parse_date(current) {
                creation_date = Some(d2);
                current = rest2;
            }
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
                spans.push(TodoTxtSpan::Tag(word));
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
                TodoTxtSpan::Tag("due:2024-05-30"),
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

    #[test]
    fn unicode_and_invalid_dates_are_plain_text() {
        for line in [
            "(é) unicode priority",
            "abcdefghié remains a task",
            "2024-02-30 invalid",
        ] {
            let item = parse_todotxt_line(line);
            assert_eq!(item.priority, None);
            assert_eq!(item.creation_date, None);
            assert_eq!(
                item.spans[0].text(),
                line.split_whitespace().next().unwrap()
            );
        }
        let item = parse_todotxt_line("x 2024-02-29 finished +家 @iş due:yarın");
        assert_eq!(item.completion_date, Some("2024-02-29"));
        assert_eq!(item.creation_date, None);
        assert_eq!(item.spans[1], TodoTxtSpan::Project("+家"));
        assert_eq!(item.spans[2], TodoTxtSpan::Context("@iş"));
        assert_eq!(item.spans[3], TodoTxtSpan::Tag("due:yarın"));
    }
}
