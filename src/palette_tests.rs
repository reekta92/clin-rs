use crate::{
    actions::{ActionContext, execute_action, execute_action_in_context, get_all_action_infos},
    app::{App, EditFocus, ViewMode},
    config::{ConfigTestGuard, FeatureState, KeybindPreset},
    event_source::EventSource,
    keybinds::{KeyCombo, ListAction},
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{fs, sync::mpsc};

const MODES: [ViewMode; 9] = [
    ViewMode::List,
    ViewMode::Edit,
    ViewMode::Help,
    ViewMode::Graph,
    ViewMode::Draw,
    ViewMode::Canvas,
    ViewMode::Bases,
    ViewMode::Outline,
    ViewMode::Setup,
];
const AREA: Rect = Rect::new(0, 0, 100, 40);

struct Fixture {
    app: App,
    root: tempfile::TempDir,
    _guard: ConfigTestGuard,
}
impl Fixture {
    fn new() -> Self {
        crate::text_edit::TEST_CLIPBOARD
            .with(|clipboard| *clipboard.borrow_mut() = Some(String::new()));
        let guard = ConfigTestGuard::lock();
        let root = tempfile::tempdir().unwrap();
        crate::config::set_config_path_override(root.path().join("config.toml"));
        let storage = crate::storage::Storage {
            data_dir: root.path().join("vault"),
            notes_dir: root.path().join("vault"),
            config_dir: root.path().join("vault/.clin"),
            templates_dir: root.path().join("templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        for path in [
            &storage.notes_dir,
            &storage.config_dir,
            &storage.templates_dir,
        ] {
            fs::create_dir_all(path).unwrap();
        }
        storage
            .create_document("selected.md", "# Selected\nHidden selection\n")
            .unwrap();
        storage
            .create_document("editing.md", "# Editing\nLive editor\n")
            .unwrap();
        storage
            .create_document("board.base", "views:\n  - type: table\n    name: Public\n")
            .unwrap();
        fs::write(
            storage.notes_dir.join("map.canvas"),
            "{\"nodes\":[],\"edges\":[]}",
        )
        .unwrap();
        fs::write(
            storage.notes_dir.join("art.draw"),
            serde_json::to_string(&crate::draw::state::DrawData::default()).unwrap(),
        )
        .unwrap();
        let mut config = crate::config::ClinConfig::default();
        config.core.storage_path = Some(storage.data_dir.clone());
        config.core.confirm_on_quit = false;
        config.features.graph_view = FeatureState::Enabled;
        config.features.draw_view = FeatureState::Enabled;
        config.features.canvas_view = FeatureState::Enabled;
        config.features.bases_view = FeatureState::Enabled;
        config.features.outline_view = FeatureState::Enabled;
        config.features.help_view = FeatureState::Enabled;
        config.features.import = FeatureState::Enabled;
        config.save().unwrap();
        let mut app = App::new(storage).unwrap();
        app.config = config;
        app.keybinds = KeybindPreset::Default.base_keybinds();
        app.notes = ["selected.md", "editing.md"]
            .into_iter()
            .map(|id| app.storage.load_note_summary(id).unwrap())
            .collect();
        app.refresh_visual_list();
        app.list.visual_index = app.list.visual_list.iter().position(|item| matches!(item,
            crate::list_view::VisualItem::Note { summary_idx, .. } if app.notes[*summary_idx].id == "selected.md")).unwrap();
        Self {
            app,
            root,
            _guard: guard,
        }
    }
    fn enter(&mut self, mode: ViewMode) {
        let app = &mut self.app;
        match mode {
            ViewMode::Edit => app.load_and_open_note("editing.md", None),
            ViewMode::Help => app.open_help_page(),
            ViewMode::Graph => app.open_graph_view(),
            ViewMode::Draw => {
                app.draw_state = Some(crate::draw::app::DrawAppState::new(
                    app.storage.clone(),
                    Some("art.draw".into()),
                    app.app_theme.clone(),
                    app.keybinds.clone(),
                    crate::keybinds::KeyMatcher::default(),
                ));
            }
            ViewMode::Canvas => {
                app.canvas_state = Some(
                    crate::pinstar_adapter::PinstarPlugin::new(
                        &app.storage.note_path("map.canvas"),
                        &app.config,
                        app.keybinds.clone(),
                        crate::keybinds::KeyMatcher::default(),
                        None,
                        &app.storage.data_dir,
                    )
                    .unwrap(),
                );
            }
            ViewMode::Bases => {
                app.bases_state = Some(crate::bases::state::BasesState::new(
                    "board.base".into(),
                    None,
                ));
            }
            ViewMode::Outline => app.open_outline_view(),
            ViewMode::Setup => app.open_setup_view(),
            ViewMode::List => {}
        }
        app.mode = mode;
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        crate::text_edit::TEST_CLIPBOARD.with(|clipboard| *clipboard.borrow_mut() = None);
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
pub(crate) fn drive(app: &mut App, event: Event) {
    drive_focus(app, event, &mut EditFocus::Body);
}
fn drive_focus(app: &mut App, event: Event, focus: &mut EditFocus) {
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    if app.mode == ViewMode::Edit {
        crate::editor_session::dispatch_editor_event(
            &mut terminal,
            app,
            event,
            focus,
            &mut crate::text_edit::MouseTextSelection::default(),
        )
        .unwrap();
    } else {
        let (_sender, receiver) = mpsc::channel();
        crate::dispatch_event(
            &mut EventSource::channel(receiver),
            app,
            event,
            focus,
            &mut terminal,
        )
        .unwrap();
    }
}
fn activate(app: &mut App, id: &str) {
    if app.command_palette.is_none() {
        drive(app, key(KeyCode::F(6)));
    }
    let palette = app
        .command_palette
        .as_mut()
        .expect("palette opened through dispatcher");
    let index = palette
        .items
        .iter()
        .position(|item| item.id == id)
        .unwrap_or_else(|| panic!("missing {id}"));
    palette.state.select(Some(index));
    drive(app, key(KeyCode::Enter));
}
fn visible(app: &App, id: &str) -> bool {
    app.command_palette
        .as_ref()
        .unwrap()
        .items
        .iter()
        .any(|item| item.id == id)
}

#[test]
fn palette_f6_opens_all_ten_views_under_every_shipped_preset() {
    for mode in MODES {
        let mut f = Fixture::new();
        f.enter(mode);
        for preset in [
            KeybindPreset::Default,
            KeybindPreset::Helix,
            KeybindPreset::Vim,
            KeybindPreset::Emacs,
        ] {
            f.app.keybinds = preset.base_keybinds();
            f.app.sync_retained_view_settings();
            drive(&mut f.app, key(KeyCode::F(6)));
            assert!(f.app.command_palette.is_some(), "{mode:?}/{preset}");
            assert!(visible(&f.app, "switch_theme"));
            if mode != ViewMode::List {
                assert!(!visible(&f.app, "note.encrypt"));
                assert!(!visible(&f.app, "create_canvas"));
            }
            if mode == ViewMode::Setup {
                let mut ids: Vec<_> = f
                    .app
                    .command_palette
                    .as_ref()
                    .unwrap()
                    .items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect();
                ids.sort_unstable();
                assert_eq!(
                    ids,
                    [
                        "keybind.preset",
                        "setup.cancel",
                        "setup.finish",
                        "switch_theme"
                    ]
                );
            }
            drive(&mut f.app, key(KeyCode::Esc));
            assert_eq!(f.app.mode, mode);
        }
    }
}

#[test]
fn palette_custom_f6_strokes_keep_existing_sequence_resolution() {
    let mut f = Fixture::new();
    for sequence in ["F6", "F6 x", "g F6"] {
        f.app.keybinds = KeybindPreset::Default.base_keybinds();
        f.app.keybinds.list.insert(
            ListAction::TogglePreview,
            vec![KeyCombo::parse(sequence).unwrap()],
        );
        f.app.seq_matcher.clear();
        let before = f.app.list.preview_enabled;
        for stroke in KeyCombo::parse(sequence).unwrap().keys {
            drive(
                &mut f.app,
                Event::Key(KeyEvent::new(stroke.code, stroke.modifiers)),
            );
            assert!(f.app.command_palette.is_none(), "{sequence}");
        }
        assert_ne!(f.app.list.preview_enabled, before, "{sequence}");
    }
    f.app.keybinds = KeybindPreset::Default.base_keybinds();
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        drive(
            &mut f.app,
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::F(6),
                KeyModifiers::NONE,
                kind,
            )),
        );
        assert!(f.app.command_palette.is_none());
    }
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::F(6), KeyModifiers::CONTROL)),
    );
    assert!(f.app.command_palette.is_none());
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
    );
    assert!(f.app.command_palette.is_some());
    drive(&mut f.app, key(KeyCode::Esc));
    f.enter(ViewMode::Edit);
    let before = f.app.editor.editor_preview_enabled;
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
    );
    assert_eq!(f.app.editor.editor_preview_enabled, !before);
    assert!(f.app.command_palette.is_none());
}

