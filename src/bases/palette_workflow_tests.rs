use super::*;
use crate::bases::palette::Page;
use crate::palette::workflows::Workflow;
fn palette(app: &mut App, query: &str) {
    assert!(app.open_command_palette(None));
    assert!(
        app.command_palette
            .as_ref()
            .unwrap()
            .context_note_id
            .is_none()
    );
    for ch in query.chars() {
        assert!(crate::events::handle_global_popups_and_palette(
            app,
            Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
            AREA
        ));
    }
    let palette = app.command_palette.as_ref().unwrap();
    assert!(
        palette.state.selected().is_some(),
        "no selected item for {query}"
    );
    assert_eq!(palette.input.lines()[0], query);
    assert_eq!(
        palette.items[palette.state.selected().unwrap()].description,
        query
    );
    assert!(crate::events::handle_global_popups_and_palette(
        app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        AREA
    ));
    if matches!(
        query,
        "Edit selected public property"
            | "Configure view"
            | "Create public note"
            | "Export public results"
            | "Move selected files to group"
    ) {
        assert!(app.command_palette.is_some());
    } else {
        assert!(app.command_palette.is_none());
    }
}
#[test]
fn shared_palette_executes_guarded_property_commands_without_encrypted_catalog_context() {
    let (_root, mut app) = fixture();
    let base = app.storage.load_document("board.base").unwrap();
    let private = fs::read(app.storage.notes_dir.join("private.clin")).unwrap();
    let actions = crate::actions::get_all_action_infos(&app);
    assert!(actions.iter().any(|action| action.id == "switch_theme"));
    assert!(actions.iter().any(|action| action.id == "notes.open"));
    for id in ["note.decrypt", "note.encrypt"] {
        assert!(
            !crate::actions::get_all_actions()
                .iter()
                .any(|action| action.id() == id)
        );
        assert!(!actions.iter().any(|action| action.id == id));
        assert!(crate::actions::execute_action(id, &mut app, Some("private.clin")).is_err());
    }
    key(&mut app, KeyCode::Right);
    palette(&mut app, "Edit selected public property");
    wait(&mut app, |app| {
        app.command_palette
            .as_ref()
            .is_some_and(|palette| palette.bases_page().is_some_and(Page::is_prompt))
    });
    workflow_value(&mut app, "17");
    wait(&mut app, |app| app.command_palette.is_none());
    idle(&mut app);
    assert_eq!(properties(&app, "public.md")["score"].as_i64(), Some(17));
    palette(&mut app, "Undo guarded write");
    idle(&mut app);
    assert_eq!(properties(&app, "public.md")["score"].as_i64(), Some(2));
    palette(&mut app, "Redo guarded write");
    idle(&mut app);
    assert_eq!(properties(&app, "public.md")["score"].as_i64(), Some(17));
    palette(&mut app, "Search public results");
    assert!(app.bases_state.as_ref().unwrap().search_popup.is_some());
    key(&mut app, KeyCode::Esc);
    idle(&mut app);
    palette(&mut app, "Configure view");
    assert!(matches!(
        app.command_palette.as_ref().unwrap().bases_page(),
        Some(Page::Menu { .. })
    ));
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        base.source()
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        private
    );
    app.mode = ViewMode::List;
    assert!(
        !crate::actions::get_all_action_infos(&app)
            .iter()
            .any(|action| action.id == "bases.edit")
    );
    assert!(crate::actions::execute_action("bases.edit", &mut app, None).is_err());
}

#[test]
fn palette_navigation_and_vault_switch_wait_for_public_write_completion() {
    let (_root, mut app) = fixture();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let (started, ready) = std::sync::mpsc::channel();
    let (release, wait_release) = std::sync::mpsc::channel();
    pool.spawn(move || {
        started.send(()).unwrap();
        wait_release.recv().unwrap();
    });
    ready.recv().unwrap();
    let baseline = app.storage.load_document("board.base").unwrap();
    let source = format!("{}\n# completed palette write\n", baseline.source());
    app.bases_state.as_mut().unwrap().write_task = Some(crate::bases::write::WriteTask::spawn(
        app.storage.clone(),
        &pool,
        crate::bases::write::Operation::Save {
            baseline,
            source,
            embedded: None,
        },
    ));
    assert!(app.open_command_palette(None));
    let palette = app.command_palette.take().unwrap();
    let selected = palette
        .items
        .iter()
        .position(|item| item.id == "notes.open")
        .unwrap();
    app.activate_palette(palette, selected);
    assert_eq!(app.mode, ViewMode::Bases);
    assert!(app.status.contains("Write in progress"));
    for id in ["help.open", "setup_wizard"] {
        assert!(crate::actions::execute_action(id, &mut app, None).is_err());
        assert_eq!(app.mode, ViewMode::Bases);
    }
    app.switch_vault(app.storage.data_dir.join("another-vault"));
    assert!(app.setup_rebootstrap.is_none());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_some());
    release.send(()).unwrap();
    wait(&mut app, |app| {
        app.bases_state.as_ref().unwrap().write_task.is_none()
    });
    assert!(
        app.storage
            .load_document("board.base")
            .unwrap()
            .source()
            .contains("completed palette write")
    );
}

