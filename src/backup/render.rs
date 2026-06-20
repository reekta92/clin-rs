fn format_relative_time(unix_secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(unix_secs);
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

use crate::backup::git_ops::FileChangeType;
use crate::backup::state::{BackupInputMode, BackupState, SettingsField};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

pub fn draw_dashboard(
    frame: &mut ratatui::Frame,
    state: &mut crate::backup::state::BackupState,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Content
            Constraint::Length(1), // Footer
        ])
        .split(area);

    let content_area = chunks[0];
    let footer_area = chunks[1];
    if state.selected_section == crate::backup::state::BackupSection::Status {
        // Content area split into list and diff if a file is selected and has diffs
        if state.selected_file.is_some() && !state.diff_lines.is_empty() {
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Ratio(43, 100), // File list
                    Constraint::Min(0),         // Diff pane
                ])
                .split(content_area);

            draw_content(frame, content_chunks[0], state);
            draw_diff_pane(frame, content_chunks[1], state);
        } else {
            draw_content(frame, content_area, state);
        }
    } else {
        // History tab
        draw_content(frame, content_area, state);
    }

    let mut right_spans = Vec::new();
    let theme = &state.theme;
    if let Some(status) = &state.status {
        right_spans.push(Span::styled(
            status.branch.clone(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        right_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        right_spans.push(Span::styled(
            format!("↑{} ↓{}", status.ahead, status.behind),
            Style::default().fg(theme.text),
        ));
        right_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));

        if !status.staged.is_empty() || !status.unstaged.is_empty() || !status.untracked.is_empty()
        {
            right_spans.push(Span::styled("modified", Style::default().fg(theme.warning)));
        } else {
            right_spans.push(Span::styled("clean", Style::default().fg(theme.success)));
        }
        right_spans.push(Span::raw(" "));
    }

    let right_line = if right_spans.is_empty() {
        None
    } else {
        Some(Line::from(right_spans))
    };

    crate::ui::draw_status_bar(
        frame,
        footer_area,
        theme,
        None,
        state.footer_hint.as_str(),
        right_line,
    );
    if state.input_mode == BackupInputMode::EditCommitMessage {
        draw_commit_popup(frame, area, state);
    }

    if state.settings_open {
        draw_settings_popup(frame, area, state);
    }
}

/// Backup-view tab (label, glyph) pairs, in BackupSection order. Shared by
/// `draw_header` (render) and the backup mouse hit-test so they never drift.
pub const BACKUP_TABS: &[(&str, &str)] = &[
    ("Status", "\u{f0e4}"),  // tachometer
    ("History", "\u{f1da}"), // history
];

pub fn draw_header(frame: &mut Frame, area: Rect, state: &BackupState) {
    let theme = &state.theme;
    let tabs: Vec<(&str, Option<&str>)> =
        BACKUP_TABS.iter().map(|&(l, g)| (l, Some(g))).collect();
    let active = if state.selected_section == crate::backup::state::BackupSection::History {
        1
    } else {
        0
    };
    let spans = crate::ui::build_tab_spans(&tabs, active, theme, state.tab_icons_only);
    crate::ui::draw_view_title_bar_with_tabs(frame, area, "Backup", spans, theme);
}

