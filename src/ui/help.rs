use ratatui::{prelude::*, widgets::*};

use super::{
    build_tab_spans, draw_status_bar, draw_view_title_bar_with_tabs, format_keybind_hints,
};
use crate::app::{App, HelpTab};
use crate::app_theme::AppThemeColors;
use crate::keybinds::{
    BackupAction, CanvasAction, ContentTreeAction, DrawAction, EditAction, GraphAction, HelpAction,
    Keybinds, ListAction,
};

pub fn help_tab_names(icon_mode: crate::config::IconMode) -> [(&'static str, &'static str); 9] {
    [
        (
            "Notes",
            crate::ui::get_icon("\u{f24a}", "\u{1f4cc}", icon_mode),
        ),
        (
            "Editor",
            crate::ui::get_icon("\u{f040}", "\u{270f}", icon_mode),
        ),
        (
            "Graph",
            crate::ui::get_icon("\u{f0e8}", "\u{1f5fa}", icon_mode),
        ),
        (
            "Draw",
            crate::ui::get_icon("\u{f1fc}", "\u{270f}", icon_mode),
        ),
        (
            "Canvas",
            crate::ui::get_icon("\u{f00a}", "\u{1f4cb}", icon_mode),
        ),
        (
            "Backup",
            crate::ui::get_icon("\u{f0c7}", "\u{1f4be}", icon_mode),
        ),
        (
            "Templates",
            crate::ui::get_icon("\u{f0c5}", "\u{1f4c4}", icon_mode),
        ),
        (
            "Content Tree",
            crate::ui::get_icon("\u{f1bb}", "\u{1f333}", icon_mode),
        ),
        (
            "About",
            crate::ui::get_icon("\u{f05a}", "\u{2139}", icon_mode),
        ),
    ]
}

#[derive(Clone)]
pub struct HelpRow {
    pub row: Row<'static>,
    pub search_text: String,
}

fn help_heading_row(title: &'static str, theme: &AppThemeColors) -> HelpRow {
    HelpRow {
        row: Row::new(vec![Cell::from(help_heading(title, theme))]),
        search_text: title.to_lowercase(),
    }
}

fn help_empty_row() -> HelpRow {
    HelpRow {
        row: Row::new(vec![Cell::from("")]),
        search_text: String::new(),
    }
}

fn help_raw_row(row: Row<'static>, search_text: &str) -> HelpRow {
    HelpRow {
        row,
        search_text: search_text.to_lowercase(),
    }
}

fn about_cli_row(cmd: &str, desc: &str, theme: &AppThemeColors) -> HelpRow {
    let search_text = format!("{} {}", cmd, desc);
    HelpRow {
        row: Row::new(vec![
            Cell::from(Line::from(vec![Span::styled(
                cmd.to_string(),
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )])),
            Cell::from(Line::from(vec![
                Span::styled("• ", Style::default().fg(theme.muted)),
                Span::raw(desc.to_owned()),
            ])),
        ]),
        search_text: search_text.to_lowercase(),
    }
}

pub fn draw_help_view(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let tabs: Vec<(&str, Option<&str>)> = help_tab_names(app.config.ui.icon_mode)
        .iter()
        .map(|&(l, g)| (l, Some(g)))
        .collect();
    let tab_spans = build_tab_spans(
        &tabs,
        app.help_tab.index(),
        &app.app_theme,
        app.config.ui.tab_icons_only,
        app.config.ui.icon_mode,
    );
    draw_view_title_bar_with_tabs(
        frame,
        chunks[0],
        "Help",
        tab_spans,
        &app.app_theme,
        Some(app.status.as_ref()),
        None,
    );

    let scroll = app.help_scroll;
    let _ = app.get_help_rows();
    let rows = app.list.help_text_cache.as_deref().unwrap();
    let theme = &app.app_theme;
    let visible_rows: Vec<Row<'static>> = rows
        .iter()
        .enumerate()
        .skip(scroll as usize)
        .map(|(abs_idx, hr)| {
            let mut row = hr.row.clone();
            if app.help_search.active && !app.help_search.results.is_empty() {
                let selected_row = app
                    .help_search
                    .results
                    .get(app.help_search.selected)
                    .map(|(idx, _)| *idx);
                let is_selected = Some(abs_idx) == selected_row;
                let is_matched = app
                    .help_search
                    .results
                    .iter()
                    .any(|(idx, _)| *idx == abs_idx);
                if is_selected {
                    row = row.style(
                        Style::default()
                            .bg(theme.highlight_bg)
                            .fg(theme.highlight_fg),
                    );
                } else if is_matched {
                    row =
                        row.style(Style::default().bg(theme.preview_bg().unwrap_or(Color::Reset)));
                }
            } else if let Some(hl_idx) = app.help_search.highlight_row {
                if abs_idx == hl_idx {
                    row = row.style(
                        Style::default()
                            .bg(theme.highlight_bg)
                            .fg(theme.highlight_fg),
                    );
                }
            }
            row
        })
        .collect();
    let table = Table::new(visible_rows, [Constraint::Length(30), Constraint::Min(20)]).block(
        Block::default()
            .style(app.app_theme.bg_style())
            .borders(Borders::NONE)
            .padding(Padding::new(2, 2, 1, 1)),
    );
    frame.render_widget(table, chunks[1]);

    let kb = &app.keybinds;
    let hints_items = vec![
        (
            format!(
                "{}/{}",
                kb.display_help(HelpAction::PrevTab),
                kb.display_help(HelpAction::NextTab)
            ),
            "switch tab",
        ),
        (
            format!(
                "{}/{}",
                kb.display_help(HelpAction::ScrollUp),
                kb.display_help(HelpAction::ScrollDown)
            ),
            "scroll",
        ),
        (kb.display_help(HelpAction::Search), "search"),
        (kb.display_help(HelpAction::Close), "close"),
    ];
    let hint = format_keybind_hints(&app.app_theme, &hints_items);
    draw_status_bar(
        frame,
        chunks[2],
        &app.app_theme,
        None,
        hint,
        None,
        app.seq_matcher.pending_display().as_deref(),
    );
    if app.help_search.active {
        draw_help_search(frame, chunks[1], app);
    }
}

