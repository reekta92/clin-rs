//! Native overlay workflows over a disposable public vault, without terminal or clipboard dependencies.
#[path = "acceptance_tests.rs"]
mod acceptance;
#[path = "creation_workflow_tests.rs"]
mod creation;
#[path = "embed_workflow_tests.rs"]
mod embeds;
#[path = "layout_workflow_tests.rs"]
mod layout;
#[path = "media_workflow_tests.rs"]
mod media;
#[path = "palette_workflow_tests.rs"]
mod palette;
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
        key: rand::random(),
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
    if matches!(result, OverlayResult::OpenCommandPalette) {
        crate::finish_overlay_event(app, crate::overlay::ViewKind::Bases, result.clone());
    }
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

fn idle(app: &mut App) {
    wait(app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none() && !state.loading)
    });
}
fn board(app: &mut App, property: &str, order: &str) {
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline,&format!("# retained\nplugin: {{opaque: true}}\nfilters: file.ext == 'md'\nviews: [{{type: kanban, name: Board, order: [file.name], groupBy: {{property: {property}}}, groupOrder: {order}}}]\n")).unwrap();
    key(app, KeyCode::Char('r'));
    idle(app);
}
fn menu(app: &mut App, label: &str) {
    key(app, KeyCode::Char(':'));
    let index = super::authoring::COMMANDS
        .iter()
        .position(|(_, name, _)| *name == label)
        .unwrap();
    for _ in 0..index {
        key(app, KeyCode::Down);
    }
    key(app, KeyCode::Enter);
}
fn prompt_text(app: &mut App, text: &str) {
    if let Some(super::authoring::Dialog::Prompt { input, .. }) =
        &mut app.bases_state.as_mut().unwrap().dialog
    {
        input.select_all();
    } else {
        panic!("expected prompt: {}", app.status);
    }
    event(app, Event::Paste(text.into()));
    key(app, KeyCode::Enter);
    idle(app);
}
fn mouse(app: &mut App, kind: crossterm::event::MouseEventKind, rect: Rect) {
    event(
        app,
        Event::Mouse(crossterm::event::MouseEvent {
            kind,
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        }),
    );
}
fn drag_card(app: &mut App, id: &str, target: usize) {
    render(app);
    let state = app.bases_state.as_ref().unwrap();
    let index=state.items.iter().position(|item|matches!(item,super::state::Item::Row(row) if state.result().unwrap().rows[*row].id==id)).unwrap();
    let source = state
        .hit_rows
        .iter()
        .find(|(_, item, _)| *item == index)
        .unwrap()
        .0;
    let target = state
        .hit_groups
        .iter()
        .find(|(_, group)| *group == target)
        .unwrap()
        .0;
    mouse(
        app,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        source,
    );
    mouse(
        app,
        crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left),
        target,
    );
    mouse(
        app,
        crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
        target,
    );
    idle(app);
}
fn properties(app: &App, id: &str) -> serde_yaml_ng::Mapping {
    crate::frontmatter::properties::PropertyDocument::parse(
        app.storage.load_document(id).unwrap().source(),
    )
    .unwrap()
    .properties
}

