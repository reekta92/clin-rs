use super::FOLDER_TEMPLATE_CONFIG;
use super::variables_tests::{app, config};
use crate::app::{App, ViewMode, WatchedFsEvent};
use crate::config::{ConfigTestGuard, FeatureState, IconMode, NotesLayout};
use crate::editor_document::EditorDocument;
use crate::list_view::{ListMode, VisualItem};
use ratatui::{Terminal, backend::TestBackend};
use std::fs;
use std::time::Instant;

fn config_index(app: &App, folder: &str) -> usize {
    app.list
        .visual_list
        .iter()
        .position(|item| {
            matches!(item,
        VisualItem::TemplateConfig { folder: f, .. } if f == folder)
        })
        .unwrap()
}

fn draw(app: &mut App) -> Terminal<TestBackend> {
    app.status = "".into();
    app.list.preview_enabled = false;
    app.list.calendar_enabled = false;
    let mut terminal = Terminal::new(TestBackend::new(120, 35)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw_list_view(frame, app))
        .unwrap();
    terminal
}

fn row(buffer: &ratatui::buffer::Buffer, y: u16, start: u16, end: u16) -> String {
    (start..end)
        .map(|x| buffer.cell((x, y)).unwrap().symbol())
        .collect()
}

#[test]
fn config_items_are_visible_in_both_layouts_and_selection_modes_but_are_not_notes() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    config(dir.path(), "book", "[variables]\nbook = 'Rust'");
    fs::create_dir_all(dir.path().join("book/chapter")).unwrap();
    let mut app = app(dir.path());
    app.catalog_folders = vec!["book".into(), "book/chapter".into()];
    app.list.show_hidden_files = false;
    app.list.show_all_files = false;
    app.list
        .folder_expanded
        .extend(["".into(), "book".into(), "book/chapter".into()]);
    for layout in [NotesLayout::Grid, NotesLayout::Tree] {
        app.list.notes_layout = layout;
        app.list.grid_folder = "book".into();
        for mode in [ListMode::Normal, ListMode::Select] {
            app.list.list_mode = mode;
            app.refresh_visual_list();
            assert_eq!(app.notes.len(), 0);
            assert_eq!(
                app.list
                    .visual_list
                    .iter()
                    .filter(|item| matches!(item, VisualItem::TemplateConfig { .. }))
                    .count(),
                1
            );
            app.list.visual_index = config_index(&app, "book");
            assert!(!app.is_selectable_index(app.list.visual_index));
            assert_eq!(app.get_current_folder_context(), "book");
            app.toggle_pin();
            assert_eq!(app.notes.len(), 0);
            assert!(app.list.selected_indices.is_empty());
        }
    }
    app.config.features.templates = FeatureState::Disabled;
    app.refresh_visual_list();
    assert!(
        !app.list
            .visual_list
            .iter()
            .any(|item| matches!(item, VisualItem::TemplateConfig { .. }))
    );
    assert!(app.list.template_config_folders.is_empty());
}

#[test]
fn folder_badges_are_local_only_and_virtual_views_do_not_show_config_items() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    config(dir.path(), "book", "[variables]\nbook = 'Rust'");
    fs::create_dir_all(dir.path().join("book/chapter")).unwrap();
    let mut app = app(dir.path());
    app.catalog_folders = vec!["book".into(), "book/chapter".into()];
    app.list.grid_folder.clear();
    app.refresh_visual_list();
    assert!(app.list.template_config_folders.contains("book"));
    assert!(!app.list.template_config_folders.contains(""));
    app.list.grid_folder = "book".into();
    app.refresh_visual_list();
    assert!(app.list.template_config_folders.contains("book"));
    assert!(!app.list.template_config_folders.contains("book/chapter"));
    assert!(
        app.storage
            .folder_template_variables("book/chapter")
            .unwrap()
            .contains_key("book")
    );
    app.list.grid_folder = "book/chapter".into();
    app.refresh_visual_list();
    assert!(
        !app.list
            .visual_list
            .iter()
            .any(|item| matches!(item, VisualItem::TemplateConfig { .. }))
    );
    for path in [
        crate::app::VIRTUAL_PINNED_PATH,
        crate::app::VIRTUAL_SMART_PATH,
        crate::app::VIRTUAL_SUBNOTES_PATH,
        "@today",
        "@custom:Books",
    ] {
        app.list.grid_folder = path.into();
        app.refresh_visual_list();
        assert!(
            !app.list
                .visual_list
                .iter()
                .any(|item| matches!(item, VisualItem::TemplateConfig { .. }))
        );
        assert!(!app.list.template_config_folders.contains(path));
    }
}