fn draw_help_search(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.app_theme;
    let max_visible = 10usize;
    let result_count = app.help_search.results.len();
    let visible_count = result_count.min(max_visible);
    let popup_width = (50u16).min(area.width.saturating_sub(4));
    let popup_height = (visible_count + 3).min(area.height.saturating_sub(4) as usize) as u16;

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = 3;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    let before = &app.help_search.query[..app.help_search.cursor];
    let after = &app.help_search.query[app.help_search.cursor..];
    let label_style = Style::default().fg(theme.text);
    let cursor_style = Style::default()
        .fg(theme.border)
        .add_modifier(Modifier::REVERSED);
    let input_line = Line::from(vec![
        Span::styled(before.to_string(), label_style),
        Span::styled(
            after
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string()),
            cursor_style,
        ),
        Span::styled(
            after
                .chars()
                .next()
                .map(|_| {
                    after[after
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| i)
                        .unwrap_or(after.len())..]
                        .to_string()
                })
                .unwrap_or_default(),
            label_style,
        ),
    ]);

    let mut lines: Vec<Line> = vec![input_line];

    if result_count == 0 && !app.help_search.query.is_empty() {
        lines.push(Line::styled(
            "  No matches",
            Style::default().fg(theme.muted),
        ));
    } else {
        let scroll_offset = app
            .help_search
            .selected
            .saturating_sub(max_visible.saturating_sub(1));
        for (i, (_, search_text)) in app
            .help_search
            .results
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(max_visible)
        {
            let is_selected = i == app.help_search.selected;
            let style = if is_selected {
                Style::default()
                    .fg(theme.bg.unwrap_or(Color::Black))
                    .bg(theme.accent)
            } else {
                Style::default().fg(theme.text)
            };
            let prefix = "  ";
            let display_width = (popup_width as usize).saturating_sub(6);
            let display = if search_text.len() > display_width {
                format!("{}…", &search_text[..display_width.saturating_sub(1)])
            } else {
                search_text.clone()
            };
            lines.push(Line::styled(format!("{prefix}{display}"), style));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Search")
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}

pub fn help_text_for_tab(
    tab: HelpTab,
    keybinds: &Keybinds,
    theme: &AppThemeColors,
    config: &crate::config::ClinConfig,
) -> Vec<HelpRow> {
    match tab {
        HelpTab::Notes => notes_help_text(keybinds, theme),
        HelpTab::Editor => editor_help_text(keybinds, theme),
        HelpTab::Graph => graph_help_text(keybinds, theme),
        HelpTab::Draw => draw_help_text(keybinds, theme),
        HelpTab::Canvas => canvas_help_text(keybinds, theme),
        HelpTab::Backup => backup_help_text(keybinds, theme),
        HelpTab::Templates => templates_help_text(keybinds, theme),
        HelpTab::ContentTree => content_tree_help_text(keybinds, theme),
        HelpTab::About => about_help_text(keybinds, theme, config),
    }
}

fn notes_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let list_move = format!(
        "{}/{}",
        keybinds.list_keys_display(ListAction::MoveUp),
        keybinds.list_keys_display(ListAction::MoveDown)
    );
    let list_move_horiz = format!(
        "{}/{}",
        keybinds.list_keys_display(ListAction::MoveLeft),
        keybinds.list_keys_display(ListAction::MoveRight)
    );
    let list_expand_collapse = format!(
        "{}/{}",
        keybinds.list_keys_display(ListAction::ExpandFolder),
        keybinds.list_keys_display(ListAction::CollapseFolder)
    );
    let list_open = keybinds.list_keys_display(ListAction::Open);
    let list_create_note = keybinds.list_keys_display(ListAction::CreateNote);
    let list_create_folder = keybinds.list_keys_display(ListAction::CreateFolder);
    let list_rename = keybinds.list_keys_display(ListAction::Rename);
    let list_rename_folder = keybinds.list_keys_display(ListAction::RenameFolder);
    let list_delete = keybinds.list_keys_display(ListAction::Delete);
    let list_duplicate = keybinds.list_keys_display(ListAction::Duplicate);
    let list_move_note = keybinds.list_keys_display(ListAction::MoveNote);
    let list_manage_tags = keybinds.list_keys_display(ListAction::ManageTags);
    let list_pin = keybinds.list_keys_display(ListAction::TogglePin);
    let list_toggle_external = keybinds.list_keys_display(ListAction::ToggleExternalEditor);
    let list_search = keybinds.list_keys_display(ListAction::Search);
    let list_select_mode = keybinds.list_keys_display(ListAction::ToggleSelectMode);
    let list_select_item = keybinds.list_keys_display(ListAction::ToggleSelectItem);
    let list_toggle_preview = keybinds.list_keys_display(ListAction::TogglePreview);
    let list_toggle_preview_fs = keybinds.list_keys_display(ListAction::TogglePreviewFullscreen);
    let list_toggle_preview_wrap = keybinds.list_keys_display(ListAction::TogglePreviewWrap);
    let list_preview_page = format!(
        "{}/{}",
        keybinds.list_keys_display(ListAction::PreviewPageUp),
        keybinds.list_keys_display(ListAction::PreviewPageDown)
    );
    let list_toggle_calendar = keybinds.list_keys_display(ListAction::ToggleCalendar);
    let list_open_graph = keybinds.list_keys_display(ListAction::OpenGraph);
    let list_cmd_palette = keybinds.list_keys_display(ListAction::OpenCommandPalette);
    let list_cycle_sort = keybinds.list_keys_display(ListAction::CycleSort);
    let list_jump_top = keybinds.list_keys_display(ListAction::JumpToTop);
    let list_jump_bottom = keybinds.list_keys_display(ListAction::JumpToBottom);
    let list_page_up = keybinds.list_keys_display(ListAction::PageUp);
    let list_page_down = keybinds.list_keys_display(ListAction::PageDown);
    let list_location = keybinds.list_keys_display(ListAction::OpenLocation);
    let list_trash = keybinds.list_keys_display(ListAction::OpenTrash);
    let list_collapse_all = keybinds.list_keys_display(ListAction::CollapseAll);
    let list_refresh = keybinds.list_keys_display(ListAction::RefreshNotes);
    let list_template = keybinds.list_keys_display(ListAction::NewFromTemplate);
    let list_help = keybinds.list_keys_display(ListAction::Help);
    let list_quit = keybinds.list_keys_display(ListAction::Quit);
    let list_cycle_focus = keybinds.list_keys_display(ListAction::CycleFocus);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Navigation", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Move selection", Some(&list_move), theme));
    rows.push(help_item_dyn(
        "Move selection in grid",
        Some(&list_move_horiz),
        theme,
    ));
    rows.push(help_item_dyn(
        "Expand/Collapse folder",
        Some(&list_expand_collapse),
        theme,
    ));
    rows.push(help_item_dyn(
        "Scroll up/down half page",
        Some(&format!("{list_page_up}/{list_page_down}")),
        theme,
    ));
    rows.push(help_item_dyn("Jump to top", Some(&list_jump_top), theme));
    rows.push(help_item_dyn(
        "Jump to bottom",
        Some(&list_jump_bottom),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Actions", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Open selected item", Some(&list_open), theme));
    rows.push(help_item_dyn(
        "Create new note",
        Some(&list_create_note),
        theme,
    ));
    rows.push(help_item_dyn(
        "Create new folder",
        Some(&list_create_folder),
        theme,
    ));
    rows.push(help_item_dyn("Rename note", Some(&list_rename), theme));
    rows.push(help_item_dyn(
        "Rename folder",
        Some(&list_rename_folder),
        theme,
    ));
    rows.push(help_item_dyn("Delete", Some(&list_delete), theme));
    rows.push(help_item_dyn(
        "Duplicate note",
        Some(&list_duplicate),
        theme,
    ));
    rows.push(help_item_dyn(
        "Move note or folder",
        Some(&list_move_note),
        theme,
    ));
    rows.push(help_item_dyn("Manage tags", Some(&list_manage_tags), theme));
    rows.push(help_item_dyn("Toggle pin", Some(&list_pin), theme));
    rows.push(help_item_dyn(
        "Toggle external editor",
        Some(&list_toggle_external),
        theme,
    ));
    rows.push(help_item_dyn(
        "Open file location",
        Some(&list_location),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Display", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Search", Some(&list_search), theme));
    rows.push(help_item_dyn(
        "Toggle select mode",
        Some(&list_select_mode),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle select item",
        Some(&list_select_item),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle preview pane",
        Some(&list_toggle_preview),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle preview fullscreen",
        Some(&list_toggle_preview_fs),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle preview wrap",
        Some(&list_toggle_preview_wrap),
        theme,
    ));
    rows.push(help_item_dyn(
        "Page preview up/down",
        Some(&list_preview_page),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle calendar",
        Some(&list_toggle_calendar),
        theme,
    ));
    rows.push(help_item_dyn(
        "Swap strip sections (layout-edit)",
        Some("Tab"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Add/remove strip section (layout-edit)",
        Some("a"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Cycle strip section (layout-edit)",
        Some("click"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Open graph view",
        Some(&list_open_graph),
        theme,
    ));
    rows.push(help_item_dyn(
        "Open command palette",
        Some(&list_cmd_palette),
        theme,
    ));
    rows.push(help_item_dyn(
        "Collapse all folders",
        Some(&list_collapse_all),
        theme,
    ));
    rows.push(help_item_dyn(
        "Refresh notes (pick up external changes)",
        Some(&list_refresh),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("General", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Cycle sort order",
        Some(&list_cycle_sort),
        theme,
    ));
    rows.push(help_item_dyn("Open trash", Some(&list_trash), theme));
    rows.push(help_item_dyn(
        "New note from template",
        Some(&list_template),
        theme,
    ));
    rows.push(help_item_dyn(
        "Cycle focus between panes",
        Some(&list_cycle_focus),
        theme,
    ));
    rows.push(help_item_dyn("Help", Some(&list_help), theme));
    rows.push(help_item_dyn("Quit", Some(&list_quit), theme));
    rows
}

fn editor_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let edit_focus = keybinds.edit_keys_display(EditAction::CycleFocus);
    let edit_back = keybinds.edit_keys_display(EditAction::Back);
    let edit_copy = keybinds.edit_keys_display(EditAction::Copy);
    let edit_cut = keybinds.edit_keys_display(EditAction::Cut);
    let edit_paste = keybinds.edit_keys_display(EditAction::Paste);
    let edit_select_all = keybinds.edit_keys_display(EditAction::SelectAll);
    let edit_undo = keybinds.edit_keys_display(EditAction::Undo);
    let edit_redo = keybinds.edit_keys_display(EditAction::Redo);
    let edit_del_word = keybinds.edit_keys_display(EditAction::DeleteWord);
    let edit_del_next_word = keybinds.edit_keys_display(EditAction::DeleteNextWord);
    let edit_md_preview = keybinds.edit_keys_display(EditAction::ToggleMarkdownPreview);
    let edit_fullscreen = keybinds.edit_keys_display(EditAction::TogglePreviewFullscreen);
    let edit_wrap = keybinds.edit_keys_display(EditAction::TogglePreviewWrap);
    let edit_preview_page = format!(
        "{}/{}",
        keybinds.edit_keys_display(EditAction::PreviewPageUp),
        keybinds.edit_keys_display(EditAction::PreviewPageDown)
    );
    let edit_move_top = keybinds.edit_keys_display(EditAction::MoveToTop);
    let edit_move_bottom = keybinds.edit_keys_display(EditAction::MoveToBottom);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Navigation", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Cycle focus (Title, Content)",
        Some(&edit_focus),
        theme,
    ));
    rows.push(help_item_dyn(
        "Return to notes (auto-saves)",
        Some(&edit_back),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Editing", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Copy", Some(&edit_copy), theme));
    rows.push(help_item_dyn("Cut", Some(&edit_cut), theme));
    rows.push(help_item_dyn("Paste", Some(&edit_paste), theme));
    rows.push(help_item_dyn("Select all", Some(&edit_select_all), theme));
    rows.push(help_item_dyn("Undo", Some(&edit_undo), theme));
    rows.push(help_item_dyn("Redo", Some(&edit_redo), theme));
    rows.push(help_item_dyn(
        "Delete previous word",
        Some(&edit_del_word),
        theme,
    ));
    rows.push(help_item_dyn(
        "Delete next word",
        Some(&edit_del_next_word),
        theme,
    ));
    rows.push(help_item_dyn(
        "Move cursor to top",
        Some(&edit_move_top),
        theme,
    ));
    rows.push(help_item_dyn(
        "Move cursor to bottom",
        Some(&edit_move_bottom),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Preview", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Toggle markdown preview",
        Some(&edit_md_preview),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle preview fullscreen",
        Some(&edit_fullscreen),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle preview wrap",
        Some(&edit_wrap),
        theme,
    ));
    rows.push(help_item_dyn(
        "Page preview up/down",
        Some(&edit_preview_page),
        theme,
    ));
    rows
}

