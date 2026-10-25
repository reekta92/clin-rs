use super::*;
use crate::app::{EditFocus, EditSidebar, HelpTab};
use crate::config::{ClinConfig, ConfigTestGuard, HintBarStyle, IconMode, NotesLayout};
use crate::list_view::VisualItem;
use crate::ui::{
    PopupHints, PopupSize, draw_popup_frame, draw_ui, subnotes_layout, truncate_cells,
};
use crate::ui::{format_keybind_hints, hit_test_tabs, icons, title_bar_tabs_region};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};

fn fixture() -> (tempfile::TempDir, App) {
    let (temp, mut app) = super::tests::grid_test_app(0);
    crate::config::set_config_path_override(app.storage.config_dir.join("config.toml"));
    app.mode = ViewMode::List;
    app.status = "".into();
    app.command_usage_path = Some(app.storage.config_dir.join("commands.json"));
    app.messages.messages.clear();
    std::fs::create_dir_all(app.storage.notes_dir.join("docs/nested")).unwrap();
    app.catalog_folders = vec!["docs".into(), "docs/nested".into()];
    for id in ["root.md", "docs/child.md", "docs/nested/deep.md"] {
        std::fs::write(app.storage.note_path(id), "# Body\n").unwrap();
        app.notes.push(app.storage.load_note_summary(id).unwrap());
    }
    (temp, app)
}

fn select_folder(app: &mut App, folder: &str) {
    app.list.visual_index = app
        .list
        .visual_list
        .iter()
        .position(|item| matches!(item, VisualItem::Folder { path, .. } if path == folder))
        .unwrap();
    app.request_preview_update();
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn text_in(buffer: &ratatui::buffer::Buffer, area: Rect) -> String {
    (area.y..area.bottom())
        .map(|y| {
            let mut text = String::new();
            let mut x = area.x;
            while x < area.right() {
                let symbol = buffer[(x, y)].symbol();
                text.push_str(symbol);
                x += unicode_width::UnicodeWidthStr::width(symbol).max(1) as u16;
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn directory_layouts_navigate_without_expanding_and_keep_actions_scoped() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    for layout in [NotesLayout::List, NotesLayout::Multi] {
        app.list.notes_layout = layout;
        app.enter_list_directory(String::new(), 0);
        assert!(!app.list.visual_list.iter().any(|item| {
            matches!(item, VisualItem::Note { summary_idx, .. } if app.notes[*summary_idx].folder == "docs")
        }));
        select_folder(&mut app, "docs");
        app.list.selected_indices.insert(app.list.visual_index);
        app.move_right();
        assert_eq!(app.list.grid_folder, "docs");
        app.begin_create_select_format();
        assert!(
            matches!(app.popups.active.take(), Some(crate::popups::ActivePopup::CreateFormat(popup)) if popup.folder == "docs")
        );
        assert!(app.list.selected_indices.is_empty());
        assert!(app.list.visual_list.iter().any(|item| {
            matches!(item, VisualItem::Note { summary_idx, .. } if app.notes[*summary_idx].id == "docs/child.md")
        }));
        select_folder(&mut app, "docs/nested");
        app.open_selected();
        assert_eq!(app.list.grid_folder, "docs/nested");
        app.move_left();
        assert_eq!(app.list.grid_folder, "docs");
        assert!(
            matches!(&app.list.visual_list[app.list.visual_index], VisualItem::Folder { path, .. } if path == "docs/nested")
        );
        app.move_left();
        assert_eq!(app.list.grid_folder, "");
        assert!(!app.list.folder_expanded.contains("docs"));
        app.cycle_grid_tab();
        assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_PINNED_PATH);
        app.move_left();
        assert_eq!(app.list.grid_folder, "");
    }
}

#[test]
fn multi_preview_shows_folder_children_and_mouse_enters_that_folder() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    let config = app
        .storage
        .folder_template_config_path("docs/nested")
        .unwrap();
    std::fs::write(config, "[variables]\nchapter = '1'\n").unwrap();
    app.list.notes_layout = NotesLayout::Multi;
    app.refresh_visual_list();
    select_folder(&mut app, "docs");
    assert!(app.list.template_config_folders.contains("docs/nested"));
    let area = Rect::new(0, 0, 120, 30);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
        .unwrap();
    let (browser, _, _) = list_view_layout(
        area,
        false,
        app.preview_position,
        false,
        false,
        0.43,
        9,
        crate::config::CalendarPosition::Bottom,
    );
    let (left, right) = browser_columns(browser, &NotesLayout::Multi);
    let right = right.unwrap();
    assert!(left.right() < right.x);
    assert!(text_in(terminal.backend().buffer(), right).contains("child"));
    assert!(!text_in(terminal.backend().buffer(), left).contains("child"));
    let index = app
        .list
        .multi_preview
        .as_ref()
        .unwrap()
        .1
        .iter()
        .position(|item| matches!(item, VisualItem::Folder { path, .. } if path == "docs/nested"))
        .unwrap();
    let original_index = app.list.visual_index;
    app.preview_fullscreen = true;
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::ScrollDown,
    ] {
        crate::events::handle_list_mouse(
            &mut app,
            mouse(kind, right.x + 2, right.y + 1 + index as u16),
            area,
        );
        assert_eq!(app.list.grid_folder, "");
        assert_eq!(app.list.visual_index, original_index);
    }
    app.preview_fullscreen = false;
    crate::events::handle_list_mouse(
        &mut app,
        mouse(
            MouseEventKind::Down(MouseButton::Left),
            right.x + 2,
            right.y + 1 + index as u16,
        ),
        area,
    );
    assert_eq!(app.list.grid_folder, "docs");
    assert!(
        matches!(&app.list.visual_list[app.list.visual_index], VisualItem::Folder { path, .. } if path == "docs/nested")
    );
    app.move_right();
    assert_eq!(app.list.grid_folder, "docs/nested");
}

#[test]
fn keyboard_layout_resize_moves_exactly_one_cell() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.layout_edit = true;
    app.list.preview_enabled = true;
    for width in [80, 137, 240] {
        app.list.last_area = Rect::new(0, 0, width, 32);
        for position in [
            crate::config::PreviewPosition::Left,
            crate::config::PreviewPosition::Right,
        ] {
            app.preview_position = position;
            app.list.preview_width_ratio = 0.43;
            let preview_width = |app: &App| {
                list_view_layout(
                    app.list.last_area,
                    true,
                    position,
                    false,
                    false,
                    app.list.preview_width_ratio,
                    9,
                    crate::config::CalendarPosition::Bottom,
                )
                .1
                .unwrap()
                .width
            };
            let initial = preview_width(&app);
            crate::events::handle_list_keys(
                &mut app,
                KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            );
            let expected = if position == crate::config::PreviewPosition::Left {
                initial - 1
            } else {
                initial + 1
            };
            assert_eq!(preview_width(&app), expected, "{width}, {position:?}");
            crate::events::handle_list_keys(
                &mut app,
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            );
            assert_eq!(preview_width(&app), initial);
        }
    }
}

