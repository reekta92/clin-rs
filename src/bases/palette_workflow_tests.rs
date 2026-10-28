use super::*;
use crate::bases::palette::Page;
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
            | "Create public note"
            | "Export public results"
            | "Move selected files to group"
    ) {
        assert!(app.command_palette.is_some());
    } else {
        assert!(app.command_palette.is_none());
    }
}
fn bases_category(app: &mut App) {
    crate::palette_tests::drive(
        app,
        Event::Key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE)),
    );
    let palette = app
        .command_palette
        .as_ref()
        .expect("configuration shortcut opens palette");
    assert!(palette.workflow.is_none());
    assert_eq!(palette.tabs[palette.active_tab].0, "Bases");
    assert_eq!(palette.items.len(), 34);
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
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
    bases_category(&mut app);
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
        wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
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
fn palette_bases_configuration_cancel_validation_and_write() {
    let (_root, mut app) = fixture();
    bases_category(&mut app);
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
    assert_eq!(app.command_palette.as_ref().unwrap().items.len(), 34);
    crate::palette_tests::drive(&mut app, Event::Paste("Row limit".into()));
    workflow_choice(&mut app, "bases.config.limit");
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
    assert!(app.command_palette.as_ref().unwrap().workflow.is_none());
    let palette = app.command_palette.as_ref().unwrap();
    assert_eq!(palette.input.lines()[0], "Row limit");
    assert_eq!(
        palette.items[palette.state.selected().unwrap()].id,
        "bases.config.limit"
    );
    assert_eq!(
        app.command_palette.as_ref().unwrap().tabs
            [app.command_palette.as_ref().unwrap().active_tab]
            .0,
        "Bases"
    );
    let mut palette = app.command_palette.take().unwrap();
    palette.input.select_all();
    palette.input.cut();
    palette.refresh_items(&app);
    app.command_palette = Some(palette);
    workflow_choice(&mut app, "bases.config.rename_view");
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
fn palette_bases_delete_confirmation_uses_value_input_and_protects_last_view() {
    let (_root, mut app) = fixture();
    let baseline = app.storage.load_document("board.base").unwrap();
    bases_category(&mut app);
    workflow_choice(&mut app, "bases.config.remove_view");
    assert!(!app.palette_bases_dialog_open());
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
    let palette = app.command_palette.as_ref().unwrap();
    assert!(palette.is_value_input());
    assert_eq!(palette.input_height(30), 3);
    assert_eq!(
        palette.workflow_title(),
        "COMMANDS › Bases › Remove current view"
    );
    workflow_value(&mut app, "delete");
    let palette = app.command_palette.as_ref().unwrap();
    assert!(palette.error.as_ref().unwrap().contains("type DELETE"));
    assert_eq!(palette.input.lines()[0], "delete");
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    app.command_palette.as_mut().unwrap().input.select_all();
    crate::palette_tests::drive(&mut app, Event::Paste("DELETE".into()));
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert!(!app.palette_bases_dialog_open());
    assert_eq!(app.command_palette.as_ref().unwrap().items.len(), 34);
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        baseline.source()
    );
    workflow_choice(&mut app, "bases.config.remove_view");
    workflow_value(&mut app, "DELETE");
    assert!(app.command_palette.as_ref().unwrap().is_submitting());
    wait(&mut app, |app| app.command_palette.is_none());
    idle(&mut app);
    let saved = app.storage.load_document("board.base").unwrap();
    let document = crate::bases::format::BaseDocument::parse(saved.source()).unwrap();
    assert_eq!(document.config.views.len(), 1);
    assert_eq!(document.config.views[0].name, "List");
    bases_category(&mut app);
    workflow_choice(&mut app, "bases.config.remove_view");
    workflow_value(&mut app, "DELETE");
    let palette = app.command_palette.as_ref().unwrap();
    assert!(
        palette
            .error
            .as_ref()
            .unwrap()
            .contains("cannot remove the last base view")
    );
    assert_eq!(palette.input.lines()[0], "DELETE");
    assert!(!palette.is_submitting());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        saved.source()
    );
}

