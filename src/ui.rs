use crate::app::{App, ConfirmPopup, EditFocus, TemplatePopup, ThemePopup, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::constants::*;
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PopupSize {
    Small,   // 40% width, 40% height. Max bounds: 60 cols x 20 rows
    Medium,  // 50% width, 50% height. Max bounds: 80 cols x 30 rows
    Large,   // 60% width, 60% height. Max bounds: 100 cols x 40 rows
    Prompt,  // 50% width. Fixed 5 height. Max bounds: 80 cols wide
    Confirm, // 50% width. Fixed 12 height. Max bounds: 80 cols wide
}

fn split_lock_spans(text: &str, theme: &AppThemeColors) -> Vec<Span<'static>> {
    let mut result = Vec::new();
    let mut last = 0;
    for (i, _) in text.match_indices('\u{f023}') {
        if i > last {
            result.push(Span::raw(text[last..i].to_string()));
        }
        result.push(Span::styled(
            "\u{f023}".to_string(),
            Style::default()
                .fg(theme.destructive)
                .add_modifier(Modifier::BOLD),
        ));
        last = i + '\u{f023}'.len_utf8();
    }
    if last < text.len() {
        result.push(Span::raw(text[last..].to_string()));
    }
    result
}

const GRID_TILE_W: u16 = 10; // outer width incl. border
const GRID_TILE_H: u16 = 5; // outer height incl. border
const GRID_GAP: u16 = 1; // space between tiles (h and v)
const GRID_LEFT_MARGIN: u16 = 2; // left inset inside list_area
const GRID_TOP_MARGIN: u16 = 3; // top inset inside list_area

fn styled_result_line(s: &str, theme: &AppThemeColors) -> Line<'static> {
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

                let mut spans = split_lock_spans(label_part, theme);
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
            let mut spans = split_lock_spans(label_part, theme);
            spans.push(Span::styled(
                count_part.to_string(),
                Style::default()
                    .fg(theme.heading)
                    .add_modifier(Modifier::BOLD),
            ));
            return Line::from(spans);
        }
    }
    Line::from(split_lock_spans(s, theme))
}

fn style_palette_name(name: &str, theme: &AppThemeColors) -> Vec<Span<'static>> {
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
        ViewMode::Backup => {}
        ViewMode::ContentTree => {}
    }

    if let Some(popup) = &app.popups.theme {
        draw_theme_popup(frame, popup, frame.area(), &app.app_theme);
    }
    if let Some(popup) = &app.popups.sort {
        draw_sort_popup(frame, popup, frame.area(), &app.app_theme);
    }
    if let Some(popup) = &app.popups.create_format {
        draw_create_format_popup(frame, popup, frame.area(), &app.app_theme);
    }
}