#[test]
fn primary_scrollbar_uses_preview_divider_and_still_accepts_dragging() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.list.preview_enabled = true;
    app.config.list.folder_graph_preview = false;
    app.list.visual_list = (0..90)
        .map(|i| VisualItem::CreateNew {
            path: i.to_string(),
            depth: 0,
        })
        .collect();
    let area = Rect::new(0, 0, 120, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    for layout in [
        NotesLayout::Tree,
        NotesLayout::Grid,
        NotesLayout::List,
        NotesLayout::Multi,
    ] {
        app.list.notes_layout = layout;
        for position in [
            crate::config::PreviewPosition::Left,
            crate::config::PreviewPosition::Right,
        ] {
            app.preview_position = position;
            terminal
                .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
                .unwrap();
            let preview = list_view_layout(
                area,
                true,
                position,
                false,
                false,
                app.list.preview_width_ratio,
                9,
                crate::config::CalendarPosition::Bottom,
            )
            .1
            .unwrap();
            let meta = app.list.last_scroll.unwrap();
            assert_eq!(
                meta.track.x,
                if position == crate::config::PreviewPosition::Left {
                    preview.right()
                } else {
                    preview.x - 1
                }
            );
            assert_eq!(
                terminal.backend().buffer()[(meta.track.x, meta.track.y)].symbol(),
                "█"
            );
            app.mouse_pos = Some((meta.track.x, meta.track.y));
            terminal
                .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
                .unwrap();
            assert_eq!(
                terminal.backend().buffer()[(meta.track.x, meta.track.y)].fg,
                app.app_theme.accent
            );
            app.mouse_pos = None;
            crate::events::handle_list_mouse(
                &mut app,
                mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    meta.track.x,
                    meta.track.bottom() - 1,
                ),
                area,
            );
            assert!(app.list.visual_index > 0);
            app.list.scroll_drag = None;
            app.list.visual_index = 0;
        }
    }
}

#[test]
fn footer_hints_fit_all_styles_and_preserve_status_cells() {
    let items = (0..7)
        .map(|i| (format!("Ctrl+界{i}"), "long action"))
        .collect::<Vec<_>>();
    for mode in [
        IconMode::Nerd,
        IconMode::Unicode,
        IconMode::None,
        IconMode::Emoji,
    ] {
        for style in HintBarStyle::ALL {
            let theme = AppThemeColors {
                icon_mode: mode,
                hint_bar_style: style,
                ..AppThemeColors::default()
            };
            let hints = format_keybind_hints(&theme, &items);
            assert!(!hints.to_string().contains("界5"));
            for width in 1..100u16 {
                let fit = crate::ui::fit_footer_line(hints.clone(), width as usize, &theme, false);
                assert!(
                    fit.width() <= width as usize,
                    "{style:?}, {mode:?}, {width}"
                );
                let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                terminal
                    .draw(|frame| {
                        crate::ui::draw_status_bar(
                            frame,
                            frame.area(),
                            &theme,
                            hints.clone(),
                            Some(Line::from("STATUS")),
                        )
                    })
                    .unwrap();
                if width >= 6 {
                    assert_eq!(
                        text_in(terminal.backend().buffer(), Rect::new(width - 6, 0, 6, 1)),
                        "STATUS"
                    );
                }
            }
        }
    }
}

#[test]
fn help_about_is_last_and_side_content_remains_reachable_at_small_sizes() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.mode = ViewMode::Help;
    let tabs = crate::ui::help_tabs(IconMode::Emoji, &app.config.features);
    assert_eq!(tabs.last().unwrap().0, HelpTab::About);
    for (index, (tab, _, _)) in tabs.iter().enumerate() {
        assert_eq!(tab.index(), index);
        assert_eq!(HelpTab::from_index(index), *tab);
        assert_eq!(tab.next().prev(), *tab);
    }
    for (width, height) in [(80, 24), (104, 24), (120, 32), (160, 48)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        for (tab, _, _) in &tabs {
            app.help_tab = *tab;
            app.reroll_help_suggestions();
            app.help_side_scroll = [u16::MAX; 2];
            terminal
                .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
                .unwrap();
            let columns = crate::ui::help_body_columns(Rect::new(0, 1, width, height - 2));
            if columns[4].width > 0 {
                assert!(
                    app.help_side_scroll
                        .iter()
                        .zip(app.help_side_max_scroll)
                        .all(|(&scroll, max)| scroll <= max)
                );

                assert!(text_in(terminal.backend().buffer(), columns[4]).contains("commands."));
            }
        }
    }
}

