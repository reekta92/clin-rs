//! Exercise covers through the native overlay, not a separate media renderer.
use super::*;

#[test]
fn card_and_kanban_covers_use_public_native_protocol_and_guarded_configuration() {
    for layout in ["cards", "kanban"] {
        let (_root, mut app) = fixture();
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.config.features.images = FeatureState::Enabled;
        let path = app.storage.notes_dir.join("cover.png");
        image::RgbaImage::from_pixel(64, 32, image::Rgba([40, 180, 70, 255]))
            .save(&path)
            .unwrap();
        let image_before = fs::read(&path).unwrap();
        let note = app.storage.load_document("public.md").unwrap();
        app.storage
            .save_document(
                &note,
                &note.source().replace(
                    "score: 2",
                    "score: 2\r\ncover: '[[cover.png]]'\r\nextra: Visible",
                ),
            )
            .unwrap();
        let public_before = app.storage.load_document("public.md").unwrap();
        let base = app.storage.load_document("board.base").unwrap();
        let source = format!(
            "# retained\nplugin: {{opaque: true}}\nfilters: file.ext == 'md'\nviews: [{{type: {layout}, name: Media, order: [file.name, score, extra], groupBy: {{property: score}}, image: note.cover, imageFit: cover, imageAspectRatio: 2, cardSize: 240, columnWidth: 240}}]\n"
        );
        app.storage.save_document(&base, &source).unwrap();
        key(&mut app, KeyCode::Char('r'));
        idle(&mut app);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            render(&mut app);
            let (entries, active, ready) = app.bases_state.as_ref().unwrap().media.stats();
            assert!(entries <= 64 && active <= 2);
            if ready == 1 {
                break;
            }
            assert!(Instant::now() < deadline, "public cover stalled");
            std::thread::sleep(Duration::from_millis(2));
        }
        key(&mut app, KeyCode::End);
        key(&mut app, KeyCode::Char(']'));
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.bases_state.as_ref().unwrap().column, 2);
        assert!(render(&mut app).contains("Visible"));
        key(&mut app, KeyCode::Char('['));
        assert_eq!(app.bases_state.as_ref().unwrap().column, 1);
        key(&mut app, KeyCode::Char('v'));
        wait(&mut app, |app| {
            app.bases_state.as_ref().unwrap().dialog.is_some()
        });
        assert!(render(&mut app).contains("PUBLIC IMAGE"));
        key(&mut app, KeyCode::Char('q'));
        assert_eq!(app.mode, ViewMode::Bases);
        menu(&mut app, "Image fit");
        prompt_text(&mut app, "contain");
        idle(&mut app);
        let configured = app.storage.load_document("board.base").unwrap();
        let document = super::super::format::BaseDocument::parse(configured.source()).unwrap();
        assert_eq!(
            document.config.views[0].options["imageFit"].as_str(),
            Some("contain")
        );
        assert!(configured.source().contains("plugin: {opaque: true}"));
        menu(&mut app, "Image aspect ratio");
        prompt_text(&mut app, "0");
        assert!(app.command_palette.as_ref().unwrap().error.is_some());
        assert_eq!(
            app.storage.load_document("board.base").unwrap().source(),
            configured.source()
        );
        key(&mut app, KeyCode::Esc);
        assert_eq!(
            app.storage.load_document("public.md").unwrap().source(),
            public_before.source()
        );
        assert_eq!(fs::read(&path).unwrap(), image_before);
        assert_eq!(
            fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
            b"not a public document"
        );
    }
}

#[test]
fn covers_preserve_hidden_search_columns_and_show_color_private_and_disabled_states() {
    let (_root, mut app) = fixture();
    let note = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(
            &note,
            &note
                .source()
                .replace("score: 2", "score: 2\r\ncover: '#a2f'"),
        )
        .unwrap();
    let base = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&base,"filters: file.ext == 'md'\nviews: [{type: cards, name: Covers, order: [file.name, score], image: note.cover}]\n").unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    let mut state = app.bases_state.take().unwrap();
    terminal
        .draw(|frame| state.overlay_render(frame, AREA, &mut app))
        .unwrap();
    app.bases_state = Some(state);
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == ratatui::style::Color::Rgb(170, 34, 255))
    );
    assert_eq!(app.bases_state.as_ref().unwrap().media.stats(), (0, 0, 0));
    key(&mut app, KeyCode::Char('/'));
    event(&mut app, Event::Paste("#a2f".into()));
    idle(&mut app);
    assert_eq!(
        app.bases_state
            .as_ref()
            .unwrap()
            .result()
            .unwrap()
            .rows
            .len(),
        0,
        "cover-only values must not match displayed-property search"
    );
    key(&mut app, KeyCode::Esc);
    idle(&mut app);
    let note = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(
            &note,
            &note.source().replace("'#a2f'", "'[[private.clin]]'"),
        )
        .unwrap();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    assert!(render(&mut app).contains("unresolv"));
    image::RgbaImage::new(16, 16)
        .save(app.storage.notes_dir.join("public.png"))
        .unwrap();
    let note = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(
            &note,
            &note.source().replace("[[private.clin]]", "[[public.png]]"),
        )
        .unwrap();
    app.config.features.images = FeatureState::Disabled;
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    assert!(render(&mut app).contains("Images disabled"));
    assert_eq!(app.bases_state.as_ref().unwrap().media.stats(), (0, 0, 0));
    key(&mut app, KeyCode::Char('v'));
    assert!(app.bases_state.as_ref().unwrap().dialog.is_none());
    assert!(app.status.contains("enable images"));
}
