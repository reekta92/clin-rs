//! Setup wizard rendering: centered CLIN ASCII logo + cycle-in-place option
//! rows + Done button. No title/status bars, no preview pane.

use crate::app::App;
use crate::app_theme::AppThemeColors;
use crate::keybinds::ListAction;
use crate::setup::{CLIN_ASCII, OPTION_ROWS, SetupState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

const COL_HEIGHT: u16 = 16;
/// Vertical column dimensions: logo (6) + gap (1) + options (5) + gap (1) + done (3).
const COL_WIDTH: u16 = 44;
const VALUE_WIDTH: usize = 18;
const PREVIEW_WIDTH: u16 = 50;
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
            Constraint::Length(6),
            Constraint::Length(1),
            Constraint::Length(OPTION_ROWS as u16),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(left_col);

    SetupLayout {
        logo: v_chunks[0],
        options: v_chunks[2],
        done: v_chunks[4],
        preview: preview_col,
    }
}

pub fn draw_setup_view(frame: &mut Frame, app: &mut App) {
    let theme = &app.app_theme;
    let Some(state) = app.setup_state.as_ref() else {
        return;
    };

    // Full-screen background.
    frame.render_widget(Block::default().style(theme.bg_style()), frame.area());

    let layout = setup_layout(frame.area());

    // Logo.
    let logo = Paragraph::new(CLIN_ASCII)
        .style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(logo, layout.logo);

    // Option rows.
    let mut lines: Vec<Line> = Vec::with_capacity(OPTION_ROWS);
    for row in 0..OPTION_ROWS {
        let active = state.selected == row;
        let base = if active {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };
        let arrow = if active {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
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
        lines.push(Line::from(vec![
            Span::styled(format!("{:<10} ", label), base),
            Span::styled("◀ ", arrow),
            Span::styled(truncated_value, base),
            Span::styled(" ▶", arrow),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        layout.options,
    );

    // Done button.
    let done_active = state.is_done_selected();
    let done_border_style = if done_active {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.muted)
    };
    let done_style = if done_active {
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
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
    let btn_w = 14u16.min(layout.done.width);
    let btn_area = Rect::new(
        layout.done.x + (layout.done.width - btn_w) / 2,
        layout.done.y,
        btn_w,
        layout.done.height,
    );
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

    fn draw_setup_preview(
        frame: &mut Frame,
        area: Rect,
        selected: usize,
        theme: &AppThemeColors,
        icon_mode: crate::config::IconMode,
        keybinds: &crate::keybinds::Keybinds,
        state: &SetupState,
    ) {
        match selected {
            0 | 1 => draw_preview_markdown(frame, area, theme, icon_mode),
            2 => draw_preview_hint_bar(frame, area, theme),
            3 => draw_preview_icons(frame, area, theme, icon_mode),
            4 => draw_preview_keybinds(frame, area, theme, keybinds),
            _ => draw_preview_overview(frame, area, theme, state),
        }
    }

    fn draw_preview_markdown(
        frame: &mut Frame,
        area: Rect,
        theme: &AppThemeColors,
        icon_mode: crate::config::IconMode,
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
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let opts = crate::markdown::MdRenderOpts {
            syntax_hl: true,
            wrap: true,
            icon_mode,
            code_theme: crate::markdown::default_code_theme().to_string(),
            code_line_numbers: true,
            wrap_indicator: false,
            link_url_max: 80,
        };
        let (lines, _slots) =
            crate::markdown::render_builtin(SETUP_PREVIEW_MD, cols, &md_theme, &opts, &cancel);
        let grid: Vec<Vec<(char, ratatui::style::Style)>> =
            lines.iter().map(|l| l.cells.clone()).collect();
        frame.render_widget(crate::snapshot::RenderedSnapshot::new(&grid), inner);
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
            "Press Esc to confirm.",
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