#[test]
fn editor_sidebar_focus_fills_title_without_adding_another_divider() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.mode = ViewMode::Edit;
    let area = Rect::new(0, 0, 120, 32);
    for sidebar in [EditSidebar::Outline, EditSidebar::Links] {
        app.editor.sidebar = sidebar;
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Sidebar))
            .unwrap();
        let layout = crate::events::compute_edit_layout(
            Rect::new(0, 1, 120, 30),
            false,
            false,
            sidebar,
            app.preview_position,
            app.zen_padding(),
        );
        let pane = layout.sidebar.unwrap();
        assert!(
            (pane.y..pane.bottom())
                .all(|y| terminal.backend().buffer()[(pane.x, y)].symbol() != "┃")
        );
        assert_eq!(
            terminal.backend().buffer()[(pane.x + 5, pane.y + 1)].bg,
            app.app_theme.accent
        );
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
            .unwrap();
        assert_ne!(
            terminal.backend().buffer()[(pane.x + 5, pane.y + 1)].bg,
            app.app_theme.accent
        );
    }
}

#[test]
fn new_configuration_values_roundtrip_through_setup_and_toml() {
    for layout in [NotesLayout::List, NotesLayout::Multi] {
        let config = ClinConfig {
            list: crate::config::ListConfig {
                default_view: layout.clone(),
                ..Default::default()
            },
            ui: crate::config::UiConfig {
                icon_mode: IconMode::Emoji,
                ..Default::default()
            },
            ..Default::default()
        };
        let restored: ClinConfig = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(restored.list.default_view, layout);
        assert_eq!(restored.ui.icon_mode, IconMode::Emoji);
        assert_eq!(
            crate::setup::layout_at(crate::setup::layout_index(&layout)),
            layout
        );
    }
    assert_eq!(
        crate::setup::icon_mode_at(crate::setup::icon_mode_index(IconMode::Emoji)),
        IconMode::Emoji
    );
}

#[test]
fn rows_tabs_and_clickable_widgets_share_hover_feedback() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.list.notes_layout = NotesLayout::List;
    app.refresh_visual_list();
    app.list.visual_index = app.list.visual_list.len() - 1;
    app.config.ui.tab_icons_only = false;
    let area = Rect::new(0, 0, 120, 32);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    app.mouse_pos = Some((3, 2));
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(3, 2)].bg,
        app.app_theme.hover_style().bg.unwrap()
    );

    let tabs = [
        (
            "Vault",
            Some(crate::ui::icons::FOLDER.get(app.config.ui.icon_mode)),
        ),
        (
            "Pinned",
            Some(crate::ui::icons::PIN.get(app.config.ui.icon_mode)),
        ),
        (
            "Smart",
            Some(crate::ui::icons::FILTER.get(app.config.ui.icon_mode)),
        ),
        (
            "Subnotes",
            Some(crate::ui::icons::SUBNOTES.get(app.config.ui.icon_mode)),
        ),
    ];
    let region = crate::ui::title_bar_tabs_region(area, "Notes");
    let x = (0..area.width)
        .find(|&x| {
            crate::ui::hit_test_tabs(
                &tabs,
                area.x,
                area.width,
                region.x,
                x,
                false,
                app.config.ui.icon_mode,
            ) == Some(1)
        })
        .unwrap();
    app.mouse_pos = Some((x, 0));
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(x, 0)].bg,
        app.app_theme.hover_style().bg.unwrap(),
        "x={x}, status={:?}, row={}",
        app.status,
        text_in(terminal.backend().buffer(), Rect::new(0, 0, 120, 1))
    );

    app.list.calendar_enabled = true;
    app.list.sections = vec![crate::config::NotesSection::Draw];
    let strip = list_view_layout(
        area,
        false,
        app.preview_position,
        true,
        false,
        0.43,
        9,
        crate::config::CalendarPosition::Bottom,
    )
    .2
    .unwrap();
    app.mouse_pos = Some((strip.x + 2, strip.y + 2));
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(strip.x + 2, strip.y + 2)].bg,
        app.app_theme.hover_style().bg.unwrap()
    );
}

#[test]
fn scrollbar_colors_ignore_hover_and_zero_height_tracks_are_safe() {
    let theme = AppThemeColors::default();
    let mut terminal = Terminal::new(TestBackend::new(1, 10)).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::scrollbar::draw_scrollbar(frame, frame.area(), 100, 10, 0, 90, &theme)
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].fg, theme.accent);
    let mut drag = None;
    assert!(
        crate::ui::scrollbar::handle_scrollbar_mouse(
            &mouse(MouseEventKind::Down(MouseButton::Left), 0, 0),
            crate::ui::scrollbar::ScrollbarMeta {
                track: Rect::ZERO,
                content_len: 100,
                viewport_len: 0
            },
            0.0,
            &mut drag,
        )
        .is_none()
    );
}

#[test]
fn editor_title_keeps_its_center_when_focus_changes() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.mode = ViewMode::Edit;
    app.config.statusline.header_left = Some("EDIT".into());
    app.config.statusline.header_right = Some("body statistics".into());
    for title in ["Stable title", "界界 title", ""] {
        let displayed_title = if title.is_empty() {
            "Untitled note"
        } else {
            title
        };
        let first_symbol = displayed_title.chars().next().unwrap().to_string();
        app.editor.title_editor = crate::events::make_title_editor(
            title,
            app.app_theme.highlight_fg,
            app.app_theme.highlight_bg,
        );
        for status in [
            crate::editor::AutosaveStatus::Saved,
            crate::editor::AutosaveStatus::Unsaved,
        ] {
            app.editor.autosave_status = status;
            let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
            let mut starts = Vec::new();
            for focus in [EditFocus::Body, EditFocus::Title, EditFocus::Sidebar] {
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
                    .unwrap();
                let start = (0..100)
                    .find(|&x| terminal.backend().buffer()[(x, 0)].symbol() == first_symbol)
                    .unwrap();
                starts.push(start);
            }
            assert!(
                starts.iter().all(|&start| start == starts[0]),
                "{title}: {starts:?}"
            );
            let title_width = unicode_width::UnicodeWidthStr::width(displayed_title) as u16;
            assert!(
                starts[0].abs_diff((100 - title_width) / 2) <= 1,
                "{title}: {starts:?}"
            );
        }
    }
}

