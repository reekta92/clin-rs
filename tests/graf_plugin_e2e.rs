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
    std::fs::create_dir_all(&notes_dir).expect("e2e fixture");
    std::fs::create_dir_all(&config_dir).expect("e2e fixture");
    std::fs::write(notes_dir.join("a.md"), "link [[b]]").expect("e2e fixture");
    std::fs::write(notes_dir.join("b.md"), "back [[a]]").expect("e2e fixture");

    let storage = clin::storage::Storage {
        data_dir: dir.join("data"),
        config_dir: config_dir.clone(),
        notes_dir,
        templates_dir: dir.join("templates"),
        key: Default::default(),
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: true,
    };
    std::fs::create_dir_all(&storage.data_dir).expect("e2e fixture");
    std::fs::create_dir_all(&storage.templates_dir).expect("e2e fixture");

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
    .expect("e2e fixture");
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
    std::fs::create_dir_all(&notes_dir).expect("e2e fixture");
    std::fs::create_dir_all(&config_dir).expect("e2e fixture");
    let storage = clin::storage::Storage {
        data_dir: dir.join("data"),
        config_dir,
        notes_dir,
        templates_dir: dir.join("templates"),
        key: Default::default(),
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: true,
    };
    clin::app::App::new(storage).expect("e2e fixture")
}

fn zoom(plugin: &GrafPlugin) -> f64 {
    plugin
        .graph_state
        .as_ref()
        .expect("e2e fixture")
        .read()
        .viewport
        .zoom
}

