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
        .draw(|frame| plugin.overlay_render(frame, Rect::new(10, 3, 80, 23), &mut app))
        .expect("render engine above the host footer");
    let point = draw::input::screen_to_canvas(49, 14, &plugin.state);
    assert!((point.0 + 1.25).abs() < 1e-10 && point.1.abs() < 1e-10);
    let original = std::fs::read(&path).expect("saved data");
    std::fs::remove_file(&path).expect("block save destination");
    std::fs::create_dir(&path).expect("blocking directory");
    let result = plugin
        .overlay_handle_event(key(KeyCode::Char('q')), &mut app, area)
        .expect("recoverable save error");
    assert!(matches!(result, OverlayResult::Continue));
    assert!(app.status.contains("failed"));
    assert_eq!(plugin.state.data.elements.len(), 1);
    std::fs::remove_dir(&path).expect("restore destination");
    std::fs::write(&path, original).expect("restore saved bytes");
    let result = plugin
        .overlay_handle_event(key(KeyCode::Char('q')), &mut app, area)
        .expect("saved departure");
    assert!(matches!(result, OverlayResult::Exit));
}

#[test]
fn drawing_saves_keep_permissions_and_reject_external_and_cooperating_writes() -> anyhow::Result<()>
{
    let root = tempfile::tempdir()?;
    let storage = clin::storage::Storage {
        data_dir: root.path().into(),
        notes_dir: root.path().into(),
        config_dir: root.path().join("config"),
        templates_dir: root.path().join("templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: clin::frontmatter::WritePolicy::default(),
    };
    let path = root.path().join("art.draw");
    draw::DrawState::create(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    let mut plugin =
        DrawPlugin::load(&storage, "art.draw", Keybinds::default(), KeyMatcher::new())?;
    plugin.state.data.width = 2048.0;
    plugin.state.save()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
    }
    let saved = std::fs::read(&path)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.path().join(".clin/write.lock"))?;
    lock.lock()?;
    plugin.state.data.width = 3072.0;
    assert!(plugin.state.save().is_err());
    assert_eq!(std::fs::read(&path)?, saved);
    drop(lock);
    plugin.state.save()?;
    let external = serde_json::to_vec(&draw::state::DrawData::default())?;
    std::fs::write(&path, &external)?;
    assert!(
        plugin
            .save_for_navigation()
            .expect_err("external changes must block navigation")
            .to_string()
            .contains("changed on disk")
    );
    assert_eq!(std::fs::read(&path)?, external);
    assert_eq!(plugin.state.data.width, 3072.0);
    Ok(())
}