#[test]
fn palette_edit_focus_and_document_identity_control_keyboard_and_mouse_insertion() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Edit);
    let selected = fs::read(f.app.storage.note_path("selected.md")).unwrap();
    for mut focus in [EditFocus::Title, EditFocus::Sidebar] {
        drive_focus(&mut f.app, key(KeyCode::F(6)), &mut focus);
        assert!(!visible(&f.app, "editor.insert_date"));
        assert!(!visible(&f.app, "paste_image"));
        assert!(visible(&f.app, "settings.line_numbers"));
        drive_focus(&mut f.app, key(KeyCode::Esc), &mut focus);
    }
    assert!(execute_action("editor.insert_date", &mut f.app, None).is_err());
    let before = f.app.editor.body.text();
    activate(&mut f.app, "editor.insert_date");
    assert_ne!(f.app.editor.body.text(), before);
    assert_eq!(
        fs::read(f.app.storage.note_path("selected.md")).unwrap(),
        selected
    );
    drive(&mut f.app, key(KeyCode::F(6)));
    let mut palette = f.app.command_palette.take().unwrap();
    palette.input.insert_str("Insert Date");
    palette.refresh_items(&f.app);
    f.app.command_palette = Some(palette);
    let popup = crate::ui::centered_rect(crate::ui::PopupSize::Large, AREA);
    let before = f.app.editor.body.text();
    drive(
        &mut f.app,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: popup.x + 2,
            row: popup.y + 5,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(f.app.command_palette.is_none());
    assert_ne!(f.app.editor.body.text(), before);
    drive(&mut f.app, key(KeyCode::F(6)));
    let context = f.app.command_palette.as_ref().unwrap().context.clone();
    f.app.editor.editing_id = Some("selected.md".into());
    assert!(execute_action_in_context("editor.insert_date", &mut f.app, &context).is_err());
    f.app.storage.data_dir = f.root.path().join("different-vault");
    assert!(execute_action_in_context("switch_theme", &mut f.app, &context).is_err());
}

