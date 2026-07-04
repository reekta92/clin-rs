use crate::app_theme::AppThemeColors;
use crate::base::eval_pipeline::SummaryValue;
use crate::base_view::state::BaseState;
use crate::keybinds::{BaseAction, Keybinds};
use ratatui::widgets::canvas::Canvas;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Cell, Clear, List, ListItem, Paragraph, Row, Table},
};

#[allow(clippy::collapsible_if)]
pub fn draw_base_view(
    frame: &mut Frame,
    area: Rect,
    state: &BaseState,
    theme: &AppThemeColors,
    keybinds: &Keybinds,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let main_area = chunks[0];
    let hint_area = chunks[1];

    if let Some(err) = &state.error {
        let paragraph = Paragraph::new(format!("Error: {}", err))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(theme.destructive)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().style(theme.bg_style()));
        frame.render_widget(paragraph, main_area);
    } else if state.total_rows() == 0 {
        let paragraph = Paragraph::new("No files match this base.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.muted))
            .block(Block::default().style(theme.bg_style()));
        frame.render_widget(paragraph, main_area);
    } else {
        use crate::base::model::ViewType;
        match state.active_view().map(|v| v.r#type) {
            Some(ViewType::List) => draw_base_list_view(frame, main_area, state, theme),
            Some(ViewType::Cards) => draw_base_cards_view(frame, main_area, state, theme),
            Some(ViewType::Map) => draw_base_map_view(frame, main_area, state, theme),
            _ => {
                // 1. Calculate content-aware column widths
                let mut col_widths = vec![0; state.result.columns.len()];
                for (col_idx, col) in state.result.columns.iter().enumerate() {
                    let mut len = col.display.len();
                    if let Some((ref sort_col, _)) = state.sort {
                        if sort_col == &col.key {
                            len += 2;
                        }
                    }
                    col_widths[col_idx] = len;
                }

                for group in &state.result.groups {
                    for row in &group.rows {
                        for (col_idx, col) in state.result.columns.iter().enumerate() {
                            let val_str = state.row_value_display(row, &col.key);
                            col_widths[col_idx] = col_widths[col_idx].max(val_str.len());
                        }
                    }
                }

                // Clamp widths to a reasonable range
                let col_widths: Vec<usize> =
                    col_widths.into_iter().map(|w| w.clamp(6, 30)).collect();

                // 2. Compute horizontal window (start..end)
                let total_cols = state.result.columns.len();
                let cursor_col = state.cursor_col.min(total_cols.saturating_sub(1));
                let mut start = state.col_offset.get().min(cursor_col);

                // Adjust start to the right if the cursor column is beyond the screen width
                loop {
                    let width_sum: usize = col_widths[start..=cursor_col].iter().sum();
                    if width_sum <= main_area.width as usize || start == cursor_col {
                        break;
                    }
                    start += 1;
                }
                state.col_offset.set(start);

                // Find end
                let mut end = cursor_col + 1;
                let mut width_sum: usize = col_widths[start..end].iter().sum();
                while end < total_cols {
                    let next_width = col_widths[end];
                    if width_sum + next_width <= main_area.width as usize {
                        width_sum += next_width;
                        end += 1;
                    } else {
                        break;
                    }
                }
                // 3. Build table with sliced columns
                let mut visible_rows = Vec::new();
                let mut data_row_count = 0;
                let mut cursor_visible_idx = None;

                for group in &state.result.groups {
                    if let Some(label) = &group.label {
                        let mut cells =
                            vec![Cell::from(format!("── {} ──", label)).style(
                                Style::default().fg(theme.tag).add_modifier(Modifier::BOLD),
                            )];
                        for _ in 1..(end - start) {
                            cells.push(Cell::from(""));
                        }
                        visible_rows.push(Row::new(cells).style(
                            Style::default().bg(theme.bg_style().bg.unwrap_or(Color::Reset)),
                        ));
                    }

                    for row in &group.rows {
                        let mut cells = Vec::new();
                        for (sub_col_idx, col) in
                            state.result.columns[start..end].iter().enumerate()
                        {
                            let col_idx = start + sub_col_idx;
                            let val_str = state.row_value_display(row, &col.key);
                            let mut cell = Cell::from(val_str);

                            // Highlight cell if selected
                            if data_row_count == state.cursor_row {
                                if col_idx == state.cursor_col {
                                    cell = cell.style(
                                        Style::default()
                                            .bg(theme.highlight_bg)
                                            .fg(theme.highlight_fg)
                                            .add_modifier(Modifier::BOLD),
                                    );
                                } else if let Some(bg) = theme.title_bar_bg() {
                                    cell = cell.style(Style::default().bg(bg));
                                } else {
                                    cell = cell.style(Style::default().bg(Color::DarkGray));
                                }
                            }
                            cells.push(cell);
                        }
                        visible_rows.push(Row::new(cells));
                        if data_row_count == state.cursor_row {
                            cursor_visible_idx = Some(visible_rows.len() - 1);
                        }
                        data_row_count += 1;
                    }
                }

                // Add summaries if present
                let mut has_any_summary = false;
                let mut summary_cells = Vec::new();
                for col in &state.result.columns[start..end] {
                    if let Some(sum_val) = state.result.summaries.get(&col.key) {
                        has_any_summary = true;
                        let display = match sum_val {
                            SummaryValue::Num(n) => {
                                if n.fract() == 0.0 {
                                    format!("{:.0}", n)
                                } else {
                                    format!("{:.2}", n)
                                }
                            }
                            SummaryValue::Str(s) => s.clone(),
                            SummaryValue::None => "".to_string(),
                        };
                        summary_cells.push(Cell::from(display));
                    } else {
                        summary_cells.push(Cell::from(""));
                    }
                }

                if has_any_summary {
                    let divider_cells = vec![Cell::from("───"); end - start];
                    visible_rows
                        .push(Row::new(divider_cells).style(Style::default().fg(theme.muted)));
                    visible_rows.push(
                        Row::new(summary_cells).style(
                            Style::default()
                                .fg(theme.muted)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    );
                }

                let widths: Vec<Constraint> = col_widths[start..end]
                    .iter()
                    .map(|&w| Constraint::Length(w as u16))
                    .collect();

                let header_cells = state.result.columns[start..end].iter().map(|col| {
                    let mut label = col.display.clone();
                    if let Some((ref sort_col, dir)) = state.sort {
                        if sort_col == &col.key {
                            match dir {
                                crate::base::model::SortDirection::Asc => label.push_str(" ▲"),
                                crate::base::model::SortDirection::Desc => label.push_str(" ▼"),
                            }
                        }
                    }
                    Cell::from(label).style(
                        Style::default()
                            .fg(theme.heading)
                            .add_modifier(Modifier::BOLD),
                    )
                });

                let header = Row::new(header_cells)
                    .style(Style::default().bg(theme.bg_style().bg.unwrap_or(Color::Reset)));

                let table = Table::new(visible_rows, widths)
                    .header(header)
                    .block(Block::default().style(theme.bg_style()));

                let mut state_clone = state.table_state.borrow_mut();
                state_clone.select(cursor_visible_idx);
                frame.render_stateful_widget(table, main_area, &mut *state_clone);
            }
        }
    }

    // Render edit popup if active
    if let Some(edit) = &state.edit {
        let hints = crate::ui::popup_hint_line(theme, "Enter confirm · Esc cancel");
        let inner_area = crate::ui::draw_popup_frame(
            frame,
            area,
            &format!("EDITING: {}", edit.prop),
            crate::ui::PopupSize::Small,
            &hints,
            theme,
        );
        frame.render_widget(&edit.input, inner_area);
    }

    // Render raw-edit overlay if active (full-area, replaces main content)
    if let Some(ta) = &state.raw_edit {
        let block = Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(theme.heading))
            .title(" EDITING BASE — Ctrl+S save \u{00b7} Esc cancel ")
            .title_alignment(Alignment::Center)
            .style(theme.bg_style());
        let inner_area = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Clear, inner_area);
        frame.render_widget(ta, inner_area);
    }

    // Hint bar
    let hints_items = vec![
        (keybinds.display_base(BaseAction::Open), "open note"),
        (keybinds.display_base(BaseAction::EditCell), "edit cell"),
        (keybinds.display_base(BaseAction::EditBase), "edit base"),
        (keybinds.display_base(BaseAction::NewNote), "new note"),
        (keybinds.display_base(BaseAction::ExportCsv), "export csv"),
        (keybinds.display_base(BaseAction::CopyTable), "copy"),
        (keybinds.display_base(BaseAction::CycleView), "next view"),
        (keybinds.display_base(BaseAction::SortAsc), "sort asc"),
        (keybinds.display_base(BaseAction::SortDesc), "sort desc"),
        (keybinds.display_base(BaseAction::Refresh), "refresh"),
        (keybinds.display_base(BaseAction::Back), "back"),
        (keybinds.display_base(BaseAction::PageDown), "page down"),
        (keybinds.display_base(BaseAction::JumpToTop), "top"),
    ];

    let mut hint = crate::ui::format_keybind_hints(theme, &hints_items);
    if let Some(status) = &state.status {
        let mut spans = vec![Span::styled(
            format!("{}  |  ", status),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )];
        spans.extend(hint.spans.clone());
        hint = Line::from(spans);
    }
    crate::ui::draw_status_bar(
        frame,
        hint_area,
        theme,
        None,
        hint,
        None,
        state.seq_matcher.pending_display().as_deref(),
    );
}

