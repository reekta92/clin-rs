use super::*;
use crate::markdown::{MarkdownRenderer, MdRenderOpts, RenderViewport};

pub(super) fn markdown(source: &str, host: Option<&str>) -> MarkdownRenderer {
    let mut renderer = MarkdownRenderer::new();
    renderer.base_host = host.map(str::to_owned);
    renderer.render_with(
        source,
        100,
        &crate::app_theme::AppThemeColors::default(),
        &MdRenderOpts::default(),
        RenderViewport {
            start: 0,
            height: 28,
        },
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while renderer.is_pending() {
        renderer.poll();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    renderer
}
pub(super) fn screen(app: &mut App, renderer: &MarkdownRenderer) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
    app.base_embeds.begin_frame();
    terminal
        .draw(|frame| {
            let document = renderer.document().unwrap();
            frame.render_widget(crate::markdown::MarkdownWidget::new(document, 0..28), AREA);
            app.base_embeds.draw(frame, AREA, renderer, 0..28, app);
            app.base_embeds.draw_popup(frame, app);
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}
fn rendered(app: &mut App, renderer: &MarkdownRenderer, expected: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let screen = screen(app, renderer);
        app.poll_bases();
        if screen.contains(expected) {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "expected {expected:?}: {screen:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn app_screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, app, crate::editor::EditFocus::Body))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn embedded_event(app: &mut App, event: Event) {
    assert!(app.base_embed_event(&event, AREA));
}
fn embedded_key(app: &mut App, code: KeyCode) {
    embedded_event(app, Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}

#[test]
fn embedded_layouts_hide_bars_use_full_region_and_restore_fullscreen_bars() {
    for layout in ["table", "list", "cards", "kanban"] {
        for inline in [false, true] {
            let (_root, mut app) = fixture();
            app.app_theme.hint_bar_style = crate::config::HintBarStyle::Classic;
            for index in 0..7 {
                app.storage
                    .create_document(&format!("row{index}.md"), "Body")
                    .unwrap();
            }
            let definition = format!(
                "filters: file.ext == 'md'\nviews: [{{type: {layout}, name: Test, order: [file.name]{}}}]\n",
                if layout == "kanban" {
                    ", groupBy: {property: status}"
                } else {
                    ""
                },
            );
            let base = app.storage.load_document("board.base").unwrap();
            app.storage.save_document(&base, &definition).unwrap();
            let source = if inline {
                format!("```base\n{definition}```\n")
            } else {
                "![[board.base#Test]]\n".into()
            };
            let host = app.storage.load_document("public.md").unwrap();
            app.storage.save_document(&host, &source).unwrap();
            app.bases_state = None;
            app.mode = ViewMode::List;
            app.invalidate_bases();
            let renderer = markdown(&source, Some("public.md"));
            let output = rendered(&mut app, &renderer, "public.md");
            assert!(!output.contains("Bases ·"), "{layout}/{inline}: {output}");
            assert!(!output.contains("Esc back"), "{layout}/{inline}: {output}");
            if layout == "list" {
                for index in 0..7 {
                    assert!(output.contains(&format!("row{index}.md")), "{output}");
                }
            }
            expand_first(&mut app, &renderer);
            assert_eq!(app.mode, ViewMode::Bases);
            let fullscreen = render(&mut app);
            assert!(fullscreen.contains("Bases ·"));
            assert!(fullscreen.contains("Esc back"));
            crate::finish_overlay_event(
                &mut app,
                crate::overlay::ViewKind::Bases,
                OverlayResult::Exit,
            );
            let restored = screen(&mut app, &renderer);
            assert!(!restored.contains("Bases ·"));
            assert!(!restored.contains("Esc back"));
        }
    }
}

#[test]
fn embedded_dialogs_keep_modal_focus_and_custom_palette_expands_without_private_context() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.mode = ViewMode::List;
    let renderer = markdown("![[board.base#List]]", Some("public.md"));
    rendered(&mut app, &renderer, "• public.md, 2");
    embedded_event(
        &mut app,
        Event::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 4,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }),
    );
    embedded_key(&mut app, KeyCode::Char('n'));
    assert!(app.base_embeds.has_popup());
    screen(&mut app, &renderer);
    let popup = crate::ui::centered_rect(crate::ui::PopupSize::Medium, AREA);
    // A click inside the popup, outside the embedded region, must remain modal.
    embedded_event(
        &mut app,
        Event::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: popup.x + 2,
            row: popup.y + 3,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(app.base_embeds.has_popup());
    embedded_key(&mut app, KeyCode::Esc);
    app.keybinds
        .bases
        .insert(
            crate::keybinds::BasesAction::CommandPalette,
            vec![crate::keybinds::KeyCombo::parse("g p").unwrap()],
        )
        .unwrap();
    embedded_key(&mut app, KeyCode::Char('g'));
    embedded_key(&mut app, KeyCode::Char('p'));
    assert_eq!(app.mode, ViewMode::Bases);
    assert!(
        app.command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.id == "bases.edit")
    );
    assert!(
        app.command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .all(|item| !item.id.contains("decrypt") && !item.id.contains("encrypt"))
    );
    app.command_palette = None;
    crate::finish_overlay_event(
        &mut app,
        crate::overlay::ViewKind::Bases,
        OverlayResult::Exit,
    );
    assert_eq!(app.mode, ViewMode::List);
    assert!(screen(&mut app, &renderer).contains("• public.md, 2"));
}

#[test]
fn palette_f6_expands_focused_embeds_in_notes_editor_and_canvas_and_keeps_dialogs_modal() {
    for mode in [ViewMode::List, ViewMode::Edit, ViewMode::Canvas] {
        let (_root, mut app) = fixture();
        app.bases_state = None;
        app.mode = ViewMode::List;
        if mode == ViewMode::Edit {
            app.load_and_open_note("public.md", None);
        }
        if mode == ViewMode::Canvas {
            let path = app.storage.notes_dir.join("map.canvas");
            fs::write(&path, "{\"nodes\":[],\"edges\":[]}").unwrap();
            app.canvas_state = Some(
                crate::pinstar_adapter::PinstarPlugin::new(
                    &path,
                    &app.config,
                    app.keybinds.clone(),
                    crate::keybinds::KeyMatcher::default(),
                    None,
                    &app.storage.data_dir,
                )
                .unwrap(),
            );
        }
        app.mode = mode;
        let renderer = markdown("![[board.base#List]]", Some("public.md"));
        rendered(&mut app, &renderer, "• public.md, 2");
        embedded_event(
            &mut app,
            Event::Mouse(crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column: 4,
                row: 1,
                modifiers: KeyModifiers::NONE,
            }),
        );
        embedded_key(&mut app, KeyCode::Char('n'));
        assert!(app.base_embeds.has_popup());
        crate::palette_tests::drive(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE)),
        );
        assert_eq!(app.mode, mode);
        assert!(app.base_embeds.has_popup());
        embedded_key(&mut app, KeyCode::Esc);
        crate::palette_tests::drive(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE)),
        );
        assert_eq!(app.mode, ViewMode::Bases);
        assert!(
            app.command_palette
                .as_ref()
                .unwrap()
                .context_note_id
                .is_none()
        );
        assert!(
            app.command_palette
                .as_ref()
                .unwrap()
                .items
                .iter()
                .any(|item| item.id == "bases.edit")
        );
        assert!(crate::events::handle_global_popups_and_palette(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AREA
        ));
        crate::finish_overlay_event(
            &mut app,
            crate::overlay::ViewKind::Bases,
            OverlayResult::Exit,
        );
        assert_eq!(app.mode, mode);
        rendered(&mut app, &renderer, "• public.md, 2");
        embedded_event(
            &mut app,
            Event::Mouse(crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column: 4,
                row: 1,
                modifiers: KeyModifiers::NONE,
            }),
        );
        embedded_key(&mut app, KeyCode::Char(':'));
        assert_eq!(app.mode, ViewMode::Bases);
        let palette = app.command_palette.as_ref().unwrap();
        assert_eq!(palette.tabs[palette.active_tab].0, "Bases");
        assert!(!app.base_embeds.has_popup());
        crate::palette_tests::drive(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        );
        crate::finish_overlay_event(
            &mut app,
            crate::overlay::ViewKind::Bases,
            OverlayResult::Exit,
        );
        assert_eq!(app.mode, mode);
    }
}

