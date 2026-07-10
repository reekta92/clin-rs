#![allow(clippy::vec_init_then_push)]
use ratatui::{prelude::*, widgets::*};

use super::{
    build_tab_spans, draw_status_bar, draw_view_title_bar_with_tabs, format_keybind_hints,
};
use crate::app::{App, HelpTab, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::keybinds::help_meta::{self, HelpMeta};
use crate::keybinds::{HelpAction, Keybinds, ListAction};
use strum::IntoEnumIterator;

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
    pub display: String,
    pub group: &'static str,
    pub tab: HelpTab,
}

fn help_heading_row(title: &'static str, theme: &AppThemeColors, tab: HelpTab) -> HelpRow {
    HelpRow {
        row: Row::new(vec![Cell::from(help_heading(title, theme))]),
        search_text: title.to_lowercase(),
        display: title.to_string(),
        group: title,
        tab,
    }
}

fn help_empty_row(tab: HelpTab) -> HelpRow {
    HelpRow {
        row: Row::new(vec![Cell::from("")]),
        search_text: String::new(),
        display: String::new(),
        group: "",
        tab,
    }
}

fn help_raw_row(
    row: Row<'static>,
    search_text: &str,
    display: &str,
    group: &'static str,
    tab: HelpTab,
) -> HelpRow {
    HelpRow {
        row,
        search_text: search_text.to_lowercase(),
        display: display.to_string(),
        group,
        tab,
    }
}

fn about_cli_row(cmd: &str, desc: &str, theme: &AppThemeColors, tab: HelpTab) -> HelpRow {
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
        display: format!("{} — {}", cmd, desc),
        group: "CLI Usage",
        tab,
    }
}
fn help_item_row(
    desc: &str,
    key: &str,
    group: &'static str,
    tab: HelpTab,
    theme: &AppThemeColors,
) -> HelpRow {
    use ratatui::layout::Alignment;
    let key_span = if key.is_empty() {
        Span::styled("\u{2014}".to_string(), Style::default().fg(theme.muted))
    } else {
        Span::styled(
            format_keybind(key),
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        )
    };
    let key_cell = Cell::from(Line::from(vec![key_span]).alignment(Alignment::Right));
    let desc_cell = Cell::from(Line::from(vec![
        Span::styled("\u{2022} ", Style::default().fg(theme.muted)),
        Span::raw(desc.to_owned()),
    ]));
    HelpRow {
        row: Row::new(vec![key_cell, desc_cell]),
        search_text: format!("{desc} {key}").to_lowercase(),
        display: desc.to_string(),
        group,
        tab,
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
    let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::Help);
    ctx.area = Some(chunks[0]);
    let (left_line, right_line) = crate::statusline::render_header(
        &ctx,
        &app.config.statusline,
        ViewMode::Help,
        &app.app_theme,
    );
    draw_view_title_bar_with_tabs(
        frame,
        chunks[0],
        "Help",
        &app.app_theme,
        left_line,
        tab_spans,
        right_line,
        Some(app.status.as_ref()),
    );

    let scroll = app.help_scroll;
    let rows = app.get_help_rows();
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
            } else if let Some(hl_idx) = app.help_search.highlight_row
                && abs_idx == hl_idx
            {
                row = row.style(
                    Style::default()
                        .bg(theme.highlight_bg)
                        .fg(theme.highlight_fg),
                );
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
    let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::Help);
    ctx.area = Some(chunks[2]);
    ctx.hints = Some(hint.spans);
    if let Some(p) = &app.seq_matcher.pending_display() {
        ctx.pending = Some(vec![Span::styled(
            format!("{} ", p),
            Style::default()
                .fg(app.app_theme.highlight_fg)
                .bg(app.app_theme.accent),
        )]);
    }

    let (left_line, right_line) = crate::statusline::render_footer(
        &ctx,
        &app.config.statusline,
        ViewMode::Help,
        &app.app_theme,
    );
    draw_status_bar(frame, chunks[2], &app.app_theme, left_line, right_line);
    if app.help_search.active {
        draw_help_search(frame, chunks[1], app);
    }
}