fn draw_base_list_view(frame: &mut Frame, area: Rect, state: &BaseState, theme: &AppThemeColors) {
    let columns = &state.result.columns;
    let primary_key = columns.first().map(|c| c.key.as_str()).unwrap_or("");

    let mut items: Vec<ListItem> = Vec::new();
    let mut number = 1usize;
    for group in &state.result.groups {
        // Group separator
        if let Some(label) = &group.label {
            items.push(
                ListItem::new(Line::from(format!("── {} ──", label)))
                    .style(Style::default().fg(theme.tag)),
            );
        }

        for row in &group.rows {
            let mut lines = Vec::new();
            // Primary field line (marker + content)
            let primary = if primary_key.is_empty() {
                String::new()
            } else {
                state.row_value_display(row, primary_key)
            };
            let prefix = match state.list_marker {
                crate::base_view::state::ListMarker::Bullet => "\u{2022} ".to_string(),
                crate::base_view::state::ListMarker::Numbered => {
                    format!("{}. ", number)
                }
                crate::base_view::state::ListMarker::None => "  ".to_string(),
            };
            lines.push(Line::from(format!("{}{}", prefix, primary)));
            number += 1;

            // Additional columns (skip primary)
            for col in columns.iter().skip(1) {
                let val = state.row_value_display(row, &col.key);
                if !val.is_empty() {
                    lines.push(Line::from(format!("  {}: {}", col.display, val)));
                }
            }

            items.push(ListItem::new(lines));
        }
    }

    let list = List::new(items)
        .block(Block::default().style(theme.bg_style()))
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
    // Compute items-vec index matching the cursor's data row (accounts for group separators)
    let mut items_idx = 0usize;
    let mut data_seen = 0usize;
    'walk: for group in &state.result.groups {
        if group.label.is_some() {
            items_idx += 1; // separator item
        }
        for _ in &group.rows {
            if data_seen == state.cursor_row {
                break 'walk;
            }
            data_seen += 1;
            items_idx += 1;
        }
    }
    let selected = if state.total_rows() == 0 {
        None
    } else {
        Some(items_idx)
    };
    let mut list_state = ratatui::widgets::ListState::default().with_selected(selected);

    frame.render_stateful_widget(list, area, &mut list_state);
}

