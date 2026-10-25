//! End-to-end view scheduling regressions and opt-in release measurements.
use crate::{
    app::{App, EditFocus, ViewMode},
    config::{ClinConfig, NotesLayout},
    storage::Storage,
};
use ratatui::{Terminal, backend::TestBackend};
use std::{
    fs,
    time::{Duration, Instant},
};

fn fixture(notes: usize) -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let storage = Storage {
        data_dir: root.path().into(),
        config_dir: root.path().join("config"),
        notes_dir: root.path().join("notes"),
        templates_dir: root.path().join("templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: crate::frontmatter::WritePolicy::default(),
    };
    for path in [
        &storage.config_dir,
        &storage.notes_dir,
        &storage.templates_dir,
    ] {
        fs::create_dir_all(path).unwrap();
    }
    crate::config::set_config_path_override(storage.config_dir.join("config.toml"));
    let mut config = ClinConfig::default();
    config.core.storage_path = Some(storage.data_dir.clone());
    config.core.confirm_on_quit = false;
    config.save().unwrap();
    fs::create_dir(storage.notes_dir.join("bulk")).unwrap();
    for index in 0..notes {
        fs::write(
            storage.notes_dir.join(format!("bulk/note{index:05}.md")),
            format!(
                "# Note {index}\n[[note{:05}]]\nBody.\n",
                (index + 1) % notes
            ),
        )
        .unwrap();
    }
    fs::write(
        storage.notes_dir.join("selected.md"),
        "# Header\nparagraph\n## Child\ncontent\n".repeat(if notes > 0 { 1250 } else { 1 }),
    )
    .unwrap();
    let nodes: Vec<_> = (0..if notes > 0 {500} else {0}).map(|index| serde_json::json!({"id": format!("n{index}"), "type":"text", "x": index%25*120, "y": index/25*80, "width":100, "height":60, "text":format!("Node {index}")})).collect();
    let edges: Vec<_> = (1..nodes.len()).map(|index| serde_json::json!({"id":format!("e{index}"), "fromNode":format!("n{}", index-1), "toNode":format!("n{index}")})).collect();
    fs::write(
        storage.notes_dir.join("map.canvas"),
        serde_json::json!({"nodes":nodes,"edges":edges}).to_string(),
    )
    .unwrap();
    let mut drawing = crate::draw::state::DrawData::default();
    for index in 0..if notes > 0 { 500 } else { 0 } {
        drawing.elements.push(crate::draw::state::DrawItem::new(
            crate::draw::state::DrawElement::Shape(crate::draw::state::Shape::Rect {
                x: f64::from(index % 25 * 40),
                y: f64::from(index / 25 * 40),
                width: 30.0,
                height: 30.0,
                color: (120, 170, 230),
            }),
        ));
    }
    fs::write(
        storage.notes_dir.join("art.draw"),
        serde_json::to_string(&drawing).unwrap(),
    )
    .unwrap();
    let mut app = App::new(storage).unwrap();
    app.confirm_on_quit = false;
    app.list.preview_enabled = false;
    app.list.calendar_enabled = false;
    app.list.sections.clear();
    app.list.notes_layout = NotesLayout::List;
    app.config.list.section_tabs = false;
    app.list.grid_folder.clear();
    app.refresh_visual_list();
    (root, app)
}
fn enter(app: &mut App, mode: ViewMode) {
    app.release_navigation_owners();
    app.mode = ViewMode::List;
    match mode {
        ViewMode::List => {}
        ViewMode::Edit => app.load_and_open_note("selected.md", None),
        ViewMode::Graph => app.open_graph_view(),
        ViewMode::Canvas => app.open_canvas_document("map.canvas").unwrap(),
        ViewMode::Draw => app.open_draw_document("art.draw").unwrap(),
        ViewMode::Outline => {
            let note = app.storage.load_note("selected.md").unwrap();
            app.outline_state = Some(crate::outline::state::OutlineState::new(
                "selected.md".into(),
                &note.title,
                &note.content,
                app.keybinds.clone(),
                crate::keybinds::KeyMatcher::default(),
            ));
            app.mode = mode;
        }
        ViewMode::Help => app.open_help_page(),
        ViewMode::Calendar => app.open_calendar_at(None).unwrap(),
        ViewMode::Setup => app.open_setup_view(),
        ViewMode::Bases => unreachable!("Bases has its own native acceptance benchmark"),
    }
    assert_eq!(app.mode, mode);
}

