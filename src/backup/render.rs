use crate::app::ViewMode;
use crate::backup::git_ops::FileChangeType;
use crate::backup::state::{BackupInputMode, BackupState, SettingsField};
use crate::keybinds::BackupAction;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
};
use std::path::Path;

pub fn draw_dashboard(
    frame: &mut ratatui::Frame,
    state: &mut crate::backup::state::BackupState,
    area: Rect,
    config: &crate::config::ClinConfig,
    app_status: Option<&str>,
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
    let has_diff = (state.selected_section == crate::backup::state::BackupSection::Status
        && state.selected_file.is_some()
        && !state.diff_lines.is_empty())
        || (state.selected_section == crate::backup::state::BackupSection::History
            && !state.diff_lines.is_empty());

    if has_diff {
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

    let theme = &state.theme;
    let kb = &state.keybinds;
    let hints_items = vec![
        (kb.display_backup(BackupAction::StageFile), "stage"),
        (kb.display_backup(BackupAction::EnterCommit), "commit"),
        (kb.display_backup(BackupAction::Push), "push"),
        (kb.display_backup(BackupAction::Pull), "pull"),
        (kb.display_backup(BackupAction::Refresh), "refresh"),
        (kb.display_backup(BackupAction::OpenSettings), "settings"),
        (kb.display_backup(BackupAction::Help), "help"),
        (kb.display_backup(BackupAction::Back), "back"),
    ];
    let hint_line = crate::ui::format_keybind_hints(theme, &hints_items);
    let mut ctx = crate::statusline::StatuslineContext::for_overlay(config, ViewMode::Backup);
    ctx.area = Some(footer_area);
    ctx.backup = Some(state);
    ctx.app_status = app_status;
    ctx.hints = Some(hint_line.spans);
    if let Some(p) = &state.seq_matcher.pending_display() {
        ctx.pending = Some(vec![Span::styled(
            format!("{} ", p),
            Style::default().fg(theme.highlight_fg).bg(theme.accent),
        )]);
    }

    let (left_line, right_line) =
        crate::statusline::render_footer(&ctx, &config.statusline, ViewMode::Backup, theme);
    crate::ui::draw_status_bar(frame, footer_area, theme, left_line, right_line);
    if state.input_mode == BackupInputMode::EditCommitMessage {
        draw_commit_popup(frame, area, state);
    }

    if state.settings_open {
        draw_settings_popup(frame, area, state);
    }
}

/// Backup-view tab (label, glyph) pairs, in BackupSection order. Shared by
/// `draw_header` (render) and the backup mouse hit-test so they never drift.
pub fn backup_tabs(icon_mode: crate::config::IconMode) -> [(&'static str, &'static str); 2] {
    [
        (
            "Status",
            crate::ui::get_icon("\u{f0e4}", "\u{1f680}", icon_mode),
        ),
        (
            "History",
            crate::ui::get_icon("\u{f1da}", "\u{1f552}", icon_mode),
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
pub fn draw_header(
    frame: &mut Frame,
    area: Rect,
    config: &crate::config::ClinConfig,
    app_status: Option<&str>,
    vault_path: &Path,
    date_format: &str,
    state: &BackupState,
    icon_mode: crate::config::IconMode,
    mouse_pos: Option<(u16, u16)>,
) {
    let theme = &state.theme;
    let backup_tabs_array = backup_tabs(icon_mode);
    let tabs: Vec<(&str, Option<&str>)> = backup_tabs_array
        .iter()
        .map(|&(l, g)| (l, Some(g)))
        .collect();
    let active = if state.selected_section == crate::backup::state::BackupSection::History {
        1
    } else {
        0
    };
    let hovered = mouse_pos.and_then(|(col, row)| {
        if row == area.y {
            let region = crate::ui::title_bar_tabs_region(area, "Backup");
            crate::ui::hit_test_tabs(
                &tabs,
                area.x,
                area.width,
                region.x,
                col,
                state.tab_icons_only,
                icon_mode,
            )
        } else {
            None
        }
    });
    let spans = crate::ui::build_tab_spans(
        &tabs,
        active,
        hovered,
        theme,
        state.tab_icons_only,
        icon_mode,
    );

    let mut ctx = crate::statusline::StatuslineContext::for_overlay(config, ViewMode::Backup);
    ctx.area = Some(area);
    ctx.backup = Some(state);
    ctx.app_status = app_status;
    ctx.vault_path = Some(vault_path);
    ctx.date_format = Some(date_format);
    let (left_line, right_line) =
        crate::statusline::render_header(&ctx, &config.statusline, ViewMode::Backup, theme);
    crate::ui::draw_view_title_bar_with_tabs(
        frame, area, "Backup", theme, left_line, spans, right_line, app_status,
    );
}

fn draw_content(frame: &mut Frame, area: Rect, state: &mut BackupState) {
    let theme = &state.theme;

    if !state.settings.enabled || state.status.is_none() {
        let block = Block::default()
            .style(theme.bg_style())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let msg = if !state.settings.enabled {
            "Backup system is disabled, turn it on from settings"
        } else {
            "Git backup not configured."
        };

        let text = vec![Line::from(Span::styled(
            msg,
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::ITALIC),
        ))];

        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(block)
            .wrap(Wrap { trim: true });

        let centered_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
        frame.render_widget(paragraph, centered_area);
        return;
    }

    let status = state
        .status
        .as_ref()
        .expect("status populated before render");

    if state.selected_section == crate::backup::state::BackupSection::Status {
        let mut items = Vec::new();
        items.push(ListItem::new(Line::from(Span::styled(
            format!("Staged ({}):", status.staged.len()),
            Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
        ))));
        if status.staged.is_empty() {
            items.push(ListItem::new(Span::styled(
                "  No staged changes",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else {
            for s in &status.staged {
                let (sym, style) = match s.status {
                    FileChangeType::Added => ("+", Style::default().fg(theme.success)),
                    FileChangeType::Modified => ("M", Style::default().fg(theme.accent)),
                    FileChangeType::Deleted => ("D", Style::default().fg(theme.destructive)),
                    FileChangeType::Renamed => ("R", Style::default().fg(theme.text)),
                };
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!("  {sym} "), style),
                    Span::styled(&s.path, Style::default().fg(theme.text)),
                ])));
            }
        }
        items.push(ListItem::new(""));

        items.push(ListItem::new(Line::from(Span::styled(
            format!("Unstaged ({}):", status.unstaged.len()),
            Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
        ))));
        if status.unstaged.is_empty() && status.untracked.is_empty() {
            items.push(ListItem::new(Span::styled(
                "  No unstaged changes",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else {
            for s in &status.unstaged {
                let (sym, style) = match s.status {
                    FileChangeType::Modified => ("M", Style::default().fg(theme.warning)),
                    FileChangeType::Deleted => ("D", Style::default().fg(theme.destructive)),
                    _ => ("M", Style::default().fg(theme.warning)),
                };
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!("  {sym} "), style),
                    Span::styled(&s.path, Style::default().fg(theme.text)),
                ])));
            }
            for path in &status.untracked {
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("  ? ", Style::default().fg(theme.muted)),
                    Span::styled(path, Style::default().fg(theme.text)),
                ])));
            }
        }

        let hovered_idx = state.mouse_pos.and_then(|(col, row)| {
            let inner_y = area.y + 1;
            let inner_h = area.height.saturating_sub(2);
            let inner_x = area.x + 2;
            let inner_w = area.width.saturating_sub(4);
            if col >= inner_x
                && col < inner_x + inner_w
                && row >= inner_y
                && row < inner_y + inner_h
            {
                let mouse_y_offset = row - inner_y;
                let offset = state.list_state.offset();
                let idx = mouse_y_offset as usize + offset;
                if idx < items.len() && state.file_index_at_rendered_line(idx).is_some() {
                    Some(idx)
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(h_idx) = hovered_idx
            && Some(h_idx) != state.list_state.selected()
        {
            items[h_idx] = items[h_idx].clone().style(theme.hover_style());
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .style(theme.bg_style())
                    .borders(Borders::NONE)
                    .padding(Padding::new(2, 2, 1, 1)),
            )
            .highlight_style(
                Style::default()
                    .bg(theme.highlight_bg)
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            );

        if !state.selectable_files.is_empty() {
            let list_idx = state.rendered_index_for_file(state.selected_index);
            state.list_state.select(Some(list_idx));
        } else {
            state.list_state.select(None);
        }
        frame.render_stateful_widget(list, area, &mut state.list_state);
    } else if state.selected_section == crate::backup::state::BackupSection::History {
        let mut items = Vec::new();
        items.push(ListItem::new(Line::from(Span::styled(
            "── Recent Commits ──",
            Style::default().fg(theme.muted),
        ))));
        if state.commits.is_empty() {
            items.push(ListItem::new(Span::styled(
                "  No commits yet",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else {
            for commit in &state.commits {
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {} ", &commit.id[..7.min(commit.id.len())]),
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(&commit.message, Style::default().fg(theme.text)),
                    Span::styled(
                        format!(
                            " ({}, {})",
                            commit.author,
                            crate::ui::format_relative_time(commit.time)
                        ),
                        Style::default().fg(theme.muted),
                    ),
                ])));
            }
        }

        let hovered_idx = state.mouse_pos.and_then(|(col, row)| {
            let inner_y = area.y + 1;
            let inner_h = area.height.saturating_sub(2);
            let inner_x = area.x + 2;
            let inner_w = area.width.saturating_sub(4);
            if col >= inner_x
                && col < inner_x + inner_w
                && row >= inner_y
                && row < inner_y + inner_h
            {
                let mouse_y_offset = row - inner_y;
                let offset = state.history_list_state.offset();
                let idx = mouse_y_offset as usize + offset;
                if idx > 0 && idx < items.len() && !state.commits.is_empty() {
                    Some(idx)
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(h_idx) = hovered_idx
            && Some(h_idx) != state.history_list_state.selected()
        {
            items[h_idx] = items[h_idx].clone().style(theme.hover_style());
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .style(theme.bg_style())
                    .borders(Borders::NONE)
                    .padding(Padding::new(2, 2, 1, 1)),
            )
            .highlight_style(
                Style::default()
                    .bg(theme.highlight_bg)
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            );

        if !state.commits.is_empty() {
            state
                .history_list_state
                .select(Some(state.selected_commit_index + 1));
        } else {
            state.history_list_state.select(None);
        }
        frame.render_stateful_widget(list, area, &mut state.history_list_state);
    }

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

    let mut lines = Vec::new();
    if state.selected_section == crate::backup::state::BackupSection::Status {
        if let Some(file) = &state.selected_file {
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
        }
    } else if state.selected_section == crate::backup::state::BackupSection::History
        && let Some(commit) = state.commits.get(state.selected_commit_index)
    {
        lines.push(Line::from(vec![
            Span::styled("Commit: ", Style::default().fg(theme.muted)),
            Span::styled(
                &commit.id[..7.min(commit.id.len())],
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" - {}", commit.message),
                Style::default().fg(theme.text),
            ),
        ]));
        lines.push(Line::from(""));
    }

    if !lines.is_empty() || !state.diff_lines.is_empty() {
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
    let hints_items = vec![
        (
            state.keybinds.display_backup(BackupAction::ConfirmCommit),
            "confirm",
        ),
        (
            state.keybinds.display_backup(BackupAction::CancelCommit),
            "cancel",
        ),
    ];
    let content = crate::ui::draw_popup_frame(
        frame,
        area,
        "COMMIT",
        crate::ui::PopupSize::Prompt,
        crate::ui::PopupHints::Keybinds(&hints_items),
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
    let hints_items = vec![
        (
            state.keybinds.display_backup(BackupAction::NextField),
            "next",
        ),
        (
            state.keybinds.display_backup(BackupAction::PrevField),
            "prev",
        ),
        (
            state.keybinds.display_backup(BackupAction::ActivateField),
            "toggle/edit",
        ),
        (
            state.keybinds.display_backup(BackupAction::CloseSettings),
            "close",
        ),
    ];
    let content = crate::ui::draw_popup_frame(
        frame,
        area,
        "BACKUP SETTINGS",
        crate::ui::PopupSize::Large,
        crate::ui::PopupHints::Keybinds(&hints_items),
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

            let is_hovered = state
                .mouse_pos
                .is_some_and(|(col, row)| crate::events::contains_cell(area, col, row));
            let block_style = if is_hovered && state.settings.focused_field != field {
                theme.hover_style()
            } else {
                theme.bg_style()
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .style(block_style);

            let inner = block.inner(area);
            let text = format!("{label}: {state_text}");
            let para = Paragraph::new(Span::styled(
                text,
                Style::default().fg(style).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center)
            .style(block_style);

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
        let is_hovered = state
            .mouse_pos
            .is_some_and(|(col, row)| crate::events::contains_cell(area, col, row));
        let block_style = if is_hovered && state.settings.focused_field != field {
            theme.hover_style()
        } else {
            theme.bg_style()
        };
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
                .style(block_style),
        );

        frame.render_widget(&cloned, area);
    }

    // Save Button
    let is_save_focused = state.settings.focused_field == SettingsField::SaveButton;
    let is_save_hovered = state
        .mouse_pos
        .is_some_and(|(col, row)| crate::events::contains_cell(chunks[7], col, row));
    let save_style = if is_save_focused {
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else if is_save_hovered {
        theme.hover_style().add_modifier(Modifier::BOLD)
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
        } else if is_save_hovered {
            theme.hover_style()
        } else {
            theme.bg_style()
        });

    let save_button = Paragraph::new("SAVE SETTINGS")
        .alignment(Alignment::Center)
        .style(save_style)
        .block(save_block);

    frame.render_widget(save_button, chunks[7]);
}