const CARD_W: u16 = 28;
const CARD_H: u16 = 7;

/// Shared Cards geometry: (cols, grid_rows, per_screen, first_visible_index).
fn cards_layout(area: Rect, cursor_row: usize) -> (usize, usize, usize, usize) {
    let cols = (area.width / CARD_W).max(1) as usize;
    let grid_rows = (area.height / CARD_H).max(1) as usize;
    let per_screen = (cols * grid_rows).max(1);
    let first = (cursor_row / per_screen) * per_screen;
    (cols, grid_rows, per_screen, first)
}

fn draw_base_cards_view(frame: &mut Frame, area: Rect, state: &BaseState, theme: &AppThemeColors) {
    let (cols, grid_rows, per_screen, first) = cards_layout(area, state.cursor_row);
    state.cards_per_screen.set(per_screen);

    // Build cell rects: split area vertically, then each row horizontally
    let row_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(std::iter::repeat_n(Constraint::Length(CARD_H), grid_rows))
        .split(area);

    let primary_key = state
        .result
        .columns
        .first()
        .map(|c| c.key.as_str())
        .unwrap_or("");

    for data_idx in first..first + per_screen {
        let tile_local = data_idx - first;
        let tile_row = tile_local / cols;
        let tile_col = tile_local % cols;

        if tile_row >= grid_rows {
            break;
        }
        if tile_col >= row_chunks[tile_row].width as usize / CARD_W as usize {
            continue;
        }
        let cell_x = row_chunks[tile_row].x + (tile_col as u16) * CARD_W;
        let cell_y = row_chunks[tile_row].y;
        let card_rect = Rect::new(cell_x, cell_y, CARD_W, CARD_H);

        if data_idx >= state.total_rows() {
            // Leave background — render nothing for this tile
            continue;
        }

        let row = match state.get_row(data_idx) {
            Some(r) => r,
            None => continue,
        };

        // Resolve optional cover color from materialized properties
        let cover_color = ["color", "cover"]
            .iter()
            .find_map(|k| row.values.get(*k))
            .and_then(|v| {
                if let crate::base::expr::Value::Str(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .filter(|s| !s.trim().is_empty())
            .map(|s| crate::ui::resolve_color(&s, theme.accent));

        // Title: primary display or file name
        let title = if !primary_key.is_empty() {
            state.row_value_display(row, primary_key)
        } else {
            row.file.name.clone()
        };

        // Body: up to 4 property lines from remaining columns
        let body_lines: Vec<String> = state
            .result
            .columns
            .iter()
            .skip(1)
            .take(4)
            .filter_map(|col| {
                let val = state.row_value_display(row, &col.key);
                if val.is_empty() {
                    None
                } else {
                    Some(format!("{}: {}", col.display, val))
                }
            })
            .collect();

        let is_cursor = data_idx == state.cursor_row;
        let border_style = if is_cursor {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.border)
        };
        let title_style = if is_cursor {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.heading)
        };

        let block = Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(border_style)
            .title(ratatui::text::Line::from(ratatui::text::Span::styled(
                title,
                title_style,
            )));
        let inner = block.inner(card_rect);

        let body_text = body_lines.join("\n");
        let paragraph = Paragraph::new(body_text)
            .style(Style::default().fg(theme.fg))
            .block(Block::default());

        frame.render_widget(block, card_rect);
        if let Some(color) = cover_color {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);
            let banner =
                Paragraph::new(" ".repeat(inner.width as usize)).style(Style::default().bg(color));
            frame.render_widget(banner, chunks[0]);
            frame.render_widget(paragraph, chunks[1]);
        } else {
            frame.render_widget(paragraph, inner);
        }
    }
}