#[test]
fn catalog_batch_preserves_cursor_and_bulk_selection_by_identity() {
    use crate::list_view::{SortField, SortOrder, VisualItem};
    let _guard = crate::config::ConfigTestGuard::lock();
    let (_root, mut app) = fixture(3);
    app.list.sort_field = SortField::Title;
    app.list.sort_order = SortOrder::Ascending;
    app.list.folder_expanded.insert("bulk".into());
    app.list.grid_folder = "bulk".into();
    app.refresh_visual_list();
    let note_index = |app: &App, id: &str| {
        app.list.visual_list.iter().position(|item| matches!(item, VisualItem::Note { summary_idx, .. } if app.notes[*summary_idx].id == id)).unwrap()
    };
    let selected = "bulk/note00001.md";
    app.list.visual_index = note_index(&app, selected);
    app.list
        .selected_indices
        .insert(note_index(&app, "bulk/note00002.md"));
    app.editor.editing_id = Some("selected.md".into());
    let path = app.storage.notes_dir.join(selected);
    fs::write(&path, "---\ntitle: A first note\n---\nChanged\n").unwrap();
    let loaded = crate::app::catalog::load_notes_blocking(
        &app.storage,
        &app.notes_worker_pool,
        false,
        false,
    )
    .unwrap();
    let upserts = loaded
        .summaries
        .into_iter()
        .filter(|note| note.id == selected)
        .map(|note| {
            let stamp = loaded.map[&note.id].0;
            (note, stamp)
        })
        .collect();
    let generation = app
        .catalog_generation
        .load(std::sync::atomic::Ordering::SeqCst);
    app.handle_catalog_events([crate::app::catalog::CatalogEvent::Delta {
        generation,
        upserts,
        removed: vec![],
        folders: None,
        processed: 1,
        total: 1,
    }]);
    assert_eq!(app.list.visual_index, note_index(&app, selected));
    assert_eq!(
        app.list.selected_indices,
        [note_index(&app, "bulk/note00002.md")].into()
    );
}

#[test]
fn modified_status_tracks_undo_save_and_pending_property_input() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let _guard = crate::config::ConfigTestGuard::lock();
    let (_root, mut app) = fixture(0);
    enter(&mut app, ViewMode::Edit);
    let modified = |app: &App| {
        crate::statusline::StatuslineContext::for_view(app, ViewMode::Edit)
            .resolve("modified")
            .unwrap()
            .into_owned()
    };
    assert_eq!(modified(&app), "off");
    app.editor.body.insert_str("changed");
    assert_eq!(modified(&app), "on");
    app.editor.body.undo();
    assert_eq!(modified(&app), "off");
    app.editor.body.redo();
    app.autosave().unwrap();
    assert_eq!(modified(&app), "off");
    app.toggle_frontmatter();
    app.frontmatter_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(modified(&app), "on");
    app.frontmatter_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(modified(&app), "off");
}

#[test]
fn image_completions_reach_the_owner_and_worker_disconnect_clears_pending() {
    use crate::image_render::worker::{DecodedImage, ImageOutput};
    let _guard = crate::config::ConfigTestGuard::lock();
    let (_root, mut app) = fixture(0);
    let (sender, requests) = std::sync::mpsc::sync_channel(16);
    let (results, receiver) = std::sync::mpsc::sync_channel(2);
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.image_decode_tx = Some(sender.clone());
    app.image_decode_rx = Some(receiver);
    let key = std::path::PathBuf::from("shared.png");
    app.editor.image_cache.request(key.clone(), 128, &sender);
    let editor_job = requests.recv().unwrap();
    app.list.image_cache.request(key.clone(), 128, &sender);
    let list_job = requests.recv().unwrap();
    for job in [editor_job, list_job] {
        results
            .send(DecodedImage {
                key: job.key,
                request: job.request,
                result: Ok(ImageOutput::Decoded(image::DynamicImage::new_rgb8(8, 8))),
            })
            .unwrap();
    }
    assert!(app.poll_editor_image_results());
    assert!(app.editor.image_cache.get_proto(&key).is_some());
    assert!(app.list.image_cache.get_proto(&key).is_some());
    app.editor
        .image_cache
        .request("pending.png".into(), 128, &sender);
    drop(results);
    assert!(app.poll_editor_image_results());
    assert!(!app.editor.image_cache.is_pending());
    assert!(app.image_decode_tx.is_none());
    assert!(!app.poll_editor_image_results());
}