#[test]
fn palette_bases_cancel_pending_source_read_keeps_original_owner() {
    let (_root, mut app) = fixture();
    let pool = std::sync::Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap(),
    );
    let (started, ready) = std::sync::mpsc::channel();
    let (release, blocked) = std::sync::mpsc::channel();
    pool.spawn(move || {
        started.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    app.notes_worker_pool = pool;
    bases_category(&mut app);
    workflow_choice(&mut app, "bases.config.source");
    assert!(app.bases_state.as_ref().unwrap().write_task.is_some());
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert!(app.command_palette.as_ref().unwrap().workflow.is_none());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    release.send(()).unwrap();
    app.poll_bases();
    assert_eq!(app.mode, ViewMode::Bases);
    assert!(app.editor.editing_id.is_none());
}

#[test]
fn palette_bases_category_registers_every_option_only_for_bases() {
    use crate::actions::{ActionCategory, ActionContext};
    let (_root, mut app) = fixture();
    let actions = crate::actions::get_all_action_infos(&app);
    let commands: Vec<_> = actions
        .iter()
        .filter(|action| action.category == ActionCategory::Bases)
        .collect();
    assert_eq!(commands.len(), 34);
    for &(id, _, name, hint) in crate::bases::palette::COMMANDS {
        let action = commands.iter().find(|action| action.id == id).unwrap();
        assert_eq!(action.name, format!("Bases: {name}"));
        assert_eq!(action.description, hint);
    }
    assert!(
        !crate::actions::get_all_actions()
            .iter()
            .any(|action| action.id() == "bases.configure")
    );
    let context = ActionContext::capture(&app, None, None);
    for mode in [
        ViewMode::List,
        ViewMode::Edit,
        ViewMode::Help,
        ViewMode::Graph,
        ViewMode::Draw,
        ViewMode::Canvas,
        ViewMode::Outline,
        ViewMode::Setup,
    ] {
        app.mode = mode;
        assert!(
            !crate::palette::palette_tabs(&app)
                .iter()
                .any(|tab| tab.0 == "Bases"),
            "{mode:?}"
        );
        assert!(
            !crate::actions::get_all_action_infos(&app)
                .iter()
                .any(|action| action.category == ActionCategory::Bases)
        );
        assert!(
            crate::actions::execute_action_in_context(
                "bases.config.rename_view",
                &mut app,
                &context
            )
            .is_err()
        );
    }
}

#[test]
fn palette_bases_remapped_configure_opens_category_and_mouse_applies() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let (_root, mut app) = fixture();
    app.keybinds.bases.insert(
        crate::keybinds::BasesAction::Configure,
        vec![crate::keybinds::KeyCombo::parse("F6").unwrap()],
    );
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE)),
    );
    let palette = app.command_palette.as_ref().unwrap();
    assert_eq!(palette.tabs[palette.active_tab].0, "Bases");
    crate::palette_tests::drive(&mut app, Event::Paste("Layout".into()));
    let render = |app: &mut App| {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, app, crate::app::EditFocus::Body))
            .unwrap();
    };
    let click_apply = |app: &mut App| {
        let area = app
            .command_palette
            .as_ref()
            .unwrap()
            .last_results_area
            .unwrap();
        crate::palette_tests::drive(
            app,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: area.x + 2,
                row: area.y + 1,
                modifiers: KeyModifiers::NONE,
            }),
        );
    };
    render(&mut app);
    click_apply(&mut app);
    assert!(!app.command_palette.as_ref().unwrap().is_value_input());
    assert_eq!(
        app.command_palette
            .as_ref()
            .unwrap()
            .bases_page()
            .unwrap()
            .current_value(),
        Some("Table")
    );
    crate::palette_tests::drive(&mut app, Event::Paste("Cards".into()));
    for (width, height) in [(8, 4), (1, 1), (100, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
            .unwrap();
    }
    click_apply(&mut app);
    wait(&mut app, |app| app.command_palette.is_none());
    idle(&mut app);
    let saved = app.storage.load_document("board.base").unwrap();
    let document = crate::bases::format::BaseDocument::parse(saved.source()).unwrap();
    assert_eq!(
        document.config.views[0].layout,
        crate::bases::format::Layout::Cards
    );
}

#[test]
fn palette_bases_source_repair_remains_available_without_a_loaded_document() {
    let (_root, mut app) = fixture();
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage
        .save_document(&baseline, "views: [invalid")
        .unwrap();
    app.bases_state.as_mut().unwrap().loaded = None;
    key(&mut app, KeyCode::Char('r'));
    wait(&mut app, |app| !app.bases_state.as_ref().unwrap().loading);
    key(&mut app, KeyCode::Char(':'));
    let palette = app.command_palette.as_ref().unwrap();
    assert_eq!(palette.tabs[palette.active_tab].0, "Bases");
    assert_eq!(palette.items.len(), 1);
    workflow_choice(&mut app, "bases.config.source");
    wait(&mut app, |app| app.mode == ViewMode::Edit);
    assert_eq!(
        app.editor.public_document_source().unwrap(),
        "views: [invalid"
    );
}

