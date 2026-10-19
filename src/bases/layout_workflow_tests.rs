use super::*;

fn body(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width.max(1), height.max(1))).unwrap();
    let mut state = app.bases_state.take().unwrap();
    terminal
        .draw(|frame| {
            crate::bases::render::render(frame, Rect::new(0, 0, width, height), &mut state, app)
        })
        .unwrap();
    app.bases_state = Some(state);
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn source(app: &mut App, text: &str) {
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline, text).unwrap();
    key(app, KeyCode::Char('r'));
    idle(app);
}

#[test]
fn footer_uses_shared_hint_styles_and_statusline_templates() {
    let (_root, mut app) = fixture();
    app.keybinds.bases.insert(
        crate::keybinds::BasesAction::Back,
        vec![crate::keybinds::KeyCombo::parse("F8").unwrap()],
    );
    app.config.statusline.footer_left = Some("{hints}".into());
    app.config.statusline.footer_right = Some("{bases_view}:{bases_rows}".into());
    for style in crate::config::HintBarStyle::ALL {
        app.app_theme.hint_bar_style = style;
        let mut terminal = Terminal::new(TestBackend::new(240, 28)).unwrap();
        let mut state = app.bases_state.take().unwrap();
        terminal
            .draw(|frame| state.render_native(frame, frame.area(), &app))
            .unwrap();
        app.bases_state = Some(state);
        let buffer = terminal.backend().buffer();
        let footer: String = (0..240).map(|x| buffer[(x, 27)].symbol()).collect();
        assert!(footer.contains("Public:1"), "{style:?}: {footer}");
        let expected = crate::ui::format_keybind_hints(
            &app.app_theme,
            &[(
                app.keybinds
                    .display_bases(crate::keybinds::BasesAction::Back),
                "back",
            )],
        );
        let prefix_width = expected.spans[0].width() as u16;
        let mut expected_terminal = Terminal::new(TestBackend::new(240, 1)).unwrap();
        expected_terminal
            .draw(|frame| {
                crate::ui::draw_status_bar(frame, frame.area(), &app.app_theme, expected, None);
            })
            .unwrap();
        for x in 0..prefix_width {
            assert_eq!(
                buffer[(x, 27)],
                expected_terminal.backend().buffer()[(x, 0)],
                "{style:?}"
            );
        }
    }
    app.config.statusline.footer_left = Some("{pending}".into());
    app.bases_state.as_mut().unwrap().matcher.count = Some(777);
    assert!(render(&mut app).contains("777 "));
}

#[test]
fn invalid_view_retains_valid_view_authoring_and_source_repair() {
    let (_root, mut app) = fixture();
    source(
        &mut app,
        "plugin: {opaque: true}\nviews: [{type: table, name: Invalid, rowHeight: huge}, {type: list, name: Valid}]\n",
    );
    assert!(render(&mut app).contains("rowHeight must"));
    key(&mut app, KeyCode::Tab);
    idle(&mut app);
    assert_eq!(
        app.bases_state.as_ref().unwrap().named_view.as_deref(),
        Some("Valid")
    );
    menu(&mut app, "List markers");
    prompt_text(&mut app, "number");
    assert!(
        app.storage
            .load_document("board.base")
            .unwrap()
            .source()
            .contains("rowHeight: huge")
    );
    key(&mut app, KeyCode::BackTab);
    idle(&mut app);
    menu(&mut app, "Edit Bases source");
    wait(&mut app, |app| app.mode == ViewMode::Edit);
    assert!(app.editor.public_document.is_some());
    assert!(app.editor.body.text().contains("opaque: true"));
}

#[test]
fn base_note_actions_keep_bytes_and_guard_move_destinations() {
    let (_root, mut app) = fixture();
    let original = app
        .storage
        .load_document("board.base")
        .unwrap()
        .source()
        .to_owned();
    let duplicate = app.storage.duplicate_note("board.base", "copies").unwrap();
    assert_eq!(
        app.storage.load_document(&duplicate).unwrap().source(),
        original
    );
    let renamed = app.storage.rename_note(&duplicate, "Renamed").unwrap();
    assert_eq!(
        app.storage.load_document(&renamed).unwrap().source(),
        original
    );
    let moved = app.storage.move_note(&renamed, "moved").unwrap();
    assert_eq!(
        app.storage.load_document(&moved).unwrap().source(),
        original
    );
    assert!(app.storage.move_note(&moved, "../escape").is_err());
    assert!(app.storage.move_note(&moved, ".private").is_err());
    app.storage
        .create_document("collision/Renamed.base", "external")
        .unwrap();
    assert!(app.storage.move_note(&moved, "collision").is_err());
    assert_eq!(
        app.storage.load_document(&moved).unwrap().source(),
        original
    );
    assert_eq!(
        app.storage
            .load_document("collision/Renamed.base")
            .unwrap()
            .source(),
        "external"
    );
}