fn draw_help_search(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.app_theme;
    let max_visible = 10usize;
    let result_count = app.help_search.results.len();

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 3u16 + (max_visible as u16).min(result_count as u16).min(12);
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = if area.height >= popup_height + 2 {
        area.y + 1
    } else {
        area.y
    };
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    let input_display = if app.help_search.query.is_empty() {
        "Search…".to_string()
    } else {
        app.help_search.query.clone()
    };
    let text_style = Style::default().fg(theme.text);
    let input_line = Line::from(vec![
        Span::styled("> ", text_style),
        Span::styled(input_display, text_style),
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
        for (i, (_, display)) in app
            .help_search
            .results
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(max_visible)
        {
            let is_selected = i == app.help_search.selected;
            let display_width = (popup_width as usize).saturating_sub(6);
            let display = if display.len() > display_width {
                format!("{}…", &display[..display_width.saturating_sub(1)])
            } else {
                display.clone()
            };
            let prefix = if is_selected { "▸ " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
            } else {
                text_style
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, display),
                style,
            )));
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
        HelpTab::Templates => templates_help_text(keybinds, theme, tab),
        HelpTab::ContentTree => content_tree_help_text(keybinds, theme),
        HelpTab::About => about_help_text(keybinds, theme, config, tab),
    }
}

fn generate_scope_help<A>(
    keybinds: &Keybinds,
    theme: &AppThemeColors,
    tab: HelpTab,
    group_order: &[&'static str],
    meta_of: fn(A) -> HelpMeta,
    keys_of: fn(&Keybinds, A) -> String,
) -> Vec<HelpRow>
where
    A: IntoEnumIterator + Copy,
{
    // bucket variants by group, preserving iteration order within each group
    let mut buckets: Vec<(&'static str, Vec<(String, &'static str)>)> = Vec::new();
    for a in A::iter() {
        let m = meta_of(a);
        let key = keys_of(keybinds, a);
        match buckets.iter_mut().find(|(g, _)| *g == m.group) {
            Some(b) => b.1.push((key, m.description)),
            None => buckets.push((m.group, vec![(key, m.description)])),
        }
    }
    let mut rows = Vec::new();
    // emit in group_order; then any leftover groups (defensive — none expected)
    let mut emitted: Vec<&'static str> = group_order.to_vec();
    for group in group_order {
        if let Some(b) = buckets.iter().find(|(g, _)| *g == *group) {
            rows.push(help_heading_row(group, theme, tab));
            rows.push(help_empty_row(tab));
            for (key, desc) in &b.1 {
                rows.push(help_item_row(desc, key, group, tab, theme));
            }
            rows.push(help_empty_row(tab));
        }
    }
    for (group, items) in &buckets {
        if !emitted.contains(group) {
            emitted.push(group);
            rows.push(help_heading_row(group, theme, tab));
            rows.push(help_empty_row(tab));
            for (key, desc) in items {
                rows.push(help_item_row(desc, key, group, tab, theme));
            }
            rows.push(help_empty_row(tab));
        }
    }
    rows
}

