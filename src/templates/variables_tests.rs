use super::*;
use crate::app::{App, ViewMode};
use crate::config::{ConfigTestGuard, FeatureState, NotesLayout};
use crate::editor_document::EditorDocument;
use crate::storage::Storage;
use std::fs;

fn storage(root: &Path) -> Storage {
    Storage {
        data_dir: root.to_path_buf(),
        config_dir: root.join(".clin"),
        notes_dir: root.to_path_buf(),
        templates_dir: root.join(".clin/templates"),
        key: rand::random(),
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: true,
    }
}

pub(super) fn config(root: &Path, folder: &str, content: &str) {
    let dir = root.join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(FOLDER_TEMPLATE_CONFIG), content).unwrap();
}

fn template() -> Template {
    Template {
        name: "Book".into(),
        title: TitleConfig {
            template: Some("{book} — {chapter}".into()),
        },
        content: ContentConfig {
            template: "{book}: {chapter} / {date}".into(),
        },
    }
}

#[test]
fn inheritance_merges_ancestors_and_nearest_values_win() {
    let dir = tempfile::tempdir().unwrap();
    let storage = storage(dir.path());
    config(
        dir.path(),
        "",
        "[variables]\nbook = 'Root Book'\nchapter = 'root'\n",
    );
    config(dir.path(), "books", "[variables]\nbook = 'Rust'\n");
    config(
        dir.path(),
        "books/chapters/04",
        "[variables]\nchapter = '04'\n",
    );
    let vars = storage
        .folder_template_variables("books/chapters/04/new")
        .unwrap();
    assert_eq!(vars["book"], "Rust");
    assert_eq!(vars["chapter"], "04");
    assert!(
        storage
            .folder_template_variables("empty")
            .unwrap()
            .contains_key("book")
    );
}

#[test]
fn inherit_false_cuts_off_ancestors_but_not_descendants() {
    let dir = tempfile::tempdir().unwrap();
    let storage = storage(dir.path());
    // Invalid discarded ancestor must not break a non-inheriting subtree.
    config(dir.path(), "", "not valid toml");
    config(
        dir.path(),
        "book",
        "inherit_parent = false\n[variables]\nbook = 'Rust'\nchapter = '01'\n",
    );
    config(
        dir.path(),
        "book/chapter",
        "inherit_parent = true\n[variables]\nchapter = '02'\n",
    );
    let vars = storage.folder_template_variables("book/chapter").unwrap();
    assert_eq!(vars["book"], "Rust");
    assert_eq!(vars["chapter"], "02");
    config(
        dir.path(),
        "book/chapter",
        "inherit_parent = false\n[variables]\nchapter = '03'\n",
    );
    let vars = storage.folder_template_variables("book/chapter").unwrap();
    assert_eq!(vars.len(), 1);
    assert_eq!(vars["chapter"], "03");
}

#[test]
fn config_validation_rejects_bad_names_types_and_reserved_builtins() {
    assert!(FolderTemplateConfig::parse("").unwrap().inherit_parent);
    for name in [
        "date", "datetime", "time", "weekday", "year", "month", "day",
    ] {
        let err = FolderTemplateConfig::parse(&format!("[variables]\n{name} = 'override'"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("reserved"), "{err}");
    }
    for content in [
        "inherit_parent = 'yes'",
        "[variables]\nchapter = 4",
        "[variables]\nchapter = ['04']",
        "[variables]\n'bad-name' = 'x'",
        "[variables]\n'1chapter' = 'x'",
        "[variables]\n'' = 'x'",
        "[variables]\n'章节' = 'x'",
        "inherit_parents = true",
    ] {
        assert!(FolderTemplateConfig::parse(content).is_err(), "{content}");
    }
    assert!(FolderTemplateConfig::parse("[variables]\n_chapter2 = '日本語'\n").is_ok());
}

#[test]
fn malformed_config_reports_source_path() {
    let dir = tempfile::tempdir().unwrap();
    let storage = storage(dir.path());
    config(dir.path(), "book", "[variables]\nchapter = 4");
    let err = format!(
        "{:#}",
        storage.folder_template_variables("book").unwrap_err()
    );
    assert!(err.contains("book"));
    assert!(err.contains(FOLDER_TEMPLATE_CONFIG));
    assert!(err.contains("string"));
}

#[test]
fn no_config_preserves_existing_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let storage = storage(dir.path());
    assert!(storage.folder_template_variables("").unwrap().is_empty());
    let rendered = storage.render_template_in_folder(&template(), "").unwrap();
    assert_eq!(rendered.title.as_deref(), Some("{book} — {chapter}"));
    assert!(!rendered.content.contains("{date}"));
    assert_eq!(rendered.missing_variables, ["book", "chapter"]);
}

