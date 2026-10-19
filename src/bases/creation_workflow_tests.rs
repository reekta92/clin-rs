use super::*;
use crate::popups::{ActivePopup, CreateFormatPopup, NoteFormat};

fn popup_key(app: &mut App, code: KeyCode) {
    assert!(crate::events::handle_global_popups_and_palette(
        app,
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        AREA
    ));
}
fn name(app: &mut App, value: &str) {
    let Some(ActivePopup::CreateNote(popup, NoteFormat::Base)) = app.popups.active.as_mut() else {
        panic!("expected native Bases filename popup");
    };
    popup.input.insert_str(value);
    popup_key(app, KeyCode::Enter);
}
#[test]
fn notes_format_picker_and_palette_create_public_bases_without_note_serialization() {
    let (_root, mut app) = fixture();
    let original = app.storage.load_document("public.md").unwrap();
    app.bases_state = None;
    app.mode = ViewMode::List;
    app.begin_create_select_format_in_folder("boards".into());
    for _ in 1..CreateFormatPopup::COUNT {
        popup_key(&mut app, KeyCode::Down);
    }
    let Some(ActivePopup::CreateFormat(popup)) = &app.popups.active else {
        panic!("expected format popup");
    };
    assert_eq!(popup.selected, 4);
    popup_key(&mut app, KeyCode::Enter);
    name(&mut app, "Created");
    wait(&mut app, |app| {
        app.bases_state.as_ref().is_some_and(|state| {
            state.write_task.is_none() && state.result().is_some() && !state.loading
        })
    });
    let created = app.storage.load_document("boards/Created.base").unwrap();
    assert_eq!(
        created.source(),
        super::super::format::BaseDocument::new_table().source()
    );
    assert_eq!(app.mode, ViewMode::Bases);
    assert_eq!(
        app.bases_state.as_ref().unwrap().previous_mode,
        ViewMode::List
    );
    assert!(!created.source().contains("clin_"));
    assert!(!created.source().contains("title:"));
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        original.source()
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        b"not a public document"
    );
    assert!(
        crate::actions::get_all_action_infos(&app)
            .iter()
            .any(|action| action.id == "bases.create")
    );
    crate::actions::execute_action("bases.create", &mut app, None).unwrap();
    if let Some(ActivePopup::CreateNote(popup, NoteFormat::Base)) = &app.popups.active {
        assert_eq!(popup.folder, "boards");
    }
    assert!(matches!(
        app.popups.active,
        Some(ActivePopup::CreateNote(_, NoteFormat::Base))
    ));
    popup_key(&mut app, KeyCode::Esc);
    assert!(app.popups.active.is_none());
    app.config.features.bases_view = FeatureState::Disabled;
    assert!(crate::actions::execute_action("bases.create", &mut app, None).is_err());
    assert!(
        !crate::actions::get_all_action_infos(&app)
            .iter()
            .any(|action| action.id == "bases.create")
    );
}
#[test]
fn base_creation_collision_and_private_paths_preserve_all_existing_bytes() {
    let (_root, mut app) = fixture();
    let baseline = app.storage.load_document("board.base").unwrap();
    app.bases_state = None;
    app.mode = ViewMode::List;
    app.begin_create_base_in_folder(String::new());
    name(&mut app, "board.base");
    wait(&mut app, |app| {
        app.bases_state
            .as_ref()
            .is_some_and(|state| state.write_task.is_none())
    });
    assert!(app.bases_state.as_ref().unwrap().error.is_some());
    assert!(app.bases_state.as_ref().unwrap().loaded.is_none());
    app.bases_refresh_requested = true;
    app.poll_bases();
    assert!(app.bases_state.as_ref().unwrap().error.is_some());
    assert!(app.bases_state.as_ref().unwrap().loaded.is_none());
    assert!(!app.bases_refresh_requested);
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        baseline.source()
    );
    for (folder, title) in [(".clin", "secret"), ("", "../escape"), ("", ".hidden")] {
        app.bases_state = None;
        app.mode = ViewMode::List;
        app.begin_create_base_in_folder(folder.into());
        name(&mut app, title);
        assert_eq!(app.mode, ViewMode::List);
        assert!(app.bases_state.is_none());
        assert!(app.status.contains("Cannot create Bases"));
    }
    assert!(!app.storage.notes_dir.join(".clin/secret.base").exists());
    assert!(
        !app.storage
            .notes_dir
            .parent()
            .unwrap()
            .join("escape.base")
            .exists()
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        b"not a public document"
    );
}
#[test]
fn format_picker_mouse_routes_base_row_and_quick_bindings_do_not_cover_bases_popups() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.mode = ViewMode::List;
    app.begin_create_select_format_in_folder(String::new());
    let area = crate::ui::centered_rect(crate::ui::PopupSize::Medium, AREA);
    let mouse = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: area.x + 1,
        row: area.y + 5,
        modifiers: KeyModifiers::NONE,
    };
    for _ in 0..2 {
        assert!(crate::events::handle_global_popup_mouse(
            &mut app, &mouse, AREA
        ));
    }
    assert!(matches!(
        app.popups.active,
        Some(ActivePopup::CreateNote(_, NoteFormat::Base))
    ));
    popup_key(&mut app, KeyCode::Esc);
    app.open_bases_view("board.base", None);
    idle(&mut app);
    for preset in [
        crate::config::KeybindPreset::Default,
        crate::config::KeybindPreset::Vim,
        crate::config::KeybindPreset::Helix,
        crate::config::KeybindPreset::Emacs,
    ] {
        let bindings = preset.base_keybinds();
        for action in [
            crate::keybinds::BasesAction::Search,
            crate::keybinds::BasesAction::OpenCover,
            crate::keybinds::BasesAction::NextProperty,
            crate::keybinds::BasesAction::PreviousProperty,
        ] {
            assert_ne!(bindings.bases_keys_display(action), "");
        }
    }
    let context = crate::statusline::StatuslineContext::for_view(&app, ViewMode::Bases);
    for key in ["bases_rows", "bases_matched", "note_count", "visual_total"] {
        assert_eq!(context.resolve(key).as_deref(), Some("1"), "{key}");
    }
    assert_eq!(context.resolve("bases_view").as_deref(), Some("Public"));
    assert_eq!(
        context.resolve("bases_property").as_deref(),
        Some("file.name")
    );
    app.quick_keybinds_open = true;
    key(&mut app, KeyCode::Char('/'));
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal
        .draw(|frame| crate::ui::quick_keybinds::draw_quick_keybinds(frame, &app))
        .unwrap();
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| cell.symbol() == " ")
    );
}
