//! E2E: GrafPlugin key/mouse dispatch drives the upstream graf lib
//! (keybind injection, connection-mode disk writes, folder preview).
use clin::graf_adapter::GrafPlugin;
use clin::keybinds::{GraphAction, Keybinds};
use clin::overlay::OverlayView as _;

fn temp_root() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("clin-dispatch-repro-{n}"))
}

fn make_plugin() -> (GrafPlugin, std::path::PathBuf) {
    let dir = temp_root();
    let _ = std::fs::remove_dir_all(&dir);
    let notes_dir = dir.join("notes");
    let config_dir = dir.join(".clin");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(notes_dir.join("a.md"), "link [[b]]").unwrap();
    std::fs::write(notes_dir.join("b.md"), "back [[a]]").unwrap();

    let storage = clin::storage::Storage {
        data_dir: dir.join("data"),
        config_dir: config_dir.clone(),
        notes_dir,
        templates_dir: dir.join("templates"),
        key: [0u8; 32],
        skip_dir_patterns: Vec::new(),
    };
    std::fs::create_dir_all(&storage.data_dir).unwrap();
    std::fs::create_dir_all(&storage.templates_dir).unwrap();

    let mut config = clin::config::ClinConfig::default();
    config.graf.filter.show_orphan = true;
    let mut keybinds = Keybinds::default();
    keybinds.graph.insert(
        GraphAction::ZoomIn,
        vec![clin::keybinds::KeyCombo::simple(
            crossterm::event::KeyCode::Char('u'),
        )],
    );
    keybinds.graph.remove(&GraphAction::ZoomOut);
    let notes = vec![
        clin::storage::NoteSummary {
            id: "a.md".into(),
            title: "a".into(),
            updated_at: 0,
            folder: "".into(),
            tags: vec![],
            pinned: false,
            links: vec!["b".into()],
            size_bytes: 0,
        },
        clin::storage::NoteSummary {
            id: "b.md".into(),
            title: "b".into(),
            updated_at: 0,
            folder: "".into(),
            tags: vec![],
            pinned: false,
            links: vec!["a".into()],
            size_bytes: 0,
        },
    ];
    let plugin = GrafPlugin::new(
        &config,
        storage,
        notes,
        vec![],
        keybinds,
        clin::keybinds::KeyMatcher::new(),
    )
    .unwrap();
    (plugin, dir)
}

fn key(c: char) -> crossterm::event::Event {
    crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char(c),
        crossterm::event::KeyModifiers::NONE,
    ))
}

fn test_app() -> clin::app::App {
    let dir = temp_root();
    let notes_dir = dir.join("notes");
    let config_dir = dir.join(".clin");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::create_dir_all(&config_dir).unwrap();
    let storage = clin::storage::Storage {
        data_dir: dir.join("data"),
        config_dir,
        notes_dir,
        templates_dir: dir.join("templates"),
        key: [0u8; 32],
        skip_dir_patterns: Vec::new(),
    };
    clin::app::App::new(storage).unwrap()
}

fn zoom(plugin: &GrafPlugin) -> f64 {
    plugin.graph_state.as_ref().unwrap().read().viewport.zoom
}

#[test]
fn zoom_key_dispatches_to_lib_apply_action() {
    let (mut plugin, _) = make_plugin();
    let mut app = test_app();
    let area = ratatui::layout::Rect::new(0, 0, 160, 40);
    let z0 = zoom(&plugin);

    let res = plugin
        .overlay_handle_event(key('u'), &mut app, area)
        .unwrap();
    let z1 = zoom(&plugin);

    println!("result={res:?} zoom {z0} -> {z1}");
    assert!(z1 > z0, "u must zoom in: {z0} -> {z1}");

    // Unbound '-' must not zoom.
    let _ = plugin
        .overlay_handle_event(key('-'), &mut app, area)
        .unwrap();
    let z2 = zoom(&plugin);
    assert_eq!(z1, z2, "unbound '-' must not zoom");
}

#[test]
fn connection_mode_writes_wikilink_to_disk() {
    let (mut plugin, dir) = make_plugin();
    let config = clin::config::ClinConfig::default();
    let mut app = test_app();
    let area = ratatui::layout::Rect::new(0, 0, 160, 40);

    // Select nearest node via pan, then arm connection mode via default 'c'.
    let _ = plugin
        .overlay_handle_event(key('k'), &mut app, area)
        .unwrap();
    let _ = plugin
        .overlay_handle_event(key('c'), &mut app, area)
        .unwrap();
    {
        let gs = plugin.graph_state.as_ref().unwrap();
        assert!(
            gs.read().connection_source.is_some(),
            "connection mode armed"
        );
    }
    // Fit both nodes on screen before scanning for the target cell.
    let _ = plugin
        .overlay_handle_event(key('a'), &mut app, area)
        .unwrap();

    // Find a screen cell whose hit_test resolves to the OTHER node (scan the
    // canvas instead of trusting the projection).
    let (target_col, target_row) = {
        let gs = plugin.graph_state.as_ref().unwrap();
        let guard = gs.read();
        let graph = guard.simulation.get_graph();
        let selected = guard.selection.primary.unwrap();
        let other = graph.node_indices().find(|i| *i != selected).unwrap();
        let outer = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(0),
            ])
            .split(area);
        let canvas = graf::canvas_area(outer[1], true);
        let settings = clin::graf_adapter::clin_settings(&config);
        let max_lc = guard.render_cache.lock().max_link_count;
        let mut found = None;
        'scan: for row in canvas.y..canvas.bottom() {
            for col in canvas.x..canvas.right() {
                let (wx, wy) = guard.viewport.screen_to_world(col, row, canvas);
                if guard
                    .viewport
                    .hit_test(wx, wy, &guard, &settings, canvas, max_lc)
                    == Some(other)
                {
                    found = Some((col, row));
                    break 'scan;
                }
            }
        }
        let (col, row) = found.expect("target node must be on screen somewhere");
        println!("clicking target {other:?} at ({col},{row})");
        (col, row)
    };
    let click = crossterm::event::Event::Mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: target_col,
        row: target_row,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    let res = plugin.overlay_handle_event(click, &mut app, area).unwrap();

    assert!(
        matches!(res, clin::overlay::OverlayResult::NoteModified(_)),
        "expected NoteModified, got {res:?}"
    );
    let wrote_link = ["a.md", "b.md"].iter().any(|f| {
        std::fs::read_to_string(dir.join("notes").join(f))
            .map(|c| c.contains("[["))
            .unwrap_or(false)
    });
    assert!(wrote_link, "wikilink must be written to a note file");
}

#[test]
fn list_folder_preview_builds_and_settles() {
    let mut app = test_app();
    app.config.list.folder_graph_preview = true;
    app.ensure_graph_preview();
    let gs = app.graph_preview.as_mut().expect("preview graph built");
    assert!(!gs.is_settled, "fresh simulation starts unsettled");
    let (min_x, max_x, _, _) = gs.graph_bounds;
    assert!(
        max_x - min_x > 0.0,
        "preview graph must have non-degenerate bounds"
    );

    // Drive the same steps the list-view preview loop runs (cap 100).
    for _ in 0..100 {
        if !gs.is_settled {
            graf::simulation_step(gs, 0.12);
        }
    }
    assert!(
        gs.is_settled,
        "simulation must settle within the UI step cap"
    );
}