fn graph_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let graph_pan = format!(
        "{}/{}/{}/{}",
        keybinds.graph_keys_display(GraphAction::PanUp),
        keybinds.graph_keys_display(GraphAction::PanDown),
        keybinds.graph_keys_display(GraphAction::PanLeft),
        keybinds.graph_keys_display(GraphAction::PanRight)
    );
    let graph_zoom = format!(
        "{}/{}",
        keybinds.graph_keys_display(GraphAction::ZoomIn),
        keybinds.graph_keys_display(GraphAction::ZoomOut)
    );
    let graph_open = keybinds.graph_keys_display(GraphAction::OpenNote);
    let graph_autofit = keybinds.graph_keys_display(GraphAction::AutoFit);
    let graph_search = keybinds.graph_keys_display(GraphAction::ToggleSearch);
    let graph_minimap = keybinds.graph_keys_display(GraphAction::ToggleMinimap);
    let graph_legend = keybinds.graph_keys_display(GraphAction::ToggleLegend);
    let graph_grid = keybinds.graph_keys_display(GraphAction::ToggleGrid);
    let graph_status = keybinds.graph_keys_display(GraphAction::ToggleStatus);
    let graph_refresh = keybinds.graph_keys_display(GraphAction::Refresh);
    let graph_reload = keybinds.graph_keys_display(GraphAction::ReloadConfig);
    let graph_help = keybinds.graph_keys_display(GraphAction::Help);
    let graph_preview = keybinds.graph_keys_display(GraphAction::TogglePreview);
    let graph_quit = keybinds.graph_keys_display(GraphAction::Quit);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Navigation", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Navigate", Some(&graph_pan), theme));
    rows.push(help_item_dyn("Zoom in/out", Some(&graph_zoom), theme));
    rows.push(help_item_dyn(
        "Open selected note",
        Some(&graph_open),
        theme,
    ));
    rows.push(help_item_dyn(
        "Auto-fit graph to viewport",
        Some(&graph_autofit),
        theme,
    ));
    rows.push(help_item_dyn("Search nodes", Some(&graph_search), theme));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Display", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Toggle minimap", Some(&graph_minimap), theme));
    rows.push(help_item_dyn("Toggle legend", Some(&graph_legend), theme));
    rows.push(help_item_dyn(
        "Toggle background grid",
        Some(&graph_grid),
        theme,
    ));
    rows.push(help_item_dyn(
        "Toggle status bar",
        Some(&graph_status),
        theme,
    ));
    rows.push(help_item_dyn("Toggle preview", Some(&graph_preview), theme));
    rows.push(help_empty_row());

    rows.push(help_heading_row("System", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Refresh physics",
        Some(&graph_refresh),
        theme,
    ));
    rows.push(help_item_dyn("Reload config", Some(&graph_reload), theme));
    rows.push(help_item_dyn("Help", Some(&graph_help), theme));
    rows.push(help_item_dyn("Quit graph view", Some(&graph_quit), theme));
    rows
}