#[test]
fn clipped_editor_titles_keep_the_same_viewport_without_focus() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.mode = ViewMode::Edit;
    app.config.statusline.header_left = Some("EDIT".into());
    app.config.statusline.header_right = Some("statistics".into());
    app.editor.title_editor = crate::events::make_title_editor(
        "A long title with 界 characters and a visible ending",
        app.app_theme.highlight_fg,
        app.app_theme.highlight_bg,
    );
    app.editor
        .title_editor
        .move_cursor(ratatui_textarea::CursorMove::End);
    for width in [3, 16, 40, 80] {
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Title))
            .unwrap();
        let title_rect = app.editor.header_title_rect;
        let focused = text_in(terminal.backend().buffer(), title_rect);
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
            .unwrap();
        assert_eq!(app.editor.header_title_rect, title_rect);
        assert_eq!(
            text_in(terminal.backend().buffer(), title_rect),
            focused,
            "width {width}"
        );
    }
}

#[test]
fn crowded_footers_drop_version_before_shortcuts_or_status() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.note_index = None;
    app.config.statusline = crate::config::StatuslineConfig::default();
    for style in HintBarStyle::ALL {
        app.app_theme.hint_bar_style = style;
        for view in [ViewMode::List, ViewMode::Edit, ViewMode::Help] {
            for (width, watermark) in [(80, false), (180, true)] {
                let mut ctx = crate::statusline::StatuslineContext::for_view(&app, view);
                ctx.area = Some(Rect::new(0, 0, width, 1));
                ctx.hints = Some(vec![Span::raw("Useful shortcuts ".repeat(4))]);
                let (left, right) = crate::statusline::render_footer(
                    &ctx,
                    &app.config.statusline,
                    view,
                    &app.app_theme,
                );
                let right = right.map(|line| line.to_string()).unwrap_or_default();
                assert_eq!(
                    right.contains("clin v"),
                    watermark,
                    "{style:?}, {view:?}, {width}: {right}"
                );
                assert!(left.to_string().contains("Useful shortcuts"));
                if view == ViewMode::List {
                    assert!(right.contains("3 notes"));
                }
            }
        }
    }
}

#[test]
fn notes_modes_show_their_complete_shortcut_sets() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.app_theme.hint_bar_style = HintBarStyle::Classic;
    app.config.statusline.footer_right = Some(String::new());
    let mut terminal = Terminal::new(TestBackend::new(240, 24)).unwrap();
    app.list.list_mode = crate::list_view::ListMode::Select;
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
        .unwrap();
    let footer = text_in(terminal.backend().buffer(), Rect::new(0, 23, 240, 1));
    for action in ["toggle", "move", "copy", "tag", "untag", "delete", "exit"] {
        assert!(footer.contains(action), "{action}: {footer}");
    }
    app.list.list_mode = crate::list_view::ListMode::Normal;
    app.layout_edit = true;
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
        .unwrap();
    let footer = text_in(terminal.backend().buffer(), Rect::new(0, 23, 240, 1));
    for key in ["Space", "Ctrl+Space", "Tab", "a", "Esc"] {
        assert!(footer.contains(key), "{key}: {footer}");
    }
    for action in [
        "resize", "preview", "widgets", "cycle", "swap", "toggle", "done",
    ] {
        assert!(footer.contains(action), "{action}: {footer}");
    }
}

#[test]
fn confirmation_hover_and_clicks_match_only_the_visible_buttons() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    let area = Rect::new(0, 0, 120, 32);
    app.show_confirm(crate::popups::ConfirmAction::QuitApp);
    let popup = app.popups.confirm.as_mut().unwrap();
    let (chunks, buttons) = crate::ui::confirm_popup_layout(area, &popup.confirm_label, true);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::draw_confirm_popup(
                frame,
                popup,
                area,
                &app.app_theme,
                true,
                Some((buttons[0].x, buttons[0].y)),
            )
        })
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(buttons[0].x, buttons[0].y)].bg,
        app.app_theme.hover_style().bg.unwrap()
    );
    crate::events::handle_global_popup_mouse(
        &mut app,
        &mouse(
            MouseEventKind::Down(MouseButton::Left),
            chunks[0].x,
            chunks[0].y,
        ),
        area,
    );
    assert!(app.popups.confirm.is_some());
    assert!(!app.should_quit);
    crate::events::handle_global_popup_mouse(
        &mut app,
        &mouse(
            MouseEventKind::Down(MouseButton::Left),
            buttons[1].x,
            buttons[1].y,
        ),
        area,
    );
    assert!(app.popups.confirm.is_none());
    assert!(!app.should_quit);
    app.show_confirm(crate::popups::ConfirmAction::QuitApp);
    crate::events::handle_global_popup_mouse(
        &mut app,
        &mouse(
            MouseEventKind::Down(MouseButton::Left),
            buttons[0].x,
            buttons[0].y,
        ),
        area,
    );
    assert!(app.should_quit);
}

