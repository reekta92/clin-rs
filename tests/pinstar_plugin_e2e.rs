//! E2E: PinstarPlugin key dispatch drives the upstream pinstar lib
//! (keybind injection, eager canvas saves, connection-mode Quit fallback,
//! OCR-style file-node insertion + render).
use clin::keybinds::{CanvasAction, Keybinds};
use clin::overlay::OverlayView as _;
use clin::pinstar_adapter::PinstarPlugin;

fn temp_root() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("clin-pinstar-e2e-{n}"))
}

fn temp_canvas(content: &str) -> (std::path::PathBuf, clin::storage::Storage) {
    let dir = temp_root();
    let _ = std::fs::remove_dir_all(&dir);
    let notes_dir = dir.join("notes");
    let config_dir = dir.join(".clin");
    std::fs::create_dir_all(&notes_dir).expect("e2e fixture");
    std::fs::create_dir_all(&config_dir).expect("e2e fixture");
    let canvas_path = notes_dir.join("board.canvas");
    std::fs::write(&canvas_path, content).expect("e2e fixture");
    let storage = clin::storage::Storage {
        data_dir: dir.join("data"),
        config_dir,
        notes_dir,
        templates_dir: dir.join("templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: clin::frontmatter::WritePolicy::default(),
    };
    std::fs::create_dir_all(&storage.data_dir).expect("e2e fixture");
    std::fs::create_dir_all(&storage.templates_dir).expect("e2e fixture");
    (canvas_path, storage)
}

fn make_plugin(canvas_content: &str) -> (PinstarPlugin, std::path::PathBuf) {
    let (path, storage) = temp_canvas(canvas_content);

    let config = clin::config::ClinConfig::default();
    let mut keybinds = Keybinds::default();
    keybinds.canvas.insert(
        CanvasAction::ZoomIn,
        vec![clin::keybinds::KeyCombo::simple(
            crossterm::event::KeyCode::Char('u'),
        )],
    );
    keybinds.canvas.remove(&CanvasAction::ZoomOut);
    keybinds.canvas.insert(
        CanvasAction::AddTextNode,
        vec![clin::keybinds::KeyCombo::simple(
            crossterm::event::KeyCode::Char('t'),
        )],
    );
    keybinds.canvas.insert(
        CanvasAction::CreateConnection,
        vec![clin::keybinds::KeyCombo::simple(
            crossterm::event::KeyCode::Char('c'),
        )],
    );
    keybinds.canvas.insert(
        CanvasAction::Quit,
        vec![clin::keybinds::KeyCombo::simple(
            crossterm::event::KeyCode::Esc,
        )],
    );

    let plugin = PinstarPlugin::new(
        &path,
        &config,
        keybinds,
        clin::keybinds::KeyMatcher::new(),
        None,
        &storage.data_dir,
    )
    .expect("e2e fixture");
    (plugin, path)
}

fn key(c: char) -> crossterm::event::Event {
    crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char(c),
        crossterm::event::KeyModifiers::NONE,
    ))
}

#[cfg(unix)]
#[test]
fn canvas_eager_saves_preserve_private_modes_and_leave_readonly_files_untouched()
-> anyhow::Result<()> {
    use ratatui::layout::Rect;
    use std::os::unix::fs::PermissionsExt;
    let (mut plugin, path) = make_plugin("{\"nodes\":[],\"edges\":[]}");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    let mut app = test_app();
    plugin.overlay_handle_event(key('t'), &mut app, Rect::new(0, 0, 80, 24))?;
    assert_eq!(
        std::fs::metadata(&path)?.permissions().mode() & 0o777,
        0o600
    );
    let saved = std::fs::read(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))?;
    plugin.overlay_handle_event(key('t'), &mut app, Rect::new(0, 0, 80, 24))?;
    assert_eq!(std::fs::read(&path)?, saved);
    assert!(app.status.contains("read-only"));
    assert_eq!(plugin.state.data.nodes.len(), 2);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn esc() -> crossterm::event::Event {
    crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
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
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: clin::frontmatter::WritePolicy::default(),
    };
    clin::app::App::new(storage).expect("e2e fixture")
}

const AREA: ratatui::layout::Rect = ratatui::layout::Rect {
    x: 0,
    y: 0,
    width: 160,
    height: 40,
};