#[test]
fn shared_markdown_geometry_never_shares_live_results_between_vaults() {
    let (_root, mut first) = fixture();
    let (_other, mut second) = fixture();
    for app in [&mut first, &mut second] {
        app.bases_state = None;
        app.mode = ViewMode::List;
    }
    let public = second.storage.load_document("public.md").unwrap();
    second
        .storage
        .save_document(&public, &public.source().replace("score: 2", "score: 999"))
        .unwrap();
    second.invalidate_bases();
    let first_renderer = markdown("![[board.base#List]]", Some("public.md"));
    let second_renderer = markdown("![[board.base#List]]", Some("public.md"));
    rendered(&mut first, &first_renderer, "• public.md, 2");
    rendered(&mut second, &second_renderer, "• public.md, 999");
    assert!(!screen(&mut first, &first_renderer).contains("999"));
    second.base_embeds.begin_frame();
    second.poll_bases();
    rendered(&mut first, &first_renderer, "• public.md, 2");
}
fn app_rendered(app: &mut App, expected: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_bases();
        app.poll_editor_renderers();
        let screen = app_screen(app);
        if screen.contains(expected) {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "expected {expected:?}: {screen:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn real_editor_preview_and_active_file_sidebar_keep_unsaved_host_and_native_focus() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.mode = ViewMode::List;
    let host = app.storage.load_document("public.md").unwrap();
    let host = app
        .storage
        .save_document(&host, "---\nscore: 12\n---\n![[board.base#List]]\n")
        .unwrap();
    app.open_public_document(host.clone());
    app.editor.editor_preview_enabled = true;
    app.update_editor_markdown_preview();
    app_rendered(&mut app, "• public.md, 12");
    app.editor.body = crate::editor_document::EditorDocument::from_source(
        "---\nscore: 87\n---\n![[board.base#List]]\nUnsaved body\n",
    );
    app.update_editor_markdown_preview();
    app_rendered(&mut app, "• public.md, 87");
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        host.source()
    );
    app.editor
        .body
        .move_cursor(ratatui_textarea::CursorMove::Jump(3, 8));
    app.toggle_bases_pane();
    assert_eq!(app.editor.sidebar, crate::editor::EditSidebar::Bases);
    app_rendered(&mut app, "• public.md, 87");
    assert!(app.editor.body.text().contains("Unsaved body"));
    app.toggle_bases_pane();
    assert_eq!(app.editor.sidebar, crate::editor::EditSidebar::None);
    app.editor.editing_id = Some("private.clin".into());
    app.toggle_bases_pane();
    assert_eq!(app.editor.sidebar, crate::editor::EditSidebar::None);
    assert!(app.editor.bases_sidebar_renderer.is_none());
    assert!(app.status.contains("public containing file"));
}

