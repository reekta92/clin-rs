#![allow(clippy::vec_init_then_push)]
use ratatui::{prelude::*, widgets::*};

use super::{
    build_tab_spans, draw_dim_vline, draw_status_bar, draw_view_title_bar_with_tabs,
    format_keybind_hints,
};
use crate::app::{App, HelpTab, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::keybinds::help_meta::{self, HelpMeta};
use crate::keybinds::{HelpAction, Keybinds, ListAction};
use strum::IntoEnumIterator;

pub fn help_tab_names() -> [&'static str; 8] {
    [
        "Notes",
        "Editor",
        "Graph",
        "Draw",
        "Canvas",
        "Backup",
        "Templates",
        "About",
    ]
}
///
/// Help-view tab (label, glyph) pairs, in `HelpTab` order.
/// Mirrors `backup_tabs` / list grid tabs. Glyphs are (nerd_font, unicode).
pub fn help_tabs(icon_mode: crate::config::IconMode) -> Vec<(&'static str, Option<&'static str>)> {
    let pairs: [(&'static str, &'static str, &'static str); 8] = [
        ("Notes", "\u{f02d}", "\u{1f4d8}"),     // book
        ("Editor", "\u{f303}", "\u{270f}"),     // pencil
        ("Graph", "\u{f1e0}", "\u{1f5c2}"),     // share-alt / stacked
        ("Draw", "\u{f1fc}", "\u{1f3a8}"),      // paint-brush / palette
        ("Canvas", "\u{f0b2}", "\u{1f4cc}"),    // thumbtack / pushpin
        ("Backup", "\u{f1d3}", "\u{1f4be}"),    // git / floppy
        ("Templates", "\u{f15b}", "\u{1f4c4}"), // file / page
        ("About", "\u{f05a}", "\u{2139}"),      // info-circle
    ];
    pairs
        .iter()
        .map(|&(label, nerd, uni)| (label, Some(crate::ui::get_icon(nerd, uni, icon_mode))))
        .collect()
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
    let style = Style::default()
        .fg(theme.highlight_fg)
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let cell0 = Cell::from(Line::from(Span::styled(
        format!(" {} ", title.to_uppercase()),
        style,
    )));
    let cell1 = Cell::from(Line::from(Span::styled(" ", style)));
    HelpRow {
        row: Row::new(vec![cell0, cell1]).style(Style::default().bg(theme.highlight_bg)),
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

    let tabs: Vec<(&str, Option<&str>)> = help_tabs(app.config.ui.icon_mode);
    let hovered = app.mouse_pos.and_then(|(col, row)| {
        if row == chunks[0].y {
            let region = crate::ui::title_bar_tabs_region(chunks[0], "Help");
            crate::ui::hit_test_tabs(
                &tabs,
                chunks[0].x,
                chunks[0].width,
                region.x,
                col,
                app.config.ui.tab_icons_only,
                app.config.ui.icon_mode,
            )
        } else {
            None
        }
    });
    let tab_spans = build_tab_spans(
        &tabs,
        app.help_tab.index(),
        hovered,
        &app.app_theme,
        app.config.ui.tab_icons_only,
        app.config.ui.icon_mode,
    );
    let rows = app.get_help_rows();
    let theme = &app.app_theme;
    // Pagination: compute page from terminal height
    let page_size = chunks[1].height.saturating_sub(2).max(1);
    app.help_page_size = page_size;
    let total_pages = rows.len().div_ceil(page_size as usize);
    let page = (app.help_page as usize).min(total_pages.saturating_sub(1));
    app.help_page = page as u16;
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
        app.load_spinner_tick,
    );

    let body_area = chunks[1];
    let show_sides = body_area.width >= 100;
    let (left_area, divider1_area, center_area, divider2_area, right_area) = if show_sides {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Length(1),
                Constraint::Percentage(60),
                Constraint::Length(1),
                Constraint::Percentage(20),
            ])
            .split(body_area);
        (panes[0], panes[1], panes[2], panes[3], panes[4])
    } else {
        (Rect::ZERO, Rect::ZERO, body_area, Rect::ZERO, Rect::ZERO)
    };

    // --- center pane: existing table + pagination, now scoped to center_area ---
    let max_w: u16 = if show_sides { center_area.width } else { 96 };
    let content_w = center_area.width.min(max_w);
    let content_x = center_area.x + (center_area.width.saturating_sub(content_w)) / 2;
    let content_area = Rect::new(content_x, center_area.y, content_w, center_area.height);

    let table_h = content_area.height;
    let start_idx = page * page_size as usize;
    let table_area = Rect::new(content_area.x, content_area.y, content_area.width, table_h);
    let visible_rows: Vec<Row<'static>> = rows
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(page_size as usize)
        .map(|(abs_idx, hr)| {
            let mut row = hr.row.clone();
            if let Some(ref popup) = app.help_search.popup {
                let has_results = !popup.results.is_empty();
                if has_results {
                    let selected_row = popup.results.get(popup.selected).map(|(idx, _)| *idx);
                    let is_selected = Some(abs_idx) == selected_row;
                    let is_matched = popup.results.iter().any(|(idx, _)| *idx == abs_idx);
                    if is_selected {
                        row = row.style(
                            Style::default()
                                .bg(theme.highlight_fg)
                                .fg(theme.highlight_bg),
                        );
                    } else if is_matched {
                        row = row
                            .style(Style::default().bg(theme.preview_bg().unwrap_or(Color::Reset)));
                    }
                }
            } else if let Some(hl_idx) = app.help_search.highlight_row
                && abs_idx == hl_idx
            {
                row = row.style(
                    Style::default()
                        .bg(theme.highlight_fg)
                        .fg(theme.highlight_bg),
                );
            }
            row
        })
        .collect();
    frame.render_widget(Block::default().style(theme.bg_style()), body_area);
    let table = Table::new(visible_rows, [Constraint::Length(30), Constraint::Min(20)]).block(
        Block::default()
            .style(theme.bg_style())
            .borders(Borders::NONE)
            .padding(Padding::new(2, 2, 1, 1)),
    );
    frame.render_widget(table, table_area);

    if show_sides {
        draw_help_info_pane(
            frame,
            left_area,
            app.help_tab,
            &app.keybinds,
            app.help_info_active,
            theme,
        );
        draw_dim_vline(frame, divider1_area, theme.border);
        draw_dim_vline(frame, divider2_area, theme.border);
        draw_help_tips_pane(
            frame,
            right_area,
            &app.help_suggestions,
            &app.keybinds,
            &app.config,
            theme,
        )
    }

    let kb = &app.keybinds;
    let mut hints_items = vec![
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
        (kb.display_help(HelpAction::Reroll), "reroll tips"),
        (kb.help_keys_display(HelpAction::Close), "close"),
        ("F2".to_string(), "keybinds"),
    ];
    if !crate::ui::help_content::tab_popup_descriptions(app.help_tab).is_empty() {
        hints_items.push(("n/N".to_string(), "cycle popup"));
    }
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
    if let Some(ref popup) = app.help_search.popup {
        let theme = &app.app_theme;
        let max_visible = 10usize;
        crate::ui::quick_search::draw_quick_search(
            frame,
            content_area,
            popup,
            theme,
            max_visible,
            |(_, display): &(usize, String),
             is_selected,
             theme: &crate::app_theme::AppThemeColors| {
                let style = if is_selected {
                    Style::default().fg(theme.fg)
                } else {
                    Style::default().fg(theme.highlight_fg)
                };
                let prefix = if is_selected { "▸ " } else { "  " };
                Line::from(Span::styled(format!("{}{}", prefix, display), style))
            },
            app.config.ui.icon_mode,
        );
    }
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
fn draw_help_info_pane(
    frame: &mut Frame,
    area: Rect,
    tab: HelpTab,
    keybinds: &Keybinds,
    active: usize,
    theme: &AppThemeColors,
) {
    let title = tab_display_name(tab);
    let mut lines = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::default());

    // Description
    lines.push(Line::from(Span::styled(
        crate::ui::help_content::tab_description(tab).to_string(),
        Style::default().fg(theme.text),
    )));

    // Popup accordion (only for tabs that have them)
    let popups = crate::ui::help_content::tab_popup_descriptions(tab);
    if !popups.is_empty() {
        let active = active.min(popups.len() - 1);
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "Popups & Overlays",
            Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::default());
        // Name list — all names always visible; active marked and highlighted.
        for (i, p) in popups.iter().enumerate() {
            let is_active = i == active;
            let marker = if is_active { "▼ " } else { "› " };
            let style = if is_active {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            };
            lines.push(Line::from(Span::styled(
                format!("{marker}{}", p.name),
                style,
            )));
        }
        lines.push(Line::default());
        // Only the active popup's description renders.
        let p = &popups[active];
        lines.push(Line::from(render_tip_body(p.body, keybinds, theme)));
    }

    let block = Block::default()
        .borders(Borders::NONE)
        .style(theme.preview_bg_style())
        .padding(Padding::new(2, 2, 1, 1));
    let body = Paragraph::new(lines)
        .wrap(Wrap::default())
        .style(theme.preview_bg_style())
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(body, area);
}

