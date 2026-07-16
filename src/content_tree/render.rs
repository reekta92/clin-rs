use crate::app::ViewMode;
use crate::app_theme::AppThemeColors;
use crate::content_tree::state::ContentTreeState;
use crate::keybinds::{ContentTreeAction, Keybinds};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
};

fn get_tree_prefix(state: &ContentTreeState, visible: &[usize], p: usize) -> String {
    let idx = visible[p];
    let node = &state.nodes[idx];
    let depth = node.depth;

    if depth == 0 {
        return String::new();
    }

    let mut prefix = String::new();

    for l in 1..=depth {
        // Check if there is a sibling below at level l
        let mut has_sibling_below = false;
        for &next_idx in &visible[p + 1..] {
            let next_depth = state.nodes[next_idx].depth;
            if next_depth == l {
                has_sibling_below = true;
                break;
            }
            if next_depth < l {
                break;
            }
        }

        if l < depth {
            if has_sibling_below {
                prefix.push_str("│   ");
            } else {
                prefix.push_str("    ");
            }
        } else {
            let is_first_child = if p > 0 {
                state.nodes[visible[p - 1]].depth < depth
            } else {
                true
            };
            let is_last_child = !has_sibling_below;

            if is_first_child && !is_last_child {
                prefix.push_str("╭── ");
            } else if is_last_child {
                prefix.push_str("╰── ");
            } else {
                prefix.push_str("├── ");
            }
        }
    }

    prefix
}

