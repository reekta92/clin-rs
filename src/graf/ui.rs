use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Color;

use ratatui::Frame;

use crate::config::ClinConfig;
use crate::graf::app::GrafAppState;

pub fn draw_ui(frame: &mut Frame, state: &GrafAppState, config: &ClinConfig, area: Rect) {
    if !state.config_errors.is_empty() {
        draw_config_errors(frame, area, &state.config_errors, config);
        return;
    }

    let (graph_area, preview_area) = if state.preview_enabled {
        let (constraints, main_idx, p_idx) = match config.list.preview_position {
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
            .split(area);
        (full_cols[main_idx], Some((full_cols[p_idx], full_cols[1])))
    } else {
        (area, None)
    };

    let colors = config.theme_colors();

    if let Some(graph_state) = &state.graph_state {
        let guard = graph_state.read().unwrap_or_else(|e| e.into_inner());
        let flags = crate::graf::render::FeatureFlags {
            show_legend: state.show_legend,
            show_grid: state.show_grid,
            show_minimap: state.show_minimap,
            show_status_bar: state.show_status_bar,
        };
        crate::graf::render::draw_graph_view(
            frame,
            graph_area,
            &guard,
            config,
            &flags,
            &state.app_theme,
            &state.keybinds,
        );
    }

    if let Some((p_area, sep_area)) = preview_area {
        draw_preview(frame, p_area, state, config);
        draw_dim_vline(frame, sep_area, state.app_theme.muted);
    }

    if state.search_active {
        draw_search(frame, area, state, config, &colors);
    }

    if let Some(ref msg) = state.config_reload_msg {
        draw_reload_notification(frame, area, msg, &colors);
    }
}

fn draw_config_errors(frame: &mut Frame, area: Rect, errors: &[String], _config: &ClinConfig) {
    let config_path = crate::config::ClinConfig::config_path()
        .unwrap_or_default()
        .display()
        .to_string();
    let mut lines = vec!["Config Errors".to_string(), "".to_string()];
    for err in errors {
        lines.push(format!("  - {err}"));
        if let Some(suggestion) = suggest_fix(err) {
            lines.push(format!("    -> {suggestion}"));
        }
    }
    lines.push("".to_string());
    lines.push(format!("Fix: {config_path}"));
    lines.push("Press any key to close".to_string());

    let text = lines.join("\n");
    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Config Error")
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .alignment(ratatui::layout::Alignment::Left);

    let max_width = lines.iter().map(|l| l.len()).max().unwrap_or(0) + 4;
    let height = lines.len() as u16 + 2;
    let popup_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(max_width as u16)) / 2,
        y: (area.height.saturating_sub(height)) / 2,
        width: max_width.min(area.width as usize) as u16,
        height: height.min(area.height),
    };

    frame.render_widget(paragraph, popup_area);
}

fn suggest_fix(err: &str) -> Option<String> {
    let err_lower = err.to_lowercase();
    if err_lower.contains("theme") {
        return Some("Valid themes: default, tokyonight, catppuccinmocha, onedark, gruvbox, dracula, nord, rosepine, everforest, kanagawa, solarized".to_string());
    }
    if err_lower.contains("background") {
        return Some("Valid backgrounds: transparent, solid".to_string());
    }
    if err_lower.contains("node_color_mode") {
        return Some("Valid modes: tag, folder, linkcount, uniform".to_string());
    }
    if err_lower.contains("edge_color_mode") {
        return Some("Valid modes: source, target, uniform".to_string());
    }
    if err_lower.contains("label_mode") {
        return Some("Valid modes: selected, neighbors, all, none".to_string());
    }
    if err_lower.contains("node_size_mode") {
        return Some("Valid modes: fixed, linkcount".to_string());
    }
    if err_lower.contains("legend_position") {
        return Some("Valid positions: topright, topleft, bottomright, bottomleft".to_string());
    }
    None
}

fn draw_search(
    frame: &mut Frame,
    area: Rect,
    state: &GrafAppState,
    config: &ClinConfig,
    colors: &crate::config::ThemeColors,
) {
    let max_visible = config.graf.search.max_visible;
    let result_count = state.search_results.len();
    let visible_count = result_count.min(max_visible);
    let popup_width = (50u16).min(area.width.saturating_sub(4));
    let popup_height = (visible_count + 3).min(area.height.saturating_sub(4) as usize) as u16;

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = 3;

    let popup_area = ratatui::layout::Rect::new(popup_x, popup_y, popup_width, popup_height);

    let before = &state.search_query[..state.search_cursor];
    let after = &state.search_query[state.search_cursor..];
    let label_style = ratatui::style::Style::default().fg(colors.label_color);
    let cursor_style = ratatui::style::Style::default()
        .fg(colors.border_color)
        .add_modifier(ratatui::style::Modifier::REVERSED);
    let input_line = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(before.to_string(), label_style),
        ratatui::text::Span::styled(
            after
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string()),
            cursor_style,
        ),
        ratatui::text::Span::styled(
            after
                .chars()
                .next()
                .map(|_| {
                    after[after
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| i)
                        .unwrap_or(after.len())..]
                        .to_string()
                })
                .unwrap_or_default(),
            label_style,
        ),
    ]);

    let mut lines: Vec<ratatui::text::Line> = vec![input_line];

    if result_count == 0 && !state.search_query.is_empty() {
        lines.push(ratatui::text::Line::styled(
            "  No matches",
            ratatui::style::Style::default().fg(colors.status_bar_color),
        ));
    } else {
        let scroll_offset = state
            .search_selected
            .saturating_sub(max_visible.saturating_sub(1));
        for (i, (_, title)) in state
            .search_results
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(max_visible)
        {
            let is_selected = i == state.search_selected;
            let style = if is_selected {
                ratatui::style::Style::default()
                    .fg(colors
                        .background_color
                        .unwrap_or(ratatui::style::Color::Black))
                    .bg(colors
                        .node_colors
                        .first()
                        .copied()
                        .unwrap_or(colors.label_color))
            } else {
                ratatui::style::Style::default().fg(colors.label_color)
            };
            let prefix = "  ";
            let display =
                crate::graf::util::truncate(title, (popup_width as usize).saturating_sub(6));
            lines.push(ratatui::text::Line::styled(
                format!("{prefix}{display}"),
                style,
            ));
        }
    }

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title("Search")
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border_color));

    let paragraph = ratatui::widgets::Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}

fn draw_reload_notification(
    frame: &mut Frame,
    area: Rect,
    msg: &str,
    colors: &crate::config::ThemeColors,
) {
    let width = (msg.len() as u16 + 4).min(area.width);
    let height = 3u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(height) / 2;

    let popup_area = ratatui::layout::Rect::new(x, y, width, height);

    let is_error = msg.starts_with("Config error");
    let border_color = if is_error {
        ratatui::style::Color::Red
    } else {
        colors.border_color
    };

    let paragraph = ratatui::widgets::Paragraph::new(msg)
        .style(ratatui::style::Style::default().fg(colors.label_color))
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(ratatui::style::Style::default().fg(border_color)),
        );

    frame.render_widget(paragraph, popup_area);
}

fn draw_preview(frame: &mut Frame, preview_rect: Rect, state: &GrafAppState, config: &ClinConfig) {
    let hide_encrypted = config.list.preview_encryption
        && state
            .preview_note_id
            .as_ref()
            .is_some_and(|id| id.ends_with(".clin"));

    crate::preview::draw_preview_pane(
        frame,
        preview_rect,
        &state.app_theme,
        state.preview_content.as_ref(),
        hide_encrypted,
        0,
        config.ui.icon_mode,
    );
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
