use crate::app::{App, EditFocus, ListFocus, TemplatePopup, ViewMode};
use crate::constants::*;
use crate::events::get_title_text;
use crate::keybinds::*;
use anyhow::{Context, Result};
use ratatui::{prelude::*, widgets::*};
use std::borrow::Cow;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tui_textarea::*;

pub fn draw_ui(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    match app.mode {
        ViewMode::List => draw_list_view(frame, app),
        ViewMode::Edit => draw_edit_view(frame, app, focus),
        ViewMode::Help => draw_help_view(frame, app),
    }
}

pub fn draw_help_view(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(area);

    let help_text = app.get_help_text();
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0));
    frame.render_widget(help, chunks[0]);

    let footer = Paragraph::new(HELP_PAGE_HINTS)
        .block(Block::default().borders(Borders::ALL).title("Navigation"));
    frame.render_widget(footer, chunks[1]);
}

pub fn help_page_text(keybinds: &Keybinds) -> Text<'static> {
    // Get keybind display strings
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

    let help_close = keybinds.help_keys_display(HelpAction::Close);
    let help_scroll = format!(
        "{}/{}",
        keybinds.help_keys_display(HelpAction::ScrollUp),
        keybinds.help_keys_display(HelpAction::ScrollDown)
    );

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            "󰠮 clin",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Help", Style::default().add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(""));

    lines.push(help_heading("󰋗", "Core Features"));
    lines.extend(help_item_dyn("Encrypted local note files (.clin)", None));
    lines.extend(help_item_dyn(
        "In-terminal note list, full text editor, and continual auto-save",
        None,
    ));
    lines.extend(help_item_dyn(
        "Open note file location from notes view",
        Some(&list_location),
    ));
    lines.extend(help_item_dyn(
        "Delete selected note or folder",
        Some(&list_delete),
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("󰮋", "Notes View"));
    lines.extend(help_item_dyn("Move selection", Some(&list_move)));
    lines.extend(help_item_dyn(
        "Expand/Collapse folder",
        Some(&list_expand_collapse),
    ));
    lines.extend(help_item_dyn(
        "Open selected folder, note, or create new",
        Some(&list_open),
    ));
    lines.extend(help_item_dyn(
        "Create new folder",
        Some(&list_create_folder),
    ));
    lines.extend(help_item_dyn("Rename folder", Some(&list_rename_folder)));
    lines.extend(help_item_dyn("Move note to folder", Some(&list_move_note)));
    lines.extend(help_item_dyn("Manage note tags", Some(&list_manage_tags)));
    lines.extend(help_item_dyn("Filter tags", Some(&list_filter_tags)));
    lines.extend(help_item_dyn("Delete note or folder", Some(&list_delete)));
    lines.extend(help_item_dyn(
        "Confirm / cancel delete",
        Some("y/Enter / n/Esc"),
    ));
    lines.extend(help_item_dyn(
        "Open selected note file location",
        Some(&list_location),
    ));
    lines.extend(help_item_dyn(
        "Change focus (notes list <-> buttons)",
        Some(&list_focus),
    ));
    lines.extend(help_item_dyn(
        "Toggle Encryption from focused button",
        Some("Enter/Space"),
    ));
    lines.extend(help_item_dyn("Open help", Some(&list_help)));
    lines.extend(help_item_dyn("Quit app", Some(&list_quit)));
    lines.extend(help_item_dyn(
        "New note from template",
        Some(&list_template),
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("󰷈", "Editor"));
    lines.extend(help_item_dyn(
        "Change focus (Title, Content, toggles)",
        Some(&edit_focus),
    ));
    lines.extend(help_item_dyn(
        "Return to notes (continually auto-saved)",
        Some(&edit_back),
    ));
    lines.extend(help_item_dyn("Save and quit", Some(&edit_quit)));
    lines.extend(help_item_dyn(
        "Copy / Cut / Paste",
        Some(&format!("{edit_copy} / {edit_cut} / {edit_paste}")),
    ));
    lines.extend(help_item_dyn(
        "Select all / Undo / Redo",
        Some(&format!("{edit_select_all} / {edit_undo} / {edit_redo}")),
    ));
    lines.extend(help_item_dyn(
        "Delete prev/next word",
        Some(&format!("{edit_del_word} / {edit_del_next_word}")),
    ));
    lines.push(Line::from(""));

    lines.push(help_heading("󰑃", "Templates"));
    lines.extend(help_item_dyn(
        "New note from template (in notes view)",
        Some(&list_template),
    ));
    lines.extend(help_item_dyn("Cancel template selection", Some("Esc")));
    lines.push(Line::from(""));

    lines.push(help_heading("󰞋", "Help Page"));
    lines.extend(help_item_dyn("Close help", Some(&help_close)));
    lines.extend(help_item_dyn("Scroll", Some(&help_scroll)));
    lines.push(Line::from(""));

    lines.push(help_heading("󰒓", "Configuration"));
    lines.extend(help_item_dyn(
        "Keybinds file: <storage>/keybinds.toml",
        None,
    ));
    lines.extend(help_item_dyn("Templates dir: <storage>/templates/", None));
    lines.extend(help_item_dyn("Run 'clin --help' for CLI commands", None));

    Text::from(lines)
}

pub fn help_heading(icon: &'static str, title: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{} ", icon),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
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

pub fn help_item_dyn(text: &str, key: Option<&str>) -> Vec<Line<'static>> {
    match key {
        Some(key) => {
            let formatted_key = format_keybind(key);
            vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        formatted_key,
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("    • ", Style::default().fg(Color::DarkGray)),
                    Span::raw(text.to_string()),
                ]),
            ]
        }
        None => vec![Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::raw(text.to_string()),
        ])],
    }
}

