use crate::app::{
    App, ConfirmPopup, EditFocus, HelpTab, ListFocus, TemplatePopup, ThemePopup, ViewMode,
};
use crate::app_theme::AppThemeColors;
use crate::constants::*;
use crate::events::get_title_text;
use crate::keybinds::*;
use crate::list_view::PreviewContent;
use anyhow::{Context, Result};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::*;
use std::borrow::Cow;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

/// Style a result line with colored tags [..] and count (..) spans
fn styled_result_line(s: &str, theme: &AppThemeColors) -> Line<'static> {
    // Check for tag section " [tags]" at end of line (before optional "  (count)")
    if let Some(tag_start) = s.find(" [") {
        let after_tag = &s[tag_start..];
        if let Some(close_bracket) = after_tag.find(']') {
            let after_bracket = &after_tag[close_bracket + 1..];
            let is_end_tag = after_bracket.is_empty() || after_bracket.starts_with(" (");
            if is_end_tag {
                let label_part = s[..tag_start].to_string();
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
                let mut spans: Vec<Span<'static>> = vec![
                    Span::raw(label_part),
                    Span::styled(
                        tag_content.to_string(),
                        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
                    ),
                ];
                if let Some(count) = count_part {
                    spans.push(Span::styled(
                        count.to_string(),
                        Style::default().fg(theme.heading).add_modifier(Modifier::BOLD),
                    ));
                }
                return Line::from(spans);
            }
        }
    }
    // Fallback: no tags but check for standalone count " (digits)" at end of line
    if let Some(count_start) = s.find(" (") {
        let count_part = &s[count_start + 1..]; // skip " "
        if count_part.starts_with('(')
            && count_part.len() > 2
            && count_part.ends_with(')')
            && count_part[1..count_part.len() - 1].chars().all(|c| c.is_ascii_digit())
        {
            let label_part = s[..count_start + 1].to_string();
            return Line::from(vec![
                Span::raw(label_part),
                Span::styled(
                    count_part.to_string(),
                    Style::default().fg(theme.heading).add_modifier(Modifier::BOLD),
                ),
            ]);
        }
    }
    Line::from(s.to_string())
}

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

    let tab_names = [
        "Notes",
        "Editor",
        "Graph",
        "Draw",
        "Canvas",
        "Templates",
        "About",
    ];
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
    ))
    .style(app.app_theme.hint_line_bg_style());
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
        crate::app::HelpTab::Templates => templates_help_text(keybinds, theme),
        crate::app::HelpTab::About => about_help_text(keybinds, theme),
    }
}