fn workflow_value(app: &mut App, value: &str) {
    app.command_palette.as_mut().unwrap().input.select_all();
    crate::palette_tests::drive(app, Event::Paste(value.into()));
    crate::palette_tests::drive(
        app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
}

fn workflow_choice(app: &mut App, id: &str) {
    let palette = app.command_palette.as_mut().unwrap();
    let index = palette.items.iter().position(|item| item.id == id).unwrap();
    palette.state.select(Some(index));
    crate::palette_tests::drive(
        app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
}

#[test]
fn palette_bases_configuration_nested_cancel_validation_and_write() {
    let (_root, mut app) = fixture();
    palette(&mut app, "Configure view");
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
    assert_eq!(app.command_palette.as_ref().unwrap().items.len(), 34);
    workflow_choice(&mut app, "8");
    workflow_value(&mut app, "invalid-limit");
    assert!(app.command_palette.as_ref().unwrap().error.is_some());
    assert_eq!(
        app.command_palette.as_ref().unwrap().input.lines()[0],
        "invalid-limit"
    );
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert!(matches!(
        app.command_palette.as_ref().unwrap().bases_page(),
        Some(Page::Menu { .. })
    ));
    workflow_choice(&mut app, "0");
    workflow_value(&mut app, "Renamed view");
    assert!(app.command_palette.as_ref().unwrap().is_submitting());
    let submitted = app.command_palette.as_ref().unwrap().input.lines().to_vec();
    crate::palette_tests::drive(
        &mut app,
        Event::Paste("must not replace pending write".into()),
    );
    assert_eq!(
        app.command_palette.as_ref().unwrap().input.lines(),
        submitted
    );
    wait(&mut app, |app| app.command_palette.is_none());
    idle(&mut app);
    assert!(
        app.storage
            .load_document("board.base")
            .unwrap()
            .source()
            .contains("Renamed view")
    );
}

#[test]
fn palette_bases_stale_property_write_retains_value_and_private_files() {
    let (_root, mut app) = fixture();
    key(&mut app, KeyCode::Right);
    palette(&mut app, "Edit selected public property");
    wait(&mut app, |app| {
        app.command_palette
            .as_ref()
            .is_some_and(|p| p.bases_page().is_some_and(Page::is_prompt))
    });
    let private = fs::read(app.storage.notes_dir.join("private.clin")).unwrap();
    let path = app.storage.note_path("public.md");
    fs::write(&path, "---\nscore: 99\n---\nexternal").unwrap();
    workflow_value(&mut app, "123");
    wait(&mut app, |app| {
        app.command_palette
            .as_ref()
            .is_some_and(|p| p.error.is_some())
    });
    assert_eq!(
        app.command_palette.as_ref().unwrap().input.lines()[0],
        "123"
    );
    assert!(!app.command_palette.as_ref().unwrap().is_submitting());
    assert!(fs::read_to_string(&path).unwrap().contains("99"));
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        private
    );
}

#[test]
fn palette_bases_delete_confirmation_stays_separate_and_restores_menu() {
    let (_root, mut app) = fixture();
    palette(&mut app, "Configure view");
    workflow_choice(&mut app, "24");
    assert!(app.palette_bases_dialog_open());
    assert!(matches!(
        app.command_palette.as_ref().unwrap().workflow,
        Some(Workflow::Bases(_))
    ));
    crate::palette_tests::drive(&mut app, Event::Paste("DELETE".into()));
    if let Some(crate::bases::authoring::Dialog::Prompt { input, .. }) =
        &app.bases_state.as_ref().unwrap().dialog
    {
        assert_eq!(input.lines()[0], "DELETE");
    } else {
        panic!("missing separate confirmation");
    }
    assert!(app.command_palette.as_ref().unwrap().input.lines()[0].is_empty());
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert!(!app.palette_bases_dialog_open());
    assert_eq!(app.command_palette.as_ref().unwrap().items.len(), 34);
}
