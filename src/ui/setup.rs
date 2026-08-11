//! Setup wizard rendering: centered logo, vault selector, options, hint, Done.

use crate::app::App;
use crate::app_theme::AppThemeColors;
use crate::keybinds::ListAction;
use crate::setup::{CLIN_ASCII, LOGO_CURSOR_ASCII, OPTION_ROWS, SetupState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap},
};

const COL_HEIGHT: u16 = 18;
/// Vertical column: logo (5), gap, options (6), gap, hint (2), Done (3).
const COL_WIDTH: u16 = 44;
const VALUE_WIDTH: usize = 18;
const PREVIEW_WIDTH: u16 = 50;
const LOGO_WIDTH: u16 = 28;
const LOGO_CURSOR_GAP: u16 = 2;
const LOGO_CURSOR_WIDTH: u16 = 4;
const SETUP_PREVIEW_MD: &str = r#"# Welcome to Clin

A terminal note-taking app with `inline code`, **bold**, and _italics_.

## Features

- Markdown rendering
- Tags and folders
- Encryption & backups

> Your notes, encrypted at rest.

```rust
fn main() {
    println!("Hello, Clin!");
}
```

| Key   | Action    |
|-------|-----------|
| j / k | navigate  |
| Enter | open      |
"#;
pub(crate) struct SetupLayout {
    pub logo: Rect,
    pub options: Rect,
    pub hint: Rect,
    pub done: Rect,
    pub preview: Rect,
}

pub(crate) fn setup_layout(area: Rect) -> SetupLayout {
    let total_w = COL_WIDTH + 2 + PREVIEW_WIDTH;
    let actual_w = total_w.min(area.width);

    let col = Rect {
        x: area.x + (area.width.saturating_sub(actual_w)) / 2,
        y: area.y + (area.height.saturating_sub(COL_HEIGHT)) / 2,
        width: actual_w,
        height: COL_HEIGHT.min(area.height),
    };

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(COL_WIDTH + 2),
            Constraint::Length(col.width.saturating_sub(COL_WIDTH + 2)),
        ])
        .split(col);

    let left_col = h_chunks[0];
    let preview_col = if actual_w >= COL_WIDTH + 2 + PREVIEW_WIDTH {
        h_chunks[1]
    } else {
        Rect::default()
    };

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(OPTION_ROWS as u16),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(3),
        ])
        .split(left_col);

    SetupLayout {
        logo: v_chunks[0],
        options: v_chunks[2],
        hint: v_chunks[4],
        done: v_chunks[5],
        preview: preview_col,
    }
}

fn draw_setup_logo(frame: &mut Frame, area: Rect, style: Style, cursor_visible: bool) {
    let logo_group_width = LOGO_WIDTH + LOGO_CURSOR_GAP + LOGO_CURSOR_WIDTH;
    let group_x = area.x + area.width.saturating_sub(logo_group_width) / 2;
    let logo_area = Rect::new(group_x, area.y, LOGO_WIDTH.min(area.width), area.height);
    frame.render_widget(
        Paragraph::new(CLIN_ASCII)
            .style(style)
            .alignment(Alignment::Left),
        logo_area,
    );

    if cursor_visible && area.width >= logo_group_width {
        let cursor_area = Rect::new(
            group_x + LOGO_WIDTH + LOGO_CURSOR_GAP,
            area.y,
            LOGO_CURSOR_WIDTH,
            area.height,
        );
        frame.render_widget(Paragraph::new(LOGO_CURSOR_ASCII).style(style), cursor_area);
    }
}