#[test]
fn folder_template_glyph_renders_in_both_layouts_without_overwriting_pin() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    config(dir.path(), "book", "[variables]\nchapter = '04'");
    fs::create_dir_all(dir.path().join("plain")).unwrap();
    let mut app = app(dir.path());
    app.catalog_folders = vec!["book".into(), "plain".into()];
    app.config.ui.icon_mode = IconMode::Unicode;
    app.list.pinned_folders.insert("book".into());
    app.pinned_on_top = false;
    app.list.folder_expanded.insert("".into());
    for layout in [NotesLayout::Grid, NotesLayout::Tree] {
        app.list.notes_layout = layout.clone();
        app.list.grid_folder.clear();
        app.refresh_visual_list();
        app.list.visual_index = app
            .list
            .visual_list
            .iter()
            .position(|item| matches!(item, VisualItem::CreateNew { .. }))
            .unwrap();
        let terminal = draw(&mut app);
        let buffer = terminal.backend().buffer();
        if layout == NotesLayout::Grid {
            for tile in &app.list.grid_tiles {
                if let VisualItem::Folder { path, .. } = &app.list.visual_list[tile.visual_index] {
                    let badges = row(
                        buffer,
                        tile.rect.y + 1,
                        tile.rect.x + 1,
                        tile.rect.right() - 1,
                    );
                    if path == "book" {
                        assert!(badges.contains('⚙'), "{badges}");
                        assert!(badges.contains('📌'), "{badges}");
                        assert!(
                            badges.contains('📁'),
                            "folder icon moved off badge row: {badges}"
                        );
                        assert!(
                            !row(
                                buffer,
                                tile.rect.y + 2,
                                tile.rect.x + 1,
                                tile.rect.right() - 1
                            )
                            .contains('📁')
                        );
                    } else {
                        assert!(!badges.contains('⚙'), "{path}: {badges}");
                    }
                }
            }
        } else {
            let book = (1..34)
                .map(|y| row(buffer, y, 0, 120))
                .find(|line| line.contains("book"))
                .unwrap();
            assert!(book.contains('⚙'), "{book}");
            assert!(book.contains('📌'), "{book}");
            let plain = (1..34)
                .map(|y| row(buffer, y, 0, 120))
                .find(|line| line.contains("plain"))
                .unwrap();
            assert!(!plain.contains('⚙'), "{plain}");
        }
        assert!(!row(buffer, 0, 0, 120).contains("Tpl:"));
    }
}

#[test]
fn config_rows_open_validated_editor_and_preview_in_both_layouts() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    config(dir.path(), "book", "[variables]\nchapter = '04'");
    let mut app = app(dir.path());
    app.catalog_folders = vec!["book".into()];
    app.list.folder_expanded.extend(["".into(), "book".into()]);
    app.list.preview_enabled = true;
    for layout in [NotesLayout::Grid, NotesLayout::Tree] {
        app.list.notes_layout = layout;
        app.list.grid_folder = "book".into();
        app.refresh_visual_list();
        app.list.visual_index = config_index(&app, "book");
        app.update_preview();
        assert!(matches!(
            app.list.preview_content,
            Some(crate::list_view::PreviewContent::Markdown(_))
        ));
        app.open_selected();
        assert_eq!(app.mode, ViewMode::Edit);
        assert!(app.editor.editing_id.is_none());
        assert!(
            app.editor
                .body
                .lines()
                .join("\n")
                .contains("chapter = '04'")
        );
        let path = app.storage.folder_template_config_path("book").unwrap();
        let before = fs::read_to_string(&path).unwrap();
        app.editor.body = EditorDocument::from_text("[variables]\nchapter = 4");
        assert!(app.autosave().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        app.editor.body = EditorDocument::from_text("[variables]\nchapter = '05'");
        app.autosave().unwrap();
        app.back_to_list(None, None);
        assert_eq!(app.mode, ViewMode::List);
        assert!(app.popups.active.is_none());
        assert_eq!(app.notes.len(), 0);
        config(dir.path(), "book", "[variables]\nchapter = '04'");
    }
}