#[test]
fn palette_paste_refresh_input_isolation_resize_and_empty_clicks() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Edit);
    let before = f.app.editor.body.text();
    let cursor = f.app.editor.body.cursor();
    drive(&mut f.app, key(KeyCode::F(6)));
    drive(&mut f.app, Event::Paste("Switch\r\nTheme".into()));
    assert_eq!(
        f.app.command_palette.as_ref().unwrap().input.lines(),
        ["Switch Theme"]
    );
    assert_eq!(
        f.app.command_palette.as_ref().unwrap().items[0].id,
        "switch_theme"
    );
    for code in ['a', 'c', 'a', 'x'] {
        drive(
            &mut f.app,
            Event::Key(KeyEvent::new(KeyCode::Char(code), KeyModifiers::CONTROL)),
        );
    }
    assert_eq!(
        crate::text_edit::read_system_clipboard().as_deref(),
        Some("Switch Theme")
    );
    assert_eq!(f.app.command_palette.as_ref().unwrap().input.lines(), [""]);
    crate::text_edit::write_system_clipboard("Switch\r\nTheme").unwrap();
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)),
    );
    assert_eq!(
        f.app.command_palette.as_ref().unwrap().input.lines(),
        ["Switch Theme"]
    );
    for code in ['a', 'x'] {
        drive(
            &mut f.app,
            Event::Key(KeyEvent::new(KeyCode::Char(code), KeyModifiers::CONTROL)),
        );
    }
    drive(&mut f.app, Event::Paste("Switch Theme".into()));
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        drive(
            &mut f.app,
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::Enter,
                KeyModifiers::NONE,
                kind,
            )),
        );
        assert!(f.app.popups.active.is_none());
    }
    drive(&mut f.app, Event::Resize(20, 10));
    assert_eq!(
        f.app.command_palette.as_ref().unwrap().input.lines(),
        ["Switch Theme"]
    );
    let popup = crate::ui::centered_rect(crate::ui::PopupSize::Large, AREA);
    drive(
        &mut f.app,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: popup.x + 2,
            row: popup.bottom() - 3,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(f.app.command_palette.is_some());
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(f.app.popups.active.is_none());
    assert!(matches!(
        f.app.command_palette.as_ref().unwrap().workflow,
        Some(crate::palette::workflows::Workflow::Theme(_))
    ));
    drive(&mut f.app, key(KeyCode::Esc));
    drive(&mut f.app, key(KeyCode::Esc));
    drive(&mut f.app, key(KeyCode::F(6)));
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
    );
    // Ctrl+C must be consumed even without a query selection or a system clipboard.
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
    );
    assert!(!f.app.should_quit);
    drive(&mut f.app, Event::Paste("不存在🦀\nunknown".into()));
    assert!(f.app.command_palette.as_ref().unwrap().items.is_empty());
    let mut terminal = Terminal::new(TestBackend::new(8, 4)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut f.app, EditFocus::Body))
        .unwrap();
    drive(
        &mut f.app,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(f.app.command_palette.is_none());
    assert_eq!(f.app.editor.body.text(), before);
    assert_eq!(f.app.editor.body.cursor(), cursor);
}

#[test]
fn palette_modal_gates_preserve_each_owners_pending_input() {
    for mode in MODES {
        let mut f = Fixture::new();
        f.enter(mode);
        match mode {
            ViewMode::List => f.app.begin_set_word_goal(),
            ViewMode::Edit => {
                f.app.editor.go_to_line_input = Some("123".into());
            }
            ViewMode::Help => {
                f.app.help_search.popup = Some(crate::ui::quick_search::QuickSearch::new(
                    "find",
                    &f.app.app_theme,
                ));
            }
            ViewMode::Graph => {
                f.app
                    .graph_plugin
                    .as_mut()
                    .unwrap()
                    .graph_mouse_state
                    .is_panning = true;
            }
            ViewMode::Draw => {
                f.app.draw_state.as_mut().unwrap().creation_origin = Some((1.0, 2.0));
            }
            ViewMode::Canvas => {
                f.app.canvas_state.as_mut().unwrap().state.floating_editor =
                    Some(ratatui_textarea::TextArea::default());
            }
            ViewMode::Bases => {
                f.app.bases_state.as_mut().unwrap().matcher.clear();
                f.app.begin_set_word_goal();
            }
            ViewMode::Outline => {
                f.app.open_vault_switcher();
            }
            ViewMode::Setup => {
                f.app.setup_state.as_mut().unwrap().confirm_exit = true;
            }
        }
        drive(&mut f.app, key(KeyCode::F(6)));
        assert!(f.app.command_palette.is_none(), "{mode:?}");
        assert_eq!(f.app.mode, mode);
        if mode == ViewMode::Edit {
            assert_eq!(f.app.editor.go_to_line_input.as_deref(), Some("123"));
        }
        if mode == ViewMode::Setup {
            assert!(f.app.setup_state.as_ref().unwrap().confirm_exit);
        }
    }
}