#[test]
fn palette_bases_fixed_options_keep_current_values_guarded_history_and_defaults() {
    for (layout, field, title, original, current, next, count) in [
        ("table", "type", "Layout", "table", "table", "list", 4),
        (
            "table",
            "rowHeight",
            "Table row height",
            "medium",
            "medium",
            "extra",
            4,
        ),
        (
            "list",
            "markers",
            "List markers",
            "number",
            "number",
            "none",
            3,
        ),
        (
            "list",
            "indentProperties",
            "Indent list properties",
            "false",
            "",
            "true",
            2,
        ),
        ("cards", "imageFit", "Image fit", "cover", "", "contain", 2),
    ] {
        let (_root, mut app) = fixture();
        let baseline = app.storage.load_document("board.base").unwrap();
        let field_yaml = if field == "type" {
            String::new()
        } else {
            format!(", {field}: {original}")
        };
        let source = format!(
            "# retained\nplugin: {{opaque: true}}\nfilters: file.ext == 'md'\nviews: [{{type: {layout}, name: Public, groupBy: {{property: score}}, order: [file.name, score]{field_yaml}}}]\n"
        );
        app.storage.save_document(&baseline, &source).unwrap();
        key(&mut app, KeyCode::Char('r'));
        idle(&mut app);
        let public = app.storage.load_document("public.md").unwrap();
        let private = fs::read(app.storage.notes_dir.join("private.clin")).unwrap();
        menu(&mut app, title);
        let palette = app.command_palette.as_ref().unwrap();
        assert!(!palette.is_value_input(), "{field}");
        assert_eq!(palette.items.len(), count, "{field}");
        let selected = &palette.items[palette.state.selected().unwrap()];
        assert_eq!(selected.id, current, "{field}");
        assert_eq!(selected.description, "Current");
        assert_eq!(
            palette.bases_page().unwrap().current_value(),
            Some(selected.name.as_str())
        );
        workflow_choice(&mut app, next);
        assert!(app.command_palette.as_ref().unwrap().is_submitting());
        crate::palette_tests::drive(&mut app, Event::Paste("blocked during save".into()));
        assert_eq!(app.command_palette.as_ref().unwrap().input.lines()[0], "");
        crate::palette_tests::drive(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        );
        wait(&mut app, |app| app.command_palette.is_none());
        idle(&mut app);
        assert_eq!(
            app.bases_state.as_ref().unwrap().undo.len(),
            1,
            "only one write"
        );
        let saved = app.storage.load_document("board.base").unwrap();
        let document = crate::bases::format::BaseDocument::parse(saved.source()).unwrap();
        let value = &document.config.views[0].options[field];
        if field == "indentProperties" {
            assert_eq!(value.as_bool(), Some(true));
        } else {
            assert_eq!(value.as_str(), Some(next));
        }
        assert!(
            saved
                .source()
                .contains("# retained\nplugin: {opaque: true}")
        );
        key(&mut app, KeyCode::Char('u'));
        idle(&mut app);
        assert_eq!(
            app.storage.load_document("board.base").unwrap().source(),
            source
        );
        event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
        );
        idle(&mut app);
        assert_eq!(
            app.storage.load_document("board.base").unwrap().source(),
            saved.source()
        );
        if field != "type" {
            menu(&mut app, title);
            let palette = app.command_palette.as_ref().unwrap();
            assert_eq!(palette.items[palette.state.selected().unwrap()].id, next);
            workflow_choice(&mut app, "");
            wait(&mut app, |app| app.command_palette.is_none());
            idle(&mut app);
            let saved = app.storage.load_document("board.base").unwrap();
            let document = crate::bases::format::BaseDocument::parse(saved.source()).unwrap();
            assert!(!document.config.views[0].options.contains_key(field));
        }
        assert_eq!(
            app.storage.load_document("public.md").unwrap().source(),
            public.source()
        );
        assert_eq!(
            fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
            private
        );
    }
}

