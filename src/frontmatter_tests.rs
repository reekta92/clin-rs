//! #181 regressions adapted from PR #195 (d277db9) for guarded Bases storage.
use crate::{
    app::{App, EditFocus, ViewMode},
    config::{ConfigTestGuard, FrontmatterMode},
    frontmatter::{
        TimestampFormat,
        properties::{PropertyDocument, PropertyFrame, parse_mapping},
    },
    storage::Storage,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use serde_yaml_ng::Value;
use std::{fs, sync::atomic::AtomicBool};

fn fixture() -> (ConfigTestGuard, tempfile::TempDir, App) {
    let guard = ConfigTestGuard::lock();
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.toml");
    fs::write(&config, "").unwrap();
    crate::config::set_config_path_override(config);
    let storage = Storage {
        data_dir: root.path().join("vault"),
        notes_dir: root.path().join("vault"),
        config_dir: root.path().join("config"),
        templates_dir: root.path().join("templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: crate::frontmatter::WritePolicy::default(),
    };
    for directory in [
        &storage.data_dir,
        &storage.config_dir,
        &storage.templates_dir,
    ] {
        fs::create_dir_all(directory).unwrap();
    }
    let app = App::new(storage).unwrap();
    (guard, root, app)
}
fn open(app: &mut App, source: &str) {
    app.mode = ViewMode::List;
    fs::write(app.storage.notes_dir.join("CL-01.md"), source).unwrap();
    app.load_and_open_note("CL-01.md", None);
    assert_eq!(app.mode, ViewMode::Edit);
}
fn yaml(app: &mut App, source: &str) {
    app.editor.ensure_frontmatter().unwrap();
    let state = app.editor.frontmatter.as_mut().unwrap();
    state.raw.select_all();
    state.raw.insert_str(source);
}
fn disk(app: &App) -> String {
    fs::read_to_string(app.storage.notes_dir.join("CL-01.md")).unwrap()
}
fn key(app: &mut App, focus: &mut EditFocus, code: KeyCode, modifiers: KeyModifiers) {
    crate::events::handle_edit_keys(app, KeyEvent::new(code, modifiers), focus);
}

#[test]
fn list_rename_changes_filename_and_preserves_note_source_and_subnotes() {
    let (_guard, _root, mut app) = fixture();
    app.keybinds = crate::keybinds::Keybinds::default();
    app.list.grid_folder.clear();
    for ext in ["md", "txt"] {
        for pinned in [false, true] {
            let id = format!("stable.{ext}");
            let source = format!(
                "---\r\n# keep header\r\ntitle: 'Old title' # keep comment\r\npinned: {pinned}\r\ncustom: [a, b]\r\n---\r\nbody [[stable]]\r\n"
            );
            fs::write(app.storage.notes_dir.join(&id), &source).unwrap();
            app.storage.frontmatter_policy.title = false;
            let subnotes = vec![crate::storage::SubNote {
                id: "child".into(),
                title: "Attached".into(),
                content: "Keep me".into(),
                updated_at: 1,
            }];
            app.storage.set_subnotes(&id, &subnotes).unwrap();
            app.notes = vec![app.storage.load_note_summary(&id).unwrap()];
            app.refresh_visual_list();
            app.list.visual_index = app
                .list
                .visual_list
                .iter()
                .position(|item| matches!(item, crate::list_view::VisualItem::Note { .. }))
                .unwrap();
            crate::events::handle_list_keys(
                &mut app,
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            );
            let Some(crate::popups::ActivePopup::NoteRename(popup)) = &mut app.popups.active else {
                panic!("r should open the note rename popup");
            };
            assert_eq!(popup.input.lines().join(""), "stable");
            let name = format!("New name {pinned}");
            popup.input.select_all();
            popup.input.insert_str(&name);
            assert!(crate::events::handle_global_popups_and_palette(
                &mut app,
                crossterm::event::Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                ratatui::layout::Rect::default(),
            ));
            let renamed = format!("{name}.{ext}");
            assert_eq!(
                fs::read_to_string(app.storage.note_path(&renamed)).unwrap(),
                source
            );
            assert!(!app.storage.note_path(&id).exists());
            assert_eq!(app.storage.get_subnotes(&renamed).unwrap(), subnotes);
            assert_eq!(
                app.storage.get_subnotes(&id).unwrap(),
                Vec::<crate::storage::SubNote>::new()
            );
            assert_eq!(app.notes.len(), 1);
            assert_eq!(app.notes[0].id, renamed);
            assert_eq!(app.notes[0].title, "Old title");
            assert_eq!(app.notes[0].pinned, pinned);
        }
    }
}

#[test]
fn list_rename_other_file_types_preserves_source_bytes() {
    let (_guard, _root, mut app) = fixture();
    for (ext, source) in [
        ("png", &b"\x89PNG\r\n\x1a\n\0\xff"[..]),
        ("base", b"views: [{type: table}]\n"),
        ("canvas", b"{\"nodes\":[],\"edges\":[]}\n"),
        ("draw", b"{\"name\":\"Keep data\"}\n"),
    ] {
        let id = format!("original.{ext}");
        fs::write(app.storage.notes_dir.join(&id), source).unwrap();
        let mut input = crate::ui::make_popup_textarea(&app.app_theme, "");
        input.insert_str("Renamed");
        app.popups.active = Some(crate::popups::ActivePopup::NoteRename(
            crate::popups::NoteRenamePopup {
                note_id: id.clone(),
                input,
            },
        ));
        app.confirm_rename_note();
        let renamed = format!("Renamed.{ext}");
        assert_eq!(
            fs::read(app.storage.notes_dir.join(&renamed)).unwrap(),
            source
        );
        assert!(!app.storage.notes_dir.join(&id).exists());
        assert!(
            app.notes
                .iter()
                .any(|note| note.id == renamed && note.title == "Renamed")
        );
    }
}

#[test]
fn frontmatter_modes_defaults_validation_and_retired_config_cleanup() {
    let (_guard, root, mut app) = fixture();
    assert_eq!(
        app.config.editor.frontmatter_mode,
        FrontmatterMode::Properties
    );
    for mode in ["properties", "raw_yaml"] {
        let config = format!(
            "[editor]\nfrontmatter_mode = '{mode}'\n[notes]\nrename_on_title_change = true\n[notes.frontmatter]\nlinks = false # preserve policy\noriginal_ext = false\nupdated_at = 'iso8601'\n"
        );
        fs::write(root.path().join("config.toml"), config).unwrap();
        app.reload_config();
        assert!(!app.storage.frontmatter_policy.links);
        assert_eq!(
            app.storage.frontmatter_policy.updated_at,
            TimestampFormat::Iso8601
        );
        let cleaned = fs::read_to_string(root.path().join("config.toml")).unwrap();
        assert!(!cleaned.contains("rename_on_title_change"));
        assert!(cleaned.contains("# preserve policy"));
    }
    assert!(
        toml::from_str::<crate::config::ClinConfig>("[editor]\nfrontmatter_mode = 'sideways'")
            .is_err()
    );
    assert!(
        toml::from_str::<crate::config::ClinConfig>("[notes.frontmatter]\nupdated_at = 'local'")
            .is_err()
    );
}

#[test]
fn frontmatter_f7_focus_paste_hide_reopen_undo_and_new_note_reset() {
    let (_guard, _root, mut app) = fixture();
    for mode in [FrontmatterMode::Properties, FrontmatterMode::RawYaml] {
        open(&mut app, "---\nstatus: open\n---\n# Body\n");
        app.config.editor.frontmatter_mode = mode;
        assert!(!app.editor.frontmatter.as_ref().unwrap().visible);
        let mut focus = EditFocus::Body;
        key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
        assert_eq!(focus, EditFocus::Frontmatter);
        if mode == FrontmatterMode::RawYaml {
            key(
                &mut app,
                &mut focus,
                KeyCode::Char('a'),
                KeyModifiers::CONTROL,
            );
            assert!(crate::events::handle_bracketed_paste(
                &mut app,
                "status: answered\nwords: 'not Body'\n".into(),
                &mut focus
            ));
        } else {
            key(&mut app, &mut focus, KeyCode::Char('a'), KeyModifiers::NONE);
            crate::events::handle_bracketed_paste(&mut app, "score".into(), &mut focus);
            key(&mut app, &mut focus, KeyCode::Tab, KeyModifiers::NONE);
            key(
                &mut app,
                &mut focus,
                KeyCode::Char('a'),
                KeyModifiers::CONTROL,
            );
            crate::events::handle_bracketed_paste(&mut app, "42".into(), &mut focus);
            key(&mut app, &mut focus, KeyCode::Enter, KeyModifiers::CONTROL);
            assert_eq!(
                parse_mapping(&app.editor.frontmatter.as_ref().unwrap().yaml())
                    .unwrap()
                    .get("score")
                    .and_then(Value::as_i64),
                Some(42)
            );
        }
        let pending = app.editor.frontmatter.as_ref().unwrap().yaml();
        key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
        assert_eq!(focus, EditFocus::Body);
        key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
        assert_eq!(app.editor.frontmatter.as_ref().unwrap().yaml(), pending);
        key(
            &mut app,
            &mut focus,
            KeyCode::Char('z'),
            KeyModifiers::CONTROL,
        );
        assert_ne!(app.editor.frontmatter.as_ref().unwrap().yaml(), pending);
        assert_eq!(app.editor.body.text(), "# Body\n");
        assert_eq!(crate::goals::count_words(&app.editor.body.text()), 2);
        app.rebuild_outline();
        assert_eq!(app.editor.outline_nodes.len(), 1);
        app.autosave().unwrap();
        app.start_blank_note_with_title(String::new(), "New note".into());
        assert!(!app.editor.frontmatter.as_ref().unwrap().visible);
        assert_eq!(app.editor.frontmatter.as_ref().unwrap().yaml(), "");
    }
}

#[test]
fn frontmatter_explicit_managed_edits_null_and_removals_win_and_filename_stays() {
    let (_guard, _root, mut app) = fixture();
    open(
        &mut app,
        "---\ntitle: Old\nupdated_at: 1\ntags: [old]\nlinks: [old]\npinned: true\ntext_align: center\nstatus: open\n---\n[[CL-02]]\n",
    );
    yaml(
        &mut app,
        "status: answered\nupdated_at: 2026-09-19\ntags: [new]\nlinks: []\npinned: null\ntext_align: right\ncustom: {nested: [true, null, 3]}\n",
    );
    app.autosave().unwrap();
    let source = disk(&app);
    let parsed = PropertyDocument::parse(&source).unwrap();
    assert!(!parsed.properties.contains_key("title"));
    assert_eq!(parsed.properties.get("pinned"), Some(&Value::Null));
    assert_eq!(
        parsed.properties.get("updated_at").and_then(Value::as_str),
        Some("2026-09-19")
    );
    assert_eq!(app.editor.editing_id.as_deref(), Some("CL-01.md"));
    assert_eq!(
        crate::events::get_title_text(&app.editor.title_editor),
        "CL-01"
    );
    app.editor.title_editor.select_all();
    app.editor.title_editor.insert_str("New title");
    app.autosave().unwrap();
    assert_eq!(app.editor.editing_id.as_deref(), Some("CL-01.md"));
    assert_eq!(
        app.storage.load_note("CL-01.md").unwrap().title,
        "New title"
    );
}

#[test]
fn frontmatter_disabled_policy_preserves_existing_and_controls_new_output() {
    let (_guard, _root, mut app) = fixture();
    app.storage.frontmatter_policy = toml::from_str::<crate::config::ClinConfig>("[notes.frontmatter]\ntitle=false\ntags=false\npinned=false\nlinks=false\ntext_align=false\nupdated_at='none'\n").unwrap().notes.frontmatter;
    open(
        &mut app,
        "---\n# fixed\ntitle: 'Manual'\nupdated_at: 2026-09-19\nlinks: [manual]\noriginal_ext: txt\n---\nBody",
    );
    let header = PropertyFrame::parse(&disk(&app)).unwrap().yaml().to_owned();
    app.editor.body.insert_str("edit ");
    app.autosave().unwrap();
    assert_eq!(PropertyFrame::parse(&disk(&app)).unwrap().yaml(), header);
    app.start_blank_note_with_title(String::new(), "new".into());
    app.editor.body.insert_str("Body");
    app.autosave().unwrap();
    let source = app
        .storage
        .load_document(app.editor.editing_id.as_deref().unwrap())
        .unwrap();
    assert_eq!(source.source(), "Body");
    assert_eq!(
        crate::events::get_title_text(&app.editor.title_editor),
        "new"
    );
}

#[test]
fn frontmatter_iso_timestamp_projection_dedup_and_uninterpretable_values() {
    let (_guard, _root, mut app) = fixture();
    for value in ["2026-09-19", "2026-09-19T00:00:00Z"] {
        open(
            &mut app,
            &format!(
                "---\nupdated_at: {value}\nlinks: []\ntags: {{unexpected: shape}}\nstatus: open\n---\nBody"
            ),
        );
        assert_eq!(
            app.storage.load_note("CL-01.md").unwrap().updated_at,
            1_789_776_000
        );
        assert_eq!(app.editor.body.text(), "Body");
        app.storage.frontmatter_policy.updated_at = TimestampFormat::Iso8601;
        app.editor
            .body
            .insert_str("[[CL-02]] [[CL-02|again]] [[CL-03]] ");
        app.autosave().unwrap();
        let source = disk(&app);
        let parsed = PropertyDocument::parse(&source).unwrap();
        assert!(
            parsed
                .properties
                .get("updated_at")
                .unwrap()
                .as_str()
                .unwrap()
                .ends_with('Z')
        );
        assert_eq!(
            parsed
                .properties
                .get("links")
                .unwrap()
                .as_sequence()
                .unwrap()
                .len(),
            2
        );
        assert!(source.contains("tags: {unexpected: shape}"));
    }
    open(
        &mut app,
        "---\nupdated_at: {unknown: timestamp}\nlinks: 7\n---\nBody",
    );
    app.editor.body.insert_str("Edit ");
    app.autosave().unwrap();
    assert!(disk(&app).contains("updated_at: {unknown: timestamp}"));
    assert!(disk(&app).contains("links: 7"));
}

#[test]
fn frontmatter_invalid_yaml_and_title_conflict_block_save_exit_and_survive_restart() {
    let (_guard, _root, mut app) = fixture();
    let original = "---\ntitle: Old\nstatus: open\n---\nBody";
    for invalid in ["status: [", "status: one\nstatus: two", "title: YAML"] {
        open(&mut app, original);
        yaml(&mut app, invalid);
        if invalid == "title: YAML" {
            app.editor.title_editor.select_all();
            app.editor.title_editor.insert_str("Title input");
        }
        app.editor.body.insert_str("pending ");
        app.write_draft();
        app.flush_drafts().unwrap();
        assert!(app.autosave().is_err());
        assert_eq!(disk(&app), original);
        let mut focus = EditFocus::Frontmatter;
        key(&mut app, &mut focus, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.mode, ViewMode::Edit);
        assert!(!app.prepare_editor_transition());
        app.storage.recover_documents().unwrap();
        assert_eq!(disk(&app), original);
        let storage = app.storage.clone();
        drop(app);
        app = App::new(storage).unwrap();
        assert_eq!(app.mode, ViewMode::Edit);
        assert_eq!(app.editor.frontmatter.as_ref().unwrap().yaml(), invalid);
        assert!(app.editor.body.text().contains("pending"));
        yaml(&mut app, "title: Old\nstatus: repaired\n");
        app.editor.title_editor.select_all();
        app.editor.title_editor.insert_str("Old");
        app.autosave().unwrap();
        app.flush_drafts().unwrap();
        assert!(app.storage.editor_drafts().unwrap().is_empty());
        app.storage.recover_documents().unwrap();
        assert!(disk(&app).contains("status: repaired"));
    }
}

#[test]
fn frontmatter_invalid_new_note_draft_never_creates_vault_file() {
    let (_guard, _root, mut app) = fixture();
    app.start_blank_note_with_title(String::new(), "New".into());
    yaml(&mut app, "broken: [");
    app.editor.body.insert_str("Unsaved body\n\n");
    let id = app.editor.editing_id.clone().unwrap();
    app.write_draft();
    app.flush_drafts().unwrap();
    let storage = app.storage.clone();
    drop(app);
    storage.recover_documents().unwrap();
    assert!(!storage.notes_dir.join(&id).exists());
    app = App::new(storage).unwrap();
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(app.editor.body.text(), "Unsaved body\n\n");
    yaml(&mut app, "custom: true\n");
    app.autosave().unwrap();
    assert!(
        app.storage
            .load_note(app.editor.editing_id.as_deref().unwrap())
            .unwrap()
            .content
            .contains("Unsaved body")
    );
}

#[test]
fn frontmatter_exact_crlf_bom_flow_noop_stale_and_readonly_protection() {
    let (_guard, _root, mut app) = fixture();
    for source in [
        "\u{feff}---\r\n# keep\r\ntitle: 'Old'\r\nstatus: open # user\r\n\r\n...\r\nBody\r\n\r\n",
        "---\n{title: Old, status: open, nested: {a: [1, 2]}}\n---\nBody\n",
    ] {
        open(&mut app, source);
        app.autosave().unwrap();
        assert_eq!(disk(&app), source);
        app.editor
            .frontmatter
            .as_mut()
            .unwrap()
            .patch("status".into(), Some(Value::String("answered".into())))
            .unwrap();
        app.autosave().unwrap();
        let saved = disk(&app);
        assert_eq!(
            PropertyDocument::parse(&saved).unwrap().body,
            PropertyDocument::parse(source).unwrap().body
        );
        assert_eq!(app.editor.editing_id.as_deref(), Some("CL-01.md"));
        yaml(&mut app, "status: pending\n");
        fs::write(app.storage.notes_dir.join("CL-01.md"), "external").unwrap();
        assert!(app.autosave().is_err());
        assert_eq!(disk(&app), "external");
    }
}

#[test]
fn frontmatter_malformed_disk_yaml_is_repairable_and_ambiguous_framing_fails() {
    let (_guard, _root, mut app) = fixture();
    open(&mut app, "---\ninvalid: [\n---\nBody");
    assert_eq!(app.editor.body.text(), "Body");
    assert!(app.autosave().is_err());
    yaml(&mut app, "status: repaired\n");
    app.autosave().unwrap();
    assert!(disk(&app).contains("status: repaired"));
    fs::write(
        app.storage.notes_dir.join("ambiguous.md"),
        "---\nmissing closing delimiter",
    )
    .unwrap();
    assert!(app.storage.load_note("ambiguous.md").is_err());
}

#[test]
fn frontmatter_changed_types_only_and_duplicate_names_are_validated() {
    let (_guard, _root, mut app) = fixture();
    fs::create_dir_all(app.storage.notes_dir.join(".obsidian")).unwrap();
    fs::write(
        app.storage.notes_dir.join(".obsidian/types.json"),
        r#"{"types":{"score":"number","status":"text"}}"#,
    )
    .unwrap();
    open(
        &mut app,
        "---\nscore: old-invalid-value\nstatus: open\n---\nBody",
    );
    yaml(&mut app, "score: old-invalid-value\nstatus: answered\n");
    app.autosave().unwrap();
    yaml(&mut app, "score: another-invalid-value\nstatus: answered\n");
    assert!(app.autosave().is_err());
    yaml(&mut app, "score: 42\nstatus: answered\n");
    app.autosave().unwrap();
    assert!(disk(&app).contains("score: 42"));
}

#[test]
fn frontmatter_small_layout_mouse_body_isolation_and_reload_retention() {
    let (_guard, root, mut app) = fixture();
    open(&mut app, "---\nstatus: open\n---\nBody");
    app.toggle_frontmatter();
    yaml(&mut app, "status: [");
    for mode in ["raw_yaml", "properties"] {
        fs::write(
            root.path().join("config.toml"),
            format!("[editor]\nfrontmatter_mode='{mode}'\n"),
        )
        .unwrap();
        app.reload_config();
        assert_eq!(app.editor.frontmatter.as_ref().unwrap().yaml(), "status: [");
        for (width, height) in [(100, 28), (20, 8), (8, 4), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Frontmatter))
                .unwrap();
        }
    }
    assert_eq!(app.editor.body.text(), "Body");
}

