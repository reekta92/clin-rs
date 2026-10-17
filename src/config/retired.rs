//! Remove retired options without touching unrelated config or its comments.
use toml_edit::DocumentMut;

fn remove_table(document: &mut DocumentMut, key: &str) -> bool {
    let Some(item) = document.remove(key) else {
        return false;
    };
    if let Some(prefix) = item
        .as_table()
        .and_then(|table| table.decor().prefix())
        .and_then(toml_edit::RawString::as_str)
    {
        let existing = document
            .as_table()
            .decor()
            .prefix()
            .and_then(toml_edit::RawString::as_str)
            .unwrap_or("");
        let prefix = format!("{prefix}{existing}");
        document.as_table_mut().decor_mut().set_prefix(prefix);
    }
    true
}

pub(super) fn remove_retired_options(document: &mut DocumentMut) -> bool {
    let mut changed = remove_table(document, "backup");
    for (section, keys) in [
        ("features", &["backup", "encryption"][..]),
        ("list", &["preview_encryption"][..]),
        ("notes", &["rename_on_title_change"][..]),
        ("statusline", &["backup"][..]),
    ] {
        if let Some(table) = document
            .get_mut(section)
            .and_then(toml_edit::Item::as_table_like_mut)
        {
            for key in keys {
                changed |= table.remove(key).is_some();
            }
        }
    }
    if document
        .get("notes")
        .and_then(toml_edit::Item::as_table_like)
        .is_some_and(|table| table.is_empty())
    {
        remove_table(document, "notes");
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retires_only_removed_fields_preserving_comments_and_preferences() {
        let mut document: DocumentMut = "# keep this comment\n[backup]\nauto_push = true\n[features]\nbackup = false\nencryption = false\ngraph_view = true # keep graph\n[list]\npreview_encryption = true\nshow_hidden = true\n[notes]\nrename_on_title_change = true\n".parse().unwrap();
        assert!(remove_retired_options(&mut document));
        let source = document.to_string();
        assert!(!source.contains("backup"));
        assert!(!source.contains("encryption"));
        assert!(!source.contains("rename_on_title_change"));
        assert!(source.contains("# keep this comment"));
        assert!(source.contains("# keep graph"));
        assert!(source.contains("show_hidden = true"));
        assert!(!remove_retired_options(&mut document));
    }
}