#[test]
fn substitution_is_single_pass_unicode_safe_and_deduplicates_warnings() {
    let mut vars = BTreeMap::new();
    vars.insert("book".into(), "日本語 {chapter} {date}".into());
    // Even direct renderer callers cannot replace reserved built-ins.
    vars.insert("date".into(), "OVERRIDE".into());
    let template = Template {
        name: "Test".into(),
        title: TitleConfig {
            template: Some("{book} {missing}".into()),
        },
        content: ContentConfig {
            template: "{book} {missing} {other} {date} {{book}} { not_a_token } {未定} {".into(),
        },
    };
    let rendered = template.render_with_variables(&vars);
    assert_eq!(
        rendered.title.as_deref(),
        Some("日本語 {chapter} {date} {missing}")
    );
    assert!(
        rendered
            .content
            .starts_with("日本語 {chapter} {date} {missing} {other} ")
    );
    assert!(rendered.content.contains("{日本語 {chapter} {date}}"));
    assert!(rendered.content.ends_with("{ not_a_token } {未定} {"));
    assert!(!rendered.content.contains("OVERRIDE"));
    assert_eq!(rendered.missing_variables, ["missing", "other"]);
    assert_eq!(
        rendered.warning().unwrap(),
        "Unknown template variables (left literal): missing, other"
    );
}

#[test]
fn folder_paths_must_stay_inside_vault() {
    let dir = tempfile::tempdir().unwrap();
    let storage = storage(dir.path());
    for folder in [
        "../outside",
        "book/../../outside",
        "/tmp/outside",
        "book\\chapter",
    ] {
        assert!(storage.template_folder_path(folder).is_err(), "{folder}");
    }
    fs::write(dir.path().join("file"), "not a folder").unwrap();
    assert!(storage.template_folder_path("file/subfolder").is_err());
    assert_eq!(
        storage.template_folder_path("./new/chapter").unwrap(),
        fs::canonicalize(dir.path()).unwrap().join("new/chapter")
    );
}

#[cfg(unix)]
#[test]
fn symlink_escapes_and_config_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let storage = storage(dir.path());
    symlink(outside.path(), dir.path().join("escape")).unwrap();
    assert!(storage.folder_template_variables("escape/new").is_err());
    fs::write(
        outside.path().join("config.toml"),
        "[variables]\nbook = 'Outside'",
    )
    .unwrap();
    symlink(
        outside.path().join("config.toml"),
        dir.path().join(FOLDER_TEMPLATE_CONFIG),
    )
    .unwrap();
    assert!(storage.folder_template_variables("").is_err());
    fs::remove_file(dir.path().join(FOLDER_TEMPLATE_CONFIG)).unwrap();
    fs::create_dir_all(dir.path().join("real")).unwrap();
    config(dir.path(), "real", "[variables]\nbook = 'Inside'");
    symlink(dir.path().join("real"), dir.path().join("alias")).unwrap();
    assert_eq!(
        storage.folder_template_variables("alias").unwrap()["book"],
        "Inside"
    );
}

#[test]
fn configs_never_appear_as_notes_even_with_all_files_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let storage = storage(dir.path());
    config(dir.path(), "", "[variables]\nbook = 'Root'");
    config(dir.path(), "book", "[variables]\nchapter = '04'");
    fs::write(dir.path().join("book/note.md"), "Note").unwrap();
    let scan = storage.scan_vault(true, true).unwrap();
    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].id, "book/note.md");
    assert_eq!(storage.list_note_ids(true, true).unwrap(), ["book/note.md"]);
    assert!(
        storage
            .load_note_summary("book/.clin-template.toml")
            .is_err()
    );
}

