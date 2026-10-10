use crate::app::{App, EditFocus, ViewMode};
use crate::editor::AutosaveStatus;
use crate::frontmatter::{self, FrontmatterEdit};
use crate::storage::Storage;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui_textarea::{CursorMove, TextArea};

fn app() -> (tempfile::TempDir, App) {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("vault");
    let config_dir = dir.path().join("config");
    let notes_dir = data_dir.join("notes");
    let templates_dir = data_dir.join("templates");
    for path in [&data_dir, &config_dir, &notes_dir, &templates_dir] {
        std::fs::create_dir_all(path).unwrap();
    }
    let storage = Storage {
        data_dir,
        config_dir,
        notes_dir,
        templates_dir,
        key: [42; 32],
        skip_dir_patterns: vec![],
        rename_on_title_change: false,
    };
    let mut app = App::new(storage).unwrap();
    app.editor.external_editor_enabled = false;
    (dir, app)
}

fn open(app: &mut App, text: &str) {
    std::fs::write(app.storage.note_path("CL-01.md"), text).unwrap();
    app.load_and_open_note("CL-01.md", None);
    assert_eq!(app.mode, ViewMode::Edit);
}

fn set_yaml(app: &mut App, text: &str) {
    app.editor.frontmatter_editor = TextArea::from(text.lines());
}

fn key(app: &mut App, focus: &mut EditFocus, code: KeyCode, modifiers: KeyModifiers) {
    crate::handle_edit_keys(app, KeyEvent::new(code, modifiers), focus);
}

#[test]
fn frontmatter_strict_yaml_and_raw_framing() {
    for bad in [
        "- list",
        "scalar",
        "status: [",
        "status: open\nstatus: closed",
        "tags: false",
        "pinned: nope",
    ] {
        assert!(frontmatter::parse_yaml(bad).is_err(), "{bad}");
    }
    let (header, body) = frontmatter::split_raw("---\r\nid: CL-01\r\n---\r\nBody\r\n");
    assert_eq!(header, Some("id: CL-01"));
    assert_eq!(body, "Body\r\n");
    assert_eq!(frontmatter::split_raw("No header"), (None, "No header"));
    assert_eq!(
        frontmatter::split_raw("---\nstatus: ["),
        (None, "---\nstatus: [")
    );
    let fm = frontmatter::parse_yaml(
        "id: '001'\nflag: true\ncount: 3\nnothing: null\nlist: [one, two]\nnested: {x: 2}",
    )
    .unwrap();
    assert_eq!(fm.extra["id"].as_str(), Some("001"));
    assert!(fm.extra["nested"].is_mapping());
}

#[test]
fn frontmatter_toggle_focus_and_paste_stay_separate_from_body() {
    let (_dir, mut app) = app();
    open(&mut app, "Body");
    let mut focus = EditFocus::Body;
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    assert_eq!(focus, EditFocus::Frontmatter);
    assert!(app.editor.frontmatter_visible);
    assert!(!app.editor.frontmatter_changed());
    assert!(crate::events::handle_bracketed_paste(
        &mut app,
        "status: ouvert\nname: café".into(),
        &mut focus
    ));
    assert_eq!(app.editor.body.lines(), &["Body"]);
    let text = app.editor.frontmatter_text();
    let cursor = app.editor.frontmatter_editor.cursor();
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    assert_eq!(focus, EditFocus::Body);
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    assert_eq!(app.editor.frontmatter_text(), text);
    assert_eq!(app.editor.frontmatter_editor.cursor(), cursor);
    key(
        &mut app,
        &mut focus,
        KeyCode::Char('t'),
        KeyModifiers::CONTROL,
    );
    assert_eq!(focus, EditFocus::Body);
    app.autosave().unwrap();
    assert_eq!(app.storage.load_note("CL-01.md").unwrap().content, "Body");
    let (fm, _) =
        frontmatter::parse(&std::fs::read_to_string(app.storage.note_path("CL-01.md")).unwrap());
    assert_eq!(fm.extra["status"].as_str(), Some("ouvert"));
    assert_eq!(fm.extra["name"].as_str(), Some("café"));
}