#[test]
fn widget_frames_share_padding_and_only_the_interface_border() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    for width in [12, 25, 60] {
        let area = Rect::new(2, 2, width, 10);
        let mut terminal = Terminal::new(TestBackend::new(70, 16)).unwrap();
        for bottom in [false, true] {
            for section in [
                crate::config::NotesSection::Calendar,
                crate::config::NotesSection::Activity,
                crate::config::NotesSection::Goals,
                crate::config::NotesSection::Todo,
                crate::config::NotesSection::Draw,
                crate::config::NotesSection::Graf,
            ] {
                terminal
                    .draw(|frame| match section {
                        crate::config::NotesSection::Calendar => {
                            crate::calendar::view::draw_widget(frame, area, &mut app, bottom, area)
                        }
                        crate::config::NotesSection::Activity => crate::calendar::draw_calendar(
                            frame,
                            area,
                            &app.app_theme,
                            &std::collections::HashMap::default(),
                            bottom,
                            crate::config::WeekStart::default(),
                            area,
                        ),
                        crate::config::NotesSection::Goals => crate::goals::draw_goals_progress(
                            frame,
                            area,
                            &app.app_theme,
                            &app.goals_progress,
                            &app.config.goals,
                            bottom,
                            area,
                        ),
                        crate::config::NotesSection::Todo => crate::todo::draw_todo(
                            frame,
                            area,
                            &app.app_theme,
                            &app.todo_state,
                            bottom,
                            area,
                        ),
                        crate::config::NotesSection::Draw => {
                            draw_strip_draw(frame, area, &app, bottom, area)
                        }
                        crate::config::NotesSection::Graf => {
                            draw_strip_graf(frame, area, &mut app, bottom, area)
                        }
                    })
                    .unwrap();
                let border_y = if bottom { area.bottom() - 1 } else { area.y };
                for x in area.x..area.right() {
                    assert_eq!(
                        terminal.backend().buffer()[(x, border_y)].symbol(),
                        "─",
                        "{section:?}"
                    );
                }
                let opposite_y = if bottom { area.y } else { area.bottom() - 1 };
                assert_eq!(
                    terminal.backend().buffer()[(area.x, opposite_y)].symbol(),
                    " ",
                    "{section:?}"
                );
            }
        }
    }
}

#[test]
fn notes_footer_has_no_selected_count_in_either_selection_mode() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.config.statusline = crate::config::StatuslineConfig::default();
    app.note_index = None;
    app.list.selected_indices.extend([0, 1]);
    for mode in [
        crate::list_view::ListMode::Normal,
        crate::list_view::ListMode::Select,
    ] {
        app.list.list_mode = mode;
        let ctx = crate::statusline::StatuslineContext::for_view(&app, ViewMode::List);
        let (_, right) = crate::statusline::render_footer(
            &ctx,
            &app.config.statusline,
            ViewMode::List,
            &app.app_theme,
        );
        let text = right.unwrap().to_string();
        assert!(text.contains("3 notes"), "{text}");
        assert!(!text.contains("selected"), "{text}");
    }
}

#[test]
fn section_tabs_toggle_and_persist_for_every_layout() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    let area = Rect::new(0, 0, 120, 32);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    app.config.ui.tab_icons_only = false;
    for layout in [
        NotesLayout::Grid,
        NotesLayout::Tree,
        NotesLayout::List,
        NotesLayout::Multi,
    ] {
        app.list.notes_layout = layout.clone();
        app.config.list.section_tabs = true;
        crate::actions::execute_action("settings.notes_section_tabs", &mut app, None).unwrap();
        assert!(!app.config.list.section_tabs);
        assert!(!ClinConfig::load().0.unwrap().list.section_tabs);
        assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_ROOT_PATH);
        for path in [
            "",
            crate::app::VIRTUAL_PINNED_PATH,
            crate::app::VIRTUAL_SMART_PATH,
            crate::app::VIRTUAL_SUBNOTES_PATH,
        ] {
            assert!(app.list.visual_list.iter().any(|item| matches!(item, VisualItem::Folder { path: folder, depth: 0, .. } if folder == path)), "{layout:?}: missing {path}");
        }
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
            .unwrap();
        assert!(
            !text_in(terminal.backend().buffer(), Rect::new(0, 0, area.width, 1))
                .contains("Pinned")
        );
        let original = app.list.grid_folder.clone();
        app.cycle_grid_tab();
        assert_eq!(app.list.grid_folder, original);
        app.list.folder_expanded.remove("");
        app.refresh_visual_list();
        select_folder(&mut app, "");
        app.open_selected();
        assert!(
            app.list
                .visual_list
                .iter()
                .any(|item| matches!(item, VisualItem::Folder { path, .. } if path == "docs"))
        );
        if layout.browses_folders() {
            assert_eq!(app.list.grid_folder, "");
            app.collapse_selected_folder();
            assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_ROOT_PATH);
            assert!(
                matches!(&app.list.visual_list[app.list.visual_index], VisualItem::Folder { path, .. } if path.is_empty())
            );
        }
        crate::actions::execute_action("settings.notes_section_tabs", &mut app, None).unwrap();
        assert!(app.config.list.section_tabs);
        assert!(ClinConfig::load().0.unwrap().list.section_tabs);
        assert_eq!(app.list.grid_folder, "");
        assert!(!app.list.visual_list.iter().any(|item| matches!(item, VisualItem::Folder { path, .. } if path.is_empty() || App::is_virtual_path(path))));
        app.status = "".into();
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
            .unwrap();
        assert!(
            text_in(terminal.backend().buffer(), Rect::new(0, 0, area.width, 1)).contains("Pinned"),
            "{layout:?} {:?}: {}",
            app.mode,
            text_in(terminal.backend().buffer(), Rect::new(0, 0, area.width, 1))
        );
        let tabs = [
            ("Vault", Some(icons::FOLDER.get(app.config.ui.icon_mode))),
            ("Pinned", Some(icons::PIN.get(app.config.ui.icon_mode))),
            ("Smart", Some(icons::FILTER.get(app.config.ui.icon_mode))),
            (
                "Subnotes",
                Some(icons::SUBNOTES.get(app.config.ui.icon_mode)),
            ),
        ];
        let region = title_bar_tabs_region(area, "Notes");
        let column = (0..area.width)
            .find(|&column| {
                hit_test_tabs(
                    &tabs,
                    area.x,
                    area.width,
                    region.x,
                    column,
                    false,
                    app.config.ui.icon_mode,
                ) == Some(1)
            })
            .unwrap();
        crate::events::handle_list_mouse(
            &mut app,
            mouse(MouseEventKind::Down(MouseButton::Left), column, 0),
            area,
        );
        assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_PINNED_PATH);
        app.cycle_grid_tab();
        assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_SMART_PATH);
        app.reverse_cycle_grid_tab();
        assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_PINNED_PATH);
    }
}