#[test]
fn kanban_drag_create_reorder_hide_and_add_use_guarded_public_writes() {
    let (_root, mut app) = fixture();
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
    board(&mut app, "status", "[A, Empty, B, null]");
    let original = app
        .storage
        .load_document("public.md")
        .unwrap()
        .source()
        .to_owned();
    drag_card(&mut app, "public.md", 1);
    assert_eq!(
        properties(&app, "public.md")
            .get("status")
            .unwrap()
            .as_str(),
        Some("Empty")
    );
    assert!(
        app.storage
            .load_document("public.md")
            .unwrap()
            .source()
            .ends_with("Public body\r\n\r\n")
    );
    key(&mut app, KeyCode::Char('u'));
    idle(&mut app);
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        original
    );
    event(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
    );
    idle(&mut app);
    render(&mut app);
    let column = app
        .bases_state
        .as_ref()
        .unwrap()
        .hit_groups
        .iter()
        .find(|(_, index)| *index == 1)
        .unwrap()
        .0;
    mouse(
        &mut app,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        Rect::new(column.right() - 2, column.y, 1, 1),
    );
    prompt_text(&mut app, "created.md");
    assert_eq!(
        properties(&app, "created.md")
            .get("status")
            .unwrap()
            .as_str(),
        Some("Empty")
    );
    assert_eq!(properties(&app, "created.md").len(), 1);
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "created.md"
    );
    menu(&mut app, "Move focused group right");
    idle(&mut app);
    assert_eq!(
        app.bases_state.as_ref().unwrap().focused_group().unwrap().0,
        2
    );
    menu(&mut app, "Hide focused group");
    idle(&mut app);
    assert!(
        !app.bases_state
            .as_ref()
            .unwrap()
            .result()
            .unwrap()
            .rows
            .iter()
            .any(|row| row.id == "created.md")
    );
    assert_eq!(
        properties(&app, "created.md")
            .get("status")
            .unwrap()
            .as_str(),
        Some("Empty")
    );
    menu(&mut app, "Add empty group");
    prompt_text(&mut app, "New");
    assert!(
        app.bases_state
            .as_ref()
            .unwrap()
            .result()
            .unwrap()
            .groups
            .iter()
            .any(|group| group.value.to_text().display() == "New" && group.rows.is_empty())
    );
    menu(&mut app, "Hide all groups");
    idle(&mut app);
    assert!(render(&mut app).contains("No visible columns"));
    menu(&mut app, "Automatic group order / show all");
    idle(&mut app);
    assert_eq!(
        app.bases_state
            .as_ref()
            .unwrap()
            .result()
            .unwrap()
            .rows
            .len(),
        3
    );
    let source = app
        .storage
        .load_document("board.base")
        .unwrap()
        .source()
        .to_owned();
    assert!(source.contains("# retained") && source.contains("opaque: true"));
    render(&mut app);
    let columns = app.bases_state.as_ref().unwrap().hit_groups.clone();
    mouse(
        &mut app,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        columns[0].0,
    );
    mouse(
        &mut app,
        crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left),
        columns[1].0,
    );
    mouse(
        &mut app,
        crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
        columns[1].0,
    );
    idle(&mut app);
    assert_eq!(
        app.bases_state.as_ref().unwrap().focused_group(),
        Some((1, None))
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        b"not a public document"
    );
}

#[test]
fn kanban_link_groups_retarget_each_note_and_preserve_header_focus() {
    let (_root, mut app) = fixture();
    for (id, source) in [
        ("a/ref.md", "A reference"),
        ("b/ref.md", "B reference"),
        ("a/card.md", "---\nref: '[[ref.md|A ref]]'\n---\nA card"),
        ("b/card.md", "---\nref: '[[ref.md|B ref]]'\n---\nB card"),
    ] {
        app.storage.create_document(id, source).unwrap();
    }
    board(&mut app, "ref", "['[[a/ref.md]]', '[[b/ref.md]]', null]");
    key(&mut app, KeyCode::Home);
    menu(&mut app, "Move focused group right");
    idle(&mut app);
    assert_eq!(
        app.bases_state.as_ref().unwrap().focused_group(),
        Some((1, None))
    );
    drag_card(&mut app, "a/card.md", 0);
    let reference = properties(&app, "a/card.md")
        .get("ref")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    assert!(reference.contains("../b/ref.md") && reference.contains("B ref"));
    let loaded = app.bases_state.as_ref().unwrap().loaded.as_ref().unwrap();
    assert_eq!(
        loaded
            .metadata
            .resolve_link("a/card.md", &reference)
            .unwrap()
            .id,
        "b/ref.md"
    );
    key(&mut app, KeyCode::Char('n'));
    prompt_text(&mut app, "a/created.md");
    let reference = properties(&app, "a/created.md")
        .get("ref")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let loaded = app.bases_state.as_ref().unwrap().loaded.as_ref().unwrap();
    assert_eq!(
        loaded
            .metadata
            .resolve_link("a/created.md", &reference)
            .unwrap()
            .id,
        "b/ref.md"
    );
}