#[test]
fn frontmatter_add_change_remove_and_managed_editable_fields() {
    let (_dir, mut app) = app();
    open(
        &mut app,
        "---\nid: CL-01\ntitle: Claim\nstatus: open\nconfidence: high\n---\n# Body",
    );
    set_yaml(
        &mut app,
        "id: CL-01\ntitle: Answered\nstatus: answered\ntags: [research]\npinned: true\ntext_align: center\nsources: [one, two]\nnested: {score: 3}",
    );
    app.autosave().unwrap();
    let text = std::fs::read_to_string(app.storage.note_path("CL-01.md")).unwrap();
    let (fm, body) = frontmatter::parse(&text);
    assert_eq!(body, "# Body");
    assert_eq!(fm.title.as_deref(), Some("Answered"));
    assert_eq!(fm.tags, ["research"]);
    assert!(fm.pinned);
    assert!(!fm.extra.contains_key("confidence"));
    assert_eq!(fm.extra["status"].as_str(), Some("answered"));
    assert!(fm.extra["sources"].is_sequence());
    assert_eq!(app.editor.text_align, crate::config::TextAlignment::Center);
    assert!(!app.editor.frontmatter_changed());
    app.load_and_open_note("CL-01.md", None);
    assert!(!app.editor.frontmatter_visible);
    assert!(!app.editor.frontmatter_text().contains("confidence"));
    set_yaml(&mut app, "");
    app.autosave().unwrap();
    let (fm, _) =
        frontmatter::parse(&std::fs::read_to_string(app.storage.note_path("CL-01.md")).unwrap());
    assert!(fm.extra.is_empty());
    assert_eq!(fm.tags, Vec::<String>::new());
    assert!(!fm.pinned);
}

#[test]
fn frontmatter_invalid_yaml_blocks_save_exit_switch_and_success_status() {
    let (_dir, mut app) = app();
    let original = "---\nstatus: open\n---\nBody";
    open(&mut app, original);
    set_yaml(&mut app, "status: [");
    app.editor.autosave_status = AutosaveStatus::Unsaved;
    app.editor.autosave_timer = Some(std::time::Instant::now());
    app.write_draft();
    app.tick_autosave();
    assert_eq!(app.editor.autosave_status, AutosaveStatus::Unsaved);
    assert!(app.editor.autosave_timer.is_none());
    let mut focus = EditFocus::Frontmatter;
    key(&mut app, &mut focus, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(
        std::fs::read_to_string(app.storage.note_path("CL-01.md")).unwrap(),
        original
    );
    std::fs::write(app.storage.note_path("other.md"), "Other").unwrap();
    app.open_note_at_line("other.md", None);
    assert_eq!(app.editor.editing_id.as_deref(), Some("CL-01.md"));
    assert!(app.storage.editor_draft_path().exists());
    app.storage.recover_editor_draft().unwrap();
    assert!(app.storage.editor_draft_path().exists());
    app.load_and_open_note("CL-01.md", None);
    assert_eq!(app.editor.frontmatter_text(), "status: [");
    assert!(app.editor.frontmatter_visible);
    set_yaml(&mut app, "status: fixed");
    app.autosave().unwrap();
    assert!(!app.storage.editor_draft_path().exists());
}

#[test]
fn frontmatter_external_header_conflict_keeps_disk_and_draft() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: open\n---\nBody");
    set_yaml(&mut app, "status: local");
    app.write_draft();
    let external = "---\nstatus: external\n---\nExternal body";
    std::fs::write(app.storage.note_path("CL-01.md"), external).unwrap();
    assert!(app.autosave().unwrap_err().contains("changed on disk"));
    assert_eq!(
        std::fs::read_to_string(app.storage.note_path("CL-01.md")).unwrap(),
        external
    );
    assert!(app.storage.editor_draft_path().exists());
}