#[test]
fn config_filename_is_muted_in_tree_and_grid_rendering() {
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    config(dir.path(), "book", "[variables]\nchapter = '04'");
    let mut app = app(dir.path());
    app.catalog_folders = vec!["book".into()];
    app.list.folder_expanded.extend(["".into(), "book".into()]);
    app.config.ui.icon_mode = IconMode::None;
    for layout in [NotesLayout::Grid, NotesLayout::Tree] {
        app.list.notes_layout = layout.clone();
        app.list.grid_folder = "book".into();
        app.refresh_visual_list();
        app.list.visual_index = app
            .list
            .visual_list
            .iter()
            .position(|item| matches!(item, VisualItem::CreateNew { .. }))
            .unwrap();
        let terminal = draw(&mut app);
        let buffer = terminal.backend().buffer();
        let label = if layout == NotesLayout::Grid {
            ".clin-t"
        } else {
            FOLDER_TEMPLATE_CONFIG
        };
        let mut found = false;
        for y in 0..35 {
            for x in 0..(120 - label.len() as u16) {
                let text = row(buffer, y, x, x + label.len() as u16);
                if text == label {
                    found = true;
                    assert_eq!(buffer.cell((x, y)).unwrap().fg, app.app_theme.muted);
                }
            }
        }
        assert!(found, "config label missing in {layout:?}");
    }
}

#[test]
fn watcher_config_changes_refresh_badges_and_items_without_moving_selected_note() {
    use notify::event::{CreateKind, RemoveKind};
    let _guard = ConfigTestGuard::lock();
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("book")).unwrap();
    fs::write(dir.path().join("book/note.md"), "Note").unwrap();
    let mut app = app(dir.path());
    app.list.notes_layout = NotesLayout::Tree;
    app.catalog_folders = vec!["book".into()];
    app.notes = vec![app.storage.load_note_summary("book/note.md").unwrap()];
    app.list.folder_expanded.extend(["".into(), "book".into()]);
    app.refresh_visual_list();
    app.list.visual_index = app
        .list
        .visual_list
        .iter()
        .position(|item| matches!(item, VisualItem::Note { .. }))
        .unwrap();
    let selected = app.list.visual_list[app.list.visual_index].clone();
    app.list.list_mode = ListMode::Select;
    app.list.selected_indices.insert(app.list.visual_index);
    let (tx, rx) = std::sync::mpsc::channel();
    app.fs_event_rx = Some(rx);
    let path = dir.path().join("book").join(FOLDER_TEMPLATE_CONFIG);
    config(dir.path(), "book", "[variables]\nchapter = '04'");
    tx.send(WatchedFsEvent {
        observed_at: Instant::now(),
        event: notify::Event::new(notify::EventKind::Create(CreateKind::File))
            .add_path(path.clone()),
    })
    .unwrap();
    assert!(crate::process_watcher_events(&mut app));
    assert_eq!(app.list.visual_list[app.list.visual_index], selected);
    config_index(&app, "book");
    assert_eq!(app.list.selected_indices.len(), 1);
    assert!(app.list.selected_indices.contains(&app.list.visual_index));
    assert!(!app.is_selectable_index(config_index(&app, "book")));
    assert!(app.list.template_config_folders.contains("book"));
    fs::remove_file(&path).unwrap();
    tx.send(WatchedFsEvent {
        observed_at: Instant::now(),
        event: notify::Event::new(notify::EventKind::Remove(RemoveKind::File)).add_path(path),
    })
    .unwrap();
    assert!(crate::process_watcher_events(&mut app));
    assert_eq!(app.list.visual_list[app.list.visual_index], selected);
    assert!(
        !app.list
            .visual_list
            .iter()
            .any(|item| matches!(item, VisualItem::TemplateConfig { .. }))
    );
    assert!(!app.list.template_config_folders.contains("book"));
    assert_eq!(app.list.selected_indices.len(), 1);
    assert!(app.list.selected_indices.contains(&app.list.visual_index));
    assert_eq!(app.notes.len(), 1);
}