fn notes_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    let mut rows = generate_scope_help(
        keybinds,
        theme,
        HelpTab::Notes,
        help_meta::list_group_order(),
        help_meta::list_action_meta,
        |kb, a| kb.list_keys_display(a),
    );
    // Layout-edit hints are not action-enum backed — append statically.
    rows.push(help_heading_row("Layout Edit", theme, HelpTab::Notes));
    rows.push(help_empty_row(HelpTab::Notes));
    rows.push(help_item_row(
        "Swap strip sections",
        "Tab",
        "Layout Edit",
        HelpTab::Notes,
        theme,
    ));
    rows.push(help_item_row(
        "Add/remove strip section",
        "a",
        "Layout Edit",
        HelpTab::Notes,
        theme,
    ));
    rows.push(help_item_row(
        "Cycle strip section",
        "click",
        "Layout Edit",
        HelpTab::Notes,
        theme,
    ));
    rows
}
fn editor_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    generate_scope_help(
        keybinds,
        theme,
        HelpTab::Editor,
        help_meta::edit_group_order(),
        help_meta::edit_action_meta,
        |kb, a| kb.edit_keys_display(a),
    )
}
fn graph_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    generate_scope_help(
        keybinds,
        theme,
        HelpTab::Graph,
        help_meta::graph_group_order(),
        help_meta::graph_action_meta,
        |kb, a| kb.graph_keys_display(a),
    )
}
fn draw_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    generate_scope_help(
        keybinds,
        theme,
        HelpTab::Draw,
        help_meta::draw_group_order(),
        help_meta::draw_action_meta,
        |kb, a| kb.draw_keys_display(a),
    )
}
fn canvas_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    generate_scope_help(
        keybinds,
        theme,
        HelpTab::Canvas,
        help_meta::canvas_group_order(),
        help_meta::canvas_action_meta,
        |kb, a| kb.canvas_keys_display(a),
    )
}
fn backup_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    generate_scope_help(
        keybinds,
        theme,
        HelpTab::Backup,
        help_meta::backup_group_order(),
        help_meta::backup_action_meta,
        |kb, a| kb.backup_keys_display(a),
    )
}
fn content_tree_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Vec<HelpRow> {
    generate_scope_help(
        keybinds,
        theme,
        HelpTab::ContentTree,
        help_meta::content_tree_group_order(),
        help_meta::content_tree_action_meta,
        |kb, a| kb.content_tree_keys_display(a),
    )
}