#[test]
fn grouped_layouts_collapse_with_keyboard_mouse_and_stable_refresh_focus() {
    let (_root, mut app) = fixture();
    app.storage
        .create_document("other.md", "---\nscore: 5\n---\nOther\n")
        .unwrap();
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline,"# retained\nplugin: {opaque: true}\nfilters: file.ext == 'md'\nviews:\n  - {type: table, name: Table, order: [file.name, score], groupBy: {property: score}, groupOrder: [2, 5, null]}\n  - {type: list, name: List, order: [file.name, score], groupBy: {property: score}, groupOrder: [2, 5, null]}\n  - {type: cards, name: Cards, order: [file.name, score], groupBy: {property: score}, groupOrder: [2, 5, null]}\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    let source = app
        .storage
        .load_document("board.base")
        .unwrap()
        .source()
        .to_owned();
    for _ in 0..3 {
        key(&mut app, KeyCode::Home);
        let count = app.bases_state.as_ref().unwrap().items.len();
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.bases_state.as_ref().unwrap().items.len(), count - 1);
        assert!(app.bases_state.as_ref().unwrap().group_collapsed(0));
        assert!(render(&mut app).contains("[+] 2"));
        key(&mut app, KeyCode::Char('r'));
        idle(&mut app);
        assert!(app.bases_state.as_ref().unwrap().group_collapsed(0));
        let state = app.bases_state.as_ref().unwrap();
        assert!(matches!(
            state.items[state.selected],
            super::state::Item::Group(0)
        ));
        render(&mut app);
        let state = app.bases_state.as_ref().unwrap();
        let header = state
            .hit_rows
            .iter()
            .find(|(_, index, _)| *index == state.selected)
            .unwrap()
            .0;
        mouse(
            &mut app,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            header,
        );
        assert_eq!(app.bases_state.as_ref().unwrap().items.len(), count);
        key(&mut app, KeyCode::Char('z'));
        assert_eq!(app.bases_state.as_ref().unwrap().items.len(), count - 1);
        key(&mut app, KeyCode::Tab);
        idle(&mut app);
        assert_eq!(app.bases_state.as_ref().unwrap().items.len(), count);
    }
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        source
    );
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Char('z'));
    let baseline = app.storage.load_document("board.base").unwrap();
    let document = super::format::BaseDocument::parse(baseline.source()).unwrap();
    let source = document
        .set_view_field(
            0,
            "groupOrder",
            Some(&serde_yaml_ng::from_str("[5, 2, null]").unwrap()),
        )
        .unwrap();
    app.storage
        .save_document(&baseline, source.source())
        .unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    let state = app.bases_state.as_ref().unwrap();
    assert!(
        state.group_collapsed(1),
        "layout {:?}, groups {:?}, collapsed {:?}",
        state.layout(),
        state
            .result()
            .unwrap()
            .groups
            .iter()
            .map(|group| &group.value)
            .collect::<Vec<_>>(),
        (0..state.result().unwrap().groups.len())
            .map(|group| state.group_collapsed(group))
            .collect::<Vec<_>>()
    );
    assert!(!state.group_collapsed(0));
    assert!(matches!(
        state.items[state.selected],
        super::state::Item::Group(1)
    ));
}