/// Parse a coordinate value from a property: "lat, lon" string or [lat, lon] list.
fn parse_coord(v: &crate::base::expr::Value) -> Option<(f64, f64)> {
    use crate::base::expr::Value;
    match v {
        Value::Str(s) => {
            let p: Vec<&str> = s.split(',').map(str::trim).collect();
            if p.len() == 2 {
                p[0].parse().ok().zip(p[1].parse().ok())
            } else {
                None
            }
        }
        Value::List(items) if items.len() == 2 => match (&items[0], &items[1]) {
            (Value::Num(lat), Value::Num(lon)) => Some((*lat, *lon)),
            _ => None,
        },
        _ => None,
    }
}

/// Map a few common Lucide icon names to unicode glyphs for Map pins. None → caller's default.
fn lucide_to_glyph(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "map-pin" | "pin" => Some("\u{1f4cd}"), // 📍
        "star" => Some("\u{2605}"),             // ★
        "heart" => Some("\u{2665}"),            // ♥
        "flag" => Some("\u{2691}"),             // ⚑
        "bookmark" => Some("\u{1f516}"),        // 🔖
        "home" => Some("\u{2302}"),             // ⌂
        "circle" => Some("\u{25cf}"),           // ●
        "square" => Some("\u{25a0}"),           // ■
        "diamond" => Some("\u{25c6}"),          // ◆
        _ => None,
    }
}