#[test]
fn canvas_file_and_inline_text_nodes_use_live_native_regions_and_exact_json_splices() {
    use crate::overlay::OverlayView;
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let text = "```base\nfilters: file.ext == 'md'\nviews: [{type: list, name: Inline, order: [file.name, score]}]\n```\n";
    let canvas = format!(
        "{{\n  \"opaque\": {{\"keep\": true}},\n  \"nodes\": [{{\"id\":\"inline\",\"type\":\"text\",\"x\":-48,\"y\":-10,\"width\":46,\"height\":18,\"text\":{}}},{{\"id\":\"file\",\"type\":\"file\",\"x\":0,\"y\":-10,\"width\":46,\"height\":18,\"file\":\"board.base\",\"subpath\":\"#List\"}}], \"edges\": []\n}}",
        serde_json::to_string(text).unwrap()
    );
    app.storage.create_document("test.canvas", &canvas).unwrap();
    app.invalidate_bases();
    app.mode = ViewMode::Canvas;
    let mut plugin = crate::pinstar_adapter::PinstarPlugin::new(
        &app.storage.notes_dir.join("test.canvas"),
        &app.config,
        app.keybinds.clone(),
        crate::keybinds::KeyMatcher::default(),
        None,
        &app.storage.data_dir,
    )
    .unwrap();
    plugin.state.zoom = 1.0;
    plugin.state.viewport_x = 0.0;
    plugin.state.viewport_y = 0.0;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_bases();
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        app.base_embeds.begin_frame();
        terminal
            .draw(|frame| plugin.overlay_render(frame, AREA, &mut app))
            .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        if screen.matches("• public.md, 2").count() == 2 {
            break;
        }
        assert!(Instant::now() < deadline, "{screen:?}");
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut origin = crate::bases::embedding::InlineOrigin::new(text.into(), 1..3).unwrap();
    origin.rebase_canvas(&canvas, "inline").unwrap();
    let changed = origin
        .replace(&canvas, "views: [{type: list, name: Changed}]\n")
        .unwrap();
    let encoded =
        serde_json::to_string("```base\nviews: [{type: list, name: Changed}]\n```\n").unwrap();
    assert_eq!(
        changed,
        canvas.replace(&serde_json::to_string(text).unwrap(), &encoded)
    );
    assert_eq!(
        app.storage.load_document("test.canvas").unwrap().source(),
        canvas
    );
}

