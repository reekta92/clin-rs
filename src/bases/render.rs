//! Viewport-bounded terminal layouts; only public query values reach rendering.

use super::{
    format::Layout,
    state::{BasesState, Item},
    value::Value,
};
use crate::{app_theme::AppThemeColors, fsutil::sanitize_for_terminal};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

pub fn display(value: &Value) -> String {
    if matches!(value, Value::Null) {
        return String::new();
    }
    sanitize_for_terminal(&value.to_text().display()).into_owned()
}
fn item_line(state: &BasesState, item: Item) -> String {
    let Some(loaded) = &state.loaded else {
        return String::new();
    };
    let Ok(result) = &loaded.result else {
        return String::new();
    };
    match item {
        Item::Group(index) => format!(
            "{} ({} files)",
            display(&result.groups[index].value),
            result.groups[index].rows.len()
        ),
        Item::Row(index) => loaded
            .columns
            .iter()
            .map(|column| {
                result.rows[index]
                    .values
                    .get(column)
                    .map_or_else(String::new, display)
            })
            .collect::<Vec<_>>()
            .join("  ·  "),
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut BasesState, theme: &AppThemeColors) {
    state.body_area = area;
    state.hit_rows.clear();
    if let Some(error) = &state.error {
        frame.render_widget(
            Paragraph::new(sanitize_for_terminal(error))
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(theme.destructive)),
            area,
        );
        return;
    }
    let Some(loaded) = &state.loaded else {
        frame.render_widget(Paragraph::new("Loading public vault metadata…"), area);
        return;
    };
    let Ok(result) = &loaded.result else {
        return;
    };
    if matches!(state.layout(), Layout::Kanban) {
        kanban(frame, area, state, theme);
        return;
    }
    if matches!(state.layout(), Layout::Cards) {
        cards(frame, area, state, theme);
        return;
    }
    let item_height = 1;
    let header = usize::from(matches!(state.layout(), Layout::Table));
    let footer = usize::from(header > 0 && !result.summaries.is_empty());
    let visible = (usize::from(area.height).saturating_sub(header + footer) / item_height).max(1);
    if state.selected < state.offset {
        state.offset = state.selected;
    }
    if state.selected >= state.offset + visible {
        state.offset = state.selected + 1 - visible;
    }
    state.offset = state.offset.min(state.items.len().saturating_sub(visible));
    let selected_style = Style::default()
        .fg(theme.highlight_fg)
        .bg(theme.highlight_bg);
    let selected_column = state.column;
    let start_column =
        selected_column.saturating_sub(usize::from(area.width / 20).saturating_sub(1));
    if matches!(state.layout(), Layout::Table) {
        let columns: Vec<_> = loaded
            .columns
            .iter()
            .skip(start_column)
            .take((usize::from(area.width) / 20).max(1))
            .collect();
        let rows = state
            .items
            .iter()
            .copied()
            .enumerate()
            .skip(state.offset)
            .take(visible)
            .map(|(position, item)| {
                for column in 0..columns.len() {
                    let rect = Rect::new(
                        area.x + (column * 20) as u16,
                        area.y + header as u16 + (position - state.offset) as u16,
                        20,
                        1,
                    )
                    .intersection(area);
                    state.hit_rows.push((rect, position, start_column + column));
                }
                match item {
                    Item::Group(index) => Row::new(
                        columns
                            .iter()
                            .enumerate()
                            .map(|(column, key)| {
                                let group = &result.groups[index];
                                let summary = group
                                    .summaries
                                    .get(*key)
                                    .map(|value| format!("Σ {}", display(value)))
                                    .unwrap_or_default();
                                let text = if column == 0 {
                                    format!(
                                        "{} ({} files) {summary}",
                                        display(&group.value),
                                        group.rows.len()
                                    )
                                } else {
                                    summary
                                };
                                Cell::from(text)
                            })
                            .collect::<Vec<_>>(),
                    )
                    .style(
                        Style::default()
                            .fg(theme.heading)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Item::Row(index) => Row::new(
                        columns
                            .iter()
                            .enumerate()
                            .map(|(column, key)| {
                                let text = result.rows[index]
                                    .values
                                    .get(*key)
                                    .map_or_else(String::new, display);
                                let style = if position == state.selected
                                    && column + start_column == selected_column
                                {
                                    selected_style
                                } else if state.selected_ids.contains(&result.rows[index].id) {
                                    Style::default().fg(theme.accent)
                                } else {
                                    Style::default().fg(theme.text)
                                };
                                Cell::from(text).style(style)
                            })
                            .collect::<Vec<_>>(),
                    ),
                }
            })
            .collect::<Vec<_>>();
        let table = Table::new(rows, columns.iter().map(|_| Constraint::Length(19)))
            .header(
                Row::new(columns.iter().map(|column| {
                    Cell::from(sanitize_for_terminal(
                        loaded.document.config.labels.get(*column).unwrap_or(column),
                    ))
                }))
                .style(
                    Style::default()
                        .fg(theme.heading)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .column_spacing(1);
        let table = if footer > 0 {
            table.footer(
                Row::new(columns.iter().map(|column| {
                    Cell::from(
                        result
                            .summaries
                            .get(*column)
                            .map(|value| format!("Σ {}", display(value)))
                            .unwrap_or_default(),
                    )
                }))
                .style(Style::default().fg(theme.heading)),
            )
        } else {
            table
        };
        frame.render_widget(table, area);
    } else {
        for (position, item) in state
            .items
            .iter()
            .copied()
            .enumerate()
            .skip(state.offset)
            .take(visible)
        {
            let rect = Rect::new(
                area.x,
                area.y + ((position - state.offset) * item_height) as u16,
                area.width,
                item_height as u16,
            )
            .intersection(area);
            state.hit_rows.push((rect, position, state.column));
            let text = item_line(state, item);
            let style = if position == state.selected {
                selected_style
            } else {
                Style::default().fg(theme.text)
            };
            frame.render_widget(Paragraph::new(text).style(style), rect);
        }
    }
}
fn card_regions(state: &BasesState, area: Rect) -> Vec<(Rect, usize, usize)> {
    let columns = (area.width / 32).max(1);
    let width = area.width / columns;
    let mut y = area.y;
    let mut column = 0;
    let mut regions = Vec::new();
    for (position, item) in state.items.iter().enumerate().skip(state.offset) {
        if matches!(item, Item::Group(_)) {
            if column > 0 {
                y = y.saturating_add(5);
                column = 0;
            }
            if y >= area.bottom() {
                break;
            }
            regions.push((Rect::new(area.x, y, area.width, 1), position, state.column));
            y = y.saturating_add(1);
        } else {
            if y >= area.bottom() {
                break;
            }
            regions.push((
                Rect::new(
                    area.x + column * width,
                    y,
                    width.saturating_sub(1).max(1),
                    4,
                )
                .intersection(area),
                position,
                state.column,
            ));
            column += 1;
            if column == columns {
                column = 0;
                y = y.saturating_add(5);
            }
        }
    }
    regions
}
fn cards(frame: &mut Frame, area: Rect, state: &mut BasesState, theme: &AppThemeColors) {
    if area.is_empty() {
        return;
    }
    state.offset = state.offset.min(state.items.len().saturating_sub(1));
    let mut regions = card_regions(state, area);
    if !regions
        .iter()
        .any(|(_, position, _)| *position == state.selected)
    {
        state.offset = state.selected;
        regions = card_regions(state, area);
    }
    let Some(loaded) = &state.loaded else {
        return;
    };
    let Ok(result) = &loaded.result else {
        return;
    };
    for (rect, position, _) in &regions {
        let selected = *position == state.selected;
        let style = if selected {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
        } else {
            Style::default().fg(theme.text)
        };
        match state.items[*position] {
            Item::Group(_) => {
                frame.render_widget(
                    Paragraph::new(item_line(state, state.items[*position])).style(
                        Style::default()
                            .fg(theme.heading)
                            .add_modifier(Modifier::BOLD),
                    ),
                    *rect,
                );
            }
            Item::Row(index) => {
                let text = loaded
                    .columns
                    .iter()
                    .map(|key| {
                        let label = loaded.document.config.labels.get(key).unwrap_or(key);
                        let value = result.rows[index]
                            .values
                            .get(key)
                            .map_or_else(String::new, display);
                        format!("{}: {value}", sanitize_for_terminal(label))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let border = if selected || state.selected_ids.contains(&result.rows[index].id) {
                    theme.accent
                } else {
                    theme.border
                };
                frame.render_widget(
                    Paragraph::new(text)
                        .style(style)
                        .wrap(Wrap { trim: false })
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(border)),
                        ),
                    *rect,
                );
            }
        }
    }
    state.hit_rows = regions;
}
fn kanban(frame: &mut Frame, area: Rect, state: &mut BasesState, theme: &AppThemeColors) {
    let Some(loaded) = &state.loaded else {
        return;
    };
    let Ok(result) = &loaded.result else {
        return;
    };
    if result.groups.is_empty() {
        frame.render_widget(
            Paragraph::new("Kanban needs a groupBy property. Open configuration to choose one."),
            area,
        );
        return;
    }
    let active = state.focused_group().map_or(0, |(group, _)| group);
    let count = (usize::from(area.width) / 26).max(1);
    let first = active.saturating_sub(count - 1);
    let width = area.width / count as u16;
    for (group_index, group) in result.groups.iter().enumerate().skip(first).take(count) {
        let rect = Rect::new(
            area.x + ((group_index - first) as u16 * width),
            area.y,
            width,
            area.height,
        );
        let title = format!("{} ({})", display(&group.value), group.rows.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(if group_index == active {
                theme.accent
            } else {
                theme.border
            }));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let selected_row = state
            .items
            .get(state.selected)
            .and_then(|item| {
                if let Item::Row(row) = item {
                    group.rows.iter().position(|candidate| candidate == row)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let count = (usize::from(inner.height) / 3).max(1);
        let offset = selected_row.saturating_sub(count - 1);
        for (position, &row) in group.rows.iter().enumerate().skip(offset).take(count) {
            let rect = Rect::new(
                inner.x,
                inner.y + ((position - offset) * 3) as u16,
                inner.width,
                3,
            )
            .intersection(inner);
            let item_position = state
                .items
                .iter()
                .position(|item| matches!(item,Item::Row(index) if *index==row))
                .unwrap_or(0);
            state.hit_rows.push((rect, item_position, state.column));
            let text = loaded
                .columns
                .iter()
                .take(3)
                .map(|column| {
                    result.rows[row]
                        .values
                        .get(column)
                        .map_or_else(String::new, display)
                })
                .collect::<Vec<_>>()
                .join("\n");
            let style = if item_position == state.selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
            } else {
                Style::default().fg(theme.text)
            };
            frame.render_widget(Paragraph::new(text).style(style), rect);
        }
    }
}
