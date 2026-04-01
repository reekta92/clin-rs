pub fn sanitize_for_terminal(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !c.is_control() || *c == '\n' || *c == '\t'
        })
        .collect()
}
