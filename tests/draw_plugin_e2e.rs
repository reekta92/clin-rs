//! The clin adapter resolves host keys and persists through the upstream engine.
use clin::{
    draw_adapter::DrawPlugin,
    keybinds::{DrawAction, KeyCombo, KeyMatcher, Keybinds},
    overlay::{OverlayResult, OverlayView},
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn custom_keys_render_save_reopen_and_failed_departure() {
    let root = tempfile::tempdir().expect("temporary vault");
    let storage = clin::storage::Storage {
        data_dir: root.path().join("vault"),
        notes_dir: root.path().join("vault"),
        config_dir: root.path().join("vault/.clin"),
        templates_dir: root.path().join("templates"),
        skip_dir_patterns: vec![],
        frontmatter_policy: clin::frontmatter::WritePolicy::default(),
    };
    for path in [
        &storage.notes_dir,
        &storage.config_dir,
        &storage.templates_dir,
    ] {
        std::fs::create_dir_all(path).expect("vault directories");
    }
    let path = storage.notes_dir.join("art.draw");
    draw::DrawState::create(&path).expect("new drawing");
    let mut keys = Keybinds::default();
    keys.draw.insert(
        DrawAction::SelectDrawTool,
        vec![KeyCombo::simple(KeyCode::Char('u'))],
    );
    let mut plugin =
        DrawPlugin::load(&storage, "art.draw", keys, KeyMatcher::new()).expect("load drawing");
    assert!(
        DrawPlugin::load(
            &storage,
            "../outside.draw",
            Keybinds::default(),
            KeyMatcher::new()
        )
        .is_err()
    );
    let mut app = clin::app::App::new(storage.clone()).expect("application");
    let area = Rect::new(10, 2, 80, 25);
    let key = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
    plugin
        .overlay_handle_event(key(KeyCode::Char('d')), &mut app, area)
        .expect("unbound key");
    assert_eq!(plugin.state.active_tool, draw::DrawTool::Cursor);
    plugin
        .overlay_handle_event(key(KeyCode::Char('u')), &mut app, area)
        .expect("custom binding");
    assert_eq!(plugin.state.active_tool, draw::DrawTool::Draw);
    for (kind, column, row) in [
        (MouseEventKind::Down(MouseButton::Left), 30, 10),
        (MouseEventKind::Drag(MouseButton::Left), 50, 15),
        (MouseEventKind::Up(MouseButton::Left), 50, 15),
    ] {
        plugin
            .overlay_handle_event(
                Event::Mouse(MouseEvent {
                    kind,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                }),
                &mut app,
                area,
            )
            .expect("drawing gesture");
    }
    assert_eq!(plugin.state.undo_stack.len(), 1);
    let reopened = DrawPlugin::load(&storage, "art.draw", Keybinds::default(), KeyMatcher::new())
        .expect("reopen");
    assert_eq!(reopened.state.data, plugin.state.data);
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).expect("render backend");
    terminal
        .draw(|frame| plugin.overlay_render(frame, Rect::new(10, 3, 80, 24), &mut app))
        .expect("render engine and footer");
    assert_eq!(plugin.state.last_area, Rect::new(10, 3, 80, 23));
    let original = std::fs::read(&path).expect("saved data");
    std::fs::remove_file(&path).expect("block save destination");
    std::fs::create_dir(&path).expect("blocking directory");
    let result = plugin
        .overlay_handle_event(key(KeyCode::Char('q')), &mut app, area)
        .expect("recoverable save error");
    assert!(matches!(result, OverlayResult::Continue));
    assert!(app.status.contains("failed to publish"));
    assert_eq!(plugin.state.data.elements.len(), 1);
    std::fs::remove_dir(&path).expect("restore destination");
    std::fs::write(&path, original).expect("restore saved bytes");
    let result = plugin
        .overlay_handle_event(key(KeyCode::Char('q')), &mut app, area)
        .expect("saved departure");
    assert!(matches!(result, OverlayResult::Exit));
}