pub fn draw_setup_view(frame: &mut Frame, app: &mut App) {
    let theme = &app.app_theme;
    let Some(state) = app.setup_state.as_mut() else {
        return;
    };

    // Full-screen background.
    frame.render_widget(Block::default().style(theme.bg_style()), frame.area());

    let layout = setup_layout(frame.area());

    // The cursor owns a fixed rectangle beside the wordmark, so blinking it
    // never changes the logo's position or the surrounding layout.
    let logo_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    draw_setup_logo(
        frame,
        layout.logo,
        logo_style,
        state.logo_cursor_visible_at(std::time::Instant::now()),
    );

    // Option rows.
    let hovered_row = app.mouse_pos.and_then(|(col, row)| {
        if col >= layout.options.x
            && col < layout.options.x + layout.options.width
            && row >= layout.options.y
            && row < layout.options.y + OPTION_ROWS as u16
        {
            Some((row - layout.options.y) as usize)
        } else {
            None
        }
    });
    let mut lines: Vec<Line> = Vec::with_capacity(OPTION_ROWS);
    for row in 0..OPTION_ROWS {
        let active = state.selected == row;
        let is_hovered = !active && Some(row) == hovered_row;
        let disabled = row == 0 && state.vault_cli_override;
        let base = if disabled {
            Style::default().fg(theme.muted)
        } else if active {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else if is_hovered {
            theme.hover_style()
        } else {
            Style::default().fg(theme.text)
        };
        let arrow = if active {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
        } else if is_hovered {
            theme.hover_style()
        } else {
            Style::default().fg(theme.muted)
        };
        let label = crate::setup::SetupState::row_label(row);
        let value = state.row_value(row);
        let truncated_value = if value.chars().count() > VALUE_WIDTH {
            let head: String = value.chars().take(VALUE_WIDTH - 2).collect();
            format!("{head}..")
        } else {
            format!("{:^VALUE_WIDTH$}", value)
        };
        // Keep Vault and cycle rows at the same fixed width. Vault reserves
        // both arrow/action columns as whitespace because selecting it opens
        // the directory flow rather than cycling a value.
        let spans = if row == 0 {
            vec![
                Span::styled(format!("{:<10} ", label), base),
                Span::styled("  ", arrow),
                Span::styled(truncated_value, base),
                Span::styled("         ", arrow),
            ]
        } else {
            vec![
                Span::styled(format!("{:<10} ", label), base),
                Span::styled("◀ ", arrow),
                Span::styled(truncated_value, base),
                Span::styled(" ▶", arrow),
                Span::styled("       ", arrow),
            ]
        };
        lines.push(Line::from(spans));
    }

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        layout.options,
    );

    frame.render_widget(
        Paragraph::new("Remember: press F1 for help or F2 for keybinds.")
            .style(
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        layout.hint,
    );

    // Done button.
    let done_active = state.is_done_selected();
    let btn_w = 14u16.min(layout.done.width);
    let btn_area = Rect::new(
        layout.done.x + (layout.done.width - btn_w) / 2,
        layout.done.y,
        btn_w,
        layout.done.height,
    );
    let done_hovered = !done_active
        && app
            .mouse_pos
            .is_some_and(|(col, row)| crate::events::contains_cell(btn_area, col, row));
    let done_border_style = if done_active {
        Style::default().fg(theme.accent)
    } else if done_hovered {
        theme.hover_style()
    } else {
        Style::default().fg(theme.muted)
    };
    let done_style = if done_active {
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
    } else if done_hovered {
        theme.hover_style()
    } else {
        Style::default().fg(theme.muted)
    };
    let done_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(done_border_style);
    let done = Paragraph::new(Line::from(Span::styled("  Done  ", done_style)))
        .block(done_block)
        .alignment(Alignment::Center);
    frame.render_widget(done, btn_area);

    if layout.preview.width > 0 && layout.preview.height > 0 {
        draw_setup_preview(
            frame,
            layout.preview,
            state.selected,
            theme,
            app.config.ui.icon_mode,
            &app.keybinds,
            state,
        );
    }

    if let Some(modal) = state.vault_modal.as_mut() {
        let content = crate::ui::draw_popup_frame(
            frame,
            frame.area(),
            "VAULT DIRECTORY",
            crate::ui::PopupSize::Prompt,
            crate::ui::PopupHints::Keybinds(&[
                ("Enter".to_string(), "confirm"),
                ("Esc".to_string(), "cancel"),
            ]),
            theme,
        );
        match modal {
            crate::setup::SetupVaultModal::PathInput { input, notice } => {
                let text = notice
                    .as_deref()
                    .or(state.vault_error.as_deref())
                    .unwrap_or(
                        "Enter an absolute vault path. Existing directories are never modified.",
                    );
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(2), Constraint::Length(1)])
                    .split(content);
                frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), chunks[0]);
                frame.render_widget(&*input, chunks[1]);
            }
            crate::setup::SetupVaultModal::ConfirmNonEmpty { path } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Use this non-empty directory as the vault? Existing files will not be modified.\n{}",
                        path.display()
                    ))
                    .wrap(Wrap { trim: true }),
                    content,
                );
            }
        }
    }

    fn draw_setup_preview(
        frame: &mut Frame,
        area: Rect,
        selected: usize,
        theme: &AppThemeColors,
        icon_mode: crate::config::IconMode,
        keybinds: &crate::keybinds::Keybinds,
        state: &mut SetupState,
    ) {
        match selected {
            0 => draw_preview_vault(frame, area, theme, state),
            1 | 2 => draw_preview_markdown(frame, area, theme, icon_mode, state),
            3 => draw_preview_hint_bar(frame, area, theme),
            4 => draw_preview_icons(frame, area, theme, icon_mode),
            5 => draw_preview_keybinds(frame, area, theme, keybinds),
            _ => draw_preview_overview(frame, area, theme, state),
        }
    }

    fn draw_preview_markdown(
        frame: &mut Frame,
        area: Rect,
        theme: &AppThemeColors,
        icon_mode: crate::config::IconMode,
        state: &mut SetupState,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                " Preview ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::new(1, 1, 1, 0));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width < 2 || inner.height < 1 {
            return;
        }

        let cols = inner.width;
        let md_theme = crate::markdown::MarkdownTheme::from_app_theme(theme);
        let opts = crate::markdown::MdRenderOpts {
            syntax_hl: true,
            wrap: true,
            icon_mode,
            code_theme: crate::markdown::default_code_theme().to_string(),
            code_line_numbers: true,
            wrap_indicator: false,
            link_url_max: 80,
        };

        let next_key = crate::setup::SetupPreviewKey {
            cols,
            theme: md_theme,
            opts: opts.clone(),
        };

        let mut should_render = false;
        if state.preview_key.is_none() {
            should_render = true;
        } else if let Some(ref cur) = state.preview_key {
            if cur.theme != next_key.theme || cur.opts != next_key.opts {
                should_render = true;
                state.pending_preview_resize = None;
            } else if cur.cols != next_key.cols {
                let now = std::time::Instant::now();
                if let Some((pending_w, _)) = state.pending_preview_resize {
                    if pending_w != next_key.cols {
                        state.pending_preview_resize = Some((next_key.cols, now));
                    }
                } else {
                    state.pending_preview_resize = Some((next_key.cols, now));
                }
            }
        }

        if let Some((_, inst)) = state.pending_preview_resize
            && inst.elapsed() >= std::time::Duration::from_millis(50)
        {
            should_render = true;
            state.pending_preview_resize = None;
        }

        if should_render {
            let viewport = crate::markdown::RenderViewport {
                start: 0,
                height: inner.height as usize,
            };
            state
                .preview_renderer
                .render_with(SETUP_PREVIEW_MD, cols, theme, &opts, viewport);
            state.preview_key = Some(next_key);
        }

        if let Some(doc) = state.preview_renderer.document() {
            let widget = crate::markdown::MarkdownWidget::new(doc, 0..inner.height as usize);
            frame.render_widget(widget, inner);
        } else {
            let loading = Paragraph::new("Loading...").alignment(Alignment::Center);
            frame.render_widget(loading, inner);
        }
    }

    fn draw_preview_hint_bar(frame: &mut Frame, area: Rect, theme: &AppThemeColors) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                " Preview ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::new(1, 1, 1, 0));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width < 2 || inner.height < 2 {
            return;
        }

        // Header bar example — adapts to theme.hint_bar_style (powerline separators).
        let header_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let left_segs = vec![crate::statusline::Segment::Text("Notes".to_string())];
        let left_line = crate::statusline::line_from_segments(&left_segs, theme, true, false);
        let right_segs = vec![
            crate::statusline::Segment::Text("3 pinned".to_string()),
            crate::statusline::Segment::Text("5 notes".to_string()),
        ];
        let right_line = crate::statusline::line_from_segments(&right_segs, theme, true, true);
        crate::ui::draw_view_title_bar(
            frame,
            header_area,
            theme,
            left_line,
            Some(right_line),
            None,
            0,
        );

        // Footer hint bar example — adapts to theme.hint_bar_style via format_keybind_hints.
        let sample_hints: Vec<(String, &'static str)> = vec![
            ("j/k".to_string(), "navigate"),
            ("Enter".to_string(), "select"),
            ("q".to_string(), "quit"),
        ];
        let hint_line = crate::ui::format_keybind_hints(theme, &sample_hints);
        let footer_area = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
        frame.render_widget(
            Paragraph::new(hint_line).style(theme.hint_line_bg_style()),
            footer_area,
        );
    }

    fn draw_preview_icons(
        frame: &mut Frame,
        area: Rect,
        theme: &AppThemeColors,
        icon_mode: crate::config::IconMode,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                " Icon Preview ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::new(1, 1, 1, 0));
        let pairs: [((&str, &str), &'static str, Color); 5] = [
            (("\u{f07b}", "\u{1f4c1}"), "Folder", theme.folder),
            (("\u{f15c}", "\u{1f4c4}"), "Note", theme.text),
            (("\u{f4cc}", "\u{1f4cc}"), "Pinned", theme.heading),
            (("\u{f02b}", "\u{1f3f7}"), "Tagged", theme.tag),
            (("\u{f023}", "\u{1f512}"), "Encrypted", theme.warning),
        ];
        let lines: Vec<Line> = pairs
            .iter()
            .map(|((nerd, unicode), label, color)| {
                let icon = crate::ui::get_icon(nerd, unicode, icon_mode);
                Line::from(vec![
                    Span::styled(format!("  {}  ", icon), Style::default().fg(*color)),
                    Span::styled(*label, Style::default().fg(theme.text)),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn draw_preview_keybinds(
        frame: &mut Frame,
        area: Rect,
        theme: &AppThemeColors,
        keybinds: &crate::keybinds::Keybinds,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                " Keybind Preview ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::new(1, 1, 1, 0));
        let kb_items = [
            (keybinds.display_list(ListAction::MoveUp), "move up"),
            (keybinds.display_list(ListAction::MoveDown), "move down"),
            (keybinds.display_list(ListAction::Open), "open"),
            (keybinds.display_list(ListAction::Search), "search"),
            (keybinds.display_list(ListAction::CreateNote), "new note"),
        ];
        let lines: Vec<Line> = kb_items
            .iter()
            .map(|(key, label)| {
                Line::from(vec![
                    Span::styled(format!("  {}  ", key), Style::default().fg(theme.accent)),
                    Span::styled(*label, Style::default().fg(theme.text)),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn draw_preview_vault(
        frame: &mut Frame,
        area: Rect,
        theme: &AppThemeColors,
        state: &SetupState,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                " Vault ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::new(1, 1, 1, 0));
        let default_path = crate::config::ClinConfig::default_storage_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("Unavailable: {error}"));
        let active_label = if state.vault_cli_override {
            "Active path (CLI override)"
        } else {
            "Active path"
        };
        let mut lines = vec![
            Line::from(Span::styled(
                "Vault controls where notes and .clin metadata are stored.",
                Style::default().fg(theme.text),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("{active_label:<24}"),
                    Style::default().fg(theme.heading),
                ),
                Span::styled(
                    state.initial_vault_path.display().to_string(),
                    Style::default().fg(theme.accent),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Selected path           ",
                    Style::default().fg(theme.heading),
                ),
                Span::styled(
                    state.vault_path.display().to_string(),
                    Style::default().fg(theme.accent),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Default path            ",
                    Style::default().fg(theme.heading),
                ),
                Span::styled(default_path, Style::default().fg(theme.muted)),
            ]),
        ];
        if state.vault_cli_override {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "CLI override is active; setup cannot change this path.",
                Style::default().fg(theme.muted),
            )));
        }
        frame.render_widget(
            Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
            area,
        );
    }

    fn draw_preview_overview(
        frame: &mut Frame,
        area: Rect,
        theme: &AppThemeColors,
        state: &SetupState,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                " Summary ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::new(1, 1, 1, 0));

        let mut lines: Vec<Line> = Vec::with_capacity(OPTION_ROWS + 3);
        lines.push(Line::from(""));

        for row in 0..OPTION_ROWS {
            let label = SetupState::row_label(row);
            let value = state.row_value(row);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<12}", label),
                    Style::default()
                        .fg(theme.heading)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(value, Style::default().fg(theme.accent)),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Enter to confirm.",
            Style::default().fg(theme.muted),
        )));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    // Esc → confirm overlay.
    if state.confirm_exit {
        draw_setup_confirm(frame, frame.area(), theme);
    }
}

fn draw_setup_confirm(frame: &mut Frame, area: Rect, theme: &AppThemeColors) {
    let inner = crate::ui::draw_confirm_popup_frame(
        frame,
        area,
        "Exit setup?",
        crate::ui::PopupSize::Confirm,
        false,
        crate::ui::PopupHints::Text(""),
        theme,
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(inner);
    let msg = Paragraph::new("Save choices and exit, or discard?").alignment(Alignment::Center);
    frame.render_widget(msg, chunks[0]);
    let detail = Paragraph::new("y = save & exit, q = discard changes, n = back")
        .style(Style::default().fg(theme.muted))
        .alignment(Alignment::Center);
    frame.render_widget(detail, chunks[1]);
    frame.render_widget(
        Block::default()
            .borders(Borders::NONE)
            .style(theme.bg_style()),
        chunks[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn render_logo(cursor_visible: bool) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(COL_WIDTH, 5)).unwrap();
        terminal
            .draw(|frame| {
                draw_setup_logo(frame, frame.area(), Style::default(), cursor_visible);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn full_height_cursor_blinks_without_moving_logo() {
        let visible = render_logo(true);
        let hidden = render_logo(false);
        let group_x = (COL_WIDTH - (LOGO_WIDTH + LOGO_CURSOR_GAP + LOGO_CURSOR_WIDTH)) / 2;
        let cursor_x = group_x + LOGO_WIDTH + LOGO_CURSOR_GAP;

        for y in 0..5 {
            for x in group_x..group_x + LOGO_WIDTH {
                assert_eq!(
                    visible.cell((x, y)).unwrap().symbol(),
                    hidden.cell((x, y)).unwrap().symbol(),
                );
            }
            for x in cursor_x..cursor_x + LOGO_CURSOR_WIDTH {
                assert_eq!(visible.cell((x, y)).unwrap().symbol(), "█");
                assert_eq!(hidden.cell((x, y)).unwrap().symbol(), " ");
            }
        }
    }

    #[test]
    fn short_top_row_keeps_l_and_i_aligned() {
        let logo = render_logo(true);
        let group_x = (COL_WIDTH - (LOGO_WIDTH + LOGO_CURSOR_GAP + LOGO_CURSOR_WIDTH)) / 2;

        // The top of `l` occupies the same columns as its stem.
        for x in group_x + 11..=group_x + 12 {
            assert_eq!(logo.cell((x, 0)).unwrap().symbol(), "█", "top x={x}");
            assert_eq!(logo.cell((x, 1)).unwrap().symbol(), "█", "stem x={x}");
        }
        // The only other top-row glyph is the dot directly above `i`.
        for x in group_x + 16..=group_x + 17 {
            assert_eq!(logo.cell((x, 0)).unwrap().symbol(), "█");
            assert_eq!(logo.cell((x, 2)).unwrap().symbol(), "█");
        }
        for x in group_x..group_x + 11 {
            assert_eq!(logo.cell((x, 0)).unwrap().symbol(), " ");
        }
    }
}