pub(super) fn app(root: &Path) -> App {
    crate::config::set_config_path_override(root.join(".clin/config.toml"));
    fs::create_dir_all(root.join(".clin/templates")).unwrap();
    let mut app = App::new(storage(root)).unwrap();
    app.editor.external_editor_enabled = false;
    app.config.features.templates = FeatureState::Enabled;
    app.config.features.backup = FeatureState::Disabled;
    app.mode = ViewMode::List;
    app.list.notes_layout = NotesLayout::Grid;
    app
}

#[test]
fn tui_creation_renders_folder_variables_and_preserves_explicit_title() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    config(dir.path(), "", "[variables]\nbook = 'Rust'");
    config(dir.path(), "book/04", "[variables]\nchapter = '04'");
    let mut app = app(dir.path());
    app.start_note_from_template(&template(), "book/04".into());
    assert_eq!(
        crate::events::get_title_text(&app.editor.title_editor),
        "Rust — 04"
    );
    assert!(
        app.editor
            .body
            .lines()
            .join("\n")
            .starts_with("Rust: 04 / ")
    );
    assert!(
        app.editor
            .editing_id
            .as_ref()
            .unwrap()
            .starts_with("book/04/")
    );
    let mut template = template();
    template.title.template = Some("{unused}".into());
    app.messages.messages.clear();
    app.start_note_from_template_with_title(&template, "book/04".into(), "Literal {book}".into());
    assert_eq!(
        crate::events::get_title_text(&app.editor.title_editor),
        "Literal {book}"
    );
    assert!(
        !app.messages
            .messages
            .iter()
            .any(|m| m.text.contains("unused"))
    );
}

#[test]
fn tui_invalid_config_prevents_creation_and_missing_tokens_warn() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.start_note_from_template(&template(), "book".into());
    assert!(
        app.messages
            .messages
            .iter()
            .any(|m| m.text.contains("Unknown template variables"))
    );
    app.back_to_list(None, None);
    config(dir.path(), "book", "[variables]\nchapter = 4");
    app.start_note_from_template(&template(), "book".into());
    assert_eq!(app.mode, ViewMode::List);
    assert!(app.editor.editing_id.is_none());
    assert!(
        app.messages
            .messages
            .iter()
            .any(|m| m.text.contains("Failed to render template"))
    );
    assert!(
        app.storage
            .scan_vault(false, false)
            .unwrap()
            .files
            .is_empty()
    );
}

#[test]
fn folder_config_editor_validates_save_and_returns_to_list_not_picker() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.list.grid_folder = "book".into();
    crate::actions::execute_action("templates.edit_folder_variables", &mut app, None).unwrap();
    let path = app.storage.folder_template_config_path("book").unwrap();
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(app.editor.template_edit_path.as_ref(), Some(&path));
    let before = fs::read_to_string(&path).unwrap();
    app.editor.body = EditorDocument::from_text("[variables]\ndate = 'invalid'");
    assert!(app.autosave().is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
    app.editor.body =
        EditorDocument::from_text("inherit_parent = false\n[variables]\nchapter = '04'");
    app.autosave().unwrap();
    assert_eq!(
        app.storage.folder_template_variables("book").unwrap()["chapter"],
        "04"
    );
    assert!(!app.storage.templates_dir.join("04.toml").exists());
    app.back_to_list(None, None);
    assert_eq!(app.mode, ViewMode::List);
    assert!(app.popups.active.is_none());
    app.edit_folder_template_variables();
    assert!(
        app.editor
            .body
            .lines()
            .join("\n")
            .contains("chapter = '04'")
    );
}

#[test]
fn folder_config_action_rejects_virtual_folders_and_disabled_templates() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.list.grid_folder = "@today".into();
    app.edit_folder_template_variables();
    assert_eq!(app.mode, ViewMode::List);
    assert!(!dir.path().join(FOLDER_TEMPLATE_CONFIG).exists());
    app.list.notes_layout = NotesLayout::Tree;
    app.list.visual_list = vec![crate::list_view::VisualItem::CreateNew {
        path: "@today".into(),
        depth: 0,
    }];
    app.list.visual_index = 0;
    app.edit_folder_template_variables();
    assert_eq!(app.mode, ViewMode::List);
    assert!(!dir.path().join(FOLDER_TEMPLATE_CONFIG).exists());
    app.list.grid_folder.clear();
    app.config.features.templates = FeatureState::Disabled;
    assert!(
        crate::actions::execute_action("templates.edit_folder_variables", &mut app, None).is_err()
    );
}
