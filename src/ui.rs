use crate::app::{App, ConfirmPopup, EditFocus, HelpTab, ListFocus, TemplatePopup, ThemePopup, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::constants::*;
use crate::list_view::PreviewContent;
use crate::events::get_title_text;
use crate::keybinds::*;
use anyhow::{Context, Result};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::*;
use std::borrow::Cow;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn draw_ui(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    if let Some(_bg) = app.app_theme.bg {
        let block = Block::default().style(app.app_theme.bg_style());
        frame.render_widget(block, frame.area());
    }

    match app.mode {
        ViewMode::List => draw_list_view(frame, app),
        ViewMode::Edit => draw_edit_view(frame, app, focus),
        ViewMode::Help => draw_help_view(frame, app),
        ViewMode::Graph => {}
        ViewMode::Draw => {}
        ViewMode::Canvas => {}
    }

    if let Some(popup) = &app.popups.theme {
        draw_theme_popup(frame, popup, frame.area(), &app.app_theme);
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

    let tab_names = ["Notes", "Editor", "Graph", "Draw", "Canvas", "About"];
    let mut tab_spans: Vec<Span<'static>> = Vec::new();
    for (i, name) in tab_names.iter().enumerate() {
        let tab = HelpTab::from_index(i);
        if tab == app.help_tab {
            tab_spans.push(Span::styled(
                format!(" {} ", name),
                Style::default()
                    .fg(app.app_theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tab_spans.push(Span::styled(
                format!(" {} ", name),
                Style::default().fg(app.app_theme.muted),
            ));
        }
        if i < tab_names.len() - 1 {
            tab_spans.push(Span::styled(
                " · ",
                Style::default().fg(app.app_theme.muted),
            ));
        }
    }
    let tab_line = Line::from(tab_spans);
    let tab_bar = Paragraph::new(tab_line)
        .style(app.app_theme.title_bar_bg_style())
        .alignment(Alignment::Center);
    frame.render_widget(tab_bar, chunks[0]);

    let help_text = app.get_help_text().clone();
    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(2, 2, 1, 1)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0));
    frame.render_widget(help, chunks[1]);

    let hint = Paragraph::new(Span::styled(
        HELP_PAGE_HINTS,
        Style::default().fg(app.app_theme.muted),
    )).style(app.app_theme.hint_line_bg_style());
    frame.render_widget(hint, chunks[2]);
}

pub fn help_text_for_tab(
    tab: crate::app::HelpTab,
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    match tab {
        crate::app::HelpTab::Notes => notes_help_text(keybinds, theme),
        crate::app::HelpTab::Editor => editor_help_text(keybinds, theme),
        crate::app::HelpTab::Graph => graph_help_text(keybinds, theme),
        crate::app::HelpTab::Draw => draw_help_text(theme),
        crate::app::HelpTab::Canvas => canvas_help_text(theme),
        crate::app::HelpTab::About => about_help_text(keybinds, theme),
    }
}

fn notes_help_text(
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let list_move = format!(
        "{}/{}",
        keybinds.list_keys_display(ListAction::MoveUp),
        keybinds.list_keys_display(ListAction::MoveDown)
    );
    let list_expand_collapse = format!(
        "{}/{}",
        keybinds.list_keys_display(ListAction::ExpandFolder),
        keybinds.list_keys_display(ListAction::CollapseFolder)
    );
    let list_open = keybinds.list_keys_display(ListAction::Open);
    let list_delete = keybinds.list_keys_display(ListAction::Delete);
    let list_location = keybinds.list_keys_display(ListAction::OpenLocation);
    let list_focus = keybinds.list_keys_display(ListAction::CycleFocus);
    let list_help = keybinds.list_keys_display(ListAction::Help);
    let list_quit = keybinds.list_keys_display(ListAction::Quit);
    let list_template = keybinds.list_keys_display(ListAction::NewFromTemplate);
    let list_create_folder = keybinds.list_keys_display(ListAction::CreateFolder);
    let list_rename_folder = keybinds.list_keys_display(ListAction::RenameFolder);
    let list_move_note = keybinds.list_keys_display(ListAction::MoveNote);
    let list_manage_tags = keybinds.list_keys_display(ListAction::ManageTags);
    let list_filter_tags = keybinds.list_keys_display(ListAction::FilterTags);

    let mut lines = Vec::new();
    lines.push(help_heading("Notes View", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Move selection", Some(&list_move), theme));
    lines.extend(help_item_dyn(
        "Expand/Collapse folder",
        Some(&list_expand_collapse),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Open selected folder, note, or create new",
        Some(&list_open),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Create new folder",
        Some(&list_create_folder),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Rename folder",
        Some(&list_rename_folder),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Move note or folder",
        Some(&list_move_note),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Manage note tags",
        Some(&list_manage_tags),
        theme,
    ));
    lines.extend(help_item_dyn("Filter tags", Some(&list_filter_tags), theme));
    lines.extend(help_item_dyn(
        "Delete note or folder",
        Some(&list_delete),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Confirm / cancel delete",
        Some("y/Enter / n/Esc"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Open selected note file location",
        Some(&list_location),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Change focus (notes list <-> buttons)",
        Some(&list_focus),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle Encryption from focused button",
        Some("Enter/Space"),
        theme,
    ));
    lines.extend(help_item_dyn("Open help", Some(&list_help), theme));
    lines.extend(help_item_dyn("Quit app", Some(&list_quit), theme));
    lines.extend(help_item_dyn(
        "New note from template",
        Some(&list_template),
        theme,
    ));
    Text::from(lines)
}

fn editor_help_text(
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let edit_quit = keybinds.edit_keys_display(EditAction::Quit);
    let edit_back = keybinds.edit_keys_display(EditAction::Back);
    let edit_focus = keybinds.edit_keys_display(EditAction::CycleFocus);
    let edit_copy = keybinds.edit_keys_display(EditAction::Copy);
    let edit_cut = keybinds.edit_keys_display(EditAction::Cut);
    let edit_paste = keybinds.edit_keys_display(EditAction::Paste);
    let edit_select_all = keybinds.edit_keys_display(EditAction::SelectAll);
    let edit_undo = keybinds.edit_keys_display(EditAction::Undo);
    let edit_redo = keybinds.edit_keys_display(EditAction::Redo);
    let edit_del_word = keybinds.edit_keys_display(EditAction::DeleteWord);
    let edit_del_next_word = keybinds.edit_keys_display(EditAction::DeleteNextWord);
    let edit_md_preview = keybinds.edit_keys_display(EditAction::ToggleMarkdownPreview);

    let mut lines = Vec::new();
    lines.push(help_heading("Editor", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Change focus (Title, Content, toggles)",
        Some(&edit_focus),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Return to notes (auto-saved on exit)",
        Some(&edit_back),
        theme,
    ));
    lines.extend(help_item_dyn("Save + quit app entirely", Some(&edit_quit), theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Copy / Cut / Paste",
        Some(&format!("{edit_copy} / {edit_cut} / {edit_paste}")),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Select all / Undo / Redo",
        Some(&format!("{edit_select_all} / {edit_undo} / {edit_redo}")),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Delete prev/next word",
        Some(&format!("{edit_del_word} / {edit_del_next_word}")),
        theme,
    ));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Toggle markdown preview panel",
        Some(&edit_md_preview),
        theme,
    ));
    Text::from(lines)
}

fn graph_help_text(
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let mut lines = Vec::new();

    lines.push(help_heading("Keyboard Controls", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Navigate nodes (up/down/left/right)",
        Some(&format!(
            "{}/{}/{}/{}",
            keybinds.graph_keys_display(crate::keybinds::GraphAction::PanUp),
            keybinds.graph_keys_display(crate::keybinds::GraphAction::PanDown),
            keybinds.graph_keys_display(crate::keybinds::GraphAction::PanLeft),
            keybinds.graph_keys_display(crate::keybinds::GraphAction::PanRight)
        )),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Zoom in/out",
        Some(&format!(
            "{}/{}",
            keybinds.graph_keys_display(crate::keybinds::GraphAction::ZoomIn),
            keybinds.graph_keys_display(crate::keybinds::GraphAction::ZoomOut)
        )),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Open selected note",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::OpenNote)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Auto-fit graph to viewport",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::AutoFit)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Search nodes by title",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::ToggleSearch)),
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Display Options", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Toggle minimap",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::ToggleMinimap)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle legend (node colors, link types)",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::ToggleLegend)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle background grid",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::ToggleGrid)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle status bar",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::ToggleStatus)),
        theme,
    ));
    lines.push(Line::from(""));

    lines.extend(help_item_dyn(
        "Refresh physics simulation",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::Refresh)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Reload graf config file",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::ReloadConfig)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Quit graph view",
        Some(&keybinds.graph_keys_display(crate::keybinds::GraphAction::Quit)),
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Mouse Controls", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Scroll wheel to zoom in/out", None, theme));
    lines.extend(help_item_dyn("Click and drag background to pan", None, theme));
    lines.extend(help_item_dyn("Click node to select", None, theme));
    lines.extend(help_item_dyn("Double-click node to open note", None, theme));
    Text::from(lines)
}