#[test]
fn palette_bases_selector_validation_and_cancel_retain_filtered_choices_and_root() {
    let (_root, mut app) = fixture();
    let baseline = app.storage.load_document("board.base").unwrap();
    let source = baseline
        .source()
        .replace("type: table", "type: table\n    markers: bullets");
    app.storage.save_document(&baseline, &source).unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    bases_category(&mut app);
    crate::palette_tests::drive(&mut app, Event::Paste("Layout".into()));
    workflow_choice(&mut app, "bases.config.layout");
    crate::palette_tests::drive(&mut app, Event::Paste("List".into()));
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    let palette = app.command_palette.as_ref().unwrap();
    assert!(palette.error.as_ref().unwrap().contains("markers must"));
    assert_eq!(palette.input.lines()[0], "List");
    assert_eq!(palette.items[palette.state.selected().unwrap()].id, "list");
    assert!(!palette.is_submitting());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    let palette = app.command_palette.as_ref().unwrap();
    assert!(palette.workflow.is_none());
    assert_eq!(palette.tabs[palette.active_tab].0, "Bases");
    assert_eq!(palette.input.lines()[0], "Layout");
    assert_eq!(
        palette.items[palette.state.selected().unwrap()].id,
        "bases.config.layout"
    );
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        source
    );
}

#[test]
fn palette_bases_selector_rejects_external_replacement_and_retains_selection() {
    let (_root, mut app) = fixture();
    menu(&mut app, "Table row height");
    crate::palette_tests::drive(&mut app, Event::Paste("Extra".into()));
    let baseline = app.storage.load_document("board.base").unwrap();
    let external = baseline.source().replace("opaque: true", "opaque: false");
    app.storage.save_document(&baseline, &external).unwrap();
    app.invalidate_bases();
    idle(&mut app);
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    let palette = app.command_palette.as_ref().unwrap();
    assert!(
        palette
            .error
            .as_ref()
            .unwrap()
            .contains("Bases document changed")
    );
    assert_eq!(palette.input.lines()[0], "Extra");
    assert_eq!(palette.items[palette.state.selected().unwrap()].id, "extra");
    assert!(!palette.is_submitting());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        external
    );
}

#[test]
fn palette_bases_value_inputs_match_creation_style_and_multiline_mouse_submit() {
    let (_root, mut app) = fixture();
    app.config.features.canvas_view = FeatureState::Enabled;
    app.config.features.draw_view = FeatureState::Enabled;
    app.mode = ViewMode::List;
    let mut creation_style = None;
    for id in ["create_canvas", "draw.create"] {
        assert!(app.open_command_palette(None));
        workflow_choice(&mut app, id);
        let palette = app.command_palette.as_ref().unwrap();
        let style = (
            palette.input.style(),
            palette.input.cursor_line_style(),
            palette.content_areas(Rect::new(0, 0, 80, 30), false),
        );
        if let Some(expected) = &creation_style {
            assert_eq!(&style, expected);
        }
        creation_style = Some(style);
        app.dismiss_command_palette();
    }
    app.mode = ViewMode::Bases;
    for title in ["Rename view", "Global filter", "Row sorting", "Formula"] {
        menu(&mut app, title);
        let palette = app.command_palette.as_ref().unwrap();
        assert!(palette.workflow_title().starts_with("COMMANDS › Bases › "));
        assert_eq!(palette.workflow_hint(), "");
        assert_eq!(palette.input_height(30), 3);
        assert_eq!(
            (
                palette.input.style(),
                palette.input.cursor_line_style(),
                palette.content_areas(Rect::new(0, 0, 80, 30), false)
            ),
            creation_style.unwrap()
        );
        assert_eq!(
            palette.input.placeholder_style().unwrap().fg,
            Some(app.app_theme.muted)
        );
        crate::palette_tests::drive(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        );
    }
    menu(&mut app, "Formula");
    crate::palette_tests::drive(&mut app, Event::Paste("Total = 1".into()));
    crate::palette_tests::drive(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
    );
    crate::palette_tests::drive(&mut app, Event::Paste("+ 2".into()));
    assert_eq!(app.command_palette.as_ref().unwrap().input_height(30), 4);
    for (width, height) in [(8, 4), (1, 1), (100, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
            .unwrap();
    }
    let area = app
        .command_palette
        .as_ref()
        .unwrap()
        .last_results_area
        .unwrap();
    crate::palette_tests::drive(
        &mut app,
        Event::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: area.x + 2,
            row: area.y + 1,
            modifiers: KeyModifiers::NONE,
        }),
    );
    wait(&mut app, |app| app.command_palette.is_none());
    idle(&mut app);
    let saved = app.storage.load_document("board.base").unwrap();
    let document = crate::bases::format::BaseDocument::parse(saved.source()).unwrap();
    assert_eq!(document.config.formulas["Total"], "1\n+ 2");
}