#[test]
fn palette_navigation_saves_dirty_edit_public_and_template_owners() {
    for source in ["ordinary", "public", "template"] {
        let mut f = Fixture::new();
        if source == "public" {
            let snapshot = f.app.storage.load_document("board.base").unwrap();
            f.app.open_public_document(snapshot);
        } else {
            f.enter(ViewMode::Edit);
        }
        let path = if source == "template" {
            let path = f.app.storage.templates_dir.join("example.toml");
            fs::write(&path, "original").unwrap();
            f.app.editor.template_edit_path = Some(path.clone());
            f.app.editor.template_source = Some("original".into());
            f.app.editor.editing_id = None;
            path
        } else {
            f.app.storage.note_path(if source == "public" {
                "board.base"
            } else {
                "editing.md"
            })
        };
        f.app.editor.body.insert_str("changed ");
        let expected = f.app.editor.body.text();
        f.app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
        f.app.open_help_page();
        activate(&mut f.app, "notes.open");
        assert_eq!(f.app.mode, ViewMode::List, "{source}");
        assert!(fs::read_to_string(path).unwrap().contains(&expected));
    }
}

#[test]
fn palette_save_failure_keeps_editor_draft_and_suspended_return_routes() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Edit);
    f.app.editor.body.insert_str("unsaved ");
    f.app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
    let expected = f.app.editor.body.text();
    let blocker = f.root.path().join("not-a-directory");
    fs::write(&blocker, "block").unwrap();
    f.app.editor.template_edit_path = Some(blocker.join("bad.toml"));
    f.app.editor.editing_id = None;
    f.app.open_help_page();
    activate(&mut f.app, "graph.open");
    assert_eq!(f.app.mode, ViewMode::Help);
    assert_eq!(f.app.return_mode, Some(ViewMode::Edit));
    assert!(f.app.graph_plugin.is_none());
    assert_eq!(f.app.editor.body.text(), expected);
    assert_eq!(
        f.app.editor.autosave_status,
        crate::editor::AutosaveStatus::Unsaved
    );
    assert!(f.app.status.contains("failed") || f.app.status.contains("Failed"));
    f.app.close_help_page();
    f.app.editor.autosave_timer = Some(std::time::Instant::now());
    f.app.tick_autosave();
    assert_eq!(
        f.app.editor.autosave_status,
        crate::editor::AutosaveStatus::Unsaved
    );
    assert!(
        f.app
            .editor
            .autosave_timer
            .is_some_and(|timer| timer > std::time::Instant::now())
    );
    f.app.switch_vault(f.root.path().join("another-vault"));
    assert!(f.app.setup_rebootstrap.is_none());
    assert!(!f.app.should_quit);
}

#[test]
fn palette_setup_theme_and_preset_remain_pending_until_finish_or_cancel() {
    let mut f = Fixture::new();
    let path = crate::config::ClinConfig::config_path().unwrap();
    let original = fs::read(&path).unwrap();
    for finish in [false, true] {
        f.app.open_setup_view();
        activate(&mut f.app, "switch_theme");
        let themes = f.app.setup_state.as_ref().unwrap().themes.clone();
        let theme = (f.app.setup_state.as_ref().unwrap().theme + 1) % themes.len();
        let palette = f.app.command_palette.as_mut().unwrap();
        let index = palette
            .items
            .iter()
            .position(|item| item.id == format!("theme:{theme}"))
            .unwrap();
        palette.state.select(Some(index));
        drive(&mut f.app, key(KeyCode::Enter));
        drive(&mut f.app, key(KeyCode::Esc));
        activate(&mut f.app, "keybind.preset");
        select_step(&mut f.app, "2");
        drive(&mut f.app, key(KeyCode::Esc));
        drive(&mut f.app, key(KeyCode::Esc));
        f.app.setup_state.as_mut().unwrap().background_solid = true;
        f.app.apply_setup_live();
        assert_eq!(fs::read(&path).unwrap(), original);
        if finish {
            activate(&mut f.app, "setup.finish");
            let config = crate::config::ClinConfig::load().0.unwrap();
            assert_eq!(config.ui.theme, themes[theme]);
            assert_eq!(config.core.keybind_preset, KeybindPreset::Vim);
        } else {
            activate(&mut f.app, "setup.cancel");
            assert!(f.app.setup_state.as_ref().unwrap().confirm_exit);
            drive(&mut f.app, key(KeyCode::Char('q')));
            assert_eq!(fs::read(&path).unwrap(), original);
        }
        assert_eq!(f.app.mode, ViewMode::List);
    }
}

#[test]
fn palette_targets_reject_deleted_changed_and_structured_documents() {
    let mut f = Fixture::new();
    assert!(
        get_all_action_infos(&f.app)
            .iter()
            .any(|action| action.id == "info.show")
    );
    execute_action("info.show", &mut f.app, None).unwrap();
    assert!(matches!(
        f.app.popups.active,
        Some(crate::popups::ActivePopup::Info(_))
    ));
    f.app.popups.active = None;
    let context = ActionContext::capture(&f.app, None, None);
    fs::write(
        f.app.storage.note_path("selected.md"),
        "external replacement",
    )
    .unwrap();
    assert!(execute_action_in_context("note.rasterize", &mut f.app, &context).is_err());
    fs::remove_file(f.app.storage.note_path("selected.md")).unwrap();
    assert!(execute_action("note.encrypt", &mut f.app, Some("selected.md")).is_err());
    assert!(execute_action("unknown", &mut f.app, None).is_err());
    f.enter(ViewMode::Edit);
    let context = ActionContext::capture(&f.app, Some(EditFocus::Body), None);
    f.app.editor.editing_id = Some("renamed.md".into());
    assert!(execute_action_in_context("editor.insert_date", &mut f.app, &context).is_err());
    assert!(execute_action_in_context("switch_theme", &mut f.app, &context).is_ok());
    f.app.popups.active = None;
    let snapshot = f.app.storage.load_document("board.base").unwrap();
    f.app.open_public_document(snapshot);
    drive(&mut f.app, key(KeyCode::F(6)));
    assert!(!visible(&f.app, "editor.insert_date"));
    assert!(!visible(&f.app, "settings.text_align"));
    assert!(!visible(&f.app, "paste_image"));
    assert!(visible(&f.app, "settings.wrap"));
}