#[test]
fn search_reuses_notes_widget_for_every_layout_and_preserves_public_bytes() {
    let (_root, mut app) = fixture();
    app.storage
        .create_document("other.md", "---\nscore: 5\n---\nUnique body text\n")
        .unwrap();
    let public = app
        .storage
        .load_document("public.md")
        .unwrap()
        .source()
        .to_owned();
    for layout in ["table", "list", "cards", "kanban"] {
        let baseline = app.storage.load_document("board.base").unwrap();
        app.storage.save_document(&baseline,&format!("filters: file.ext == 'md'\nviews: [{{type: {layout}, name: Native, order: [file.name, score], groupBy: {{property: score}}, groupOrder: [2, 5, null]}}]\n")).unwrap();
        key(&mut app, KeyCode::Char('r'));
        idle(&mut app);
        let before = app
            .storage
            .load_document("board.base")
            .unwrap()
            .source()
            .to_owned();
        key(&mut app, KeyCode::Char('/'));
        assert!(render(&mut app).contains("Find:"));
        event(&mut app, Event::Paste("missing".into()));
        key(&mut app, KeyCode::Enter); // response not installed: never select stale row
        assert!(app.bases_state.as_ref().unwrap().search_popup.is_some());
        app.bases_state
            .as_mut()
            .unwrap()
            .search_popup
            .as_mut()
            .unwrap()
            .input
            .select_all();
        event(&mut app, Event::Paste("PuB".into()));
        event(&mut app, Event::Paste("LIC".into()));
        idle(&mut app);
        let state = app.bases_state.as_ref().unwrap();
        assert_eq!(state.search_text, "PuBLIC");
        assert_eq!(state.result().unwrap().rows.len(), 1);
        assert_eq!(state.search_popup.as_ref().unwrap().query(), "PuBLIC");
        assert!(!render(&mut app).contains("private.clin"));
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            app.bases_state.as_ref().unwrap().row().unwrap().id,
            "public.md"
        );
        assert!(app.bases_state.as_ref().unwrap().search_popup.is_none());
        key(&mut app, KeyCode::Char('/'));
        app.bases_state
            .as_mut()
            .unwrap()
            .search_popup
            .as_mut()
            .unwrap()
            .input
            .select_all();
        event(&mut app, Event::Paste("Unique body text".into()));
        idle(&mut app);
        assert!(
            app.bases_state
                .as_ref()
                .unwrap()
                .result()
                .unwrap()
                .rows
                .is_empty()
        );
        assert!(render(&mut app).contains("No matches"));
        key(&mut app, KeyCode::Esc);
        idle(&mut app);
        assert_eq!(app.bases_state.as_ref().unwrap().search_text, "PuBLIC");
        assert_eq!(
            app.bases_state.as_ref().unwrap().row().unwrap().id,
            "public.md"
        );
        key(&mut app, KeyCode::Char('/'));
        app.bases_state
            .as_mut()
            .unwrap()
            .search_popup
            .as_mut()
            .unwrap()
            .input
            .select_all();
        key(&mut app, KeyCode::Backspace);
        idle(&mut app);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.bases_state.as_ref().unwrap().search_text, "");
        assert_eq!(
            app.bases_state
                .as_ref()
                .unwrap()
                .result()
                .unwrap()
                .rows
                .len(),
            2
        );
        assert_eq!(
            app.storage.load_document("board.base").unwrap().source(),
            before
        );
        assert_eq!(
            app.storage.load_document("public.md").unwrap().source(),
            public
        );
    }
}

#[test]
fn search_restores_collapsed_header_and_survives_public_metadata_refresh() {
    let (_root, mut app) = fixture();
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline,"filters: file.ext == 'md'\nviews: [{type: table, name: Native, order: [file.name], groupBy: {property: score}}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('/'));
    event(&mut app, Event::Paste("no matches".into()));
    idle(&mut app);
    key(&mut app, KeyCode::Esc);
    idle(&mut app);
    let state = app.bases_state.as_ref().unwrap();
    assert!(state.group_collapsed(0));
    assert!(matches!(
        state.items[state.selected],
        super::state::Item::Group(0)
    ));
    key(&mut app, KeyCode::Char('/'));
    event(&mut app, Event::Paste("public".into()));
    idle(&mut app);
    key(&mut app, KeyCode::Enter);
    assert!(!app.bases_state.as_ref().unwrap().group_collapsed(0));
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "public.md"
    );
    app.storage
        .create_document("public-new.md", "---\nscore: 2\n---\nBody")
        .unwrap();
    app.invalidate_bases();
    idle(&mut app);
    assert_eq!(
        app.bases_state
            .as_ref()
            .unwrap()
            .result()
            .unwrap()
            .rows
            .len(),
        2
    );
    assert_eq!(app.bases_state.as_ref().unwrap().search_text, "public");
    key(&mut app, KeyCode::Char('/'));
    let before = app
        .bases_state
        .as_ref()
        .unwrap()
        .search_popup
        .as_ref()
        .unwrap()
        .query();
    event(&mut app, Event::Paste("x".repeat(64 * 1024 + 1)));
    assert_eq!(
        app.bases_state
            .as_ref()
            .unwrap()
            .search_popup
            .as_ref()
            .unwrap()
            .query(),
        before
    );
    assert!(app.status.contains("64 KiB"));
}

