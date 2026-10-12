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
    let card = matches!(state.layout(), Layout::Cards);
    let item_height = if card { 4 } else { 1 };
    let header = usize::from(matches!(state.layout(), Layout::Table));
    let visible = (usize::from(area.height).saturating_sub(header) / item_height).max(1);
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
                state.hit_rows.push((
                    Rect::new(
                        area.x,
                        area.y + header as u16 + (position - state.offset) as u16,
                        area.width,
                        1,
                    ),
                    position,
                    start_column,
                ));
                match item {
                    Item::Group(index) => Row::new([Cell::from(format!(
                        "{} ({} files)",
                        display(&result.groups[index].value),
                        result.groups[index].rows.len()
                    ))])
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
        frame.render_widget(
            Table::new(rows, columns.iter().map(|_| Constraint::Length(19)))
                .header(
                    Row::new(columns.iter().map(|column| Cell::from((*column).clone()))).style(
                        Style::default()
                            .fg(theme.heading)
                            .add_modifier(Modifier::BOLD),
                    ),
                )
                .column_spacing(1),
            area,
        );
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
            let mut paragraph = Paragraph::new(text).style(style);
            if card {
                paragraph = paragraph
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.border)),
                    )
                    .wrap(Wrap { trim: false });
            }
            frame.render_widget(paragraph, rect);
        }
    }
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
    let active = state
        .items
        .get(state.selected)
        .and_then(|item| match item {
            Item::Group(group) => Some(*group),
            Item::Row(row) => result
                .groups
                .iter()
                .position(|group| group.rows.contains(row)),
        })
        .unwrap_or(0);
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