fn templates_help_text(keybinds: &Keybinds, theme: &AppThemeColors, tab: HelpTab) -> Vec<HelpRow> {
    let list_template = keybinds.list_keys_display(ListAction::NewFromTemplate);

    let mut rows = Vec::new();
    rows.push(help_heading_row("Picker", theme, tab));
    rows.push(help_empty_row(tab));
    rows.push(help_item_dyn(
        "Open template picker from notes view",
        Some(&list_template),
        theme,
        "Picker",
        tab,
    ));
    rows.push(help_item_dyn(
        "Search templates by name",
        Some("Type in search bar"),
        theme,
        "Picker",
        tab,
    ));
    rows.push(help_item_dyn(
        "Switch search/results focus",
        Some("Tab"),
        theme,
        "Picker",
        tab,
    ));
    rows.push(help_item_dyn(
        "Open help from template picker",
        Some("?"),
        theme,
        "Picker",
        tab,
    ));
    rows.push(help_empty_row(tab));

    rows.push(help_heading_row("Files", theme, tab));
    rows.push(help_empty_row(tab));
    rows.push(help_item_dyn(
        "Templates directory",
        Some("~/.config/clin/templates/"),
        theme,
        "Files",
        tab,
    ));
    rows.push(help_item_dyn(
        "Default template filename",
        Some("default.toml"),
        theme,
        "Files",
        tab,
    ));
    rows.push(help_empty_row(tab));

    rows.push(help_heading_row("Variables", theme, tab));
    rows.push(help_empty_row(tab));
    rows.push(help_item_dyn(
        "Variable: current date",
        Some("{date}"),
        theme,
        "Variables",
        tab,
    ));
    rows.push(help_item_dyn(
        "Variable: date and time",
        Some("{datetime}"),
        theme,
        "Variables",
        tab,
    ));
    rows.push(help_item_dyn(
        "Variable: current time",
        Some("{time}"),
        theme,
        "Variables",
        tab,
    ));
    rows.push(help_item_dyn(
        "Variable: weekday name",
        Some("{weekday}"),
        theme,
        "Variables",
        tab,
    ));
    rows.push(help_item_dyn(
        "Variable: 4-digit year",
        Some("{year}"),
        theme,
        "Variables",
        tab,
    ));
    rows.push(help_item_dyn(
        "Variable: zero-padded month",
        Some("{month}"),
        theme,
        "Variables",
        tab,
    ));
    rows.push(help_item_dyn(
        "Variable: zero-padded day",
        Some("{day}"),
        theme,
        "Variables",
        tab,
    ));
    rows
}
fn about_help_text(
    _keybinds: &Keybinds,
    theme: &AppThemeColors,
    config: &crate::config::ClinConfig,
    tab: HelpTab,
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
        &format!("clin v{}", env!("CARGO_PKG_VERSION")),
        "About",
        tab,
    ));
    rows.push(help_empty_row(tab));
    rows.push(help_item_dyn(
        "Feature-packed terminal note management app",
        None,
        theme,
        "About",
        tab,
    ));
    rows.push(help_empty_row(tab));

    if config.counts_enabled() {
        rows.push(help_heading_row("Count Prefix", theme, tab));
        rows.push(help_empty_row(tab));
        rows.push(help_item_dyn(
            "Type a number before a motion key to repeat it N times (e.g. 3j, 11k, 5G)",
            None,
            theme,
            "Count Prefix",
            tab,
        ));
        rows.push(help_empty_row(tab));
    }

    rows.push(help_heading_row("Configuration", theme, tab));
    rows.push(help_empty_row(tab));
    rows.push(help_item_dyn(
        "Keybinds overlay: ~/.config/clin/keybinds_<preset>.toml",
        None,
        theme,
        "Configuration",
        tab,
    ));
    rows.push(help_item_dyn(
        "Theme + storage:  ~/.config/clin/config.toml",
        None,
        theme,
        "Configuration",
        tab,
    ));
    rows.push(help_item_dyn(
        "Templates dir: <storage>/templates/",
        None,
        theme,
        "Configuration",
        tab,
    ));
    rows.push(help_empty_row(tab));
    rows.push(help_heading_row("CLI Usage", theme, tab));
    rows.push(help_empty_row(tab));
    rows.push(about_cli_row("clin", "Launch interactive TUI", theme, tab));
    rows.push(about_cli_row(
        "clin --config <PATH>",
        "Override config file",
        theme,
        tab,
    ));
    rows.push(about_cli_row("clin help", "Show CLI help", theme, tab));
    rows.push(help_empty_row(tab));
    rows.push(about_cli_row(
        "clin notes list",
        "List note titles",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin notes new [TITLE]",
        "Create note + open TUI",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin notes open <TITLE>",
        "Open existing note",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin notes quick <text> [TITLE]",
        "Quick note without TUI",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin notes search <query>",
        "Search notes",
        theme,
        tab,
    ));
    rows.push(help_empty_row(tab));
    rows.push(about_cli_row(
        "clin storage show",
        "Show current storage path",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin storage set <PATH>",
        "Set storage directory",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin storage reset",
        "Reset to default storage",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin storage migrate",
        "Migrate data from old location",
        theme,
        tab,
    ));
    rows.push(help_empty_row(tab));
    rows.push(about_cli_row(
        "clin keybinds show",
        "Show current keybindings",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin keybinds export",
        "Export keybinds as TOML",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin keybinds reset",
        "Reset keybinds to defaults",
        theme,
        tab,
    ));
    rows.push(help_empty_row(tab));
    rows.push(about_cli_row(
        "clin templates list",
        "List available templates",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin templates init",
        "Create example templates",
        theme,
        tab,
    ));
    rows.push(help_empty_row(tab));
    rows.push(about_cli_row(
        "clin config show",
        "Print effective config as TOML",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin config path",
        "Print config file path",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin config edit",
        "Open config in $EDITOR",
        theme,
        tab,
    ));
    rows.push(about_cli_row(
        "clin config reset",
        "Reset config to defaults",
        theme,
        tab,
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

fn help_item_dyn(
    text: &str,
    key: Option<&str>,
    theme: &AppThemeColors,
    group: &'static str,
    tab: HelpTab,
) -> HelpRow {
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
                display: text.to_string(),
                group,
                tab,
            }
        }
        None => HelpRow {
            row: Row::new(vec![
                Cell::from(""),
                Cell::from(Line::from(vec![Span::raw(text.to_owned())])),
            ]),
            search_text,
            display: text.to_string(),
            group,
            tab,
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