#[test]
fn frontmatter_txt_notes_and_specialized_editors() {
    let (_dir, mut app) = app();
    std::fs::write(app.storage.note_path("plain.txt"), "Plain body").unwrap();
    app.load_and_open_note("plain.txt", None);
    let mut focus = EditFocus::Body;
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    assert_eq!(focus, EditFocus::Frontmatter);
    set_yaml(&mut app, "status: tracked");
    app.autosave().unwrap();
    assert_eq!(app.editor.editing_id.as_deref(), Some("plain.txt"));
    assert_eq!(
        app.storage.load_note("plain.txt").unwrap().content,
        "Plain body"
    );
    for id in ["board.canvas", "drawing.draw", "secret.clin"] {
        app.editor.editing_id = Some(id.into());
        app.editor.frontmatter_visible = false;
        focus = EditFocus::Body;
        key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
        assert!(!app.editor.frontmatter_visible);
        assert_eq!(focus, EditFocus::Body);
    }
    app.editor.editing_id = None;
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    assert!(!app.editor.frontmatter_visible);
}

#[test]
fn frontmatter_select_all_replaces_a_recovered_single_line() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: [\n---\nBody");
    let mut focus = EditFocus::Frontmatter;
    key(
        &mut app,
        &mut focus,
        KeyCode::Char('a'),
        KeyModifiers::CONTROL,
    );
    assert!(app.editor.frontmatter_editor.selection_range().is_some());
    crate::events::handle_bracketed_paste(
        &mut app,
        "status: fixed\nlist: [one, two]".into(),
        &mut focus,
    );
    assert_eq!(
        app.editor.frontmatter_text(),
        "status: fixed\nlist: [one, two]"
    );
}

#[test]
fn frontmatter_title_conflicts_are_explicit() {
    let (_dir, mut app) = app();
    open(&mut app, "---\ntitle: Original\n---\nBody");
    set_yaml(&mut app, "title: YAML title");
    app.editor.title_editor = TextArea::from(["Header title"]);
    assert!(app.autosave().unwrap_err().contains("both fields"));
    app.editor.title_editor = TextArea::from(["YAML title"]);
    app.autosave().unwrap();
    assert_eq!(
        app.storage.load_note("CL-01.md").unwrap().title,
        "YAML title"
    );
}

#[test]
fn frontmatter_title_control_updates_yaml_and_respects_rename_policy() {
    let (_dir, mut app) = app();
    open(&mut app, "---\ntitle: Original\nstatus: open\n---\nBody");
    app.storage.rename_on_title_change = true;
    app.editor.title_editor = TextArea::from(["Renamed"]);
    app.autosave().unwrap();
    let id = app.editor.editing_id.as_deref().unwrap();
    assert_ne!(id, "CL-01.md");
    assert!(!app.storage.note_path("CL-01.md").exists());
    let fm = frontmatter::parse_yaml(&app.editor.frontmatter_text()).unwrap();
    assert_eq!(fm.title.as_deref(), Some("Renamed"));
    assert_eq!(fm.extra["status"].as_str(), Some("open"));
    assert!(!app.editor.frontmatter_changed());
    assert_eq!(app.storage.load_note(id).unwrap().content, "Body");
}

#[test]
fn frontmatter_unclosed_leading_rule_stays_in_body() {
    let (_dir, mut app) = app();
    let body = "---\n# Markdown body";
    open(&mut app, body);
    assert_eq!(app.editor.frontmatter_text(), "");
    assert_eq!(app.editor.body.lines().join("\n"), body);
    app.autosave().unwrap();
    assert_eq!(app.storage.load_note("CL-01.md").unwrap().content, body);
}

#[test]
fn frontmatter_malformed_disk_header_is_repairable_without_body_leak() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: [\n---\nActual body");
    assert_eq!(app.editor.body.lines(), &["Actual body"]);
    assert_eq!(app.editor.frontmatter_text(), "status: [");
    assert!(app.autosave().is_err());
    set_yaml(&mut app, "status: fixed");
    app.autosave().unwrap();
    assert_eq!(
        app.storage.load_note("CL-01.md").unwrap().content,
        "Actual body"
    );
}