fn notes_help_text(keybinds: &Keybinds, theme: &crate::app_theme::AppThemeColors) -> Text<'static> {
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
    lines.push(Line::from(""));
    lines.push(help_heading("Popups", theme));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Search Popup (/)",
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Search notes by title or filename",
        Some("Type query"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Full-text grep note content",
        Some("g:  (prefix to query)"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Filter by folder",
        Some("f:folder_name  (e.g. \"test f:inbox\")"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Empty folder = Vault root",
        Some("f:  (restricts to root notes only)"),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Tag Popup (t)",
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Add/remove tags from selected note",
        Some("Type tag name, Enter to add"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Select suggestion",
        Some("Tab  or  ↓"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Delete a tag from all notes",
        Some("d  on tag in All Tags list"),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Filter Tags Popup (F)",
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Show only notes with specific tags",
        Some("Click/drag tags to filter list"),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Template Popup (T)",
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Create note from saved template",
        Some("Select template, Enter to use"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Create new template from current note",
        Some("n / Create button"),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Confirm Delete Popup",
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Accept (delete)",
        Some("y / Enter / Confirm button"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Cancel / decline",
        Some("n / Esc / Cancel button"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle button focus",
        Some("Tab / ← →"),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Other Popups",
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Theme selector",
        Some("Cycle with ↑↓, preview applies"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Folder rename / create",
        Some("Enter new name, confirm with Enter"),
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
    lines.extend(help_item_dyn(
        "Save + quit app entirely",
        Some(&edit_quit),
        theme,
    ));
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

fn graph_help_text(keybinds: &Keybinds, theme: &crate::app_theme::AppThemeColors) -> Text<'static> {
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
    lines.extend(help_item_dyn(
        "Click and drag background to pan",
        None,
        theme,
    ));
    lines.extend(help_item_dyn("Click node to select", None, theme));
    lines.extend(help_item_dyn("Double-click node to open note", None, theme));
    Text::from(lines)
}

fn draw_help_text(theme: &crate::app_theme::AppThemeColors) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(help_heading("Tools", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Draw freehand strokes", Some("d"), theme));
    lines.extend(help_item_dyn("Shape tool (opens picker)", Some("s"), theme));
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
    lines.extend(help_item_dyn(
        "Right-click or middle-click drag to pan",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Select tool from toolbar at bottom",
        None,
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("General", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Auto-saved on changes & quit", None, theme));
    lines.extend(help_item_dyn("Exit canvas view", Some("Esc"), theme));
    Text::from(lines)
}

fn canvas_help_text(theme: &crate::app_theme::AppThemeColors) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(help_heading("Navigation", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Move selection",
        Some("←/→/↑/↓ / h/l/k/j"),
        theme,
    ));
    lines.extend(help_item_dyn("Zoom in", Some("+/="), theme));
    lines.extend(help_item_dyn("Zoom out", Some("-/_"), theme));
    lines.extend(help_item_dyn("Zoom in (fine)", Some("Ctrl+j"), theme));
    lines.extend(help_item_dyn("Zoom out (fine)", Some("Ctrl+k"), theme));
    lines.push(Line::from(""));
    lines.push(help_heading("Editing", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Open / edit selected node",
        Some("i / Enter"),
        theme,
    ));
    lines.extend(help_item_dyn("Connect two nodes", Some("i / Enter"), theme));
    lines.extend(help_item_dyn("Context menu", Some("a"), theme));
    lines.push(Line::from(""));
    lines.push(help_heading("Interface", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Toggle grid", Some("Ctrl+g"), theme));
    lines.extend(help_item_dyn("Toggle editor pane", Some("Ctrl+e"), theme));
    lines.extend(help_item_dyn(
        "Focus editor / ext toggle",
        Some("Tab"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle external editor mode",
        Some("Space"),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(help_heading("Editor (focused)", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Exit editor focus", Some("Esc / Tab"), theme));
    lines.extend(help_item_dyn(
        "Save raw editor changes",
        Some("Ctrl+s"),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(help_heading("General", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("Save canvas file", Some("Ctrl+s"), theme));
    lines.extend(help_item_dyn("Cancel connection", Some("Esc"), theme));
    lines.extend(help_item_dyn("Exit canvas view", Some("Esc"), theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn("* Ctrl+Enter in editor to save", None, theme));
    lines.extend(help_item_dyn("* Nodes auto-saved on changes", None, theme));
    Text::from(lines)
}

fn templates_help_text(
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let list_template = keybinds.list_keys_display(ListAction::NewFromTemplate);

    let mut lines = Vec::new();
    lines.push(help_heading("Template System", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Open template picker from notes view",
        Some(&list_template),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Search templates by name (in picker)",
        Some("Type in search bar"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Switch search/results focus",
        Some("Tab"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Open this help from template picker",
        Some("?"),
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Template Location", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Templates directory",
        Some("~/.config/clin/templates/"),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Default template (auto-used on new note)",
        Some("default.toml"),
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Minimal Template Skeleton", theme));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("name", Style::default().fg(theme.success)),
        Span::raw(" = \"My Template\""),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "[title]",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("template", Style::default().fg(theme.success)),
        Span::raw(" = \"Note - {date}\""),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "[content]",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("template", Style::default().fg(theme.success)),
        Span::raw(" = \"# {weekday}, {date}\\n\\n\""),
    ]));
    lines.push(Line::from(""));

    lines.push(help_heading("Supported Variables", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Current date (YYYY-MM-DD)",
        Some("{date}"),
        theme,
    ));
    lines.extend(help_item_dyn("Date and time", Some("{datetime}"), theme));
    lines.extend(help_item_dyn("Current time (HH:MM)", Some("{time}"), theme));
    lines.extend(help_item_dyn("Full weekday name", Some("{weekday}"), theme));
    lines.extend(help_item_dyn("4-digit year", Some("{year}"), theme));
    lines.extend(help_item_dyn("Zero-padded month", Some("{month}"), theme));
    lines.extend(help_item_dyn("Zero-padded day", Some("{day}"), theme));
    lines.push(Line::from(""));

    lines.push(help_heading("Examples", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Meeting: title=\"Meeting - {date}\", content with Agenda/Action Items",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Todo: title=\"Tasks - {date}\", content with priority sections",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Journal: title=\"Journal - {date}\", content with mood/gratitude prompts",
        None,
        theme,
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("Tips & Troubleshooting", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Create examples: clin --create-example-templates",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "List templates: clin --list-templates",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Unknown {variables} are left as-is in output",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Use multiline strings (triple quotes \"\"\") for content",
        None,
        theme,
    ));
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
        Span::styled(
            "  clin",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("                         Launch interactive TUI"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin -n [TITLE]",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("                Create note + open editor"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin -q <text> [TITLE]",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("        Quick note without TUI"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin -e <TITLE>",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("                Open existing note by title"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin -l",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("                          List all note titles"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin -h, --help",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("                 Show CLI help message"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --storage-path",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("           Show current storage path"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --set-storage-path <PATH>",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  Set storage directory"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --reset-storage-path",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("       Reset to default storage"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --migrate-storage",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("         Migrate data from old location"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --keybinds",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("                Show current keybindings"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --export-keybinds",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("         Export keybinds as TOML"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --reset-keybinds",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("          Reset keybinds to defaults"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --list-templates",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("          List available templates"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  clin --create-example-templates",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
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
        .constraints([Constraint::Min(5), Constraint::Length(1)])
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
        let list_area = Rect::new(
            full_cols[0].x,
            full_cols[0].y,
            full_cols[0].width,
            chunks[0].height,
        );
        let preview_area = Some(Rect::new(
            full_cols[2].x,
            full_cols[2].y,
            full_cols[2].width,
            chunks[0].height,
        ));
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
                let is_pinned = name == crate::app::VIRTUAL_PINNED_LABEL;
                let icon = if is_pinned {
                    if *is_expanded { " " } else { " " }
                } else if *is_expanded {
                    " "
                } else {
                    " "
                };
                let color = if is_pinned {
                    app.app_theme.heading
                } else {
                    app.app_theme.folder
                };
                let sanitized_name = crate::sanitize::sanitize_for_terminal(name);
                let mut text = format!("{indent}{icon} {sanitized_name} ({note_count})");
                if app.list.list_mode == crate::list_view::ListMode::Select {
                    let checkbox = if app.list.selected_indices.contains(&vi) {
                        "[x] "
                    } else {
                        "[ ] "
                    };
                    text = format!("{indent}{checkbox}{icon} {sanitized_name} ({note_count})");
                }
                items.push(ListItem::new(Line::from(vec![Span::styled(
                    text,
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(color),
                )])));
            }
            crate::app::VisualItem::Note {
                summary_idx,
                depth,
                is_clin,
                is_draw,
                is_canvas,
                in_virtual_pinned_folder,
                ..
            } => {
                let summary = &app.notes[*summary_idx];
                let indent = "  ".repeat(*depth);

                let when = format_relative_time(summary.updated_at);
                let mut text_style = Style::default();

                let mut spans = Vec::new();
                spans.push(Span::raw(indent));

                if app.list.list_mode == crate::list_view::ListMode::Select {
                    let checkbox = if app.list.selected_indices.contains(&vi) {
                        "[x] "
                    } else {
                        "[ ] "
                    };
                    spans.push(Span::styled(
                        checkbox,
                        if app.list.selected_indices.contains(&vi) {
                            Style::default()
                                .fg(app.app_theme.accent)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.app_theme.muted)
                        },
                    ));
                }

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

                if *in_virtual_pinned_folder {
                    let source = if summary.folder.is_empty() {
                        "Vault".to_string()
                    } else {
                        summary.folder.clone()
                    };
                    spans.push(Span::styled(
                        format!(
                            "  (from {})",
                            crate::sanitize::sanitize_for_terminal(&source)
                        ),
                        Style::default().fg(app.app_theme.muted),
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

    if app.list.list_mode == crate::list_view::ListMode::Select {
        let mode_label = if app.list.tag_to_assign.is_some() {
            "TAG MODE"
        } else {
            "SELECT MODE"
        };
        let select_hint = format!(
            " {}: {} selected ",
            mode_label,
            app.list.selected_indices.len()
        );
        let select_para = Paragraph::new(Span::styled(
            select_hint,
            Style::default()
                .fg(app.app_theme.highlight_fg)
                .bg(app.app_theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        let max_width = list_area.width.saturating_sub(4);
        let select_width = 34.min(max_width);
        let select_area = Rect::new(list_area.x + 2, list_area.y, select_width, 1);
        frame.render_widget(select_para, select_area);
    }

    if let Some(preview_rect) = preview_area {
        // Check if selected note is encrypted and preview_encryption is enabled
        let hide_encrypted = app.preview_encryption
            && app.list.visual_list.get(app.list.visual_index).is_some_and(|item| {
                matches!(item, crate::app::VisualItem::Note { is_clin: true, .. })
            });

        if hide_encrypted {
            // Show lock icon instead of preview content
            let lock_lines = vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "\u{f023}  Encrypted Note",
                        Style::default()
                            .fg(app.app_theme.destructive)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "Content hidden — decrypt to preview",
                        Style::default().fg(app.app_theme.muted),
                    ),
                ]),
            ];
            let lock_para = Paragraph::new(lock_lines)
                .style(app.app_theme.preview_bg_style())
                .block(
                    Block::default()
                        .style(app.app_theme.preview_bg_style())
                        .borders(Borders::NONE)
                        .padding(Padding::new(2, 2, 1, 1)),
                );
            frame.render_widget(lock_para, preview_rect);
        } else {
            // Only show preview content if it matches the current selection
            let content_is_current = app.list.preview_content_index == Some(app.list.visual_index);

            if !content_is_current {
                let placeholder = Paragraph::new("Select a note to preview")
                    .style(app.app_theme.preview_bg_style())
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(placeholder, preview_rect);
            } else {
            match &app.list.preview_content {
                Some(PreviewContent::Markdown(renderer)) if !renderer.is_pending() => {
                    let content_empty = renderer
                        .screen()
                        .contents()
                        .trim()
                        .is_empty();
                    if content_empty {
                        let placeholder = Paragraph::new(Line::from(vec![
                            Span::styled(
                                "(empty note)",
                                Style::default()
                                    .fg(app.app_theme.muted),
                            ),
                        ]))
                        .style(app.app_theme.preview_bg_style())
                        .block(
                            Block::default()
                                .style(app.app_theme.preview_bg_style())
                                .borders(Borders::NONE)
                                .padding(Padding::new(2, 2, 1, 1)),
                        );
                        frame.render_widget(placeholder, preview_rect);
                    } else {
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
            } // match
            } // content_is_current else
        } // hide_encrypted else
    } // preview_rect if let

    draw_hint_line(
        frame,
        chunks[1],
        app,
        LIST_HELP_HINTS,
        app.list.list_focus == ListFocus::ExternalEditorToggle,
        true,
    );
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
        let popup_area = centered_rect(NOTES_POPUP_LARGE_W_PCT, NOTES_POPUP_LARGE_H_PCT, area);
        frame.render_widget(Clear, popup_area);

        let suggestion_height = if popup.suggestions.is_empty() {
            0u16
        } else {
            (popup.suggestions.len() as u16).min(5).max(1)
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3u16 + suggestion_height),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(popup_area);

        // Combined input + suggestions section
        let input_border = if popup.focus == crate::popups::TagPopupFocus::Input {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(chunks[0]);

        let input_block = Block::default()
            .style(app.app_theme.bg_style())
            .borders(Borders::ALL)
            .border_style(input_border)
            .title("Manage Tags");
        let input_inner = input_block.inner(input_chunks[0]);
        frame.render_widget(input_block, chunks[0]);
        frame.render_widget(&popup.input, input_inner);

        if !popup.suggestions.is_empty() {
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

            let suggestions_list = List::new(suggestion_items)
                .block(
                    Block::default()
                        .borders(Borders::NONE)
                        .style(app.app_theme.bg_style()),
                )
                .highlight_style(Style::default());
            // suggestions use manual highlighting, not ListState
            frame.render_widget(suggestions_list, input_chunks[1]);
        }

        // All existing tags as a selectable list
        let all_tags_border = if popup.focus == crate::popups::TagPopupFocus::AllTagsList {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let tag_items: Vec<ListItem> = if popup.all_tags.is_empty() {
            vec![ListItem::new(Span::styled(
                "(no tags)",
                Style::default().fg(app.app_theme.muted),
            ))]
        } else {
            popup
                .all_tags
                .iter()
                .map(|tag| ListItem::new(format!("{}", tag)))
                .collect()
        };

        let tags_list = List::new(tag_items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(all_tags_border)
                    .title("All Tags"),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        let mut tags_state = ListState::default();
        if popup.focus == crate::popups::TagPopupFocus::AllTagsList && !popup.all_tags.is_empty() {
            tags_state.select(Some(popup.all_tags_selected));
        }
        frame.render_stateful_widget(tags_list, chunks[1], &mut tags_state);

        draw_popup_footer(
            frame,
            chunks[2],
            &app.app_theme,
            "Ctrl+S batch assign · Tab accept · Enter save · d delete from all · Esc cancel",
        );
    }

    if let Some(picker) = &app.popups.folder_picker {
        let popup_area = centered_rect(NOTES_POPUP_LARGE_W_PCT, NOTES_POPUP_LARGE_H_PCT, area);
        frame.render_widget(Clear, popup_area);

        let inner = popup_area;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let search_border = if picker.focus == crate::app::FolderPickerFocus::Search {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let search = Paragraph::new(format!("Search: {}", picker.query))
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(search_border)
                    .title("Folder Search"),
            )
            .style(Style::default().fg(app.app_theme.muted));
        frame.render_widget(search, chunks[0]);

        let items: Vec<ListItem> = if picker.filtered_folders.is_empty() {
            vec![ListItem::new(Span::styled(
                "(no matching folders)",
                Style::default().fg(app.app_theme.muted),
            ))]
        } else {
            picker
                .filtered_folders
                .iter()
                .map(|f| {
                    let label = if f.is_empty() { "Vault (Root)" } else { f };
                    ListItem::new(label)
                })
                .collect()
        };

        let title = match &picker.mode {
            crate::app::FolderPickerMode::MoveNote { .. } => "Move note to folder".to_string(),
            crate::app::FolderPickerMode::MoveFolder { folder_path } => {
                let folder_name = folder_path.rsplit('/').next().unwrap_or(folder_path);
                format!("Move '{}' folder to", folder_name)
            }
            crate::app::FolderPickerMode::BulkMoveNotes { note_ids } => {
                format!("Move {} selected note(s) to", note_ids.len())
            }
        };

        let results_border = if picker.focus == crate::app::FolderPickerFocus::Results {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(results_border)
                    .title(title),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        let mut state = ListState::default();
        if picker.focus == crate::app::FolderPickerFocus::Results
            && !picker.filtered_folders.is_empty()
        {
            state.select(Some(picker.selected));
        }

        frame.render_stateful_widget(list, chunks[1], &mut state);
        draw_popup_footer(
            frame,
            chunks[2],
            &app.app_theme,
            "Tab switch  Enter move  Esc cancel",
        );
    }

    if let Some(palette) = &mut app.command_palette {
        let palette_area = centered_rect(NOTES_POPUP_LARGE_W_PCT, NOTES_POPUP_LARGE_H_PCT, area);
        frame.render_widget(Clear, palette_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
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
                    .border_style(Style::default().fg(app.app_theme.muted))
                    .title("Commands"),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        frame.render_stateful_widget(list, chunks[1], &mut palette.state);
        draw_popup_footer(
            frame,
            chunks[2],
            &app.app_theme,
            "Enter run  ↑/↓ select  Esc close",
        );
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
        let popup_area = centered_rect(NOTES_POPUP_LARGE_W_PCT, NOTES_POPUP_LARGE_H_PCT, area);
        frame.render_widget(Clear, popup_area);

        let query_text = popup.input.lines().join("");
        let parsed = crate::app::parse_search_query(&query_text);
        let has_filter = parsed.folder_filter.is_some()
            || parsed.pinned_only
            || parsed.tag_filter.is_some()
            || parsed.grep_mode;

        let constraints = if has_filter {
            vec![
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
            ]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(1),
            ]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(popup_area);

        // Render filter badge if any filter active
        if has_filter {
            let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
            let mut first = true;

            let add_sep = |spans: &mut Vec<Span<'static>>, first: &mut bool, theme: &crate::app_theme::AppThemeColors| {
                if !*first {
                    spans.push(Span::styled(
                        " · ",
                        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
                    ));
                }
                *first = false;
            };

            if let Some(ref f) = parsed.folder_filter {
                let text = if f.is_empty() { "Vault" } else { f.as_str() };
                add_sep(&mut spans, &mut first, &app.app_theme);
                spans.push(Span::styled(
                    "\u{f07c} ",
                    Style::default().fg(app.app_theme.accent).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(app.app_theme.accent).add_modifier(Modifier::BOLD),
                ));
            }
            if parsed.pinned_only {
                add_sep(&mut spans, &mut first, &app.app_theme);
                spans.push(Span::styled(
                    "\u{f08d} Pinned",
                    Style::default().fg(app.app_theme.accent).add_modifier(Modifier::BOLD),
                ));
            }
            if parsed.grep_mode {
                add_sep(&mut spans, &mut first, &app.app_theme);
                let grep_display = if parsed.grep_text.is_empty() {
                    "Grep".to_string()
                } else {
                    parsed.grep_text.clone()
                };
                spans.push(Span::styled(
                    format!("\u{f002} {}", grep_display),
                    Style::default().fg(app.app_theme.accent).add_modifier(Modifier::BOLD),
                ));
            }
            if let Some(ref tags) = parsed.tag_filter {
                add_sep(&mut spans, &mut first, &app.app_theme);
                let tag_text = if tags.is_empty() {
                    String::new()
                } else {
                    tags.join(", ")
                };
                spans.push(Span::styled(
                    "\u{f02b} ",
                    Style::default().fg(app.app_theme.accent).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    tag_text,
                    Style::default().fg(app.app_theme.accent).add_modifier(Modifier::BOLD),
                ));
            }

            let filter_line = Line::from(spans);
            let filter_para = Paragraph::new(filter_line)
                .style(app.app_theme.bg_style());
            frame.render_widget(filter_para, chunks[1]);
        }

        let input_chunk = chunks[0];
        let results_chunk = if has_filter { chunks[2] } else { chunks[1] };
        let footer_chunk = if has_filter { chunks[3] } else { chunks[2] };

        popup.input.set_style(app.app_theme.bg_style());
        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(if popup.focus == crate::popups::SearchFocus::Input {
                    Style::default().fg(app.app_theme.heading)
                } else {
                    Style::default().fg(app.app_theme.muted)
                })
                .title("Search"),
        );
        frame.render_widget(&popup.input, input_chunk);

        let has_title = !popup.title_results.is_empty();
        let has_grep = !popup.grep_results.is_empty();

        let results_focused = popup.focus == crate::popups::SearchFocus::Results;
        let results_border = if results_focused {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };

        let (all_items, results_title) = if has_grep {
            // Grep tree view
            let mut visible: Vec<(usize, String)> = Vec::new();
            let mut i = 0;
            while i < popup.grep_results.len() {
                let is_collapsed = popup.grep_is_header[i]
                    && popup.grep_collapsed.contains(&i);
                let icon = if popup.grep_is_header[i] {
                    if is_collapsed { "\u{25b6}" } else { "\u{25bc}" }
                } else {
                    ""
                };
                visible.push((i, format!("{}{}", icon, popup.grep_results[i])));
                i += 1;
                if is_collapsed {
                    while i < popup.grep_results.len() && !popup.grep_is_header[i] {
                        i += 1;
                    }
                }
            }
            let items: Vec<ListItem> = visible.iter()
                .map(|(_, t)| ListItem::new(styled_result_line(t, &app.app_theme)))
                .collect();
            (items, "Results")
        } else if has_title {
            let items: Vec<ListItem> = popup
                .title_results
                .iter()
                .map(|entry| ListItem::new(styled_result_line(entry, &app.app_theme)))
                .collect();
            (items, "Results")
        } else {
            let msg = if query_text.trim().is_empty() && !has_filter {
                "Type to search notes"
            } else {
                "No results"
            };
            (vec![ListItem::new(Span::styled(
                msg.to_string(),
                Style::default().fg(app.app_theme.muted),
            ))], "Results")
        };

        let results_list = List::new(all_items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(results_border)
                    .title(results_title),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");
        let mut list_state = ListState::default();
        if results_focused && has_grep {
            // Map grep_selected (real index) to visible position
            let mut vis_pos = 0;
            let mut i = 0;
            while i < popup.grep_results.len() && i <= popup.grep_selected {
                let is_collapsed = popup.grep_is_header[i]
                    && popup.grep_collapsed.contains(&i);
                if i == popup.grep_selected {
                    list_state.select(Some(vis_pos));
                    break;
                }
                vis_pos += 1;
                i += 1;
                if is_collapsed {
                    while i < popup.grep_results.len() && !popup.grep_is_header[i] {
                        i += 1;
                    }
                }
            }
        } else if results_focused && has_title {
            list_state.select(Some(popup.title_selected));
        }
        frame.render_stateful_widget(results_list, results_chunk, &mut list_state);

        draw_popup_footer(
            frame,
            footer_chunk,
            &app.app_theme,
            "Tab switch · Enter open · Esc cancel · f:folder p:pinned t:tag g:text",
        );
    }

    if let Some(trash) = &app.popups.trash_view {
        let popup_area = centered_rect(70, 70, area);
        frame.render_widget(Clear, popup_area);

        let list_area = Rect::new(
            popup_area.x,
            popup_area.y,
            popup_area.width,
            popup_area.height.saturating_sub(1),
        );
        let footer_area = Rect::new(
            popup_area.x,
            popup_area.y + list_area.height,
            popup_area.width,
            1,
        );

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
                    .title("Trash"),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        let mut state = ListState::default();
        state.select(Some(trash.selected));

        frame.render_stateful_widget(list, list_area, &mut state);
        draw_popup_footer(
            frame,
            footer_area,
            &app.app_theme,
            "r restore  d delete  E empty  q close",
        );
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
    let popup_area = centered_rect(NOTES_POPUP_LARGE_W_PCT, NOTES_POPUP_LARGE_H_PCT, area);

    frame.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(popup_area);

    let search_border = if popup.focus == crate::popups::TemplatePopupFocus::Search {
        Style::default().fg(theme.heading)
    } else {
        Style::default().fg(theme.muted)
    };
    let search = Paragraph::new(format!("Search: {}", popup.query))
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(search_border)
                .title("Search Templates"),
        )
        .style(Style::default().fg(theme.muted));
    frame.render_widget(search, chunks[0]);

    let items: Vec<ListItem> = if popup.filtered_templates.is_empty() {
        vec![ListItem::new(Span::styled(
            "(no matching templates)",
            Style::default().fg(theme.muted),
        ))]
    } else {
        popup
            .filtered_templates
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
            .collect()
    };

    let results_border = if popup.focus == crate::popups::TemplatePopupFocus::Results {
        Style::default().fg(theme.heading)
    } else {
        Style::default().fg(theme.muted)
    };
    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(results_border),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  ");

    let mut state = ListState::default();
    if popup.focus == crate::popups::TemplatePopupFocus::Results
        && !popup.filtered_templates.is_empty()
    {
        state.select(Some(popup.selected));
    }

    frame.render_stateful_widget(list, chunks[1], &mut state);

    draw_popup_footer(
        frame,
        chunks[2],
        theme,
        "Tab switch · Enter use template · n create new · d delete · Space edit · ? help · Esc cancel",
    );
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
            Constraint::Length(3),
            Constraint::Length(3),
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
        Style::default().fg(theme.muted)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .title(" Themes ")
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

    let gen_label = if popup.general_is_solid {
        "General Background Color: ON"
    } else {
        "General Background Color: OFF"
    };
    let graph_label = if popup.graph_is_solid {
        "Graph Background Color: ON"
    } else {
        "Graph Background Color: OFF"
    };

    let gen_style = if popup.general_is_solid {
        Style::default().fg(theme.success)
    } else {
        Style::default().fg(theme.destructive)
    };
    let gen_block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(if popup.focus == crate::app::ThemePopupFocus::GeneralBg {
            Style::default().fg(theme.heading)
        } else {
            Style::default().fg(theme.muted)
        });
    let gen_inner = gen_block.inner(chunks[1]);
    let gen_para = Paragraph::new(Span::styled(gen_label, gen_style))
        .alignment(Alignment::Center)
        .style(theme.bg_style());
    frame.render_widget(gen_block, chunks[1]);
    frame.render_widget(gen_para, gen_inner);

    let graph_style = if popup.graph_is_solid {
        Style::default().fg(theme.success)
    } else {
        Style::default().fg(theme.destructive)
    };
    let graph_block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(if popup.focus == crate::app::ThemePopupFocus::GraphBg {
            Style::default().fg(theme.heading)
        } else {
            Style::default().fg(theme.muted)
        });
    let graph_inner = graph_block.inner(chunks[2]);
    let graph_para = Paragraph::new(Span::styled(graph_label, graph_style))
        .alignment(Alignment::Center)
        .style(theme.bg_style());
    frame.render_widget(graph_block, chunks[2]);
    frame.render_widget(graph_para, graph_inner);

    draw_popup_footer(
        frame,
        chunks[3],
        theme,
        "Tab navigate · Enter select · Esc close",
    );
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

pub fn line_number_gutter(
    line_count: usize,
    cursor_row: usize,
    scroll_row: usize,
    height: u16,
    theme: &AppThemeColors,
    top_padding: u16,
) -> Paragraph<'static> {
    let digits = line_count.max(1).to_string().len();
    let display_lines = height as usize;
    let mut gutter_lines: Vec<Line<'static>> = Vec::with_capacity(display_lines);
    for i in 0..display_lines.min(line_count.saturating_sub(scroll_row)) {
        let current_line_idx = i + scroll_row;
        let is_current = current_line_idx == cursor_row;
        let style = if is_current {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        gutter_lines.push(Line::from(vec![Span::styled(
            format!("{:>width$} ", current_line_idx + 1, width = digits),
            style,
        )]));
    }
    for _ in gutter_lines.len()..display_lines {
        gutter_lines.push(Line::from(Span::raw(" ")));
    }
    Paragraph::new(gutter_lines)
        .style(theme.preview_bg_style())
        .block(
            Block::default()
                .padding(Padding::new(0, 0, top_padding, 0))
                .style(theme.preview_bg_style()),
        )
}

pub fn draw_edit_view(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    let area = frame.area();

    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
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
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(edit_area);

    let title_area = inner_chunks[0];
    let editor_container = inner_chunks[1];

    app.editor
        .title_editor
        .set_style(app.app_theme.title_bar_bg_style().fg(app.app_theme.heading));
    app.editor.title_editor.set_block(
        Block::default()
            .style(app.app_theme.title_bar_bg_style())
            .borders(Borders::NONE)
            .padding(Padding::new(2, 1, 1, 1)),
    );
    app.editor
        .title_editor
        .set_cursor_style(if focus == EditFocus::Title {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        });
    app.editor
        .title_editor
        .set_cursor_line_style(Style::default());
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
            let gutter_area = Rect::new(
                content_area.x,
                content_area.y,
                gutter_width.min(content_area.width),
                content_area.height,
            );
            let gutter = line_number_gutter(
                line_count,
                cursor_row,
                scroll_row,
                content_area.height,
                &app.app_theme,
                0,
            );
            frame.render_widget(gutter, gutter_area);
            Rect::new(
                content_area.x + gutter_area.width,
                content_area.y,
                content_area.width.saturating_sub(gutter_area.width),
                content_area.height,
            )
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
        app.editor
            .editor
            .set_cursor_style(if focus == EditFocus::Body {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            });
        app.editor
            .editor
            .set_cursor_line_style(if focus == EditFocus::Body {
                Style::default().bg(app.app_theme.preview_bg().unwrap_or(Color::DarkGray))
            } else {
                Style::default()
            });
        frame.render_widget(&app.editor.editor, editor_area);
        if focus == EditFocus::Body {
            let cursor_bg = app
                .app_theme
                .preview_bg()
                .unwrap_or(app.app_theme.highlight_bg);
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
            let gutter_area = Rect::new(
                content_area.x,
                content_area.y,
                gutter_width.min(content_area.width),
                content_area.height,
            );
            let gutter = line_number_gutter(
                line_count,
                cursor_row,
                scroll_row,
                content_area.height,
                &app.app_theme,
                0,
            );
            frame.render_widget(gutter, gutter_area);
            Rect::new(
                content_area.x + gutter_area.width,
                content_area.y,
                content_area.width.saturating_sub(gutter_area.width),
                content_area.height,
            )
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
        app.editor
            .editor
            .set_cursor_style(if focus == EditFocus::Body {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            });
        app.editor
            .editor
            .set_cursor_line_style(if focus == EditFocus::Body {
                Style::default().bg(app.app_theme.preview_bg().unwrap_or(Color::DarkGray))
            } else {
                Style::default()
            });
        frame.render_widget(&app.editor.editor, editor_area);
        if focus == EditFocus::Body {
            let cursor_bg = app
                .app_theme
                .preview_bg()
                .unwrap_or(app.app_theme.highlight_bg);
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

fn draw_popup_footer(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
    hints: &str,
) {
    let footer = Paragraph::new(Span::styled(hints, Style::default().fg(theme.muted)))
        .alignment(Alignment::Center)
        .style(theme.hint_line_bg_style());
    frame.render_widget(footer, area);
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

fn draw_hint_line(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    hints: &str,
    ext_focused: bool,
    show_ext: bool,
) {
    let status = app.status.as_ref();
    let right_text = if !status.is_empty() && status != "Ready" {
        crate::sanitize::sanitize_for_terminal(status)
    } else {
        Cow::Owned(hints.to_string())
    };

    let mut spans: Vec<Span> = Vec::new();

    if show_ext {
        let ext_label = if app.editor.external_editor_enabled {
            "ext:on"
        } else {
            "ext:off"
        };
        let ext_style = if ext_focused {
            Style::default()
                .fg(app.app_theme.highlight_fg)
                .bg(app.app_theme.heading)
                .add_modifier(Modifier::BOLD)
        } else if app.editor.external_editor_enabled {
            Style::default()
                .fg(app.app_theme.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        spans.push(Span::styled(format!(" {} ", ext_label), ext_style));
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled(
        right_text,
        Style::default().fg(app.app_theme.muted),
    ));

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(app.app_theme.hint_line_bg_style());
    frame.render_widget(para, area);
}

fn draw_corner_watermark(frame: &mut Frame, area: Rect, color: Color) {
    let version = env!("CARGO_PKG_VERSION");
    let text = format!("clin v{}", version);
    let width = text.len() as u16;
    if area.width < width + 2 || area.height < 1 {
        return;
    }
    let wm_area = Rect::new(area.x + area.width - width - 1, area.y, width, 1);
    let para = Paragraph::new(text).style(Style::default().fg(color));
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
    if y < area.y || y >= area.bottom() {
        return;
    }
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
                return Ok(Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                ));
            }
        } else if which::which("kdialog").is_ok() {
            let output = Command::new("kdialog")
                .arg("--getopenfilename")
                .arg(".")
                .arg(format!("*{}", filter_ext))
                .output()?;
            if output.status.success() {
                return Ok(Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                ));
            }
        }
    } else if cfg!(target_os = "macos") {
        let posix_script = format!(
            "POSIX path of (choose file with prompt \"Select a {} file\" of type {{\"{}\"}})",
            filter_name,
            filter_ext.trim_start_matches('.')
        );
        let output = Command::new("osascript")
            .arg("-e")
            .arg(posix_script)
            .output()?;
        if output.status.success() {
            return Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ));
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