#[test]
fn inline_reference_text_and_fence_limits_preserve_surrounding_markdown() {
    let renderer = markdown(
        "Before ![[board.base#List]] after\n\n`![[private.base]]`\n",
        Some("public.md"),
    );
    let document = renderer.document().unwrap();
    assert_eq!(document.base_slots.len(), 1);
    let text: String = document
        .lines()
        .iter()
        .flat_map(|line| line.spans.iter().map(|span| &*span.text))
        .collect();
    assert!(text.contains("Before") && text.contains("after") && text.contains("private.base"));
    let multiple = markdown(
        "![[board.base#List]] ![[board.base#Public]]",
        Some("public.md"),
    );
    assert_eq!(multiple.document().unwrap().base_slots.len(), 2);
    let renderer = markdown(&"![[board.base]]\n\n".repeat(40), Some("public.md"));
    let document = renderer.document().unwrap();
    assert_eq!(document.base_slots.len(), 32);
    assert!(document.lines().iter().any(|line| {
        line.spans
            .iter()
            .any(|span| span.text.contains("at most 32"))
    }));
}

#[test]
fn named_and_inline_embeds_render_live_native_values_and_refresh_without_source_changes() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.mode = ViewMode::Edit;
    let baseline = app.storage.load_document("public.md").unwrap();
    let source = "![[board.base#List]]\n\n```base\nfilters: file.ext == 'md'\nviews: [{type: table, order: [file.name, score]}]\n```\nAfter\n";
    app.storage.save_document(&baseline, source).unwrap();
    let renderer = markdown(source, Some("public.md"));
    assert_eq!(renderer.document().unwrap().base_slots.len(), 2);
    assert!(renderer.document().unwrap().last_non_blank_line().unwrap() >= 15);
    let output = rendered(&mut app, &renderer, "After");
    assert!(output.contains("Loading"));
    rendered(&mut app, &renderer, "• public.md");
    let public = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(&public, &format!("---\nscore: 987\n---\n{source}"))
        .unwrap();
    app.invalidate_bases();
    rendered(&mut app, &renderer, "987");
    assert_eq!(
        app.storage.load_document("board.base").unwrap().source(),
        "# retained\nplugin: {opaque: true}\nfilters: file.ext == 'md'\nviews:\n  - type: table\n    name: Public\n    order: [file.name, score]\n  - type: list\n    name: List\n    order: [file.name, score]\n"
    );
    assert_eq!(
        fs::read(app.storage.notes_dir.join("private.clin")).unwrap(),
        b"not a public document"
    );
}

#[test]
fn private_hosts_missing_and_ambiguous_views_show_errors_and_never_enter_queries() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.mode = ViewMode::List;
    let private = markdown("![[board.base]]", Some("private.clin"));
    assert!(screen(&mut app, &private).contains("public containing file"));
    let missing = markdown("![[board.base#Missing]]", Some("public.md"));
    rendered(&mut app, &missing, "named base view does not exist");
    let secret = markdown("![[private.base]]", None);
    rendered(&mut app, &secret, "missing or ambiguous");
    let snapshot = app.storage.load_document("board.base").unwrap();
    app.storage
        .save_document(
            &snapshot,
            "views: [{type: table, name: Duplicate}, {type: list, name: Duplicate}]\n",
        )
        .unwrap();
    app.invalidate_bases();
    let ambiguous = markdown("![[board.base#Duplicate]]", Some("public.md"));
    rendered(&mut app, &ambiguous, "base view name is ambiguous");
}

