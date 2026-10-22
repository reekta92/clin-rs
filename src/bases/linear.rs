//! Table/List geometry shares one viewport and the same property hit regions as rendering.
use super::{BasesState, Item, display, group_line, wrap_cell};
use crate::bases::layout_options::{LayoutOptions, ListOptions, Markers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn viewport(
    items: &[Item],
    selected: usize,
    offset: usize,
    height: u16,
    item_height: impl Fn(usize, Item) -> u16,
) -> (usize, Vec<(usize, u16)>) {
    if height == 0 || items.is_empty() {
        return (0, Vec::new());
    }
    let selected = selected.min(items.len() - 1);
    let mut offset = offset
        .min(selected)
        .max(selected.saturating_sub(usize::from(height)));
    let mut used: u32 = items[offset..=selected]
        .iter()
        .enumerate()
        .map(|(index, item)| u32::from(item_height(offset + index, *item).clamp(1, height)))
        .sum();
    while used > u32::from(height) && offset < selected {
        used -= u32::from(item_height(offset, items[offset]).clamp(1, height));
        offset += 1;
    }
    let mut remaining = height;
    let mut visible = Vec::new();
    for (position, item) in items.iter().enumerate().skip(offset) {
        if remaining == 0 {
            break;
        }
        let height = item_height(position, *item).clamp(1, remaining);
        visible.push((position, height));
        remaining -= height;
    }
    (offset, visible)
}

fn content_viewport(
    items: &[Item],
    offset: usize,
    height: u16,
    item_height: impl Fn(usize, Item) -> u16,
) -> Vec<(usize, u16, u16)> {
    let mut top = 0;
    let mut visible = Vec::new();
    for (position, item) in items.iter().enumerate() {
        let full_height = item_height(position, *item).max(1);
        let end = top + usize::from(full_height);
        if end > offset && top < offset + usize::from(height) {
            visible.push((position, full_height, offset.saturating_sub(top) as u16));
        }
        top = end;
        if top >= offset + usize::from(height) {
            break;
        }
    }
    visible
}

fn table_item_height(state: &BasesState, item: Item, widths: &[u16]) -> u16 {
    let Item::Row(index) = item else {
        return 1;
    };
    let loaded = state.loaded.as_ref().expect("loaded table");
    let values = &loaded.result.as_ref().expect("valid table").rows[index].values;
    loaded
        .columns
        .iter()
        .zip(widths)
        .map(|(property, &width)| {
            let text = values.get(property).map_or_else(String::new, display);
            wrap_cell(&text, width.saturating_sub(u16::from(width > 1)), u16::MAX)
                .lines()
                .count()
                .max(1) as u16
        })
        .max()
        .unwrap_or(1)
}

pub(super) fn content_height(state: &BasesState, width: u16, cell_width: u16) -> usize {
    let Some(loaded) = &state.loaded else {
        return 1;
    };
    let Some(options) = &loaded.layout_options else {
        return 1;
    };
    match options {
        LayoutOptions::Table(options) => {
            let widths = options.fitted_widths(&loaded.columns, width, cell_width);
            1 + usize::from(
                state
                    .result()
                    .is_some_and(|result| !result.summaries.is_empty()),
            ) + state
                .items
                .iter()
                .map(|item| {
                    if matches!(item, Item::Row(_)) {
                        usize::from(options.height.max(table_item_height(state, *item, &widths)))
                    } else {
                        1
                    }
                })
                .sum::<usize>()
        }
        LayoutOptions::List(options) => state
            .items
            .iter()
            .map(|item| match item {
                Item::Group(_) => 1,
                Item::Row(index) => list_row(state, *index, options, width, u16::MAX, 0, true)
                    .lines
                    .len(),
            })
            .sum::<usize>()
            .max(1),
        LayoutOptions::Other => 1,
    }
}

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    state: &mut BasesState,
    app: &crate::app::App,
    content_offset: Option<usize>,
) {
    if area.is_empty() {
        return;
    }
    state.body_area = area;
    state.hit_rows.clear();
    state.hit_groups.clear();
    let gutter = area;
    let area = Rect {
        width: area
            .width
            .saturating_sub(u16::from(content_offset.is_none() && area.width > 1)),
        ..area
    };
    let loaded = state.loaded.as_ref().expect("loaded view");
    let result = loaded.result.as_ref().expect("valid query");
    let theme = &app.app_theme;
    let selected_style = Style::default()
        .fg(theme.highlight_fg)
        .bg(theme.highlight_bg);
    let heading = Style::default()
        .fg(theme.heading)
        .add_modifier(Modifier::BOLD);
    match loaded.layout_options.as_ref() {
        Some(LayoutOptions::Table(options)) => {
            let widths = options.fitted_widths(&loaded.columns, area.width, super::font(app).0);
            let header = if let Some(offset) = content_offset {
                u16::from(offset == 0)
            } else {
                u16::from(area.height > 1 || state.items.is_empty())
            };
            let footer = if let Some(offset) = content_offset {
                u16::from(
                    !result.summaries.is_empty()
                        && content_height(state, area.width, super::font(app).0).saturating_sub(1)
                            >= offset
                        && content_height(state, area.width, super::font(app).0)
                            <= offset + usize::from(area.height),
                )
            } else {
                u16::from(!result.summaries.is_empty() && area.height > header + 1)
            };
            let body = Rect::new(
                area.x,
                area.y + header,
                area.width,
                area.height - header - footer,
            );
            let item_height = |_: usize, item| {
                if matches!(item, Item::Row(_)) {
                    if content_offset.is_some() {
                        options.height.max(table_item_height(state, item, &widths))
                    } else {
                        options.height
                    }
                } else {
                    1
                }
            };
            let (offset, visible) = if let Some(offset) = content_offset {
                (
                    0,
                    content_viewport(
                        &state.items,
                        offset.saturating_sub(1),
                        body.height,
                        item_height,
                    ),
                )
            } else {
                let (offset, visible) = viewport(
                    &state.items,
                    state.selected,
                    state.offset,
                    body.height,
                    item_height,
                );
                (
                    offset,
                    visible
                        .into_iter()
                        .map(|(position, height)| (position, height, 0))
                        .collect(),
                )
            };
            state.offset = offset;
            state.scrollbar = crate::ui::scrollbar::ScrollbarMeta {
                track: crate::ui::scrollbar::track_rect(Rect {
                    y: body.y,
                    height: body.height,
                    ..gutter
                }),
                content_len: state.items.len(),
                viewport_len: visible.len(),
            };
            let selected = state.column.min(loaded.columns.len().saturating_sub(1));
            let width = |index: usize| {
                if content_offset.is_some() {
                    widths[index]
                } else {
                    options
                        .width(&loaded.columns[index], super::font(app).0)
                        .min(area.width)
                }
            };
            let mut start = if content_offset.is_some() {
                0
            } else {
                selected
            };
            let mut used = width(selected);
            while content_offset.is_none()
                && start > 0
                && used.saturating_add(width(start - 1)) <= area.width
            {
                start -= 1;
                used += width(start);
            }
            let mut x = area.x;
            for (column, key) in loaded.columns.iter().enumerate().skip(start) {
                if x >= area.right() {
                    break;
                }
                let width = width(column).min(area.right() - x);
                // Keep the final terminal cell usable when the viewport is only one cell wide.
                let text_width = width.saturating_sub(u16::from(width > 1));
                if header > 0 {
                    let label = crate::fsutil::sanitize_for_terminal(
                        loaded.document.config.labels.get(key).unwrap_or(key),
                    );
                    frame.render_widget(
                        Paragraph::new(label).style(heading),
                        Rect::new(x, area.y, text_width, 1),
                    );
                }
                if footer > 0 {
                    let text = result
                        .summaries
                        .get(key)
                        .map(|value| format!("Σ {}", display(value)))
                        .unwrap_or_default();
                    frame.render_widget(
                        Paragraph::new(text).style(heading),
                        Rect::new(x, area.bottom() - 1, text_width, 1),
                    );
                }
                let mut y = body.y;
                for &(position, full_height, skipped) in &visible {
                    let height = (full_height - skipped).min(body.bottom().saturating_sub(y));
                    let rect = Rect::new(x, y, width, height);
                    state.hit_rows.push((rect, position, column));
                    let (text, style) = match state.items[position] {
                        Item::Group(index) => {
                            let summary = result.groups[index]
                                .summaries
                                .get(key)
                                .map(|value| format!("Σ {}", display(value)))
                                .unwrap_or_default();
                            let text = if column == start {
                                format!("{} {summary}", group_line(state, index))
                            } else {
                                summary
                            };
                            (
                                text,
                                if position == state.selected {
                                    selected_style
                                } else {
                                    heading
                                },
                            )
                        }
                        Item::Row(index) => {
                            let text = result.rows[index]
                                .values
                                .get(key)
                                .map_or_else(String::new, display);
                            let style = if position == state.selected && column == selected {
                                selected_style
                            } else if state.selected_ids.contains(&result.rows[index].id) {
                                Style::default().fg(theme.accent)
                            } else {
                                Style::default().fg(theme.text)
                            };
                            (wrap_cell(&text, text_width, full_height), style)
                        }
                    };
                    let hovered = app
                        .mouse_pos
                        .is_some_and(|(x, y)| rect.contains((x, y).into()));
                    let focused = position == state.selected
                        && (column == selected || matches!(state.items[position], Item::Group(_)));
                    let style = if hovered && !focused {
                        style.patch(theme.hover_style())
                    } else {
                        style
                    };
                    frame.render_widget(
                        Paragraph::new(text).style(style).scroll((skipped, 0)),
                        Rect::new(x, y, text_width, height),
                    );
                    y += height;
                }
                x += width;
            }
        }
        Some(LayoutOptions::List(options)) => {
            let row = |position, index| {
                list_row(
                    state,
                    index,
                    options,
                    area.width,
                    if content_offset.is_some() {
                        u16::MAX
                    } else {
                        area.height
                    },
                    if position == state.selected {
                        state.column
                    } else {
                        0
                    },
                    content_offset.is_some(),
                )
            };
            let item_height = |position, item| match item {
                Item::Group(_) => 1,
                Item::Row(index) => row(position, index).lines.len() as u16,
            };
            let (offset, visible) = if let Some(offset) = content_offset {
                (
                    0,
                    content_viewport(&state.items, offset, area.height, item_height),
                )
            } else {
                let (offset, visible) = viewport(
                    &state.items,
                    state.selected,
                    state.offset,
                    area.height,
                    item_height,
                );
                (
                    offset,
                    visible
                        .into_iter()
                        .map(|(position, height)| (position, height, 0))
                        .collect(),
                )
            };
            let mut y = area.y;
            // Build before mutating hit regions: list text and its hits always share geometry.
            let rows: Vec<_> = visible
                .iter()
                .map(|&(position, height, skipped)| {
                    let content = match state.items[position] {
                        Item::Group(index) => ListRow {
                            lines: vec![Line::from(group_line(state, index))],
                            hits: vec![(Rect::new(0, 0, area.width, 1), state.column)],
                        },
                        Item::Row(index) => row(position, index),
                    };
                    (position, height, skipped, content)
                })
                .collect();
            state.offset = offset;
            state.scrollbar = crate::ui::scrollbar::ScrollbarMeta {
                track: crate::ui::scrollbar::track_rect(gutter),
                content_len: state.items.len(),
                viewport_len: visible.len(),
            };
            for (position, full_height, skipped, content) in rows {
                let height = (full_height - skipped).min(area.bottom().saturating_sub(y));
                let style = if position == state.selected {
                    selected_style
                } else if matches!(state.items[position], Item::Group(_)) {
                    heading
                } else if matches!(state.items[position], Item::Row(index) if state.selected_ids.contains(&result.rows[index].id))
                {
                    Style::default().fg(theme.accent)
                } else {
                    Style::default().fg(theme.text)
                };
                for (rect, column) in content.hits {
                    if rect.y >= skipped && rect.y - skipped < height {
                        state.hit_rows.push((
                            Rect::new(area.x + rect.x, y + rect.y - skipped, rect.width, 1),
                            position,
                            column,
                        ));
                    }
                }
                let hovered = app.mouse_pos.is_some_and(|(x, mouse_y)| {
                    Rect::new(area.x, y, area.width, height).contains((x, mouse_y).into())
                });
                let style = if hovered && position != state.selected {
                    style.patch(theme.hover_style())
                } else {
                    style
                };
                frame.render_widget(
                    Paragraph::new(content.lines)
                        .style(style)
                        .scroll((skipped, 0)),
                    Rect::new(area.x, y, area.width, height),
                );
                y += height;
            }
        }
        _ => {}
    }
    if content_offset.is_some() || gutter.width <= 1 {
        state.scrollbar = crate::ui::scrollbar::ScrollbarMeta::default();
        return;
    }
    crate::ui::scrollbar::draw_scrollbar(
        frame,
        state.scrollbar.track,
        state.scrollbar.content_len,
        state.scrollbar.viewport_len,
        state.selected,
        state.items.len().saturating_sub(1),
        theme,
        app.mouse_pos,
    );
}