#[test]
fn section_folders_respect_features_and_cannot_be_modified() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.config.list.section_tabs = false;
    app.config.features.smart_folders = crate::config::FeatureState::Disabled;
    app.config.features.subnotes = crate::config::FeatureState::Disabled;
    for layout in [
        NotesLayout::Grid,
        NotesLayout::Tree,
        NotesLayout::List,
        NotesLayout::Multi,
    ] {
        app.list.notes_layout = layout;
        app.enter_list_directory(app.notes_root_directory(), 0);
        assert!(!app.list.visual_list.iter().any(|item| matches!(item, VisualItem::Folder { path, .. } if path == crate::app::VIRTUAL_SMART_PATH || path == crate::app::VIRTUAL_SUBNOTES_PATH)));
    }
    app.config.features.smart_folders = crate::config::FeatureState::Enabled;
    app.refresh_visual_list();
    select_folder(&mut app, "");
    app.begin_move_folder();
    assert!(app.popups.active.is_none());
    select_folder(&mut app, crate::app::VIRTUAL_SMART_PATH);
    app.list.selected_indices.insert(app.list.visual_index);
    assert_eq!(
        app.collect_selected_notes_and_folders().1,
        Vec::<String>::new()
    );
    app.begin_move_folder();
    assert!(app.popups.active.is_none());
    app.begin_rename_folder();
    assert!(app.popups.active.is_none());
    app.begin_delete_selected();
    assert!(app.popups.confirm.is_none());
    app.move_right();
    assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_SMART_PATH);
    app.move_left();
    assert_eq!(app.list.grid_folder, crate::app::VIRTUAL_ROOT_PATH);
}

#[test]
fn editor_scrollbar_uses_each_splitter_and_keeps_mouse_scrolling() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.mode = ViewMode::Edit;
    let source = "Line\n".repeat(100);
    app.editor.body = crate::editor_document::EditorDocument::from_source(&source);
    let area = Rect::new(0, 0, 120, 32);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    for position in [
        crate::config::PreviewPosition::Left,
        crate::config::PreviewPosition::Right,
    ] {
        app.preview_position = position;
        for (sidebar, preview) in [
            (EditSidebar::None, true),
            (EditSidebar::Outline, false),
            (EditSidebar::Links, false),
            (EditSidebar::None, false),
        ] {
            app.editor.sidebar = sidebar;
            app.editor.editor_preview_enabled = preview;
            app.editor.body_viewport_row = 0;
            app.editor.scroll_drag = None;
            terminal
                .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
                .unwrap();
            let layout = crate::events::compute_edit_layout(
                Rect::new(0, 1, 120, 30),
                false,
                preview,
                sidebar,
                position,
                app.zen_padding(),
            );
            let meta = app.editor.last_scroll.unwrap();
            assert_eq!(
                meta.track,
                crate::ui::scrollbar::pane_track(layout.body, layout.splitter)
            );
            assert!(
                (meta.track.y..meta.track.bottom())
                    .any(|y| terminal.backend().buffer()[(meta.track.x, y)].fg
                        == app.app_theme.accent)
            );
            let mut focus = EditFocus::Body;
            let mut selection = crate::text_edit::MouseTextSelection::default();
            crate::events::handle_edit_mouse(
                &mut app,
                mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    meta.track.x,
                    meta.track.bottom() - 1,
                ),
                area,
                &mut focus,
                &mut selection,
            );
            assert!(
                app.editor.body_viewport_row > 0,
                "{position:?}, {sidebar:?}"
            );
        }
    }
    app.preview_fullscreen = true;
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    assert!(app.editor.last_scroll.is_none());
}

#[test]
fn smart_folder_chevrons_only_render_in_tree_layout() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    let mut terminal = Terminal::new(TestBackend::new(40, 2)).unwrap();
    for layout in [
        NotesLayout::Tree,
        NotesLayout::Grid,
        NotesLayout::List,
        NotesLayout::Multi,
    ] {
        app.list.notes_layout = layout.clone();
        for mode in [
            IconMode::Nerd,
            IconMode::Unicode,
            IconMode::Emoji,
            IconMode::None,
        ] {
            app.config.ui.icon_mode = mode;
            for expanded in [false, true] {
                app.list.visual_list = vec![VisualItem::SmartFolder {
                    kind: crate::list_view::SmartFolderKind::Today,
                    label: "Today".to_owned(),
                    depth: 0,
                    is_expanded: expanded,
                    note_count: 3,
                }];
                terminal
                    .draw(|frame| {
                        frame.render_widget(
                            ratatui::widgets::List::new(vec![app.format_visual_item(0)]),
                            frame.area(),
                        )
                    })
                    .unwrap();
                let text = text_in(terminal.backend().buffer(), Rect::new(0, 0, 40, 1));
                let glyph = icons::TODAY.get(mode);
                let arrow = if expanded {
                    icons::EXPANDED.get(mode)
                } else {
                    icons::COLLAPSED.get(mode)
                };
                let prefix = if mode == IconMode::None {
                    "Today".to_owned()
                } else if layout == NotesLayout::Tree {
                    format!("{arrow} {glyph} Today")
                } else {
                    format!("{glyph} Today")
                };
                assert!(text.starts_with(&prefix), "{layout:?}, {mode:?}: {text:?}");
            }
        }
    }
}