#[test]
fn palette_canvas_image_history_and_failed_save_keep_live_data() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Canvas);
    let selected = fs::read(f.app.storage.note_path("selected.md")).unwrap();
    crate::actions::ocr::insert_image_reference(&mut f.app, "attachments/one.png").unwrap();
    let state = &f.app.canvas_state.as_ref().unwrap().state;
    assert_eq!(state.data.nodes.len(), 1);
    assert_eq!(state.undo_stack.len(), 1);
    assert!(
        state
            .raw_editor
            .lines()
            .join("\n")
            .contains("attachments/one.png")
    );
    // The file exists, but its atomic-write temporary name exceeds the filesystem component limit.
    let blocked = f
        .app
        .storage
        .notes_dir
        .join(format!("{}.canvas", "x".repeat(220)));
    fs::write(&blocked, "{\"nodes\":[],\"edges\":[]}").unwrap();
    f.app.canvas_state.as_mut().unwrap().state.path = blocked;
    assert!(
        crate::actions::ocr::insert_image_reference(&mut f.app, "attachments/two.png").is_err()
    );
    assert_eq!(
        f.app.canvas_state.as_ref().unwrap().state.data.nodes.len(),
        2
    );
    activate(&mut f.app, "notes.open");
    assert_eq!(f.app.mode, ViewMode::Canvas);
    assert_eq!(
        f.app.canvas_state.as_ref().unwrap().state.data.nodes.len(),
        2
    );
    assert_eq!(
        fs::read(f.app.storage.note_path("selected.md")).unwrap(),
        selected
    );
    f.app.canvas_state = None;
    assert!(crate::actions::ocr::insert_image_reference(&mut f.app, "ignored.png").is_err());
}

#[test]
fn palette_draw_save_failure_and_fileless_changes_block_departure() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Draw);
    f.app.draw_state.as_mut().unwrap().data.background = Some("#ffffff".into());
    activate(&mut f.app, "notes.open");
    assert_eq!(f.app.mode, ViewMode::List);
    assert!(
        fs::read_to_string(f.app.storage.note_path("art.draw"))
            .unwrap()
            .contains("#ffffff")
    );
    f.enter(ViewMode::Draw);
    f.app.draw_state.as_mut().unwrap().current_file = None;
    f.app.draw_state.as_mut().unwrap().data.background = Some("#000000".into());
    activate(&mut f.app, "graph.open");
    assert_eq!(f.app.mode, ViewMode::Draw);
    assert!(f.app.status.contains("no save target"));
    let blocker = f.root.path().join("blocked");
    fs::write(&blocker, "block").unwrap();
    f.app.draw_state.as_mut().unwrap().storage.notes_dir = blocker;
    f.app.draw_state.as_mut().unwrap().current_file = Some("art.draw".into());
    activate(&mut f.app, "notes.open");
    assert_eq!(f.app.mode, ViewMode::Draw);
    assert!(f.app.draw_state.is_some());
    assert!(f.app.graph_plugin.is_none());
}

#[test]
fn palette_live_settings_reach_retained_owners_without_resetting_graph_or_history() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Graph);
    let graph = f
        .app
        .graph_plugin
        .as_ref()
        .unwrap()
        .graph_state
        .as_ref()
        .unwrap()
        .clone();
    activate(&mut f.app, "settings.graph_show_legend");
    assert_eq!(
        f.app.graph_plugin.as_ref().unwrap().show_legend,
        f.app.config.graf.visual.show_legend
    );
    assert!(std::sync::Arc::ptr_eq(
        &graph,
        f.app
            .graph_plugin
            .as_ref()
            .unwrap()
            .graph_state
            .as_ref()
            .unwrap()
    ));
    f.app.open_help_page();
    f.app.begin_keybind_preset_selection();
    if let Some(crate::popups::ActivePopup::KeybindPreset(popup)) = &mut f.app.popups.active {
        popup.selected = 3;
    }
    f.app.select_keybind_preset();
    f.app.close_keybind_preset_popup();
    assert_eq!(
        f.app.graph_plugin.as_ref().unwrap().keybinds.graph,
        f.app.keybinds.graph
    );
    f.app.config.ui.theme = "dracula".into();
    f.app.refresh_theme_from_config();
    assert_eq!(
        f.app.graph_plugin.as_ref().unwrap().app_theme.fg,
        f.app.app_theme.fg
    );
    f.app.close_help_page();
    assert_eq!(f.app.mode, ViewMode::Graph);
    assert_eq!(f.app.return_mode, Some(ViewMode::List));
    assert!(std::sync::Arc::ptr_eq(
        &graph,
        f.app
            .graph_plugin
            .as_ref()
            .unwrap()
            .graph_state
            .as_ref()
            .unwrap()
    ));
    activate(&mut f.app, "notes.open");
}

