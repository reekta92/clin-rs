//! Native overlay workflows over a disposable public vault, without terminal or clipboard dependencies.
use crate::{
    app::{App, HelpTab, ViewMode},
    config::FeatureState,
    keybinds::Keybinds,
    overlay::{OverlayResult, OverlayView},
    storage::Storage,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{
    fs,
    time::{Duration, Instant},
};

const AREA: Rect = Rect::new(0, 0, 100, 28);

fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let storage = Storage {
        data_dir: root.path().join("data"),
        notes_dir: root.path().join("vault"),
        config_dir: root.path().join("vault/.clin"),
        templates_dir: root.path().join("templates"),
        key: [0; 32],
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: true,
    };
    for directory in [
        &storage.data_dir,
        &storage.notes_dir,
        &storage.config_dir,
        &storage.templates_dir,
    ] {
        fs::create_dir_all(directory).unwrap();
    }
    storage
        .create_document("public.md", "---\r\nscore: 2\r\n---\r\nPublic body\r\n\r\n")
        .unwrap();
    storage.create_document("board.base", "# retained\nplugin: {opaque: true}\nfilters: file.ext == 'md'\nviews:\n  - type: table\n    name: Public\n    order: [file.name, score]\n  - type: list\n    name: List\n    order: [file.name, score]\n").unwrap();
    fs::write(
        storage.notes_dir.join("private.clin"),
        b"not a public document",
    )
    .unwrap();
    let mut app = App::new(storage).unwrap();
    app.config.features.bases_view = FeatureState::Enabled;
    app.keybinds = Keybinds::default();
    app.open_bases_view("board.base", None);
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| !state.loading && state.result().is_some())
    });
    (root, app)
}
fn wait(app: &mut App, condition: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_bases();
        if condition(app) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Bases workflow stalled: {}",
            app.status
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn event(app: &mut App, event: Event) -> OverlayResult {
    let mut state = app.bases_state.take().unwrap();
    let result = state.overlay_handle_event(event, app, AREA).unwrap();
    app.bases_state = Some(state);
    result
}
fn key(app: &mut App, code: KeyCode) -> OverlayResult {
    event(app, Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}
fn render(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    let mut state = app.bases_state.take().unwrap();
    terminal
        .draw(|frame| state.overlay_render(frame, AREA, app))
        .unwrap();
    app.bases_state = Some(state);
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn typed_edit_undo_redo_and_verbatim_source_handoff_are_public_only() {
    let (_root, mut app) = fixture();
    let initial = app
        .storage
        .load_document("public.md")
        .unwrap()
        .source()
        .to_owned();
    let result = app.bases_state.as_ref().unwrap().result().unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].id, "public.md");
    assert!(!render(&mut app).contains("private.clin"));
    event(
        &mut app,
        Event::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 25,
            row: 2,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert_eq!(app.bases_state.as_ref().unwrap().column, 1);
    key(&mut app, KeyCode::Char('e'));
    wait(&mut app, |app| {
        app.bases_state.as_ref().unwrap().dialog.is_some()
    });
    key(&mut app, KeyCode::Delete);
    event(&mut app, Event::Paste("7".into()));
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none() && !state.loading)
    });
    let edited = app
        .storage
        .load_document("public.md")
        .unwrap()
        .source()
        .to_owned();
    assert_eq!(edited, initial.replace("score: 2", "score: 7"));
    key(&mut app, KeyCode::Char('u'));
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none() && !state.loading)
    });
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        initial
    );
    event(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
    );
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none() && !state.loading)
    });
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        edited
    );
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| app.mode == ViewMode::Edit);
    app.autosave().unwrap();
    app.back_to_list(None, None);
    assert_eq!(app.mode, ViewMode::Bases);
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        edited
    );
    assert!(matches!(
        key(&mut app, KeyCode::Char('?')),
        OverlayResult::OpenHelp(HelpTab::Bases)
    ));
    assert!(matches!(
        key(&mut app, KeyCode::Char('q')),
        OverlayResult::Exit
    ));
}

#[test]
fn table_labels_summaries_and_reopening_preserve_native_return_mode() {
    let (_root, mut app) = fixture();
    let previous = app.bases_state.as_ref().unwrap().previous_mode;
    app.open_bases_view("board.base", None);
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    assert_eq!(app.bases_state.as_ref().unwrap().previous_mode, previous);
    let base = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&base,"properties: {score: {displayName: Rating}}\nfilters: file.ext == 'md'\nviews: [{type: table, name: Scores, order: [file.name, score], groupBy: {property: file.ext}, summaries: {score: Sum}}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    let screen = render(&mut app);
    assert!(screen.contains("Rating"));
    assert_eq!(
        screen.matches("Σ 2").count(),
        2,
        "group and view summaries must both remain visible"
    );
}

#[test]
fn kanban_navigation_crosses_columns_and_retains_empty_group_focus() {
    let (_root, mut app) = fixture();
    let base = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&base,"filters: file.ext == 'md'\nviews: [{type: kanban, name: Board, order: [file.name], groupBy: {property: status}, groupOrder: [A, Empty, B]}]\n").unwrap();
    let public = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(
            &public,
            &public.source().replace("score: 2", "score: 2\r\nstatus: A"),
        )
        .unwrap();
    app.storage
        .create_document("b.md", "---\nstatus: B\n---\nB body\n")
        .unwrap();
    key(&mut app, KeyCode::Char('r'));
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    assert!(render(&mut app).contains("Empty"));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "public.md"
    );
    for _ in 0..3 {
        key(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "public.md"
    );
    key(&mut app, KeyCode::Char('l'));
    assert_eq!(
        app.bases_state.as_ref().unwrap().focused_group(),
        Some((1, None))
    );
    key(&mut app, KeyCode::Char('l'));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.bases_state.as_ref().unwrap().row().unwrap().id, "b.md");
    key(&mut app, KeyCode::Char('h'));
    assert_eq!(
        app.bases_state.as_ref().unwrap().focused_group(),
        Some((1, None))
    );
    key(&mut app, KeyCode::Char('h'));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "public.md"
    );
}