pub fn draw_list_view(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "clin",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  encrypted terminal notes"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Notes"));
    frame.render_widget(header, chunks[0]);

    let mut items: Vec<ListItem> = Vec::new();

    for item in &app.visual_list {
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
                let text = format!("{indent}{icon} {name} ({note_count})");
                items.push(ListItem::new(Line::from(vec![Span::styled(
                    text,
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Blue),
                )])));
            }
            crate::app::VisualItem::Note {
                summary_idx,
                depth,
                is_clin,
                ..
            } => {
                let summary = &app.notes[*summary_idx];
                let indent = "  ".repeat(*depth);

                let when = format_relative_time(summary.updated_at);
                let mut text_style = Style::default();

                let mut spans = Vec::new();
                spans.push(Span::raw(indent));
                spans.push(Span::raw("  "));

                if !is_clin {
                    spans.push(Span::styled(
                        "[UENC] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if !app.encryption_enabled {
                    text_style = text_style.fg(Color::Red);
                    spans.push(Span::styled("[ENC] ", text_style));
                }

                spans.push(Span::styled(summary.title.as_str(), text_style));

                // Tag badges
                for tag in &summary.tags {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("[{}]", tag),
                        Style::default().fg(Color::LightMagenta),
                    ));
                }

                spans.push(Span::raw(format!("  ({when})")));
                items.push(ListItem::new(Line::from(spans)));
            }
            crate::app::VisualItem::CreateNew { depth, .. } => {
                let indent = "  ".repeat(*depth);
                let text = format!("{indent}  Create new note");
                items.push(ListItem::new(Line::from(vec![Span::styled(
                    text,
                    Style::default().fg(Color::Green),
                )])));
            }
        }
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Select"))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  > ");

    app.list_state.select(Some(app.visual_index));
    frame.render_stateful_widget(list, chunks[1], &mut app.list_state);

    let enc_button_label = if app.encryption_enabled {
        "[ Enc: ON ]"
    } else {
        "[ Enc: OFF ]"
    };
    let enc_button_style = if app.list_focus == ListFocus::EncryptionToggle {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.encryption_enabled {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let ext_button_label = if app.external_editor_enabled {
        "[ Ext: ON ]"
    } else {
        "[ Ext: OFF ]"
    };
    let ext_button_style = if app.list_focus == ListFocus::ExternalEditorToggle {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.external_editor_enabled {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let footer_line = Line::from(vec![
        Span::styled(enc_button_label, enc_button_style),
        Span::raw(" "),
        Span::styled(ext_button_label, ext_button_style),
        Span::raw("   "),
        Span::raw(app.status.as_ref()),
    ]);

    let footer =
        Paragraph::new(footer_line).block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(footer, chunks[2]);

    // Draw template popup if open
    if let Some(popup) = &app.template_popup {
        draw_template_popup(frame, popup, area);
    }

    if let Some(popup) = &mut app.folder_popup {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(popup) = &mut app.tag_popup {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&popup.input, popup_area);
    }

    if let Some(popup) = &mut app.filter_popup {
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&*popup, popup_area);
    }

    if let Some(picker) = &app.folder_picker {
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

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Select Folder to Move to"),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut state = ListState::default();
        state.select(Some(picker.selected));

        frame.render_stateful_widget(list, popup_area, &mut state);
    }
}

pub fn draw_template_popup(frame: &mut Frame, popup: &TemplatePopup, area: Rect) {
    // Create popup area
    let popup_area = centered_rect(60, 60, area);

    // Clear the area
    frame.render_widget(Clear, popup_area);

    // Build list items
    let items: Vec<ListItem> = popup
        .templates
        .iter()
        .map(|t| {
            ListItem::new(Line::from(vec![
                Span::styled(&t.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("  ({})", t.filename),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select Template (Enter to select, Esc to cancel)")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(popup.selected));

    frame.render_stateful_widget(list, popup_area, &mut state);
}

pub fn draw_edit_view(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    // Set block directly on app's editor to avoid clone
    let title_border = if focus == EditFocus::Title {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    app.title_editor.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(title_border)
            .title("Title"),
    );
    frame.render_widget(&app.title_editor, chunks[0]);

    if get_title_text(&app.title_editor).is_empty() {
        let title_inner = chunks[0].inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let placeholder = Paragraph::new(Line::from(Span::styled(
            "Untitled note",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(placeholder, title_inner);
    }

    // Set block directly on app's editor to avoid clone
    let body_border = if focus == EditFocus::Body {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    app.editor.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(body_border)
            .title("Content"),
    );
    frame.render_widget(&app.editor, chunks[1]);

    let enc_button_label = if app.encryption_enabled {
        "[ Enc: ON ]"
    } else {
        "[ Enc: OFF ]"
    };
    let enc_button_style = if focus == EditFocus::EncryptionToggle {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.encryption_enabled {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let ext_button_label = if app.external_editor_enabled {
        "[ Ext: ON ]"
    } else {
        "[ Ext: OFF ]"
    };
    let ext_button_style = if focus == EditFocus::ExternalEditorToggle {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.external_editor_enabled {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let status_line = Line::from(vec![
        Span::styled(enc_button_label, enc_button_style),
        Span::raw(" "),
        Span::styled(ext_button_label, ext_button_style),
        Span::raw("   "),
        Span::raw(app.status.as_ref()),
    ]);

    let status =
        Paragraph::new(status_line).block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(status, chunks[2]);

    if app.status.starts_with("Save failed") || app.status.starts_with("Could not open") {
        let popup = centered_rect(75, 20, area);
        frame.render_widget(Clear, popup);
        let text = Paragraph::new(app.status.as_ref())
            .block(Block::default().borders(Borders::ALL).title("Error"))
            .wrap(Wrap { trim: true });
        frame.render_widget(text, popup);
    }

    if let Some(menu) = &app.context_menu {
        let items = vec![
            ListItem::new(" Copy       "),
            ListItem::new(" Cut        "),
            ListItem::new(" Paste      "),
            ListItem::new(" Select All "),
        ];
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let menu_area = Rect::new(menu.x, menu.y, 14, 6);
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

    // Suppress stdio to prevent corrupting the TUI terminal state
    Command::new(command)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch {command}"))?;
    Ok(())
}