/// Help-view tab (label, glyph) pairs, in HelpTab order. Shared by
/// `draw_help_view` (render) and the help-tab mouse hit-test so they never drift.
pub const HELP_TAB_NAMES: &[(&str, &str)] = &[
    ("Notes", "\u{f24a}"),        // sticky-note
    ("Editor", "\u{f040}"),       // pencil
    ("Graph", "\u{f0e8}"),        // sitemap
    ("Draw", "\u{f1fc}"),         // paint-brush
    ("Canvas", "\u{f00a}"),       // th-large
    ("Backup", "\u{f0c7}"),       // floppy-save
    ("Templates", "\u{f0c5}"),    // copy
    ("Content Tree", "\u{f1bb}"), // tree
    ("About", "\u{f05a}"),        // info-circle
];

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

    let tabs: Vec<(&str, Option<&str>)> =
        HELP_TAB_NAMES.iter().map(|&(l, g)| (l, Some(g))).collect();
    let tab_spans = build_tab_spans(
        &tabs,
        app.help_tab.index(),
        &app.app_theme,
        app.config.ui.tab_icons_only,
    );
    draw_view_title_bar_with_tabs(frame, chunks[0], "Help", tab_spans, &app.app_theme);

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

    draw_status_bar(
        frame,
        chunks[2],
        &app.app_theme,
        None,
        HELP_PAGE_HINTS,
        None,
    );
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
        crate::app::HelpTab::Draw => draw_help_text(keybinds, theme),
        crate::app::HelpTab::Canvas => canvas_help_text(keybinds, theme),
        crate::app::HelpTab::Backup => backup_help_text(keybinds, theme),
        crate::app::HelpTab::Templates => templates_help_text(keybinds, theme),
        crate::app::HelpTab::ContentTree => content_tree_help_text(keybinds, theme),
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
    let list_page_up = keybinds.list_keys_display(ListAction::PageUp);
    let list_page_down = keybinds.list_keys_display(ListAction::PageDown);
    let list_help = keybinds.list_keys_display(ListAction::Help);
    let list_quit = keybinds.list_keys_display(ListAction::Quit);
    let list_template = keybinds.list_keys_display(ListAction::NewFromTemplate);
    let list_create_folder = keybinds.list_keys_display(ListAction::CreateFolder);
    let list_rename_folder = keybinds.list_keys_display(ListAction::RenameFolder);
    let list_move_note = keybinds.list_keys_display(ListAction::MoveNote);
    let list_manage_tags = keybinds.list_keys_display(ListAction::ManageTags);
    let list_pin = keybinds.list_keys_display(ListAction::TogglePin);
    let list_select_mode = keybinds.list_keys_display(ListAction::ToggleSelectMode);
    let list_select_item = keybinds.list_keys_display(ListAction::ToggleSelectItem);
    let list_trash = keybinds.list_keys_display(ListAction::OpenTrash);
    let list_search = keybinds.list_keys_display(ListAction::Search);
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
        "Scroll Up / Down half page",
        Some(&format!("{list_page_up}/{list_page_down}")),
        theme,
    ));
    lines.extend(help_item_dyn("Toggle pin note", Some(&list_pin), theme));
    lines.extend(help_item_dyn(
        "Toggle select mode",
        Some(&list_select_mode),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Select / deselect item",
        Some(&list_select_item),
        theme,
    ));
    lines.extend(help_item_dyn(
        "View / restore trash",
        Some(&list_trash),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle external editor mode",
        Some(&keybinds.list_keys_display(ListAction::ToggleExternalEditor)),
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
        format!(" Search Popup ({list_search})"),
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Search notes / Filter by tags",
        Some("Type query / tag"),
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
        format!(
            " Tag Popup ({})",
            keybinds.list_keys_display(ListAction::ManageTags)
        ),
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
        format!(
            " Template Popup ({})",
            keybinds.list_keys_display(ListAction::NewFromTemplate)
        ),
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
        format!(
            " Markdown Preview ({})",
            keybinds.list_keys_display(ListAction::TogglePreview)
        ),
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Toggle preview pane in notes list",
        None,
        theme,
    ));
    lines.extend(help_item_dyn(
        "Expand preview to full width / restore",
        Some(&keybinds.list_keys_display(ListAction::TogglePreviewFullscreen)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle word-wrap in preview",
        Some(&keybinds.list_keys_display(ListAction::TogglePreviewWrap)),
        theme,
    ));
    lines.push(Line::from(vec![Span::styled(
        format!(
            " Calendar ({})",
            keybinds.list_keys_display(ListAction::ToggleCalendar)
        ),
        Style::default().fg(theme.heading),
    )]));
    lines.extend(help_item_dyn(
        "Toggle month calendar (note activity) in notes list",
        None,
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
    let _edit_quit = keybinds.edit_keys_display(EditAction::Quit);
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
    let edit_fullscreen = keybinds.edit_keys_display(EditAction::TogglePreviewFullscreen);
    let edit_wrap = keybinds.edit_keys_display(EditAction::TogglePreviewWrap);

    let mut lines = Vec::new();
    lines.push(help_heading("Editor", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Change focus (Title, Content)",
        Some(&edit_focus),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Return to notes (auto-saved on exit)",
        Some(&edit_back),
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
    lines.extend(help_item_dyn(
        "Expand preview to full width / restore",
        Some(&edit_fullscreen),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle word-wrap in preview",
        Some(&edit_wrap),
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
    lines.extend(help_item_dyn("Open filter menu", Some("f"), theme));
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
    lines.extend(help_item_dyn("Show/Hide legend", Some("L"), theme));
    lines.extend(help_item_dyn("Show/Hide minimap", Some("M"), theme));
    lines.extend(help_item_dyn("Show/Hide grid", Some("G"), theme));
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

fn draw_help_text(keybinds: &Keybinds, theme: &crate::app_theme::AppThemeColors) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(help_heading("Tools", theme));
    lines.extend(help_item_dyn(
        "Draw freehand strokes",
        Some(&keybinds.draw_keys_display(DrawAction::SelectDrawTool)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Shape tool (opens picker)",
        Some(&keybinds.draw_keys_display(DrawAction::ToggleShapeSelector)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Place text label at click position",
        Some(&keybinds.draw_keys_display(DrawAction::SelectTextTool)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Erase elements (hover + click/drag)",
        Some(&keybinds.draw_keys_display(DrawAction::SelectEraseTool)),
        theme,
    ));

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
    lines.extend(help_item_dyn(
        "Exit canvas view",
        Some(&keybinds.draw_keys_display(DrawAction::Quit)),
        theme,
    ));
    Text::from(lines)
}

fn canvas_help_text(
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(help_heading("Navigation", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Move selection",
        Some(&format!(
            "{}/{}/{}/{}",
            keybinds.canvas_keys_display(CanvasAction::MoveLeft),
            keybinds.canvas_keys_display(CanvasAction::MoveRight),
            keybinds.canvas_keys_display(CanvasAction::MoveUp),
            keybinds.canvas_keys_display(CanvasAction::MoveDown),
        )),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Zoom in",
        Some(&keybinds.canvas_keys_display(CanvasAction::ZoomIn)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Zoom out",
        Some(&keybinds.canvas_keys_display(CanvasAction::ZoomOut)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Zoom in (fine)",
        Some(&keybinds.canvas_keys_display(CanvasAction::ZoomFineIn)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Zoom out (fine)",
        Some(&keybinds.canvas_keys_display(CanvasAction::ZoomFineOut)),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(help_heading("Editing", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Open / edit selected node",
        Some(&keybinds.canvas_keys_display(CanvasAction::EditOrConnect)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Connect two nodes",
        Some(&keybinds.canvas_keys_display(CanvasAction::EditOrConnect)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Context menu",
        Some(&keybinds.canvas_keys_display(CanvasAction::OpenContextMenu)),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(help_heading("Interface", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Toggle grid",
        Some(&keybinds.canvas_keys_display(CanvasAction::ToggleGrid)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle editor pane",
        Some(&keybinds.canvas_keys_display(CanvasAction::ToggleEditorPane)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Focus editor pane",
        Some(&keybinds.canvas_keys_display(CanvasAction::CycleFocus)),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(help_heading("Editor (focused)", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Exit editor focus",
        Some(&keybinds.canvas_keys_display(CanvasAction::EditorUnfocus)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Save raw editor changes",
        Some(&keybinds.canvas_keys_display(CanvasAction::EditorSyncRaw)),
        theme,
    ));
    lines.push(Line::from(""));
    lines.push(help_heading("General", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Save canvas file",
        Some(&keybinds.canvas_keys_display(CanvasAction::Save)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Cancel connection",
        Some(&keybinds.canvas_keys_display(CanvasAction::Quit)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Exit canvas view",
        Some(&keybinds.canvas_keys_display(CanvasAction::Quit)),
        theme,
    ));
    Text::from(lines)
}

fn backup_help_text(
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(help_heading("Backup View", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Scroll Up / Down",
        Some(&format!(
            "{}/{}",
            keybinds.backup_keys_display(BackupAction::MoveUp),
            keybinds.backup_keys_display(BackupAction::MoveDown)
        )),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Refresh status",
        Some(&keybinds.backup_keys_display(BackupAction::Refresh)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Commit changes",
        Some(&keybinds.backup_keys_display(BackupAction::EnterCommit)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Push to remote",
        Some(&keybinds.backup_keys_display(BackupAction::Push)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Open settings",
        Some(&keybinds.backup_keys_display(BackupAction::OpenSettings)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Cycle sections",
        Some(&keybinds.backup_keys_display(BackupAction::CycleSection)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Back to list",
        Some(&keybinds.backup_keys_display(BackupAction::Back)),
        theme,
    ));
    Text::from(lines)
}

fn content_tree_help_text(
    keybinds: &Keybinds,
    theme: &crate::app_theme::AppThemeColors,
) -> Text<'static> {
    use crate::keybinds::ContentTreeAction;
    let mut lines = Vec::new();
    lines.push(help_heading("Content Tree", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Move Up / Down",
        Some(&format!(
            "{}/{}",
            keybinds.content_tree_keys_display(ContentTreeAction::MoveUp),
            keybinds.content_tree_keys_display(ContentTreeAction::MoveDown)
        )),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle Collapse",
        Some(&keybinds.content_tree_keys_display(ContentTreeAction::ToggleCollapse)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Expand All",
        Some(&keybinds.content_tree_keys_display(ContentTreeAction::ExpandAll)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Collapse All",
        Some(&keybinds.content_tree_keys_display(ContentTreeAction::CollapseAll)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Jump to section",
        Some(&keybinds.content_tree_keys_display(ContentTreeAction::Open)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Back to previous view",
        Some(&keybinds.content_tree_keys_display(ContentTreeAction::Back)),
        theme,
    ));
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
                .map(|k| format!("<{k}>"))
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

/// Compute the list / preview / calendar layout rectangles for the notes view.
///
/// Returns `(list_area, preview_area, calendar_area)`. The optional month
/// calendar always sits at the bottom of the **list** column only; the preview
/// column keeps the full content height, so the calendar never cuts through or
/// under the preview pane.
pub(crate) fn list_view_layout(
    area: Rect,
    preview_enabled: bool,
    preview_position: crate::config::PreviewPosition,
    calendar_enabled: bool,
    preview_fullscreen: bool,
) -> (Rect, Option<Rect>, Option<Rect>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    if preview_fullscreen {
        return (chunks[1], Some(chunks[1]), None);
    }

    // Horizontal split into list + preview columns. The preview column keeps
    // the full content height regardless of the calendar.
    let (list_column, preview_area) = if preview_enabled {
        let (constraints, list_idx, p_idx) = match preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(43, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(43, 100),
                ],
                0,
                2,
            ),
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(chunks[1]);
        (full_cols[list_idx], Some(full_cols[p_idx]))
    } else {
        (
            Rect::new(area.x, area.y + 1, area.width, chunks[1].height),
            None,
        )
    };

    // The optional month calendar sits at the bottom of the LIST column only.
    let (list_area, calendar_area) = if calendar_enabled {
        let sp = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(9)])
            .split(list_column);
        (sp[0], Some(sp[1]))
    } else {
        (list_column, None)
    };

    (list_area, preview_area, calendar_area)
}

pub fn draw_list_view(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    if app.preview_fullscreen {
        let preview_info = get_preview_info(app);
        draw_view_title_bar(frame, chunks[0], "Notes", &app.app_theme, preview_info);
    } else if app.list.notes_layout == crate::config::NotesLayout::Grid {
        let tabs = [("Vault", Some("\u{f07b}")), ("Pinned", Some("\u{f4cc}"))];
        let is_pinned = app.list.grid_folder == crate::app::VIRTUAL_PINNED_PATH;
        let tab_spans = build_tab_spans(
            &tabs,
            if is_pinned { 1 } else { 0 },
            &app.app_theme,
            app.config.ui.tab_icons_only,
        );
        draw_view_title_bar_with_tabs(frame, chunks[0], "Notes", tab_spans, &app.app_theme);

        // Show details of the selected note at the top right (clock/relative time + tags)
        if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
            app.list.visual_list.get(app.list.visual_index)
        {
            let s = &app.notes[*summary_idx];
            let mut spans = Vec::new();

            let when = format_relative_time(s.updated_at);
            spans.push(Span::styled(
                " \u{f017} ",
                Style::default().fg(app.app_theme.muted),
            ));
            spans.push(Span::styled(
                when.into_owned(),
                Style::default().fg(app.app_theme.muted),
            ));

            if !s.tags.is_empty() {
                spans.push(Span::styled(
                    "  \u{f02b} ",
                    Style::default()
                        .fg(app.app_theme.tag)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    s.tags.join(", "),
                    Style::default().fg(app.app_theme.fg),
                ));
            }
            spans.push(Span::raw(" ")); // padding right

            let detail_para = Paragraph::new(Line::from(spans)).alignment(Alignment::Right);
            frame.render_widget(detail_para, chunks[0]);
        } else if let Some(crate::app::VisualItem::Folder {
            name, note_count, ..
        }) = app.list.visual_list.get(app.list.visual_index)
            && name != ".."
        {
            let mut spans = Vec::new();
            let suffix = if *note_count == 1 { "note" } else { "notes" };
            spans.push(Span::styled(
                " \u{f0ca} ",
                Style::default().fg(app.app_theme.folder),
            ));
            spans.push(Span::styled(
                format!("{note_count} {suffix}"),
                Style::default().fg(app.app_theme.fg),
            ));
            spans.push(Span::raw(" ")); // padding right

            let detail_para = Paragraph::new(Line::from(spans)).alignment(Alignment::Right);
            frame.render_widget(detail_para, chunks[0]);
        }
    } else {
        draw_view_title_bar(frame, chunks[0], "Notes", &app.app_theme, None);
    }

    let (list_area, preview_area, calendar_area) = list_view_layout(
        area,
        app.list.preview_enabled,
        app.preview_position,
        app.list.calendar_enabled,
        app.preview_fullscreen,
    );
    if let Some(p) = preview_area {
        app.list.last_preview_pane_width = p.width;
    }

    if !app.preview_fullscreen {
        let is_grid = app.list.notes_layout == crate::config::NotesLayout::Grid;
        let mut items: Vec<ListItem> = Vec::new();

        if is_grid {
            app.list.grid_tiles.clear();

            // --- render directory breadcrumbs at the top of the list area ---
            let is_pinned = app.list.grid_folder == crate::app::VIRTUAL_PINNED_PATH;
            let mut spans = Vec::new();
            if is_pinned {
                spans.push(Span::styled(
                    " \u{f4cc} Pinned",
                    Style::default()
                        .fg(app.app_theme.heading)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    " \u{f07b} Vault",
                    Style::default()
                        .fg(app.app_theme.folder)
                        .add_modifier(Modifier::BOLD),
                ));
                if !app.list.grid_folder.is_empty() {
                    for part in app.list.grid_folder.split('/') {
                        spans.push(Span::styled(
                            " / ",
                            Style::default().fg(app.app_theme.muted),
                        ));
                        spans.push(Span::styled(
                            part.to_string(),
                            Style::default().fg(app.app_theme.fg),
                        ));
                    }
                }
            }
            let dir_rect = Rect::new(list_area.x, list_area.y + 1, list_area.width, 1);
            frame.render_widget(Paragraph::new(Line::from(spans)), dir_rect);

            // --- columns / visible rows ---
            let cols = ((list_area.width.saturating_sub(GRID_LEFT_MARGIN + GRID_GAP))
                / (GRID_TILE_W + GRID_GAP))
                .max(1) as usize;
            let rows = ((list_area.height.saturating_sub(GRID_TOP_MARGIN + GRID_GAP))
                / (GRID_TILE_H + GRID_GAP)) as usize;
            app.list.grid_columns = cols; // events.rs grid nav reads this (Up/Down move by cols)

            let len = app.list.visual_list.len();

            // --- clamp grid_scroll so the selected tile stays visible, without over-scrolling ---
            if cols > 0 && rows > 0 && len > 0 {
                let sel_row = app.list.visual_index / cols;
                if sel_row < app.list.grid_scroll {
                    app.list.grid_scroll = sel_row;
                }
                let last_visible = app.list.grid_scroll + rows.saturating_sub(1);
                if sel_row > last_visible {
                    app.list.grid_scroll = sel_row.saturating_sub(rows.saturating_sub(1));
                }
                let max_scroll = (len - 1) / cols;
                if app.list.grid_scroll > max_scroll {
                    app.list.grid_scroll = max_scroll;
                }
            } else {
                app.list.grid_scroll = 0;
            }

            let start = app.list.grid_scroll * cols;
            let count = (rows * cols).min(len.saturating_sub(start));
            let buf = frame.buffer_mut();

            for i in 0..count {
                let vi = start + i;
                if vi >= len {
                    break;
                }
                let row = i / cols;
                let col = i % cols;
                let tile_rect = ratatui::layout::Rect::new(
                    list_area.x + GRID_LEFT_MARGIN + (col as u16) * (GRID_TILE_W + GRID_GAP),
                    list_area.y + GRID_TOP_MARGIN + (row as u16) * (GRID_TILE_H + GRID_GAP),
                    GRID_TILE_W,
                    GRID_TILE_H,
                );
                let is_selected = vi == app.list.visual_index;

                // --- resolve (icon char, glyph color, display name): SAME mapping the old code used ---
                let item = &app.list.visual_list[vi];
                let (icon_char, glyph_color, raw_name) = match item {
                    crate::app::VisualItem::Folder { name, .. } => {
                        let is_pinned = name == crate::app::VIRTUAL_PINNED_LABEL;
                        let is_parent = name == "..";
                        let ic = if is_pinned {
                            '\u{f4cc}'
                        } else if is_parent {
                            '\u{f062}' // Arrow Up ()
                        } else {
                            '\u{f07b}' // Folder ()
                        };
                        let col = if is_pinned {
                            app.app_theme.heading
                        } else {
                            app.app_theme.folder
                        };
                        (ic, col, name.clone())
                    }
                    crate::app::VisualItem::Note {
                        summary_idx,
                        is_clin,
                        is_draw,
                        is_canvas,
                        ..
                    } => {
                        let s = &app.notes[*summary_idx];
                        let col = if s.pinned {
                            app.app_theme.heading
                        } else if *is_clin {
                            app.app_theme.destructive
                        } else if *is_draw {
                            app.app_theme.success
                        } else if *is_canvas {
                            app.app_theme.accent
                        } else {
                            app.app_theme.text
                        };
                        let ic = if s.pinned {
                            '\u{f4cc}'
                        } else if *is_clin {
                            '\u{f023}'
                        } else if *is_draw {
                            '\u{f1fc}'
                        } else if *is_canvas {
                            '\u{f005}'
                        } else {
                            '\u{f15c}'
                        };
                        (ic, col, s.title.clone())
                    }
                    crate::app::VisualItem::CreateNew { .. } => {
                        ('\u{f067}', app.app_theme.success, "Create...".to_string())
                    }
                };

                // --- tile border (plain border = "button") ---
                let mut block = Block::default().borders(Borders::ALL);
                if is_selected {
                    block = block.border_style(Style::default().fg(app.app_theme.highlight_bg));
                } else {
                    block = block.border_style(Style::default().fg(app.app_theme.border));
                }
                let inner = block.inner(tile_rect);
                block.render(tile_rect, buf); // paints border

                // --- icon: centered on the top inner row ---
                let icon_style = if is_selected {
                    Style::default()
                        .fg(glyph_color)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(glyph_color)
                };
                let inner_w = inner.width as usize; // GRID_TILE_W - 2 = 10
                let icon_x = inner.x + (inner_w.saturating_sub(1) / 2) as u16; // center the 1-wide glyph
                if let Some(cell) = buf.cell_mut((icon_x, inner.y)) {
                    cell.set_char(icon_char).set_style(icon_style);
                }

                // --- tag icon: top right corner for items that have tags ---
                let has_tags = match item {
                    crate::app::VisualItem::Note { summary_idx, .. } => {
                        !app.notes[*summary_idx].tags.is_empty()
                    }
                    _ => false,
                };
                if has_tags {
                    let tag_x = inner.x + inner.width.saturating_sub(1);
                    if let Some(cell) = buf.cell_mut((tag_x, inner.y)) {
                        let tag_style = if is_selected {
                            Style::default()
                                .fg(app.app_theme.tag)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.app_theme.tag)
                        };
                        cell.set_char('\u{f02b}').set_style(tag_style);
                    }
                }

                // --- name: sanitize, truncate to inner width, center, write on the bottom row (row 2) ---
                let sanitized = crate::sanitize::sanitize_for_terminal(&raw_name);
                let mut chars: Vec<char> = sanitized.chars().collect();
                if chars.len() > inner_w {
                    chars.truncate(inner_w - 1);
                    chars.push('…');
                }
                let pad = inner_w.saturating_sub(chars.len());
                let left = pad / 2;
                let name_style = if is_selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let mut name_string: String = " ".repeat(left);
                name_string.extend(chars.iter());
                let name_row = inner.y + 2; // bottom row of the 3 inner rows
                for (k, ch) in name_string.chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((inner.x + k as u16, name_row)) {
                        cell.set_char(ch).set_style(name_style);
                    }
                }

                // --- record tile for mouse hit-testing ---
                app.list.grid_tiles.push(crate::list_view::GridTile {
                    visual_index: vi,
                    rect: tile_rect,
                });
            }
            // do NOT render a List widget here; do NOT touch list_state (tree view still uses it).
        } else {
            items.reserve(app.list.display_items.len());
            for item in &app.list.display_items {
                items.push(item.clone());
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
        }

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
    }
    if let Some(preview_rect) = preview_area {
        let hide_encrypted = app.preview_encryption
            && app
                .list
                .visual_list
                .get(app.list.visual_index)
                .is_some_and(|item| {
                    matches!(item, crate::app::VisualItem::Note { is_clin: true, .. })
                });

        let content_is_current = app.list.preview_content_index == Some(app.list.visual_index);
        let content = if content_is_current || app.list.pending_preview_update {
            app.list.preview_content.as_ref()
        } else {
            None
        };

        crate::preview::draw_preview_pane(
            frame,
            preview_rect,
            &app.app_theme,
            content,
            hide_encrypted,
            app.list.snapshot_scroll_offset,
        );
    }
    if let Some(cal_rect) = calendar_area {
        if app.config.goals.enabled && cal_rect.width >= 42 {
            let layout_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(25), Constraint::Min(0)])
                .split(cal_rect);
            crate::calendar::draw_calendar(frame, layout_chunks[0], &app.app_theme, &app.notes);
            let _ = app.get_current_goals_progress();
            crate::goals::draw_goals_progress(
                frame,
                layout_chunks[1],
                &app.app_theme,
                &app.goals_progress,
                &app.config.goals,
            );
        } else {
            crate::calendar::draw_calendar(frame, cal_rect, &app.app_theme, &app.notes);
        }
    }

    let hint = resolved_status_hint(app, LIST_HELP_HINTS);
    let badge = Some(ext_badge(
        app.editor.external_editor_enabled,
        &app.app_theme,
    ));
    draw_status_bar(frame, chunks[2], &app.app_theme, badge, &hint, None);
    draw_corner_watermark(frame, chunks[2], app.app_theme.muted);
    if app.list.preview_enabled && !app.preview_fullscreen {
        let constraints = match app.preview_position {
            crate::config::PreviewPosition::Left => [
                Constraint::Ratio(43, 100),
                Constraint::Length(1),
                Constraint::Min(0),
            ],
            crate::config::PreviewPosition::Right => [
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Ratio(43, 100),
            ],
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(chunks[1]);
        draw_dim_vline(frame, full_cols[1], app.app_theme.muted);
    }

    if let Some(popup) = &app.popups.template {
        draw_template_popup(frame, popup, area, &app.app_theme);
    }

    if let Some(popup) = &mut app.popups.folder {
        let title = match popup.mode {
            crate::popups::FolderPopupMode::Create { .. } => "NEW FOLDER",
            crate::popups::FolderPopupMode::Rename { .. } => "RENAME FOLDER",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Prompt,
            "Enter confirm · Esc cancel",
            &app.app_theme,
        );

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.heading)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.tag {
        let suggestion_height = if popup.suggestions.is_empty() {
            0u16
        } else {
            (popup.suggestions.len() as u16).clamp(1, 5)
        };
        let content = draw_popup_frame(
            frame,
            area,
            "TAGS",
            PopupSize::Large,
            "Ctrl+S batch assign · Tab accept · Enter save · d delete from all · Esc cancel",
            &app.app_theme,
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3u16 + suggestion_height),
                Constraint::Min(3),
            ])
            .split(content);

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
            .title("");
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
                    ListItem::new(format!("  {tag}")).style(style)
                })
                .collect();

            let suggestions_list = List::new(suggestion_items)
                .block(
                    Block::default()
                        .borders(Borders::NONE)
                        .style(app.app_theme.bg_style()),
                )
                .highlight_style(Style::default());

            frame.render_widget(suggestions_list, input_chunks[1]);
        }

        let all_tags_border = if popup.focus == crate::popups::TagPopupFocus::AllTagsList {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let tag_empty = popup.all_tags.is_empty();
        let tag_items: Vec<ListItem> = if tag_empty {
            vec![ListItem::new(Span::styled(
                "No tags found",
                Style::default().fg(app.app_theme.muted),
            ))]
        } else {
            popup
                .all_tags
                .iter()
                .map(|tag| ListItem::new(tag.to_string()))
                .collect()
        };

        let tags_list = build_list_widget(tag_items, &app.app_theme)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(all_tags_border),
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
    }

    if let Some(picker) = &mut app.popups.folder_picker {
        let title = match picker.mode {
            crate::popups::FolderPickerMode::CopyNote { .. } => "COPY",
            _ => "MOVE",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Large,
            "Tab switch  Enter move  Esc cancel",
            &app.app_theme,
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(content);

        let search_border = if picker.focus == crate::app::FolderPickerFocus::Search {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        picker.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(search_border)
                .title(""),
        );
        frame.render_widget(&picker.input, chunks[0]);

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

        let _title = match &picker.mode {
            crate::app::FolderPickerMode::MoveNote { .. } => "Move note to folder".to_string(),
            crate::app::FolderPickerMode::CopyNote { .. } => "Copy note to folder".to_string(),
            crate::app::FolderPickerMode::MoveFolder { folder_path } => {
                let folder_name = folder_path.rsplit('/').next().unwrap_or(folder_path);
                format!("Move '{folder_name}' folder to")
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
                    .title(""),
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
    }

    if let Some(palette) = &mut app.command_palette {
        let content = draw_popup_frame(
            frame,
            area,
            "COMMANDS",
            PopupSize::Large,
            "Tab category · Enter run · ↑/↓ select · Esc close",
            &app.app_theme,
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // search input
                Constraint::Length(1), // tab bar
                Constraint::Min(0),    // results list
            ])
            .split(content);

        palette.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.muted))
                .title(""),
        );
        frame.render_widget(&palette.input, chunks[0]);

        let tabs: Vec<(&str, Option<&str>)> = crate::palette::PALETTE_TABS
            .iter()
            .map(|(l, g, _)| (*l, Some(*g)))
            .collect();
        let tab_spans = build_tab_spans(
            &tabs,
            palette.active_tab,
            &app.app_theme,
            app.config.ui.tab_icons_only,
        );
        let tabs_w = Paragraph::new(Line::from(tab_spans))
            .alignment(Alignment::Center)
            .style(app.app_theme.hint_line_bg_style());
        frame.render_widget(tabs_w, chunks[1]);

        let items: Vec<ListItem> = palette
            .items
            .iter()
            .map(|item| {
                let mut spans = vec![Span::styled(
                    format!("{} ", &item.glyph),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                )];
                spans.extend(style_palette_name(&item.name, &app.app_theme));
                ListItem::new(vec![
                    Line::from(spans),
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
                    .title(""),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        frame.render_stateful_widget(list, chunks[2], &mut palette.state);
    }

    if let Some(popup) = &mut app.popups.note_rename {
        let content = draw_popup_frame(
            frame,
            area,
            "RENAME",
            PopupSize::Prompt,
            "Enter rename · Esc cancel",
            &app.app_theme,
        );

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.heading)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.goals {
        let (title, sub) = match popup.mode {
            crate::popups::GoalsPopupMode::WordGoal => {
                ("DAILY WORD GOAL", "Enter word count · Esc cancel")
            }
            crate::popups::GoalsPopupMode::NoteGoal => {
                ("DAILY NOTE GOAL", "Enter note count · Esc cancel")
            }
        };
        let content = draw_popup_frame(frame, area, title, PopupSize::Prompt, sub, &app.app_theme);

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.heading)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some((popup, format)) = &mut app.popups.create_note {
        let title = match format {
            crate::popups::NoteFormat::Markdown => "NEW NOTE",
            crate::popups::NoteFormat::Draw => "NEW DRAWING",
            crate::popups::NoteFormat::Canvas => "NEW CANVAS",
            crate::popups::NoteFormat::PlainText => "NEW TEXT FILE",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Prompt,
            "Enter create · Esc cancel",
            &app.app_theme,
        );
        popup.input.set_block(popup_block("", &app.app_theme));
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.import {
        let title = match popup.source {
            crate::popups::ImportSource::File => "IMPORT FILE",
            crate::popups::ImportSource::Csv => "IMPORT CSV/TSV",
            crate::popups::ImportSource::Json => "IMPORT JSON",
            crate::popups::ImportSource::Url => "IMPORT URL",
            crate::popups::ImportSource::Clipboard => "IMPORT CLIPBOARD",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Large,
            "Enter import · Esc cancel",
            &app.app_theme,
        );
        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.muted)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.search {
        let content = draw_popup_frame(
            frame,
            area,
            "SEARCH",
            PopupSize::Large,
            "Tab switch · Enter open · Esc cancel · f:folder p:pinned t:tag g:text · \\e\\ escapes filters",
            &app.app_theme,
        );

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
            ]
        } else {
            vec![Constraint::Length(3), Constraint::Min(3)]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(content);

        if has_filter {
            let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
            let mut first = true;

            let add_sep = |spans: &mut Vec<Span<'static>>,
                           first: &mut bool,
                           theme: &crate::app_theme::AppThemeColors| {
                if !*first {
                    spans.push(Span::styled(
                        " · ",
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                *first = false;
            };

            if let Some(ref f) = parsed.folder_filter {
                let text = if f.is_empty() { "Vault" } else { f.as_str() };
                add_sep(&mut spans, &mut first, &app.app_theme);
                spans.push(Span::styled(
                    "\u{f07c} ",
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if parsed.pinned_only {
                add_sep(&mut spans, &mut first, &app.app_theme);
                spans.push(Span::styled(
                    "\u{f08d} Pinned",
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
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
                    format!("\u{f002} {grep_display}"),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
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
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    tag_text,
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            let filter_line = Line::from(spans);
            let filter_para = Paragraph::new(filter_line).style(app.app_theme.bg_style());
            frame.render_widget(filter_para, chunks[1]);
        }

        let input_chunk = chunks[0];
        let results_chunk = if has_filter { chunks[2] } else { chunks[1] };

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(if popup.focus == crate::popups::SearchFocus::Input {
                    Style::default().fg(app.app_theme.heading)
                } else {
                    Style::default().fg(app.app_theme.muted)
                })
                .title(""),
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
            let mut visible: Vec<(usize, String)> = Vec::new();
            let mut i = 0;
            while i < popup.grep_results.len() {
                let is_collapsed = popup.grep_is_header[i] && !popup.grep_expanded.contains(&i);
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
            let items: Vec<ListItem> = visible
                .iter()
                .map(|(_, t)| ListItem::new(styled_result_line(t, &app.app_theme)))
                .collect();
            (items, "")
        } else if has_title {
            let items: Vec<ListItem> = popup
                .title_results
                .iter()
                .map(|entry| ListItem::new(styled_result_line(entry, &app.app_theme)))
                .collect();
            (items, "")
        } else {
            let msg = if query_text.trim().is_empty() && !has_filter {
                "Type to search notes"
            } else {
                "No results"
            };
            (
                vec![ListItem::new(Span::styled(
                    msg.to_string(),
                    Style::default().fg(app.app_theme.muted),
                ))],
                "",
            )
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
            let mut vis_pos = 0;
            let mut i = 0;
            while i < popup.grep_results.len() && i <= popup.grep_selected {
                let is_collapsed = popup.grep_is_header[i] && !popup.grep_expanded.contains(&i);
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
    }

    if let Some(trash) = &app.popups.trash_view {
        let content = draw_popup_frame(
            frame,
            area,
            "TRASH",
            PopupSize::Large,
            "r restore · d delete · E empty · q close",
            &app.app_theme,
        );

        let border_color = if trash.items.is_empty() {
            app.app_theme.muted
        } else {
            app.app_theme.heading
        };

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
                    .border_style(Style::default().fg(border_color))
                    .title(""),
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

        frame.render_stateful_widget(list, content, &mut state);
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
    let content = draw_popup_frame(
        frame,
        area,
        "TEMPLATES",
        PopupSize::Large,
        "Tab switch · Enter use template · n create template · d delete · Space edit · ? help · Esc cancel",
        theme,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(content);

    let mut input = popup.input.clone();
    input.set_style(theme.bg_style());
    input.set_block(
        Block::default()
            .style(theme.bg_style())
            .borders(Borders::ALL)
            .border_style(
                if popup.focus == crate::popups::TemplatePopupFocus::Search {
                    Style::default().fg(theme.heading)
                } else {
                    Style::default().fg(theme.muted)
                },
            )
            .title(""),
    );
    frame.render_widget(&input, chunks[0]);

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
}

pub fn draw_theme_popup(
    frame: &mut Frame,
    popup: &ThemePopup,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
    let content = draw_popup_frame(
        frame,
        area,
        "THEMES",
        PopupSize::Medium,
        "Tab navigate · Enter select · Esc close",
        theme,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(content);

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
}

pub fn draw_sort_popup(
    frame: &mut Frame,
    popup: &crate::popups::SortPopup,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
    let content_area = draw_popup_frame(
        frame,
        area,
        "SORT BY",
        PopupSize::Medium,
        "↑↓: Navigate • Enter: Select • Esc: Cancel",
        theme,
    );

    let options = [
        "Title (A-Z)",
        "Title (Z-A)",
        "Modified (newest)",
        "Modified (oldest)",
    ];
    let items: Vec<ListItem> = options
        .iter()
        .map(|&opt| ListItem::new(Line::from(Span::raw(opt))))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(popup.selected));
    frame.render_stateful_widget(list, content_area, &mut state);
}

pub fn draw_create_format_popup(
    frame: &mut Frame,
    popup: &crate::popups::CreateFormatPopup,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
    let content_area = draw_popup_frame(
        frame,
        area,
        "CREATE NEW",
        PopupSize::Medium,
        "↑↓: Navigate • Enter: Select • Esc: Cancel",
        theme,
    );

    let options = [
        "Markdown Note (.md)",
        "Plain Text (.txt)",
        "Drawing (.draw)",
        "Canvas (.canvas)",
    ];
    let items: Vec<ListItem> = options
        .iter()
        .map(|&opt| ListItem::new(Line::from(Span::raw(opt))))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(popup.selected));
    frame.render_stateful_widget(list, content_area, &mut state);
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
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let preview_info = get_preview_info(app);
    draw_view_title_bar(
        frame,
        outer_chunks[0],
        "Editor",
        &app.app_theme,
        preview_info,
    );
    let body_area = outer_chunks[1];
    let hint_area = outer_chunks[2];

    let (edit_area, preview_area_rect, splitter_area) = if app.preview_fullscreen {
        app.editor.last_preview_pane_width = body_area.width;
        (body_area, Some(body_area), None)
    } else if app.editor.editor_preview_enabled {
        let (constraints, main_idx, p_idx) = match app.preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(43, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(43, 100),
                ],
                0,
                2,
            ),
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(body_area);
        app.editor.last_preview_pane_width = cols[p_idx].width;
        (cols[main_idx], Some(cols[p_idx]), Some(cols[1]))
    } else {
        (body_area, None, None)
    };

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(edit_area);

    let title_area = inner_chunks[0];
    let editor_container = inner_chunks[1];

    if !app.preview_fullscreen {
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
    }

    if let Some(preview_area_rect) = preview_area_rect {
        let content_area = editor_container;

        if !app.preview_fullscreen {
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
        }

        match &app.editor.md_preview_renderer {
            Some(renderer) if !renderer.is_pending() && renderer.pages_built() => {
                if let Some(page_grid) = renderer.current_page_grid() {
                    let snapshot = crate::snapshot::RenderedSnapshot::new(page_grid).block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                    frame.render_widget(snapshot, preview_area_rect);
                    if renderer.total_pages() > 1 {
                        let indicator = format!(
                            " {}/{} ",
                            renderer.current_page() + 1,
                            renderer.total_pages()
                        );
                        let ind_width = indicator.len() as u16;
                        let ind_x = preview_area_rect.right().saturating_sub(ind_width + 2);
                        let ind_y = preview_area_rect.bottom().saturating_sub(1);
                        if ind_x >= preview_area_rect.x && ind_y >= preview_area_rect.y {
                            let ind_area = Rect::new(ind_x, ind_y, ind_width, 1);
                            let ind_widget = Paragraph::new(Span::styled(
                                indicator,
                                Style::default()
                                    .fg(app.app_theme.muted)
                                    .add_modifier(Modifier::DIM),
                            ));
                            frame.render_widget(ind_widget, ind_area);
                        }
                    }
                }
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

    let hint = resolved_status_hint(app, EDIT_HELP_HINTS);
    draw_status_bar(frame, hint_area, &app.app_theme, None, &hint, None);
    draw_corner_watermark(frame, hint_area, app.app_theme.muted);
    if let Some(splitter_area) = splitter_area {
        draw_dim_vline(frame, splitter_area, app.app_theme.muted);
    }

    if app.status.starts_with("Save failed") || app.status.starts_with("Could not open") {
        let popup = centered_rect(PopupSize::Small, area);
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
        let labels = [" Copy ", " Cut ", " Paste ", " Select All "];
        let items: Vec<ListItem> = labels.iter().map(|l| ListItem::new(*l)).collect();
        let menu_width = labels.iter().map(|l| l.len() as u16).max().unwrap_or(0);
        let menu_height = labels.len() as u16;

        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.preview_bg_style())
                    .borders(Borders::NONE),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let x = menu.x.min(area.width.saturating_sub(menu_width));
        let y = menu.y.min(area.height.saturating_sub(menu_height));
        let menu_area = Rect::new(x, y, menu_width, menu_height);

        let mut state = ListState::default();
        state.select(Some(menu.selected));

        frame.render_widget(Clear, menu_area);
        frame.render_stateful_widget(list, menu_area, &mut state);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewHeaderInfo {
    pub path: String,
    pub item_name: String,
    pub prev_name: Option<String>,
    pub next_name: Option<String>,
}

fn get_item_name(app: &App, idx: usize) -> Option<String> {
    if let Some(item) = app.list.visual_list.get(idx) {
        match item {
            crate::list_view::VisualItem::Folder { name, .. } => Some(name.clone()),
            crate::list_view::VisualItem::Note { summary_idx, .. } => {
                app.notes.get(*summary_idx).map(|n| n.title.clone())
            }
            crate::list_view::VisualItem::CreateNew { .. } => Some("Create...".to_string()),
        }
    } else {
        None
    }
}

fn get_preview_info(app: &App) -> Option<PreviewHeaderInfo> {
    if !app.preview_fullscreen {
        return None;
    }

    let current_index = if app.mode == ViewMode::Edit {
        if let Some(id) = &app.editor.editing_id {
            if let Some(note_pos) = app.notes.iter().position(|n| &n.id == id) {
                app.list.visual_list.iter().position(|item| {
                    if let crate::list_view::VisualItem::Note { summary_idx, .. } = item {
                        *summary_idx == note_pos
                    } else {
                        false
                    }
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        Some(app.list.visual_index)
    };

    if app.mode == ViewMode::Edit {
        if let Some(id) = &app.editor.editing_id {
            if let Some(note) = app.notes.iter().find(|n| &n.id == id) {
                let folder = if note.folder.is_empty() {
                    "Vault".to_string()
                } else {
                    format!("Vault/{}", note.folder)
                };
                let title = get_title_text(&app.editor.title_editor).into_owned();
                let title = if title.is_empty() {
                    "Untitled note".to_string()
                } else {
                    title
                };
                let (prev_name, next_name) = if let Some(idx) = current_index {
                    let prev = if idx > 0 {
                        get_item_name(app, idx - 1)
                    } else {
                        None
                    };
                    let next = if idx + 1 < app.list.visual_list.len() {
                        get_item_name(app, idx + 1)
                    } else {
                        None
                    };
                    (prev, next)
                } else {
                    (None, None)
                };
                return Some(PreviewHeaderInfo {
                    path: folder,
                    item_name: title,
                    prev_name,
                    next_name,
                });
            }
        } else if let Some(path) = &app.editor.template_edit_path {
            let parent = path
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let folder = if parent.is_empty() {
                "Templates".to_string()
            } else {
                format!("Templates/{}", parent)
            };
            return Some(PreviewHeaderInfo {
                path: folder,
                item_name: filename,
                prev_name: None,
                next_name: None,
            });
        }
    }

    if let Some(item) = app.list.visual_list.get(app.list.visual_index) {
        let (path, item_name) = match item {
            crate::list_view::VisualItem::Folder { path, name, .. } => {
                if path.is_empty() {
                    ("Vault".to_string(), "Vault".to_string())
                } else if path == crate::app::VIRTUAL_PINNED_PATH {
                    ("Pinned".to_string(), "Pinned".to_string())
                } else {
                    if let Some(slash_idx) = path.rfind('/') {
                        let parent = &path[..slash_idx];
                        (format!("Vault/{}", parent), name.clone())
                    } else {
                        ("Vault".to_string(), name.clone())
                    }
                }
            }
            crate::list_view::VisualItem::Note { summary_idx, .. } => {
                if let Some(note) = app.notes.get(*summary_idx) {
                    let folder = if note.folder.is_empty() {
                        "Vault".to_string()
                    } else {
                        format!("Vault/{}", note.folder)
                    };
                    (folder, note.title.clone())
                } else {
                    return None;
                }
            }
            crate::list_view::VisualItem::CreateNew { path, .. } => {
                let folder = if path.is_empty() {
                    "Vault".to_string()
                } else {
                    format!("Vault/{}", path)
                };
                (folder, "Create...".to_string())
            }
        };

        let idx = app.list.visual_index;
        let prev_name = if idx > 0 {
            get_item_name(app, idx - 1)
        } else {
            None
        };
        let next_name = if idx + 1 < app.list.visual_list.len() {
            get_item_name(app, idx + 1)
        } else {
            None
        };

        Some(PreviewHeaderInfo {
            path,
            item_name,
            prev_name,
            next_name,
        })
    } else {
        None
    }
}

/// Renders a full-width view title bar. Title is left-aligned with popup-banner
/// styling: uppercase, bold, fg=highlight_fg, bg=heading. The rest of the row
/// fills with title_bar_bg.
pub fn draw_view_title_bar(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    theme: &AppThemeColors,
    preview_info: Option<PreviewHeaderInfo>,
) {
    let display_text = format!(" {} ", title.to_uppercase());
    let title_span = Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    );
    let mut spans = vec![title_span];
    if let Some(info) = preview_info {
        spans.push(Span::styled("  ", Style::default()));
        let parts: Vec<&str> = info.path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" / ", Style::default().fg(theme.fg)));
            }
            spans.push(Span::styled(
                part.to_string(),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::styled(" ❯ ", Style::default().fg(theme.accent)));
        spans.push(Span::styled(
            info.item_name,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));

        if info.prev_name.is_some() || info.next_name.is_some() {
            spans.push(Span::styled("  (", Style::default().fg(theme.fg)));
            let mut added = false;
            if let Some(prev) = info.prev_name {
                spans.push(Span::styled("prev: ", Style::default().fg(theme.fg)));
                spans.push(Span::styled(prev, Style::default().fg(theme.heading)));
                added = true;
            }
            if let Some(next) = info.next_name {
                if added {
                    spans.push(Span::styled(" · ", Style::default().fg(theme.fg)));
                }
                spans.push(Span::styled("next: ", Style::default().fg(theme.fg)));
                spans.push(Span::styled(next, Style::default().fg(theme.heading)));
            }
            spans.push(Span::styled(")", Style::default().fg(theme.fg)));
        }
    }
    let bar = Paragraph::new(Line::from(spans)).style(theme.title_bar_bg_style());
    frame.render_widget(bar, area);
}

/// Renders a full-width bar with a left-aligned title (popup-banner style)
/// and centered tab spans. Tabs are confined to the region right of the title
/// so they never overlap, even at narrow widths.
pub fn draw_view_title_bar_with_tabs(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    tab_spans: Vec<Span<'static>>,
    theme: &AppThemeColors,
) {
    // 1. Background fill across the whole row.
    frame.render_widget(Paragraph::new("").style(theme.title_bar_bg_style()), area);

    // 2. Tabs, centered (or left-aligned on overflow) inside the region right of
    //    the title. Width math mirrors `hit_test_tabs` so mouse hits stay aligned.
    let tabs_region = title_bar_tabs_region(area, title);
    let total: u16 = tab_spans
        .iter()
        .map(|s| s.content.chars().count() as u16)
        .fold(0u16, u16::saturating_add);
    let start_x = tabs_region.x + tabs_region.width.saturating_sub(total) / 2;
    let render_w = total.min(tabs_region.width);
    let tabs_area = Rect::new(start_x, area.y, render_w, area.height);
    frame.render_widget(
        Paragraph::new(Line::from(tab_spans)).style(theme.title_bar_bg_style()),
        tabs_area,
    );

    // 3. Title on the left, confined to its own cells so it can never reach the
    //    tab region. Styling unchanged from the original.
    let display_text = format!(" {} ", title.to_uppercase());
    let title_w = display_text.chars().count() as u16;
    let title_area = Rect::new(area.x, area.y, title_w.min(area.width), area.height);
    let title_span = Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(Paragraph::new(Line::from(vec![title_span])), title_area);
}

/// Sub-rect of a 1-row title-bar `area` reserved for the tab block: everything
/// to the right of the `" TITLE "` text. Used by both `draw_view_title_bar_with_tabs`
/// (render) and the `hit_test_tabs` callers (input) so the rendered and
/// hit-tested regions can never drift.
pub fn title_bar_tabs_region(area: Rect, title: &str) -> Rect {
    let title_w = format!(" {} ", title.to_uppercase())
        .chars()
        .count()
        .min(area.width as usize) as u16;
    Rect {
        x: area.x + title_w,
        y: area.y,
        width: area.width.saturating_sub(title_w),
        height: area.height,
    }
}

/// Display text for one tab. `icons_only` ⇒ glyph only; otherwise glyph + label.
/// Private — keep width in sync with `tab_display_width`.
fn tab_display_text(label: &str, glyph: Option<&str>, icons_only: bool) -> String {
    match (icons_only, glyph) {
        (true, Some(g)) => format!(" {g} "),
        (true, None) => format!(" {label} "),
        (false, Some(g)) => format!(" {g} {label} "),
        (false, None) => format!(" {label} "),
    }
}

/// Cell width of one tab's text. Must equal `tab_display_text(...).chars().count()`.
fn tab_display_width(label: &str, glyph: Option<&str>, icons_only: bool) -> u16 {
    let label_w = label.chars().count() as u16;
    match (icons_only, glyph) {
        (true, Some(g)) => 2 + g.chars().count() as u16, // " g "
        (true, None) => 2 + label_w,                     // " label "
        (false, Some(g)) => 3 + g.chars().count() as u16 + label_w, // " g label "
        (false, None) => 2 + label_w,                    // " label "
    }
}

/// Build tab spans in the unified Vault/Pinned style: the active tab is a filled
/// pill (highlight_fg on accent, bold), inactive tabs are muted, each pair of
/// tabs separated by a single space. Pass the result to the title bar (or any
/// centered Paragraph) and to `hit_test_tabs` unchanged.
pub fn build_tab_spans(
    tabs: &[(&str, Option<&str>)],
    active: usize,
    theme: &AppThemeColors,
    icons_only: bool,
) -> Vec<Span<'static>> {
    let active_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::BOLD);
    let mut spans = Vec::with_capacity(tabs.len() * 2);
    for (i, (label, glyph)) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = if i == active {
            active_style
        } else {
            inactive_style
        };
        spans.push(Span::styled(
            tab_display_text(label, *glyph, icons_only),
            style,
        ));
    }
    spans
}

/// Given the SAME `tabs` slice passed to `build_tab_spans`, the left edge and
/// width of the centered region the tabs render in, and a click column, return
/// the index of the clicked tab or `None` if the click is outside the tab row.
/// A click on a single-space gap counts as the tab to its right (matches the
/// existing Vault/Pinned behavior).
pub fn hit_test_tabs(
    tabs: &[(&str, Option<&str>)],
    region_x: u16,
    region_width: u16,
    click_x: u16,
    icons_only: bool,
) -> Option<usize> {
    let widths: Vec<u16> = tabs
        .iter()
        .map(|(l, g)| tab_display_width(l, *g, icons_only))
        .collect();
    let mut total: u16 = 0;
    for (i, w) in widths.iter().enumerate() {
        total = total.saturating_add(*w);
        if i + 1 < tabs.len() {
            total = total.saturating_add(1); // single-space separator
        }
    }
    let start_x = region_x + region_width.saturating_sub(total) / 2;
    if click_x < start_x || click_x >= start_x.saturating_add(total) {
        return None;
    }
    let mut offset = start_x;
    for (i, w) in widths.iter().enumerate() {
        if click_x < offset.saturating_add(*w) {
            return Some(i);
        }
        offset = offset.saturating_add(*w).saturating_add(1);
    }
    None
}

pub fn draw_popup_banner(frame: &mut Frame, popup_area: Rect, title: &str, theme: &AppThemeColors) {
    let display_text = format!(" {} ", title.to_uppercase());
    let width = display_text.len() as u16;
    if popup_area.y == 0 {
        return;
    }
    let banner_area = Rect::new(
        popup_area.x + (popup_area.width.saturating_sub(width)) / 2,
        popup_area.y - 1,
        width.min(popup_area.width),
        1,
    );
    frame.render_widget(Clear, banner_area);
    let p = Paragraph::new(Line::from(vec![Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    )]));
    frame.render_widget(p, banner_area);
}

pub fn centered_rect(size: PopupSize, area: Rect) -> Rect {
    let (width_pct, height_pct, max_w, max_h, fixed_h) = match size {
        PopupSize::Small => (40, 40, 60, 20, None),
        PopupSize::Medium => (50, 50, 80, 30, None),
        PopupSize::Large => (60, 60, 100, 40, None),
        PopupSize::Prompt => (50, 0, 80, 0, Some(5)),
        PopupSize::Confirm => (50, 0, 80, 0, Some(12)),
    };

    let width = (area.width * width_pct / 100).clamp(30.min(area.width), max_w.min(area.width));

    let height = if let Some(h) = fixed_h {
        h.min(area.height)
    } else {
        (area.height * height_pct / 100).clamp(5.min(area.height), max_h.min(area.height))
    };

    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub fn popup_block<'a>(title: &'a str, theme: &AppThemeColors) -> ratatui::widgets::Block<'a> {
    let mut block = ratatui::widgets::Block::default()
        .style(theme.bg_style())
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.heading));
    if !title.is_empty() {
        block = block.title(title);
    }
    block
}

pub fn build_list_widget<'a>(
    items: impl IntoIterator<Item = ratatui::widgets::ListItem<'a>>,
    theme: &AppThemeColors,
) -> ratatui::widgets::List<'a> {
    ratatui::widgets::List::new(items).highlight_style(
        ratatui::style::Style::default()
            .bg(theme.highlight_bg)
            .fg(theme.highlight_fg),
    )
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

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    }
}

pub struct StatusBarBadge {
    pub label: Cow<'static, str>,
    pub style: Style,
}

/// Build the `ext:on`/`ext:off` badge.
pub fn ext_badge(enabled: bool, theme: &AppThemeColors) -> StatusBarBadge {
    let label = if enabled { "ext:on" } else { "ext:off" };
    let style = if enabled {
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    StatusBarBadge {
        label: format!(" {label} ").into(),
        style,
    }
}

/// The single unified status bar. Left = optional badge + hint (muted);
/// right = optional right-aligned muted text (e.g. graf stats). Bg is always
/// theme.hint_line_bg_style(). Two Paragraphs on the same area (left Line +
/// right-aligned) do not collide because they occupy different cells.
pub fn draw_status_bar(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    badge: Option<StatusBarBadge>,
    hint: &str,
    right: Option<ratatui::text::Line<'static>>,
) {
    let mut left_spans: Vec<Span> = Vec::new();
    if let Some(b) = badge {
        left_spans.push(Span::styled(b.label, b.style));
        left_spans.push(Span::raw("  "));
    }
    left_spans.push(Span::styled(hint, Style::default().fg(theme.muted)));

    // If there's a right side, we split the area
    if let Some(right_line) = right {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(0),
                ratatui::layout::Constraint::Length(right_line.width() as u16),
            ])
            .split(area);

        let left_para = Paragraph::new(Line::from(left_spans)).style(theme.hint_line_bg_style());
        frame.render_widget(left_para, chunks[0]);

        let right_para = Paragraph::new(right_line)
            .alignment(Alignment::Right)
            .style(theme.hint_line_bg_style());
        frame.render_widget(right_para, chunks[1]);
    } else {
        let para = Paragraph::new(Line::from(left_spans)).style(theme.hint_line_bg_style());
        frame.render_widget(para, area);
    }
}

fn resolved_status_hint<'a>(app: &'a App, default: &'static str) -> Cow<'a, str> {
    let status = app.status.as_ref();
    if status != app.default_status_text() && !status.is_empty() && status != "Ready" {
        crate::sanitize::sanitize_for_terminal(status)
    } else {
        Cow::Borrowed(default)
    }
}

pub fn draw_popup_footer(
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

/// Renders standard popup chrome (clear + banner + footer) and returns the content Rect.
/// Caller splits and renders content into the returned Rect.
pub fn draw_popup_frame(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    size: PopupSize,
    hints: &str,
    theme: &crate::app_theme::AppThemeColors,
) -> Rect {
    let popup_area = centered_rect(size, area);
    frame.render_widget(Clear, popup_area);
    draw_popup_banner(frame, popup_area, title, theme);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(popup_area);
    draw_popup_footer(frame, chunks[1], theme, hints);
    chunks[0]
}

/// Renders confirmation popup chrome (clear + banner + bordered block) and returns the inner Rect.
/// Caller renders message/detail/buttons into the returned Rect.
pub fn draw_confirm_popup_frame(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    size: PopupSize,
    is_destructive: bool,
    theme: &crate::app_theme::AppThemeColors,
) -> Rect {
    let popup_area = centered_rect(size, area);
    frame.render_widget(Clear, popup_area);
    draw_popup_banner(frame, popup_area, title, theme);
    let border_color = if is_destructive {
        theme.destructive
    } else {
        theme.heading
    };
    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    inner
}

pub fn draw_confirm_popup(
    frame: &mut Frame,
    popup: &ConfirmPopup,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
    let inner = draw_confirm_popup_frame(
        frame,
        area,
        "CONFIRM",
        PopupSize::Confirm,
        popup.is_destructive,
        theme,
    );

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

pub fn draw_dim_vline(frame: &mut Frame, area: Rect, color: Color) {
    let buf = frame.buffer_mut();
    for row in area.top()..area.bottom() {
        if let Some(cell) = buf.cell_mut((area.x, row)) {
            cell.set_symbol("│");
            cell.set_fg(color);
        }
    }
}

fn draw_corner_watermark(frame: &mut Frame, area: Rect, color: Color) {
    let version = env!("CARGO_PKG_VERSION");
    let text = format!("clin v{version}");
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
                .arg(format!("--file-filter={filter_name} | *{filter_ext}"))
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
                .arg(format!("*{filter_ext}"))
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
            "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Filter = '{filter_name} (*{filter_ext})|*{filter_ext}'; $f.ShowDialog() | Out-Null; $f.FileName"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PreviewPosition;
    use ratatui::layout::Rect;

    /// The month calendar must sit at the bottom of the LIST column only and
    /// never overlap the preview pane (preview keeps full content height).
    #[test]
    fn calendar_never_overlaps_preview_and_stays_in_list_column() {
        let area = Rect::new(0, 0, 100, 24);
        // Content region (between title bar and status bar) height.
        let content_h = 22;

        for &position in &[PreviewPosition::Left, PreviewPosition::Right] {
            let (list_area, preview_area, calendar_area) = list_view_layout(
                area, true, // preview enabled
                position, true,  // calendar enabled
                false, // preview_fullscreen
            );

            let preview = preview_area.expect("preview area present when enabled");
            let cal = calendar_area.expect("calendar area present when enabled");

            // Preview keeps the full content height (calendar does not steal rows).
            assert_eq!(
                preview.height, content_h,
                "preview full height @ {position:?}"
            );

            // Calendar spans the list column width exactly (not the preview's).
            assert_eq!(
                cal.x, list_area.x,
                "calendar left == list left @ {position:?}"
            );
            assert_eq!(
                cal.width, list_area.width,
                "calendar width == list width @ {position:?}"
            );

            // Calendar sits directly below the list area.
            assert_eq!(
                cal.y,
                list_area.y + list_area.height,
                "calendar below list @ {position:?}"
            );
            assert_eq!(cal.height, 9, "calendar height @ {position:?}");

            // Calendar and preview are disjoint (separated in x since y overlaps).
            let disjoint = cal.right() <= preview.x || preview.right() <= cal.x;
            assert!(disjoint, "calendar must not overlap preview @ {position:?}");
        }
    }

    /// With preview off the calendar spans the full content width at the bottom.
    #[test]
    fn calendar_full_width_when_no_preview() {
        let area = Rect::new(0, 0, 80, 24);
        let (list_area, preview_area, calendar_area) =
            list_view_layout(area, false, PreviewPosition::Right, true, false);

        assert!(preview_area.is_none());
        let cal = calendar_area.expect("calendar enabled");
        assert_eq!(cal.width, list_area.width);
        assert_eq!(cal.y, list_area.y + list_area.height);
        assert_eq!(cal.height, 9);
    }

    /// With the calendar disabled there is no calendar area and list is full.
    #[test]
    fn no_calendar_area_when_disabled() {
        let area = Rect::new(0, 0, 80, 24);
        let (_, preview_area, calendar_area) =
            list_view_layout(area, true, PreviewPosition::Right, false, false);
        assert!(calendar_area.is_none());
        assert!(preview_area.is_some());
    }

    #[test]
    fn test_get_preview_info() {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&templates_dir).unwrap();

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).unwrap();

        // 1. preview_fullscreen is false
        app.preview_fullscreen = false;
        assert!(get_preview_info(&app).is_none());

        app.preview_fullscreen = true;

        // 2. ViewMode::List, empty list
        app.mode = ViewMode::List;
        app.list.visual_list.clear();
        assert!(get_preview_info(&app).is_none());

        // 3. ViewMode::List, selected folder
        app.list.visual_list = vec![crate::list_view::VisualItem::Folder {
            path: "work/projects".to_string(),
            name: "projects".to_string(),
            depth: 1,
            is_expanded: false,
            note_count: 0,
        }];
        app.list.visual_index = 0;
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work".to_string(),
                item_name: "projects".to_string(),
                prev_name: None,
                next_name: None,
            })
        );

        // 4. ViewMode::List, selected note with prev and next
        let note = crate::storage::NoteSummary {
            id: "note1".to_string(),
            title: "My Note".to_string(),
            updated_at: 0,
            folder: "work/projects".to_string(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 0,
        };
        app.notes = vec![note];
        app.list.visual_list = vec![
            crate::list_view::VisualItem::Folder {
                path: "work/projects".to_string(),
                name: "projects".to_string(),
                depth: 1,
                is_expanded: true,
                note_count: 1,
            },
            crate::list_view::VisualItem::Note {
                summary_idx: 0,
                depth: 2,
                is_clin: false,
                is_draw: false,
                is_canvas: false,
                in_virtual_pinned_folder: false,
            },
            crate::list_view::VisualItem::Folder {
                path: "other".to_string(),
                name: "other".to_string(),
                depth: 1,
                is_expanded: false,
                note_count: 0,
            },
        ];
        app.list.visual_index = 1;
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work/projects".to_string(),
                item_name: "My Note".to_string(),
                prev_name: Some("projects".to_string()),
                next_name: Some("other".to_string()),
            })
        );

        // 5. ViewMode::Edit, editing a note
        app.mode = ViewMode::Edit;
        app.editor.editing_id = Some("note1".to_string());
        app.editor.title_editor = crate::events::make_title_editor(
            "Unsaved Title Change",
            app.app_theme.highlight_fg,
            app.app_theme.highlight_bg,
        );
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work/projects".to_string(),
                item_name: "Unsaved Title Change".to_string(),
                prev_name: Some("projects".to_string()),
                next_name: Some("other".to_string()),
            })
        );
    }
}