fn tab_display_name(tab: HelpTab) -> &'static str {
    match tab {
        HelpTab::Notes => "Notes",
        HelpTab::Editor => "Editor",
        HelpTab::Graph => "Graph",
        HelpTab::Draw => "Draw",
        HelpTab::Canvas => "Canvas",
        HelpTab::Backup => "Backup",
        HelpTab::Templates => "Templates",
        HelpTab::About => "About",
    }
}

fn draw_help_tips_pane(
    frame: &mut Frame,
    area: Rect,
    suggestions: &[crate::ui::HelpSuggestion],
    keybinds: &Keybinds,
    config: &crate::config::ClinConfig,
    theme: &AppThemeColors,
) {
    let block = Block::default()
        .borders(Borders::NONE)
        .style(theme.preview_bg_style())
        .padding(Padding::new(2, 2, 1, 1));

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Tips",
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::default());

    if suggestions.is_empty() {
        lines.push(Line::styled(
            "No suggestions",
            Style::default().fg(theme.muted),
        ));
    } else {
        for (i, s) in suggestions.iter().enumerate() {
            if i > 0 {
                lines.push(Line::default()); // blank separator between tips
            }
            lines.push(Line::from(Span::styled(
                s.title.to_string(),
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )));
            let parsed_spans = render_tip_body(s.body, keybinds, theme);
            lines.push(Line::from(parsed_spans));
            if let Some(note) = s.requires.caveat_if_unsatisfied(config) {
                lines.push(Line::from(Span::styled(
                    format!("  \u{26a0} {note}"),
                    Style::default().fg(theme.warning),
                )));
            }
        }
    }
    // Fixed tip: F2 keybinds overlay (always shown)
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "Quick keybinds",
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(render_tip_body(
        "Press `` F2 `` in any view to toggle a **keybinds overlay** showing all available shortcuts for the current context.",
        keybinds,
        theme,
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap::default())
        .style(theme.preview_bg_style())
        .block(block);
    frame.render_widget(p, area);
}