#[test]
fn inline_configuration_changes_only_anchored_yaml_and_stale_or_unsaved_hosts_reject() {
    use crate::bases::{
        embedding::{Definition, Request},
        eval::Environment,
        worker::BasesWorker,
    };
    let (_root, mut app) = fixture();
    let snapshot = app.storage.load_document("public.md").unwrap();
    let source = "Before\r\n```base\r\nviews: [{type: table, name: Inline}]\r\n```\r\nAfter\r\n";
    app.storage.save_document(&snapshot, source).unwrap();
    let mut worker = BasesWorker::new(app.storage.clone()).unwrap();
    worker.request_embed(
        Request {
            key: 1,
            session: 1,
            anchor: 0,
            host: Some("public.md".into()),
            view: None,
            host_source: Some(source.into()),
            body_only: false,
            canvas_node: None,
            definition: Definition::Inline {
                source: "views: [{type: table, name: Inline}]\n".into(),
                lines: 2..3,
            },
        },
        Environment::default(),
        String::new(),
        false,
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let loaded = loop {
        if let Some((key, loaded)) = worker.poll_completed() {
            assert_eq!(key, Some(1));
            break loaded.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    let edited = loaded
        .document
        .set_view_field(
            0,
            "name",
            Some(&serde_yaml_ng::Value::String("Changed".into())),
        )
        .unwrap();
    let changed = loaded.configuration_source(&mut app, &edited).unwrap();
    assert_eq!(changed, source.replace("Inline", "\"Changed\""));
    app.storage
        .save_document(&loaded.snapshot, "external edit")
        .unwrap();
    assert!(
        app.storage
            .save_document(&loaded.snapshot, &changed)
            .is_err()
    );
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        "external edit"
    );
}

fn expand_first(app: &mut App, renderer: &MarkdownRenderer) {
    rendered(app, renderer, "public.md");
    for kind in [
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
    ] {
        embedded_event(
            app,
            Event::Mouse(crossterm::event::MouseEvent {
                kind,
                column: 4,
                row: 1,
                modifiers: KeyModifiers::NONE,
            }),
        );
    }
    embedded_event(
        app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL)),
    );
    app.command_palette = None;
    idle(app);
}
fn rename_inline(app: &mut App, name: &str) -> anyhow::Result<()> {
    let mut state = app.bases_state.take().unwrap();
    let loaded = state.loaded.as_ref().unwrap();
    let target = crate::bases::authoring::Target::Config {
        command: crate::bases::authoring::Command::View("name"),
        baseline: loaded.snapshot.clone(),
        view: loaded.view,
    };
    let result = state.submit(app, &target, name);
    app.bases_state = Some(state);
    result
}
#[test]
fn inline_save_undo_redo_synchronizes_editor_and_rejects_pending_host_changes() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let old = app.storage.load_document("public.md").unwrap();
    let source = "```base\r\nfilters: file.ext == 'md'\r\nviews: [{type: list, name: Inline, order: [file.name]}]\r\n```\r\nAfter\r\n";
    let host = app.storage.save_document(&old, source).unwrap();
    app.open_public_document(host);
    let renderer = markdown(source, Some("public.md"));
    expand_first(&mut app, &renderer);
    rename_inline(&mut app, "Changed").unwrap();
    idle(&mut app);
    let changed = app.storage.load_document("public.md").unwrap();
    assert_eq!(changed.source(), source.replace("Inline", "\"Changed\""));
    assert_eq!(
        app.editor.public_document_source().unwrap(),
        changed.source()
    );
    key(&mut app, KeyCode::Char('u'));
    idle(&mut app);
    assert_eq!(app.editor.public_document_source().unwrap(), source);
    event(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
    );
    idle(&mut app);
    assert_eq!(
        app.editor.public_document_source().unwrap(),
        changed.source()
    );
    app.editor.body.insert_str("unsaved");
    assert!(rename_inline(&mut app, "Rejected").is_err());
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        changed.source()
    );
    assert!(app.editor.body.text().contains("unsaved"));
}
#[test]
fn ordinary_host_promotion_preserves_bytes_title_filename_and_opening_baseline() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let old = app.storage.load_document("public.md").unwrap();
    let source = "---\r\n# retained\r\nscore: 2\r\n---\r\n![[board.base#List]]\r\n\r\n";
    let host = app.storage.save_document(&old, source).unwrap();
    app.load_and_open_note("public.md", None);
    let cursor = app.editor.body.cursor();
    app.guard_embed_host(&host).unwrap();
    assert!(app.editor.public_document_body_only);
    assert_eq!(app.editor.body.cursor(), cursor);
    assert_eq!(app.editor.public_document_source().unwrap(), source);
    app.autosave().unwrap();
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        source
    );
    app.editor.title_editor.select_all();
    app.editor.title_editor.insert_str("New title");
    app.autosave().unwrap();
    let renamed = app.storage.load_document("public.md").unwrap();
    assert!(renamed.source().contains("title: \"New title\""));
    assert!(renamed.source().contains("# retained\r\nscore: 2\r\n"));
    assert!(renamed.source().ends_with("![[board.base#List]]\r\n\r\n"));
    assert!(!app.storage.notes_dir.join("New title.md").exists());
    app.load_and_open_note("public.md", None);
    let external = app
        .storage
        .save_document(&renamed, &renamed.source().replace("score: 2", "score: 3"))
        .unwrap();
    app.guard_embed_host(&external).unwrap();
    assert!(app.autosave().is_err());
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        external.source()
    );
}
#[test]
fn ordinary_host_promotion_reuses_queued_recovery_identity() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.load_and_open_note("public.md", None);
    let baseline = app.editor.note_baseline.clone().unwrap();
    app.editor.body.insert_str("pending ");
    app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
    app.write_draft();
    app.guard_embed_host(&baseline).unwrap();
    assert_eq!(
        app.editor.public_document.as_ref().unwrap().recovery_id(),
        baseline.recovery_id()
    );
    app.autosave().unwrap();
    app.flush_drafts().unwrap();
    assert_eq!(
        app.storage.recover_documents().unwrap(),
        Vec::<String>::new()
    );
    assert!(
        app.storage
            .load_document("public.md")
            .unwrap()
            .source()
            .contains("pending ")
    );
    assert_eq!(
        fs::read_dir(app.storage.data_dir.join(".clin/document-recovery-v2"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn metadata_rebase_retains_unsaved_body_and_recovery_draft_through_undo() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let host = app.storage.load_document("public.md").unwrap();
    app.load_and_open_note("public.md", None);
    app.guard_embed_host(&host).unwrap();
    app.editor.body.insert_str("unsaved");
    app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
    app.write_draft();
    let after = app
        .storage
        .save_document(&host, &host.source().replace("score: 2", "score: 7"))
        .unwrap();
    app.synchronize_public_change(&host, &after);
    assert!(
        app.editor
            .public_document_source()
            .unwrap()
            .contains("score: 7")
    );
    assert!(app.editor.body.text().contains("unsaved"));
    let restored = app.storage.save_document(&after, host.source()).unwrap();
    app.synchronize_public_change(&after, &restored);
    app.flush_drafts().unwrap();
    assert!(
        app.editor
            .public_document_source()
            .unwrap()
            .contains("score: 2")
    );
    let expected = app.editor.public_document_source().unwrap();
    assert_eq!(
        app.storage.recover_documents().unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        expected
    );
}
#[test]
fn private_render_option_disables_recognition_before_shared_cache_lookup() {
    let source = "![[board.base]]";
    let public = markdown(source, Some("public.md"));
    assert_eq!(public.document().unwrap().base_slots.len(), 1);
    let mut private = MarkdownRenderer::new();
    let opts = MdRenderOpts {
        render_bases: false,
        ..MdRenderOpts::default()
    };
    private.render_with(
        source,
        100,
        &crate::app_theme::AppThemeColors::default(),
        &opts,
        RenderViewport {
            start: 0,
            height: 28,
        },
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while private.is_pending() {
        private.poll();
        assert!(Instant::now() < deadline);
    }
    assert_eq!(private.document().unwrap().base_slots, []);
}

#[test]
fn source_handoff_returns_to_saved_inline_definition_and_suspended_host() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let old = app.storage.load_document("public.md").unwrap();
    let source = "```base\nfilters: file.ext == 'md'\nviews: [{type: list, name: Inline, order: [file.name]}]\n```\n";
    let host = app.storage.save_document(&old, source).unwrap();
    app.open_public_document(host);
    let renderer = markdown(source, Some("public.md"));
    expand_first(&mut app, &renderer);
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| app.mode == ViewMode::Edit);
    assert!(app.embedded_host_editor.is_some());
    app.editor.body.select_all();
    app.editor
        .body
        .insert_str(source.replace("Inline", "Manual"));
    app.autosave().unwrap();
    app.back_to_list(None, None);
    assert_eq!(app.mode, ViewMode::Bases);
    idle(&mut app);
    assert_eq!(
        app.bases_state.as_ref().unwrap().named_view.as_deref(),
        Some("Manual")
    );
    rename_inline(&mut app, "Final").unwrap();
    idle(&mut app);
    crate::finish_overlay_event(
        &mut app,
        crate::overlay::ViewKind::Bases,
        OverlayResult::Exit,
    );
    assert_eq!(app.mode, ViewMode::Edit);
    assert!(app.editor.body.text().contains("Final"));
    app.autosave().unwrap();
    assert!(
        app.storage
            .load_document("public.md")
            .unwrap()
            .source()
            .contains("Final")
    );
}

#[test]
fn palette_failed_navigation_restores_taken_boxed_editor_and_expanded_bases_route() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let old = app.storage.load_document("public.md").unwrap();
    let source = "```base\nfilters: file.ext == 'md'\nviews: [{type: list, name: Inline, order: [file.name]}]\n```\n";
    let host = app.storage.save_document(&old, source).unwrap();
    app.open_public_document(host);
    let renderer = markdown(source, Some("public.md"));
    expand_first(&mut app, &renderer);
    key(&mut app, KeyCode::Enter);
    wait(&mut app, |app| app.mode == ViewMode::Edit);
    let active = app.editor.body.text();
    let baseline = app.storage.load_document("public.md").unwrap();
    fs::write(app.storage.notes_dir.join("public.md"), "External change\n").unwrap();
    let boxed = app.embedded_host_editor.as_mut().unwrap();
    boxed.public_document = None;
    boxed.editing_id = Some("public.md".into());
    boxed.note_baseline = Some(baseline);
    boxed.body.insert_str("pending host ");
    let pending = boxed.body.text();
    assert!(crate::actions::execute_action("notes.open", &mut app, None).is_err());
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(app.return_mode, Some(ViewMode::Bases));
    assert_eq!(app.editor.body.text(), active);
    assert_eq!(
        app.embedded_host_editor.as_ref().unwrap().body.text(),
        pending
    );
    assert!(app.bases_state.is_some());
    app.flush_drafts().unwrap();
    assert!(
        fs::read_dir(app.storage.data_dir.join(".clin/document-recovery-v2"))
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("draft-"))
    );
    app.back_to_list(None, None);
    assert_eq!(app.mode, ViewMode::Bases);
    crate::finish_overlay_event(
        &mut app,
        crate::overlay::ViewKind::Bases,
        OverlayResult::Exit,
    );
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(app.editor.body.text(), pending);
}
#[test]
fn canvas_inline_write_and_history_reload_native_state_and_reject_pending_raw_editor() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let text = "```base\nfilters: file.ext == 'md'\nviews: [{type: list, name: Inline, order: [file.name]}]\n```\n";
    let source = format!(
        "{{\n  \"opaque\":true, \"nodes\":[{{\"id\":\"inline\",\"type\":\"text\",\"x\":0,\"y\":0,\"width\":400,\"height\":200,\"text\":{}}}], \"edges\":[]\n}}",
        serde_json::to_string(text).unwrap()
    );
    app.storage.create_document("test.canvas", &source).unwrap();
    app.invalidate_bases();
    app.canvas_state = Some(
        crate::pinstar_adapter::PinstarPlugin::new(
            &app.storage.notes_dir.join("test.canvas"),
            &app.config,
            app.keybinds.clone(),
            crate::keybinds::KeyMatcher::default(),
            None,
            &app.storage.data_dir,
        )
        .unwrap(),
    );
    app.mode = ViewMode::Canvas;
    let mut renderer = markdown(text, Some("test.canvas"));
    renderer.base_canvas_node = Some("inline".into());
    expand_first(&mut app, &renderer);
    rename_inline(&mut app, "Changed").unwrap();
    idle(&mut app);
    let changed = app.storage.load_document("test.canvas").unwrap();
    assert_eq!(
        changed.source(),
        source.replace("Inline", "\\\"Changed\\\"")
    );
    assert!(
        app.canvas_state
            .as_ref()
            .unwrap()
            .state
            .raw_editor
            .lines()
            .join("\n")
            .contains("Changed")
    );
    key(&mut app, KeyCode::Char('u'));
    idle(&mut app);
    assert_eq!(
        app.storage.load_document("test.canvas").unwrap().source(),
        source
    );
    event(
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
    );
    idle(&mut app);
    app.canvas_state
        .as_mut()
        .unwrap()
        .state
        .raw_editor
        .insert_str("invalid pending JSON");
    assert!(rename_inline(&mut app, "Rejected").is_err());
    let mut state = app.bases_state.take().unwrap();
    assert!(state.replay(&mut app, false).is_err());
    app.bases_state = Some(state);
    assert_eq!(
        app.storage.load_document("test.canvas").unwrap().source(),
        changed.source()
    );
    assert!(
        app.canvas_state
            .as_ref()
            .unwrap()
            .state
            .raw_editor
            .lines()
            .join("\n")
            .contains("invalid pending JSON")
    );
}

