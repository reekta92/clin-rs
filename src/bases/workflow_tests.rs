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
    key(&mut app, KeyCode::Char('l'));
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
