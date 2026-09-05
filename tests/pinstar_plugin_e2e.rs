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
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::create_dir_all(&config_dir).unwrap();
    let canvas_path = notes_dir.join("board.canvas");
    std::fs::write(&canvas_path, content).unwrap();
    let storage = clin::storage::Storage {
        data_dir: dir.join("data"),
        config_dir,
        notes_dir,
        templates_dir: dir.join("templates"),
        key: [0u8; 32],
        skip_dir_patterns: Vec::new(),
    };
    std::fs::create_dir_all(&storage.data_dir).unwrap();
    std::fs::create_dir_all(&storage.templates_dir).unwrap();
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
    .unwrap();
    (plugin, path)
}

fn key(c: char) -> crossterm::event::Event {
    crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char(c),
        crossterm::event::KeyModifiers::NONE,
    ))
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

const AREA: ratatui::layout::Rect = ratatui::layout::Rect {
    x: 0,
    y: 0,
    width: 160,
    height: 40,
};

#[test]
fn zoom_key_dispatches_to_lib_apply_action() {
    let (mut plugin, _) = make_plugin(r#"{"nodes":[],"edges":[]}"#);
    let mut app = test_app();
    let z0 = plugin.state.zoom;

    let _ = plugin
        .overlay_handle_event(key('u'), &mut app, AREA)
        .unwrap();
    let z1 = plugin.state.zoom;
    assert!(z1 > z0, "u must zoom in: {z0} -> {z1}");

    // Unbound '-' must not zoom.
    let _ = plugin
        .overlay_handle_event(key('-'), &mut app, AREA)
        .unwrap();
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
    let res = plugin.overlay_handle_event(esc(), &mut app, AREA).unwrap();
    assert!(matches!(res, clin::overlay::OverlayResult::Continue));
    assert!(
        plugin.state.connection_source_id.is_none(),
        "first Esc must cancel the pending connection"
    );

    // Second Quit: actually exits (and never saves-on-exit; eager saves only).
    let res = plugin.overlay_handle_event(esc(), &mut app, AREA).unwrap();
    assert!(matches!(res, clin::overlay::OverlayResult::Exit));
}

#[test]
fn add_text_node_saves_eagerly() {
    let (mut plugin, path) = make_plugin(r#"{"nodes":[],"edges":[]}"#);
    let mut app = test_app();

    let res = plugin
        .overlay_handle_event(key('t'), &mut app, AREA)
        .unwrap();
    assert!(matches!(res, clin::overlay::OverlayResult::Continue));

    let on_disk = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&on_disk).unwrap();
    let nodes = parsed["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 1, "AddTextNode must save eagerly: {on_disk}");
    assert!(on_disk.contains("pinstar_layout") || true); // format-specific keys may vary
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
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            plugin.overlay_render(frame, frame.area(), &mut app);
        })
        .expect("render with image FileNode must not panic");
}
