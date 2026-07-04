use crate::app::App;
use crate::app_theme::AppThemeColors;
use crate::keybinds::SetupAction;
use crate::ui::{build_list_widget, draw_view_title_bar, list_state_selected};
use ratatui::{prelude::*, widgets::*};

pub(crate) struct SetupLayout {
    pub title: Rect,
    pub status: Rect,
    pub sidebar: Option<Rect>,
    pub content: Rect,
    pub preview: Option<Rect>,
}

pub(crate) fn setup_layout(area: Rect) -> SetupLayout {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let show_preview = area.width >= 100;
    let show_sidebar = area.width >= 70;
    let mut constraints = Vec::new();
    if show_sidebar {
        constraints.push(Constraint::Length(22));
    }
    constraints.push(Constraint::Min(30));
    if show_preview {
        constraints.push(Constraint::Length(40));
    }
    let body_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(outer[1]);

    let mut sidebar = None;
    let mut idx = 0;
    if show_sidebar {
        sidebar = Some(body_split[idx]);
        idx += 1;
    }
    let content = body_split[idx];
    idx += 1;
    let mut preview = None;
    if show_preview {
        preview = Some(body_split[idx]);
    }

    SetupLayout {
        title: outer[0],
        status: outer[2],
        sidebar,
        content,
        preview,
    }
}

pub fn draw_setup_view(frame: &mut Frame, app: &App) {
    let theme = &app.app_theme;
    let Some(state) = app.setup_state.as_ref() else {
        return;
    };

    let layout = setup_layout(frame.area());

    let title = format!(
        "SETUP \u{2014} {} \u{2022} {}/{}",
        crate::setup::SETUP_STEPS[state.step].0,
        state.step,
        crate::setup::SETUP_TOTAL_STEPS
    );
    draw_view_title_bar(
        frame,
        layout.title,
        &title,
        theme,
        None,
        Some(app.status.as_ref()),
        None,
    );

    if let Some(sidebar_area) = layout.sidebar {
        draw_setup_sidebar(frame, sidebar_area, theme, state, app.config.ui.icon_mode);
    }
    draw_setup_content(frame, layout.content, theme, state, app.config.ui.icon_mode);
    if let Some(preview_area) = layout.preview {
        draw_setup_preview(frame, preview_area, theme, state);
    }

    let kb = &app.keybinds;
    let mut hints_items = vec![
        (kb.display_setup(SetupAction::Up), "up"),
        (kb.display_setup(SetupAction::Down), "down"),
        (kb.display_setup(SetupAction::Prev), "prev"),
        (kb.display_setup(SetupAction::Next), "next"),
        (kb.display_setup(SetupAction::ToggleField), "field"),
    ];
    if state.is_toggle_active() {
        hints_items.push(("Space/h/l".to_string(), "change"));
    }
    hints_items.push((kb.display_setup(SetupAction::Finish), "finish"));
    let hint_line = crate::ui::format_keybind_hints(theme, &hints_items);
    crate::ui::draw_status_bar(frame, layout.status, theme, None, hint_line, None, None);
}

/// Render the step sidebar with checkmarks and highlighting.
fn draw_setup_sidebar(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
    icon_mode: crate::config::IconMode,
) {
    let items: Vec<ListItem> = crate::setup::SETUP_STEPS
        .iter()
        .enumerate()
        .map(|(i, (label, glyph))| {
            let prefix = if state.visited[i] {
                "✓ ".to_string() // checkmark
            } else {
                "  ".to_string()
            };
            let display = if icon_mode != crate::config::IconMode::None {
                format!("{prefix}{glyph} {label}")
            } else {
                format!("{prefix}{label}")
            };
            ListItem::new(Line::from(Span::raw(display)))
        })
        .collect();

    let list = build_list_widget(items, theme)
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading))
                .title(" STEPS "),
        );

    let mut list_state = list_state_selected(Some(state.step));
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Dispatch to the per-step content renderer.
fn draw_setup_content(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
    icon_mode: crate::config::IconMode,
) {
    match state.step {
        0 => draw_setup_welcome(frame, area, theme, icon_mode),
        1 => draw_setup_theme_and_bg(frame, area, theme, state),
        2 => draw_setup_choices(
            frame,
            area,
            theme,
            state,
            crate::setup::SETUP_PRESETS,
            state.keybind_preset,
        ),
        3 => draw_setup_toggle(
            frame,
            area,
            theme,
            "Mouse enabled",
            state.mouse_enabled,
            true,
        ),
        4 => draw_setup_choices(
            frame,
            area,
            theme,
            state,
            crate::setup::SETUP_DENSITIES,
            state.list_density,
        ),
        5 => draw_setup_choices(
            frame,
            area,
            theme,
            state,
            crate::setup::SETUP_HINT_STYLES,
            state.hint_bar_style,
        ),
        6 => draw_setup_goals(frame, area, theme, state),
        7 => draw_setup_backup(frame, area, theme, state),
        8 => draw_setup_storage_path(frame, area, theme, state),
        9 => draw_setup_done(frame, area, theme, state),
        _ => {}
    }
}