#[test]
fn palette_persistence_load_and_save_failures_are_visible_errors() {
    let mut f = Fixture::new();
    let path = crate::config::ClinConfig::config_path().unwrap();
    fs::write(&path, "invalid = [").unwrap();
    assert!(execute_action("settings.scrollbars", &mut f.app, None).is_err());
    assert!(f.app.status.contains("Failed to save config"));
    let blocker = f.root.path().join("blocked");
    fs::write(&blocker, "block").unwrap();
    crate::config::set_config_path_override(blocker.join("config.toml"));
    assert!(execute_action("settings.tab_icons_only", &mut f.app, None).is_err());
    assert!(f.app.status.contains("Failed to save config"));
}

#[test]
fn palette_destination_failure_preserves_saved_source_and_return_route() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Edit);
    f.app.editor.body.insert_str("saved before preparation ");
    let expected = f.app.editor.body.text();
    f.app.open_help_page();
    f.app.palette_destination_failure = true;
    activate(&mut f.app, "graph.open");
    assert_eq!(f.app.mode, ViewMode::Help);
    assert_eq!(f.app.return_mode, Some(ViewMode::Edit));
    assert!(f.app.graph_plugin.is_none());
    assert_eq!(f.app.editor.body.text(), expected);
    assert!(
        fs::read_to_string(f.app.storage.note_path("editing.md"))
            .unwrap()
            .contains(&expected)
    );
}

#[test]
fn palette_help_to_setup_restores_original_editor_and_drafts_on_save_failure() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Edit);
    f.app.editor.body.insert_str("recoverable ");
    let expected = f.app.editor.body.text();
    f.app.open_help_page();
    activate(&mut f.app, "setup_wizard");
    assert_eq!(f.app.mode, ViewMode::Setup);
    assert_eq!(f.app.return_mode, Some(ViewMode::Edit));
    activate(&mut f.app, "setup.cancel");
    drive(&mut f.app, key(KeyCode::Char('q')));
    assert_eq!(f.app.mode, ViewMode::Edit);
    assert_eq!(f.app.editor.body.text(), expected);
    fs::write(f.app.storage.note_path("editing.md"), "External change\n").unwrap();
    f.app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
    activate(&mut f.app, "notes.open");
    assert_eq!(f.app.mode, ViewMode::Edit);
    assert_eq!(f.app.editor.body.text(), expected);
    f.app.flush_drafts().unwrap();
    assert!(
        fs::read_dir(f.app.storage.data_dir.join(".clin/document-recovery-v2"))
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("draft-"))
    );
    assert_eq!(
        f.app.editor.autosave_status,
        crate::editor::AutosaveStatus::Unsaved
    );
}

#[test]
fn palette_notes_alias_and_right_click_preserve_tree_and_grid_targets() {
    for layout in [
        crate::config::NotesLayout::Tree,
        crate::config::NotesLayout::Grid,
    ] {
        let mut f = Fixture::new();
        f.app.list.notes_layout = layout.clone();
        f.app.list.preview_enabled = false;
        f.app.refresh_visual_list();
        let target = f.app.list.visual_list.iter().position(|item| matches!(item,
            crate::list_view::VisualItem::Note { summary_idx, .. } if f.app.notes[*summary_idx].id == "editing.md")).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut f.app, EditFocus::Body))
            .unwrap();
        let (column, row) = if layout == crate::config::NotesLayout::Grid {
            let tile = f
                .app
                .list
                .grid_tiles
                .iter()
                .find(|tile| tile.visual_index == target)
                .unwrap();
            (tile.rect.x + 1, tile.rect.y + 1)
        } else {
            (3, 2 + target as u16)
        };
        drive(
            &mut f.app,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            }),
        );
        assert_eq!(
            f.app
                .command_palette
                .as_ref()
                .unwrap()
                .context_note_id
                .as_deref(),
            Some("editing.md")
        );
    }
    let mut f = Fixture::new();
    f.enter(ViewMode::Bases);
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
    );
    assert!(
        f.app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.id == "bases.refresh")
    );
    assert!(f.app.bases_state.is_some());
}

#[test]
fn palette_autosave_title_change_keeps_filename_and_insertion_context() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Edit);
    f.app.editor.title_editor = crate::events::make_title_editor(
        "Renamed",
        f.app.app_theme.highlight_fg,
        f.app.app_theme.highlight_bg,
    );
    drive(&mut f.app, key(KeyCode::F(6)));
    let context = f.app.command_palette.as_ref().unwrap().context.clone();
    f.app.autosave().unwrap();
    assert_eq!(f.app.editor.editing_id.as_deref(), Some("editing.md"));
    assert_eq!(
        f.app.storage.load_note("editing.md").unwrap().title,
        "Renamed"
    );
    assert!(!f.app.storage.notes_dir.join("Renamed.md").exists());
    let before = f.app.editor.body.text();
    assert!(execute_action_in_context("editor.insert_date", &mut f.app, &context).is_ok());
    assert_ne!(f.app.editor.body.text(), before);
}

#[test]
fn palette_retained_draw_canvas_outline_settings_survive_help_and_setup() {
    for mode in [ViewMode::Draw, ViewMode::Canvas, ViewMode::Outline] {
        let mut f = Fixture::new();
        f.enter(mode);
        f.app.open_help_page();
        f.app.config.ui.theme = "dracula".into();
        f.app.refresh_theme_from_config();
        f.app.config.features.tags = FeatureState::Deleted;
        f.app.apply_keybind_preset(KeybindPreset::Helix);
        match mode {
            ViewMode::Draw => {
                let state = f.app.draw_state.as_ref().unwrap();
                assert_eq!(state.theme.fg, f.app.app_theme.fg);
                assert_eq!(state.keybinds.draw, f.app.keybinds.draw);
            }
            ViewMode::Canvas => assert_eq!(
                f.app.canvas_state.as_ref().unwrap().keybinds.canvas,
                f.app.keybinds.canvas
            ),
            ViewMode::Outline => assert_eq!(
                f.app.outline_state.as_ref().unwrap().keybinds.outline,
                f.app.keybinds.outline
            ),
            _ => unreachable!(),
        }
        assert!(!f.app.keybinds.list.contains_key(&ListAction::ManageTags));
        f.app.open_setup_view();
        assert_eq!(f.app.return_mode, Some(mode));
        f.app.abort_setup();
        assert_eq!(f.app.mode, mode);
    }
}

