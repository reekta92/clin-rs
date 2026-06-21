use ratatui::{prelude::*, widgets::*};

use crate::app::{App, HelpTab};
use crate::app_theme::AppThemeColors;
use crate::keybinds::{
    Keybinds, ListAction, EditAction, GraphAction, DrawAction, CanvasAction,
    BackupAction, ContentTreeAction
};
use crate::constants::HELP_PAGE_HINTS;
use super::{build_tab_spans, draw_view_title_bar_with_tabs, draw_status_bar};

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
    tab: HelpTab,
    keybinds: &Keybinds,
    theme: &AppThemeColors,
) -> Text<'static> {
    match tab {
        HelpTab::Notes => notes_help_text(keybinds, theme),
        HelpTab::Editor => editor_help_text(keybinds, theme),
        HelpTab::Graph => graph_help_text(keybinds, theme),
        HelpTab::Draw => draw_help_text(keybinds, theme),
        HelpTab::Canvas => canvas_help_text(keybinds, theme),
        HelpTab::Backup => backup_help_text(keybinds, theme),
        HelpTab::Templates => templates_help_text(keybinds, theme),
        HelpTab::ContentTree => content_tree_help_text(keybinds, theme),
        HelpTab::About => about_help_text(keybinds, theme),
    }
}

fn notes_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Text<'static> {
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
    theme: &AppThemeColors,
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

fn graph_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Text<'static> {
    let mut lines = Vec::new();

    lines.push(help_heading("Keyboard Controls", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Navigate nodes (up/down/left/right)",
        Some(&format!(
            "{}/{}/{}/{}",
            keybinds.graph_keys_display(GraphAction::PanUp),
            keybinds.graph_keys_display(GraphAction::PanDown),
            keybinds.graph_keys_display(GraphAction::PanLeft),
            keybinds.graph_keys_display(GraphAction::PanRight)
        )),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Zoom in/out",
        Some(&format!(
            "{}/{}",
            keybinds.graph_keys_display(GraphAction::ZoomIn),
            keybinds.graph_keys_display(GraphAction::ZoomOut)
        )),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Open selected note",
        Some(&keybinds.graph_keys_display(GraphAction::OpenNote)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Auto-fit graph to viewport",
        Some(&keybinds.graph_keys_display(GraphAction::AutoFit)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Search nodes by title",
        Some(&keybinds.graph_keys_display(GraphAction::ToggleSearch)),
        theme,
    ));
    lines.extend(help_item_dyn("Open filter menu", Some("f"), theme));
    lines.push(Line::from(""));

    lines.push(help_heading("Display Options", theme));
    lines.push(Line::from(""));
    lines.extend(help_item_dyn(
        "Toggle minimap",
        Some(&keybinds.graph_keys_display(GraphAction::ToggleMinimap)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle legend (node colors, link types)",
        Some(&keybinds.graph_keys_display(GraphAction::ToggleLegend)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle background grid",
        Some(&keybinds.graph_keys_display(GraphAction::ToggleGrid)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Toggle status bar",
        Some(&keybinds.graph_keys_display(GraphAction::ToggleStatus)),
        theme,
    ));
    lines.extend(help_item_dyn("Show/Hide legend", Some("L"), theme));
    lines.extend(help_item_dyn("Show/Hide minimap", Some("M"), theme));
    lines.extend(help_item_dyn("Show/Hide grid", Some("G"), theme));
    lines.push(Line::from(""));

    lines.extend(help_item_dyn(
        "Refresh physics simulation",
        Some(&keybinds.graph_keys_display(GraphAction::Refresh)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Reload graf config file",
        Some(&keybinds.graph_keys_display(GraphAction::ReloadConfig)),
        theme,
    ));
    lines.extend(help_item_dyn(
        "Quit graph view",
        Some(&keybinds.graph_keys_display(GraphAction::Quit)),
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

fn draw_help_text(keybinds: &Keybinds, theme: &AppThemeColors) -> Text<'static> {
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
    theme: &AppThemeColors,
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
    theme: &AppThemeColors,
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
    theme: &AppThemeColors,
) -> Text<'static> {
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
    theme: &AppThemeColors,
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
    theme: &AppThemeColors,
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
    theme: &AppThemeColors,
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
    theme: &AppThemeColors,
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

pub fn styled_result_line(s: &str, theme: &AppThemeColors) -> Line<'static> {
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