#[test]
fn zoom_key_dispatches_to_lib_apply_action() {
    let (mut plugin, _) = make_plugin();
    let mut app = test_app();
    let area = ratatui::layout::Rect::new(0, 0, 160, 40);
    let z0 = zoom(&plugin);

    let res = plugin
        .overlay_handle_event(key('u'), &mut app, area)
        .expect("e2e fixture");
    let z1 = zoom(&plugin);

    println!("result={res:?} zoom {z0} -> {z1}");
    assert!(z1 > z0, "u must zoom in: {z0} -> {z1}");

    // Unbound '-' must not zoom.
    let _ = plugin
        .overlay_handle_event(key('-'), &mut app, area)
        .expect("e2e fixture");
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
        .expect("e2e fixture");
    let _ = plugin
        .overlay_handle_event(key('c'), &mut app, area)
        .expect("e2e fixture");
    {
        let gs = plugin.graph_state.as_ref().expect("e2e fixture");
        assert!(
            gs.read().connection_source.is_some(),
            "connection mode armed"
        );
    }
    // Fit both nodes on screen before scanning for the target cell.
    let _ = plugin
        .overlay_handle_event(key('a'), &mut app, area)
        .expect("e2e fixture");

    // Find a screen cell whose hit_test resolves to the OTHER node. The
    // physics thread keeps moving nodes and graf's AutoFit fits a fixed
    // 200-unit square (canvas-agnostic), so under parallel-test CPU
    // contention the fit can leave a tall bbox offscreen. Retry for a
    // bounded time; on each miss, re-center on the node bbox and apply an
    // aspect-correct zoom (f64 cells per world unit on both axes).
    let (target_col, target_row) = {
        let mut found = None;
        for _ in 0..100 {
            let attempt = {
                let gs = plugin.graph_state.as_ref().expect("e2e fixture");
                let mut guard = gs.write();
                let graph = guard.simulation.get_graph();
                let selected = guard.selection.primary.expect("e2e fixture");
                let other = graph
                    .node_indices()
                    .find(|i| *i != selected)
                    .expect("e2e fixture");
                let outer = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(area);
                let canvas = graf::canvas_area(outer[1], true);
                // Aspect-correct fit: center on bbox, zoom = cells per world
                // unit bounded by both canvas axes with 25% margin.
                let (mut min_x, mut max_x, mut min_y, mut max_y) =
                    (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
                for n in graph.node_weights() {
                    min_x = min_x.min(n.location.x as f64);
                    max_x = max_x.max(n.location.x as f64);
                    min_y = min_y.min(n.location.y as f64);
                    max_y = max_y.max(n.location.y as f64);
                }
                let range_x = (max_x - min_x).max(1.0);
                let range_y = (max_y - min_y).max(1.0);
                let fit = 0.75
                    * (f64::from(canvas.width) / range_x).min(f64::from(canvas.height) / range_y);
                let vp = &mut guard.viewport;
                vp.center_x = (min_x + max_x) / 2.0;
                vp.center_y = (min_y + max_y) / 2.0;
                vp.zoom = fit;
                vp.auto_fit_zoom = fit;

                let settings = clin::graf_adapter::clin_settings(&config);
                let max_lc = guard.render_cache.lock().max_link_count;
                let mut hit = None;
                'scan: for row in canvas.y..canvas.bottom() {
                    for col in canvas.x..canvas.right() {
                        let (wx, wy) = guard.viewport.screen_to_world(col, row, canvas);
                        if guard
                            .viewport
                            .hit_test(wx, wy, &guard, &settings, canvas, max_lc)
                            == Some(other)
                        {
                            hit = Some((col, row));
                            break 'scan;
                        }
                    }
                }
                hit
            };
            if let Some(cell) = attempt {
                found = Some(cell);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        found.expect("target node must be on screen somewhere")
    };
    let click = crossterm::event::Event::Mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: target_col,
        row: target_row,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    let res = plugin
        .overlay_handle_event(click, &mut app, area)
        .expect("e2e fixture");

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

#[test]
fn feature_view_files_graph_preview_same_key() {
    let td = tempfile::tempdir().unwrap();
    let dir = td.path().to_path_buf();
    let notes_dir = dir.join("notes");
    let config_dir = dir.join(".clin");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(notes_dir.join("plain.md"), "plain text").unwrap();
    let canvas_data = r#"{"nodes":[{"type":"text","id":"a","x":0,"y":0,"width":100,"height":100,"text":""}],"edges":[]}"#;
    std::fs::write(notes_dir.join("canvas.canvas"), canvas_data).unwrap();
    let draw_data = r#"{"version":2,"width":500,"height":500,"elements":[]}"#;
    std::fs::write(notes_dir.join("draw.draw"), draw_data).unwrap();

    let storage = clin::storage::Storage {
        data_dir: dir.join("data"),
        config_dir: config_dir.clone(),
        notes_dir,
        templates_dir: dir.join("templates"),
        key: Default::default(),
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: true,
    };
    std::fs::create_dir_all(&storage.data_dir).unwrap();
    std::fs::create_dir_all(&storage.templates_dir).unwrap();

    let mut config = clin::config::ClinConfig::default();
    config.graf.filter.show_orphan = true;
    config.graf.preview_enabled = true;

    let notes = vec![
        clin::storage::NoteSummary {
            id: "plain.md".into(),
            title: "plain".into(),
            updated_at: 0,
            folder: "".into(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 0,
        },
        clin::storage::NoteSummary {
            id: "canvas.canvas".into(),
            title: "canvas".into(),
            updated_at: 0,
            folder: "".into(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 0,
        },
        clin::storage::NoteSummary {
            id: "draw.draw".into(),
            title: "draw".into(),
            updated_at: 0,
            folder: "".into(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 0,
        },
    ];
    let mut plugin = GrafPlugin::new(
        &config,
        storage,
        notes.clone(),
        vec![],
        Keybinds::default(),
        clin::keybinds::KeyMatcher::new(),
    )
    .unwrap();
    plugin.last_preview_pane_width = 80;
    plugin.last_preview_pane_height = 24;

    // Wait for layout so nodes exist
    std::thread::sleep(std::time::Duration::from_millis(50));
    let plain_idx = notes.iter().position(|n| n.id == "plain.md").unwrap();
    let canvas_idx = notes.iter().position(|n| n.id == "canvas.canvas").unwrap();
    let draw_idx = notes.iter().position(|n| n.id == "draw.draw").unwrap();

    // 1. Select Draw, warm preview
    plugin
        .graph_state
        .as_ref()
        .unwrap()
        .write()
        .selection
        .select_only(fdg_sim::petgraph::graph::NodeIndex::new(draw_idx));
    plugin.sync_preview(&config);
    assert!(matches!(
        plugin.preview_content,
        Some(clin::list_view::PreviewContent::DrawGrid { .. })
    ));

    // Disable Draw
    config.features.draw_view = clin::config::FeatureState::Disabled;
    plugin.sync_preview(&config);
    assert!(plugin.preview_content.is_none());

    // Re-enable Draw -> returns without reselection
    config.features.draw_view = clin::config::FeatureState::Enabled;
    plugin.sync_preview(&config);
    assert!(matches!(
        plugin.preview_content,
        Some(clin::list_view::PreviewContent::DrawGrid { .. })
    ));

    // 2. Select Canvas, warm preview
    plugin
        .graph_state
        .as_ref()
        .unwrap()
        .write()
        .selection
        .select_only(fdg_sim::petgraph::graph::NodeIndex::new(canvas_idx));
    plugin.sync_preview(&config);
    assert!(matches!(
        plugin.preview_content,
        Some(clin::list_view::PreviewContent::CanvasGrid { .. })
    ));

    // Deleted Canvas
    config.features.canvas_view = clin::config::FeatureState::Deleted;
    plugin.sync_preview(&config);
    assert!(plugin.preview_content.is_none());

    // Re-enable Canvas -> returns without reselection
    config.features.canvas_view = clin::config::FeatureState::Enabled;
    plugin.sync_preview(&config);
    assert!(matches!(
        plugin.preview_content,
        Some(clin::list_view::PreviewContent::CanvasGrid { .. })
    ));

    // 3. Rebuild graph with flags disabled
    config.features.draw_view = clin::config::FeatureState::Disabled;
    config.features.canvas_view = clin::config::FeatureState::Deleted;
    plugin.refresh_simulation(&config);
    std::thread::sleep(std::time::Duration::from_millis(50));
    {
        let guard = plugin.graph_state.as_ref().unwrap().read();
        let graph = guard.simulation.get_graph();
        let remaining_ids: std::collections::HashSet<_> =
            graph.node_weights().map(|n| n.data.id.clone()).collect();
        assert_eq!(remaining_ids.len(), 1);
        assert!(remaining_ids.contains("plain.md"));
    }

    // 4. All-view-owned nodes hidden case
    plugin.notes.retain(|n| n.id != "plain.md"); // only view-owned nodes left in source
    plugin.refresh_simulation(&config); // error rebuild since nothing is visible
    assert!(
        plugin.graph_state.is_none()
            || plugin
                .graph_state
                .as_ref()
                .unwrap()
                .read()
                .simulation
                .get_graph()
                .node_count()
                == 0
    );
    assert!(plugin.preview_content.is_none());

    // Explicitly shut down physics
    if let Some(tx) = plugin.graph_kill_tx.take() {
        let _ = tx.send(());
    }
}
