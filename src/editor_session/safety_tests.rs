use super::*;
use crate::editor_document::EditorDocument;
use crossterm::event::KeyEvent;
use ratatui::backend::TestBackend;
use std::time::Instant;

fn fixture() -> (crate::config::ConfigTestGuard, tempfile::TempDir, App) {
    let guard = crate::config::ConfigTestGuard::lock();
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.toml");
    std::fs::write(&config, "").unwrap();
    crate::config::set_config_path_override(config);
    let storage = crate::storage::Storage {
        data_dir: root.path().join("vault"),
        notes_dir: root.path().join("vault"),
        config_dir: root.path().join("config"),
        templates_dir: root.path().join("templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: crate::frontmatter::WritePolicy::default(),
    };
    for directory in [
        &storage.notes_dir,
        &storage.config_dir,
        &storage.templates_dir,
    ] {
        std::fs::create_dir_all(directory).unwrap();
    }
    let mut app = App::new(storage).unwrap();
    app.start_blank_note_with_title("original".into(), "session".into());
    app.autosave().unwrap();
    (guard, root, app)
}

#[test]
fn save_failure_remains_unsaved_retryable_and_blocks_quit_and_view_switch() {
    let (_guard, _root, mut app) = fixture();
    let id = app.editor.editing_id.clone().unwrap();
    let path = app.storage.notes_dir.join(&id);
    app.editor.body = EditorDocument::from_text("my unsaved edits");
    app.editor.last_saved_time = None;
    std::fs::write(&path, "external edits").unwrap();
    assert!(app.autosave().is_err());
    assert_eq!(
        app.editor.autosave_status,
        crate::editor::AutosaveStatus::Unsaved
    );
    assert!(app.editor.autosave_timer.is_some());
    assert!(app.editor.last_saved_time.is_none());
    for quit in [true, false] {
        app.should_quit = quit;
        app.mode = if quit {
            ViewMode::Edit
        } else {
            ViewMode::Graph
        };
        assert!(!finish_editor_transition(&mut app));
        assert!(!app.should_quit);
        assert_eq!(app.mode, ViewMode::Edit);
        assert_eq!(app.editor.body.lines().join("\n"), "my unsaved edits");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external edits");
    }
}

#[test]
fn ctrl_c_without_selection_does_not_quit_or_save() {
    let (_guard, _root, mut app) = fixture();
    let id = app.editor.editing_id.clone().unwrap();
    let before = std::fs::read(app.storage.notes_dir.join(&id)).unwrap();
    let mut terminal = ratatui::Terminal::new(TestBackend::new(100, 30)).unwrap();
    let mut focus = EditFocus::Body;
    let mut selection = MouseTextSelection::default();
    dispatch_editor_event(
        &mut terminal,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        &mut focus,
        &mut selection,
    )
    .unwrap();
    assert!(!app.should_quit);
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(
        std::fs::read(app.storage.notes_dir.join(&id)).unwrap(),
        before
    );
}

#[cfg(unix)]
#[test]
fn readonly_notes_keep_raw_source_on_exit_and_newline_is_an_edit() {
    use std::os::unix::fs::PermissionsExt;
    let (_guard, _root, mut app) = fixture();
    let id = app.editor.editing_id.clone().unwrap();
    let path = app.storage.notes_dir.join(&id);
    for body in ["Body", "Body\n", "Body\n\n", "Body\r\n\r\n"] {
        app.mode = ViewMode::List;
        let source =
            format!("---\r\n# keep comment\r\ntitle: 'session'\r\ncustom: [a, b]\r\n---\r\n{body}");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&path, &source).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        app.load_and_open_note(&id, None);
        let mut terminal = ratatui::Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut focus = EditFocus::Body;
        let mut selection = MouseTextSelection::default();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
            .unwrap();
        dispatch_editor_event(
            &mut terminal,
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &mut focus,
            &mut selection,
        )
        .unwrap();
        assert_eq!(app.mode, ViewMode::List);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        app.load_and_open_note(&id, None);
        app.editor
            .body
            .move_cursor(ratatui_textarea::CursorMove::Bottom);
        app.editor
            .body
            .move_cursor(ratatui_textarea::CursorMove::End);
        dispatch_editor_event(
            &mut terminal,
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            &mut focus,
            &mut selection,
        )
        .unwrap();
        assert!(
            app.autosave().is_err(),
            "adding a newline must attempt a real write"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
#[ignore = "full input/render/durable recovery and save benchmark"]
fn full_editor_persistence_perf() {
    let (_guard, _root, mut app) = fixture();
    let id = app.editor.editing_id.clone().unwrap();
    for rows in [5_000, 100_000] {
        app.load_and_open_note(&id, None);
        app.editor.body =
            EditorDocument::from_text(&(0..rows).fold(String::new(), |mut source, row| {
                use std::fmt::Write;
                writeln!(source, "ordinary text for line {row}").unwrap();
                source
            }));
        app.autosave().unwrap();
        let mut cycles = Vec::new();
        let mut sessions = Vec::new();
        for _ in 0..4 {
            app.load_and_open_note(&id, None);
            let before = app.storage.load_note(&id).unwrap().content.len();
            let (sender, receiver) = std::sync::mpsc::channel();
            let mut events = crate::event_source::EventSource::channel(receiver);
            let mut terminal = ratatui::Terminal::new(TestBackend::new(100, 30)).unwrap();
            let mut keys = 0;
            let mut last_cycle = None;
            let started = Instant::now();
            run_editor_session(&mut terminal, &mut app, &mut events, &mut |_| {
                if let Some(started) = last_cycle.take() {
                    cycles.push(Instant::now().duration_since(started).as_secs_f64() * 1000.0);
                }
                if keys < 100 {
                    // One key per draw: do not hide input work through burst coalescing.
                    sender
                        .send(Event::Key(KeyEvent::new(
                            KeyCode::Char('x'),
                            KeyModifiers::NONE,
                        )))
                        .unwrap();
                    keys += 1;
                    last_cycle = Some(Instant::now());
                } else {
                    sender
                        .send(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
                        .unwrap();
                }
                false
            })
            .unwrap();
            sessions.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(app.mode, ViewMode::List);
            assert_eq!(
                app.storage.load_note(&id).unwrap().content.len(),
                before + 100
            );
            assert_eq!(
                std::fs::read_dir(app.storage.data_dir.join(".clin/document-recovery-v2"))
                    .unwrap()
                    .count(),
                0
            );
        }
        cycles.sort_by(f64::total_cmp);
        sessions.sort_by(f64::total_cmp);
        let p95 = cycles[(cycles.len() * 95).div_ceil(100) - 1];
        println!(
            "full_editor rows={rows} samples={} cycle_p50_ms={:.3} cycle_p95_ms={p95:.3} session_100_keys_max_ms={:.3}",
            cycles.len(),
            cycles[cycles.len() / 2],
            sessions[sessions.len() - 1]
        );
        if std::env::var_os("CLIN_PERF_ENFORCE").is_some() {
            assert!(
                p95 < 50.0,
                "full editor input/render cycle exceeded 50 ms: {p95}"
            );
        }
    }
}

#[test]
fn failed_save_blocks_new_note_other_note_public_source_and_palette_actions() {
    let (_guard, _root, mut app) = fixture();
    let id = app.editor.editing_id.clone().unwrap();
    let public = app.storage.create_document("other.md", "other").unwrap();
    let path = app.storage.notes_dir.join(&id);
    app.editor.body.insert_str("unsaved ");
    std::fs::write(&path, "external").unwrap();
    app.start_blank_note_with_title(String::new(), "new".into());
    app.load_and_open_note("other.md", None);
    app.open_public_document(public);
    assert!(crate::actions::execute_action("graph.open", &mut app, None).is_err());
    assert_eq!(app.editor.editing_id.as_deref(), Some(id.as_str()));
    assert_eq!(app.editor.body.text(), "unsaved ");
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "external");
}

#[test]
fn template_conflict_and_readonly_failures_keep_editor_and_original_filename() {
    let (_guard, _root, mut app) = fixture();
    let path = app.storage.templates_dir.join("original.toml");
    let source = "name = 'original'\ncontent = 'body'\n";
    std::fs::write(&path, source).unwrap();
    app.editor.editing_id = None;
    app.editor.public_document = None;
    app.editor.template_edit_path = Some(path.clone());
    app.editor.template_source = Some(source.into());
    app.editor.body = EditorDocument::from_source("name = 'renamed'\ncontent = 'edits'\n");
    std::fs::write(&path, "external").unwrap();
    assert!(app.autosave().is_err());
    assert!(!app.storage.template_path("renamed").exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
    app.should_quit = true;
    assert!(!finish_editor_transition(&mut app));
    assert_eq!(app.editor.template_edit_path.as_ref(), Some(&path));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&path, source).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        assert!(app.autosave().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert!(!app.storage.template_path("renamed").exists());
    }
}

#[test]
fn bracketed_paste_queues_recovery_before_exit() {
    let (_guard, _root, mut app) = fixture();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut events = crate::event_source::EventSource::channel(receiver);
    let mut terminal = ratatui::Terminal::new(TestBackend::new(100, 30)).unwrap();
    let mut draws = 0;
    run_editor_session(&mut terminal, &mut app, &mut events, &mut |app| {
        draws += 1;
        if draws == 1 {
            sender.send(Event::Paste("pasted 界\ntext".into())).unwrap();
        } else {
            app.flush_drafts().unwrap();
            assert_eq!(
                std::fs::read_dir(app.storage.data_dir.join(".clin/document-recovery-v2"))
                    .unwrap()
                    .count(),
                1
            );
            sender
                .send(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
                .unwrap();
        }
        false
    })
    .unwrap();
    assert_eq!(draws, 2);
    let id = app
        .notes
        .iter()
        .find(|note| note.title == "session")
        .unwrap()
        .id
        .clone();
    assert_eq!(
        app.storage.load_note(&id).unwrap().content,
        "pasted 界\ntext"
    );
}