#[test]
fn table_short_viewports_keep_focus_and_exclude_headers_and_summaries_from_hits() {
    let (_root, mut app) = fixture();
    source(
        &mut app,
        "filters: file.ext == 'md'\nviews: [{type: table, rowHeight: extra, order: [file.name, score], groupBy: {property: score}, summaries: {score: Sum}, columnSize: {file.name: 6000, score: 8000}}]\n",
    );
    app.bases_state.as_mut().unwrap().selected = 1;
    for (width, height) in [(0, 0), (1, 1), (2, 2), (8, 3), (30, 10), (100, 28)] {
        body(&mut app, width, height);
        let state = app.bases_state.as_ref().unwrap();
        assert!(state.hit_rows.iter().all(|(rect, position, _)| {
            *position < state.items.len()
                && rect.right() <= width
                && rect.bottom() <= height
                && !rect.is_empty()
        }));
        if width > 0 && height > 0 {
            assert!(
                state
                    .hit_rows
                    .iter()
                    .any(|(_, position, _)| *position == state.selected)
            );
        }
    }
    app.bases_state.as_mut().unwrap().column = 1;
    assert!(body(&mut app, 1, 1).contains('2'));
    assert_eq!(app.bases_state.as_ref().unwrap().hit_rows[0].2, 1);
    app.bases_state.as_mut().unwrap().selected = 0;
    body(&mut app, 100, 10);
    assert_eq!(app.bases_state.as_ref().unwrap().hit_rows[0].0.height, 1);
    key(&mut app, KeyCode::Char('z'));
    body(&mut app, 100, 10);
    assert_eq!(app.bases_state.as_ref().unwrap().items.len(), 1);
}

#[test]
fn list_settings_wrap_focus_mouse_and_lossless_reset() {
    let (_root, mut app) = fixture();
    let original = app.storage.load_document("public.md").unwrap();
    source(
        &mut app,
        "# retained\nplugin: {opaque: true}\nfilters: file.ext == 'md'\nviews: [{type: list, name: List, order: [file.name, score, absent], markers: number, indentProperties: false, separator: ' | '}]\n",
    );
    assert!(body(&mut app, 100, 10).contains("1. public.md | 2 | "));
    menu(&mut app, "Indent list properties");
    prompt_text(&mut app, "true");
    let screen = body(&mut app, 100, 10);
    assert!(screen.contains("1. public.md"));
    let score = app
        .bases_state
        .as_ref()
        .unwrap()
        .hit_rows
        .iter()
        .find(|(_, _, column)| *column == 1)
        .unwrap()
        .0;
    assert_eq!(score.y, 1);
    mouse(
        &mut app,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        score,
    );
    assert_eq!(app.bases_state.as_ref().unwrap().column, 1);
    assert!(body(&mut app, 1, 1).contains('2'));
    key(&mut app, KeyCode::Char(']'));
    body(&mut app, 1, 1);
    assert!(
        app.bases_state
            .as_ref()
            .unwrap()
            .hit_rows
            .iter()
            .any(|(_, _, column)| *column == 2)
    );
    menu(&mut app, "List markers");
    event(&mut app, Event::Paste("bullets".into()));
    key(&mut app, KeyCode::Enter);
    assert!(app.command_palette.as_ref().unwrap().items.is_empty());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    key(&mut app, KeyCode::Esc);
    menu(&mut app, "List markers");
    prompt_text(&mut app, "none");
    app.bases_state.as_mut().unwrap().column = 0;
    assert!(!body(&mut app, 100, 10).contains("1. public"));
    for label in [
        "List markers",
        "Indent list properties",
        "List property separator",
    ] {
        menu(&mut app, label);
        prompt_text(&mut app, "");
    }
    assert!(body(&mut app, 100, 10).contains("• public.md, 2, "));
    let saved = app.storage.load_document("board.base").unwrap();
    assert!(saved.source().contains("# retained") && saved.source().contains("opaque: true"));
    assert!(!saved.source().contains("indentProperties"));
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        original.source()
    );
}

