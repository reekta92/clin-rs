use crate::constants::BACKUP_HELP_HINTS;
fn format_relative_time(unix_secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(unix_secs);
    if diff < 60 { "just now".to_string() }
    else if diff < 3600 { format!("{}m ago", diff / 60) }
    else if diff < 86400 { format!("{}h ago", diff / 3600) }
    else { format!("{}d ago", diff / 86400) }
}

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Padding},
    Frame,
};
use crate::backup::state::{BackupInputMode, BackupState, SettingsField};
use crate::backup::git_ops::FileChangeType;

pub fn draw_dashboard(frame: &mut Frame, state: &BackupState) {
    let area = frame.area();
    let theme = &state.theme;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(1), // Footer
        ])
        .split(area);

    draw_header(frame, chunks[0], state);
    
    // Content area split into list and diff if a file is selected and has diffs
    if state.selected_file.is_some() && !state.diff_lines.is_empty() {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // File list
                Constraint::Percentage(60), // Diff pane
            ])
            .split(chunks[1]);

        draw_content(frame, content_chunks[0], state);
        draw_diff_pane(frame, content_chunks[1], state);
    } else {
        draw_content(frame, chunks[1], state);
    }
    
    crate::ui::draw_status_bar(
        frame,
        chunks[2],
        theme,
        None,
        BACKUP_HELP_HINTS,
        None,
    );

    if state.input_mode == BackupInputMode::EditCommitMessage {
        draw_commit_popup(frame, area, state);
    }

    if state.settings_open {
        draw_settings_popup(frame, area, state);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, state: &BackupState) {
    let theme = &state.theme;
    let mut spans = vec![
        Span::styled(" BACKUP ", Style::default().fg(theme.heading).add_modifier(Modifier::BOLD)),
    ];

    if let Some(status) = &state.status {
        spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        spans.push(Span::styled(&status.branch, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        spans.push(Span::styled(
            format!("↑{} ↓{}", status.ahead, status.behind),
            Style::default().fg(theme.text)
        ));

        spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        if !status.staged.is_empty() || !status.unstaged.is_empty() || !status.untracked.is_empty() {
            spans.push(Span::styled("modified", Style::default().fg(theme.warning)));
        } else {
            spans.push(Span::styled("clean", Style::default().fg(theme.success)));
        }
    }

    let header = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .style(theme.title_bar_bg_style());
    
    frame.render_widget(header, area);
}

fn draw_content(frame: &mut Frame, area: Rect, state: &BackupState) {
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
            Line::from(Span::styled(msg, Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC))),
            Line::from(Span::styled("Press / to open settings.", Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC))),
        ];
        
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(block)
            .wrap(Wrap { trim: true });
            
        let centered_area = crate::ui::centered_rect(60, 20, area);
        frame.render_widget(paragraph, centered_area);
        return;
    }

    let mut lines = Vec::new();
    let status = state.status.as_ref().unwrap();

    // Staged Changes
    lines.push(Line::from(Span::styled(format!("Staged ({}):", status.staged.len()), Style::default().fg(theme.heading).add_modifier(Modifier::BOLD))));
    if status.staged.is_empty() {
        lines.push(Line::from(Span::styled("  No staged changes", Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC))));
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
                Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", sym), style),
                Span::styled(&s.path, Style::default().fg(theme.text)),
            ]).style(line_style));
        }
    }
    lines.push(Line::from(""));

    // Unstaged Changes
    lines.push(Line::from(Span::styled(format!("Unstaged ({}):", status.unstaged.len()), Style::default().fg(theme.heading).add_modifier(Modifier::BOLD))));
    if status.unstaged.is_empty() && status.untracked.is_empty() {
        lines.push(Line::from(Span::styled("  No unstaged changes", Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC))));
    } else {
        for s in &status.unstaged {
            let is_selected = state.selected_file.as_ref() == Some(&s.path);
            let (sym, style) = match s.status {
                FileChangeType::Modified => ("M", Style::default().fg(theme.warning)),
                FileChangeType::Deleted => ("D", Style::default().fg(theme.destructive)),
                _ => ("M", Style::default().fg(theme.warning)),
            };
            
            let line_style = if is_selected {
                Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", sym), style),
                Span::styled(&s.path, Style::default().fg(theme.text)),
            ]).style(line_style));
        }
        for path in &status.untracked {
            let is_selected = state.selected_file.as_ref() == Some(path);
            let line_style = if is_selected {
                Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            lines.push(Line::from(vec![
                Span::styled("  ? ", Style::default().fg(theme.muted)),
                Span::styled(path, Style::default().fg(theme.text)),
            ]).style(line_style));
        }
    }
    lines.push(Line::from(""));

    // History
    lines.push(Line::from(Span::styled("── Recent Commits ──", Style::default().fg(theme.muted))));
    if state.commits.is_empty() {
        lines.push(Line::from(Span::styled("  No commits yet", Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC))));
    } else {
        for commit in &state.commits {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", commit.id), Style::default().fg(theme.accent)),
                Span::styled(&commit.message, Style::default().fg(theme.text)),
                Span::styled(format!(" ({}, {})", commit.author, format_relative_time(commit.time)), Style::default().fg(theme.muted)),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default()
            .style(theme.bg_style())
            .borders(Borders::NONE)
            .padding(Padding::new(2, 2, 1, 1)))
        .wrap(Wrap { trim: false })
        .scroll((state.scroll, 0));

    frame.render_widget(paragraph, area);

    // Status Message Flash
    if let Some(msg) = &state.status_message {
        let flash_area = Rect {
            x: area.x + 2,
            y: area.y + area.height - 2,
            width: area.width.saturating_sub(4),
            height: 1,
        };
        let style = if msg.to_lowercase().contains("error") || msg.to_lowercase().contains("failed") {
            Style::default().fg(theme.destructive)
        } else {
            Style::default().fg(theme.success)
        };
        frame.render_widget(Paragraph::new(msg.as_str()).style(style), flash_area);
    }
}
fn draw_diff_pane(frame: &mut Frame, area: Rect, state: &BackupState) {
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
            Span::styled(file, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
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
}

fn draw_commit_popup(frame: &mut Frame, area: Rect, state: &BackupState) {
    let theme = &state.theme;
    let content = crate::ui::draw_popup_frame(
        frame,
        area,
        "COMMIT",
        60,
        15,
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
        50,
        65,
        "j/k navigate · Enter toggle/edit · Esc cancel",
        theme,
    );

    let chunks = Layout::default()
        .constraints([
            Constraint::Length(1), // General heading
            Constraint::Length(3), // Enabled
            Constraint::Length(1), // spacer
            Constraint::Length(1), // Auto-Backup heading
            Constraint::Length(3), // Backup on Save
            Constraint::Length(3), // Backup on Quit
            Constraint::Length(3), // Auto-Push
            Constraint::Length(1), // spacer
            Constraint::Length(1), // Remote heading
            Constraint::Length(3), // Remote URL
            Constraint::Length(3), // Remote Name
            Constraint::Length(1), // spacer
            Constraint::Length(3), // Save button
            Constraint::Min(0),    // filler
        ])
        .split(content);

    let draw_heading = |frame: &mut Frame, area: Rect, text: &str| {
        let p = Paragraph::new(Span::styled(text, Style::default().fg(theme.muted)))
            .alignment(Alignment::Left)
            .block(Block::default().padding(Padding::horizontal(1)));
        frame.render_widget(p, area);
    };

    draw_heading(frame, chunks[0], "── General ──");
    draw_heading(frame, chunks[3], "── Auto-Backup ──");
    draw_heading(frame, chunks[8], "── Remote ──");

    // Toggles
    let toggles = [
        (chunks[1], SettingsField::Enabled, "Backup System Enabled", state.settings.enabled),
        (chunks[4], SettingsField::BackupOnSave, "Backup on every note save", state.settings.backup_on_save),
        (chunks[5], SettingsField::BackupOnQuit, "Backup on app exit", state.settings.backup_on_quit),
        (chunks[6], SettingsField::AutoPush, "Auto-push after backup", state.settings.auto_push),
    ];

    for (area, field, label, value) in toggles {
        let border_color = if state.settings.focused_field == field { theme.heading } else { theme.muted };
        let state_text = if value { " ON" } else { " OFF" };
        let state_color = if value { theme.success } else { theme.destructive };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(theme.bg_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let text = Line::from(vec![
            Span::styled(format!(" {}", label), Style::default().fg(theme.text)),
            Span::styled(state_text, Style::default().fg(state_color)),
        ]);
        frame.render_widget(Paragraph::new(text).alignment(Alignment::Left), inner);
    }

    // TextAreas
    let text_fields = [
        (chunks[9], SettingsField::RemoteUrl, " Remote URL ", &state.settings.remote_url),
        (chunks[10], SettingsField::RemoteName, " Remote Name ", &state.settings.remote_name),
    ];

    for (area, field, title, textarea) in text_fields {
        let border_color = if state.settings.focused_field == field { theme.heading } else { theme.muted };
        let mut cloned = textarea.clone();
        
        let is_editing = state.input_mode == BackupInputMode::EditSettingsField && state.settings.focused_field == field;
        
        if is_editing {
            cloned.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            cloned.set_cursor_style(Style::default());
        }
        cloned.set_cursor_line_style(Style::default());

        cloned.set_block(Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color))
            .style(theme.bg_style()));
        
        frame.render_widget(&cloned, area);
    }

    // Save Button
    let save_border = if state.settings.focused_field == SettingsField::SaveButton { theme.heading } else { theme.muted };
    let save_button = Paragraph::new("SAVE")
        .alignment(Alignment::Center)
        .style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(save_border)).style(theme.bg_style()));
    frame.render_widget(save_button, chunks[12]);

}