fn draw_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let draw_quit = keybinds.draw_keys_display(DrawAction::Quit);
    let draw_tool = keybinds.draw_keys_display(DrawAction::SelectDrawTool);
    let draw_shape = keybinds.draw_keys_display(DrawAction::ToggleShapeSelector);
    let draw_text = keybinds.draw_keys_display(DrawAction::SelectTextTool);
    let draw_erase = keybinds.draw_keys_display(DrawAction::SelectEraseTool);
    let draw_shape_up = keybinds.draw_keys_display(DrawAction::ShapeSelectorUp);
    let draw_shape_down = keybinds.draw_keys_display(DrawAction::ShapeSelectorDown);
    let draw_shape_confirm = keybinds.draw_keys_display(DrawAction::ShapeSelectorConfirm);
    let draw_shape_cancel = keybinds.draw_keys_display(DrawAction::ShapeSelectorCancel);
    let draw_text_confirm = keybinds.draw_keys_display(DrawAction::TextEditorConfirm);
    let draw_text_cancel = keybinds.draw_keys_display(DrawAction::TextEditorCancel);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Tools", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Draw freehand strokes",
        Some(&draw_tool),
        theme,
    ));
    rows.push(help_item_dyn(
        "Shape tool (opens picker)",
        Some(&draw_shape),
        theme,
    ));
    rows.push(help_item_dyn("Place text label", Some(&draw_text), theme));
    rows.push(help_item_dyn("Erase elements", Some(&draw_erase), theme));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Shape Selector", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Select previous shape",
        Some(&draw_shape_up),
        theme,
    ));
    rows.push(help_item_dyn(
        "Select next shape",
        Some(&draw_shape_down),
        theme,
    ));
    rows.push(help_item_dyn(
        "Confirm shape selection",
        Some(&draw_shape_confirm),
        theme,
    ));
    rows.push(help_item_dyn(
        "Cancel shape selection",
        Some(&draw_shape_cancel),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Text Editor", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Confirm text edit",
        Some(&draw_text_confirm),
        theme,
    ));
    rows.push(help_item_dyn(
        "Cancel text edit",
        Some(&draw_text_cancel),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("General", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Exit canvas view", Some(&draw_quit), theme));
    rows
}

fn canvas_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let canvas_move = format!(
        "{}/{}/{}/{}",
        keybinds.canvas_keys_display(CanvasAction::MoveLeft),
        keybinds.canvas_keys_display(CanvasAction::MoveRight),
        keybinds.canvas_keys_display(CanvasAction::MoveUp),
        keybinds.canvas_keys_display(CanvasAction::MoveDown)
    );
    let canvas_zoom_in = keybinds.canvas_keys_display(CanvasAction::ZoomIn);
    let canvas_zoom_out = keybinds.canvas_keys_display(CanvasAction::ZoomOut);
    let canvas_zoom_fine_in = keybinds.canvas_keys_display(CanvasAction::ZoomFineIn);
    let canvas_zoom_fine_out = keybinds.canvas_keys_display(CanvasAction::ZoomFineOut);
    let canvas_edit = keybinds.canvas_keys_display(CanvasAction::EditOrConnect);
    let canvas_context = keybinds.canvas_keys_display(CanvasAction::OpenContextMenu);
    let canvas_grid = keybinds.canvas_keys_display(CanvasAction::ToggleGrid);
    let canvas_editor_pane = keybinds.canvas_keys_display(CanvasAction::ToggleEditorPane);
    let canvas_focus = keybinds.canvas_keys_display(CanvasAction::CycleFocus);
    let canvas_editor_unfocus = keybinds.canvas_keys_display(CanvasAction::EditorUnfocus);
    let canvas_editor_sync = keybinds.canvas_keys_display(CanvasAction::EditorSyncRaw);
    let canvas_save = keybinds.canvas_keys_display(CanvasAction::Save);
    let canvas_help = keybinds.canvas_keys_display(CanvasAction::Help);
    let canvas_rename_confirm = keybinds.canvas_keys_display(CanvasAction::RenameConfirm);
    let canvas_rename_cancel = keybinds.canvas_keys_display(CanvasAction::RenameCancel);
    let canvas_menu_close = keybinds.canvas_keys_display(CanvasAction::MenuClose);
    let canvas_menu_up = keybinds.canvas_keys_display(CanvasAction::MenuUp);
    let canvas_menu_down = keybinds.canvas_keys_display(CanvasAction::MenuDown);
    let canvas_menu_select = keybinds.canvas_keys_display(CanvasAction::MenuSelect);
    let canvas_close_editor = keybinds.canvas_keys_display(CanvasAction::CloseEditor);
    let canvas_close_editor_alt = keybinds.canvas_keys_display(CanvasAction::CloseEditorAlt);
    let canvas_resize_confirm = keybinds.canvas_keys_display(CanvasAction::ConfirmResize);
    let canvas_resize_cancel = keybinds.canvas_keys_display(CanvasAction::CancelResize);
    let canvas_quit = keybinds.canvas_keys_display(CanvasAction::Quit);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Navigation", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Move selection", Some(&canvas_move), theme));
    rows.push(help_item_dyn("Zoom in", Some(&canvas_zoom_in), theme));
    rows.push(help_item_dyn("Zoom out", Some(&canvas_zoom_out), theme));
    rows.push(help_item_dyn(
        "Zoom in (fine)",
        Some(&canvas_zoom_fine_in),
        theme,
    ));
    rows.push(help_item_dyn(
        "Zoom out (fine)",
        Some(&canvas_zoom_fine_out),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Editing", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Open / edit / connect",
        Some(&canvas_edit),
        theme,
    ));
    rows.push(help_item_dyn("Context menu", Some(&canvas_context), theme));
    rows.push(help_item_dyn("Save canvas file", Some(&canvas_save), theme));
    rows.push(help_item_dyn(
        "Rename confirm",
        Some(&canvas_rename_confirm),
        theme,
    ));
    rows.push(help_item_dyn(
        "Rename cancel",
        Some(&canvas_rename_cancel),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Interface", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Toggle grid", Some(&canvas_grid), theme));
    rows.push(help_item_dyn(
        "Toggle editor pane",
        Some(&canvas_editor_pane),
        theme,
    ));
    rows.push(help_item_dyn("Cycle focus", Some(&canvas_focus), theme));
    rows.push(help_item_dyn(
        "Exit editor focus",
        Some(&canvas_editor_unfocus),
        theme,
    ));
    rows.push(help_item_dyn(
        "Save raw editor changes",
        Some(&canvas_editor_sync),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Menus & Popups", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Close context menu",
        Some(&canvas_menu_close),
        theme,
    ));
    rows.push(help_item_dyn(
        "Menu select up",
        Some(&canvas_menu_up),
        theme,
    ));
    rows.push(help_item_dyn(
        "Menu select down",
        Some(&canvas_menu_down),
        theme,
    ));
    rows.push(help_item_dyn(
        "Menu confirm",
        Some(&canvas_menu_select),
        theme,
    ));
    rows.push(help_item_dyn(
        "Close editor",
        Some(&canvas_close_editor),
        theme,
    ));
    rows.push(help_item_dyn(
        "Close editor (alt)",
        Some(&canvas_close_editor_alt),
        theme,
    ));
    rows.push(help_item_dyn(
        "Resize confirm",
        Some(&canvas_resize_confirm),
        theme,
    ));
    rows.push(help_item_dyn(
        "Resize cancel",
        Some(&canvas_resize_cancel),
        theme,
    ));
    rows.push(help_item_dyn("Help", Some(&canvas_help), theme));
    rows.push(help_item_dyn("Quit canvas view", Some(&canvas_quit), theme));
    rows
}
fn backup_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let backup_move = format!(
        "{}/{}",
        keybinds.backup_keys_display(BackupAction::MoveUp),
        keybinds.backup_keys_display(BackupAction::MoveDown)
    );
    let backup_scroll_diff = format!(
        "{}/{}",
        keybinds.backup_keys_display(BackupAction::ScrollDiffUp),
        keybinds.backup_keys_display(BackupAction::ScrollDiffDown)
    );
    let backup_refresh = keybinds.backup_keys_display(BackupAction::Refresh);
    let backup_commit = keybinds.backup_keys_display(BackupAction::EnterCommit);
    let backup_push = keybinds.backup_keys_display(BackupAction::Push);
    let backup_settings = keybinds.backup_keys_display(BackupAction::OpenSettings);
    let backup_cycle = keybinds.backup_keys_display(BackupAction::CycleSection);
    let backup_back = keybinds.backup_keys_display(BackupAction::Back);
    let backup_stage_file = keybinds.backup_keys_display(BackupAction::StageFile);
    let backup_unstage_file = keybinds.backup_keys_display(BackupAction::UnstageFile);
    let backup_stage_all = keybinds.backup_keys_display(BackupAction::StageAll);
    let backup_pull = keybinds.backup_keys_display(BackupAction::Pull);
    let backup_help = keybinds.backup_keys_display(BackupAction::Help);
    let backup_cancel_commit = keybinds.backup_keys_display(BackupAction::CancelCommit);
    let backup_confirm_commit = keybinds.backup_keys_display(BackupAction::ConfirmCommit);
    let backup_close_settings = keybinds.backup_keys_display(BackupAction::CloseSettings);
    let backup_next_field = keybinds.backup_keys_display(BackupAction::NextField);
    let backup_prev_field = keybinds.backup_keys_display(BackupAction::PrevField);
    let backup_activate = keybinds.backup_keys_display(BackupAction::ActivateField);
    let backup_cancel_edit = keybinds.backup_keys_display(BackupAction::CancelEditField);
    let backup_confirm_edit = keybinds.backup_keys_display(BackupAction::ConfirmEditField);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Navigation", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Move selection", Some(&backup_move), theme));
    rows.push(help_item_dyn(
        "Scroll diff up/down",
        Some(&backup_scroll_diff),
        theme,
    ));
    rows.push(help_item_dyn("Cycle sections", Some(&backup_cycle), theme));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Actions", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Stage file", Some(&backup_stage_file), theme));
    rows.push(help_item_dyn("Unstage file", Some(&backup_unstage_file), theme));
    rows.push(help_item_dyn("Stage all changes", Some(&backup_stage_all), theme));
    rows.push(help_item_dyn(
        "Refresh status",
        Some(&backup_refresh),
        theme,
    ));
    rows.push(help_item_dyn("Enter commit", Some(&backup_commit), theme));
    rows.push(help_item_dyn(
        "Confirm commit",
        Some(&backup_confirm_commit),
        theme,
    ));
    rows.push(help_item_dyn(
        "Cancel commit",
        Some(&backup_cancel_commit),
        theme,
    ));
    rows.push(help_item_dyn("Push to remote", Some(&backup_push), theme));
    rows.push(help_item_dyn("Pull from remote", Some(&backup_pull), theme));
    rows.push(help_item_dyn(
        "Open settings",
        Some(&backup_settings),
        theme,
    ));
    rows.push(help_item_dyn(
        "Close settings",
        Some(&backup_close_settings),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Settings Fields", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Next field", Some(&backup_next_field), theme));
    rows.push(help_item_dyn(
        "Previous field",
        Some(&backup_prev_field),
        theme,
    ));
    rows.push(help_item_dyn(
        "Activate field",
        Some(&backup_activate),
        theme,
    ));
    rows.push(help_item_dyn(
        "Confirm edit field",
        Some(&backup_confirm_edit),
        theme,
    ));
    rows.push(help_item_dyn(
        "Cancel edit field",
        Some(&backup_cancel_edit),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("General", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Show help", Some(&backup_help), theme));
    rows.push(help_item_dyn("Back to list", Some(&backup_back), theme));
    rows
}

fn content_tree_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let ct_move = format!(
        "{}/{}",
        keybinds.content_tree_keys_display(ContentTreeAction::MoveUp),
        keybinds.content_tree_keys_display(ContentTreeAction::MoveDown)
    );
    let ct_collapse = keybinds.content_tree_keys_display(ContentTreeAction::ToggleCollapse);
    let ct_expand_all = keybinds.content_tree_keys_display(ContentTreeAction::ExpandAll);
    let ct_collapse_all = keybinds.content_tree_keys_display(ContentTreeAction::CollapseAll);
    let ct_open = keybinds.content_tree_keys_display(ContentTreeAction::Open);
    let ct_back = keybinds.content_tree_keys_display(ContentTreeAction::Back);
    let ct_help = keybinds.content_tree_keys_display(ContentTreeAction::Help);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Navigation", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Move selection", Some(&ct_move), theme));
    rows.push(help_item_dyn(
        "Toggle collapse/expand",
        Some(&ct_collapse),
        theme,
    ));
    rows.push(help_item_dyn("Expand all", Some(&ct_expand_all), theme));
    rows.push(help_item_dyn("Collapse all", Some(&ct_collapse_all), theme));
    rows.push(help_empty_row());
    rows.push(help_heading_row("Actions", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn("Jump to section", Some(&ct_open), theme));
    rows.push(help_item_dyn("Back", Some(&ct_back), theme));
    rows.push(help_item_dyn("Help", Some(&ct_help), theme));
    rows
}