fn draw_setup_welcome(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    icon_mode: crate::config::IconMode,
) {
    let icon = crate::ui::get_icon("\u{f015}", "\u{1f3e0}", icon_mode);
    let wordmark = format!("{icon} clin");
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            wordmark,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Feature-packed terminal note management",
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "• Encrypted local notes",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "• Force-directed graph view",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "• Git-backed vault",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "• Command palette",
            Style::default().fg(theme.text),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter to begin · Esc saves defaults",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" WELCOME ")
        .title_alignment(Alignment::Center);

    let para = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(theme.bg_style())
        .block(block);
    frame.render_widget(para, area);
}
fn draw_setup_theme_and_bg(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    draw_setup_theme_choices(frame, chunks[0], theme, state, state.focus == 0);
    draw_setup_toggle(
        frame,
        chunks[1],
        theme,
        "Background (solid)",
        state.background_solid,
        state.focus == 1,
    );
}

fn draw_setup_theme_choices(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
    focus: bool,
) {
    use crate::config::{Theme, UiConfig};
    use std::str::FromStr;

    let items: Vec<ListItem> = crate::setup::SETUP_THEMES
        .iter()
        .map(|name| {
            let swatch_cfg = UiConfig {
                theme: Theme::from_str(name).unwrap_or_default(),
                ..UiConfig::default()
            };
            let accent = AppThemeColors::from_config(&swatch_cfg).accent;
            let dot = Span::styled("● ", Style::default().fg(accent));
            let label = Span::raw(*name);
            ListItem::new(Line::from(vec![dot, label]))
        })
        .collect();

    let list = build_list_widget(items, theme)
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(if focus {
                    Style::default().fg(theme.heading)
                } else {
                    Style::default().fg(theme.muted)
                }),
        );

    let mut list_state = list_state_selected(Some(state.theme));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_setup_choices(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    _state: &crate::setup::SetupState,
    options: &[&str],
    selected: usize,
) {
    let items: Vec<ListItem> = options
        .iter()
        .map(|opt| ListItem::new(Line::from(Span::raw(*opt))))
        .collect();

    let list = build_list_widget(items, theme)
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        );

    let mut list_state = list_state_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_setup_toggle(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    label: &str,
    value: bool,
    focus: bool,
) {
    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(if focus {
            Style::default().fg(theme.heading)
        } else {
            Style::default().fg(theme.muted)
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let value_str = if value { "ON" } else { "OFF" };
    let style = if value {
        Style::default().fg(theme.success)
    } else {
        Style::default().fg(theme.destructive)
    };
    let para = Paragraph::new(Span::styled(format!("{label}: {value_str}"), style))
        .alignment(Alignment::Center)
        .style(theme.bg_style());
    frame.render_widget(para, inner);
}

fn draw_setup_textarea(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    ta: &ratatui_textarea::TextArea,
    focus: bool,
    label: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(3)])
        .split(area);

    let label_line = Line::from(Span::styled(
        label,
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(label_line).style(theme.bg_style()),
        chunks[0],
    );

    let mut input = ta.clone();
    input.set_style(theme.bg_style());
    input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(if focus {
                Style::default().fg(theme.heading)
            } else {
                Style::default().fg(theme.muted)
            }),
    );
    // Backlight removed as requested.
    frame.render_widget(&input, chunks[1]);
}

fn draw_setup_goals(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(5)])
        .split(area);

    let toggle_text = if state.goals_enabled {
        "Enabled"
    } else {
        "Disabled"
    };
    let toggle_fg = if state.goals_enabled {
        theme.success
    } else {
        theme.destructive
    };
    let toggle_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if state.focus == 0 {
            Style::default().fg(theme.heading)
        } else {
            Style::default().fg(theme.muted)
        });
    let toggle_inner = toggle_block.inner(chunks[0]);
    frame.render_widget(toggle_block, chunks[0]);
    let toggle_para = Paragraph::new(Span::styled(
        format!("Daily Goals: {toggle_text}"),
        Style::default().fg(toggle_fg),
    ))
    .alignment(Alignment::Center)
    .style(theme.bg_style());
    frame.render_widget(toggle_para, toggle_inner);

    draw_setup_textarea(
        frame,
        chunks[1],
        theme,
        &state.word_goal_input,
        state.focus == 1,
        "Word goal:",
    );
}

