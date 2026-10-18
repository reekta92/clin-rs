//! Content-sized embedded layouts; only regions intersecting the preview are painted.
use super::*;
use crate::bases::layout_options::CardOptions;
use ratatui::buffer::Buffer;

enum RegionKind {
    Item(usize),
    Column(usize),
}

struct Region {
    top: usize,
    height: usize,
    rect: Rect,
    kind: RegionKind,
}

struct Content {
    height: usize,
    regions: Vec<Region>,
}

fn card_height(
    state: &BasesState,
    row: usize,
    width: u16,
    options: &CardOptions,
    cell: (u16, u16),
) -> u16 {
    let loaded = state.loaded.as_ref().expect("loaded cards");
    let values = &state.result().expect("valid cards").rows[row].values;
    let inner_width = width.saturating_sub(2);
    let properties: usize = loaded
        .columns
        .iter()
        .skip(1)
        .map(|property| {
            let label = sanitize_for_terminal(
                loaded
                    .document
                    .config
                    .labels
                    .get(property)
                    .unwrap_or(property),
            );
            let text = format!(
                "› {label}: {}",
                values.get(property).map_or_else(String::new, display)
            );
            wrap_cell(&text, inner_width, u16::MAX)
                .lines()
                .count()
                .max(1)
        })
        .sum();
    let cover = options.cover_height(inner_width, cell, i16::MAX as u16);
    usize::from(cover)
        .saturating_add(properties.max(2))
        .saturating_add(2)
        .clamp(4, usize::from(u16::MAX)) as u16
}

fn cards_layout(state: &BasesState, width: u16, cell: (u16, u16)) -> Content {
    let loaded = state.loaded.as_ref().expect("loaded cards");
    let options = CardOptions::parse(&loaded.document.config.views[loaded.view], cell.0)
        .expect("validated card options");
    let columns = usize::from((width / options.width).max(1));
    let column_width = width / columns as u16;
    let mut content = Content {
        height: 0,
        regions: Vec::new(),
    };
    if matches!(state.layout(), Layout::Kanban) {
        let result = state.result().expect("valid kanban");
        let positions: std::collections::HashMap<_, _> = state
            .items
            .iter()
            .enumerate()
            .filter_map(|(position, item)| {
                if let Item::Row(row) = item {
                    Some((*row, position))
                } else {
                    None
                }
            })
            .collect();
        for (batch, groups) in result.groups.chunks(columns).enumerate() {
            let batch_start = content.regions.len();
            let top = content.height;
            let mut bottom = top;
            for (column, group) in groups.iter().enumerate() {
                let group_index = batch * columns + column;
                let x = column as u16 * column_width;
                let mut y = top + 1;
                for row in &group.rows {
                    let height =
                        card_height(state, *row, column_width.saturating_sub(2), &options, cell);
                    content.regions.push(Region {
                        top: y,
                        height: usize::from(height),
                        rect: Rect::new(
                            x.saturating_add(1),
                            0,
                            column_width.saturating_sub(2),
                            height,
                        ),
                        kind: RegionKind::Item(positions[row]),
                    });
                    y += usize::from(height);
                }
                bottom = bottom.max((y + 1).max(top + 3));
                content.regions.push(Region {
                    top,
                    height: 0,
                    rect: Rect::new(x, 0, column_width, 0),
                    kind: RegionKind::Column(group_index),
                });
            }
            // Column borders are drawn before cards, with each batch sharing one baseline.
            for region in &mut content.regions[batch_start..] {
                if matches!(region.kind, RegionKind::Column(_)) {
                    region.height = bottom - top;
                }
            }
            content.height = bottom + 1;
        }
    } else {
        let mut column = 0;
        let mut row_height = 0;
        for (position, item) in state.items.iter().enumerate() {
            if matches!(item, Item::Group(_)) {
                if column > 0 {
                    content.height += row_height + 1;
                    column = 0;
                    row_height = 0;
                }
                content.regions.push(Region {
                    top: content.height,
                    height: 1,
                    rect: Rect::new(0, 0, width, 1),
                    kind: RegionKind::Item(position),
                });
                content.height += 1;
            } else if let Item::Row(row) = item {
                let card_width = column_width.saturating_sub(1).max(1);
                let height = card_height(state, *row, card_width, &options, cell);
                content.regions.push(Region {
                    top: content.height,
                    height: usize::from(height),
                    rect: Rect::new(column as u16 * column_width, 0, card_width, height),
                    kind: RegionKind::Item(position),
                });
                row_height = row_height.max(usize::from(height));
                column += 1;
                if column == columns {
                    content.height += row_height + 1;
                    column = 0;
                    row_height = 0;
                }
            }
        }
        if column > 0 {
            content.height += row_height + 1;
        }
    }
    if matches!(state.layout(), Layout::Kanban) || matches!(state.items.last(), Some(Item::Row(_)))
    {
        content.height = content.height.saturating_sub(1);
    }
    content.height = content.height.max(1);
    content
}