fn draw_content(frame: &mut Frame, area: Rect, state: &mut BackupState) {
    let theme = &state.theme;

    if !state.settings.enabled || state.status.is_none() {
        let block = Block::default()
            .style(theme.bg_style())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let msg = if !state.settings.enabled {
            "Git backup system is disabled."
        } else {
            "Git backup not configured."
        };

        let text = vec![
            Line::from(Span::styled(
                msg,
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(Span::styled(
                "Press Ctrl+P to open settings.",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(block)
            .wrap(Wrap { trim: true });

        let centered_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
        frame.render_widget(paragraph, centered_area);
        return;
    }

    let mut lines = Vec::new();
    let status = state
        .status
        .as_ref()
        .expect("status populated before render");
    if state.selected_section == crate::backup::state::BackupSection::Status {
        // Staged Changes
        lines.push(Line::from(Span::styled(
            format!("Staged ({}):", status.staged.len()),
            Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
        )));
        if status.staged.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No staged changes",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else {
            for s in &status.staged {
                let is_selected = state.selected_file.as_ref() == Some(&s.path);
                let (sym, style) = match s.status {
                    FileChangeType::Added => ("+", Style::default().fg(theme.success)),
                    FileChangeType::Modified => ("M", Style::default().fg(theme.accent)),
                    FileChangeType::Deleted => ("D", Style::default().fg(theme.destructive)),
                    FileChangeType::Renamed => ("R", Style::default().fg(theme.text)),
                };

                let line_style = if is_selected {
                    Style::default()
                        .bg(theme.highlight_bg)
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(
                    Line::from(vec![
                        Span::styled(format!("  {sym} "), style),
                        Span::styled(&s.path, Style::default().fg(theme.text)),
                    ])
                    .style(line_style),
                );
            }
        }
        lines.push(Line::from(""));

        // Unstaged Changes
        lines.push(Line::from(Span::styled(
            format!("Unstaged ({}):", status.unstaged.len()),
            Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
        )));
        if status.unstaged.is_empty() && status.untracked.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No unstaged changes",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else {
            for s in &status.unstaged {
                let is_selected = state.selected_file.as_ref() == Some(&s.path);
                let (sym, style) = match s.status {
                    FileChangeType::Modified => ("M", Style::default().fg(theme.warning)),
                    FileChangeType::Deleted => ("D", Style::default().fg(theme.destructive)),
                    _ => ("M", Style::default().fg(theme.warning)),
                };

                let line_style = if is_selected {
                    Style::default()
                        .bg(theme.highlight_bg)
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(
                    Line::from(vec![
                        Span::styled(
                            format!(
                                "  [{}] {sym} ",
                                if state.selected_for_commit.contains(&s.path) {
                                    'x'
                                } else {
                                    ' '
                                }
                            ),
                            style,
                        ),
                        Span::styled(&s.path, Style::default().fg(theme.text)),
                    ])
                    .style(line_style),
                );
            }
            for path in &status.untracked {
                let is_selected = state.selected_file.as_ref() == Some(path);
                let line_style = if is_selected {
                    Style::default()
                        .bg(theme.highlight_bg)
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(
                    Line::from(vec![
                        Span::styled(
                            format!(
                                "  [{}] ? ",
                                if state.selected_for_commit.contains(path) {
                                    'x'
                                } else {
                                    ' '
                                }
                            ),
                            Style::default().fg(theme.muted),
                        ),
                        Span::styled(path, Style::default().fg(theme.text)),
                    ])
                    .style(line_style),
                );
            }
        }
    } else if state.selected_section == crate::backup::state::BackupSection::History {
        // History
        lines.push(Line::from(Span::styled(
            "── Recent Commits ──",
            Style::default().fg(theme.muted),
        )));
        if state.commits.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No commits yet",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else {
            for commit in &state.commits {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", commit.id),
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(&commit.message, Style::default().fg(theme.text)),
                    Span::styled(
                        format!(
                            " ({}, {})",
                            commit.author,
                            format_relative_time(commit.time)
                        ),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
        }
    }

    let scroll_val = if state.selected_section == crate::backup::state::BackupSection::Status {
        state.scroll
    } else {
        state.history_scroll
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(2, 2, 1, 1)),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll_val, 0));

    frame.render_widget(paragraph, area);

    // Status Message Flash
    if let Some(msg) = &state.status_message {
        let flash_area = ratatui::layout::Rect {
            x: area.x + 2,
            y: area.y + area.height - 2,
            width: area.width.saturating_sub(4),
            height: 1,
        };
        let style = if msg.to_lowercase().contains("error") || msg.to_lowercase().contains("failed")
        {
            Style::default().fg(theme.destructive)
        } else {
            Style::default().fg(theme.success)
        };
        frame.render_widget(Paragraph::new(msg.as_str()).style(style), flash_area);
    }
    state.last_content_height = area.height;
}

fn draw_diff_pane(frame: &mut Frame, area: Rect, state: &mut BackupState) {
    let theme = &state.theme;
    let block = Block::default()
        .title(" Diff ")
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(theme.border))
        .style(theme.bg_style());

    if let Some(file) = &state.selected_file {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("File: ", Style::default().fg(theme.muted)),
            Span::styled(
                file,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));

        for line in &state.diff_lines {
            let style = if line.starts_with('+') {
                Style::default().fg(theme.success)
            } else if line.starts_with('-') {
                Style::default().fg(theme.destructive)
            } else {
                Style::default().fg(theme.text)
            };
            lines.push(Line::from(Span::styled(line, style)));
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((state.diff_scroll, 0));
        frame.render_widget(paragraph, area);
    }
    state.last_diff_height = area.height;
}

fn draw_commit_popup(frame: &mut Frame, area: Rect, state: &BackupState) {
    let theme = &state.theme;
    let content = crate::ui::draw_popup_frame(
        frame,
        area,
        "COMMIT",
        crate::ui::PopupSize::Prompt,
        "Enter confirm · Esc cancel",
        theme,
    );

    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    let inner = block.inner(content);
    frame.render_widget(block, content);

    let mut textarea = state.commit_textarea.clone();
    textarea.set_block(
        Block::default()
            .borders(Borders::NONE)
            .padding(Padding::horizontal(1))
            .style(theme.bg_style()),
    );
    textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    textarea.set_cursor_line_style(Style::default());

    frame.render_widget(&textarea, inner);
}

fn draw_settings_popup(frame: &mut Frame, area: Rect, state: &BackupState) {
    let theme = &state.theme;
    let content = crate::ui::draw_popup_frame(
        frame,
        area,
        "BACKUP SETTINGS",
        crate::ui::PopupSize::Large,
        "j/k navigate · Enter toggle/edit · Esc cancel",
        theme,
    );

    let outer_block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.heading));
    let inner_content = outer_block.inner(content);
    frame.render_widget(outer_block, content);

    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3), // Enabled
            Constraint::Length(3), // Backup on Save
            Constraint::Length(3), // Backup on Quit
            Constraint::Length(3), // Auto Push
            Constraint::Length(3), // Remote URL
            Constraint::Length(3), // Remote Name
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Save button
            Constraint::Min(0),
        ])
        .split(inner_content);

    // Helper for rendering toggles
    let render_toggle =
        |frame: &mut Frame, area: Rect, label: &str, value: bool, field: SettingsField| {
            let state_text = if value { "ON" } else { "OFF" };
            let style = if value {
                theme.success
            } else {
                theme.destructive
            };
            let border_color = if state.settings.focused_field == field {
                theme.heading
            } else {
                theme.muted
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .style(theme.bg_style());

            let inner = block.inner(area);
            let text = format!("{label}: {state_text}");
            let para = Paragraph::new(Span::styled(
                text,
                Style::default().fg(style).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center)
            .style(theme.bg_style());

            frame.render_widget(block, area);
            frame.render_widget(para, inner);
        };

    render_toggle(
        frame,
        chunks[0],
        "Backup System Enabled",
        state.settings.enabled,
        SettingsField::Enabled,
    );
    render_toggle(
        frame,
        chunks[1],
        "Backup on every note save",
        state.settings.backup_on_save,
        SettingsField::BackupOnSave,
    );
    render_toggle(
        frame,
        chunks[2],
        "Backup on app exit",
        state.settings.backup_on_quit,
        SettingsField::BackupOnQuit,
    );
    render_toggle(
        frame,
        chunks[3],
        "Auto-push after backup",
        state.settings.auto_push,
        SettingsField::AutoPush,
    );

    // TextAreas
    let text_fields = [
        (
            chunks[4],
            SettingsField::RemoteUrl,
            "Remote URL...",
            &state.settings.remote_url,
        ),
        (
            chunks[5],
            SettingsField::RemoteName,
            "Remote Name...",
            &state.settings.remote_name,
        ),
    ];

    for (area, field, placeholder, textarea) in text_fields {
        let border_color = if state.settings.focused_field == field {
            theme.heading
        } else {
            theme.muted
        };
        let mut cloned = textarea.clone();
        cloned.set_placeholder_text(placeholder);
        cloned.set_placeholder_style(
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::ITALIC),
        );

        let is_editing = state.input_mode == BackupInputMode::EditSettingsField
            && state.settings.focused_field == field;

        if is_editing {
            cloned.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            cloned.set_cursor_style(Style::default());
        }
        cloned.set_cursor_line_style(Style::default());

        cloned.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .style(theme.bg_style()),
        );

        frame.render_widget(&cloned, area);
    }

    // Save Button
    let is_save_focused = state.settings.focused_field == SettingsField::SaveButton;
    let save_style = if is_save_focused {
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    };

    let save_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if is_save_focused {
            theme.heading
        } else {
            theme.muted
        }))
        .style(if is_save_focused {
            Style::default().bg(theme.accent)
        } else {
            theme.bg_style()
        });

    let save_button = Paragraph::new("SAVE SETTINGS")
        .alignment(Alignment::Center)
        .style(save_style)
        .block(save_block);

    frame.render_widget(save_button, chunks[7]);
}