fn draw_help_text(
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(help_heading("Tools", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Draw freehand strokes",
        Some("d"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Shape tool (opens picker)",
        Some("s"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Place text label at click position",
        Some("t"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Erase elements (hover + click/drag)",
        Some("e"),
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Shapes", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Press s, then pick type in popup (Up/Down)",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Shape types: Rect, Ellipse, Diamond, Line, Arrow",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Click + drag to place shape at desired size",
        None,
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Text Editing", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Right-click on existing text to edit content",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Edit line: Enter to confirm, Esc to cancel",
        None,
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Navigation", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Scroll wheel to zoom in/out", None, theme));
    lines.extend(help_item_dyn("Right-click or middle-click drag to pan", None, theme));
    lines.extend(help_item_dyn("Select tool from toolbar at bottom", None, theme));
    lines.push(Line::from(""));

    lines.push(help_heading("General", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Auto-saved on changes & quit", None, theme));
    lines.extend(help_item_dyn("Exit canvas view", Some("Esc"), theme));
    Text::from(lines)
}

fn canvas_help_text(
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(help_heading("Navigation", theme));
    lines.extend(help_item_dyn("Move selection", Some("←/→/↑/↓ / h/l/k/j"), theme));
    lines.extend(help_item_dyn("Zoom in", Some("+/="), theme));
    lines.extend(help_item_dyn("Zoom out", Some("-/_"), theme));
    lines.extend(help_item_dyn("Zoom in (fine)", Some("Ctrl+j"), theme));
    lines.extend(help_item_dyn("Zoom out (fine)", Some("Ctrl+k"), theme));
    lines.push(Line::from(""));
    lines.push(help_heading("Editing", theme));
    lines.extend(help_item_dyn("Open / edit selected node", Some("i / Enter"), theme));
    lines.extend(help_item_dyn("Connect two nodes", Some("i / Enter"), theme));
    lines.extend(help_item_dyn("Context menu", Some("a"), theme));
    lines.push(Line::from(""));
    lines.push(help_heading("Interface", theme));
    lines.extend(help_item_dyn("Toggle grid", Some("Ctrl+g"), theme));
    lines.extend(help_item_dyn("Toggle editor pane", Some("Ctrl+e"), theme));
    lines.extend(help_item_dyn("Focus editor / ext toggle", Some("Tab"), theme));
    lines.extend(help_item_dyn("Toggle external editor mode", Some("Space"), theme));
    lines.push(Line::from(""));
    lines.push(help_heading("Editor (focused)", theme));
    lines.extend(help_item_dyn("Exit editor focus", Some("Esc / Tab"), theme));
    lines.extend(help_item_dyn("Save raw editor changes", Some("Ctrl+s"), theme));
    lines.push(Line::from(""));
    lines.push(help_heading("General", theme));
    lines.extend(help_item_dyn("Save canvas file", Some("Ctrl+s"), theme));
    lines.extend(help_item_dyn("Cancel connection", Some("Esc"), theme));
    lines.extend(help_item_dyn("Exit canvas view", Some("Esc"), theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("* Ctrl+Enter in editor to save", None, theme));
    lines.extend(help_item_dyn("* Nodes auto-saved on changes", None, theme));
    Text::from(lines)
}

fn about_help_text(
    _keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
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
    ]));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Encrypted terminal note-taking app",
        None,
        theme,
    ));

    lines.push(Line::from(""));

    lines.push(help_heading("Configuration", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Keybinds file: ~/.config/clin/keybinds.toml",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Theme + storage:  ~/.config/clin/config.toml",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Templates dir: <storage>/templates/",
        None,
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("CLI Usage", theme));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  clin", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("                         Launch interactive TUI"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin -n [TITLE]", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("                Create note + open editor"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin -q <text> [TITLE]", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("        Quick note without TUI"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin -e <TITLE>", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("                Open existing note by title"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin -l", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("                          List all note titles"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin -h, --help", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("                 Show CLI help message"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --storage-path", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("           Show current storage path"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --set-storage-path <PATH>", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("  Set storage directory"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --reset-storage-path", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("       Reset to default storage"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --migrate-storage", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("         Migrate data from old location"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --keybinds", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("                Show current keybindings"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --export-keybinds", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("         Export keybinds as TOML"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --reset-keybinds", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("          Reset keybinds to defaults"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --list-templates", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("          List available templates"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  clin --create-example-templates", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        Span::raw("  Create example template files"),
    ]));

    Text::from(lines)
}

pub fn help_heading(
    title: &'static str,
    theme: &crate::app_theme::AppThemeColors,
) -> Line<'static> {
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
                .map(|k| format!("<{}>", k))
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    parts.join(" / ")
}

pub fn help_item_dyn(
    text: &str,
    key: Option<&str>,
    theme: &crate::app_theme::AppThemeColors,
) -> Vec<Line<'static>> {
    match key {
        Some(key) => {
            let formatted_key = format_keybind(key);
            vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        formatted_key,
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("    • ", Style::default().fg(theme.muted)),
                    Span::raw(text.to_owned()),
                ]),
            ]
        }
        None => vec![Line::from(vec![
            Span::styled("  • ", Style::default().fg(theme.muted)),
            Span::raw(text.to_owned()),
        ])],
    }
}

pub fn draw_list_view(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    let (list_area, preview_area) = if app.list.preview_enabled {
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);
        let list_area = Rect::new(full_cols[0].x, full_cols[0].y, full_cols[0].width, chunks[0].height);
        let preview_area = Some(Rect::new(full_cols[2].x, full_cols[2].y, full_cols[2].width, chunks[0].height));
        (list_area, preview_area)
    } else {
        (chunks[0], None)
    };

    let mut items: Vec<ListItem> = Vec::with_capacity(app.list.visual_list.len());

    for (vi, item) in app.list.visual_list.iter().enumerate() {
        match item {
            crate::app::VisualItem::Folder {
                path: _,
                name,
                depth,
                is_expanded,
                note_count,
            } => {
                let indent = "  ".repeat(*depth);
                let icon = if *is_expanded { " " } else { " " };
                let sanitized_name = crate::sanitize::sanitize_for_terminal(name);
                let text = format!("{indent}{icon} {sanitized_name} ({note_count})");
                items.push(ListItem::new(Line::from(vec![Span::styled(
                    text,
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(app.app_theme.folder),
                )])));
            }
            crate::app::VisualItem::Note {
                summary_idx,
                depth,
                is_clin,
                is_draw,
                is_canvas,
                ..
            } => {
                let summary = &app.notes[*summary_idx];
                let indent = "  ".repeat(*depth);

                let when = format_relative_time(summary.updated_at);
                let mut text_style = Style::default();

                let mut spans = Vec::new();
                spans.push(Span::raw(indent));
                spans.push(Span::raw("  "));

                if summary.pinned {
                    spans.push(Span::styled(
                        "\u{f4cc} ",
                        Style::default()
                            .fg(app.app_theme.heading)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                if *is_clin {
                    text_style = text_style.fg(app.app_theme.muted);
                    spans.push(Span::styled(
                        "\u{f023} ",
                        Style::default()
                            .fg(app.app_theme.destructive)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                if *is_draw {
                    spans.push(Span::styled(
                        "\u{f1fc} ",
                        Style::default()
                            .fg(app.app_theme.success)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                if *is_canvas {
                    spans.push(Span::styled(
                        "\u{f005} ",
                        Style::default()
                            .fg(app.app_theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                let sanitized_title =
                    crate::sanitize::sanitize_for_terminal(summary.title.as_str());
                spans.push(Span::styled(sanitized_title, text_style));

                for tag in &summary.tags {
                    spans.push(Span::raw(" "));
                    let sanitized_tag = crate::sanitize::sanitize_for_terminal(tag);
                    spans.push(Span::styled(
                        format!("[{}]", sanitized_tag),
                        Style::default().fg(app.app_theme.tag),
                    ));
                }

                if vi == app.list.visual_index {
                    spans.push(Span::styled(
                        format!("  ({when})"),
                        Style::default().fg(app.app_theme.muted),
                    ));
                }
                items.push(ListItem::new(Line::from(spans)));
            }
            crate::app::VisualItem::CreateNew { depth, .. } => {
                let indent = "  ".repeat(*depth);
                let text = format!("{indent}  Create new note");
                items.push(ListItem::new(Line::from(vec![Span::styled(
                    text,
                    Style::default().fg(app.app_theme.success),
                )])));
            }
        }
    }

    let list = List::new(items)
        .block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(2, 2, 1, 1)),
        )
        .highlight_style(
            Style::default()
                .fg(app.app_theme.highlight_fg)
                .bg(app.app_theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    app.list.list_state.select(Some(app.list.visual_index));
    frame.render_stateful_widget(list, list_area, &mut app.list.list_state);

    if let Some(preview_rect) = preview_area {
        match &app.list.preview_content {
            Some(PreviewContent::Markdown(renderer)) if !renderer.is_pending() => {
                let widget = crate::markdown::ScrollablePseudoTerminal::new(renderer.screen())
                    .scroll_offset(renderer.scroll_offset())
                    .theme_bg(app.app_theme.preview_bg())
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(widget, preview_rect);
            }
            Some(PreviewContent::Markdown(_)) => {
                let loading = Paragraph::new("Rendering preview...")
                    .style(Style::default().fg(app.app_theme.muted))
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(loading, preview_rect);
            }
            Some(PreviewContent::CanvasGrid(grid) | PreviewContent::DrawGrid(grid)) => {
                let snapshot = crate::snapshot::RenderedSnapshot::new(grid)
                    .scroll_offset(app.list.snapshot_scroll_offset)
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(snapshot, preview_rect);
            }
            None => {
                let placeholder = Paragraph::new("Select a note to preview")
                    .style(app.app_theme.preview_bg_style())
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(placeholder, preview_rect);
            }
        }
    }

    draw_hint_line(frame, chunks[1], app, LIST_HELP_HINTS, app.list.list_focus == ListFocus::ExternalEditorToggle, true);
    draw_corner_watermark(frame, chunks[1], app.app_theme.muted);
    if app.list.preview_enabled {
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);
        draw_dim_vline(frame, full_cols[1], app.app_theme.muted);
    }

    if let Some(popup) = &app.popups.template {
        draw_template_popup(frame, popup, area, &app.app_theme);
    }

    if let Some(popup) = &mut app.popups.folder {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(popup) = &mut app.popups.tag {
        let popup_area = centered_rect(60, 40, area);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Min(5),
            ])
            .split(popup_area);

        let input_block = Block::default()
            .style(app.app_theme.bg_style())
            .borders(Borders::ALL)
            .title("Manage Tags (comma separated) - Tab: autocomplete, Enter: save, Esc: cancel");
        let input_inner = input_block.inner(chunks[0]);
        frame.render_widget(input_block, chunks[0]);
        frame.render_widget(&popup.input, input_inner);

        let suggestion_items: Vec<ListItem> = popup
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let style = if i == popup.suggestion_index {
                    Style::default()
                        .fg(app.app_theme.highlight_fg)
                        .bg(app.app_theme.heading)
                } else {
                    Style::default()
                };
                ListItem::new(format!("  {}", tag)).style(style)
            })
            .collect();

        let suggestions_list = List::new(suggestion_items).block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .title("Suggestions (Tab: accept, Shift+D: delete)"),
        );
        frame.render_widget(suggestions_list, chunks[1]);

        let tag_display = popup.all_tags.join("  •  ");
        let tags_paragraph = Paragraph::new(tag_display).wrap(Wrap { trim: true }).block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .title("All existing tags"),
        );
        frame.render_widget(tags_paragraph, chunks[2]);
    }

    if let Some(popup) = &mut app.popups.filter_tag {
        let popup_area = centered_rect(60, 40, area);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Min(5),
            ])
            .split(popup_area);

        let input_block = Block::default().style(app.app_theme.bg_style()).borders(Borders::ALL).title(
            "Filter Tags (comma separated OR logic) - Tab: autocomplete, Enter: apply, Esc: cancel",
        );
        let input_inner = input_block.inner(chunks[0]);
        frame.render_widget(input_block, chunks[0]);
        frame.render_widget(&popup.input, input_inner);

        let suggestion_items: Vec<ListItem> = popup
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let style = if i == popup.suggestion_index {
                    Style::default()
                        .fg(app.app_theme.highlight_fg)
                        .bg(app.app_theme.heading)
                } else {
                    Style::default()
                };
                ListItem::new(format!("  {}", tag)).style(style)
            })
            .collect();

        let suggestions_list = List::new(suggestion_items).block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .title("Suggestions (Tab to accept)"),
        );
        frame.render_widget(suggestions_list, chunks[1]);

        let tag_display = popup.all_tags.join("  •  ");
        let tags_paragraph = Paragraph::new(tag_display).wrap(Wrap { trim: true }).block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .title("All existing tags"),
        );
        frame.render_widget(tags_paragraph, chunks[2]);
    }

    if let Some(picker) = &app.popups.folder_picker {
        let popup_area = centered_rect(40, 60, area);
        frame.render_widget(Clear, popup_area);

        let items: Vec<ListItem> = picker
            .folders
            .iter()
            .map(|f| {
                let label = if f.is_empty() { "Vault (Root)" } else { f };
                ListItem::new(label)
            })
            .collect();

        let title = match &picker.mode {
            crate::app::FolderPickerMode::MoveNote { .. } => {
                "Move note to folder".to_string()
            }
            crate::app::FolderPickerMode::MoveFolder { folder_path } => {
                let folder_name = folder_path.rsplit('/').next().unwrap_or(folder_path);
                format!("Move '{}' folder to", folder_name)
            }
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .title(title),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut state = ListState::default();
        state.select(Some(picker.selected));

        frame.render_stateful_widget(list, popup_area, &mut state);
    }

    if let Some(palette) = &mut app.command_palette {
        let palette_area = centered_rect(60, 60, area);
        frame.render_widget(Clear, palette_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(palette_area);

        frame.render_widget(&palette.input, chunks[0]);

        let items: Vec<ListItem> = palette
            .items
            .iter()
            .map(|item| {
                ListItem::new(vec![
                    Line::from(Span::styled(
                        &item.name,
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        &item.description,
                        Style::default().fg(app.app_theme.muted),
                    )),
                ])
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .title(" Commands "),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, chunks[1], &mut palette.state);
    }

    if let Some(popup) = &mut app.popups.note_rename {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(popup) = &mut app.popups.note_create {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(popup) = &mut app.popups.draw_create {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(popup) = &mut app.popups.canvas_create {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(popup) = &mut app.popups.search {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(trash) = &app.popups.trash_view {
        let popup_area = centered_rect(70, 70, area);
        frame.render_widget(Clear, popup_area);

        let items: Vec<ListItem> = trash
            .items
            .iter()
            .map(|item| {
                let name = item.name.to_string_lossy();
                let when = format_relative_time(item.time_deleted as u64);
                ListItem::new(Line::from(vec![
                    Span::raw(name.to_string()),
                    Span::styled(
                        format!("  ({when})"),
                        Style::default().fg(app.app_theme.muted),
                    ),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .title("Trash - r:restore d:delete E:empty q:close"),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut state = ListState::default();
        state.select(Some(trash.selected));

        frame.render_stateful_widget(list, popup_area, &mut state);
    }

    if let Some(popup) = &app.popups.confirm {
        draw_confirm_popup(frame, popup, area, &app.app_theme);
    }
}

pub fn draw_template_popup(
    frame: &mut Frame,
    popup: &TemplatePopup,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
    let popup_area = centered_rect(60, 60, area);

    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = popup
        .templates
        .iter()
        .map(|t| {
            ListItem::new(Line::from(vec![
                Span::styled(&t.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("  ({})", t.filename),
                    Style::default().fg(theme.muted),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .title("Select Template (Enter to select, Esc to cancel)")
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(popup.selected));

    frame.render_stateful_widget(list, popup_area, &mut state);
}

pub fn draw_theme_popup(
    frame: &mut Frame,
    popup: &ThemePopup,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
    let popup_area = centered_rect(40, 60, area);
    frame.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(popup_area);

    let items: Vec<ListItem> = popup
        .themes
        .iter()
        .map(|t| ListItem::new(Line::from(Span::raw(t))))
        .collect();

    let list_style = if popup.focus == crate::app::ThemePopupFocus::ThemeList {
        Style::default().fg(theme.heading)
    } else {
        Style::default()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .title(" Themes (Tab to navigate) ")
                .border_style(list_style),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(popup.selected));
    frame.render_stateful_widget(list, chunks[0], &mut state);

    let gen_label = if popup.general_is_solid { "general:bg on" } else { "general:bg off" };
    let graph_label = if popup.graph_is_solid { "graph:bg on" } else { "graph:bg off" };

    let gen_style = if popup.focus == crate::app::ThemePopupFocus::GeneralBg {
        Style::default().fg(theme.highlight_fg).bg(theme.heading).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let graph_style = if popup.focus == crate::app::ThemePopupFocus::GraphBg {
        Style::default().fg(theme.highlight_fg).bg(theme.heading).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };

    let pills = Line::from(vec![
        Span::styled(format!(" {} ", gen_label), gen_style),
        Span::raw("  "),
        Span::styled(format!(" {} ", graph_label), graph_style),
    ]);
    let pills_para = Paragraph::new(pills)
        .alignment(Alignment::Center)
        .style(theme.bg_style());
    frame.render_widget(pills_para, chunks[1]);
}

pub fn get_textarea_scroll(textarea: &TextArea) -> (usize, usize) {
    let mut scroll_row = 0;
    let mut scroll_col = 0;

    let debug_str = format!("{textarea:?}");
    if let Some(start) = debug_str.find("viewport: Viewport(") {
        let after_start = &debug_str[start + "viewport: Viewport(".len()..];
        if let Some(end) = after_start.find(')') {
            let number_str = &after_start[..end];
            if let Ok(number) = number_str.parse::<u64>() {
                scroll_row = ((number >> 16) & 0xFFFF) as usize;
                scroll_col = (number & 0xFFFF) as usize;
            }
        }
    }
    (scroll_row, scroll_col)
}

pub fn line_number_gutter(line_count: usize, cursor_row: usize, scroll_row: usize, height: u16, theme: &AppThemeColors, top_padding: u16) -> Paragraph<'static> {
    let digits = line_count.max(1).to_string().len();
    let display_lines = height as usize;
    let mut gutter_lines: Vec<Line<'static>> = Vec::with_capacity(display_lines);
    for i in 0..display_lines.min(line_count.saturating_sub(scroll_row)) {
        let current_line_idx = i + scroll_row;
        let is_current = current_line_idx == cursor_row;
        let style = if is_current {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        gutter_lines.push(Line::from(vec![
            Span::styled(
                format!("{:>width$} ", current_line_idx + 1, width = digits),
                style,
            ),
        ]));
    }
    for _ in gutter_lines.len()..display_lines {
        gutter_lines.push(Line::from(Span::raw(" ")));
    }
    Paragraph::new(gutter_lines)
        .style(theme.preview_bg_style())
        .block(Block::default().padding(Padding::new(0, 0, top_padding, 0)).style(theme.preview_bg_style()))
}

pub fn draw_edit_view(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    let area = frame.area();

    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let body_area = outer_chunks[0];
    let hint_area = outer_chunks[1];

    let (edit_area, preview_area_rect, splitter_area) = if app.editor.editor_preview_enabled {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(body_area);
        (cols[0], Some(cols[2]), Some(cols[1]))
    } else {
        (body_area, None, None)
    };

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(edit_area);

    let title_area = inner_chunks[0];
    let editor_container = inner_chunks[1];

    app.editor.title_editor.set_style(
        app.app_theme.title_bar_bg_style().fg(app.app_theme.heading)
    );
    app.editor.title_editor.set_block(
        Block::default()
            .style(app.app_theme.title_bar_bg_style())
            .borders(Borders::NONE)
            .padding(Padding::new(2, 1, 1, 1)),
    );
    app.editor.title_editor.set_cursor_style(
        if focus == EditFocus::Title {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        },
    );
    app.editor.title_editor.set_cursor_line_style(Style::default());
    frame.render_widget(&app.editor.title_editor, title_area);

    if get_title_text(&app.editor.title_editor).is_empty() {
        let title_inner = Rect::new(
            title_area.x + 3,
            title_area.y + 1,
            title_area.width.saturating_sub(4),
            1,
        );
        let placeholder = Paragraph::new(Line::from(Span::styled(
            "Untitled note",
            Style::default().fg(app.app_theme.muted),
        )));
        frame.render_widget(placeholder, title_inner);
    }

    if let Some(preview_area_rect) = preview_area_rect {
        let content_area = editor_container;

        let line_count = app.editor.editor.lines().len();
        let cursor_row = app.editor.editor.cursor().0;
        let scroll_row = get_textarea_scroll(&app.editor.editor).0;
        
        let editor_area = if app.editor.show_line_numbers {
            let digits = line_count.max(1).to_string().len() as u16;
            let gutter_width = digits + 1;
            let gutter_area = Rect::new(content_area.x, content_area.y, gutter_width.min(content_area.width), content_area.height);
            let gutter = line_number_gutter(line_count, cursor_row, scroll_row, content_area.height, &app.app_theme, 0);
            frame.render_widget(gutter, gutter_area);
            Rect::new(content_area.x + gutter_area.width, content_area.y, content_area.width.saturating_sub(gutter_area.width), content_area.height)
        } else {
            content_area
        };

        app.editor.editor.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(0, 2, 0, 0)),
        );
        app.editor.editor.set_style(app.app_theme.bg_style());
        app.editor.editor.set_cursor_style(
            if focus == EditFocus::Body {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            },
        );
        app.editor.editor.set_cursor_line_style(
            if focus == EditFocus::Body {
                Style::default().bg(app.app_theme.preview_bg().unwrap_or(Color::DarkGray))
            } else {
                Style::default()
            },
        );
        frame.render_widget(&app.editor.editor, editor_area);
        if focus == EditFocus::Body {
            let cursor_bg = app.app_theme.preview_bg().unwrap_or(app.app_theme.highlight_bg);
            fill_cursor_line_bg(frame, &app.editor.editor, editor_area, cursor_bg);
        }

        match &app.editor.md_preview_renderer {
            Some(renderer) if !renderer.is_pending() => {
                let md_widget = crate::markdown::ScrollablePseudoTerminal::new(renderer.screen())
                    .scroll_offset(renderer.scroll_offset())
                    .theme_bg(app.app_theme.preview_bg())
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(md_widget, preview_area_rect);
            }
            Some(_) => {
                let loading = Paragraph::new("Rendering preview...")
                    .style(Style::default().fg(app.app_theme.muted))
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(loading, preview_area_rect);
            }
            None => {
                let placeholder = Paragraph::new("Press Ctrl+P to render preview")
                    .style(app.app_theme.preview_bg_style())
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(placeholder, preview_area_rect);
            }
        }
    } else {
        let line_count = app.editor.editor.lines().len();
        let cursor_row = app.editor.editor.cursor().0;
        let scroll_row = get_textarea_scroll(&app.editor.editor).0;
        let content_area = editor_container;

        let editor_area = if app.editor.show_line_numbers {
            let digits = line_count.max(1).to_string().len() as u16;
            let gutter_width = digits + 1;
            let gutter_area = Rect::new(content_area.x, content_area.y, gutter_width.min(content_area.width), content_area.height);
            let gutter = line_number_gutter(line_count, cursor_row, scroll_row, content_area.height, &app.app_theme, 0);
            frame.render_widget(gutter, gutter_area);
            Rect::new(content_area.x + gutter_area.width, content_area.y, content_area.width.saturating_sub(gutter_area.width), content_area.height)
        } else {
            content_area
        };

        app.editor.editor.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(0, 2, 0, 0)),
        );
        app.editor.editor.set_style(app.app_theme.bg_style());
        app.editor.editor.set_cursor_style(
            if focus == EditFocus::Body {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            },
        );
        app.editor.editor.set_cursor_line_style(
            if focus == EditFocus::Body {
                Style::default().bg(app.app_theme.preview_bg().unwrap_or(Color::DarkGray))
            } else {
                Style::default()
            },
        );
        frame.render_widget(&app.editor.editor, editor_area);
        if focus == EditFocus::Body {
            let cursor_bg = app.app_theme.preview_bg().unwrap_or(app.app_theme.highlight_bg);
            fill_cursor_line_bg(frame, &app.editor.editor, editor_area, cursor_bg);
        }
    }

    draw_hint_line(frame, hint_area, app, EDIT_HELP_HINTS, false, false);
    draw_corner_watermark(frame, hint_area, app.app_theme.muted);
    if let Some(splitter_area) = splitter_area {
        draw_dim_vline(frame, splitter_area, app.app_theme.muted);
    }

    if app.status.starts_with("Save failed") || app.status.starts_with("Could not open") {
        let popup = centered_rect(75, 20, area);
        frame.render_widget(Clear, popup);
        let text = Paragraph::new(app.status.as_ref())
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .title("Error"),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(text, popup);
    }

    if let Some(menu) = &app.popups.context_menu {
        let items = vec![
            ListItem::new(" Copy       "),
            ListItem::new(" Cut        "),
            ListItem::new(" Paste      "),
            ListItem::new(" Select All "),
        ];
        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.preview_bg_style())
                    .borders(Borders::NONE),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let menu_area = Rect::new(menu.x, menu.y, 14, 4);
        let mut state = ListState::default();
        state.select(Some(menu.selected));

        frame.render_widget(Clear, menu_area);
        frame.render_stateful_widget(list, menu_area, &mut state);
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1].inner(Margin {
        vertical: 0,
        horizontal: 0,
    })
}

pub fn text_area_from_content(content: &str) -> TextArea<'static> {
    if content.is_empty() {
        TextArea::default()
    } else {
        let lines: Vec<String> = content.lines().map(ToString::to_string).collect();
        TextArea::from(lines)
    }
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

pub fn format_relative_time(unix_ts: u64) -> Cow<'static, str> {
    let now = now_unix_secs();
    let diff = now.saturating_sub(unix_ts);

    if diff < 60 {
        return Cow::Borrowed("just now");
    }
    if diff < 3600 {
        return Cow::Owned(format!("{}m ago", diff / 60));
    }
    if diff < 86_400 {
        return Cow::Owned(format!("{}h ago", diff / 3600));
    }

    let secs = UNIX_EPOCH + Duration::from_secs(unix_ts);
    let dt: chrono::DateTime<chrono::Local> = secs.into();
    Cow::Owned(dt.format("%Y-%m-%d %H:%M").to_string())
}

pub fn draw_confirm_popup(
    frame: &mut Frame,
    popup: &ConfirmPopup,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
    let popup_area = centered_rect(50, 30, area);
    frame.render_widget(Clear, popup_area);

    let border_color = if popup.is_destructive {
        theme.destructive
    } else {
        theme.heading
    };

    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(popup.title.as_str());

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let message = Paragraph::new(popup.message.as_str()).alignment(Alignment::Center);
    frame.render_widget(message, chunks[0]);

    if let Some(detail) = &popup.detail {
        let detail_para = Paragraph::new(detail.as_str())
            .style(Style::default().fg(theme.muted))
            .alignment(Alignment::Center);
        frame.render_widget(detail_para, chunks[1]);
    }

    let (confirm_style, cancel_style) = if popup.selected_button == 0 {
        let confirm = if popup.is_destructive {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.destructive)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.success)
                .add_modifier(Modifier::BOLD)
        };
        let cancel = Style::default().fg(theme.muted).patch(theme.bg_style());
        (confirm, cancel)
    } else {
        let confirm = if popup.is_destructive {
            Style::default()
                .fg(theme.destructive)
                .patch(theme.bg_style())
        } else {
            Style::default().fg(theme.success).patch(theme.bg_style())
        };
        let cancel = Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.highlight_bg)
            .add_modifier(Modifier::BOLD);
        (confirm, cancel)
    };

    let buttons = Line::from(vec![
        Span::styled(format!(" {} (y) ", popup.confirm_label), confirm_style),
        Span::raw("   "),
        Span::styled(" Cancel (n) ", cancel_style),
    ]);
    let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
    frame.render_widget(buttons_para, chunks[3]);
}

fn draw_dim_vline(frame: &mut Frame, area: Rect, color: Color) {
    let buf = frame.buffer_mut();
    for row in area.top()..area.bottom() {
        if let Some(cell) = buf.cell_mut((area.x, row)) {
            cell.set_symbol("│");
            cell.set_fg(color);
        }
    }
}

fn draw_hint_line(frame: &mut Frame, area: Rect, app: &App, hints: &str, ext_focused: bool, show_ext: bool) {
    let status = app.status.as_ref();
    let right_text = if !status.is_empty() && status != "Ready" {
        crate::sanitize::sanitize_for_terminal(status)
    } else {
        Cow::Owned(hints.to_string())
    };

    let mut spans: Vec<Span> = Vec::new();

    if show_ext {
        let ext_label = if app.editor.external_editor_enabled { "ext:on" } else { "ext:off" };
        let ext_style = if ext_focused {
            Style::default().fg(app.app_theme.highlight_fg).bg(app.app_theme.heading).add_modifier(Modifier::BOLD)
        } else if app.editor.external_editor_enabled {
            Style::default().fg(app.app_theme.success).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        spans.push(Span::styled(format!(" {} ", ext_label), ext_style));
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled(right_text, Style::default().fg(app.app_theme.muted)));

    let line = Line::from(spans);
    let para = Paragraph::new(line)
        .style(app.app_theme.hint_line_bg_style());
    frame.render_widget(para, area);
}

fn draw_corner_watermark(frame: &mut Frame, area: Rect, color: Color) {
    let version = env!("CARGO_PKG_VERSION");
    let text = format!("clin v{}", version);
    let width = text.len() as u16;
    if area.width < width + 2 || area.height < 1 { return; }
    let wm_area = Rect::new(area.x + area.width - width - 1, area.y, width, 1);
    let para = Paragraph::new(text)
        .style(Style::default().fg(color));
    frame.render_widget(para, wm_area);
}

pub fn fill_cursor_line_bg(frame: &mut Frame, editor: &TextArea, area: Rect, bg: Color) {
    if editor.selection_range().is_some() {
        return;
    }
    let (scroll_row, _) = get_textarea_scroll(editor);
    let cursor_row = editor.cursor().0;
    let screen_row = cursor_row.saturating_sub(scroll_row) as u16;
    let inner_y = editor.block().map(|b| b.inner(area).y).unwrap_or(area.y);
    let y = inner_y + screen_row;
    if y < area.y || y >= area.bottom() { return; }
    let buf = frame.buffer_mut();
    for x in area.left()..area.right() {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_bg(bg);
        }
    }
}

pub fn open_in_file_manager(path: &Path) -> Result<()> {
    use std::process::Stdio;

    let command = if cfg!(target_os = "linux") {
        "xdg-open"
    } else if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        anyhow::bail!("opening file manager is not supported on this platform")
    };

    Command::new(command)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch {command}"))?;
    Ok(())
}

pub fn pick_file(filter_name: &str, filter_ext: &str) -> Result<Option<String>> {
    if cfg!(target_os = "linux") {
        if which::which("zenity").is_ok() {
            let output = Command::new("zenity")
                .arg("--file-selection")
                .arg(format!("--file-filter={} | *{}", filter_name, filter_ext))
                .output()?;
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()));
            }
        } else if which::which("kdialog").is_ok() {
            let output = Command::new("kdialog")
                .arg("--getopenfilename")
                .arg(".")
                .arg(format!("*{}", filter_ext))
                .output()?;
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()));
            }
        }
    } else if cfg!(target_os = "macos") {
        let posix_script = format!(
            "POSIX path of (choose file with prompt \"Select a {} file\" of type {{\"{}\"}})",
            filter_name, filter_ext.trim_start_matches('.')
        );
        let output = Command::new("osascript")
            .arg("-e")
            .arg(posix_script)
            .output()?;
        if output.status.success() {
            return Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()));
        }
    } else if cfg!(target_os = "windows") {
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Filter = '{} (*{})|*{}'; $f.ShowDialog() | Out-Null; $f.FileName",
            filter_name, filter_ext, filter_ext
        );
        let output = Command::new("powershell")
            .arg("-Command")
            .arg(ps_script)
            .output()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}
