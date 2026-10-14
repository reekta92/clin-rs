use super::*;
use crate::bases::authoring::Dialog;
fn palette(app: &mut App, query: &str) {
    event(
        app,
        Event::Key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
    );
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
    assert!(app.command_palette.is_none());
}
#[test]
fn shared_palette_executes_guarded_property_commands_without_encrypted_catalog_context() {
    let (_root, mut app) = fixture();
    let base = app.storage.load_document("board.base").unwrap();
    let private = fs::read(app.storage.notes_dir.join("private.clin")).unwrap();
    let actions = crate::actions::get_all_action_infos(&app);
    assert!(actions.iter().all(|action| action.id.starts_with("bases.")));
    assert!(crate::actions::execute_action("decrypt", &mut app, Some("private.clin")).is_err());
    key(&mut app, KeyCode::Right);
    palette(&mut app, "Edit selected public property");
    wait(&mut app, |app| {
        app.bases_state.as_ref().unwrap().dialog.is_some()
    });
    prompt_text(&mut app, "17");
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
        app.bases_state.as_ref().unwrap().dialog,
        Some(Dialog::Menu { .. })
    ));
    key(&mut app, KeyCode::Esc);
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