#[test]
fn expanding_preview_preserves_original_bases_source_handoff_return_stack() {
    let (_root, mut app) = fixture();
    let snapshot = app.storage.load_document("public.md").unwrap();
    app.open_public_document(snapshot);
    assert_eq!(app.return_mode, Some(ViewMode::Bases));
    let renderer = markdown("![[board.base#List]]", Some("public.md"));
    expand_first(&mut app, &renderer);
    crate::finish_overlay_event(
        &mut app,
        crate::overlay::ViewKind::Bases,
        OverlayResult::Exit,
    );
    assert_eq!(app.mode, ViewMode::Edit);
    assert_eq!(app.return_mode, Some(ViewMode::Bases));
    assert_eq!(app.bases_state.as_ref().unwrap().id, "board.base");
    app.back_to_list(None, None);
    assert_eq!(app.mode, ViewMode::Bases);
    idle(&mut app);
    assert!(app.bases_state.as_ref().unwrap().result().is_some());
}

#[test]
fn notes_preview_and_linked_note_popup_use_the_public_containing_file() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    let host = app.storage.load_document("public.md").unwrap();
    app.storage
        .save_document(&host, "---\nscore: 2\n---\n![[board.base#List]]\n")
        .unwrap();
    app.request_notes_reconcile();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        while let Ok(event) = app.catalog_event_rx.try_recv() {
            app.handle_catalog_event(event);
        }
        if app.catalog_status.is_none() && app.notes.iter().any(|note| note.id == "public.md") {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    app.mode = ViewMode::List;
    app.list.notes_layout = crate::config::NotesLayout::Tree;
    app.list.preview_enabled = true;
    app.refresh_visual_list();
    let index = app
        .notes
        .iter()
        .position(|note| note.id == "public.md")
        .unwrap();
    app.list.visual_index = app
        .list
        .visual_list
        .iter()
        .position(|item| {
            matches!(item,
        crate::app::VisualItem::Note { summary_idx, .. } if *summary_idx == index)
        })
        .unwrap();
    app.update_preview();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(crate::app::PreviewContent::Markdown(renderer)) = &mut app.list.preview_content
        {
            renderer.poll();
        }
        app.poll_bases();
        if app_screen(&mut app).contains("• public.md, 2") {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let snapshot = app.storage.load_document("public.md").unwrap();
    app.open_public_document(snapshot);
    app.editor.body = crate::editor_document::EditorDocument::from_source("[[public.md]]");
    app.editor
        .body
        .move_cursor(ratatui_textarea::CursorMove::Jump(0, 4));
    app.open_link_preview();
    app_rendered(&mut app, "• public.md, 2");
    assert!(app.editor.link_preview);
    assert_eq!(
        app.editor
            .link_preview_renderer
            .as_ref()
            .unwrap()
            .base_host
            .as_deref(),
        Some("public.md")
    );
}

#[test]
fn frontmatter_pending_header_rejects_bases_writes_for_live_and_suspended_editor() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.mode = ViewMode::List;
    app.load_and_open_note("public.md", None);
    let before = app.storage.load_document("public.md").unwrap();
    app.guard_embed_host(&before).unwrap();
    app.editor
        .frontmatter
        .as_mut()
        .unwrap()
        .patch("score".into(), Some(serde_yaml_ng::Value::Number(7.into())))
        .unwrap();
    for suspended in [false, true] {
        if suspended {
            app.embedded_host_editor = Some(Box::new(std::mem::take(&mut app.editor)));
        }
        let task = super::super::write::WriteTask::for_app(
            &app,
            super::super::write::Operation::Commit {
                snapshots: vec![before.clone()],
                key: "score".into(),
                value: Some(serde_yaml_ng::Value::Number(9.into())),
            },
        );
        assert!(task.poll().unwrap().is_err());
        assert_eq!(
            app.storage.load_document("public.md").unwrap().source(),
            before.source()
        );
    }
}