#[test]
fn frontmatter_drafts_recover_new_and_legacy_formats() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: open\n---\nBody");
    let edit = FrontmatterEdit {
        text: "status: answered".into(),
        original: app.editor.frontmatter_original.clone(),
        saved_text: app.editor.frontmatter_saved.clone(),
        saved_title: app.editor.frontmatter_saved_title.clone(),
    };
    app.storage
        .write_editor_draft_with_frontmatter("CL-01.md", "Claim", "Recovered body", Some(edit))
        .unwrap();
    app.storage.recover_editor_draft().unwrap();
    let text = std::fs::read_to_string(app.storage.note_path("CL-01.md")).unwrap();
    let (fm, body) = frontmatter::parse(&text);
    assert_eq!(fm.extra["status"].as_str(), Some("answered"));
    assert_eq!(body, "Recovered body");
    let old = (
        "CL-01.md".to_string(),
        "Legacy".to_string(),
        "Legacy body".to_string(),
    );
    let encoded = bincode::serde::encode_to_vec(&old, bincode::config::standard()).unwrap();
    let encrypted = app.storage.encrypt(&encoded).unwrap();
    std::fs::write(app.storage.editor_draft_path(), encrypted).unwrap();
    app.storage.recover_editor_draft().unwrap();
    assert_eq!(
        app.storage.load_note("CL-01.md").unwrap().content,
        "Legacy body"
    );
}

#[test]
fn frontmatter_conflicting_title_draft_requires_correction() {
    let (_dir, mut app) = app();
    open(&mut app, "---\ntitle: Original\n---\nBody");
    set_yaml(&mut app, "title: YAML title");
    app.editor.title_editor = TextArea::from(["Control title"]);
    app.write_draft();
    app.storage.recover_editor_draft().unwrap();
    assert_eq!(app.storage.load_note("CL-01.md").unwrap().title, "Original");
    assert!(app.storage.editor_draft_path().exists());
    assert!(app.restore_pending_editor_draft());
    assert!(app.autosave().unwrap_err().contains("both fields"));
    app.editor.title_editor = TextArea::from(["YAML title"]);
    app.autosave().unwrap();
    assert_eq!(
        app.storage.load_note("CL-01.md").unwrap().title,
        "YAML title"
    );
}

#[test]
fn frontmatter_invalid_new_note_draft_is_restored_without_a_disk_note() {
    let (_dir, mut app) = app();
    app.start_blank_note_with_title(String::new(), "New draft".into());
    let original_id = app.editor.editing_id.clone().unwrap();
    assert!(!app.storage.note_path(&original_id).exists());
    let mut focus = EditFocus::Body;
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    assert_eq!(focus, EditFocus::Frontmatter);
    set_yaml(&mut app, "status: [");
    app.editor.body = crate::editor_document::EditorDocument::from_text("Unsaved body");
    app.write_draft();
    app.editor = crate::editor::NoteEditor::default();
    app.mode = ViewMode::List;
    app.storage.recover_editor_draft().unwrap();
    assert!(app.restore_pending_editor_draft());
    assert_eq!(app.mode, ViewMode::Edit);
    assert!(app.editor.frontmatter_visible);
    assert_eq!(app.editor.frontmatter_text(), "status: [");
    assert_eq!(app.editor.body.lines(), &["Unsaved body"]);
    set_yaml(&mut app, "status: recovered");
    app.autosave().unwrap();
    let id = app.editor.editing_id.as_deref().unwrap();
    assert_eq!(app.storage.load_note(id).unwrap().content, "Unsaved body");
    assert!(!app.storage.editor_draft_path().exists());
}

#[test]
fn frontmatter_draft_title_uses_editor_baseline_not_generated_disk_fields() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: open\n---\nBody");
    set_yaml(&mut app, "status: answered");
    app.autosave().unwrap();
    let title = app.editor.frontmatter_saved_title.clone();
    assert!(!app.editor.frontmatter_saved.contains("title:"));
    app.editor.body = crate::editor_document::EditorDocument::from_text("Recovered body");
    app.write_draft();
    app.storage.recover_editor_draft().unwrap();
    let note = app.storage.load_note("CL-01.md").unwrap();
    assert_eq!(note.title, title);
    assert_eq!(note.content, "Recovered body");
}

#[test]
fn frontmatter_write_failure_retains_buffer_and_draft() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: open\n---\nBody");
    set_yaml(&mut app, "status: edited");
    app.write_draft();
    let path = app.storage.note_path("CL-01.md");
    let backup = path.with_extension("backup");
    std::fs::rename(&path, &backup).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(app.autosave().is_err());
    assert_eq!(app.editor.frontmatter_text(), "status: edited");
    assert!(app.storage.editor_draft_path().exists());
    assert_eq!(
        std::fs::read_to_string(backup).unwrap(),
        "---\nstatus: open\n---\nBody"
    );
}