#[test]
fn configuration_prompt_rejects_reloaded_source_instead_of_refreshing_baseline() {
    let (_root, mut app) = fixture();
    menu(&mut app, "Rename view");
    let baseline = app.storage.load_document("board.base").unwrap();
    let external = baseline.source().replace("opaque: true", "opaque: false");
    app.storage.save_document(&baseline, &external).unwrap();
    app.invalidate_bases();
    idle(&mut app);
    prompt_text(&mut app, "My view");
    assert!(app.status.contains("Bases document changed"));
    assert!(app.bases_state.as_ref().unwrap().dialog.is_some());
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        external
    );
}

#[test]
fn kanban_checkbox_moves_preserve_types_and_none_removes_property() {
    let (_root, mut app) = fixture();
    fs::create_dir_all(app.storage.notes_dir.join(".obsidian")).unwrap();
    fs::write(
        app.storage.notes_dir.join(".obsidian/types.json"),
        r#"{"types":{"done":"checkbox"}}"#,
    )
    .unwrap();
    let public = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(
            &public,
            &public
                .source()
                .replace("score: 2", "score: 2\r\ndone: false"),
        )
        .unwrap();
    board(&mut app, "done", "[false, true, null]");
    drag_card(&mut app, "public.md", 1);
    assert_eq!(
        properties(&app, "public.md").get("done").unwrap().as_bool(),
        Some(true)
    );
    drag_card(&mut app, "public.md", 2);
    assert!(!properties(&app, "public.md").contains_key("done"));
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Char('n'));
    prompt_text(&mut app, "unchecked.md");
    assert_eq!(
        properties(&app, "unchecked.md")
            .get("done")
            .unwrap()
            .as_bool(),
        Some(false)
    );
    key(&mut app, KeyCode::Char('m'));
    prompt_text(&mut app, "");
    assert!(!properties(&app, "unchecked.md").contains_key("done"));
}

#[test]
fn folder_columns_move_binary_batches_with_undo_redo_and_root_keyboard_target() {
    let (_root, mut app) = fixture();
    let original = "---\r\ntitle: Exact\r\n---\r\nBody\r\n\r\n";
    app.storage.create_document("A/note.md", original).unwrap();
    fs::write(app.storage.notes_dir.join("A/image.png"), [0, 255, 128, 42]).unwrap();
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline,"# retained\nplugin: {opaque: true}\nfilters: file.ext == 'md' || file.ext == 'png'\nviews: [{type: kanban, name: Folder, order: [file.name], groupBy: {property: file.folder}, groupOrder: [A, B, null]}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    app.bases_state
        .as_mut()
        .unwrap()
        .selected_ids
        .extend(["A/note.md".into(), "A/image.png".into()]);
    drag_card(&mut app, "A/note.md", 1);
    assert!(!app.storage.notes_dir.join("A/note.md").exists());
    assert_eq!(
        app.storage.load_document("B/note.md").unwrap().source(),
        original
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("B/image.png")).unwrap(),
        [0, 255, 128, 42]
    );
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "B/note.md"
    );
    key(&mut app, KeyCode::Char('u'));
    idle(&mut app);
    assert_eq!(
        app.storage.load_document("A/note.md").unwrap().source(),
        original
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("A/image.png")).unwrap(),
        [0, 255, 128, 42]
    );
    event(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
    );
    idle(&mut app);
    assert!(!app.storage.notes_dir.join("A/image.png").exists());
    key(&mut app, KeyCode::Char('m'));
    prompt_text(&mut app, "");
    assert_eq!(
        app.storage.load_document("note.md").unwrap().source(),
        original
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("image.png")).unwrap(),
        [0, 255, 128, 42]
    );
    assert!(!app.storage.notes_dir.join("B/note.md").exists());
    assert!(
        app.storage
            .load_document("board.base")
            .unwrap()
            .source()
            .contains("opaque: true")
    );
    assert_eq!(
        app.storage.recover_documents().unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        b"not a public document"
    );
}