#[test]
fn static_views_do_not_redraw_while_idle() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let _guard = crate::config::ConfigTestGuard::lock();
    for mode in [
        ViewMode::List,
        ViewMode::Edit,
        ViewMode::Canvas,
        ViewMode::Draw,
        ViewMode::Outline,
        ViewMode::Help,
        ViewMode::Calendar,
        ViewMode::Setup,
    ] {
        let (_root, mut app) = fixture(0);
        enter(&mut app, mode);
        if mode == ViewMode::Calendar {
            // Initial event and note-link loads can complete on separate frames.
            app.poll_calendar();
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.calendar.render_pending()
                || app
                    .calendar_view
                    .as_ref()
                    .is_some_and(crate::calendar::view::View::render_pending)
            {
                assert!(Instant::now() < deadline, "calendar rendering timed out");
                std::thread::sleep(Duration::from_millis(1));
                app.poll_calendar();
            }
            assert!(!app.poll_calendar(), "loaded calendar must remain idle");
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let driver = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(650));
            let keys = if mode == ViewMode::Edit {
                vec![KeyCode::Esc, KeyCode::Char('q')]
            } else if mode == ViewMode::Setup {
                vec![KeyCode::Esc, KeyCode::Enter, KeyCode::Char('q')]
            } else {
                vec![KeyCode::Char('q'), KeyCode::Char('q')]
            };
            for code in keys {
                sender
                    .send(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
                    .unwrap();
            }
        });
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        crate::run_app(
            &mut terminal,
            &mut app,
            &mut crate::event_source::EventSource::channel(receiver),
        )
        .unwrap();
        driver.join().unwrap();
        let frames = terminal.get_frame().count();
        assert!(frames <= 3, "{mode:?} drew {frames} idle frames");
    }
}

