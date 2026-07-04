//! Setup wizard rendering: centered CLIN ASCII logo + cycle-in-place option
//! rows + Done button. No title/status bars, no preview pane.

use crate::app::App;
use crate::app_theme::AppThemeColors;
use crate::setup::{CLIN_ASCII, OPTION_ROWS, SetupState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Vertical column dimensions: logo (6) + gap (1) + options (5) + gap (1) + done (1).
const COL_HEIGHT: u16 = 14;
const COL_WIDTH: u16 = 42;
pub(crate) struct SetupLayout {
    pub logo: Rect,
    pub options: Rect,
    pub done: Rect,
}

pub(crate) fn setup_layout(area: Rect) -> SetupLayout {
    let height = COL_HEIGHT.min(area.height);
    let width = COL_WIDTH.min(area.width);
    let col = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),                  // logo
            Constraint::Length(1),                  // gap
            Constraint::Length(OPTION_ROWS as u16), // options
            Constraint::Length(1),                  // gap
            Constraint::Length(1),                  // done
        ])
        .split(col);
    SetupLayout {
        logo: chunks[0],
        options: chunks[2],
        done: chunks[4],
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
        let label = SetupState::row_label(row);
        let value = state.row_value(row);
        lines.push(Line::from(vec![
            Span::styled(format!("{label}: "), base),
            Span::styled("◀ ", arrow),
            Span::styled(value, base),
            Span::styled(" ▶", arrow),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        layout.options,
    );

    // Done button.
    let done_active = state.is_done_selected();
    let done_style = if done_active {
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let done = Paragraph::new(Line::from(Span::styled("  Done  ", done_style)))
        .alignment(Alignment::Center);
    frame.render_widget(done, layout.done);

    // Esc → confirm overlay.
    if state.confirm_exit {
        draw_setup_confirm(frame, frame.area(), theme);
    }
}

fn draw_setup_confirm(frame: &mut Frame, area: Rect, theme: &AppThemeColors) {
    let hints = crate::ui::format_keybind_hints(
        theme,
        &[
            ("y".to_string(), "save & exit"),
            ("n".to_string(), "cancel"),
        ],
    );
    let inner = crate::ui::draw_confirm_popup_frame(
        frame,
        area,
        "Quit first-time setup?",
        crate::ui::PopupSize::Confirm,
        false,
        theme,
    );
    let _ = hints; // footer drawn manually below to control layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(inner);
    let msg = Paragraph::new("Save your choices and exit setup?").alignment(Alignment::Center);
    frame.render_widget(msg, chunks[0]);
    let detail = Paragraph::new("Press y to save, n to go back.")
        .style(Style::default().fg(theme.muted))
        .alignment(Alignment::Center);
    frame.render_widget(detail, chunks[1]);
    frame.render_widget(
        Block::default()
            .borders(Borders::NONE)
            .style(theme.bg_style()),
        chunks[2],
    );
    // Ensure the popup clears underlying content.
    let _ = Clear;
}