#[test]
fn palette_strokes_connections_and_setup_rebootstrap_preserve_pending_owners() {
    let mut f = Fixture::new();
    f.enter(ViewMode::Draw);
    f.app.draw_state.as_mut().unwrap().current_stroke = Some(crate::draw::state::Stroke {
        points: vec![(1.0, 2.0)],
        color: (255, 255, 255),
    });
    drive(&mut f.app, key(KeyCode::F(6)));
    assert!(f.app.command_palette.is_none());
    assert_eq!(
        f.app
            .draw_state
            .as_ref()
            .unwrap()
            .current_stroke
            .as_ref()
            .unwrap()
            .points,
        [(1.0, 2.0)]
    );
    f.enter(ViewMode::Canvas);
    f.app
        .canvas_state
        .as_mut()
        .unwrap()
        .state
        .connection_source_id = Some("pending-node".into());
    drive(&mut f.app, key(KeyCode::F(6)));
    assert!(f.app.command_palette.is_none());
    assert_eq!(
        f.app
            .canvas_state
            .as_ref()
            .unwrap()
            .state
            .connection_source_id
            .as_deref(),
        Some("pending-node")
    );
    f.enter(ViewMode::Edit);
    f.app.editor.body.insert_str("pending wizard source ");
    f.app.editor.editing_id = None;
    let blocker = f.root.path().join("blocked");
    fs::write(&blocker, "block").unwrap();
    f.app.editor.template_edit_path = Some(blocker.join("template.toml"));
    let expected = f.app.editor.body.text();
    let config = fs::read(crate::config::ClinConfig::config_path().unwrap()).unwrap();
    f.app.open_setup_view();
    f.app.setup_state.as_mut().unwrap().vault_path = f.root.path().join("other-vault");
    activate(&mut f.app, "setup.finish");
    assert_eq!(f.app.mode, ViewMode::Setup);
    assert_eq!(f.app.return_mode, Some(ViewMode::Edit));
    assert_eq!(f.app.editor.body.text(), expected);
    assert!(f.app.setup_rebootstrap.is_none());
    assert!(!f.app.should_quit);
    assert_eq!(
        fs::read(crate::config::ClinConfig::config_path().unwrap()).unwrap(),
        config
    );
}

#[test]
fn palette_workflow_theme_goal_back_and_legacy_launches() {
    let mut f = Fixture::new();
    drive(&mut f.app, key(KeyCode::F(6)));
    drive(&mut f.app, Event::Paste("Switch Theme".into()));
    let root_ids: Vec<_> = f
        .app
        .command_palette
        .as_ref()
        .unwrap()
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(f.app.popups.active.is_none());
    let themes = f.app.command_palette.as_ref().unwrap().items.len();
    drive(&mut f.app, Event::Paste("no matching theme here".into()));
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(f.app.command_palette.is_some());
    assert!(f.app.command_palette.as_ref().unwrap().items.is_empty());
    drive(&mut f.app, key(KeyCode::Esc));
    let palette = f.app.command_palette.as_ref().unwrap();
    assert_eq!(palette.input.lines()[0], "Switch Theme");
    assert_eq!(
        palette
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        root_ids
    );
    assert!(themes > 2);
    drive(&mut f.app, key(KeyCode::Esc));
    f.app.begin_theme_selection();
    assert!(matches!(
        f.app.popups.active,
        Some(crate::popups::ActivePopup::Theme(_))
    ));
    f.app.close_theme_popup();
    activate(&mut f.app, "settings.word_goal");
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
    );
    drive(&mut f.app, Event::Paste("invalid".into()));
    drive(&mut f.app, key(KeyCode::Enter));
    let palette = f.app.command_palette.as_ref().unwrap();
    assert_eq!(palette.input.lines()[0], "invalid");
    assert!(palette.error.is_some());
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
    );
    drive(&mut f.app, Event::Paste("321".into()));
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(f.app.command_palette.is_none());
    assert_eq!(f.app.config.goals.word_goal, 321);
    assert_eq!(
        crate::config::ClinConfig::load().0.unwrap().goals.word_goal,
        321
    );
}

fn select_step(app: &mut App, id: &str) {
    let palette = app.command_palette.as_mut().unwrap();
    let index = palette
        .items
        .iter()
        .position(|item| item.id == id)
        .unwrap_or_else(|| panic!("missing step {id}"));
    palette.state.select(Some(index));
    drive(app, key(KeyCode::Enter));
}

fn replace_step_input(app: &mut App, text: &str) {
    drive(
        app,
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
    );
    drive(app, Event::Paste(text.into()));
}