#[test]
fn notes_row_density_keeps_hover_and_mouse_targets_on_the_rendered_rows() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    let area = Rect::new(0, 0, 120, 32);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    for density in [
        crate::config::ListDensity::Compact,
        crate::config::ListDensity::Comfortable,
    ] {
        app.list.list_density = density;
        for layout in [NotesLayout::Tree, NotesLayout::List, NotesLayout::Multi] {
            app.list.notes_layout = layout.clone();
            app.list.visual_list = vec![
                VisualItem::TemplateConfig {
                    folder: "docs".into(),
                    depth: 0,
                },
                VisualItem::SmartFolder {
                    kind: crate::list_view::SmartFolderKind::Today,
                    label: "Today".into(),
                    depth: 0,
                    is_expanded: false,
                    note_count: 3,
                },
            ];
            app.list.visual_index = 0;
            *app.list.list_state.offset_mut() = 0;
            let row = 2 + app.list.row_height();
            app.mouse_pos = Some((3, row));
            terminal
                .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
                .unwrap();
            assert!(
                text_in(terminal.backend().buffer(), Rect::new(0, row, 45, 1)).contains("Today")
            );
            assert_eq!(
                terminal.backend().buffer()[(3, row)].bg,
                app.app_theme.hover_style().bg.unwrap()
            );
            crate::events::handle_list_mouse(
                &mut app,
                mouse(MouseEventKind::Down(MouseButton::Left), 3, row),
                area,
            );
            assert_eq!(app.list.visual_index, 1, "{layout:?}");
        }
        app.list.notes_layout = NotesLayout::Multi;
        app.enter_list_directory(String::new(), 0);
        select_folder(&mut app, "docs");
        terminal
            .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::app::EditFocus::Body))
            .unwrap();
        let pane = browser_columns(Rect::new(0, 1, 120, 30), &NotesLayout::Multi)
            .1
            .unwrap();
        let index = app.list.multi_preview.as_ref().unwrap().1.iter().position(|item| matches!(item, VisualItem::Note { summary_idx, .. } if app.notes[*summary_idx].id == "docs/child.md")).unwrap();
        let row = pane.y + 1 + index as u16 * app.list.row_height();
        assert!(
            text_in(
                terminal.backend().buffer(),
                Rect::new(pane.x, row, pane.width, 1)
            )
            .contains("child")
        );
        crate::events::handle_list_mouse(
            &mut app,
            mouse(MouseEventKind::Down(MouseButton::Left), pane.x + 3, row),
            area,
        );
        assert_eq!(app.list.grid_folder, "docs");
        assert_eq!(app.list.visual_index, index);
    }
}

#[test]
fn popup_scrollbars_share_borders_and_keep_corners_and_mouse_targets() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    let area = Rect::new(0, 0, 100, 32);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    app.begin_theme_selection();
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    let Some(crate::popups::ActivePopup::Theme(popup)) = &app.popups.active else {
        panic!("theme popup");
    };
    let meta = popup.last_scroll.unwrap();
    assert!(meta.content_len > meta.viewport_len);
    assert_eq!(
        terminal.backend().buffer()[(meta.track.x, meta.track.y - 1)].symbol(),
        "┐"
    );
    assert_eq!(
        terminal.backend().buffer()[(meta.track.x, meta.track.bottom())].symbol(),
        "┘"
    );
    crate::events::handle_global_popup_mouse(
        &mut app,
        &mouse(
            MouseEventKind::Down(MouseButton::Left),
            meta.track.x,
            meta.track.bottom() - 1,
        ),
        area,
    );
    assert!(
        matches!(&app.popups.active, Some(crate::popups::ActivePopup::Theme(popup)) if popup.selected > 0)
    );
    app.popups.active = None;
    let subnotes = (0..70)
        .map(|index| crate::storage::SubNote {
            id: index.to_string(),
            title: format!("Subnote {index}"),
            content: "Body".into(),
            updated_at: 1,
        })
        .collect::<Vec<_>>();
    app.storage.set_subnotes("root.md", &subnotes).unwrap();
    app.open_subnotes_popup_for("root.md", None);
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    let Some(crate::popups::ActivePopup::Subnotes(popup)) = &app.popups.active else {
        panic!("subnotes popup");
    };
    let meta = popup.last_scroll.unwrap();
    assert!(meta.content_len > meta.viewport_len);
    assert_eq!(
        terminal.backend().buffer()[(meta.track.x, meta.track.y - 1)].symbol(),
        "┐"
    );
    assert_eq!(
        terminal.backend().buffer()[(meta.track.x, meta.track.bottom())].symbol(),
        "┘"
    );
    crate::events::handle_global_popup_mouse(
        &mut app,
        &mouse(
            MouseEventKind::Down(MouseButton::Left),
            meta.track.x,
            meta.track.bottom() - 1,
        ),
        area,
    );
    assert!(
        matches!(&app.popups.active, Some(crate::popups::ActivePopup::Subnotes(popup)) if popup.selected > 0)
    );
}

#[test]
fn popup_hints_fit_and_preserve_save_and_cancel() {
    let theme = AppThemeColors::default();
    for width in [30, 40, 80] {
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal
            .draw(|frame| {
                draw_popup_frame(
                    frame,
                    frame.area(),
                    "界面",
                    PopupSize::Large,
                    PopupHints::Keybinds(&[
                        ("Tab".into(), "navigate between all fields"),
                        ("Enter".into(), "save"),
                        ("Esc".into(), "cancel"),
                    ]),
                    &theme,
                );
            })
            .unwrap();
        let text = text_in(
            terminal.backend().buffer(),
            terminal.backend().buffer().area,
        );
        assert!(text.contains("Enter save"), "{width}: {text}");
        assert!(text.contains("Esc cancel"), "{width}: {text}");
        assert_eq!(truncate_cells("界e\u{301}界", 4), "界e\u{301}…");
    }
}

#[test]
fn search_hints_keep_the_current_action_and_cancel_visible() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.begin_search();
    for width in [40, 80, 120] {
        for (focus, action) in [
            (crate::popups::SearchFocus::Input, "Enter close"),
            (crate::popups::SearchFocus::Results, "Enter jump"),
        ] {
            let Some(crate::popups::ActivePopup::Search(popup)) = &mut app.popups.active else {
                panic!("search popup should remain open");
            };
            popup.focus = focus;
            popup.title_result_ids = vec!["root.md".into()];
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal
                .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
                .unwrap();
            let text = text_in(terminal.backend().buffer(), Rect::new(0, 0, width, 24));
            assert!(text.contains(action), "{width}: {text}");
            assert!(text.contains("Esc cancel"), "{width}: {text}");
        }
    }
}