#[test]
fn folder_column_collision_rejects_entire_batch_and_stale_undo_retains_new_bytes() {
    let (_root, mut app) = fixture();
    app.storage.create_document("A/one.md", "one").unwrap();
    app.storage.create_document("A/two.md", "two").unwrap();
    app.storage
        .create_document("B/two.md", "collision")
        .unwrap();
    board(&mut app, "file.folder", "[A, B, null]");
    app.bases_state
        .as_mut()
        .unwrap()
        .selected_ids
        .extend(["A/one.md".into(), "A/two.md".into()]);
    drag_card(&mut app, "A/one.md", 1);
    assert_eq!(
        app.storage.load_document("A/one.md").unwrap().source(),
        "one"
    );
    assert_eq!(
        app.storage.load_document("A/two.md").unwrap().source(),
        "two"
    );
    assert_eq!(
        app.storage.load_document("B/two.md").unwrap().source(),
        "collision"
    );
    assert!(app.status.contains("already exists"));
    app.bases_state.as_mut().unwrap().selected_ids.clear();
    drag_card(&mut app, "A/one.md", 1);
    let baseline = app.storage.load_document("B/one.md").unwrap();
    app.storage.save_document(&baseline, "external").unwrap();
    key(&mut app, KeyCode::Char('u'));
    idle(&mut app);
    assert_eq!(
        app.storage.load_document("B/one.md").unwrap().source(),
        "external"
    );
    assert!(!app.storage.notes_dir.join("A/one.md").exists());
    assert!(app.status.contains("changed"));
}

#[test]
fn kanban_date_columns_and_folder_creation_retain_native_types() {
    let (_root, mut app) = fixture();
    fs::create_dir_all(app.storage.notes_dir.join(".obsidian")).unwrap();
    fs::write(
        app.storage.notes_dir.join(".obsidian/types.json"),
        r#"{"types":{"when":"date"}}"#,
    )
    .unwrap();
    let public = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(
            &public,
            &public
                .source()
                .replace("score: 2", "score: 2\r\nwhen: 2024-01-01"),
        )
        .unwrap();
    board(&mut app, "when", "['2024-01-01', '2024-02-01']");
    assert_eq!(
        app.bases_state
            .as_ref()
            .unwrap()
            .result()
            .unwrap()
            .rows
            .len(),
        1
    );
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Char('l'));
    key(&mut app, KeyCode::Char('n'));
    prompt_text(&mut app, "february.md");
    assert_eq!(
        properties(&app, "february.md")
            .get("when")
            .unwrap()
            .as_str(),
        Some("2024-02-01")
    );
    drag_card(&mut app, "public.md", 1);
    assert_eq!(
        properties(&app, "public.md").get("when").unwrap().as_str(),
        Some("2024-02-01")
    );
    board(&mut app, "file.folder", "[null, Drafts]");
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Char('l'));
    key(&mut app, KeyCode::Char('n'));
    prompt_text(&mut app, "Drafts/created.md");
    assert_eq!(
        app.storage
            .load_document("Drafts/created.md")
            .unwrap()
            .source(),
        ""
    );
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "Drafts/created.md"
    );
}

#[test]
fn kanban_readonly_groups_reject_column_mutation_but_allow_menu_order() {
    let (_root, mut app) = fixture();
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline,"filters: file.ext == 'md'\nformulas: {Kind: file.ext}\nviews: [{type: kanban, name: Board, order: [file.name], groupBy: {property: formula.Kind}, groupOrder: [md, Empty]}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    let original = app
        .storage
        .load_document("public.md")
        .unwrap()
        .source()
        .to_owned();
    key(&mut app, KeyCode::Char('n'));
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
    assert!(app.status.contains("read-only"));
    drag_card(&mut app, "public.md", 1);
    assert!(app.status.contains("read-only"));
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        original
    );
    menu(&mut app, "Move focused group right");
    idle(&mut app);
    assert_eq!(
        app.bases_state.as_ref().unwrap().focused_group().unwrap().0,
        1
    );
}