pub fn draw_content_tree(
    frame: &mut Frame,
    area: Rect,
    state: &mut ContentTreeState,
    theme: &AppThemeColors,
    keybinds: &Keybinds,
    config: &crate::config::ClinConfig,
    app_status: Option<&str>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Tree + Side Pane
            Constraint::Length(1), // Hint line
        ])
        .split(area);

    let main_area = chunks[0];
    let hint_area = chunks[1];

    // 2. Draw Tree and Side Pane Content
    if state.load_error {
        let err_p = Paragraph::new("Could not load note")
            .style(Style::default().fg(theme.destructive))
            .alignment(Alignment::Center);
        frame.render_widget(err_p, main_area);
    } else {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(45, 100), // Left: Tree
                Constraint::Length(1),      // Separator
                Constraint::Min(0),         // Right: Full Content
            ])
            .split(main_area);

        let left_area = content_chunks[0];
        let sep_area = content_chunks[1];
        let right_area = content_chunks[2];

        // Draw Left Tree list
        let visible = state.visible_indices();
        let mut items = Vec::new();

        for (p, &idx) in visible.iter().enumerate() {
            if let Some(node) = state.nodes.get(idx) {
                let prefix = get_tree_prefix(state, &visible, p);

                let mut spans = Vec::new();
                if !prefix.is_empty() {
                    spans.push(Span::styled(prefix, Style::default().fg(theme.muted)));
                }

                match &node.kind {
                    crate::content_tree::parse::NodeKind::Header { title, .. } => {
                        let arrow = if node.has_children {
                            if state.expanded.contains(&idx) {
                                "▼ "
                            } else {
                                "▶ "
                            }
                        } else {
                            "  "
                        };
                        spans.push(Span::styled(
                            arrow,
                            Style::default()
                                .fg(theme.heading)
                                .add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            title.clone(),
                            Style::default()
                                .fg(theme.heading)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    crate::content_tree::parse::NodeKind::ListItem { text } => {
                        spans.push(Span::styled("• ", Style::default().fg(theme.accent)));
                        spans.push(Span::styled(text.clone(), Style::default().fg(theme.text)));
                    }
                    crate::content_tree::parse::NodeKind::Paragraph { preview, .. } => {
                        spans.push(Span::styled(
                            preview.clone(),
                            Style::default().fg(theme.muted),
                        ));
                    }
                    crate::content_tree::parse::NodeKind::CodeBlock { lang, .. } => {
                        spans.push(Span::styled(
                            format!("```{lang}"),
                            Style::default().fg(theme.muted),
                        ));
                    }
                }

                items.push(ListItem::new(Line::from(spans)));
            }
        }

        let selected_pos = visible.iter().position(|&x| x == state.selected);
        let mut list_state = crate::ui::list_state_selected(selected_pos, state.tree_scroll_offset);

        let item_count = items.len();
        let list = List::new(items)
            .block(Block::default().style(theme.bg_style()))
            .highlight_style(
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, left_area, &mut list_state);
        state.tree_scroll_offset = list_state.offset();
        state.tree_list_rect = left_area;
        crate::ui::paint_list_hover(
            frame,
            left_area,
            &list_state,
            item_count,
            state.mouse_pos,
            theme.hover_style(),
        );
        let content_len = visible.len();
        let viewport_len = left_area.height as usize;
        state.last_tree_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track: crate::ui::scrollbar::track_rect(left_area),
            content_len,
            viewport_len,
        });
        if config.ui.scrollbars {
            // ScrollbarState interprets position as selection index [0, content_len-1],
            // but tree_scroll_offset is a scroll offset [0, content_len-viewport_len].
            // Scale to selection range so thumb reaches bottom at max scroll.
            let max_offset = content_len.saturating_sub(viewport_len);
            let scrollbar_pos = if max_offset > 0 {
                state
                    .tree_scroll_offset
                    .saturating_mul(content_len.saturating_sub(1))
                    / max_offset
            } else {
                0
            };
            crate::ui::scrollbar::draw_scrollbar(
                frame,
                left_area,
                content_len,
                viewport_len,
                scrollbar_pos,
                content_len.saturating_sub(1),
                theme,
            );
        }
        // Draw vertical separator
        crate::ui::draw_dim_vline(frame, sep_area, theme.muted);

        // Draw Right Side Pane: Full content of selected node
        if !state.nodes.is_empty() && state.selected < state.nodes.len() {
            let node = &state.nodes[state.selected];
            let full_text = node.full_text();

            let right_block = Block::default()
                .title(Span::styled(
                    " Full Content ",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ))
                .style(theme.bg_style())
                .padding(ratatui::widgets::Padding::new(2, 2, 1, 1));

            let p = Paragraph::new(full_text)
                .block(right_block)
                .style(Style::default().fg(theme.text))
                .wrap(ratatui::widgets::Wrap { trim: false });

            frame.render_widget(p, right_area);
        } else {
            let right_block = Block::default()
                .title(Span::styled(
                    " Full Content ",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ))
                .style(theme.bg_style());
            let p = Paragraph::new("").block(right_block);
            frame.render_widget(p, right_area);
        }
    }

    // 3. Draw Hint line
    let hints_items = vec![
        (
            format!(
                "{}/{}",
                keybinds.display_content_tree(ContentTreeAction::MoveDown),
                keybinds.display_content_tree(ContentTreeAction::MoveUp)
            ),
            "move",
        ),
        (
            keybinds.display_content_tree(ContentTreeAction::ToggleCollapse),
            "fold",
        ),
        (
            keybinds.display_content_tree(ContentTreeAction::Open),
            "jump",
        ),
        (
            keybinds.display_content_tree(ContentTreeAction::Back),
            "back",
        ),
    ];
    let hint = crate::ui::format_keybind_hints(theme, &hints_items);
    let mut ctx = crate::statusline::StatuslineContext::for_overlay(config, ViewMode::ContentTree);
    ctx.area = Some(hint_area);
    ctx.content_tree = Some(state);
    ctx.app_status = app_status;
    ctx.hints = Some(hint.spans);
    if let Some(p) = &state.seq_matcher.pending_display() {
        ctx.pending = Some(vec![Span::styled(
            format!("{} ", p),
            Style::default().fg(theme.highlight_fg).bg(theme.accent),
        )]);
    }

    let (left_line, right_line) =
        crate::statusline::render_footer(&ctx, &config.statusline, ViewMode::ContentTree, theme);
    crate::ui::draw_status_bar(frame, hint_area, theme, left_line, right_line);
}