#[test]
fn palette_workflow_options_creation_and_import_validation() {
    let mut f = Fixture::new();
    for (action, row, stays) in [
        ("settings.icon_mode", "1", false),
        ("settings.hint_bar_style", "4", true),
        ("keybind.preset", "2", true),
        ("settings.cycle_sort", "1", false),
    ] {
        activate(&mut f.app, action);
        assert!(f.app.popups.active.is_none());
        select_step(&mut f.app, row);
        assert_eq!(f.app.command_palette.is_some(), stays);
        if stays {
            drive(&mut f.app, key(KeyCode::Esc));
            drive(&mut f.app, key(KeyCode::Esc));
        }
    }
    assert_eq!(f.app.config.ui.icon_mode, crate::config::IconMode::Unicode);
    assert_eq!(
        f.app.config.ui.hint_bar_style,
        crate::config::HintBarStyle::Bubbles
    );
    assert_eq!(f.app.config.core.keybind_preset, KeybindPreset::Vim);
    activate(&mut f.app, "insert.url_append");
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(f.app.command_palette.as_ref().unwrap().error.is_some());
    replace_step_input(&mut f.app, "https://example.invalid/");
    fs::write(
        f.app.storage.note_path("selected.md"),
        "external replacement",
    )
    .unwrap();
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(
        f.app
            .command_palette
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("unavailable")
    );
    assert_eq!(
        fs::read_to_string(f.app.storage.note_path("selected.md")).unwrap(),
        "external replacement"
    );
    drive(&mut f.app, key(KeyCode::Esc));
    drive(&mut f.app, key(KeyCode::Esc));
    activate(&mut f.app, "draw.create");
    replace_step_input(&mut f.app, "../escape");
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(f.app.command_palette.as_ref().unwrap().error.is_some());
    replace_step_input(&mut f.app, "art");
    let original = fs::read(f.app.storage.note_path("art.draw")).unwrap();
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(f.app.command_palette.as_ref().unwrap().error.is_some());
    assert_eq!(
        fs::read(f.app.storage.note_path("art.draw")).unwrap(),
        original
    );
    replace_step_input(&mut f.app, "new-art");
    drive(&mut f.app, key(KeyCode::Enter));
    assert_eq!(f.app.mode, ViewMode::Draw);
    assert!(f.app.command_palette.is_none());
    assert!(f.app.storage.note_path("new-art.draw").is_file());
}

#[test]
fn palette_workflow_vault_alias_and_separate_confirmation() {
    let mut f = Fixture::new();
    let other = f.root.path().join("other-vault");
    fs::create_dir_all(&other).unwrap();
    f.app.config.core.vaults.push(other.clone());
    f.app.config.save().unwrap();
    activate(&mut f.app, "vault.switch");
    assert!(f.app.vault_switcher.is_none());
    drive(&mut f.app, key(KeyCode::F(2)));
    replace_step_input(&mut f.app, "My vault");
    drive(&mut f.app, key(KeyCode::Enter));
    assert!(
        f.app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.name == "My vault")
    );
    let raw = other.to_string_lossy();
    let palette = f.app.command_palette.as_mut().unwrap();
    let row = palette
        .items
        .iter()
        .position(|item| item.id == format!("vault:{raw}"))
        .unwrap();
    palette.state.select(Some(row));
    drive(&mut f.app, key(KeyCode::Delete));
    assert!(f.app.popups.confirm.is_some());
    let query = f.app.command_palette.as_ref().unwrap().input.lines()[0].clone();
    drive(&mut f.app, Event::Paste("must not reach palette".into()));
    assert_eq!(
        f.app.command_palette.as_ref().unwrap().input.lines()[0],
        query
    );
    drive(&mut f.app, key(KeyCode::Esc));
    assert!(f.app.popups.confirm.is_none());
    assert!(f.app.command_palette.is_some());
    drive(&mut f.app, key(KeyCode::Delete));
    f.app.confirm_action();
    f.app.refresh_command_palette();
    assert!(
        !f.app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.id == format!("vault:{raw}"))
    );
    drive(&mut f.app, key(KeyCode::Esc));
    drive(&mut f.app, key(KeyCode::Esc));
    drive(&mut f.app, key(KeyCode::F(4)));
    assert!(f.app.vault_switcher.is_some());
    assert!(f.app.command_palette.is_none());
}

#[test]
fn palette_workflow_subnotes_preserve_multiline_and_nested_back() {
    let mut f = Fixture::new();
    activate(&mut f.app, "manage_subnotes_list");
    select_step(&mut f.app, "add");
    select_step(&mut f.app, "title");
    replace_step_input(&mut f.app, "Child title");
    drive(&mut f.app, key(KeyCode::Enter));
    select_step(&mut f.app, "content");
    replace_step_input(&mut f.app, "first\nsecond");
    drive(&mut f.app, key(KeyCode::Enter));
    drive(&mut f.app, Event::Paste("third".into()));
    assert_eq!(
        f.app.command_palette.as_ref().unwrap().input.lines(),
        &["first", "second", "third"]
    );
    drive(
        &mut f.app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL)),
    );
    drive(&mut f.app, key(KeyCode::Esc));
    assert!(
        f.app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.name == "Child title")
    );
    drive(&mut f.app, key(KeyCode::Esc));
    let saved = f.app.storage.get_subnotes("selected.md").unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].title, "Child title");
    assert_eq!(saved[0].content, "first\nsecond\nthird");
    drive(&mut f.app, key(KeyCode::Esc));
    assert!(f.app.command_palette.is_none());
    f.app.open_subnotes_popup();
    assert!(matches!(
        f.app.popups.active,
        Some(crate::popups::ActivePopup::Subnotes(_))
    ));
}