#[test]
fn list_numbering_restarts_per_group_and_unicode_focus_survives_clipping() {
    let (_root, mut app) = fixture();
    app.storage
        .create_document("界界e\u{301}.md", "---\nscore: 4\n---\nbody")
        .unwrap();
    source(
        &mut app,
        "filters: file.ext == 'md'\nviews: [{type: list, order: [file.name, score], markers: number, groupBy: {property: score}, separator: ''}]\n",
    );
    let screen = body(&mut app, 100, 10);
    assert!(screen.contains("1. public.md, 2"));
    assert!(
        screen.contains("1. 界") && screen.contains("e\u{301}.md, 4"),
        "{screen:?}"
    );
    app.bases_state.as_mut().unwrap().selected = 3;
    app.bases_state.as_mut().unwrap().column = 1;
    assert!(body(&mut app, 2, 2).contains('4'));
    assert!(
        app.bases_state
            .as_ref()
            .unwrap()
            .hit_rows
            .iter()
            .any(|(_, item, column)| *item == 3 && *column == 1)
    );
}
#[test]
fn table_heights_widths_wrapping_focus_and_authoring_are_native_and_guarded() {
    let (_root, mut app) = fixture();
    let public = app.storage.load_document("public.md").unwrap();
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline,"# retained\nplugin: {opaque: true}\nfilters: file.ext == 'md'\nviews: [{type: table, name: Sized, order: [file.name, score], rowHeight: tall, columnSize: {file.name: 64, score: 96}}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    let screen = render(&mut app);
    assert!(screen.contains("public."));
    let hits = app.bases_state.as_ref().unwrap().hit_rows.clone();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].0.width, 8);
    assert_eq!(hits[0].0.height, 4);
    assert_eq!(hits[1].0.width, 12);
    mouse(
        &mut app,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        Rect::new(hits[1].0.x, hits[1].0.y + 2, 1, 1),
    );
    assert_eq!(app.bases_state.as_ref().unwrap().column, 1);
    menu(&mut app, "Table row height");
    prompt_text(&mut app, "extra");
    let loaded = app.bases_state.as_ref().unwrap().loaded.as_ref().unwrap();
    assert_eq!(
        loaded.document.config.views[0].options["rowHeight"].as_str(),
        Some("extra")
    );
    render(&mut app);
    assert_eq!(app.bases_state.as_ref().unwrap().hit_rows[0].0.height, 8);
    let before = app.storage.load_document("board.base").unwrap();
    menu(&mut app, "Table row height");
    event(&mut app, Event::Paste("giant".into()));
    key(&mut app, KeyCode::Enter);
    assert!(app.command_palette.as_ref().unwrap().items.is_empty());
    assert!(app.bases_state.as_ref().unwrap().write_task.is_none());
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        before.source()
    );
    key(&mut app, KeyCode::Esc);
    menu(&mut app, "Table column widths");
    prompt_text(&mut app, "{file.name: 720, score: 720}");
    render(&mut app);
    assert_eq!(
        app.bases_state.as_ref().unwrap().hit_rows.last().unwrap().2,
        1
    );
    assert!(
        app.storage
            .load_document("board.base")
            .unwrap()
            .source()
            .contains("opaque: true")
    );
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        public.source()
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        b"not a public document"
    );
}

#[test]
fn paging_and_scrollbars_follow_rendered_rows_in_every_layout() {
    let (_root, mut app) = fixture();
    for index in 0..24 {
        app.storage
            .create_document(&format!("row{index}.md"), "---\nscore: 2\n---\nBody")
            .unwrap();
    }
    for layout in ["table", "list", "cards", "kanban"] {
        source(
            &mut app,
            &format!(
                "filters: file.ext == 'md'\nviews: [{{type: {layout}, order: [file.name, score], rowHeight: extra, groupBy: {{property: score}}}}]\n"
            ),
        );
        app.bases_state.as_mut().unwrap().selected = 0;
        body(&mut app, 70, 18);
        let state = app.bases_state.as_ref().unwrap();
        let visible = state
            .hit_rows
            .iter()
            .map(|(_, index, _)| index)
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert!(
            state.scrollbar.content_len > state.scrollbar.viewport_len,
            "{layout}"
        );
        assert_eq!(state.scrollbar.track.width, 1);
        let selected = state.selected;
        key(&mut app, KeyCode::PageDown);
        assert_eq!(
            app.bases_state.as_ref().unwrap().selected,
            selected + visible.saturating_sub(1).max(1),
            "{layout}"
        );
        body(&mut app, 70, 18);
        let state = app.bases_state.as_ref().unwrap();
        assert!(
            state
                .hit_rows
                .iter()
                .any(|(_, index, _)| *index == state.selected),
            "{layout}"
        );
    }
}

#[test]
fn eighth_help_tab_uses_bases_bindings_and_feature_visibility() {
    let (_root, mut app) = fixture();
    app.config.features = crate::config::FeaturesConfig::default();
    app.open_help_page_with_tab(HelpTab::Notes);
    crate::events::handle_help_keys(
        &mut app,
        KeyEvent::new(KeyCode::Char('8'), KeyModifiers::NONE),
    );
    assert_eq!(app.help_tab, HelpTab::Bases);
    app.config.features.bases_view = FeatureState::Disabled;
    assert!(
        crate::ui::help_tabs(app.config.ui.icon_mode, &app.config.features)
            .iter()
            .all(|(tab, _, _)| *tab != HelpTab::Bases)
    );
}
