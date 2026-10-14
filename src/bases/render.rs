//! Viewport-bounded terminal layouts; only public query values reach rendering.

use super::{
    format::Layout,
    state::{BasesState, Item},
    value::Value,
};
use crate::fsutil::sanitize_for_terminal;
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
        Item::Group(index) => group_line(state, index),
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

fn group_line(state: &BasesState, index: usize) -> String {
    let group = &state.result().expect("group belongs to query").groups[index];
    format!(
        "[{}] {} ({} files)",
        if state.group_collapsed(index) {
            "+"
        } else {
            "-"
        },
        if matches!(group.value, Value::Null) {
            "None".into()
        } else {
            display(&group.value)
        },
        group.rows.len()
    )
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut BasesState, app: &crate::app::App) {
    let theme = &app.app_theme;
    state.body_area = area;
    state.hit_rows.clear();
    state.hit_groups.clear();
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
        frame.render_widget(
            Paragraph::new(if state.creating {
                "Creating Bases document…"
            } else {
                "Loading public vault metadata…"
            }),
            area,
        );
        return;
    };
    let Ok(result) = &loaded.result else {
        return;
    };
    if matches!(state.layout(), Layout::Kanban) {
        kanban(frame, area, state, app);
        return;
    }
    if matches!(state.layout(), Layout::Cards) {
        cards(frame, area, state, app);
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
        for position in state.offset..state.items.len().min(state.offset + visible) {
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
        }
        let rows = state
            .items
            .iter()
            .copied()
            .enumerate()
            .skip(state.offset)
            .take(visible)
            .map(|(position, item)| match item {
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
                                format!("{} {summary}", group_line(state, index))
                            } else {
                                summary
                            };
                            Cell::from(text)
                        })
                        .collect::<Vec<_>>(),
                )
                .style(if position == state.selected {
                    selected_style.add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(theme.heading)
                        .add_modifier(Modifier::BOLD)
                }),
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
fn card_regions(
    state: &BasesState,
    area: Rect,
    options: &super::layout_options::CardOptions,
    cell: (u16, u16),
) -> Vec<(Rect, usize, usize)> {
    let columns = (area.width / options.width).max(1);
    let width = area.width / columns;
    let height = 4u16.saturating_add(options.cover_height(
        width.saturating_sub(3),
        cell,
        area.height.saturating_sub(4),
    ));
    let mut y = area.y;
    let mut column = 0;
    let mut regions = Vec::new();
    for (position, item) in state.items.iter().enumerate().skip(state.offset) {
        if matches!(item, Item::Group(_)) {
            if column > 0 {
                y = y.saturating_add(height.saturating_add(1));
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
                    height,
                )
                .intersection(area),
                position,
                state.column,
            ));
            column += 1;
            if column == columns {
                column = 0;
                y = y.saturating_add(height.saturating_add(1));
            }
        }
    }
    regions
}
fn cards(frame: &mut Frame, area: Rect, state: &mut BasesState, app: &crate::app::App) {
    let Some(loaded) = &state.loaded else {
        return;
    };
    let Ok(options) = super::layout_options::CardOptions::parse(
        &loaded.document.config.views[loaded.view],
        font(app).0,
    ) else {
        return;
    };
    let cell = font(app);
    let theme = &app.app_theme;
    if area.is_empty() {
        return;
    }
    state.offset = state.offset.min(state.items.len().saturating_sub(1));
    let mut regions = card_regions(state, area, &options, cell);
    if !regions
        .iter()
        .any(|(_, position, _)| *position == state.selected)
    {
        state.offset = state.selected;
        regions = card_regions(state, area, &options, cell);
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
                    Paragraph::new(item_line(state, state.items[*position])).style(if selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(theme.heading)
                            .add_modifier(Modifier::BOLD)
                    }),
                    *rect,
                );
            }
            Item::Row(index) => {
                card(
                    frame,
                    *rect,
                    loaded,
                    CardSelection {
                        row: index,
                        property: state.column,
                        focused: selected,
                        checked: state.selected_ids.contains(&result.rows[index].id),
                    },
                    &mut state.media,
                    app,
                    &options,
                );
            }
        }
    }
    state.hit_rows = regions;
}
fn kanban(frame: &mut Frame, area: Rect, state: &mut BasesState, app: &crate::app::App) {
    let theme = &app.app_theme;
    let Some(loaded) = &state.loaded else {
        return;
    };
    let Ok(result) = &loaded.result else {
        return;
    };
    if result.groups.is_empty() {
        frame.render_widget(
            Paragraph::new(if loaded.document.config.views[loaded.view].group_by.is_some() {"No visible columns. Open configuration to add a group or restore automatic order."}else{"Kanban needs a groupBy property. Open configuration to choose one."}),
            area,
        );
        return;
    }
    let Ok(options) = super::layout_options::CardOptions::parse(
        &loaded.document.config.views[loaded.view],
        font(app).0,
    ) else {
        return;
    };
    let active = state.focused_group().map_or(0, |(group, _)| group);
    let count = (usize::from(area.width) / usize::from(options.width)).max(1);
    let first = active.saturating_sub(count - 1);
    let width = area.width / count as u16;
    for (group_index, group) in result.groups.iter().enumerate().skip(first).take(count) {
        let rect = Rect::new(
            area.x + ((group_index - first) as u16 * width),
            area.y,
            width,
            area.height,
        );
        state.hit_groups.push((rect, group_index));
        if let Some(position) = state
            .items
            .iter()
            .position(|item| matches!(item,Item::Group(index) if *index==group_index))
        {
            state.hit_rows.push((
                Rect::new(rect.x, rect.y, rect.width, 1),
                position,
                state.column,
            ));
        }
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
        if rect.width >= 3
            && loaded.document.config.views[loaded.view]
                .group_by
                .as_ref()
                .is_some_and(|group| {
                    group.property == "file.folder"
                        || super::edits::property_key(&group.property).is_ok()
                })
        {
            frame.render_widget(
                Paragraph::new("+").style(Style::default().fg(theme.accent)),
                Rect::new(rect.right().saturating_sub(2), rect.y, 1, 1),
            );
        }
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
        let height = 4u16
            .saturating_add(options.cover_height(
                inner.width.saturating_sub(2),
                font(app),
                inner.height.saturating_sub(4),
            ))
            .min(inner.height.max(1));
        let count = (usize::from(inner.height) / usize::from(height)).max(1);
        let offset = selected_row.saturating_sub(count - 1);
        for (position, &row) in group.rows.iter().enumerate().skip(offset).take(count) {
            let rect = Rect::new(
                inner.x,
                inner.y + ((position - offset) * usize::from(height)) as u16,
                inner.width,
                height,
            )
            .intersection(inner);
            let item_position = state
                .items
                .iter()
                .position(|item| matches!(item,Item::Row(index) if *index==row))
                .unwrap_or(0);
            state.hit_rows.push((rect, item_position, state.column));
            card(
                frame,
                rect,
                loaded,
                CardSelection {
                    row,
                    property: state.column,
                    focused: item_position == state.selected,
                    checked: state.selected_ids.contains(&result.rows[row].id),
                },
                &mut state.media,
                app,
                &options,
            );
        }
    }
}
fn font(app: &crate::app::App) -> (u16, u16) {
    app.image_picker.as_ref().map_or((8, 16), |picker| {
        let font = picker.font_size();
        if font.width == 1 && font.height == 2 {
            (8, 16)
        } else {
            (font.width.max(1), font.height.max(1))
        }
    })
}
struct CardSelection {
    row: usize,
    property: usize,
    focused: bool,
    checked: bool,
}
fn card(
    frame: &mut Frame,
    rect: Rect,
    loaded: &super::worker::LoadedBase,
    selection: CardSelection,
    media: &mut super::media::PublicImages,
    app: &crate::app::App,
    options: &super::layout_options::CardOptions,
) {
    let result = loaded
        .result
        .as_ref()
        .expect("card belongs to successful query");
    let row = &result.rows[selection.row];
    let theme = &app.app_theme;
    let style = if selection.focused {
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.highlight_bg)
    } else {
        theme.bg_style().fg(theme.text)
    };
    let title = loaded
        .columns
        .first()
        .and_then(|property| row.values.get(property))
        .map_or_else(String::new, display)
        .replace('\n', " ");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(style)
        .border_style(
            Style::default().fg(if selection.focused || selection.checked {
                theme.accent
            } else {
                theme.border
            }),
        );
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let height = options
        .cover_height(inner.width, font(app), inner.height.saturating_sub(2))
        .min(inner.height.saturating_sub(1));
    let cover = Rect::new(inner.x, inner.y, inner.width, height);
    let body = Rect::new(
        inner.x,
        inner.y.saturating_add(height),
        inner.width,
        inner.height.saturating_sub(height),
    );
    if let Some(property) = &options.image
        && let Some(value) = row.values.get(property)
    {
        media.render(
            frame,
            cover,
            super::media::Cover {
                value,
                host: &row.id,
                fit: options.fit,
            },
            &loaded.metadata,
            app,
        );
    }
    let start = selection
        .property
        .saturating_sub(usize::from(body.height).saturating_sub(1))
        .max(1);
    let lines: Vec<ratatui::text::Line> = loaded
        .columns
        .iter()
        .enumerate()
        .skip(start)
        .take(usize::from(body.height))
        .map(|(index, property)| {
            let label = loaded
                .document
                .config
                .labels
                .get(property)
                .unwrap_or(property);
            let text = format!(
                "{}{}: {}",
                if selection.focused && index == selection.property {
                    "› "
                } else {
                    ""
                },
                sanitize_for_terminal(label),
                row.values.get(property).map_or_else(String::new, display)
            );
            ratatui::text::Line::styled(
                text,
                if selection.focused && index == selection.property {
                    style.add_modifier(Modifier::BOLD)
                } else {
                    style
                },
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(style), body);
}