#[test]
fn frontmatter_metadata_only_edits_refresh_real_preview_and_sidebar_before_save() {
    let (_root, mut app) = fixture();
    app.bases_state = None;
    app.mode = ViewMode::List;
    let before = app.storage.load_document("public.md").unwrap();
    let host = app
        .storage
        .save_document(&before, "---\nscore: 2\n---\n![[board.base#List]]\n")
        .unwrap();
    app.load_and_open_note("public.md", None);
    app.editor.editor_preview_enabled = true;
    app.update_editor_markdown_preview();
    app_rendered(&mut app, "• public.md, 2");
    let body_revision = app.editor.body.revision();
    app.editor
        .frontmatter
        .as_mut()
        .unwrap()
        .patch(
            "score".into(),
            Some(serde_yaml_ng::Value::Number(87.into())),
        )
        .unwrap();
    app_rendered(&mut app, "• public.md, 87");
    assert_eq!(app.editor.body.revision(), body_revision);
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        host.source()
    );
    app.editor
        .body
        .move_cursor(ratatui_textarea::CursorMove::Jump(0, 8));
    app.toggle_bases_pane();
    assert_eq!(app.editor.sidebar, crate::editor::EditSidebar::Bases);
    app_rendered(&mut app, "• public.md, 87");
    assert_eq!(
        app.storage.load_document("public.md").unwrap().source(),
        host.source()
    );
}