#[test]
fn frontmatter_live_bases_metadata_rebase_conflict_and_suspended_owner() {
    let (_guard, _root, mut app) = fixture();
    open(&mut app, "---\nscore: 2\n---\nBody\n");
    let before = app.storage.load_document("CL-01.md").unwrap();
    let revision = app.editor.live_note_revision();
    yaml(&mut app, "score: 7\n");
    assert_ne!(revision, app.editor.live_note_revision());
    let metadata = app
        .storage
        .load_vault_metadata(&AtomicBool::new(false))
        .unwrap();
    let live = metadata
        .with_host_source("CL-01.md", &app.editor.live_note_source().unwrap(), false)
        .unwrap();
    assert_eq!(
        live.file("CL-01.md")
            .unwrap()
            .properties()
            .unwrap()
            .get("score")
            .and_then(Value::as_i64),
        Some(7)
    );
    app.guard_embed_host(&before).unwrap();
    assert!(
        app.editor
            .public_document_source()
            .unwrap()
            .contains("score: 7")
    );
    assert!(app.ensure_inline_host_write(&before).is_err());
    let after = app
        .storage
        .save_document(&before, "---\nscore: 9\n---\nBody\n")
        .unwrap();
    assert!(!app.editor.synchronize_public_snapshot(&before, &after));
    assert!(
        app.editor
            .public_document_source()
            .unwrap()
            .contains("score: 7")
    );
    app.embedded_host_editor = Some(Box::new(std::mem::take(&mut app.editor)));
    assert!(app.ensure_inline_host_write(&after).is_err());
    assert!(disk(&app).contains("score: 9"));
    app.editor = *app.embedded_host_editor.take().unwrap();
    yaml(&mut app, "score: 9\n");
    app.editor.public_document = Some(after.clone());
    app.editor.body.insert_str("pending body ");
    let next = app
        .storage
        .save_document(&after, "---\nscore: 12\n---\nBody\n")
        .unwrap();
    assert!(app.editor.synchronize_public_snapshot(&after, &next));
    assert!(app.editor.body.text().contains("pending body"));
    assert!(
        app.editor
            .public_document_source()
            .unwrap()
            .contains("score: 12")
    );
}