#[test]
fn rename_hints_follow_host_bindings_and_ignore_main_footer_style() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};
    let (mut plugin, _) = make_plugin(r#"{"nodes":[],"edges":[]}"#);
    let mut app = test_app();
    plugin.keybinds.canvas.insert(
        CanvasAction::RenameConfirm,
        vec![clin::keybinds::KeyCombo::simple(KeyCode::F(7))],
    );
    plugin.keybinds.canvas.insert(
        CanvasAction::RenameCancel,
        vec![clin::keybinds::KeyCombo::simple(KeyCode::F(8))],
    );
    for background in [
        clin::config::Background::Solid,
        clin::config::Background::Transparent,
    ] {
        app.config.ui.background = background;
        for hint_style in clin::config::HintBarStyle::ALL {
            app.config.ui.hint_bar_style = hint_style;
            app.app_theme =
                clin::app_theme::AppThemeColors::from_config(&app.config.ui, &mut Vec::new());
            plugin.state.rename_popup = Some(ratatui_textarea::TextArea::default());
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| plugin.overlay_render(frame, frame.area(), &mut app))
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                text.contains("RENAME NODE")
                    && text.contains("F7 confirm")
                    && text.contains("F8 cancel"),
                "{text}"
            );
            assert_eq!(text.matches("confirm").count(), 1);
            assert!(!text.contains("Enter"));
            plugin
                .overlay_handle_event(
                    Event::Key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE)),
                    &mut app,
                    ratatui::layout::Rect::new(0, 0, 80, 24),
                )
                .unwrap();
            assert!(plugin.state.rename_popup.is_none());
        }
    }
}

#[test]
fn zoom_key_dispatches_to_lib_apply_action() {
    let (mut plugin, _) = make_plugin(r#"{"nodes":[],"edges":[]}"#);
    let mut app = test_app();
    let z0 = plugin.state.zoom;

    let _ = plugin
        .overlay_handle_event(key('u'), &mut app, AREA)
        .expect("e2e fixture");
    let z1 = plugin.state.zoom;
    assert!(z1 > z0, "u must zoom in: {z0} -> {z1}");

    // Unbound '-' must not zoom.
    let _ = plugin
        .overlay_handle_event(key('-'), &mut app, AREA)
        .expect("e2e fixture");
    assert_eq!(plugin.state.zoom, z1, "unbound '-' must not zoom");
}

#[test]
fn quit_cancels_connection_then_exits() {
    let (mut plugin, _) = make_plugin(
        r#"{"nodes":[{"type":"text","id":"a","x":0,"y":0,"width":10,"height":10,"text":"A"},
                    {"type":"text","id":"b","x":50,"y":0,"width":10,"height":10,"text":"B"}],
           "edges":[]}"#,
    );
    let mut app = test_app();

    plugin.state.selection.select_only("a".into());
    plugin.state.start_connection();
    assert!(plugin.state.connection_source_id.is_some());

    // First Quit: connection-mode fallback cancels the pending connection.
    let res = plugin
        .overlay_handle_event(esc(), &mut app, AREA)
        .expect("e2e fixture");
    assert!(matches!(res, clin::overlay::OverlayResult::Continue));
    assert!(
        plugin.state.connection_source_id.is_none(),
        "first Esc must cancel the pending connection"
    );

    // Second Quit: actually exits (and never saves-on-exit; eager saves only).
    let res = plugin
        .overlay_handle_event(esc(), &mut app, AREA)
        .expect("e2e fixture");
    assert!(matches!(res, clin::overlay::OverlayResult::Exit));
}

#[test]
fn add_text_node_saves_eagerly() {
    let (mut plugin, path) = make_plugin(r#"{"nodes":[],"edges":[]}"#);
    let mut app = test_app();

    let res = plugin
        .overlay_handle_event(key('t'), &mut app, AREA)
        .expect("e2e fixture");
    assert!(matches!(res, clin::overlay::OverlayResult::Continue));

    let on_disk = std::fs::read_to_string(&path).expect("e2e fixture");
    let parsed: serde_json::Value = serde_json::from_str(&on_disk).expect("e2e fixture");
    let nodes = parsed["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 1, "AddTextNode must save eagerly: {on_disk}");
}

#[test]
fn ocr_insert_file_node_renders() {
    let (mut plugin, _) = make_plugin(r#"{"nodes":[],"edges":[]}"#);
    let mut app = test_app();

    plugin
        .state
        .data
        .nodes
        .push(pinstar::data::CanvasNode::File(pinstar::data::FileNode {
            id: "img1".into(),
            x: -20.0,
            y: -10.0,
            width: 200.0,
            height: 100.0,
            file: "pics/shot.png".into(),
            subpath: None,
            title: Some("shot".into()),
            color: None,
        }));
    plugin.state.fit_to_view(AREA);

    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("e2e fixture");
    terminal
        .draw(|frame| {
            plugin.overlay_render(frame, frame.area(), &mut app);
        })
        .expect("render with image FileNode must not panic");
}
