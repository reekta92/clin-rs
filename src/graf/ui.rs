use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Color;
use ratatui::widgets::Clear;

use ratatui::Frame;

use crate::config::ClinConfig;
use crate::graf::app::GrafAppState;

pub fn draw_ui(
    frame: &mut Frame,
    state: &mut GrafAppState,
    config: &ClinConfig,
    area: Rect,
    theme: &crate::app_theme::AppThemeColors,
) {
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
        let p_area = full_cols[p_idx];
        state.last_preview_pane_width = p_area.width;
        state.last_preview_pane_height = p_area.height;
        (full_cols[main_idx], Some((p_area, full_cols[1])))
    } else {
        (area, None)
    };

    if !state.config_errors.is_empty() {
        draw_config_errors(frame, area, &state.config_errors, config);
        return;
    }

    let colors = config.theme_colors();

    if let Some(graph_state) = &state.graph_state {
        let guard = graph_state.read();
        let banner = guard.mode_banner;
        let mut draw_area = graph_area;
        if let Some(mode) = banner {
            let text = match mode {
                crate::graf::graph::ModeBanner::CreateConnection => {
                    " CONNECTION MODE \u{2014} select target "
                }
                crate::graf::graph::ModeBanner::DeleteConnection => {
                    " DELETE CONNECTION MODE \u{2014} select target "
                }
                crate::graf::graph::ModeBanner::LocalGraph => " LOCAL GRAPH ONLY ",
                crate::graf::graph::ModeBanner::GroupedGraph => " GROUPED GRAPH ONLY ",
                crate::graf::graph::ModeBanner::BoxSelect => " BOX SELECT \u{2014} drag, release ",
            };
            let w = text.chars().count() as u16;
            let x = graph_area.x + graph_area.width.saturating_sub(w) / 2;
            frame.render_widget(
                Clear,
                Rect::new(graph_area.x, graph_area.y, graph_area.width, 1),
            );
            frame.render_widget(
                ratatui::widgets::Paragraph::new(ratatui::text::Line::from(
                    ratatui::text::Span::styled(
                        text,
                        ratatui::style::Style::default()
                            .fg(theme.highlight_fg)
                            .bg(theme.accent)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                )),
                Rect::new(x, graph_area.y, w, 1),
            );
            draw_area = Rect::new(
                graph_area.x,
                graph_area.y + 1,
                graph_area.width,
                graph_area.height.saturating_sub(1),
            );
        }
        let flags = crate::graf::render::FeatureFlags {
            show_legend: state.show_legend,
            show_grid: state.show_grid,
            show_minimap: state.show_minimap,
            show_status_bar: state.show_status_bar,
            show_looking_glass: state.show_looking_glass,
        };
        crate::graf::render::draw_graph_view(
            frame,
            draw_area,
            &guard,
            config,
            &flags,
            theme,
            &state.keybinds,
            state.seq_matcher.pending_display().as_deref(),
            state.mouse_pos,
        );
    }

    if let Some((p_area, sep_area)) = preview_area {
        draw_preview(frame, p_area, state, config);
        draw_dim_vline(frame, sep_area, state.app_theme.muted);
    }

    if let Some(popup) = &state.search_popup {
        let max_visible = config.graf.search.max_visible;
        let theme = &state.app_theme;
        let popup_width = (50u16).min(area.width.saturating_sub(4));
        crate::ui::quick_search::draw_quick_search(
            frame,
            area,
            popup,
            theme,
            max_visible,
            move |(_, title), is_selected, theme: &crate::app_theme::AppThemeColors| {
                let style = if is_selected {
                    ratatui::style::Style::default().fg(theme.fg)
                } else {
                    ratatui::style::Style::default().fg(theme.highlight_fg)
                };
                let prefix = if is_selected { "▸ " } else { "  " };
                let display =
                    crate::graf::util::truncate(title, (popup_width as usize).saturating_sub(6));
                ratatui::text::Line::styled(format!("{prefix}{display}"), style)
            },
            config.ui.icon_mode,
        );
    }

    if let Some(ref msg) = state.config_reload_msg {
        draw_reload_notification(frame, area, msg, &colors, theme);
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

fn draw_reload_notification(
    frame: &mut Frame,
    area: Rect,
    msg: &str,
    colors: &crate::config::ThemeColors,
    theme: &crate::app_theme::AppThemeColors,
) {
    let width = (msg.len() as u16 + 4).min(area.width);
    let height = 3u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(height) / 2;

    let popup_area = ratatui::layout::Rect::new(x, y, width, height);

    let is_error = msg.starts_with("Config error");
    let border_color = if is_error {
        theme.destructive
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