#[test]
fn info_scrolls_to_last_section_with_keyboard_and_mouse() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.popups.active = Some(crate::popups::ActivePopup::Info(crate::popups::InfoPopup {
        title: "Information".into(),
        items: vec![crate::popups::InfoItem::Text {
            heading: "Long path".into(),
            body: format!("{}\nLast evidence", "界面 ".repeat(100)),
        }],
        scroll_offset: 0,
        last_scroll: None,
    }));
    let area = Rect::new(0, 0, 80, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    assert!(crate::events::handle_global_popups_and_palette(
        &mut app,
        crossterm::event::Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        area
    ));
    terminal
        .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    assert!(text_in(terminal.backend().buffer(), area).contains("Last evidence"));
    let Some(crate::popups::ActivePopup::Info(popup)) = &app.popups.active else {
        panic!("Info closed");
    };
    let end = popup.scroll_offset;
    let track = popup.last_scroll.unwrap().track;
    crate::events::handle_global_popup_mouse(
        &mut app,
        &mouse(MouseEventKind::ScrollUp, track.x, track.y),
        area,
    );
    assert!(
        matches!(&app.popups.active, Some(crate::popups::ActivePopup::Info(p)) if p.scroll_offset < end)
    );
}

#[test]
fn subnotes_keeps_focused_editor_usable_in_narrow_windows() {
    for width in [30, 48, 72, 100] {
        let content = Rect::new(4, 3, width, 12);
        let [list, title, body] =
            subnotes_layout(content, crate::popups::SubnotesFocus::EditContent);
        assert!(body.width >= 30);
        assert!(content.contains(body.as_position()));
        assert!(title.bottom() <= body.y);
        if width < 60 {
            assert!(list.is_empty());
            assert_eq!(body.width, content.width);
            assert_eq!(
                subnotes_layout(content, crate::popups::SubnotesFocus::List)[0],
                content
            );
        }
    }
}

#[test]
fn calendar_dialogs_share_chrome_and_help_remains_reachable() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    for (width, height) in [(80, 24), (120, 40), (40, 12)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let area = Rect::new(0, 0, width, height);
        let mut view = crate::calendar::view::View::new(chrono::Local::now().date_naive());
        view.dialog = Some(crate::calendar::view::Dialog::Help);
        terminal
            .draw(|frame| {
                crate::overlay::OverlayView::overlay_render(&mut view, frame, area, &mut app)
            })
            .unwrap();
        assert!(text_in(terminal.backend().buffer(), area).contains("CALENDAR HELP"));
        crate::overlay::OverlayView::overlay_handle_event(
            &mut view,
            crossterm::event::Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            &mut app,
            area,
        )
        .unwrap();
        terminal
            .draw(|frame| {
                crate::overlay::OverlayView::overlay_render(&mut view, frame, area, &mut app)
            })
            .unwrap();
        let text = text_in(terminal.backend().buffer(), area);
        assert!(text.contains("local."), "{width}×{height}: {text}");
        view.dialog = Some(crate::calendar::view::Dialog::Delete("event".into()));
        terminal
            .draw(|frame| {
                crate::overlay::OverlayView::overlay_render(&mut view, frame, area, &mut app)
            })
            .unwrap();
        let popup = crate::ui::centered_rect(PopupSize::Confirm, area);
        assert_eq!(
            terminal.backend().buffer()[(popup.x, popup.y)].fg,
            app.app_theme.destructive
        );
    }
}

#[test]
fn confirmation_scroll_preserves_cancel_selection_and_reveals_consequences() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.show_confirm(crate::popups::ConfirmAction::DeleteTemplate {
        filename: "template.md".into(),
        name: "Template".into(),
    });
    app.popups.confirm.as_mut().unwrap().message = "Long 界面 title ".repeat(50);
    app.popups.confirm.as_mut().unwrap().detail = Some("Permanent removal. Last evidence".into());
    let area = Rect::new(0, 0, 80, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    crate::events::handle_global_popups_and_palette(
        &mut app,
        crossterm::event::Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        area,
    );
    terminal
        .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    assert!(text_in(terminal.backend().buffer(), area).contains("Last evidence"));
    let popup = app.popups.confirm.as_ref().unwrap();
    assert_eq!(popup.selected_button, 1);
    assert!(popup.scroll_offset > 0);
}

#[test]
fn header_overlays_render_safely_in_short_and_narrow_frames() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    for (width, height) in [(1, 1), (8, 4), (40, 12), (80, 24), (120, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        app.quick_keybinds_open = true;
        terminal
            .draw(|frame| crate::ui::quick_keybinds::draw_quick_keybinds(frame, &app))
            .unwrap();
        app.quick_keybinds_open = false;
        app.messages.force_open = true;
        app.messages.push(
            "界面: a long message with combining e\u{301}".into(),
            crate::app::messages::MessageSeverity::Warning,
        );
        terminal
            .draw(|frame| {
                crate::ui::message_overlay::draw_message_overlay(
                    frame,
                    &app,
                    &app.app_theme,
                    frame.area(),
                )
            })
            .unwrap();
    }
}

#[test]
fn setup_confirmation_buttons_render_and_back_click_stays_in_setup() {
    let _guard = ConfigTestGuard::lock();
    let (_temp, mut app) = fixture();
    app.mode = ViewMode::Setup;
    app.config.core.mouse_enabled = true;
    app.setup_state = Some(crate::setup::SetupState::from_config(
        &app.config,
        &app.app_theme,
        app.storage.notes_dir.clone(),
        false,
    ));
    app.setup_state.as_mut().unwrap().confirm_exit = true;
    let area = Rect::new(0, 0, 80, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
        .unwrap();
    let text = text_in(terminal.backend().buffer(), area);
    assert!(
        text.contains("Save & exit") && text.contains("Discard changes") && text.contains("Back")
    );
    let back = crate::ui::setup::setup_confirm_buttons(area)[2];
    crate::events::handle_setup_mouse(
        &mut app,
        mouse(MouseEventKind::Down(MouseButton::Left), back.x + 2, back.y),
        area,
    );
    assert!(!app.setup_state.as_ref().unwrap().confirm_exit);
    assert_eq!(app.mode, ViewMode::Setup);
}