fn draw_base_map_view(frame: &mut Frame, area: Rect, state: &BaseState, theme: &AppThemeColors) {
    // Collect coordinate points from all rows
    let coord_keys = ["coordinates", "coords", "location"];
    let mut points: Vec<(usize, f64, f64, String, ratatui::style::Color, &'static str)> =
        Vec::new();
    for row_idx in 0..state.total_rows() {
        let row = match state.get_row(row_idx) {
            Some(r) => r,
            None => continue,
        };
        let coord_val = coord_keys
            .iter()
            .find_map(|k| row.values.get(*k))
            .and_then(parse_coord);
        if let Some((lat, lon)) = coord_val {
            let label = state
                .result
                .columns
                .first()
                .map(|c| state.row_value_display(row, &c.key))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| row.file.name.clone());
            let color = ["marker_color", "color"]
                .iter()
                .find_map(|k| row.values.get(*k))
                .and_then(|v| {
                    if let crate::base::expr::Value::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .filter(|s| !s.trim().is_empty())
                .map(|s| crate::ui::resolve_color(&s, theme.accent))
                .unwrap_or(theme.accent);
            let glyph = ["marker_icon", "icon"]
                .iter()
                .find_map(|k| row.values.get(*k))
                .and_then(|v| {
                    if let crate::base::expr::Value::Str(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .and_then(lucide_to_glyph)
                .unwrap_or("\u{25cf}"); // ●
            points.push((row_idx, lat, lon, label, color, glyph));
        }
    }

    if points.is_empty() {
        let msg = "No coordinates found — set a `coordinates: \"lat, lon\"` (or `[lat, lon]`) property on matching notes.";
        let paragraph = Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.muted))
            .block(Block::default().style(theme.bg_style()));
        frame.render_widget(paragraph, area);
        return;
    }

    // Compute bounds
    let lat_min = points.iter().map(|p| p.1).reduce(f64::min).unwrap_or(0.0);
    let lat_max = points.iter().map(|p| p.1).reduce(f64::max).unwrap_or(0.0);
    let lon_min = points.iter().map(|p| p.2).reduce(f64::min).unwrap_or(0.0);
    let lon_max = points.iter().map(|p| p.2).reduce(f64::max).unwrap_or(0.0);

    let (lat_min, lat_max) = if lat_min == lat_max {
        (lat_min - 0.01, lat_max + 0.01)
    } else {
        let margin = (lat_max - lat_min) * 0.05;
        (lat_min - margin, lat_max + margin)
    };
    let (lon_min, lon_max) = if lon_min == lon_max {
        (lon_min - 0.01, lon_max + 0.01)
    } else {
        let margin = (lon_max - lon_min) * 0.05;
        (lon_min - margin, lon_max + margin)
    };

    // Header line: show cursor point's label + coords
    let header_text =
        if let Some((_, lat, lon, label, ..)) = points.iter().find(|p| p.0 == state.cursor_row) {
            format!("Location: {}  (lat {:.4}, lon {:.4})", label, lat, lon)
        } else {
            "No coordinate for this row".to_string()
        };

    // Split area: 1 line for header, rest for canvas
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    let header_area = chunks[0];
    let canvas_area = chunks[1];

    let header_span = ratatui::text::Span::styled(
        header_text,
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(Paragraph::new(header_span), header_area);

    // Canvas scatter plot
    let canvas = Canvas::default()
        .block(Block::default().style(theme.bg_style()))
        .x_bounds([lon_min, lon_max])
        .y_bounds([lat_min, lat_max])
        .marker(ratatui::symbols::Marker::Braille)
        .paint(|ctx: &mut ratatui::widgets::canvas::Context| {
            for (idx, lat, lon, _, color, glyph) in &points {
                let ch = if *idx == state.cursor_row {
                    "\u{25c9}"
                } else {
                    glyph
                };
                let style = if *idx == state.cursor_row {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(*color)
                };
                ctx.print(*lon, *lat, ratatui::text::Span::styled(ch, style));
            }
        });
    frame.render_widget(canvas, canvas_area);
}

pub fn hit_test_cards(
    state: &BaseState,
    area: Rect,
    mouse_row: u16,
    mouse_col: u16,
) -> Option<usize> {
    if state.total_rows() == 0 {
        return None;
    }

    let (cols, grid_rows, _per_screen, first) = cards_layout(area, state.cursor_row);

    let tile_row = ((mouse_row as i32 - area.y as i32) / CARD_H as i32).max(0) as u16;
    let tile_col = ((mouse_col as i32 - area.x as i32) / CARD_W as i32).max(0) as u16;

    if tile_row as usize >= grid_rows || tile_col as usize >= cols {
        return None;
    }

    let data_row = first + (tile_row as usize) * cols + (tile_col as usize);
    if data_row < state.total_rows() {
        Some(data_row)
    } else {
        None
    }
}
pub fn hit_test(
    state: &BaseState,
    area: Rect,
    mouse_row: u16,
    mouse_col: u16,
) -> Option<(usize, usize)> {
    if state.total_rows() == 0 {
        return None;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    let main_area = chunks[0];

    // Compute widths
    let mut col_widths = vec![0; state.result.columns.len()];
    for (col_idx, col) in state.result.columns.iter().enumerate() {
        let mut len = col.display.len();
        if let Some((ref sort_col, _)) = state.sort
            && sort_col == &col.key
        {
            len += 2;
        }
        col_widths[col_idx] = len;
    }
    for group in &state.result.groups {
        for row in &group.rows {
            for (col_idx, col) in state.result.columns.iter().enumerate() {
                let val_str = state.row_value_display(row, &col.key);
                col_widths[col_idx] = col_widths[col_idx].max(val_str.len());
            }
        }
    }
    let col_widths: Vec<usize> = col_widths.into_iter().map(|w| w.clamp(6, 30)).collect();

    let total_cols = state.result.columns.len();
    let cursor_col = state.cursor_col.min(total_cols.saturating_sub(1));
    let start = state.col_offset.get().min(cursor_col);

    let mut end = cursor_col + 1;
    let mut width_sum: usize = col_widths[start..end].iter().sum();
    while end < total_cols {
        let next_width = col_widths[end];
        if width_sum + next_width <= main_area.width as usize {
            width_sum += next_width;
            end += 1;
        } else {
            break;
        }
    }

    let body_top = main_area.y + 1;
    if mouse_row < body_top || mouse_row >= main_area.bottom() {
        return None;
    }

    let target_visible_idx = (mouse_row - body_top) as usize;
    let mut visible_idx = 0;
    let mut target_data_row = None;
    let mut data_row_count = 0;

    'outer: for group in &state.result.groups {
        if group.label.is_some() {
            if visible_idx == target_visible_idx {
                break 'outer;
            }
            visible_idx += 1;
        }
        for _row in &group.rows {
            if visible_idx == target_visible_idx {
                target_data_row = Some(data_row_count);
                break 'outer;
            }
            visible_idx += 1;
            data_row_count += 1;
        }
    }

    let data_row = target_data_row?;
    let mut current_x = main_area.x as usize;
    let mut target_col = None;

    for (sub_col_idx, &width) in col_widths[start..end].iter().enumerate() {
        let col_idx = start + sub_col_idx;
        let next_x = current_x + width;
        if (mouse_col as usize) >= current_x && (mouse_col as usize) < next_x {
            target_col = Some(col_idx);
            break;
        }
        current_x = next_x;
    }

    let col = if (mouse_col as usize) >= current_x {
        if end > start { end - 1 } else { 0 }
    } else {
        target_col?
    };

    Some((data_row, col))
}

pub fn hit_test_list(state: &BaseState, area: Rect, mouse_row: u16) -> Option<usize> {
    if state.total_rows() == 0 {
        return None;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    let main_area = chunks[0];
    let body_start = main_area.y;

    if mouse_row < body_start || mouse_row >= main_area.bottom() {
        return None;
    }

    let relative_row = (mouse_row - body_start) as usize;
    let mut consumed = 0usize;
    let additional_cols: Vec<&str> = state
        .result
        .columns
        .iter()
        .skip(1)
        .map(|c| c.key.as_str())
        .collect();

    for (group_idx, group) in state.result.groups.iter().enumerate() {
        // Group separator line
        if group.label.is_some() {
            if consumed == relative_row {
                return None; // clicked on separator
            }
            consumed += 1;
        }

        for (row_idx, row) in group.rows.iter().enumerate() {
            // Row height: 1 primary line + 1 per non-empty additional column
            let extra_lines = additional_cols
                .iter()
                .filter(|&&key| !state.row_value_display(row, key).is_empty())
                .count();
            let row_height = 1 + extra_lines;

            if relative_row >= consumed && relative_row < consumed + row_height {
                // Compute global data row index
                let mut data_idx = 0usize;
                for (g_idx, g) in state.result.groups.iter().enumerate() {
                    if g_idx < group_idx {
                        data_idx += g.rows.len();
                    } else if g_idx == group_idx {
                        data_idx += row_idx;
                        break;
                    }
                }
                return Some(data_idx);
            }
            consumed += row_height;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::expr::Value;
    use ratatui::layout::Rect;

    #[test]
    fn parse_coord_string_and_list() {
        assert_eq!(
            parse_coord(&Value::Str("48.86, 2.35".into())),
            Some((48.86, 2.35))
        );
        assert_eq!(
            parse_coord(&Value::List(vec![Value::Num(1.0), Value::Num(2.0)])),
            Some((1.0, 2.0))
        );
    }
    #[test]
    fn parse_coord_rejects_bad() {
        assert_eq!(parse_coord(&Value::Str("bad".into())), None);
        assert_eq!(parse_coord(&Value::Str("1".into())), None); // single value
        assert_eq!(parse_coord(&Value::List(vec![Value::Num(1.0)])), None); // wrong len
        assert_eq!(parse_coord(&Value::Num(1.0)), None);
        assert_eq!(parse_coord(&Value::Null), None);
    }
    #[test]
    fn cards_layout_paging() {
        // 80 wide / 24 tall, CARD_W=28 CARD_H=7 → cols=2, grid_rows=3, per_screen=6
        let area = Rect::new(0, 0, 80, 24);
        assert_eq!(cards_layout(area, 0), (2, 3, 6, 0));
        assert_eq!(cards_layout(area, 5), (2, 3, 6, 0)); // still page 0
        assert_eq!(cards_layout(area, 6), (2, 3, 6, 6)); // page 1
        assert_eq!(cards_layout(area, 13), (2, 3, 6, 12)); // page 2
    }
    #[test]
    fn cards_layout_tiny_terminal() {
        let area = Rect::new(0, 0, 28, 7); // exactly one tile
        assert_eq!(cards_layout(area, 0), (1, 1, 1, 0));
        assert_eq!(cards_layout(area, 5), (1, 1, 1, 5)); // per_screen=1 → first==cursor
    }
}