#[test]
fn frontmatter_new_note_resets_previous_state_and_preserves_body_editing() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: open\n---\nBody");
    set_yaml(&mut app, "status: answered");
    app.editor.frontmatter_visible = true;
    app.start_blank_note_with_title(String::new(), "Next note".into());
    assert!(!app.editor.frontmatter_visible);
    assert_eq!(app.editor.frontmatter_text(), "");
    assert!(app.editor.frontmatter_original.is_none());
    let mut focus = EditFocus::Body;
    crate::events::handle_bracketed_paste(&mut app, "New body".into(), &mut focus);
    app.autosave().unwrap();
    let id = app.editor.editing_id.as_deref().unwrap();
    assert_eq!(app.storage.load_note(id).unwrap().content, "New body");
    let (fm, _) = frontmatter::parse(&std::fs::read_to_string(app.storage.note_path(id)).unwrap());
    assert!(fm.extra.is_empty());
}

#[test]
fn frontmatter_alignment_uses_single_header_and_same_save_path() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: open\n---\nBody");
    app.config.editor.soft_wrap = true;
    app.apply_editor_prefs();
    app.cycle_text_alignment();
    let text = std::fs::read_to_string(app.storage.note_path("CL-01.md")).unwrap();
    let (fm, body) = frontmatter::parse(&text);
    assert_eq!(body, "Body");
    assert_eq!(fm.text_align, Some(crate::config::TextAlignment::Center));
    assert_eq!(fm.extra["status"].as_str(), Some("open"));
    set_yaml(&mut app, "status: answered\ntext_align: right");
    app.autosave().unwrap();
    assert_eq!(app.editor.text_align, crate::config::TextAlignment::Right);
}

#[test]
fn frontmatter_layout_mouse_focus_and_undo() {
    let (_dir, mut app) = app();
    open(&mut app, "---\nstatus: open\n---\nBody");
    let mut focus = EditFocus::Body;
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    for (w, h) in [(100, 30), (40, 10), (10, 4), (10, 3)] {
        let mut terminal = ratatui::Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
            .unwrap();
        assert!(app.editor.frontmatter_rect.bottom() <= h);
        assert!(app.editor.frontmatter_rect.height > 0);
    }
    for sidebar in [
        crate::editor::EditSidebar::None,
        crate::editor::EditSidebar::Outline,
        crate::editor::EditSidebar::Links,
    ] {
        for preview in [false, true] {
            for zen in [false, true] {
                app.editor.sidebar = sidebar;
                app.editor.editor_preview_enabled = preview;
                app.zen_mode = zen;
                let mut terminal = ratatui::Terminal::new(TestBackend::new(80, 24)).unwrap();
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
                    .unwrap();
                assert!(app.editor.frontmatter_rect.height > 0);
                assert_eq!(app.editor.body.lines(), &["Body"]);
            }
        }
    }
    app.editor.sidebar = crate::editor::EditSidebar::None;
    app.editor.editor_preview_enabled = false;
    app.zen_mode = false;
    let mut terminal = ratatui::Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
        .unwrap();
    let rect = app.editor.frontmatter_rect;
    let mut selection = crate::text_edit::MouseTextSelection::default();
    focus = EditFocus::Body;
    crate::handle_edit_mouse(
        &mut app,
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: rect.x + 1,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        },
        ratatui::layout::Rect::new(0, 0, 100, 30),
        &mut focus,
        &mut selection,
    );
    assert_eq!(focus, EditFocus::Frontmatter);
    app.editor.frontmatter_editor.cancel_selection();
    app.editor.frontmatter_editor.move_cursor(CursorMove::End);
    let before = app.editor.frontmatter_text();
    app.editor.frontmatter_editor.insert_str("X");
    assert!(app.editor.frontmatter_editor.undo());
    assert_eq!(app.editor.frontmatter_text(), before);
    assert!(app.editor.frontmatter_editor.redo());
    assert_eq!(app.editor.frontmatter_text(), format!("{before}X"));
    assert_eq!(app.editor.body.lines(), &["Body"]);
}