#[test]
fn multiline_formula_input_applies_once_and_invalid_input_retains_source() {
    let (_root, mut app) = fixture();
    key(&mut app, KeyCode::Char(':'));
    for _ in 0..10 {
        key(&mut app, KeyCode::Char('j'));
    }
    key(&mut app, KeyCode::Enter);
    event(&mut app, Event::Paste("Total = 1".into()));
    event(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
    );
    event(&mut app, Event::Paste("+ 2".into()));
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none() && !state.loading)
    });
    let source = app.storage.load_document("board.base").unwrap();
    let doc = super::format::BaseDocument::parse(source.source()).unwrap();
    assert!(
        doc.config.formulas.contains_key("Total"),
        "status: {}; source: {}",
        app.status,
        source.source()
    );
    assert_eq!(doc.config.formulas["Total"], "1\n+ 2");
    key(&mut app, KeyCode::Char(':'));
    for _ in 0..10 {
        key(&mut app, KeyCode::Char('j'));
    }
    key(&mut app, KeyCode::Enter);
    event(&mut app, Event::Paste("Total = console.log(1)".into()));
    key(&mut app, KeyCode::Enter);
    assert!(app.bases_state.as_ref().unwrap().dialog.is_some());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        source.source()
    );
    key(&mut app, KeyCode::Esc);
}

#[test]
fn watcher_debounce_retains_events_and_refreshes_without_a_second_event() {
    use crate::app::WatchedFsEvent;
    use notify::{
        EventKind,
        event::{DataChange, ModifyKind},
    };
    let (_root, mut app) = fixture();
    let source = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(&source, &source.source().replace("score: 2", "score: 7"))
        .unwrap();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1024);
    app.fs_event_rx = Some(receiver);
    let changed = notify::Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
        .add_path(app.storage.notes_dir.join("public.md"));
    sender
        .send(WatchedFsEvent {
            observed_at: Instant::now(),
            event: changed.clone(),
        })
        .unwrap();
    crate::process_watcher_events(&mut app);
    assert_eq!(app.watcher_pending_events.len(), 1);
    assert!(!app.bases_state.as_ref().unwrap().loading);
    app.watcher_window_start = Some(
        Instant::now()
            .checked_sub(Duration::from_millis(260))
            .unwrap(),
    );
    crate::process_watcher_events(&mut app);
    assert!(app.watcher_pending_events.is_empty());
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    assert!(
        matches!(&app.bases_state.as_ref().unwrap().result().unwrap().rows[0].values["score"],super::value::Value::Number(value) if *value==7.0)
    );
    for _ in 0..513 {
        sender
            .send(WatchedFsEvent {
                observed_at: Instant::now(),
                event: changed.clone(),
            })
            .unwrap();
    }
    crate::process_watcher_events(&mut app);
    assert!(app.watcher_pending_events.is_empty());
    assert!(app.watcher_window_start.is_none());
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
}

#[test]
fn metadata_invalidation_survives_cancelled_refresh_view_switch_and_reopen() {
    let (_root, mut app) = fixture();
    let source = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(&source, &source.source().replace("score: 2", "score: 7"))
        .unwrap();
    app.invalidate_bases();
    key(&mut app, KeyCode::Tab);
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    assert!(
        matches!(&app.bases_state.as_ref().unwrap().result().unwrap().rows[0].values["score"],super::value::Value::Number(value) if *value==7.0)
    );
    app.bases_state = None;
    app.mode = ViewMode::List;
    let source = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(&source, &source.source().replace("score: 7", "score: 9"))
        .unwrap();
    app.send_catalog_paths(vec![crate::app::catalog::PathChange::Upsert(
        "public.md".into(),
    )]);
    app.open_bases_view("board.base", None);
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    assert!(
        matches!(&app.bases_state.as_ref().unwrap().result().unwrap().rows[0].values["score"],super::value::Value::Number(value) if *value==9.0)
    );
}

#[test]
fn configuration_paste_validates_and_preserves_unknown_data() {
    let (_root, mut app) = fixture();
    key(&mut app, KeyCode::Char(':'));
    assert!(render(&mut app).contains("CONFIGURATION"));
    key(&mut app, KeyCode::Enter);
    event(&mut app, Event::Paste("Renamed".into()));
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none() && !state.loading)
    });
    let source = app.storage.load_document("board.base").unwrap();
    assert!(source.source().contains("# retained"));
    assert!(source.source().contains("plugin: {opaque: true}"));
    assert!(source.source().contains("Renamed"));
    key(&mut app, KeyCode::Char('u'));
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none() && !state.loading)
    });
    assert!(
        app.storage
            .load_document("board.base")
            .unwrap()
            .source()
            .contains("name: Public")
    );
    assert!(
        app.bases_state.as_ref().unwrap().result().is_some(),
        "Undo must restore valid view selection"
    );
}