fn render_tip_body(body: &str, kb: &Keybinds, theme: &AppThemeColors) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = body;

    while !remaining.is_empty() {
        let next_brace = remaining.find('{');
        let next_dbl_backtick = remaining.find("``");
        let next_star = remaining.find("**");

        let mut earliest = None;
        let mut token_type = "";
        let mut token_len = 0;

        if let Some(idx) = next_brace {
            earliest = Some(idx);
            token_type = "brace";
            token_len = 1;
        }
        if let Some(idx) = next_dbl_backtick
            && earliest.is_none_or(|e| idx < e)
        {
            earliest = Some(idx);
            token_type = "backtick";
            token_len = 2;
        }
        if let Some(idx) = next_star
            && earliest.is_none_or(|e| idx < e)
        {
            earliest = Some(idx);
            token_type = "star";
            token_len = 2;
        }

        if let Some(idx) = earliest {
            if idx > 0 {
                spans.push(Span::styled(
                    remaining[..idx].to_string(),
                    Style::default().fg(theme.text),
                ));
            }

            remaining = &remaining[idx + token_len..];

            match token_type {
                "brace" => {
                    if let Some(end_idx) = remaining.find('}') {
                        let token = &remaining[..end_idx];
                        let keybind_display = resolve_tip_key(token, kb);
                        spans.push(Span::styled(
                            keybind_display,
                            Style::default()
                                .fg(theme.accent)
                                .add_modifier(Modifier::BOLD),
                        ));
                        remaining = &remaining[end_idx + 1..];
                    } else {
                        spans.push(Span::styled(
                            format!("{{{remaining}"),
                            Style::default().fg(theme.text),
                        ));
                        break;
                    }
                }
                "backtick" => {
                    if let Some(end_idx) = remaining.find("``") {
                        let literal = &remaining[..end_idx];
                        spans.push(Span::styled(
                            literal.to_string(),
                            Style::default()
                                .fg(theme.accent)
                                .add_modifier(Modifier::BOLD),
                        ));
                        remaining = &remaining[end_idx + 2..];
                    } else {
                        spans.push(Span::styled(
                            format!("``{remaining}"),
                            Style::default().fg(theme.text),
                        ));
                        break;
                    }
                }
                "star" => {
                    if let Some(end_idx) = remaining.find("**") {
                        let emphasis = &remaining[..end_idx];
                        spans.push(Span::styled(
                            emphasis.to_string(),
                            Style::default()
                                .fg(theme.heading)
                                .add_modifier(Modifier::BOLD),
                        ));
                        remaining = &remaining[end_idx + 2..];
                    } else {
                        spans.push(Span::styled(
                            format!("**{remaining}"),
                            Style::default().fg(theme.text),
                        ));
                        break;
                    }
                }
                _ => unreachable!(),
            }
        } else {
            spans.push(Span::styled(
                remaining.to_string(),
                Style::default().fg(theme.text),
            ));
            break;
        }
    }

    spans
}