pub(in crate::bases) fn content_height(state: &BasesState, width: u16, cell: (u16, u16)) -> usize {
    if state.error.is_some() || state.loaded.is_none() {
        return crate::bases::embedding::SLOT_HEIGHT;
    }
    if matches!(state.layout(), Layout::Cards | Layout::Kanban) {
        cards_layout(state, width, cell).height
    } else {
        super::linear::content_height(state, width, cell.0)
    }
}

pub(in crate::bases) fn render_content(
    frame: &mut Frame,
    area: Rect,
    offset: usize,
    state: &mut BasesState,
    app: &crate::app::App,
) {
    if state.error.is_some()
        || state.loaded.is_none()
        || state.items.is_empty() && !matches!(state.layout(), Layout::Kanban)
        || matches!(state.layout(), Layout::Kanban)
            && state
                .result()
                .is_some_and(|result| result.groups.is_empty())
    {
        super::render(frame, area, state, app);
        return;
    }
    if !matches!(state.layout(), Layout::Cards | Layout::Kanban) {
        super::linear::render(frame, area, state, app, Some(offset));
        state.scrollbar = crate::ui::scrollbar::ScrollbarMeta::default();
        return;
    }
    state.body_area = area;
    state.hit_rows.clear();
    state.hit_groups.clear();
    state.scrollbar = crate::ui::scrollbar::ScrollbarMeta::default();
    state.offset = 0;
    let content = cards_layout(state, area.width, font(app));
    let loaded = state.loaded.as_ref().expect("loaded cards");
    let options = CardOptions::parse(&loaded.document.config.views[loaded.view], font(app).0)
        .expect("validated card options");
    for region in content
        .regions
        .iter()
        .filter(|region| matches!(region.kind, RegionKind::Column(_)))
        .chain(
            content
                .regions
                .iter()
                .filter(|region| matches!(region.kind, RegionKind::Item(_))),
        )
    {
        let start = region.top.max(offset);
        let end = (region.top + region.height).min(offset + usize::from(area.height));
        if start >= end {
            continue;
        }
        let visible = Rect::new(
            area.x + region.rect.x,
            area.y + (start - offset) as u16,
            region.rect.width,
            (end - start) as u16,
        )
        .intersection(area);
        if visible.is_empty() {
            continue;
        }
        if let RegionKind::Column(group_index) = region.kind {
            let loaded = state.loaded.as_ref().expect("loaded cards");
            let group = &loaded.result.as_ref().expect("valid kanban").groups[group_index];
            let focused = state
                .focused_group()
                .is_some_and(|(index, _)| index == group_index);
            let style = Style::default().fg(if focused {
                app.app_theme.accent
            } else {
                app.app_theme.border
            });
            let mut borders = Borders::LEFT | Borders::RIGHT;
            if start == region.top {
                borders |= Borders::TOP;
            }
            if end == region.top + region.height {
                borders |= Borders::BOTTOM;
            }
            let mut block = Block::default().borders(borders).border_style(style);
            if start == region.top {
                block = block.title(format!("{} ({})", display(&group.value), group.rows.len()));
            }
            frame.render_widget(block, visible);
            state.hit_groups.push((visible, group_index));
            if start == region.top
                && let Some(position) = state
                    .items
                    .iter()
                    .position(|item| matches!(item, Item::Group(index) if *index == group_index))
            {
                state.hit_rows.push((
                    Rect {
                        height: 1,
                        ..visible
                    },
                    position,
                    state.column,
                ));
                if visible.width >= 3
                    && loaded.document.config.views[loaded.view]
                        .group_by
                        .as_ref()
                        .is_some_and(|group| {
                            group.property == "file.folder"
                                || crate::bases::edits::property_key(&group.property).is_ok()
                        })
                {
                    frame.render_widget(
                        Paragraph::new("+").style(Style::default().fg(app.app_theme.accent)),
                        Rect::new(visible.right() - 2, visible.y, 1, 1),
                    );
                }
            }
            continue;
        }
        let RegionKind::Item(position) = region.kind else {
            continue;
        };
        state.hit_rows.push((visible, position, state.column));
        let item = state.items[position];
        let group_text = if let Item::Group(index) = item {
            group_line(state, index)
        } else {
            String::new()
        };
        let loaded = state.loaded.as_ref().expect("loaded cards");
        clipped(
            frame,
            visible,
            (start - region.top) as u16,
            region.rect.height,
            |frame, rect| match item {
                Item::Group(_) => frame.render_widget(
                    Paragraph::new(group_text).style(
                        Style::default()
                            .fg(app.app_theme.heading)
                            .add_modifier(Modifier::BOLD),
                    ),
                    rect,
                ),
                Item::Row(row) => card(
                    frame,
                    rect,
                    loaded,
                    CardSelection {
                        row,
                        property: state.column,
                        focused: position == state.selected,
                        checked: state
                            .selected_ids
                            .contains(&loaded.result.as_ref().expect("valid cards").rows[row].id),
                        fit_content: true,
                        hovered: app
                            .mouse_pos
                            .is_some_and(|(x, y)| visible.contains((x, y).into())),
                    },
                    &mut state.media,
                    app,
                    &options,
                ),
            },
        );
        if let Item::Row(row) = item
            && let Some(property) = &options.image
            && let Some(value) = loaded.result.as_ref().expect("valid cards").rows[row]
                .values
                .get(property)
        {
            let height = options.cover_height(
                region.rect.width.saturating_sub(2),
                font(app),
                region.rect.height.saturating_sub(4).min(i16::MAX as u16),
            );
            let top = region.top + 1;
            let start = top.max(offset);
            let end = (top + usize::from(height)).min(offset + usize::from(area.height));
            if start < end {
                let cover = Rect::new(
                    area.x + region.rect.x + 1,
                    area.y + (start - offset) as u16,
                    region.rect.width.saturating_sub(2),
                    (end - start) as u16,
                );
                state.media.render(
                    frame,
                    cover,
                    crate::bases::media::Cover {
                        value,
                        host: &loaded.result.as_ref().expect("valid cards").rows[row].id,
                        fit: options.fit,
                        // Slice the image protocol itself; escape sequences cannot be clipped as cells.
                        clip: Some((height, (start - top) as u16)),
                    },
                    &loaded.metadata,
                    app,
                );
            }
        }
    }
}

fn clipped(
    frame: &mut Frame,
    visible: Rect,
    skipped: u16,
    height: u16,
    draw: impl FnOnce(&mut Frame, Rect),
) {
    if skipped == 0 && visible.height == height {
        draw(frame, visible);
        return;
    }
    let virtual_area = Rect::new(visible.x, 0, visible.width, height);
    let screen = std::mem::replace(frame.buffer_mut(), Buffer::empty(virtual_area));
    draw(frame, virtual_area);
    let rendered = std::mem::replace(frame.buffer_mut(), screen);
    for y in 0..visible.height {
        for x in visible.x..visible.right() {
            frame.buffer_mut()[(x, visible.y + y)] = rendered[(x, skipped + y)].clone();
        }
    }
}
