use ratatui::layout::Rect;
use ratatui::Frame;

use crate::graf::app::GrafAppState;
use crate::graf::config::GrafConfig;
use crate::keybinds::{GraphAction, Keybinds};

pub fn draw_ui(frame: &mut Frame, state: &GrafAppState, config: &GrafConfig, keybinds: &Keybinds) {
    let area = frame.area();

    if state.show_help {
        draw_help(frame, area, config, keybinds);
        return;
    }

    if !state.config_errors.is_empty() {
        draw_config_errors(frame, area, &state.config_errors, config);
        return;
    }

    
    let colors = config.theme_colors();

    if let Some(graph_state) = &state.graph_state {
        let guard = graph_state.read().unwrap_or_else(|e| e.into_inner());
        let flags = crate::graph::render::FeatureFlags {
            show_legend: state.show_legend,
            show_grid: state.show_grid,
            show_minimap: state.show_minimap,
            show_status_bar: state.show_status_bar,
        };
        crate::graph::render::draw_graph_view(frame, &guard, config, &flags);
    }

    if state.search_active {
        draw_search(frame, area, state, config, &colors);
    }

    if let Some(ref msg) = state.config_reload_msg {
        draw_reload_notification(frame, area, msg, &colors);
    }
}

fn draw_config_errors(frame: &mut Frame, area: Rect, errors: &[String], config: &GrafConfig) {
    let config_path = crate::graf::config::GrafConfig::config_path()
        .unwrap_or_default()
        .display()
        .to_string();
    let mut lines = vec!["Config Errors".to_string(), "".to_string()];
    for err in errors {
        lines.push(format!("  - {}", err));
        if let Some(suggestion) = suggest_fix(err) {
            lines.push(format!("    -> {}", suggestion));
        }
    }
    lines.push("".to_string());
    lines.push(format!("Fix: {}", config_path));
    lines.push("Press any key to close".to_string());

    let text = lines.join("\n");
    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Config Error")
                .border_type(config.display.border_style.to_border_type()),
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
    if err_lower.contains("border_style") {
        return Some("Valid styles: plain, rounded, double, none".to_string());
    }
    if err_lower.contains("legend_position") {
        return Some("Valid positions: topright, topleft, bottomright, bottomleft".to_string());
    }
    None
}

fn draw_help(frame: &mut Frame, area: Rect, config: &GrafConfig, keybinds: &Keybinds) {
    let help_text: Vec<String> = vec![
        "Keyboard".to_string(),
        format!(
            "  {:<12}Jump between nodes",
            keybinds.graph_keys_display(GraphAction::PanUp)
        ),
        format!(
            "  {:<12}Zoom in/out",
            keybinds.graph_keys_display(GraphAction::ZoomIn)
        ),
        format!(
            "  {:<12}Open selected file",
            keybinds.graph_keys_display(GraphAction::OpenNote)
        ),
        format!(
            "  {:<12}Auto-fit view",
            keybinds.graph_keys_display(GraphAction::AutoFit)
        ),
        format!(
            "  {:<12}Search nodes",
            keybinds.graph_keys_display(GraphAction::ToggleSearch)
        ),
        format!(
            "  {:<12}Toggle minimap",
            keybinds.graph_keys_display(GraphAction::ToggleMinimap)
        ),
        format!(
            "  {:<12}Toggle legend",
            keybinds.graph_keys_display(GraphAction::ToggleLegend)
        ),
        format!(
            "  {:<12}Toggle grid",
            keybinds.graph_keys_display(GraphAction::ToggleGrid)
        ),
        format!(
            "  {:<12}Toggle status bar",
            keybinds.graph_keys_display(GraphAction::ToggleStatus)
        ),
        format!(
            "  {:<12}Refresh simulation",
            keybinds.graph_keys_display(GraphAction::Refresh)
        ),
        format!(
            "  {:<12}Reload config",
            keybinds.graph_keys_display(GraphAction::ReloadConfig)
        ),
        format!(
            "  {:<12}Toggle help",
            keybinds.graph_keys_display(GraphAction::Help)
        ),
        format!(
            "  {:<12}Quit",
            keybinds.graph_keys_display(GraphAction::Quit)
        ),
        "".to_string(),
        "Mouse".to_string(),
        "  Scroll    Zoom in/out".to_string(),
        "  Drag bg   Pan view".to_string(),
        "  Drag node Move node".to_string(),
        "  Click     Select node".to_string(),
        "  Dbl-click Open file".to_string(),
    ];

    let text: String = help_text.join("\n");
    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Help")
                .border_type(config.display.border_style.to_border_type()),
        )
        .alignment(ratatui::layout::Alignment::Left);

    let max_width = help_text.iter().map(|l| l.len()).max().unwrap_or(0) + 4;
    let help_height = help_text.len() as u16 + 2;
    let help_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(max_width as u16)) / 2,
        y: (area.height.saturating_sub(help_height)) / 2,
        width: max_width.min(area.width as usize) as u16,
        height: help_height.min(area.height),
    };

    frame.render_widget(paragraph, help_area);
}

fn draw_search(
    frame: &mut Frame,
    area: Rect,
    state: &GrafAppState,
    config: &GrafConfig,
    colors: &crate::graf::config::ThemeColors,
) {
    let max_visible = config.search.max_visible;
    let result_count = state.search_results.len();
    let visible_count = result_count.min(max_visible);
    let popup_width = config.search.popup_width.min(area.width.saturating_sub(4));
    let popup_height = (visible_count + 3).min(area.height.saturating_sub(4) as usize) as u16;

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = config.search.popup_y;

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
                    .fg(colors.background_color.unwrap_or(ratatui::style::Color::Black))
                    .bg(colors.node_colors.get(0).copied().unwrap_or(colors.label_color))
            } else {
                ratatui::style::Style::default().fg(colors.label_color)
            };
            let prefix = "  ";
            let display =
                crate::graf::util::truncate(title, (popup_width as usize).saturating_sub(6));
            lines.push(ratatui::text::Line::styled(
                format!("{}{}", prefix, display),
                style,
            ));
        }
    }

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title("Search")
        .border_type(config.display.border_style.to_border_type())
        .border_style(ratatui::style::Style::default().fg(colors.border_color));

    let paragraph = ratatui::widgets::Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}

fn draw_reload_notification(
    frame: &mut Frame,
    area: Rect,
    msg: &str,
    colors: &crate::graf::config::ThemeColors,
) {
    let width = (msg.len() as u16 + 4).min(area.width);
    let height = 3u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(height) / 2;

    let popup_area = ratatui::layout::Rect::new(x, y, width, height);

    let is_error = msg.starts_with("Config error");
    let fg = if is_error {
        ratatui::style::Color::Red
    } else {
        colors.label_color
    };

    let paragraph = ratatui::widgets::Paragraph::new(msg)
        .style(ratatui::style::Style::default().fg(fg))
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(ratatui::style::Style::default().fg(colors.border_color)),
        );

    frame.render_widget(paragraph, popup_area);
}