pub(crate) fn resolve_tip_key(token: &str, kb: &Keybinds) -> String {
    use crate::keybinds::*;

    let Some((scope, action)) = token.split_once(':') else {
        return format!("[ERR:{}]", token);
    };

    match scope {
        "list" => match action {
            "ToggleSelectMode" => kb.list_keys_display(ListAction::ToggleSelectMode),
            "ToggleSelectItem" => kb.list_keys_display(ListAction::ToggleSelectItem),
            "Search" => kb.list_keys_display(ListAction::Search),
            "NewFromTemplate" => kb.list_keys_display(ListAction::NewFromTemplate),
            "CreateFolder" => kb.list_keys_display(ListAction::CreateFolder),
            "CreateNote" => kb.list_keys_display(ListAction::CreateNote),
            "ExpandToLevel" => kb.list_keys_display(ListAction::ExpandToLevel),
            "ManageSubnotes" => kb.list_keys_display(ListAction::ManageSubnotes),
            "TogglePin" => kb.list_keys_display(ListAction::TogglePin),
            "CycleSort" => kb.list_keys_display(ListAction::CycleSort),
            "TogglePreview" => kb.list_keys_display(ListAction::TogglePreview),
            "OpenCommandPalette" => kb.list_keys_display(ListAction::OpenCommandPalette),
            "ToggleFoldersFirst" => kb.list_keys_display(ListAction::ToggleFoldersFirst),
            "Delete" => kb.list_keys_display(ListAction::Delete),
            "ToggleExternalEditor" => kb.list_keys_display(ListAction::ToggleExternalEditor),
            "OpenGraph" => kb.list_keys_display(ListAction::OpenGraph),
            "OpenCanvas" => kb.list_keys_display(ListAction::OpenCanvas),
            "ManageTags" => kb.list_keys_display(ListAction::ManageTags),
            "OpenTrash" => kb.list_keys_display(ListAction::OpenTrash),
            _ => format!("[ERR:{}]", token),
        },
        "edit" => match action {
            "InsertTab" => kb.edit_keys_display(EditAction::InsertTab),
            "ToggleMarkdownPreview" => kb.edit_keys_display(EditAction::ToggleMarkdownPreview),
            "TogglePreviewFullscreen" => kb.edit_keys_display(EditAction::TogglePreviewFullscreen),
            "Undo" => kb.edit_keys_display(EditAction::Undo),
            "Redo" => kb.edit_keys_display(EditAction::Redo),
            "DeleteWord" => kb.edit_keys_display(EditAction::DeleteWord),
            "DeleteNextWord" => kb.edit_keys_display(EditAction::DeleteNextWord),
            "CycleFocus" => kb.edit_keys_display(EditAction::CycleFocus),
            "Back" => kb.edit_keys_display(EditAction::Back),
            "ManageSubnotes" => kb.edit_keys_display(EditAction::ManageSubnotes),
            "Find" => kb.edit_keys_display(EditAction::Find),
            "InsertDate" => kb.edit_keys_display(EditAction::InsertDate),
            "ToggleWrap" => kb.edit_keys_display(EditAction::ToggleWrap),
            "ToggleOutline" => kb.edit_keys_display(EditAction::ToggleOutline),
            "ToggleLinks" => kb.edit_keys_display(EditAction::ToggleLinks),
            "PreviewLink" => kb.edit_keys_display(EditAction::PreviewLink),
            _ => format!("[ERR:{}]", token),
        },
        "help" => match action {
            "Reroll" => kb.help_keys_display(HelpAction::Reroll),
            "Search" => kb.help_keys_display(HelpAction::Search),
            "Close" => kb.help_keys_display(HelpAction::Close),
            "NextTab" => kb.help_keys_display(HelpAction::NextTab),
            "PrevTab" => kb.help_keys_display(HelpAction::PrevTab),
            _ => format!("[ERR:{}]", token),
        },
        "graph" => match action {
            "AutoFit" => kb.graph_keys_display(GraphAction::AutoFit),
            "ToggleSearch" => kb.graph_keys_display(GraphAction::ToggleSearch),
            "ToggleMinimap" => kb.graph_keys_display(GraphAction::ToggleMinimap),
            "ToggleLegend" => kb.graph_keys_display(GraphAction::ToggleLegend),
            "ToggleGrid" => kb.graph_keys_display(GraphAction::ToggleGrid),
            "ReloadConfig" => kb.graph_keys_display(GraphAction::ReloadConfig),
            "OpenNote" => kb.graph_keys_display(GraphAction::OpenNote),
            "ZoomIn" => kb.graph_keys_display(GraphAction::ZoomIn),
            "ZoomOut" => kb.graph_keys_display(GraphAction::ZoomOut),
            "ToggleStatus" => kb.graph_keys_display(GraphAction::ToggleStatus),
            "TogglePreview" => kb.graph_keys_display(GraphAction::TogglePreview),
            "Refresh" => kb.graph_keys_display(GraphAction::Refresh),
            "Help" => kb.graph_keys_display(GraphAction::Help),
            "Quit" => kb.graph_keys_display(GraphAction::Quit),
            _ => format!("[ERR:{}]", token),
        },
        "draw" => match action {
            "ToggleShapeSelector" => kb.draw_keys_display(DrawAction::ToggleShapeSelector),
            "SelectDrawTool" => kb.draw_keys_display(DrawAction::SelectDrawTool),
            "SelectTextTool" => kb.draw_keys_display(DrawAction::SelectTextTool),
            "SelectEraseTool" => kb.draw_keys_display(DrawAction::SelectEraseTool),
            "ToggleGrid" => kb.draw_keys_display(DrawAction::ToggleGrid),
            _ => format!("[ERR:{}]", token),
        },
        "canvas" => match action {
            "OpenContextMenu" => kb.canvas_keys_display(CanvasAction::OpenContextMenu),
            "EditOrConnect" => kb.canvas_keys_display(CanvasAction::EditOrConnect),
            "ToggleGrid" => kb.canvas_keys_display(CanvasAction::ToggleGrid),
            "ToggleEditorPane" => kb.canvas_keys_display(CanvasAction::ToggleEditorPane),
            "ZoomIn" => kb.canvas_keys_display(CanvasAction::ZoomIn),
            "ZoomOut" => kb.canvas_keys_display(CanvasAction::ZoomOut),
            "ZoomFineIn" => kb.canvas_keys_display(CanvasAction::ZoomFineIn),
            "ZoomFineOut" => kb.canvas_keys_display(CanvasAction::ZoomFineOut),
            "Quit" => kb.canvas_keys_display(CanvasAction::Quit),
            "CycleFocus" => kb.canvas_keys_display(CanvasAction::CycleFocus),
            "Save" => kb.canvas_keys_display(CanvasAction::Save),
            "MenuClose" => kb.canvas_keys_display(CanvasAction::MenuClose),
            "EditorUnfocus" => kb.canvas_keys_display(CanvasAction::EditorUnfocus),
            "EditorSyncRaw" => kb.canvas_keys_display(CanvasAction::EditorSyncRaw),
            _ => format!("[ERR:{}]", token),
        },
        "backup" => match action {
            "StageFile" => kb.backup_keys_display(BackupAction::StageFile),
            "UnstageFile" => kb.backup_keys_display(BackupAction::UnstageFile),
            "StageAll" => kb.backup_keys_display(BackupAction::StageAll),
            "EnterCommit" => kb.backup_keys_display(BackupAction::EnterCommit),
            "ConfirmCommit" => kb.backup_keys_display(BackupAction::ConfirmCommit),
            "CancelCommit" => kb.backup_keys_display(BackupAction::CancelCommit),
            "Push" => kb.backup_keys_display(BackupAction::Push),
            "Pull" => kb.backup_keys_display(BackupAction::Pull),
            "Refresh" => kb.backup_keys_display(BackupAction::Refresh),
            "CycleSection" => kb.backup_keys_display(BackupAction::CycleSection),
            "OpenSettings" => kb.backup_keys_display(BackupAction::OpenSettings),
            "CloseSettings" => kb.backup_keys_display(BackupAction::CloseSettings),
            "NextField" => kb.backup_keys_display(BackupAction::NextField),
            "PrevField" => kb.backup_keys_display(BackupAction::PrevField),
            _ => format!("[ERR:{}]", token),
        },
        "outline" => match action {
            "Open" => kb.outline_keys_display(OutlineAction::Open),
            "MoveUp" => kb.outline_keys_display(OutlineAction::MoveUp),
            "MoveDown" => kb.outline_keys_display(OutlineAction::MoveDown),
            "ToggleCollapse" => kb.outline_keys_display(OutlineAction::ToggleCollapse),
            "ExpandAll" => kb.outline_keys_display(OutlineAction::ExpandAll),
            "CollapseAll" => kb.outline_keys_display(OutlineAction::CollapseAll),
            "Back" => kb.outline_keys_display(OutlineAction::Back),
            "Help" => kb.outline_keys_display(OutlineAction::Help),
            _ => format!("[ERR:{}]", token),
        },
        _ => format!("[ERR:{}]", token),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_tip_body_parsing() {
        let kb = crate::keybinds::Keybinds::default();
        let theme = crate::app_theme::AppThemeColors::default();

        // Test plain text with no markup
        let spans = render_tip_body("Hello world", &kb, &theme);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].content.to_string().contains("Hello world"));

        // Test keybind resolution (list:ToggleSelectMode should resolve to a non-ERR string)
        let spans = render_tip_body("Press {list:ToggleSelectMode} to toggle", &kb, &theme);
        assert_eq!(
            spans.len(),
            3,
            "plain / keybind / plain should yield 3 spans"
        );
        assert!(spans[0].content.to_string().contains("Press "));
        // Middle span is the resolved keybind (accent colored)
        let key_text = spans[1].content.to_string();
        assert!(
            !key_text.contains("[ERR:"),
            "keybind should resolve successfully, got: {key_text}"
        );
        assert!(!key_text.is_empty(), "resolved keybind should not be empty");

        // Test double-backtick literal
        let spans = render_tip_body("Use ``Tab`` to switch", &kb, &theme);
        assert_eq!(spans.len(), 3, "plain / literal / plain");
        assert_eq!(spans[1].content.to_string(), "Tab");

        // Test bold emphasis
        let spans = render_tip_body("This is **important** text", &kb, &theme);
        assert_eq!(spans.len(), 3, "plain / bold / plain");
        assert_eq!(spans[1].content.to_string(), "important");

        // Test all three markup types in one body
        let body = "Press {list:CreateNote} for a **new note** in ``Notes``";
        let spans = render_tip_body(body, &kb, &theme);
        assert!(
            spans.len() >= 5,
            "multiple markup types should produce multiple spans"
        );

        // Verify all resolved tokens are present (not ERR)
        for span in &spans {
            assert!(
                !span.content.to_string().contains("[ERR:"),
                "no ERR in any span"
            );
        }
    }

    #[test]
    fn test_popup_key_resolution() {
        let kb = crate::keybinds::Keybinds::default();
        let open_keys = [
            "list:ManageTags",
            "list:ManageSubnotes",
            "list:OpenCommandPalette",
            "list:Search",
            "list:OpenTrash",
            "edit:ManageSubnotes",
            "outline:MoveUp",
            "outline:MoveDown",
            "outline:ToggleCollapse",
            "outline:ExpandAll",
            "outline:CollapseAll",
            "outline:Open",
            "outline:Back",
            "outline:Help",
        ];
        for &token in &open_keys {
            let resolved = crate::ui::help::resolve_tip_key(token, &kb);
            assert!(
                !resolved.starts_with("[ERR:"),
                "Token '{token}' should resolve without error, got: {resolved}"
            );
            assert!(
                !resolved.is_empty(),
                "Token '{token}' should resolve to a non-empty string"
            );
        }
    }
}