fn draw_setup_backup(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(5)])
        .split(area);

    let toggle_text = if state.backup_enabled {
        "Enabled"
    } else {
        "Disabled"
    };
    let toggle_fg = if state.backup_enabled {
        theme.success
    } else {
        theme.destructive
    };
    let toggle_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if state.focus == 0 {
            Style::default().fg(theme.heading)
        } else {
            Style::default().fg(theme.muted)
        });
    let toggle_inner = toggle_block.inner(chunks[0]);
    frame.render_widget(toggle_block, chunks[0]);
    let toggle_para = Paragraph::new(Span::styled(
        format!("Git Auto-Backup: {toggle_text}"),
        Style::default().fg(toggle_fg),
    ))
    .alignment(Alignment::Center)
    .style(theme.bg_style());
    frame.render_widget(toggle_para, toggle_inner);

    draw_setup_textarea(
        frame,
        chunks[1],
        theme,
        &state.remote_url_input,
        state.focus == 1,
        "Remote URL (optional):",
    );
}

fn draw_setup_storage_path(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(1)])
        .split(area);

    draw_setup_textarea(
        frame,
        chunks[0],
        theme,
        &state.storage_path_input,
        true,
        "Vault/Storage path:",
    );

    let helper = Paragraph::new(Span::styled(
        "Leave blank for default (~/.local/share/clin). Supports ~ and $VAR.",
        Style::default().fg(theme.muted),
    ))
    .style(theme.bg_style());
    frame.render_widget(helper, chunks[1]);
}

fn draw_setup_done(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
) {
    let theme_name = crate::setup::SETUP_THEMES[state.theme];
    let preset_name = crate::setup::SETUP_PRESETS[state.keybind_preset];
    let mouse_str = if state.mouse_enabled { "Yes" } else { "No" };
    let density_str = crate::setup::SETUP_DENSITIES[state.list_density];
    let hint_str = crate::setup::SETUP_HINT_STYLES[state.hint_bar_style];
    let background_str = if state.background_solid {
        "Solid"
    } else {
        "Transparent"
    };
    let goals_str = if state.goals_enabled {
        format!(
            "Enabled ({} words/day)",
            state.word_goal_input.lines().join("").trim()
        )
    } else {
        "Disabled".to_string()
    };
    let backup_str = if state.backup_enabled {
        let url = state.remote_url_input.lines().join("").trim().to_string();
        if url.is_empty() {
            "Enabled".to_string()
        } else {
            format!("Enabled ({url})")
        }
    } else {
        "Disabled".to_string()
    };
    let path_str = {
        let raw = state.storage_path_input.lines().join("").trim().to_string();
        if raw.is_empty() {
            "Default (~/.local/share/clin)".to_string()
        } else {
            raw
        }
    };

    let summary_items = vec![
        ("Theme:", theme_name),
        ("Keybinds:", preset_name),
        ("Mouse:", mouse_str),
        ("Density:", density_str),
        ("Hint Bar:", hint_str),
        ("Background:", background_str),
        ("Daily Goals:", &goals_str),
        ("Auto-Backup:", &backup_str),
        ("Vault:", &path_str),
    ];

    let mut text = vec![
        Line::from(Span::styled(
            "Summary",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (label, val) in summary_items {
        text.push(Line::from(vec![
            Span::styled(format!("{:<13}", label), Style::default().fg(theme.muted)),
            Span::raw(val.to_string()),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Enter to finish & save. Esc also saves.",
        Style::default().fg(theme.text),
    )));
    let para = Paragraph::new(text).style(theme.bg_style());
    frame.render_widget(para, area);
}

// ── Preview pane ──

/// Live theme preview pane showing styled spans in the current theme colors.
/// On the Done step, shows the summary instead.
fn draw_setup_preview(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    state: &crate::setup::SetupState,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.heading))
        .title(" PREVIEW ");

    let inner = block.inner(area);
    if state.step == crate::setup::SETUP_TOTAL_STEPS {
        frame.render_widget(block, area);
        draw_setup_done(frame, inner, theme, state);
        return;
    }

    let lines = vec![
        Line::from(Span::styled(
            "# Sample Note",
            Style::default()
                .fg(theme.heading)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Body text in the active theme.",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "muted / hint text",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled(
            "#tag  $folder",
            Style::default().fg(theme.tag),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "`code span`",
            Style::default().fg(theme.accent),
        )),
        Line::from(Span::styled(
            "OK success",
            Style::default().fg(theme.success),
        )),
        Line::from(Span::styled(
            "!! destructive",
            Style::default().fg(theme.destructive),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).style(theme.bg_style()).block(block),
        area,
    );
}