fn templates_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let list_template = keybinds.list_keys_display(ListAction::NewFromTemplate);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Picker", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Open template picker from notes view",
        Some(&list_template),
        theme,
    ));
    rows.push(help_item_dyn(
        "Search templates by name",
        Some("Type in search bar"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Switch search/results focus",
        Some("Tab"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Open help from template picker",
        Some("?"),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Files", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Templates directory",
        Some("~/.config/clin/templates/"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Default template filename",
        Some("default.toml"),
        theme,
    ));
    rows.push(help_empty_row());

    rows.push(help_heading_row("Variables", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Variable: current date",
        Some("{date}"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Variable: date and time",
        Some("{datetime}"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Variable: current time",
        Some("{time}"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Variable: weekday name",
        Some("{weekday}"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Variable: 4-digit year",
        Some("{year}"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Variable: zero-padded month",
        Some("{month}"),
        theme,
    ));
    rows.push(help_item_dyn(
        "Variable: zero-padded day",
        Some("{day}"),
        theme,
    ));
    rows
}
fn about_help_text(
    _keybinds: &Keybinds,
    theme: &AppThemeColors,
    config: &crate::config::ClinConfig,
) -> Vec<HelpRow> {
    let mut rows = Vec::new();
    rows.push(help_raw_row(
        Row::new(vec![Cell::from(Line::from(vec![
            Span::styled(
                "clin",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  v{}", env!("CARGO_PKG_VERSION")),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))]),
        &format!("clin v{}", env!("CARGO_PKG_VERSION")),
    ));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Feature-packed terminal note management app",
        None,
        theme,
    ));
    rows.push(help_empty_row());

    if config.counts_enabled() {
        rows.push(help_heading_row("Count Prefix", theme));
        rows.push(help_empty_row());
        rows.push(help_item_dyn(
            "Type a number before a motion key to repeat it N times (e.g. 3j, 11k, 5G)",
            None,
            theme,
        ));
        rows.push(help_empty_row());
    }

    rows.push(help_heading_row("Configuration", theme));
    rows.push(help_empty_row());
    rows.push(help_item_dyn(
        "Keybinds overlay: ~/.config/clin/keybinds_<preset>.toml",
        None,
        theme,
    ));
    rows.push(help_item_dyn(
        "Theme + storage:  ~/.config/clin/config.toml",
        None,
        theme,
    ));
    rows.push(help_item_dyn(
        "Templates dir: <storage>/templates/",
        None,
        theme,
    ));
    rows.push(help_empty_row());
    rows.push(help_heading_row("CLI Usage", theme));
    rows.push(help_empty_row());
    rows.push(about_cli_row("clin", "Launch interactive TUI", theme));
    rows.push(about_cli_row(
        "clin --config <PATH>",
        "Override config file",
        theme,
    ));
    rows.push(about_cli_row("clin help", "Show CLI help", theme));
    rows.push(help_empty_row());
    rows.push(about_cli_row("clin notes list", "List note titles", theme));
    rows.push(about_cli_row(
        "clin notes new [TITLE]",
        "Create note + open TUI",
        theme,
    ));
    rows.push(about_cli_row(
        "clin notes open <TITLE>",
        "Open existing note",
        theme,
    ));
    rows.push(about_cli_row(
        "clin notes quick <text> [TITLE]",
        "Quick note without TUI",
        theme,
    ));
    rows.push(about_cli_row(
        "clin notes search <query>",
        "Search notes",
        theme,
    ));
    rows.push(help_empty_row());
    rows.push(about_cli_row(
        "clin storage show",
        "Show current storage path",
        theme,
    ));
    rows.push(about_cli_row(
        "clin storage set <PATH>",
        "Set storage directory",
        theme,
    ));
    rows.push(about_cli_row(
        "clin storage reset",
        "Reset to default storage",
        theme,
    ));
    rows.push(about_cli_row(
        "clin storage migrate",
        "Migrate data from old location",
        theme,
    ));
    rows.push(help_empty_row());
    rows.push(about_cli_row(
        "clin keybinds show",
        "Show current keybindings",
        theme,
    ));
    rows.push(about_cli_row(
        "clin keybinds export",
        "Export keybinds as TOML",
        theme,
    ));
    rows.push(about_cli_row(
        "clin keybinds reset",
        "Reset keybinds to defaults",
        theme,
    ));
    rows.push(help_empty_row());
    rows.push(about_cli_row(
        "clin templates list",
        "List available templates",
        theme,
    ));
    rows.push(about_cli_row(
        "clin templates init",
        "Create example templates",
        theme,
    ));
    rows.push(help_empty_row());
    rows.push(about_cli_row(
        "clin config show",
        "Print effective config as TOML",
        theme,
    ));
    rows.push(about_cli_row(
        "clin config path",
        "Print config file path",
        theme,
    ));
    rows.push(about_cli_row(
        "clin config edit",
        "Open config in $EDITOR",
        theme,
    ));
    rows.push(about_cli_row(
        "clin config reset",
        "Reset config to defaults",
        theme,
    ));
    rows
}

pub fn help_heading(title: &'static str, theme: &AppThemeColors) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {} ", title.to_uppercase()),
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.highlight_bg)
            .add_modifier(Modifier::BOLD),
    ))
}

fn format_keybind(key: &str) -> String {
    let parts: Vec<_> = key
        .split(" / ")
        .map(|group| {
            group
                .split('/')
                .map(|k| k.to_string())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    parts.join(" / ")
}

pub fn help_item_dyn(text: &str, key: Option<&str>, theme: &AppThemeColors) -> HelpRow {
    let search_text = match key {
        Some(key) => format!("{} {}", text, key).to_lowercase(),
        None => text.to_lowercase(),
    };
    match key {
        Some(key) => {
            let formatted_key = format_keybind(key);
            HelpRow {
                row: Row::new(vec![
                    Cell::from(Line::from(vec![Span::styled(
                        formatted_key,
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD),
                    )])),
                    Cell::from(Line::from(vec![
                        Span::styled("• ", Style::default().fg(theme.muted)),
                        Span::raw(text.to_owned()),
                    ])),
                ]),
                search_text,
            }
        }
        None => HelpRow {
            row: Row::new(vec![
                Cell::from(""),
                Cell::from(Line::from(vec![Span::raw(text.to_owned())])),
            ]),
            search_text,
        },
    }
}

fn split_lock_spans(
    text: &str,
    theme: &AppThemeColors,
    icon_mode: crate::config::IconMode,
) -> Vec<Span<'static>> {
    let mut result = Vec::new();
    if icon_mode == crate::config::IconMode::None {
        let mut last = 0;
        for (i, c) in text.char_indices() {
            if c == '\u{f023}' || c == '\u{1f512}' {
                if i > last {
                    result.push(Span::raw(text[last..i].to_string()));
                }
                last = i + c.len_utf8();
            }
        }
        if last < text.len() {
            result.push(Span::raw(text[last..].to_string()));
        }
        return result;
    }

    let mut last = 0;
    let lock_char = crate::ui::get_char('\u{f023}', '\u{1f512}', icon_mode);
    for (i, c) in text.char_indices() {
        if c == '\u{f023}' || c == '\u{1f512}' {
            if i > last {
                result.push(Span::raw(text[last..i].to_string()));
            }
            result.push(Span::styled(
                lock_char.to_string(),
                Style::default()
                    .fg(theme.destructive)
                    .add_modifier(Modifier::BOLD),
            ));
            last = i + c.len_utf8();
        }
    }
    if last < text.len() {
        result.push(Span::raw(text[last..].to_string()));
    }
    result
}

pub fn styled_result_line(
    s: &str,
    theme: &AppThemeColors,
    icon_mode: crate::config::IconMode,
) -> Line<'static> {
    if let Some(tag_start) = s.find(" [") {
        let after_tag = &s[tag_start..];
        if let Some(close_bracket) = after_tag.find(']') {
            let after_bracket = &after_tag[close_bracket + 1..];
            let is_end_tag = after_bracket.is_empty() || after_bracket.starts_with(" (");
            if is_end_tag {
                let label_part = &s[..tag_start];
                let tag_end = if let Some(count_start) = after_tag.find(" (") {
                    count_start
                } else {
                    after_tag.len()
                };
                let tag_content = &after_tag[..tag_end];
                let count_part = if tag_end < after_tag.len() {
                    Some(&after_tag[tag_end..])
                } else {
                    None
                };

                let mut spans = split_lock_spans(label_part, theme, icon_mode);
                spans.push(Span::styled(
                    tag_content.to_string(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));

                if let Some(count) = count_part {
                    spans.push(Span::styled(
                        count.to_string(),
                        Style::default()
                            .fg(theme.heading)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                return Line::from(spans);
            }
        }
    }

    if let Some(count_start) = s.find(" (") {
        let count_part = &s[count_start + 1..];
        if count_part.starts_with('(')
            && count_part.len() > 2
            && count_part.ends_with(')')
            && count_part[1..count_part.len() - 1]
                .chars()
                .all(|c| c.is_ascii_digit())
        {
            let label_part = &s[..count_start + 1];
            let mut spans = split_lock_spans(label_part, theme, icon_mode);
            spans.push(Span::styled(
                count_part.to_string(),
                Style::default()
                    .fg(theme.heading)
                    .add_modifier(Modifier::BOLD),
            ));
            return Line::from(spans);
        }
    }
    Line::from(split_lock_spans(s, theme, icon_mode))
}

pub fn style_palette_name(name: &str, theme: &AppThemeColors) -> Vec<Span<'static>> {
    if let Some(pos) = name.find(" [") {
        let base = &name[..pos];
        let state = &name[pos..];

        let state_style = if state.contains("[On]") {
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD)
        } else if state.contains("[Off]") {
            Style::default()
                .fg(theme.destructive)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD)
        };

        vec![
            Span::styled(
                base.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(state.to_string(), state_style),
        ]
    } else if let Some(stripped) = name.strip_prefix("Sort Order: ") {
        vec![
            Span::styled(
                "Sort Order: ".to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                stripped.to_string(),
                Style::default()
                    .fg(theme.heading)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    } else {
        vec![Span::styled(
            name.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        )]
    }
}