#[test]
#[ignore = "1k/10k/50k release view, input, and switching benchmark"]
fn mixed_vault_view_performance() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let _guard = crate::config::ConfigTestGuard::lock();
    for notes in [1_000, 10_000, 50_000] {
        let setup = Instant::now();
        let (_root, mut app) = fixture(notes);
        println!("vault={notes} fixture_and_catalog={:?}", setup.elapsed());
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        for mode in [
            ViewMode::List,
            ViewMode::Edit,
            ViewMode::Graph,
            ViewMode::Canvas,
            ViewMode::Draw,
            ViewMode::Outline,
            ViewMode::Help,
            ViewMode::Calendar,
            ViewMode::Setup,
        ] {
            let opened = Instant::now();
            enter(&mut app, mode);
            let opening = opened.elapsed();
            let mut samples = Vec::new();
            for _ in 0..100 {
                app.poll_calendar();
                app.poll_renderers();
                app.poll_bases();
                let start = Instant::now();
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
                    .unwrap();
                samples.push(start.elapsed());
            }
            samples.sort();
            println!(
                "vault={notes} view={mode:?} open={opening:?} render_p50={:?} render_p95={:?} render_p99={:?}",
                samples[50], samples[95], samples[99]
            );
            if std::env::var_os("CLIN_PERF_ENFORCE").is_some() {
                assert!(
                    samples[95] <= Duration::from_millis(16),
                    "{mode:?} frame budget exceeded"
                );
            }
            if mode != ViewMode::Edit {
                let (_sender, receiver) = std::sync::mpsc::channel();
                let mut events = crate::event_source::EventSource::channel(receiver);
                let mut focus = EditFocus::Body;
                let mut samples = Vec::new();
                for index in 0..100 {
                    let key = if index % 2 == 0 {
                        KeyCode::Down
                    } else {
                        KeyCode::Up
                    };
                    let start = Instant::now();
                    crate::dispatch_event(
                        &mut events,
                        &mut app,
                        Event::Key(KeyEvent::new(key, KeyModifiers::NONE)),
                        &mut focus,
                        &mut terminal,
                    )
                    .unwrap();
                    terminal
                        .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
                        .unwrap();
                    samples.push(start.elapsed());
                    assert_eq!(app.mode, mode);
                }
                samples.sort();
                println!(
                    "vault={notes} view={mode:?} input_and_render_p95={:?}",
                    samples[95]
                );
                if std::env::var_os("CLIN_PERF_ENFORCE").is_some() {
                    assert!(
                        samples[95] <= Duration::from_millis(16),
                        "{mode:?} input/frame budget exceeded"
                    );
                }
            }
        }
        enter(&mut app, ViewMode::List);
        for layout in [
            NotesLayout::Grid,
            NotesLayout::Tree,
            NotesLayout::List,
            NotesLayout::Multi,
        ] {
            app.list.notes_layout = layout.clone();
            app.list.grid_folder = if matches!(layout, NotesLayout::Grid | NotesLayout::List) {
                "bulk".into()
            } else {
                String::new()
            };
            app.list.folder_expanded.insert(String::new());
            app.list.folder_expanded.insert("bulk".into());
            app.refresh_visual_list();
            app.list.visual_index = app.list.visual_list.iter().position(|item| matches!(item, crate::list_view::VisualItem::Folder {path, ..} if path == "bulk")).unwrap_or(0);
            app.refresh_multi_preview();
            let mut samples = Vec::new();
            for _ in 0..100 {
                let start = Instant::now();
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
                    .unwrap();
                samples.push(start.elapsed());
            }
            samples.sort();
            println!(
                "vault={notes} layout={layout:?} render_p95={:?}",
                samples[95]
            );
            if std::env::var_os("CLIN_PERF_ENFORCE").is_some() {
                assert!(samples[95] <= Duration::from_millis(16));
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "10-minute release resource soak; run alone with --ignored --nocapture"]
fn repeated_views_resource_soak() {
    fn resources() -> (usize, usize, usize) {
        let status = fs::read_to_string("/proc/self/status").unwrap();
        let rss = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        (
            rss,
            fs::read_dir("/proc/self/task").unwrap().count(),
            fs::read_dir("/proc/self/fd").unwrap().count(),
        )
    }
    let _guard = crate::config::ConfigTestGuard::lock();
    let (_root, mut app) = fixture(1_000);
    image::DynamicImage::new_rgb8(1024, 768)
        .save(app.storage.notes_dir.join("soak.png"))
        .unwrap();
    let selected = app.storage.notes_dir.join("selected.md");
    fs::write(
        &selected,
        format!(
            "![soak](soak.png)\n{}",
            fs::read_to_string(&selected).unwrap()
        ),
    )
    .unwrap();
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    let session = crate::session::start_session(&mut app);
    app.editor.editor_preview_enabled = true;
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let seconds = std::env::var("CLIN_PERF_SOAK_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(600);
    let start = Instant::now();
    let mut baseline = None;
    let mut peak = (0, 0, 0);
    let mut cycles = 0;
    let mut next_report = Duration::ZERO;
    while start.elapsed() < Duration::from_secs(seconds) || cycles < 100 {
        for mode in [
            ViewMode::List,
            ViewMode::Edit,
            ViewMode::Graph,
            ViewMode::Canvas,
            ViewMode::Draw,
            ViewMode::Outline,
            ViewMode::Help,
            ViewMode::Calendar,
            ViewMode::Setup,
        ] {
            enter(&mut app, mode);
            for _ in 0..3 {
                app.poll_calendar();
                app.poll_renderers();
                app.poll_editor_image_results();
                app.poll_bases();
                crate::process_watcher_events(&mut app);
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
                    .unwrap();
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        cycles += 1;
        if cycles == 20 {
            baseline = Some(resources());
        }
        let sample = resources();
        peak = (
            peak.0.max(sample.0),
            peak.1.max(sample.1),
            peak.2.max(sample.2),
        );
        if start.elapsed() >= next_report {
            println!(
                "soak elapsed={:?} cycles={cycles} rss_kib={} threads={} fds={}",
                start.elapsed(),
                sample.0,
                sample.1,
                sample.2
            );
            next_report += Duration::from_secs(30);
        }
    }
    enter(&mut app, ViewMode::List);
    app.suspend_inactive_work();
    std::thread::sleep(Duration::from_millis(250));
    let final_usage = resources();
    let baseline = baseline.unwrap();
    println!(
        "soak cycles={cycles} baseline={baseline:?} final={final_usage:?} peak={peak:?} (RSS KiB, threads, fds)"
    );
    assert!(
        final_usage.0 <= baseline.0 + 128 * 1024,
        "retained memory grew beyond 128 MiB"
    );
    assert!(
        final_usage.1 <= baseline.1 + 4,
        "threads accumulated across view switches"
    );
    assert!(
        final_usage.2 <= baseline.2 + 4,
        "file descriptors accumulated across view switches"
    );
    crate::session::finish_session(&mut app, session).unwrap();
}