#[test]
fn multiline_formula_input_applies_once_and_invalid_input_retains_source() {
    let (_root, mut app) = fixture();
    menu(&mut app, "Formula");
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
    menu(&mut app, "Formula");
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
fn cards_form_a_bounded_grid_and_keep_last_row_visible() {
    let (_root, mut app) = fixture();
    for index in 0..120 {
        app.storage
            .create_document(
                &format!("extra-{index:03}.md"),
                "---\nscore: 4\n---\nBody\n",
            )
            .unwrap();
    }
    let base = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&base,"filters: file.ext == 'md'\nviews: [{type: cards, name: Cards, order: [file.name, score]}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    render(&mut app);
    let state = app.bases_state.as_ref().unwrap();
    assert_eq!(state.result().unwrap().rows.len(), 121);
    key(&mut app, KeyCode::Home);
    render(&mut app);
    let state = app.bases_state.as_ref().unwrap();
    assert!(
        state.hit_rows.len() < 25,
        "render only visible cards, not whole dataset"
    );
    assert_eq!(state.hit_rows[0].0.y, state.hit_rows[1].0.y);
    assert!(state.hit_rows[0].0.x < state.hit_rows[1].0.x);
    key(&mut app, KeyCode::End);
    render(&mut app);
    let state = app.bases_state.as_ref().unwrap();
    assert!(
        state
            .hit_rows
            .iter()
            .any(|(rect, position, _)| *position == state.selected
                && rect.intersection(AREA) == *rect)
    );
}

#[test]
fn public_image_activation_uses_native_preview_and_preserves_bytes() {
    let (_root, mut app) = fixture();
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.config.features.images = FeatureState::Enabled;
    let path = app.storage.notes_dir.join("picture.png");
    image::RgbaImage::from_pixel(32, 16, image::Rgba([25, 180, 40, 255]))
        .save(&path)
        .unwrap();
    let original = fs::read(&path).unwrap();
    let base = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&base,"filters: file.ext == 'png'\nviews: [{type: table, name: Images, order: [file.name]}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    assert_eq!(
        app.bases_state.as_ref().unwrap().row().unwrap().id,
        "picture.png"
    );
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| {
        app.bases_state.as_ref().unwrap().dialog.is_some()
    });
    assert!(render(&mut app).contains("PUBLIC IMAGE"));
    assert_eq!(app.mode, ViewMode::Bases);
    key(&mut app, KeyCode::Char('q'));
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
    assert!(app.needs_full_redraw);
    assert_eq!(fs::read(&path).unwrap(), original);
    let base = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&base,"filters: file.ext == 'md'\nformulas: {cover: 'image(\"picture.png\")'}\nviews: [{type: table, name: Decorations, order: [formula.cover]}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| {
        app.bases_state.as_ref().unwrap().dialog.is_some()
    });
    assert!(render(&mut app).contains("PUBLIC IMAGE"));
    app.config.core.enable_key_sequences = true;
    let previous = app
        .keybinds
        .bases
        .insert(
            crate::keybinds::BasesAction::Back,
            vec![crate::keybinds::KeyCombo::parse("z z").unwrap()],
        )
        .unwrap();
    key(&mut app, KeyCode::Char('z'));
    assert!(app.bases_state.as_ref().unwrap().dialog.is_some());
    key(&mut app, KeyCode::Char('z'));
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
    app.keybinds
        .bases
        .insert(crate::keybinds::BasesAction::Back, previous);
    app.config.features.images = FeatureState::Disabled;
    key(&mut app, KeyCode::Enter);
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    assert!(app.status.contains("enable images"));
    assert!(app.storage.load_public_attachment("private.clin").is_err());
    fs::create_dir_all(app.storage.notes_dir.join(".private")).unwrap();
    fs::write(
        app.storage.notes_dir.join(".private/picture.png"),
        &original,
    )
    .unwrap();
    assert!(
        app.storage
            .load_public_attachment(".private/picture.png")
            .is_err()
    );
    fs::write(
        app.storage.notes_dir.join("large.png"),
        vec![0; 8 * 1024 * 1024 + 1],
    )
    .unwrap();
    assert!(app.storage.load_public_attachment("large.png").is_err());
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("image.png"), &original).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("image.png"),
            app.storage.notes_dir.join("escape.png"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            app.storage.notes_dir.join("private.clin"),
            app.storage.notes_dir.join("private-alias.png"),
        )
        .unwrap();
        assert!(app.storage.load_public_attachment("escape.png").is_err());
        assert!(
            app.storage
                .load_public_attachment("private-alias.png")
                .is_err()
        );
    }
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