struct ListRow {
    lines: Vec<Line<'static>>,
    hits: Vec<(Rect, usize)>,
}
fn list_row(
    state: &BasesState,
    index: usize,
    options: &ListOptions,
    width: u16,
    height: u16,
    focus: usize,
    fit_content: bool,
) -> ListRow {
    let loaded = state.loaded.as_ref().expect("loaded list");
    let result = loaded.result.as_ref().expect("valid list");
    let values = &result.rows[index].values;
    let number = loaded.row_numbers[index];
    let marker = match options.markers {
        Markers::Bullet => "• ".into(),
        Markers::Number => format!("{number}. "),
        Markers::None => String::new(),
    };
    let mut row = ListRow {
        lines: vec![Line::default()],
        hits: Vec::new(),
    };
    let mut x = 0;
    let mut y = 0;
    // Keep title and focused property visible even when earlier properties exceed the viewport.
    let start = if focus > 0 && !fit_content { focus } else { 1 };
    let columns = std::iter::once(if height == 1 && focus > 0 { focus } else { 0 })
        .chain((start..loaded.columns.len()).filter(|column| *column != focus || height != 1));
    for column in columns {
        let prefix = if column == focus && focus > 0 && !fit_content {
            String::new()
        } else if column == 0 {
            marker.clone()
        } else if options.indent {
            "  ".into()
        } else {
            crate::fsutil::sanitize_for_terminal(&options.separator).into_owned()
        };
        if column > 0 && (options.indent || (column == focus && focus > 0 && !fit_content)) && x > 0
        {
            x = 0;
            y += 1;
        }
        let value = values
            .get(&loaded.columns[column])
            .map_or_else(String::new, display);
        let text = format!("{prefix}{value}");
        let text = if text.is_empty() {
            " ".to_owned()
        } else {
            text
        };
        for cluster in text.graphemes(true) {
            let size = cluster.width() as u16;
            if size > width {
                continue;
            }
            if column == 0 && focus > 0 && !fit_content && x + size > width {
                break;
            }
            if x + size > width {
                x = 0;
                y += 1;
            }
            if y >= height {
                return row;
            }
            while row.lines.len() <= usize::from(y) {
                row.lines.push(Line::default());
            }
            let style = if column == focus {
                Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default()
            };
            row.lines[usize::from(y)]
                .spans
                .push(Span::styled(cluster.to_owned(), style));
            if size > 0 {
                if let Some((last, last_column)) = row.hits.last_mut()
                    && *last_column == column
                    && last.y == y
                    && last.right() == x
                {
                    last.width += size;
                } else {
                    row.hits.push((Rect::new(x, y, size, 1), column));
                }
            }
            x += size;
        }
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewport_clips_tall_rows_and_counts_group_headers_in_cells() {
        let items = [Item::Group(0), Item::Row(0), Item::Row(1)];
        let height = |_, item| if matches!(item, Item::Group(_)) { 1 } else { 8 };
        assert_eq!(viewport(&items, 1, 0, 3, height), (1, vec![(1, 3)]));
        assert_eq!(viewport(&items, 0, 0, 3, height), (0, vec![(0, 1), (1, 2)]));
        assert_eq!(viewport(&items, 2, 0, 0, height), (0, vec![]));
        assert_eq!(viewport(&items, 2, 0, 1, height), (2, vec![(2, 1)]));
    }
}