#[test]
fn frontmatter_alignment_pin_duplicate_preserve_custom_yaml_and_pending_body() {
    let (_guard, _root, mut app) = fixture();
    open(&mut app, "---\n# user\ncustom: 'retained'\n---\nBody");
    app.storage.toggle_pin("CL-01.md").unwrap();
    app.mode = ViewMode::List;
    app.load_and_open_note("CL-01.md", None);
    app.editor.body.insert_str("pending ");
    app.config.editor.soft_wrap = true;
    app.apply_editor_prefs();
    app.cycle_text_alignment();
    app.autosave().unwrap();
    assert!(disk(&app).contains("# user\ncustom: 'retained'"));
    assert_eq!(disk(&app).matches("---").count(), 2);
    assert!(disk(&app).contains("pending Body"));
    let duplicate = app.storage.duplicate_note("CL-01.md", "").unwrap();
    assert!(
        fs::read_to_string(app.storage.notes_dir.join(duplicate))
            .unwrap()
            .contains("custom: 'retained'")
    );
}

#[test]
fn frontmatter_session_saves_metadata_only_and_clears_coalesced_draft() {
    use crossterm::event::Event;
    let (_guard, _root, mut app) = fixture();
    open(&mut app, "---\nstatus: open\n---\nBody\n");
    app.config.editor.frontmatter_mode = FrontmatterMode::RawYaml;
    let (sender, receiver) = std::sync::mpsc::channel();
    for event in [
        Event::Key(KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        Event::Paste("status: answered\n".into()),
        Event::Key(KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    ] {
        sender.send(event).unwrap();
    }
    let mut events = crate::event_source::EventSource::channel(receiver);
    let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
    crate::editor_session::run_editor_session(&mut terminal, &mut app, &mut events, &mut |_| false)
        .unwrap();
    assert_eq!(app.mode, ViewMode::List);
    assert_eq!(disk(&app), "---\nstatus: answered\n---\nBody\n");
    app.flush_drafts().unwrap();
    app.storage.recover_documents().unwrap();
    assert_eq!(disk(&app), "---\nstatus: answered\n---\nBody\n");
}

#[cfg(unix)]
#[test]
fn frontmatter_readonly_metadata_save_retains_raw_input_and_private_draft() {
    use std::os::unix::fs::PermissionsExt;
    let (_guard, _root, mut app) = fixture();
    let source = "---\nstatus: open\n---\nBody";
    open(&mut app, source);
    yaml(&mut app, "status: answered\n");
    fs::set_permissions(
        app.storage.notes_dir.join("CL-01.md"),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    app.write_draft();
    app.flush_drafts().unwrap();
    assert!(app.autosave().is_err());
    assert_eq!(disk(&app), source);
    assert_eq!(
        app.editor.frontmatter.as_ref().unwrap().yaml(),
        "status: answered\n"
    );
    for entry in fs::read_dir(app.storage.data_dir.join(".clin/document-recovery-v2")).unwrap() {
        assert_eq!(
            entry.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn frontmatter_remapped_toggle_and_focus_scoped_palette_commands() {
    let (_guard, _root, mut app) = fixture();
    open(&mut app, "---\nstatus: open\n---\nBody");
    app.keybinds.edit.insert(
        crate::keybinds::EditAction::ToggleFrontmatter,
        vec![crate::keybinds::KeyCombo::parse("Alt+f").unwrap()],
    );
    let mut focus = EditFocus::Body;
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    assert!(!app.editor.frontmatter.as_ref().unwrap().visible);
    key(&mut app, &mut focus, KeyCode::Char('f'), KeyModifiers::ALT);
    assert_eq!(focus, EditFocus::Frontmatter);
    let context = crate::actions::ActionContext::capture(&app, Some(focus), None);
    app.open_command_palette(Some(focus));
    assert!(app.command_palette.is_some());
    assert_eq!(context.focus, Some(EditFocus::Frontmatter));
    app.command_palette = None;
    for mode in [FrontmatterMode::Properties, FrontmatterMode::RawYaml] {
        app.config.editor.frontmatter_mode = mode;
        key(
            &mut app,
            &mut focus,
            KeyCode::Char(';'),
            KeyModifiers::CONTROL,
        );
        key(
            &mut app,
            &mut focus,
            KeyCode::Char('i'),
            KeyModifiers::CONTROL,
        );
        assert_eq!(app.editor.body.text(), "Body");
    }
}

#[test]
fn frontmatter_unsafe_anchor_patch_rejects_before_disk_or_working_source_change() {
    let (_guard, _root, mut app) = fixture();
    let source = "---\ntitle: &title Original\ncustom: *title\n---\nBody";
    open(&mut app, source);
    let state = app.editor.frontmatter.as_mut().unwrap();
    let yaml = state.yaml();
    assert!(
        state
            .patch("title".into(), Some(Value::String("Changed".into())))
            .is_err()
    );
    assert_eq!(state.yaml(), yaml);
    assert_eq!(disk(&app), source);
}

#[test]
fn frontmatter_unfinished_property_input_survives_restart_and_mode_reload() {
    let (_guard, root, mut app) = fixture();
    open(&mut app, "---\nstatus: open\n---\nBody\n\n");
    let mut focus = EditFocus::Body;
    key(&mut app, &mut focus, KeyCode::F(7), KeyModifiers::NONE);
    key(&mut app, &mut focus, KeyCode::Char('a'), KeyModifiers::NONE);
    crate::events::handle_bracketed_paste(&mut app, "nested".into(), &mut focus);
    key(&mut app, &mut focus, KeyCode::Tab, KeyModifiers::NONE);
    key(
        &mut app,
        &mut focus,
        KeyCode::Char('a'),
        KeyModifiers::CONTROL,
    );
    crate::events::handle_bracketed_paste(&mut app, "{items: [".into(), &mut focus);
    app.write_draft();
    app.flush_drafts().unwrap();
    let storage = app.storage.clone();
    drop(app);
    app = App::new(storage).unwrap();
    assert_eq!(app.editor.body.text(), "Body\n\n");
    assert!(app.editor.show_line_numbers);
    let input = app
        .editor
        .frontmatter
        .as_ref()
        .unwrap()
        .input
        .as_ref()
        .unwrap();
    assert_eq!(input.name.lines(), ["nested"]);
    assert_eq!(input.value.lines(), ["{items: ["]);
    fs::write(
        root.path().join("config.toml"),
        "[editor]\nfrontmatter_mode = 'raw_yaml'\n",
    )
    .unwrap();
    app.reload_config();
    focus = EditFocus::Frontmatter;
    key(
        &mut app,
        &mut focus,
        KeyCode::Char('a'),
        KeyModifiers::CONTROL,
    );
    crate::events::handle_bracketed_paste(&mut app, "{items: [1, null, true]}".into(), &mut focus);
    key(&mut app, &mut focus, KeyCode::Enter, KeyModifiers::CONTROL);
    assert!(app.editor.frontmatter.as_ref().unwrap().input.is_none());
    app.autosave().unwrap();
    assert!(disk(&app).contains("items"));
}
